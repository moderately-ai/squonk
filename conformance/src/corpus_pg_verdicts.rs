// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! PostgreSQL accept/reject verdicts over the broad vendored corpora
//! (run-pg-accept-reject-over-vendored-corpora).
//!
//! The three large vendored corpora — sqlglot identity (955), sqllogictest (373),
//! and sqlglot-complex (238) — are otherwise validated only by parse-acceptance and
//! self-consistency round-trip
//! (`corpus_sqlglot`/`corpus_sqllogictest`/`corpus_complex`). Neither oracle can
//! catch a semantic mis-parse that renders stably (the ROLLUP/CUBE P0 class), nor an
//! accept/reject *divergence* from a real engine. This module routes every vendored
//! statement through the free in-process `pg_query` accept/reject oracle
//! ([`pg_accept_reject_divergence`](crate::pg::pg_accept_reject_divergence)) under
//! the Postgres preset and asserts each statement's verdict either matches
//! PostgreSQL or is named in
//! [`PG_DIVERGENCE_ALLOWLIST`](crate::pg::PG_DIVERGENCE_ALLOWLIST).
//!
//! Both directions are findings, of different classes:
//! - **over-acceptance** (we accept, PostgreSQL rejects) — a validator-correctness
//!   bug: we admit SQL the real engine refuses.
//! - **coverage gap** (PostgreSQL accepts, we reject) — a breadth gap: valid
//!   PostgreSQL our surface does not yet reach.
//!
//! The corpora are multi-dialect — sqlglot identity in particular carries BigQuery /
//! Snowflake / MySQL syntax that is not PostgreSQL at all. That is self-filtering: a
//! non-PostgreSQL statement both parsers reject is *not* a divergence, so only the
//! statements where the two engines actually disagree surface here.
//!
//! The fixtures load through the shared [`verdict_harness`](crate::verdict_harness)
//! corpus loader rather than reaching out of the sibling corpus modules' private
//! `SPEC`s, keeping this PG-verdict subset self-contained — the same
//! independent-consumer split the bench corpus loader
//! (`bench/benches/corpus/mod.rs`) already uses. Per-corpus counts are pinned as
//! anti-vanishing guards, mirroring those loaders.

use crate::pg::{
    PgDivergenceKind, PgMediatedStructuralOracle, PgMediatedVerdict, pg_accept_reject_divergence,
    pg_divergence_allowlisted, postgres_accepts, squonk_accepts,
};
use crate::verdict_harness::{
    DivergenceEntry, assert_entries_are_ticketed, assert_entries_still_diverge,
    sqlglot_complex_statements, sqlglot_identity_lines, sqllogictest_lines,
};
use squonk::dialect::Postgres;
use squonk::error::{Found, ParseError, ParseErrorKind};
use squonk::parse_with;

/// A vendored corpus: a label, its statements in source order, and the pinned count
/// that trips if a statement vanishes from the fixture.
struct Corpus {
    label: &'static str,
    statements: Vec<&'static str>,
    pinned: usize,
}

/// Every vendored corpus, in a fixed order.
fn corpora() -> Vec<Corpus> {
    vec![
        Corpus {
            label: "sqlglot",
            statements: sqlglot_identity_lines(),
            pinned: 955,
        },
        Corpus {
            label: "sqllogictest",
            statements: sqllogictest_lines(),
            pinned: 373,
        },
        Corpus {
            label: "sqlglot-complex",
            statements: sqlglot_complex_statements(),
            pinned: 238,
        },
    ]
}

/// Which way an accept/reject divergence points.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// We accept, PostgreSQL rejects — a validator-correctness bug class.
    OverAcceptance,
    /// PostgreSQL accepts, we reject — a coverage gap.
    CoverageGap,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Direction::OverAcceptance => "over-acceptance (we-accept/PG-rejects)",
            Direction::CoverageGap => "coverage-gap  (PG-accepts/we-reject)",
        }
    }
}

/// One statement on which `pg_query` and `squonk` disagree.
struct Divergence {
    corpus: &'static str,
    direction: Direction,
    sql: &'static str,
    detail: String,
    allowlisted: bool,
}

/// Compute every accept/reject divergence across the vendored corpora, tagging each
/// with its direction and whether it is already allowlisted.
fn divergences() -> Vec<Divergence> {
    let mut out = Vec::new();
    for corpus in corpora() {
        assert_eq!(
            corpus.statements.len(),
            corpus.pinned,
            "{} corpus statement count changed; if intentional, update the pin",
            corpus.label,
        );
        for &sql in &corpus.statements {
            let Some(detail) = pg_accept_reject_divergence(sql) else {
                continue;
            };
            // A divergence means the two engines disagree, so PostgreSQL's verdict
            // alone fixes the direction: PG-accepts ⇒ we-reject (gap); PG-rejects ⇒
            // we-accept (over-acceptance).
            let direction = if postgres_accepts(sql) {
                Direction::CoverageGap
            } else {
                Direction::OverAcceptance
            };
            out.push(Divergence {
                corpus: corpus.label,
                direction,
                sql,
                detail,
                allowlisted: pg_divergence_allowlisted(PgDivergenceKind::AcceptReject, sql),
            });
        }
    }
    out
}

#[test]
fn vendored_corpus_pg_verdicts_are_none_or_allowlisted() {
    let divergences = divergences();

    // Per (corpus, direction) tallies, printed always so a green run still documents
    // the distribution and a drift shows the fresh counts to triage.
    eprintln!("PostgreSQL accept/reject verdicts over vendored corpora:");
    for corpus in ["sqlglot", "sqllogictest", "sqlglot-complex"] {
        for direction in [Direction::OverAcceptance, Direction::CoverageGap] {
            let matching: Vec<&Divergence> = divergences
                .iter()
                .filter(|d| d.corpus == corpus && d.direction == direction)
                .collect();
            let allow = matching.iter().filter(|d| d.allowlisted).count();
            eprintln!(
                "  {corpus:<16} {:<38} {:>4} ({allow} allowlisted, {} untriaged)",
                direction.label(),
                matching.len(),
                matching.len() - allow,
            );
        }
    }

    let untriaged: Vec<&Divergence> = divergences.iter().filter(|d| !d.allowlisted).collect();
    if !untriaged.is_empty() {
        eprintln!("\nUNTRIAGED divergences ({}):", untriaged.len());
        for d in &untriaged {
            eprintln!(
                "  [{}] [{}] {:?}  -- {}",
                d.corpus,
                d.direction.label(),
                d.sql,
                d.detail,
            );
        }
    }

    assert!(
        untriaged.is_empty(),
        "{} untriaged PostgreSQL accept/reject divergence(s) over vendored corpora; \
         triage each into PG_DIVERGENCE_ALLOWLIST (with a ticket + reason) or fix it",
        untriaged.len(),
    );
}

// ---------------------------------------------------------------------------
// Oracle-mediated structural lane (conformance-mediated-structural-lane-pg)
// ---------------------------------------------------------------------------

