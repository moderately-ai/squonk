// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared "growing corpus" harness for `corpus_sqlglot` and `corpus_sqllogictest`.
//!
//! Both replayers partition a vendored/extracted fixture into the same four
//! classes by running the same classify/rewrite/test logic over it — see either
//! module's doc comment for the partition model and anti-vanishing guarantees.
//! `corpus_roundtrip`'s own doc records that its `roundtrip()` body was already
//! hoisted out of the two files for this reason; the classify/partition/rewrite/
//! test scaffolding around it was left duplicated, differing only in corpus
//! name/path literals and eprintln labels. This module holds that scaffolding
//! once, driven by a per-corpus [`CorpusSpec`] data record. `corpus_complex` is
//! intentionally a different shape (pinned per-dataset counts, no `supported.sql`
//! cache) and stays separate.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use squonk::dialect::{Ansi, Postgres};
use squonk::parse_with;

use crate::corpus_roundtrip::{
    ROUNDTRIP_DEFECT_TICKET, Roundtrip, RoundtripDefectCase, defect_ticket_exists, roundtrip,
};

/// Which surface a corpus line lands in, decided by running the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Parses and round-trips under [`Ansi`] in both render modes.
    AnsiSupported,
    /// Needs [`Postgres`] to parse, and round-trips there.
    PostgresSupported,
    /// Parses under some dialect but fails a round-trip oracle.
    RoundtripDefect,
    /// Outside the current parser surface entirely.
    Unparsed,
}

/// Classify one corpus line: Ansi first, then Postgres as a fallback.
fn classify(sql: &str) -> Class {
    match roundtrip(sql, Ansi) {
        Roundtrip::Ok => Class::AnsiSupported,
        Roundtrip::Failed(_) => Class::RoundtripDefect,
        Roundtrip::Unparsable => match roundtrip(sql, Postgres) {
            Roundtrip::Ok => Class::PostgresSupported,
            Roundtrip::Failed(_) => Class::RoundtripDefect,
            Roundtrip::Unparsable => Class::Unparsed,
        },
    }
}

/// Whether the tests are running in golden-rewrite mode (the repo-wide
/// `REWRITE=1` convention also used by the datadriven goldens).
fn rewrite_mode() -> bool {
    env::var_os("REWRITE").is_some()
}

/// A growing-corpus replayer's data: the vendored fixture, its regenerable
/// `supported.sql` cache, and the two small pinned classification lists — every
/// per-corpus literal the shared classify/rewrite/test harness below needs.
/// Holding this as data (rather than a module of consts + free functions) is
/// what lets `corpus_sqlglot` and `corpus_sqllogictest` share one implementation
/// of each.
pub(crate) struct CorpusSpec {
    /// Human label for the coverage-percentage eprintln (e.g. `"sqlglot"`).
    pub(crate) log_label: &'static str,
    /// The vendored/extracted fixture text, one statement per line, no
    /// blanks/comments.
    pub(crate) raw_text: &'static str,
    /// Pinned `raw_text` line count; a statement silently vanishing from the
    /// fixture trips this gate.
    pub(crate) pinned_total: usize,
    /// The committed `supported.sql` text (`include_str!`'d).
    pub(crate) supported_sql: &'static str,
    /// `supported.sql`'s path relative to `CARGO_MANIFEST_DIR`, so a `REWRITE=1`
    /// run knows where to write it back.
    pub(crate) supported_relpath: &'static str,
    /// The full generated-file banner (SPDX header, regenerate instructions, and
    /// description), written verbatim before the statement lines on `REWRITE=1`.
    pub(crate) banner: &'static str,
    /// Statements that need the [`Postgres`] preset to parse and round-trip, in
    /// `raw_text` order.
    pub(crate) postgres_only_supported: &'static [&'static str],
    /// Statements that parse but fail a round-trip oracle, each carrying a
    /// triage label, in `raw_text` order.
    pub(crate) ansi_roundtrip_defects: &'static [RoundtripDefectCase],
}

