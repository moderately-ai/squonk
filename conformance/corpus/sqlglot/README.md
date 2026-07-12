<!-- SPDX-License-Identifier: MIT -->

# sqlglot Identity Corpus

This directory vendors sqlglot's transpiler identity fixture as a broad, growing round-trip target for the local conformance tests. Source material comes from the upstream sqlglot repository at https://github.com/tobymao/sqlglot, at commit `fd6d4d61c25e7918118fc22c5579098a86a58e10` and is covered by the MIT license (see `LICENSE` in this directory, copied verbatim from that repository).

`identity.sql` is `tests/fixtures/identity.sql` vendored byte-for-byte: 955 single-line statements and expressions that sqlglot round-trips across many SQL dialects. Because it is multi-dialect transpiler fixture data, most lines fall outside the M1 ANSI surface today — that is the point. This corpus is a coverage target that grows as the parser grows, not a set of statements expected to all pass now. Its companion `identity.sql.license` carries the SPDX marker so the file stays byte-identical to upstream while the license gate still sees attribution.

## Supported vs guide

The split is decided by running every line through `squonk::parse_with`, never by hand. `supported.sql` holds the subset that parses and round-trips under the `Ansi` dialect in both the canonical and fully-parenthesized oracles; it is a regenerable, source-ordered, verbatim copy of those `identity.sql` lines, exercised through `assert_roundtrips` and `assert_roundtrips_parenthesized`.

There is no separate `guide.sql`. The PostgreSQL corpus needs a guide file because it vendors no full upstream file, so un-promoted statements would otherwise be lost; here `identity.sql` is the full corpus, so every not-yet-supported statement is already tracked there and a `guide.sql` would only duplicate the unsupported majority. The conformance test instead machine-checks that every one of the 955 lines lands in exactly one class, so nothing is silently dropped, and that `supported.sql` matches the live classification so it can never drift.

Two small classes are pinned in the test (`conformance/src/corpus_sqlglot.rs`) rather than in a file, mirroring how the PostgreSQL guide keeps unsupported cases ticketed instead of silently dropped:

- Postgres-only: statements that need the `Postgres` preset to parse and round-trip, which the `Ansi` round-trip oracle cannot reach. They are validated under `Postgres` and credited to coverage — the ticket's "fall back to Postgres only if a statement needs it, and say which".
- Round-trip defects: statements that parse but fail a round-trip oracle. The one current case renders correctly in canonical form but breaks the fully-parenthesized oracle, where `WHERE ((values + 1) > 3)` re-lexes the non-reserved keyword `values` as the start of a `VALUES` row after `(`. It is tracked under `prod-corpus-idempotence-stability` (corpus render-stability triage), not fixed here — parser changes are out of scope for corpus ingestion.

## Coverage

At the vendored commit, 387 of 955 statements (40.52%) are validated: 379 round-trip under `Ansi` and 8 more under `Postgres`. The remaining 567 are outside the current surface and 1 is the tracked round-trip defect. The conformance test reports this breakdown on every run.

## Regenerating

`supported.sql` is regenerable. Running the conformance tests with `REWRITE=1` — the same convention as the datadriven goldens — rewrites it from the current classification and prints the suggested `POSTGRES_ONLY_SUPPORTED` and `ANSI_ROUNDTRIP_DEFECTS` lists for the test consts:

    REWRITE=1 cargo nextest run -p squonk-conformance corpus_sqlglot

Because the supported set is a checked-in cache, coverage changes surface as a reviewable diff rather than silently.