/// The both-accept subset size over the three vendored corpora — the denominator the
/// mediated structural lane compares over (our Postgres preset AND `pg_query` both parse
/// the statement). Pinned as an anti-vanishing guard, like the per-corpus counts above:
/// a corpus/parser edit that silently shrinks the comparable set would weaken the lane.
/// Re-measured on the current tree (the spike measured 1111; +1 for the PG `-|-`
/// range-adjacency operator, now parsed under the general operator surface —
/// pg-operator-surface-regex-geometric-network — so it both-accepts and enters the lane).
const PG_MEDIATED_BOTH_ACCEPT_PINNED: usize = 1112;

/// Known mediated structural divergences, knowingly tolerated with a ticket — the same
/// exact-SQL, staleness-enforced ledger discipline as `PG_DIVERGENCE_ALLOWLIST` and the
/// sqlite/duckdb over-acceptance ledgers. A mediated mismatch implicates the parser OR
/// the renderer, so an entry parked here (rather than fixed) must record which. Currently
/// empty: the spike measured 0 mismatch / 0 unparseable over the both-accept subset.
const PG_MEDIATED_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[];

/// Whether `sql` is named in the mediated divergence ledger.
fn mediated_allowlisted(sql: &str) -> bool {
    PG_MEDIATED_DIVERGENCE_ALLOWLIST
        .iter()
        .any(|entry| entry.sql == sql)
}

/// The fingerprint-mediated structural lane over the PG both-accept corpora
/// (conformance-mediated-structural-lane-pg): for every statement our Postgres preset and
/// `pg_query` both parse, our canonical render must fingerprint identically to the
/// original. A NEW mismatch (or an unparseable render) drifts the pin and fails.
///
/// This is the COMMODITY structural check; the hand-written `PgStructuralOracle`
/// (neutral-shape mapper) stays as the PREMIUM tier — it encodes the literal-value /
/// alias-name / IN-list-arity sensitivity this lane is blind to. A green here is NOT full
/// structural coverage.
#[test]
fn vendored_corpus_pg_mediated_structural_lane_holds() {
    // Ledger staleness contract (mirrors the accept/reject and sqlite/duckdb ledgers):
    // every entry names a real ticket, and every entry must STILL diverge — an entry
    // that fingerprint-matches (a fixed divergence) or has fallen out of the both-accept
    // subset (a skip) is stale, so the assertion fails until the entry is deleted.
    assert_entries_are_ticketed(PG_MEDIATED_DIVERGENCE_ALLOWLIST);
    assert_entries_still_diverge(PG_MEDIATED_DIVERGENCE_ALLOWLIST, |entry| {
        matches!(
            PgMediatedStructuralOracle.verdict(entry.sql),
            PgMediatedVerdict::Mismatch { .. } | PgMediatedVerdict::RenderUnparseable(_),
        )
    });

    let oracle = PgMediatedStructuralOracle;
    let mut both_accept = 0usize;
    let mut matched = 0usize;
    let mut mismatch = 0usize;
    let mut unparseable = 0usize;
    // Non-allowlisted divergences (mismatch or unparseable) — the failing set.
    let mut untriaged: Vec<(&str, &str, PgMediatedVerdict)> = Vec::new();
    // A both-accept statement pg_query could not fingerprint — the spike measured zero;
    // surface any as a drift rather than swallowing it.
    let mut skipped: Vec<(&str, &str, String)> = Vec::new();

    for corpus in corpora() {
        assert_eq!(
            corpus.statements.len(),
            corpus.pinned,
            "{} corpus statement count changed; if intentional, update the pin",
            corpus.label,
        );
        for &sql in &corpus.statements {
            // The lane compares tree shape only over the BOTH-ACCEPT subset. A statement
            // either side rejects is not comparable here — the accept/reject sweep above
            // owns those.
            if !(squonk_accepts(sql) && postgres_accepts(sql)) {
                continue;
            }
            both_accept += 1;
            match oracle.verdict(sql) {
                PgMediatedVerdict::Match => matched += 1,
                verdict @ PgMediatedVerdict::Mismatch { .. } => {
                    mismatch += 1;
                    if !mediated_allowlisted(sql) {
                        untriaged.push((corpus.label, sql, verdict));
                    }
                }
                verdict @ PgMediatedVerdict::RenderUnparseable(_) => {
                    unparseable += 1;
                    if !mediated_allowlisted(sql) {
                        untriaged.push((corpus.label, sql, verdict));
                    }
                }
                PgMediatedVerdict::Skip(reason) => skipped.push((corpus.label, sql, reason)),
            }
        }
    }

    // Printed always, so a green run documents the distribution and a drift shows fresh
    // counts to triage (mirrors the accept/reject sweep's always-print block).
    eprintln!(
        "PostgreSQL fingerprint-mediated structural lane over both-accept corpora:\n  \
         both-accept {both_accept}  match {matched}  mismatch {mismatch}  \
         unparseable {unparseable}  ({} untriaged, {} skipped)",
        untriaged.len(),
        skipped.len(),
    );

    assert_eq!(
        both_accept, PG_MEDIATED_BOTH_ACCEPT_PINNED,
        "PG both-accept subset size for the mediated lane changed \
         ({both_accept} vs pinned {PG_MEDIATED_BOTH_ACCEPT_PINNED}); if a corpus/parser \
         change is intentional, re-measure and update PG_MEDIATED_BOTH_ACCEPT_PINNED",
    );

    for (corpus, sql, reason) in &skipped {
        eprintln!("  [SKIP over both-accept] [{corpus}] {sql:?} -- {reason}");
    }
    assert!(
        skipped.is_empty(),
        "{} both-accept PG statement(s) pg_query could not fingerprint — the spike \
         measured zero; investigate before pinning",
        skipped.len(),
    );

    if !untriaged.is_empty() {
        eprintln!(
            "\nUNTRIAGED mediated structural divergences ({}):",
            untriaged.len()
        );
        for (corpus, sql, verdict) in &untriaged {
            eprintln!("  [{corpus}] {sql:?}\n    {verdict:?}");
        }
    }
    assert!(
        untriaged.is_empty(),
        "{} untriaged PostgreSQL mediated structural divergence(s): our canonical render \
         fingerprints differently from the original (or pg_query rejects our render). A \
         mismatch implicates the PARSER (wrong tree) OR the RENDERER (wrong canonical \
         form) — triage against the ADR-0014 render round-trip gates \
         (`assert_roundtrips` / `assert_roundtrips_parenthesized`, and the \
         `corpus_roundtrip` differential reparse) to localize which, then FIX it or add \
         an exact-SQL, ticketed entry to PG_MEDIATED_DIVERGENCE_ALLOWLIST",
        untriaged.len(),
    );
}

