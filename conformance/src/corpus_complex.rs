// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! sqlglot complex-query corpus conformance — the realistic-complex counterpart to
//! the single-line [`corpus_sqlglot`](crate::corpus_sqlglot) identity corpus.
//!
//! The vendored fixtures under `corpus/sqlglot-complex/` are the *input* queries of
//! sqlglot's TPC-H / TPC-DS benchmark suites and its CTE/subquery optimizer
//! fixtures (see `corpus/sqlglot-complex/README.md` for provenance and the
//! extraction recipe). They are deliberately broad — multi-dialect, deeply nested,
//! INTERVAL/date arithmetic, correlated subqueries — so most exceed the M1 surface
//! today. Like the identity corpus, this is a *growing* coverage target.
//!
//! ## Two oracles: parse-acceptance coverage + round-trip stability
//!
//! Mirrors `corpus_sqlglot`'s "run the parser to decide the subset" model. Two
//! independent checks run over the same fixtures.
//!
//! *Parse-acceptance coverage* partitions every statement into exactly one class:
//!
//! - **Ansi** — accepted by [`parse_with`] under [`Ansi`].
//! - **Postgres-only** — rejected under `Ansi` but accepted under [`Postgres`].
//! - **Unparsed** — outside the current surface under either preset.
//!
//! Parse acceptance is the coverage metric on purpose: it is exactly the predicate
//! the upstream comparison bench uses to pick the both-accept subset it measures
//! (`bench/benches/upstream/mod.rs`), so the coverage this test pins and the subset
//! the bench times are the same set.
//!
//! *Round-trip stability* (ADR-0014 P3, prod-corpus-idempotence-stability) then asks
//! the stronger question over the *accepted* subset: does each accepted statement
//! parse -> render -> re-parse back to a structurally equal tree, in both render
//! modes, under the preset that accepted it? This is where the large multi-dialect
//! queries earn their keep beyond a lenient accept. The three outcomes are kept
//! distinct: a parse-reject (tracked by the coverage partition above), a
//! *renderable-stable* accept (the majority — counted and pinned), and an
//! *accept-only* case the parser takes but a render mode cannot yet round-trip. The
//! last class is triage-labelled and pinned in [`COMPLEX_ROUNDTRIP_DEFECTS`] via the
//! shared `corpus_roundtrip` machinery, so a query accepted-but-not-renderable is
//! recorded with its class rather than silently credited as "covered".
//!
//! ## Anti-vanishing guarantees
//!
//! Every dataset pins its candidate count and its per-preset acceptance counts
//! ([`DATASETS`]). A statement vanishing from a fixture trips the candidate pin; a
//! query silently dropping out of (or into) the parseable surface trips an
//! acceptance pin, surfacing as a reviewable diff rather than a silent shift. The
//! failure message prints the freshly measured table in the exact shape of the pin,
//! so refreshing it on an intentional change is a copy-paste (the
//! `tests/corpus_allocations.rs` convention). Round-trip stability pins the same
//! way: [`COMPLEX_RENDERABLE_STABLE_TOTAL`] fixes the renderable-stable count and
//! [`COMPLEX_ROUNDTRIP_DEFECTS`] fixes the accept-only set, so a statement sliding
//! between renderable and accept-only flips both pins together.
//!
//! Outside the pinned Ansi/Postgres/renderable-stable machinery above, every
//! statement across every dataset that parses under
//! [`Lenient`](squonk::dialect::Lenient) is separately asserted to round-trip
//! under it, unpinned (mirrors `corpus_sqlglot`/`corpus_sqllogictest`'s sibling
//! check): Lenient's defining trait is a wide, still-growing acceptance boundary, so
//! only round-trip stability is checked, not the boundary itself.

use squonk::dialect::{Ansi, Postgres};
use squonk::parse_with;

use crate::corpus_roundtrip::{
    ROUNDTRIP_DEFECT_TICKET, Roundtrip, RoundtripDefect, RoundtripDefectCase, defect_ticket_exists,
    roundtrip,
};

