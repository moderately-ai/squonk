// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! sqllogictest SQL-corpus conformance — a second broad round-trip target.
//!
//! The vendored fixture `corpus/sqllogictest/statements.sql` is SQL *extracted*
//! from sqllogictest's result-interleaved `.test` files: one statement per line,
//! pulled out of the `statement ok` / `query <types> <sort>` records (the
//! expected-result rows, hashes, and `skipif`/`onlyif` control lines are
//! dropped). See `corpus/sqllogictest/README.md` for the exact source files,
//! per-file caps, and license. Unlike the sqlglot corpus — whose `identity.sql`
//! is byte-for-byte upstream — this fixture is derived, because upstream ships no
//! plain-SQL file; the extraction is the vendoring step. As with sqlglot it is a
//! *growing* coverage target: statements move into the supported subset as the
//! parser grows.
//!
//! ## Partition model
//!
//! The split is decided by *running* every line through the parser, never by
//! hand. Each line falls into exactly one class:
//!
//! - **Ansi-supported** — parses and round-trips under `Ansi` in both the
//!   canonical and fully-parenthesized oracles. These are materialized into the
//!   regenerable `corpus/sqllogictest/supported.sql` and driven through the
//!   public [`assert_roundtrips`](crate::assert_roundtrips) /
//!   [`assert_roundtrips_parenthesized`](crate::assert_roundtrips_parenthesized)
//!   oracles.
//! - **Postgres-only** — needs the `Postgres` preset to parse and round-trip
//!   (the Ansi oracle cannot reach them). Pinned in `SPEC.postgres_only_supported`
//!   and validated under Postgres, so coverage credited here stays honest.
//! - **Round-trip defect** — parses but fails a round-trip oracle. Pinned in
//!   `SPEC.ansi_roundtrip_defects` and tracked under `ROUNDTRIP_DEFECT_TICKET`;
//!   this mirrors the PostgreSQL guide machinery, which keeps unsupported cases
//!   ticketed instead of silently dropped.
//! - **Unparsed** — outside the current surface; tracked implicitly because
//!   `statements.sql` keeps every extracted line. There is no separate
//!   `guide.sql`: the PostgreSQL corpus needs a guide file because it vendors no
//!   full upstream file, whereas `statements.sql` *is* the full extracted corpus,
//!   so a guide file would only duplicate the unsupported majority. The test
//!   instead machine-checks that every line is accounted for in exactly one
//!   class.
//!
//! ## Anti-vanishing guarantees
//!
//! `SPEC.pinned_total` pins the vendored statement count, so a line disappearing
//! from the fixture trips the gate. `supported.sql` is regenerable: running with
//! `REWRITE=1` rewrites it from the current classification, so coverage changes
//! surface as a reviewable diff rather than silently. The two small pinned lists
//! are const-compared, so a Postgres-only case promoting to Ansi, a defect
//! getting fixed, or a fresh regression all fail loudly.
//!
//! ## Lenient
//!
//! Outside the Ansi/Postgres partition above, every vendored line that parses under
//! [`Lenient`](squonk::dialect::Lenient) is separately asserted to round-trip
//! under it ([`assert_accepted_lines_round_trip`](crate::corpus_roundtrip::assert_accepted_lines_round_trip)).
//! Unpinned on purpose: Lenient's whole point is a wide, still-growing acceptance
//! boundary, so only "whatever it accepts stays round-trip-stable" is checked here,
//! not the boundary itself.

use crate::corpus_partition::CorpusSpec;

const SPEC: CorpusSpec = CorpusSpec {
    log_label: "sqllogictest",
    raw_text: include_str!("../corpus/sqllogictest/statements.sql"),
    pinned_total: 373,
    supported_sql: include_str!("../corpus/sqllogictest/supported.sql"),
    supported_relpath: "corpus/sqllogictest/supported.sql",
    banner: "\
-- SPDX-License-Identifier: CC0-1.0
--
-- sqllogictest-corpus supported subset (GENERATED — do not edit by hand).
-- Regenerate: REWRITE=1 cargo nextest run -p squonk-conformance corpus_sqllogictest
--
-- Each line parses and round-trips under the Ansi dialect in both the
-- canonical and fully-parenthesized oracles. Lines are verbatim copies of
-- corpus/sqllogictest/statements.sql, kept in source order. See README.md.

",
    postgres_only_supported: &[],
    ansi_roundtrip_defects: &[],
};

#[cfg(test)]
mod tests {
    use squonk::dialect::Lenient;

    use super::SPEC;

    #[test]
    fn vendored_corpus_is_pinned_and_fully_partitioned() {
        SPEC.vendored_corpus_is_pinned_and_fully_partitioned();
    }

    #[test]
    fn ansi_supported_subset_round_trips_under_both_oracles() {
        SPEC.ansi_supported_subset_round_trips_under_both_oracles();
    }

    #[test]
    fn postgres_only_supported_round_trips_under_postgres() {
        SPEC.postgres_only_supported_round_trips_under_postgres();
    }

    #[test]
    fn ansi_roundtrip_defects_remain_ticketed_and_still_diverge() {
        SPEC.ansi_roundtrip_defects_remain_ticketed_and_still_diverge();
    }

    /// Lenient has no pinned coverage tier of its own (it is the permissive union,
    /// not a growing-surface target like Ansi/Postgres above): every vendored line
    /// it *does* accept must still round-trip under it.
    #[test]
    fn lenient_accepted_lines_round_trip_under_lenient() {
        crate::corpus_roundtrip::assert_accepted_lines_round_trip(SPEC.raw_text, Lenient);
    }
}