// ---------------------------------------------------------------------------
// Spec-audit: the FULL src/test/regress/sql corpus (spec-audit-pg-regress-corpus)
// ---------------------------------------------------------------------------
//
// PostgreSQL's own regression suite is its executable spec. Where the three vendored
// corpora above are multi-dialect and the hand-curated `corpus/postgres/` group is a
// small structural-oracle slice, this is the *full* statement set extracted from all
// 227 `src/test/regress/sql/*.sql` files (`corpus/pg-regress/`, pinned to REL_17_10 /
// PostgreSQL 17.10 so it aligns with the in-process `pg_query` PG-17 oracle). It is a
// pure MEASUREMENT surface: the sweep below PINS the accept/reject quadrant + the
// per-family divergence counts and PRINTS the ranked inventory, but files no tickets and
// gates nothing to zero (the ranked inventory drives separate fix tickets). See
// `corpus/pg-regress/README.md` + `extract_pg_regress.py`.
//
// `pg_query` is `ParseOnly` and in-process, so — unlike the DuckDB/SQLite `PrepareBind`
// sweeps — there is no schema provisioning, no binding/syntax reject split, and no
// `Quadrant`: every statement yields a clean two-engine verdict and falls in one of four
// cells. The corpus is a flat statement list grouped under `# file:` markers (provenance,
// so a divergence family traces back to its source file); the flat view is every non-`#`
// non-blank line.
const PG_REGRESS: &str = include_str!("../corpus/pg-regress/statements.sql");
const PG_STMT_PRODUCTIONS: &str = include_str!("../corpus/pg-regress/stmt-productions.txt");

/// Anti-vanishing count pins (a line vanishing from the fixture trips these). Measured
/// off the REL_17_10 extraction (`extract_pg_regress.py`, global dedup).
const PG_REGRESS_STATEMENTS_PINNED: usize = 35341;
const PG_REGRESS_FILES_PINNED: usize = 227;

/// One `# file:` group in the regress corpus: the source path (provenance) and the
/// statements drawn from it, in source order.
struct RegressGroup {
    file: &'static str,
    statements: Vec<&'static str>,
}

/// Parse [`PG_REGRESS`] into per-file groups. Line-oriented with a single `# file:`
/// marker per group; no extracted statement begins with `#`, so the marker is
/// unambiguous.
fn regress_groups() -> Vec<RegressGroup> {
    let mut groups: Vec<RegressGroup> = Vec::new();
    for line in PG_REGRESS.lines() {
        if let Some(file) = line.strip_prefix("# file:") {
            groups.push(RegressGroup {
                file: file.trim(),
                statements: Vec::new(),
            });
        } else if !line.trim().is_empty() {
            groups
                .last_mut()
                .expect("a statement line precedes its `# file:` header")
                .statements
                .push(line);
        }
    }
    groups
}

/// The flat statement view (every non-marker, non-blank line), in source order.
fn regress_statements() -> Vec<&'static str> {
    PG_REGRESS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect()
}

/// The direct alternatives of PostgreSQL 17's top-level `stmt` grammar production.
/// This is independent negative space: unlike a corpus family inventory, it names an
/// upstream production even when no vendored statement happens to exercise it.
fn pg_stmt_productions() -> std::collections::BTreeSet<&'static str> {
    PG_STMT_PRODUCTIONS
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
}

/// Map one accepted raw parse tree back to the `stmt` alternative that built it.
/// Most grammar alternatives and protobuf variants have the same name. The explicit
/// arms are the places where PostgreSQL intentionally folds multiple spellings into one
/// raw node, or libpg_query normalizes the C production name when generating Rust.
fn pg_stmt_production(sql: &str, node_kind: &str) -> Option<&'static str> {
    let family = statement_family(sql);
    match (node_kind, family.as_str()) {
        ("CreateTableAsStmt", "CREATE MATERIALIZED VIEW") => Some("CreateMatViewStmt"),
        ("CreateTableAsStmt", _) => Some("CreateAsStmt"),
        ("CreatePlangStmt", _) => Some("CreatePLangStmt"),
        ("CompositeTypeStmt" | "CreateEnumStmt" | "CreateRangeStmt", _) => Some("DefineStmt"),
        ("AlterTableStmt", "ALTER TYPE") => Some("AlterCompositeTypeStmt"),
        ("AlterTableMoveAllStmt", _) => Some("AlterTableStmt"),
        ("AlterTableSpaceOptionsStmt", _) => Some("AlterTblSpcStmt"),
        ("AlterTsconfigurationStmt", _) => Some("AlterTSConfigurationStmt"),
        ("AlterTsdictionaryStmt", _) => Some("AlterTSDictionaryStmt"),
        ("CreateRoleStmt", "CREATE GROUP") => Some("CreateGroupStmt"),
        ("CreateRoleStmt", "CREATE USER") => Some("CreateUserStmt"),
        ("CreateRoleStmt", _) => Some("CreateRoleStmt"),
        ("AlterRoleStmt", "ALTER GROUP") => Some("AlterGroupStmt"),
        ("AlterRoleStmt", _) => Some("AlterRoleStmt"),
        ("GrantStmt", "REVOKE") => Some("RevokeStmt"),
        ("GrantStmt", _) => Some("GrantStmt"),
        ("GrantRoleStmt", "REVOKE") => Some("RevokeRoleStmt"),
        ("GrantRoleStmt", _) => Some("GrantRoleStmt"),
        ("VariableSetStmt", "RESET") => Some("VariableResetStmt"),
        ("VariableSetStmt", _) => Some("VariableSetStmt"),
        ("VacuumStmt", "ANALYZE") => Some("AnalyzeStmt"),
        ("DropStmt", "DROP AGGREGATE") => Some("RemoveAggrStmt"),
        ("DropStmt", "DROP FUNCTION" | "DROP PROCEDURE" | "DROP ROUTINE") => Some("RemoveFuncStmt"),
        ("DropStmt", "DROP OPERATOR") => Some("RemoveOperStmt"),
        ("DropStmt", "DROP CAST") => Some("DropCastStmt"),
        ("DropStmt", "DROP OPERATOR CLASS") => Some("DropOpClassStmt"),
        ("DropStmt", "DROP OPERATOR FAMILY") => Some("DropOpFamilyStmt"),
        ("DropStmt", "DROP TRANSFORM") => Some("DropTransformStmt"),
        _ => PG_STMT_PRODUCTIONS
            .lines()
            .find(|production| *production == node_kind),
    }
}

/// The leading uppercased word-tokens of `sql` (ASCII identifier chars only), up to
/// `max`. Punctuation and whitespace bound tokens, so `CREATE TABLE foo(` yields
/// `["CREATE", "TABLE", "FOO"]`.
fn leading_words(sql: &str, max: usize) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in sql.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch.to_ascii_uppercase());
        } else {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
                if words.len() == max {
                    return words;
                }
            }
        }
    }
    if !cur.is_empty() && words.len() < max {
        words.push(cur);
    }
    words
}