/// One vendored dataset: its name, its `include_str!`'d text, and the pinned
/// counts. `candidates` is the total statement count (anti-vanishing); `ansi` and
/// `postgres` are the parse-acceptance counts under each preset, with
/// `postgres >= ansi` because the Postgres preset accepts a superset of Ansi.
struct Dataset {
    name: &'static str,
    text: &'static str,
    candidates: usize,
    ansi: usize,
    postgres: usize,
}

const TPC_H: &str = include_str!("../corpus/sqlglot-complex/tpc-h.sql");
const TPC_DS: &str = include_str!("../corpus/sqlglot-complex/tpc-ds.sql");
const MERGE_SUBQUERIES: &str = include_str!("../corpus/sqlglot-complex/merge_subqueries.sql");
const UNNEST_SUBQUERIES: &str = include_str!("../corpus/sqlglot-complex/unnest_subqueries.sql");
const PUSHDOWN_CTE_ALIAS_COLUMNS: &str =
    include_str!("../corpus/sqlglot-complex/pushdown_cte_alias_columns.sql");
const ELIMINATE_CTES: &str = include_str!("../corpus/sqlglot-complex/eliminate_ctes.sql");

/// The vendored datasets, in a fixed order so every derived number is
/// deterministic and git-diffable. Candidate counts are the verified extraction
/// sizes; acceptance counts are measured (refresh via the failure message).
///
/// Acceptance covers typed DATE/TIME/TIMESTAMP/INTERVAL literals
/// (`prod-literal-date-time-interval`), and every accepted
/// statement here additionally round-trips structurally under Ansi, so these are
/// genuine full parses, not lenient accepts.
const DATASETS: &[Dataset] = &[
    Dataset {
        name: "tpc-h",
        text: TPC_H,
        candidates: 22,
        ansi: 22,
        postgres: 22,
    },
    Dataset {
        name: "tpc-ds",
        text: TPC_DS,
        candidates: 99,
        ansi: 99,
        postgres: 99,
    },
    Dataset {
        name: "merge_subqueries",
        text: MERGE_SUBQUERIES,
        candidates: 67,
        ansi: 66,
        postgres: 66,
    },
    Dataset {
        name: "unnest_subqueries",
        text: UNNEST_SUBQUERIES,
        candidates: 38,
        ansi: 34,
        postgres: 36,
    },
    Dataset {
        name: "pushdown_cte_alias_columns",
        text: PUSHDOWN_CTE_ALIAS_COLUMNS,
        candidates: 6,
        ansi: 4,
        postgres: 4,
    },
    Dataset {
        name: "eliminate_ctes",
        text: ELIMINATE_CTES,
        candidates: 6,
        ansi: 5,
        postgres: 5,
    },
];

/// Pinned count of accepted statements that round-trip structurally in *both* render
/// modes under their accepting preset. A statement regressing from renderable-stable
/// to an accept-only defect (or a defect getting fixed) flips this and
/// [`COMPLEX_ACCEPT_ONLY_DEFECTS`] together. Equal to the 232 accepted statements the
/// parse-acceptance pins total (230 Ansi + 2 Postgres-only): every accepted statement
/// now round-trips in both render modes, leaving no accept-only defect.
///
/// The former 11-case accept-only cluster — 3+-arm set-operation subqueries in
/// derived-table (FROM) position whose Parenthesized render the parser rejected — was
/// fixed in
/// `parse-parenthesized-set-operation-operand-in-derived-table-from-position`, which
/// also lifted one previously-unparsed TPC-DS query (Q87,
/// `FROM ((SELECT …) EXCEPT … ) cool_cust`) into the accepted-and-stable set.
const COMPLEX_RENDERABLE_STABLE_TOTAL: usize = 232;

/// Pinned count of *accept-only* cases — accepted by the parser but not round-trip
/// stable — kept distinct from the renderable-stable majority and from parse-rejects.
/// Zero since the parenthesized-set-op-derived-table cluster was fixed; a regression
/// reintroducing an accept-only case trips this pin and the
/// [`COMPLEX_DEFECT_SIGNATURE`] triage guard together.
const COMPLEX_ACCEPT_ONLY_DEFECTS: usize = 0;