impl CorpusSpec {
    /// Every vendored statement, in source order (one per line, no blanks/comments).
    fn statements(&self) -> Vec<&'static str> {
        self.raw_text.lines().collect()
    }

    /// The committed supported subset, skipping the SPDX/banner header. Corpus
    /// statements never start with `--` and are never blank, so this cleanly
    /// recovers the statement lines.
    fn committed_supported(&self) -> Vec<&'static str> {
        self.supported_sql
            .lines()
            .filter(|line| !line.trim_start().starts_with("--") && !line.trim().is_empty())
            .collect()
    }

    fn supported_sql_path(&self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(self.supported_relpath)
    }

    /// Rewrite `supported.sql` from the current classification.
    fn rewrite_supported(&self, statements: &[&str]) {
        let mut out = String::from(self.banner);
        for statement in statements {
            out.push_str(statement);
            out.push('\n');
        }
        fs::write(self.supported_sql_path(), out).expect("rewrite supported.sql");
    }

    pub(crate) fn vendored_corpus_is_pinned_and_fully_partitioned(&self) {
        let lines = self.statements();
        assert_eq!(
            lines.len(),
            self.pinned_total,
            "vendored {} corpus statement count changed; if intentional, update pinned_total",
            self.log_label,
        );

        let mut ansi = Vec::new();
        let mut postgres = Vec::new();
        let mut defects = Vec::new();
        let mut unparsed = 0usize;
        for line in &lines {
            match classify(line) {
                Class::AnsiSupported => ansi.push(*line),
                Class::PostgresSupported => postgres.push(*line),
                Class::RoundtripDefect => defects.push(*line),
                Class::Unparsed => unparsed += 1,
            }
        }

        // Every line lands in exactly one class.
        assert_eq!(
            ansi.len() + postgres.len() + defects.len() + unparsed,
            lines.len(),
            "classification did not cover every statement"
        );

        if rewrite_mode() {
            self.rewrite_supported(&ansi);
            eprintln!(
                "REWRITE: wrote {} Ansi-supported statements to supported.sql",
                ansi.len()
            );
            eprintln!("REWRITE: POSTGRES_ONLY_SUPPORTED should be:\n{postgres:#?}");
            // Defects need a human triage label (RoundtripDefect), so they are
            // surfaced for review rather than auto-written into the const.
            eprintln!(
                "REWRITE: ANSI_ROUNDTRIP_DEFECTS should be (triage-label each):\n{defects:#?}"
            );
            return;
        }

        // supported.sql is the regenerable cache of the Ansi-supported set.
        assert_eq!(
            self.committed_supported(),
            ansi,
            "supported.sql is stale; regenerate with REWRITE=1"
        );
        // The two small curated classes are pinned exactly.
        assert_eq!(
            postgres, self.postgres_only_supported,
            "POSTGRES_ONLY_SUPPORTED drifted; update the const"
        );
        let expected_defects: Vec<&str> = self
            .ansi_roundtrip_defects
            .iter()
            .map(|case| case.sql)
            .collect();
        assert_eq!(
            defects, expected_defects,
            "ANSI_ROUNDTRIP_DEFECTS drifted; update the const (and triage-label new cases)"
        );

        let supported = ansi.len() + postgres.len();
        eprintln!(
            "{label} coverage: {supported}/{total} = {pct:.2}% \
             (ansi {ansi}, postgres-only {pg}, round-trip defects {def}, unparsed {un})",
            label = self.log_label,
            total = lines.len(),
            pct = 100.0 * supported as f64 / lines.len() as f64,
            ansi = ansi.len(),
            pg = postgres.len(),
            def = defects.len(),
            un = unparsed,
        );
    }

    pub(crate) fn ansi_supported_subset_round_trips_under_both_oracles(&self) {
        // Drive the committed file through the public oracles the ticket names,
        // so a regression fails here even before the partition check.
        for sql in self.committed_supported() {
            crate::assert_roundtrips(sql);
            crate::assert_roundtrips_parenthesized(sql);
        }
    }

    pub(crate) fn postgres_only_supported_round_trips_under_postgres(&self) {
        for sql in self.postgres_only_supported {
            match roundtrip(sql, Postgres) {
                Roundtrip::Ok => {}
                Roundtrip::Failed(message) => panic!("{sql:?} no longer round-trips: {message}"),
                Roundtrip::Unparsable => {
                    panic!(
                        "{sql:?} no longer parses under Postgres; update POSTGRES_ONLY_SUPPORTED"
                    )
                }
            }
            // If it has started parsing under Ansi it belongs in supported.sql.
            assert!(
                matches!(roundtrip(sql, Ansi), Roundtrip::Unparsable),
                "{sql:?} now parses under Ansi; move it into supported.sql (REWRITE=1)"
            );
        }
    }

    pub(crate) fn ansi_roundtrip_defects_remain_ticketed_and_still_diverge(&self) {
        assert!(
            defect_ticket_exists(),
            "round-trip defect ticket {ROUNDTRIP_DEFECT_TICKET} must exist"
        );

        for case in self.ansi_roundtrip_defects {
            let sql = case.sql;
            assert!(
                parse_with(sql, Ansi).is_ok(),
                "{sql:?} should still parse under Ansi"
            );
            assert!(
                matches!(roundtrip(sql, Ansi), Roundtrip::Failed(_)),
                "{sql:?} now round-trips; remove it from ANSI_ROUNDTRIP_DEFECTS (it will move to \
                 supported.sql on REWRITE=1)"
            );
        }
    }
}