/// The object kind a `CREATE`/`ALTER`/`DROP` targets, skipping leading modifiers
/// (`OR REPLACE`, `TEMP`, `UNIQUE`, …) and joining the known multi-word object names
/// (`MATERIALIZED VIEW`, `FOREIGN TABLE`, `TEXT SEARCH …`, `OPERATOR CLASS`, …).
fn ddl_object(rest: &[String]) -> String {
    const MODIFIERS: &[&str] = &[
        "OR",
        "REPLACE",
        "TEMP",
        "TEMPORARY",
        "GLOBAL",
        "LOCAL",
        "UNLOGGED",
        "UNIQUE",
        "RECURSIVE",
        "DEFAULT",
    ];
    let mut i = 0;
    while i < rest.len() && MODIFIERS.contains(&rest[i].as_str()) {
        i += 1;
    }
    let Some(obj) = rest.get(i) else {
        return "<obj?>".to_string();
    };
    let next = rest.get(i + 1).map(String::as_str);
    let two = || {
        next.map(|n| format!("{obj} {n}"))
            .unwrap_or_else(|| obj.clone())
    };
    // Only combine with a FIXED trailing keyword — never a user-supplied identifier.
    // `CREATE USER foo` must stay `CREATE USER` (only `USER MAPPING` is multi-word);
    // `MATERIALIZED`/`FOREIGN`/`TEXT`/`ACCESS`/`EVENT`/`PROCEDURAL`/`LARGE` are always
    // followed by their fixed second keyword.
    match (obj.as_str(), next) {
        ("MATERIALIZED", _)
        | ("FOREIGN", _)
        | ("TEXT", _)
        | ("ACCESS", _)
        | ("EVENT", _)
        | ("PROCEDURAL", _)
        | ("LARGE", _) => two(),
        ("USER", Some("MAPPING")) | ("OPERATOR", Some("CLASS")) | ("OPERATOR", Some("FAMILY")) => {
            two()
        }
        _ => obj.clone(),
    }
}

/// The divergence family of `sql`: its statement head. For the object-DDL heads
/// (`CREATE`/`ALTER`/`DROP`) the object kind is appended (`CREATE OPERATOR` vs
/// `CREATE TABLE`), since the spec-audit prediction is that object-DDL/utility statements
/// dominate the gap — head-grouping is exactly the cut that tests it. Everything else
/// buckets under its leading keyword (`SELECT`, `INSERT`, `EXPLAIN`, `GRANT`, …).
fn statement_family(sql: &str) -> String {
    let trimmed = sql.trim_start();
    if trimmed.starts_with('(') {
        return "( paren-query )".to_string();
    }
    let words = leading_words(trimmed, 5);
    let Some(head) = words.first() else {
        return "<other>".to_string();
    };
    match head.as_str() {
        "CREATE" | "ALTER" | "DROP" => format!("{head} {}", ddl_object(&words[1..])),
        other => other.to_string(),
    }
}

/// Which way a regress divergence points (PG's verdict alone fixes it, since `pg_query`
/// is the oracle).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegressCell {
    AgreeAccept,
    AgreeReject,
    /// PG accepts, we reject — a coverage gap (the bulk class the audit predicts).
    CoverageGap,
    /// We accept, PG rejects — an over-acceptance (a validator-correctness divergence).
    OverAccept,
}

fn regress_cell(sql: &str) -> RegressCell {
    match (squonk_accepts(sql), postgres_accepts(sql)) {
        (true, true) => RegressCell::AgreeAccept,
        (false, false) => RegressCell::AgreeReject,
        (false, true) => RegressCell::CoverageGap,
        (true, false) => RegressCell::OverAccept,
    }
}

#[test]
fn pg_regress_corpus_is_pinned_and_parses_without_panicking() {
    // Always-on (no oracle needed): the count pins guard the fixture, the grouped/flat
    // views must cohere, and every statement must run through our parser to a *verdict*,
    // never a panic (the P1 class this audit hunts — reported first).
    let groups = regress_groups();
    let flat = regress_statements();
    assert_eq!(
        flat.len(),
        PG_REGRESS_STATEMENTS_PINNED,
        "pg-regress/statements.sql count changed; if intentional, re-pin PG_REGRESS_STATEMENTS_PINNED",
    );
    assert_eq!(
        groups.len(),
        PG_REGRESS_FILES_PINNED,
        "pg-regress/statements.sql file-group count changed; re-pin PG_REGRESS_FILES_PINNED",
    );
    let grouped: usize = groups.iter().map(|g| g.statements.len()).sum();
    assert_eq!(
        grouped,
        flat.len(),
        "grouped statement count must equal the flat view; regenerate from extract_pg_regress.py",
    );
    assert!(
        groups
            .iter()
            .all(|g| g.file.starts_with("src/test/regress/sql/")),
        "every group must name its upstream src/test/regress/sql/*.sql source file",
    );

    // Panic/hang class: run every statement through our parser under catch_unwind, with a
    // silenced hook so a (hypothetical) panic does not spam the log — collect the SQL of
    // any that panic and report them (never swallow). Track the accept/reject split and
    // the slowest line so a hang regression is legible.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut panics: Vec<&str> = Vec::new();
    let (mut accept, mut reject) = (0usize, 0usize);
    let mut slowest = (std::time::Duration::ZERO, "");
    for &sql in &flat {
        let start = std::time::Instant::now();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parse_with(sql, squonk::ParseConfig::new(Postgres)).is_ok()
        })) {
            Ok(true) => accept += 1,
            Ok(false) => reject += 1,
            Err(_) => panics.push(sql),
        }
        let elapsed = start.elapsed();
        if elapsed > slowest.0 {
            slowest = (elapsed, sql);
        }
    }
    std::panic::set_hook(prev_hook);

    eprintln!(
        "squonk {Postgres:?} over the PG regress corpus: {accept} accept / {reject} reject \
         of {} statements; slowest {:.2}ms",
        flat.len(),
        slowest.0.as_secs_f64() * 1e3,
    );
    if !panics.is_empty() {
        eprintln!(
            "\nPARSER PANICS ({}) — P1, report immediately:",
            panics.len()
        );
        for sql in &panics {
            eprintln!("  PANIC {sql:?}");
        }
    }
    assert!(
        panics.is_empty(),
        "{} statement(s) PANICKED our parser (P1) — a verdict must never be a panic; see the list above",
        panics.len(),
    );
    assert!(
        accept > 0 && reject > 0,
        "parser produced a degenerate split"
    );
}

/// PostgreSQL `stmt` alternatives not exercised by an oracle-accepted statement in the
/// vendored REL_17_10 regress suite. This is a measured pin, not a support claim: an
/// exercised alternative can still contain many unexercised sub-productions.
const PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS: &[&str] = &[
    "AlterExtensionContentsStmt",
    "AlterExtensionStmt",
    "AlterObjectDependsStmt",
    "AlterSystemStmt",
    "CreateAssertionStmt",
    "CreateExtensionStmt",
    "DropTransformStmt",
];