/// The triage signature that the (now-fixed) accept-only cluster carried: a 3+-arm
/// set-operation subquery in derived-table (FROM) position whose Parenthesized render
/// — `FROM ((SELECT ...) UNION ...) x` — the parser once rejected with this
/// expected-token phrase. Retained as a guard: should any future accept-only defect
/// reappear, the stability test asserts it against this signature so an *unrelated*
/// regression cannot silently masquerade as the known (closed) cluster.
const COMPLEX_DEFECT_SIGNATURE: &str = "a joined table inside parentheses";

/// Minimal, readable repros of accept-only defect clusters, each carrying a
/// [`RoundtripDefect`] triage label and tracked under [`ROUNDTRIP_DEFECT_TICKET`].
///
/// Empty: the parenthesized-set-op-derived-table cluster this corpus once tracked is
/// fixed (its repro now round-trips and lives in the supported corpora — `lib::CORPUS`
/// and `pg::PG_DERIVED_TABLE_SET_OP_CORPUS`). A future accept-only cluster is
/// represented here by a hand-minimized repro (the `corpus_generated::SHRUNK_REGRESSIONS`
/// philosophy: a small repro, not a large draw), asserted to still parse, still
/// round-trip-fail, and still match its signature so the triage record cannot rot.
const COMPLEX_ROUNDTRIP_DEFECTS: &[RoundtripDefectCase] = &[];

/// Drop a leading run of blank lines and `--` line-comments from a `;`-delimited
/// chunk (the SPDX/banner header precedes the first statement), returning the
/// trimmed remainder. Sub-slicing preserves the `'static` lifetime of the corpus
/// text. Matches the bench loader (`bench/benches/corpus/mod.rs`) so both cut the
/// vendored files into the same statements.
fn strip_leading_comment_lines(chunk: &str) -> &str {
    let mut rest = chunk.trim();
    while rest.starts_with("--") {
        let cut = rest.find('\n').map_or(rest.len(), |i| i + 1);
        rest = rest[cut..].trim_start();
    }
    rest.trim_end()
}

/// Cut one vendored dataset file into its statements, in source order. The vendored
/// files carry no semicolon inside any statement (enforced at vendoring time and
/// pinned by the candidate count), so a plain `;` split is exact.
fn statements(text: &str) -> Vec<&str> {
    text.split(';')
        .map(strip_leading_comment_lines)
        .filter(|sql| !sql.is_empty())
        .collect()
}

/// Which surface a statement lands in, decided by running the parser.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Accepted under [`Ansi`].
    Ansi,
    /// Rejected under `Ansi` but accepted under [`Postgres`].
    PostgresOnly,
    /// Outside the current surface under either preset.
    Unparsed,
}

/// Classify one statement: Ansi first, then Postgres as a fallback.
fn classify(sql: &str) -> Class {
    if parse_with(sql, squonk::ParseConfig::new(Ansi)).is_ok() {
        Class::Ansi
    } else if parse_with(sql, squonk::ParseConfig::new(Postgres)).is_ok() {
        Class::PostgresOnly
    } else {
        Class::Unparsed
    }
}

/// Measured coverage of one dataset.
struct Coverage {
    name: &'static str,
    candidates: usize,
    ansi: usize,
    postgres: usize,
}

/// Measure one dataset's coverage by classifying every statement.
fn measure(dataset: &Dataset) -> Coverage {
    let all = statements(dataset.text);
    let mut ansi = 0;
    let mut postgres = 0;
    for sql in &all {
        match classify(sql) {
            Class::Ansi => {
                ansi += 1;
                postgres += 1;
            }
            Class::PostgresOnly => postgres += 1,
            Class::Unparsed => {}
        }
    }
    Coverage {
        name: dataset.name,
        candidates: all.len(),
        ansi,
        postgres,
    }
}