/// Authored parse-only probes for the engine-supported alternatives absent from
/// PostgreSQL's regression corpus. Parser support is recorded independently: these
/// probes close the engine-production inventory, not squonk coverage gaps.
const PG_UNEXERCISED_STMT_PROBES: &[(&str, &str, bool)] = &[
    (
        "AlterExtensionContentsStmt",
        "ALTER EXTENSION ext ADD TABLE t",
        true,
    ),
    (
        "AlterExtensionStmt",
        "ALTER EXTENSION ext UPDATE TO '2.0'",
        true,
    ),
    (
        "AlterObjectDependsStmt",
        "ALTER FUNCTION f(integer) DEPENDS ON EXTENSION ext",
        true,
    ),
    (
        "AlterSystemStmt",
        "ALTER SYSTEM SET work_mem = '64MB'",
        true,
    ),
    ("CreateExtensionStmt", "CREATE EXTENSION ext", true),
    (
        "DropTransformStmt",
        "DROP TRANSFORM FOR hstore LANGUAGE plpython3u",
        true,
    ),
];

/// PostgreSQL 17 retains this SQL-standard alternative in `stmt`, but its grammar
/// action raises `FEATURE_NOT_SUPPORTED` instead of constructing a raw parse node.
const PG_ENGINE_UNIMPLEMENTED_STMT_PRODUCTIONS: &[(&str, &str)] =
    &[("CreateAssertionStmt", "CREATE ASSERTION a CHECK (true)")];

#[test]
fn pg_regress_statement_production_coverage_is_measured() {
    use std::collections::BTreeSet;

    let productions = pg_stmt_productions();
    assert_eq!(
        productions.len(),
        124,
        "PostgreSQL top-level stmt production count drifted; regenerate stmt-productions.txt",
    );

    let mut exercised = BTreeSet::new();
    let mut unmapped = BTreeSet::new();
    for sql in regress_statements() {
        let Ok(parsed) = pg_query::parse(sql) else {
            continue;
        };
        assert_eq!(
            parsed.protobuf.stmts.len(),
            1,
            "one extracted corpus line must produce exactly one raw statement: {sql:?}",
        );
        let node = parsed.protobuf.stmts[0]
            .stmt
            .as_ref()
            .and_then(|stmt| stmt.node.as_ref())
            .unwrap_or_else(|| panic!("accepted statement has no raw node: {sql:?}"));
        let debug = format!("{node:?}");
        let node_kind = debug
            .split_once('(')
            .map_or(debug.as_str(), |(kind, _)| kind);
        if let Some(production) = pg_stmt_production(sql, node_kind) {
            assert!(
                productions.contains(production),
                "mapped production {production:?} is absent from stmt-productions.txt",
            );
            exercised.insert(production);
        } else {
            unmapped.insert((node_kind.to_owned(), statement_family(sql), sql.to_owned()));
        }
    }

    let unexercised: Vec<_> = productions.difference(&exercised).copied().collect();
    eprintln!(
        "PostgreSQL 17 top-level stmt production coverage from REL_17_10 regress: {}/{} ({:.1}%)",
        exercised.len(),
        productions.len(),
        100.0 * exercised.len() as f64 / productions.len() as f64,
    );
    eprintln!("  exercised: {exercised:?}");
    eprintln!("  UNEXERCISED: {unexercised:?}");
    eprintln!("  UNMAPPED raw nodes: {unmapped:?}");
    assert!(
        unmapped.is_empty(),
        "accepted regress statements reached raw nodes not mapped to stmt productions",
    );

    let pinned: Vec<_> = PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS.to_vec();
    assert_eq!(
        unexercised, pinned,
        "PG regress stmt-production coverage drifted; review both exercised and unexercised sets before re-baselining PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS",
    );
    let pinned_exercised = productions.len() - PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS.len();
    assert_eq!(
        exercised.len(),
        pinned_exercised,
        "exercised and unexercised production counts must partition the grammar inventory",
    );
    let pinned: BTreeSet<_> = productions
        .iter()
        .copied()
        .filter(|production| !PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS.contains(production))
        .collect();
    assert_eq!(
        exercised, pinned,
        "PG regress stmt-production partition is incomplete",
    );
}

#[test]
fn pg_unexercised_statement_productions_have_permanent_oracle_probes() {
    use std::collections::BTreeSet;

    let productions = pg_stmt_productions();
    let regress_unexercised: BTreeSet<_> = PG_REGRESS_UNEXERCISED_STMT_PRODUCTIONS
        .iter()
        .copied()
        .collect();
    let mut probed = BTreeSet::new();

    for &(expected_production, sql, expected_squonk_support) in PG_UNEXERCISED_STMT_PROBES {
        let parsed = pg_query::parse(sql).unwrap_or_else(|err| {
            panic!("pg_query rejected {expected_production} probe {sql:?}: {err:?}")
        });
        assert_eq!(
            parsed.protobuf.stmts.len(),
            1,
            "one probe must produce one statement"
        );
        let node = parsed.protobuf.stmts[0]
            .stmt
            .as_ref()
            .and_then(|stmt| stmt.node.as_ref())
            .unwrap_or_else(|| panic!("accepted probe has no raw node: {sql:?}"));
        let debug = format!("{node:?}");
        let node_kind = debug
            .split_once('(')
            .map_or(debug.as_str(), |(kind, _)| kind);
        let actual_production = pg_stmt_production(sql, node_kind);
        assert_eq!(
            actual_production,
            Some(expected_production),
            "probe reached the wrong top-level production: {sql:?}",
        );
        assert_eq!(
            squonk_accepts(sql),
            expected_squonk_support,
            "squonk support changed for {expected_production}; review the coverage boundary",
        );
        assert!(
            probed.insert(expected_production),
            "duplicate production probe"
        );
    }

    let mut engine_unimplemented = BTreeSet::new();
    for &(production, sql) in PG_ENGINE_UNIMPLEMENTED_STMT_PRODUCTIONS {
        let err = match pg_query::parse(sql) {
            Ok(_) => panic!("{production} unexpectedly produced a raw parse node"),
            Err(err) => err,
        };
        assert!(
            format!("{err:?}").contains("not yet implemented"),
            "{production} must remain grammar-present but engine-unimplemented: {err:?}",
        );
        assert!(engine_unimplemented.insert(production));
    }

    let accounted: BTreeSet<_> = probed.union(&engine_unimplemented).copied().collect();
    assert_eq!(
        accounted, regress_unexercised,
        "authored probes and engine-unimplemented alternatives must exactly partition the regress gaps",
    );
    let combined_exercised = productions.len() - engine_unimplemented.len();
    eprintln!(
        "PostgreSQL 17 top-level stmt production coverage from regress + authored probes: \
         {combined_exercised}/{} ({:.1}%); engine-unimplemented: {engine_unimplemented:?}",
        productions.len(),
        100.0 * combined_exercised as f64 / productions.len() as f64,
    );
    assert_eq!(
        combined_exercised, 123,
        "combined production coverage pin drifted"
    );
}

// --- Measured spec-audit pins (spec-audit-pg-regress-corpus) -------------------------
//
// Baselined against pg_query 6.1.1 (libpg_query / PostgreSQL 17) + the vendored full
// regress corpus (REL_17_10) under our `Postgres` preset. This is a measurement
// baseline, not a zero gate: a drift fails loudly so the inventory is re-read and
// re-baselined, but nothing is forced to zero and no ticket is required here. The tuple is
// `(agree_accept, coverage_gap, over_accept, agree_reject)`; over-accept is pinned 0. The
// coverage-gap column counts single PG statements our parser under-consumes and honestly
// rejects (e.g. `RESET SESSION AUTHORIZATION`, `CREATE INDEX ... INCLUDE (...)`) rather than
// mis-splitting an unconsumed tail into further statements — a pre-existing under-consumption
// tracked in its own follow-ups, not an over-acceptance.
const PG_REGRESS_QUADRANT: (usize, usize, usize, usize) = (29673, 5406, 0, 262);

/// Per-family coverage-gap counts (PG accepts / we reject), ranked by statement head
/// (object kind appended for CREATE/ALTER/DROP). Families below
/// [`PG_REGRESS_FAMILY_FLOOR`] fold into the pinned `(<tail>, n)` bucket so long-tail churn
/// does not force a re-pin, while head families each pin.
///
/// `PREPARE`'s remaining gaps are all `PREPARE TRANSACTION '<gid>'` — PostgreSQL's unrelated
/// two-phase-commit statement sharing the `PREPARE` head, not modelled here. `CREATE FUNCTION`'s
/// residual gap counts against the mode-bearing and other routine forms not yet covered.
const PG_REGRESS_GAP_FAMILIES: &[(&str, usize)] = &[
    ("ALTER TABLE", 355),
    ("CREATE TRIGGER", 322),
    ("CREATE FUNCTION", 262),
    ("ANALYZE", 161),
    ("CREATE INDEX", 159),
    ("DROP ROLE", 155),
    ("CREATE TYPE", 151),
    ("CREATE ROLE", 149),
    ("VACUUM", 134),
    ("FETCH", 132),
    ("ALTER PUBLICATION", 127),
    ("CREATE AGGREGATE", 126),
    ("CREATE RULE", 124),
    ("CREATE DOMAIN", 120),
    ("REINDEX", 107),
    ("CREATE STATISTICS", 100),
    ("DECLARE", 91),
    ("ALTER TYPE", 90),
    ("ALTER INDEX", 84),
    ("UPDATE", 80),
    ("DROP TRIGGER", 78),
    ("CREATE PUBLICATION", 76),
    ("ALTER FOREIGN TABLE", 73),
    ("DROP TYPE", 72),
    ("DROP DOMAIN", 71),
    ("CREATE POLICY", 63),
    ("INSERT", 61),
    ("COMMENT", 56),
    ("CREATE OPERATOR", 56),
    ("ALTER OPERATOR FAMILY", 55),
    ("DROP USER", 55),
    ("CREATE COLLATION", 53),
    ("CREATE USER", 53),
    ("ALTER SUBSCRIPTION", 45),
    ("ALTER TEXT SEARCH", 45),
    ("CREATE VIEW", 41),
    ("ALTER ROLE", 40),
    ("LOCK", 40),
    ("ALTER DOMAIN", 39),
    ("REVOKE", 39),
    ("CREATE TEXT SEARCH", 38),
    ("ALTER FUNCTION", 37),
    ("CALL", 36),
    ("DROP FUNCTION", 35),
    ("CREATE SUBSCRIPTION", 31),
    ("ALTER SEQUENCE", 30),
    ("CREATE PROCEDURE", 30),
    ("CLOSE", 29),
    ("DROP PUBLICATION", 29),
    ("GRANT", 29),
    ("CREATE EVENT TRIGGER", 28),
    ("DROP OPERATOR", 28),
    ("CLUSTER", 27),
    ("ALTER OPERATOR", 26),
    ("ALTER PRIVILEGES", 26),
    ("ALTER FOREIGN DATA", 25),
    ("ALTER SERVER", 24),
    ("ALTER VIEW", 24),
    ("CREATE FOREIGN TABLE", 24),
    ("CREATE CAST", 23),
    ("CREATE SERVER", 23),
    ("CREATE USER MAPPING", 23),
    ("DROP OPERATOR FAMILY", 20),
    ("CREATE FOREIGN DATA", 19),
    ("CREATE SEQUENCE", 19),
    ("DROP RULE", 19),
    ("EXPLAIN", 19),
    ("CREATE OPERATOR FAMILY", 18),
    ("DROP EVENT TRIGGER", 17),
    ("DROP STATISTICS", 17),
    ("PREPARE", 17),
    ("DROP SERVER", 16),
    ("CREATE OPERATOR CLASS", 15),
    ("DROP POLICY", 15),
    ("ALTER AGGREGATE", 14),
    ("ALTER POLICY", 14),
    ("DROP AGGREGATE", 14),
    ("DROP FOREIGN DATA", 14),
    ("ALTER OPERATOR CLASS", 13),
    ("ALTER STATISTICS", 13),
    ("CREATE SCHEMA", 13),
    ("DROP FOREIGN TABLE", 13),
    ("DROP OWNED", 13),
    ("DROP TEXT SEARCH", 13),
    ("DROP COLLATION", 12),
    ("DROP INDEX", 11),
    ("ALTER CONVERSION", 10),
    ("ALTER USER MAPPING", 10),
    ("DROP CAST", 10),
    ("DROP USER MAPPING", 10),
    ("(<tail: 52 families>)", 233),
];

/// Per-family over-acceptance counts (we accept / PG rejects), ranked and pinned
/// separately from coverage gaps (same floor/tail rule).
const PG_REGRESS_OVERACCEPT_FAMILIES: &[(&str, usize)] = &[];

/// Families with fewer than this many divergences fold into the `(<tail>, n)` bucket.
const PG_REGRESS_FAMILY_FLOOR: usize = 10;

/// Roll a raw family→count map into the pinned shape: head families (count ≥ floor) sorted
/// by descending count then name, with the sub-floor tail summed into a single
/// `("(<tail: k families>)", n)` entry so long-tail churn does not force a re-pin.
fn rolled_families(counts: &std::collections::BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut head: Vec<(String, usize)> = counts
        .iter()
        .filter(|(_, n)| **n >= PG_REGRESS_FAMILY_FLOOR)
        .map(|(f, n)| (f.clone(), *n))
        .collect();
    head.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let tail: Vec<(&String, &usize)> = counts
        .iter()
        .filter(|(_, n)| **n < PG_REGRESS_FAMILY_FLOOR)
        .collect();
    if !tail.is_empty() {
        let tail_sum: usize = tail.iter().map(|(_, n)| **n).sum();
        head.push((format!("(<tail: {} families>)", tail.len()), tail_sum));
    }
    head
}