/// Render-stability measurement over the accepted subset of every dataset, in fixed
/// source order.
///
/// For each accepted statement, round-trips it under the preset that accepted it
/// (Ansi if Ansi takes it, else Postgres) and sorts the outcome into renderable
/// -stable (round-trips in both modes) or accept-only defect (parses but a render
/// fails to round-trip). Parse-rejects are skipped — they are the coverage
/// partition's concern, not stability's. Returns `(stable_count, defects)` where
/// each defect is its SQL paired with the round-trip diff, for triage.
fn measure_stability() -> (usize, Vec<(&'static str, String)>) {
    let mut stable = 0usize;
    let mut defects = Vec::new();
    for dataset in DATASETS {
        for sql in statements(dataset.text) {
            let outcome = match classify(sql) {
                Class::Ansi => roundtrip(sql, Ansi),
                Class::PostgresOnly => roundtrip(sql, Postgres),
                Class::Unparsed => continue,
            };
            match outcome {
                Roundtrip::Ok => stable += 1,
                Roundtrip::Failed(message) => defects.push((sql, message)),
                // `classify` already proved the statement parses under the chosen
                // preset, so re-parsing the same text cannot reject; surface the
                // impossible loudly rather than miscount it as stable.
                Roundtrip::Unparsable => {
                    unreachable!("accepted statement no longer parses under its preset: {sql:?}")
                }
            }
        }
    }
    (stable, defects)
}

#[cfg(test)]
mod tests {
    use squonk::dialect::Lenient;

    use super::*;

    #[test]
    fn complex_corpus_is_pinned_and_coverage_is_reported() {
        let measured: Vec<Coverage> = DATASETS.iter().map(measure).collect();

        // Print the per-dataset coverage and a copy-pasteable pin block, so an
        // intentional coverage change is refreshed by reading this output.
        let mut total_c = 0usize;
        let mut total_a = 0usize;
        let mut total_p = 0usize;
        eprintln!("sqlglot complex-corpus coverage (parse acceptance):");
        for cov in &measured {
            eprintln!(
                "  {:<28} candidates {:>3}  ansi {:>3} ({:>5.1}%)  postgres {:>3} ({:>5.1}%)",
                cov.name,
                cov.candidates,
                cov.ansi,
                100.0 * cov.ansi as f64 / cov.candidates as f64,
                cov.postgres,
                100.0 * cov.postgres as f64 / cov.candidates as f64,
            );
            total_c += cov.candidates;
            total_a += cov.ansi;
            total_p += cov.postgres;
        }
        eprintln!(
            "  {:<28} candidates {total_c:>3}  ansi {total_a:>3} ({:>5.1}%)  postgres {total_p:>3} ({:>5.1}%)",
            "TOTAL",
            100.0 * total_a as f64 / total_c as f64,
            100.0 * total_p as f64 / total_c as f64,
        );
        eprintln!("# pin block (candidates, ansi, postgres):");
        for cov in &measured {
            eprintln!(
                "#   {:<28} {:>3} {:>3} {:>3}",
                cov.name, cov.candidates, cov.ansi, cov.postgres
            );
        }

        // Pin every dataset: candidate count (anti-vanishing) and the per-preset
        // acceptance counts (coverage drift). Asserted per field so a failure names
        // the dataset and metric that moved.
        for (dataset, cov) in DATASETS.iter().zip(&measured) {
            assert_eq!(
                cov.candidates, dataset.candidates,
                "{}: candidate count changed (fixture drifted?); update DATASETS",
                dataset.name
            );
            assert_eq!(
                cov.ansi, dataset.ansi,
                "{}: Ansi parse-acceptance changed; update DATASETS",
                dataset.name
            );
            assert_eq!(
                cov.postgres, dataset.postgres,
                "{}: Postgres parse-acceptance changed; update DATASETS",
                dataset.name
            );
            assert!(
                cov.postgres >= cov.ansi,
                "{}: Postgres must accept a superset of Ansi",
                dataset.name
            );
        }
    }

    /// The stronger oracle (ADR-0014 P3): every accepted statement either round-trips
    /// structurally in both render modes, or is a tracked, triage-labelled accept-only
    /// defect. This is what makes corpus idempotence *meaningful* for the complex
    /// queries — beyond the parse-acceptance coverage the test above pins. (Runs in
    /// ~0.1 s over the whole corpus, so it stays default-on rather than `#[ignore]`d.)
    #[test]
    fn complex_corpus_accepted_subset_round_trips_or_is_a_tracked_defect() {
        let (stable, defects) = measure_stability();

        // Print before asserting (the module's "measure, print, pin" idiom), so a
        // drift shows the fresh counts and the round-trip diffs to triage, in
        // copy-pasteable form.
        eprintln!(
            "sqlglot complex-corpus round-trip stability: {stable} renderable-stable, \
             {} accept-only defect(s) (cluster: {})",
            defects.len(),
            RoundtripDefect::ParserBug.description(),
        );
        for (sql, message) in &defects {
            eprintln!("# accept-only defect: {sql:?}");
            for line in message.lines() {
                eprintln!("#   {line}");
            }
        }

        assert!(
            defect_ticket_exists(),
            "round-trip defect ticket {ROUNDTRIP_DEFECT_TICKET} must exist"
        );

        // Every accept-only corpus case is the one known cluster: a *different* class
        // of non-stability would not carry this signature, so it cannot hide inside the
        // pinned count. (See COMPLEX_DEFECT_SIGNATURE for the cluster's triage.)
        for (sql, message) in &defects {
            assert!(
                message.contains(COMPLEX_DEFECT_SIGNATURE),
                "unclassified complex round-trip defect (not the known ParserBug cluster); \
                 triage it and update the consts: {sql:?}\n{message}"
            );
        }

        // Accept-only count is pinned *separately* from the renderable-stable count: a
        // parse-accepted-but-not-renderable case is a distinct class from both a clean
        // round-trip and a parse-reject (which the coverage test tracks).
        assert_eq!(
            defects.len(),
            COMPLEX_ACCEPT_ONLY_DEFECTS,
            "complex-corpus accept-only defect count changed; update COMPLEX_ACCEPT_ONLY_DEFECTS"
        );
        assert_eq!(
            stable, COMPLEX_RENDERABLE_STABLE_TOTAL,
            "complex-corpus renderable-stable count changed; update COMPLEX_RENDERABLE_STABLE_TOTAL"
        );

        // The minimal labelled repro(s) still exhibit the cluster, so the readable
        // triage record stays honest as the parser evolves.
        for case in COMPLEX_ROUNDTRIP_DEFECTS {
            eprintln!(
                "# minimal repro [{:?}: {}]: {:?}",
                case.label,
                case.label.description(),
                case.sql,
            );
            match roundtrip(case.sql, Ansi) {
                Roundtrip::Failed(message) => assert!(
                    message.contains(COMPLEX_DEFECT_SIGNATURE),
                    "minimal repro {:?} fails for a different reason now: {message}",
                    case.sql
                ),
                Roundtrip::Ok => panic!(
                    "minimal repro {:?} now round-trips — the {:?} cluster may be fixed; \
                     update COMPLEX_ROUNDTRIP_DEFECTS and the pinned counts",
                    case.sql, case.label
                ),
                Roundtrip::Unparsable => {
                    panic!("minimal repro {:?} no longer parses under Ansi", case.sql)
                }
            }
        }
    }

    /// Lenient has no pinned coverage tier here either (mirrors the sibling test in
    /// `corpus_sqlglot`/`corpus_sqllogictest`): every statement across every dataset
    /// that Lenient accepts must still round-trip under it. No dataset-level pin —
    /// Lenient's defining trait is a wide, evolving acceptance boundary, so only the
    /// round-trip-stability property is checked, not the boundary itself.
    #[test]
    fn lenient_accepted_statements_round_trip_under_lenient() {
        for dataset in DATASETS {
            for sql in statements(dataset.text) {
                match roundtrip(sql, Lenient) {
                    Roundtrip::Ok | Roundtrip::Unparsable => {}
                    Roundtrip::Failed(message) => panic!(
                        "{}: {sql:?} parses under Lenient but does not round-trip: {message}",
                        dataset.name
                    ),
                }
            }
        }
    }
}