#[test]
fn pg_regress_spec_audit_inventory() {
    use std::collections::BTreeMap;

    // Always-on: `pg_query` is in-process and `ParseOnly`, so this needs no oracle feature.
    let groups = regress_groups();

    let mut agree_accept = 0usize;
    let mut agree_reject = 0usize;
    let mut coverage_gap = 0usize;
    let mut over_accept = 0usize;
    let mut gap_families: BTreeMap<String, usize> = BTreeMap::new();
    let mut over_families: BTreeMap<String, usize> = BTreeMap::new();
    // One sample (file, sql) per family, for the printed inventory.
    let mut gap_sample: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    let mut over_examples: Vec<(&str, &str)> = Vec::new(); // every over-accept (report all)

    for group in &groups {
        for &sql in &group.statements {
            match regress_cell(sql) {
                RegressCell::AgreeAccept => agree_accept += 1,
                RegressCell::AgreeReject => agree_reject += 1,
                RegressCell::CoverageGap => {
                    coverage_gap += 1;
                    let fam = statement_family(sql);
                    *gap_families.entry(fam.clone()).or_default() += 1;
                    gap_sample.entry(fam).or_insert((group.file, sql));
                }
                RegressCell::OverAccept => {
                    over_accept += 1;
                    *over_families.entry(statement_family(sql)).or_default() += 1;
                    over_examples.push((group.file, sql));
                }
            }
        }
    }

    let total = agree_accept + agree_reject + coverage_gap + over_accept;
    let pct = |n: usize| 100.0 * n as f64 / total as f64;
    eprintln!(
        "\n=== PG regress spec-audit inventory ({Postgres:?} vs pg_query PG-17, REL_17_10, \
         {total} statements) ==="
    );
    eprintln!(
        "  agree accept   (A/A)  {agree_accept:>6}  ({:.1}%)",
        pct(agree_accept)
    );
    eprintln!(
        "  COVERAGE GAP   (R/A)  {coverage_gap:>6}  ({:.1}%)   <- PG accepts, we reject",
        pct(coverage_gap),
    );
    eprintln!(
        "  over-accept    (A/R)  {over_accept:>6}  ({:.1}%)   <- we accept, PG rejects",
        pct(over_accept),
    );
    eprintln!(
        "  agree reject   (R/R)  {agree_reject:>6}  ({:.1}%)",
        pct(agree_reject)
    );
    let head_cov = agree_accept + over_accept; // statements OUR parser accepts
    eprintln!(
        "  our accept-rate       {:.1}%   (statement-head coverage vs PG's accepted set: {:.1}%)",
        pct(head_cov),
        100.0 * agree_accept as f64 / (agree_accept + coverage_gap) as f64,
    );

    let ranked = |counts: &BTreeMap<String, usize>| -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = counts.iter().map(|(f, &n)| (f.clone(), n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v
    };

    eprintln!("\n  --- coverage-gap families ranked (PG accepts, we reject) ---");
    for (fam, n) in ranked(&gap_families) {
        let (file, sql) = gap_sample[&fam];
        let sample: String = sql.chars().take(72).collect();
        eprintln!("    {n:>6}  {fam:<24} e.g. [{file}] {sample:?}");
    }
    eprintln!("\n  --- over-acceptance families ranked (we accept, PG rejects) ---");
    for (fam, n) in ranked(&over_families) {
        eprintln!("    {n:>6}  {fam}");
    }
    eprintln!(
        "\n  --- every over-acceptance ({}) [we accept, PG parse-rejects] ---",
        over_examples.len()
    );
    for (file, sql) in &over_examples {
        eprintln!("    A/R [{file}] {sql:?}");
    }

    let got = (agree_accept, coverage_gap, over_accept, agree_reject);
    let got_gap = rolled_families(&gap_families);
    let got_over = rolled_families(&over_families);
    eprintln!("\n  MEASURED quadrant tuple:     {got:?}");
    eprintln!("  MEASURED gap families:       {got_gap:?}");
    eprintln!("  MEASURED over-accept families: {got_over:?}");

    // --- Anti-drift pins (measurement baseline, not a gate) ---
    let want_gap: Vec<(String, usize)> = PG_REGRESS_GAP_FAMILIES
        .iter()
        .map(|(f, n)| (f.to_string(), *n))
        .collect();
    let want_over: Vec<(String, usize)> = PG_REGRESS_OVERACCEPT_FAMILIES
        .iter()
        .map(|(f, n)| (f.to_string(), *n))
        .collect();
    assert_eq!(
        got, PG_REGRESS_QUADRANT,
        "PG regress quadrant drifted; re-baseline PG_REGRESS_QUADRANT",
    );
    assert_eq!(
        got_gap, want_gap,
        "PG regress coverage-gap families drifted; re-baseline PG_REGRESS_GAP_FAMILIES",
    );
    assert_eq!(
        got_over, want_over,
        "PG regress over-acceptance families drifted; re-baseline PG_REGRESS_OVERACCEPT_FAMILIES",
    );
}

// ---------------------------------------------------------------------------
// Head-family sub-classification by reject shape
// (pg-regress-select-family-characterization)
// ---------------------------------------------------------------------------
//
// The inventory above ranks coverage gaps by STATEMENT HEAD, where SELECT (1757) and
// CREATE TABLE (1736) are the two giants. Unlike the object-DDL tail, the SELECT gaps are
// QUERY-SURFACE misses (expression forms, operators, clauses) inside our flagship family,
// not a missing statement kind — so head-grouping does not name their root construct. This
// lane sub-classifies a head family's coverage gaps by REJECT SHAPE: the
// `(expected, offending-token)` pair our parser emits, which is the cheap first cut that
// clusters e.g. every `#>>`/`?|`/`@@` operator-tail gap, or every `SUBSTRING(x FROM …)`
// special-form gap, together. The offending lexeme is normalized so an operator or a
// clause keyword stays verbatim (the discriminating token) while an arbitrary
// table/column identifier or literal collapses to a class token (so it does not fragment
// a cluster). The test PRINTS the ranked sub-family table — the deliverable that
// feeds fix tickets — and PINS only the per-family gap total (coherent
// with the head-family pin above) as an anti-vanishing guard. Measurement only: it changes
// no parser behaviour, files no tickets, and gates nothing to zero.

/// Normalize the offending lexeme of a reject into a cluster-stable token. Literals and
/// end-of-input collapse to a class token; a bare word (keyword OR identifier) is kept
/// verbatim, uppercased. Keeping bare words verbatim is deliberate: in these gaps the
/// offending token is almost always the construct keyword itself — `POSITION`, `VARIADIC`,
/// `JSON_VALUE`, `XMLSERIALIZE`, `SYMMETRIC`, `INHERITS`, `COLLATE` — so uppercasing it
/// (PG regress writes many in upper case) makes the cluster key name the root construct.
/// Operator/punctuation stays verbatim — the load-bearing token for the PG operator tail.
/// A very long bare word is treated as a user identifier/value and collapsed, so a stray
/// table/column name does not seed a singleton cluster.
fn normalize_found(found: &Found) -> String {
    let text = match found {
        Found::EndOfInput => return "<eof>".to_string(),
        Found::Text(t) => t.as_ref().trim(),
    };
    let Some(first) = text.chars().next() else {
        return "<empty>".to_string();
    };
    if first.is_ascii_digit() {
        return "<num>".to_string();
    }
    if first == '\'' || first == '"' || first == '$' {
        // string literal, quoted identifier, or dollar-quoted body
        return "<quoted>".to_string();
    }
    if text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        if text.len() > 20 {
            return "<ident>".to_string();
        }
        return text.to_ascii_uppercase();
    }
    // Operator / punctuation: the load-bearing token for the PG operator-tail gaps.
    text.to_string()
}

/// The reject-shape cluster key for a coverage-gap statement: the parser's
/// `(expected, normalized-found)` pair (a recursion-limit reject is its own key).
fn reject_shape(err: &ParseError) -> (String, String) {
    if err.kind == ParseErrorKind::RecursionLimitExceeded {
        return ("<recursion-limit>".to_string(), String::new());
    }
    (
        err.expected.as_str().to_string(),
        normalize_found(&err.found),
    )
}

/// One reject-shape cluster: its `(expected, found)` key, the count of coverage-gap
/// statements that reject that way, and one example (file + SQL).
struct RejectCluster {
    expected: String,
    found: String,
    count: usize,
    example: &'static str,
    example_file: &'static str,
}

/// Sub-classify a head family's coverage gaps (PG accepts, we reject) by reject shape.
/// Returns the gap total and the clusters ranked by descending count. One parse of our
/// parser per family statement; `pg_query` only for the reject-and-not-agree-reject check.
fn subclassify_family(groups: &[RegressGroup], family: &str) -> (usize, Vec<RejectCluster>) {
    use std::collections::BTreeMap;
    // key -> (count, example sql, example file). BTree for deterministic tie-break order.
    let mut clusters: BTreeMap<(String, String), (usize, &str, &str)> = BTreeMap::new();
    let mut total = 0usize;
    for group in groups {
        for &sql in &group.statements {
            if statement_family(sql) != family {
                continue;
            }
            // A coverage gap is: our parser rejects AND pg_query accepts. Parse once.
            let Err(err) = parse_with(sql, squonk::ParseConfig::new(Postgres)) else {
                continue; // we accept — agree-accept or over-accept, not a gap
            };
            if !postgres_accepts(sql) {
                continue; // agree-reject — not a coverage gap
            }
            total += 1;
            let key = reject_shape(&err);
            let entry = clusters.entry(key).or_insert((0, sql, group.file));
            entry.0 += 1;
        }
    }
    let mut ranked: Vec<RejectCluster> = clusters
        .into_iter()
        .map(
            |((expected, found), (count, example, example_file))| RejectCluster {
                expected,
                found,
                count,
                example,
                example_file,
            },
        )
        .collect();
    ranked.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.expected.cmp(&b.expected))
            .then_with(|| a.found.cmp(&b.found))
    });
    (total, ranked)
}

/// The head families this lane sub-classifies, with their pinned gap totals (coherent
/// with `PG_REGRESS_GAP_FAMILIES` above — a drift there must drift here identically).
const SUBCLASSIFIED_FAMILIES: &[(&str, usize)] = &[("SELECT", 2)];

#[test]
fn pg_regress_head_family_subclassification() {
    let groups = regress_groups();
    for &(family, pinned) in SUBCLASSIFIED_FAMILIES {
        let (total, clusters) = subclassify_family(&groups, family);
        eprintln!(
            "\n=== {family} coverage-gap sub-classification by reject shape — \
             {total} gaps across {} reject-shape clusters ===",
            clusters.len(),
        );
        assert_eq!(
            total, pinned,
            "{family} coverage-gap total ({total}) drifted from its head-family pin \
             ({pinned}); re-baseline SUBCLASSIFIED_FAMILIES and PG_REGRESS_GAP_FAMILIES together",
        );
        // Print every cluster ranked; the offending-token normalization keeps this to a
        // legible list (the deliverable inventory) rather than one row per statement.
        for c in &clusters {
            let example: String = c.example.chars().take(100).collect();
            eprintln!(
                "  {:>5}  expected {:?} / found {:?}\n         e.g. [{}] {:?}",
                c.count, c.expected, c.found, c.example_file, example,
            );
        }
    }
}

// =====================================================================================
// Flag-aware generative differential (oracle-parity-postgres)
// =====================================================================================
//
// Complements the raw-byte `pg_differential_raw_bytes` fuzz lane with grammar-guided
// generation over PostgreSQL's flag-gated surfaces (the user directive that corpus
// replay alone is not parity). ParseOnly against libpg_query — no schema setup.

use crate::properties::dialect_features::{
    POSTGRES_FEATURE_PROBES, POSTGRES_FEATURE_SEEDS, arb_feature_statement,
};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use squonk::Dialect;

/// Generative over-acceptance ledger (empty: PG preset matches libpg_query on the
/// probe surface today). Minimized findings land here with a ticket.
const POSTGRES_GENERATIVE_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[];

/// One generated statement's accept/reject pair under Postgres vs libpg_query.
fn pg_generative_divergence(sql: &str) -> Option<String> {
    let ours = squonk_accepts(sql);
    let pg = postgres_accepts(sql);
    if ours == pg {
        return None;
    }
    if POSTGRES_GENERATIVE_DIVERGENCE_ALLOWLIST
        .iter()
        .any(|e| e.sql == sql)
    {
        return None;
    }
    Some(if ours && !pg {
        format!("over-acceptance: we accept, libpg_query rejects: {sql:?}")
    } else {
        format!("coverage gap: libpg_query accepts, we reject: {sql:?}")
    })
}

#[test]
fn postgres_feature_generative_differential_replays_committed_seeds() {
    let divergences: Vec<String> = POSTGRES_FEATURE_SEEDS
        .iter()
        .filter_map(|&sql| pg_generative_divergence(sql))
        .collect();
    assert!(
        divergences.is_empty(),
        "flag-aware PG generative differential found {} un-ledgered divergence(s) over seeds:\n  {}",
        divergences.len(),
        divergences.join("\n  "),
    );
}

#[test]
fn postgres_feature_generative_differential_explores_flag_aware_surface() {
    use squonk::dialect::Postgres;

    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 512,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &[0xB6; 32]),
    );
    let strategy = arb_feature_statement(Postgres.features(), POSTGRES_FEATURE_PROBES);
    for _ in 0..512 {
        let tree = strategy
            .new_tree(&mut runner)
            .expect("arb_feature_statement is infallible");
        let (_family, sql) = tree.current();
        if let Some(detail) = pg_generative_divergence(&sql) {
            panic!("flag-aware PG generative differential: {detail}");
        }
    }
}

#[test]
fn postgres_generative_allowlist_entries_name_tickets_and_still_diverge() {
    assert_entries_are_ticketed(POSTGRES_GENERATIVE_DIVERGENCE_ALLOWLIST);
    assert_entries_still_diverge(POSTGRES_GENERATIVE_DIVERGENCE_ALLOWLIST, |entry| {
        let ours = squonk_accepts(entry.sql);
        let pg = postgres_accepts(entry.sql);
        ours != pg
    });
}
