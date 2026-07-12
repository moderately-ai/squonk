// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Bolero fuzz targets for the M1 parser surface.
//!
//! The functions in this module are ordinary Rust entry points so the stable
//! `cargo test` Bolero checks and a later nightly/libFuzzer wrapper can share the
//! same target bodies. Raw SQL bytes exercise tokenizer/parser panic sites, while
//! the structured target feeds an `arbitrary`-generated legal AST subset through
//! the parenthesized render-and-reparse structural oracle from ADR-0014.

use arbitrary::{Arbitrary, Unstructured};
use squonk::dialect::{Ansi, DuckDb, Lenient, MySql, Postgres, Sqlite};
use squonk::{Parsed, parse_with};
use squonk_ast::render::RenderMode;
use squonk_ast::{ForeignKeyMatch, InsertSource, NoExt, Statement};

use crate::pg;
use crate::properties::{GENERATED_RESOLVER, normalize_statement, render_generated};
use crate::shared_interner;
use crate::spans;

mod builders;
mod pg_comparable;

use builders::*;
use pg_comparable::structurally_pg_comparable;

/// Raised from the original 4096 to 64 KiB (`fuzz-widen-dialect-and-render-coverage`):
/// measured against the stable `bolero_parse_no_panic_runs_under_cargo_test` gate and
/// a `cargo +nightly fuzz run parse_no_panic -- -runs=5000` smoke, both showed no
/// meaningful runtime change (Bolero: ~0.02s either way; libFuzzer: 5000 runs/s),
/// because almost every generated buffer still fails to lex/parse in the first few
/// bytes regardless of the ceiling — raising the ceiling mostly stops silently
/// dropping the rare larger generated input rather than making the common case
/// costlier. Still far short of the ~2 MB / 100k-statement adversarial-stress regime
/// the dedicated (and deliberately throttled) stress-benchmark tickets cover — this
/// cap is sized for a per-iteration fuzz/property budget, not a stress benchmark.
const MAX_PARSE_INPUT_BYTES: usize = 65536;
const MAX_ROUNDTRIP_INPUT_BYTES: usize = 4096;

const ZERO_ROUNDTRIP_REPLAY: [u8; 256] = [0; 256];
const ONE_ROUNDTRIP_REPLAY: [u8; 256] = [1; 256];
const MIXED_ROUNDTRIP_REPLAY: [u8; 256] = [0x5a; 256];

/// Minimized raw-SQL replay cases for `parse_no_panic`.
///
/// The collection-literal seeds (duckdb-collection-literals) exercise the DuckDb
/// grammar's accept paths and every dialect's reject paths, including the
/// unterminated-bracket/brace boundaries that mirror the unterminated-string seed.
pub const PARSE_NO_PANIC_REPLAYS: &[&[u8]] = &[
    b"",
    b";;;",
    b"SELECT",
    b"SELECT FROM",
    b"SELECT 'unterminated",
    b"SELECT /* unterminated",
    b"SELECT a FROM t LEFT JOIN",
    b"SELECT a < b < c",
    b"SELECT [{'a': 42}, {'b': 84}]",
    b"SELECT MAP {[1, 2]: {'k': []}}",
    b"SELECT [1, 2",
    b"SELECT {'a':",
    b"SELECT MAP {",
    b"SELECT [1][:2][3:]",
    // SQLite-distinctive surfaces (oracle-parity-sqlite): the misfeature family the
    // flag-aware generator exercises, seeded here so the raw mutator starts from real
    // SQLite shapes rather than rediscovering the `[bracket]`/backtick quotes, the
    // typeless/liberal-type CREATE TABLE, INDEXED BY, the postfix null tests, and bare IS.
    b"CREATE TABLE t (a, b)",
    b"CREATE TABLE t (a UNSIGNED BIG INT)",
    b"SELECT * FROM t INDEXED BY ix",
    b"SELECT 1 WHERE 1 NOT NULL",
    b"SELECT 1 IS 1",
    b"SELECT 1 AS `back_ticked`",
    b"SELECT 1 AS [bracketed]",
];

/// Minimized arbitrary-byte replay cases for the structured round-trip target.
pub const ROUNDTRIP_REPLAYS: &[&[u8]] = &[
    &ZERO_ROUNDTRIP_REPLAY,
    &ONE_ROUNDTRIP_REPLAY,
    &MIXED_ROUNDTRIP_REPLAY,
];

/// Minimized arbitrary-byte replay cases for the PostgreSQL differential target.
///
/// Seeds that exercise the differential loop without diverging today; a future
/// fuzz-found-and-minimized divergence is committed here alongside its triage (an
/// allowlist entry naming a ticket, or a parser/mapping fix).
pub const DIFFERENTIAL_REPLAYS: &[&[u8]] = &[
    &ZERO_ROUNDTRIP_REPLAY,
    &ONE_ROUNDTRIP_REPLAY,
    &MIXED_ROUNDTRIP_REPLAY,
];

/// Seed + replay corpus for the raw-byte PostgreSQL differential target.
///
/// Unlike [`DIFFERENTIAL_REPLAYS`] (arbitrary bytes decoded into a legal generated
/// AST), these are raw SQL bytes fed straight to the accept/reject oracle, so they
/// double as libFuzzer seeds: the mutator starts from real SQL shapes rather than
/// having to rediscover the grammar. Every entry must be accept/reject-agreeing
/// today (`None`-or-allowlisted) so `pg_differential_raw_bytes_replays_committed_inputs`
/// stays green; a fuzz-found, minimized divergence lands here alongside its triage (a
/// `PG_DIVERGENCE_ALLOWLIST` entry naming a ticket, or a parser/mapping fix).
///
/// The first two are hand-picked divergence seeds — a
/// zero-length quoted identifier and a `U&` escape of the NUL code point — both now
/// REJECTED by both parsers, i.e. good divergence-free negative seeds
/// (eager-validate-unicode-escape-strings-for-oracle-parity). The rest are near-miss
/// shapes around them (accepted or rejected by both) that give the mutator productive
/// starting points for the actual over-acceptance hunt.
pub const PG_DIFFERENTIAL_RAW_BYTES_REPLAYS: &[&[u8]] = &[
    br#"SELECT "" FROM t"#,
    br"U&'\0000'",
    br#"SELECT "a" FROM t"#,
    br"SELECT U&'\0041' FROM t",
    b"SELECT 1",
    b"SELECT * FROM t WHERE a = 1",
    b"VALUES (1)",
    b"SELECT a b c",
    // The `TABLE name` command and empty SELECT target list, now parsed
    // (parse-pg-table-command-and-empty-select): both directions of each newly-armed
    // surface — the accepted spellings and the reject boundary PostgreSQL enforces
    // (`TABLE t x` has no alias; `SELECT DISTINCT` requires a target list).
    b"TABLE t",
    b"TABLE ONLY t",
    b"TABLE a UNION TABLE b",
    b"TABLE t x",
    b"SELECT",
    b"SELECT;",
    b"SELECT FROM t",
    b"SELECT WHERE a = 1",
    b"SELECT DISTINCT",
    // The PostgreSQL `DO` anonymous code block, one of the three residual under-acceptance
    // classes the fuzz-mute-repoint census surfaced (pg-do-statement-body), now closed. Both
    // directions of the surface: the accepted spellings — both `LANGUAGE`/body orders, plain
    // and dollar-quoted bodies, and the raw-parse forms libpg_query accepts but only rejects
    // at execution (a language-only block, a repeated body, a repeated language) — and the
    // reject boundary PostgreSQL enforces at parse (a bodyless bare `DO`, a non-string item,
    // a dangling `LANGUAGE`, a trailing token, and a reserved-word language name).
    b"DO $$BEGIN NULL; END$$",
    b"DO 'BEGIN NULL; END'",
    b"DO LANGUAGE plpgsql $$x$$",
    b"DO $$x$$ LANGUAGE plpgsql",
    b"DO LANGUAGE plpgsql",
    b"DO $$a$$ $$b$$",
    b"DO 'x' LANGUAGE a LANGUAGE b",
    b"DO",
    b"DO 42",
    b"DO $$x$$ LANGUAGE",
    b"DO 'x' FOO",
    b"DO LANGUAGE select 'x'",
    // The `DO` argument literal-type boundary (pg-do-arg-literal-type-discrimination), a
    // class distinct from the statement-body and separator fixes: PostgreSQL's `DO`
    // discriminates string-literal *types*. Both directions of the surface. The
    // OVER-acceptance fixed: a code block is an `Sconst`, so a bit-string (`b'…'`/`x'…'`,
    // a `bit`-typed `BCONST`/`XCONST`) or national (`N'…'`, a bare `N` word to pg) constant
    // is a syntax error there, not a body — each rejected now, agreeing with pg. The
    // UNDER-acceptance fixed: the `LANGUAGE` operand is a `NonReservedWord_or_Sconst`, so an
    // `Sconst` string (plain, `E'…'`, dollar-quoted) is accepted alongside a bare word — and
    // a bit/hex constant there stays a reject, matching pg.
    b"DO b'0'",
    b"DO x'ab'",
    b"DO N'x'",
    b"DO 'x' LANGUAGE 'plpgsql'",
    b"DO 'x' LANGUAGE E'plpgsql'",
    b"DO LANGUAGE 'plpgsql' $$x$$",
    b"DO 'x' LANGUAGE b'0'",
    b"DO 'x' LANGUAGE x'ab'",
    // `DO LANGUAGE N'p'` was the one residual of the DO-arg soak
    // (pg-national-strings-lexing-divergence): the PG preset armed `national_strings`, so we
    // lexed `N'p'` as one national-string token, which the `NonReservedWord_or_Sconst`
    // LANGUAGE operand rejects (not an `Sconst`), while PostgreSQL — which has no `N'…'`
    // constant, rewriting `N'` to the identifier `nchar` — read language `nchar` + body `'p'`
    // and accepted. Dropping national lexing from the PG preset flips us to agreement: `N`
    // lexes as a bare word (the language name), `'p'` as the code block. Both the
    // language-only-N and the body-plus-language-N orders replay.
    b"DO LANGUAGE N'p'",
    b"DO $$x$$ LANGUAGE N'p'",
    // The `CREATE/ALTER EXTENSION ... VERSION` operand, a `NonReservedWord_or_Sconst` sibling
    // of the `DO` code block/language (nonreserved-word-or-sconst-literal-kind-siblings). The
    // version value discriminates string-literal *types* the same way: an `Sconst` (plain,
    // `E'…'`, dollar-quoted) or a bare non-reserved word / quoted identifier is accepted, while
    // a bit-string (`b'…'`/`x'…'`, a `bit`-typed `BCONST`/`XCONST`), a national (`N'…'`)
    // constant, or a reserved keyword is the syntax error libpg_query reports — each rejected
    // now, agreeing with pg. Both the `CREATE` and `ALTER … UPDATE TO` forms share the operand.
    b"CREATE EXTENSION foo VERSION bar",
    b"CREATE EXTENSION foo VERSION \"bar\"",
    b"CREATE EXTENSION foo VERSION 'bar'",
    b"CREATE EXTENSION foo VERSION E'bar'",
    b"CREATE EXTENSION foo VERSION $$bar$$",
    b"CREATE EXTENSION foo VERSION b'0'",
    b"CREATE EXTENSION foo VERSION x'ab'",
    b"CREATE EXTENSION foo VERSION N'x'",
    b"CREATE EXTENSION foo VERSION select",
    b"ALTER EXTENSION foo UPDATE TO 'bar'",
    b"ALTER EXTENSION foo UPDATE TO b'0'",
    // The `CREATE FUNCTION … LANGUAGE <name>` operand, the third
    // `NonReservedWord_or_Sconst` sibling (routine-language-name-word-or-sconst). The field
    // was a bare `Ident` that could not spell the string forms, so an `Sconst` there was an
    // under-acceptance; the operand is now the shared `LanguageName` (Word | String) union.
    // Both directions, pg_query-measured: the bare word and every `Sconst` spelling (plain,
    // `E'…'`, dollar-quoted) are accepted, while a bit-string (`b'…'`/`x'…'`) or national
    // (`N'…'`) constant is not an `Sconst` and stays the syntax error libpg_query reports.
    b"CREATE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE 'sql' AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE E'sql' AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE $$sql$$ AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE b'0' AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE x'ab' AS 'select 1'",
    b"CREATE FUNCTION f() RETURNS int LANGUAGE N'sql' AS 'select 1'",
    // The prefix-typed / temporal literal *value* position, another `Sconst`-only operand
    // (typed-literal-value-sconst-per-engine). PostgreSQL's `ConstTypename Sconst` /
    // `func_name '(' … ')' Sconst` / `ConstDatetime Sconst` productions take an `Sconst`, so
    // a bit-string value (`B'…'`/`X'…'`, a `bit`-typed `BCONST`/`XCONST`) is the syntax error
    // libpg_query reports across every head — generalized (`float8`), parameterized
    // (`char(1)`), func-name (`left(1)`), schema-qualified (`pg_catalog.float8`), and the
    // temporal `DATE`/`TIMESTAMP`/`TIME`/`INTERVAL` — while the `Sconst` spellings (plain,
    // `E'…'`, Unicode-escape `U&'…'`, dollar-quoted) fold to the typed literal. The
    // over-acceptance fixed here made each bit-string form reject, agreeing with pg; both
    // directions replay.
    b"SELECT float8 B'1'",
    b"SELECT float8 X'ab'",
    b"SELECT char(1) B'1'",
    b"SELECT left(1) X'ab'",
    b"SELECT pg_catalog.float8 B'1'",
    b"SELECT DATE X'ab'",
    b"SELECT TIMESTAMP B'1'",
    b"SELECT TIME X'ab'",
    b"SELECT INTERVAL X'ab'",
    b"SELECT float8 'x'",
    b"SELECT float8 U&'1'",
    b"SELECT float8 $$1$$",
    b"SELECT DATE '1998-01-01'",
    // Two statements abutting with no `;` separator between them
    // (pg-do-statement-separator-divergence). `Do''Do''` is the minimized fuzz reproducer:
    // a `DO ''` empty-body block immediately followed by another, which libpg_query rejects
    // because the top-level statement list is `;`-delimited. The over-acceptance was NOT in
    // the DO arm — it spanned every statement kind that can cleanly stop mid-stream
    // (`VALUES (1) VALUES (2)`, `TABLE t TABLE t`), masked for `SELECT 1 SELECT 2` because
    // the reserved `SELECT` cannot be a projection alias. `parse_next_statement` requires a
    // `;` or end of input after a statement, so all of these reject while the `;`-separated
    // form is accepted (`DO ''; DO ''`), pinning that only the missing separator is rejected.
    b"Do''Do''",
    b"DO '' DO ''",
    b"VALUES (1) VALUES (2)",
    b"TABLE t TABLE t",
    b"DO ''; DO ''",
    // A bare single-glyph (and multi-glyph dedicated) operator token in prefix position,
    // the last of the three residual under-acceptance classes the fuzz-mute-repoint census
    // surfaced (pg-bare-prefix-operator-glyphs), now closed. PostgreSQL admits any `Op`-class
    // token in prefix position (`qual_Op a_expr`), which the general custom-operator surface
    // (pg-operator-surface-regex-geometric-network) already covered for the `Custom` residue
    // but not for the *dedicated* built-in operator tokens whose primary meaning is infix.
    // Both directions of the surface. The accepted prefix spellings — the single-glyph
    // dedicated-infix tokens the census named plus `|`, and the multi-glyph `jsonb` /
    // containment / arrow / shift / concat tokens the census tested only in infix position:
    b"SELECT # 3",
    b"SELECT & 3",
    b"SELECT ? 3",
    b"SELECT | 3",
    b"SELECT #> 3",
    b"SELECT ?| 3",
    b"SELECT @> 3",
    b"SELECT -> 3",
    b"SELECT || 3",
    b"SELECT << 3",
    // The infix partition is undisturbed: `#`/`&`/`|` stay bitwise infix, `?` stays a
    // `jsonb` existence operator, given a left operand.
    b"SELECT a # b",
    b"SELECT 1 & 2",
    b"SELECT 1 | 2",
    // The reject boundary PostgreSQL enforces: the special single-char / comparison grammar
    // tokens are NOT `Op`, so they never open a primary (`=`/`^`/`%` reject in prefix), and a
    // bare `?` with no operand rejects (it is not the anonymous placeholder in PostgreSQL).
    b"SELECT = 3",
    b"SELECT ^ 3",
    b"SELECT %",
    b"SELECT ?",
    // A line comment embedding a raw NUL byte (`--`, NUL, `-`): the minimized reproducer
    // for the raw-byte differential crash (fuzz-pg-differential-crash-2b8d66f9). libpg_query
    // rejects any interior NUL — a query reaches it as a NUL-terminated C string, so its
    // `CString::new` fails before parsing — while our comment scanner used to consume bytes
    // to end-of-line without inspecting them and accept it, an over-acceptance. Both reject
    // now: the tokenizer rejects a NUL inside a comment (`NulByteInComment`), completing the
    // per-lexeme NUL gate that already covered string literals and quoted identifiers.
    b"--\x00-",
    // A bare (AS-less) output alias after a *qualified* wildcard `t.*`
    // (parse-qualified-wildcard-bare-alias). The minimized fuzz reproducer is
    // `SELECT hEE.*LC;` — `hEE.*` then the bare label `LC`. PostgreSQL reads `t.*` as an
    // ordinary columnref, so it takes the standard `[AS] label` projection alias; we used
    // to stop the qualified-wildcard item before the trailing word and reject it, an
    // under-acceptance. Now gated on `SelectSyntax::qualified_wildcard_alias` (on for
    // PostgreSQL/DuckDB, engine-measured), so both accept. Both directions of the surface:
    // the accepted spellings (the reproducer plus the canonical bare/`AS` forms) and the
    // reject boundary PostgreSQL keeps — a bare `*` is the non-aliasable `target_el: '*'`
    // production, so `SELECT * a` rejects on both sides (the wildcard-alias asymmetry).
    b"SELECT hEE.*LC;",
    b"SELECT t.* a FROM t",
    b"SELECT t.* AS a FROM t",
    b"SELECT * a FROM t",
    // A `--` line comment terminated by a bare carriage return (`--!` + three `\r` + `p`):
    // the minimized reproducer for the next comment-scanner divergence the fuzz proof run
    // found (tokenizer-line-comment-terminator-set). PostgreSQL's flex scanner ends a `--`
    // comment at `\r` as well as `\n` (`non_newline [^\n\r]`), so the comment is only `--!`
    // and the trailing `p` is a bare token that fails to parse -> REJECT; our scanner used to
    // consume to `\n`/EOF only, swallowing `\r\r\rp` as trivia and accepting (zero statements)
    // -> the accept/reject over-acceptance. The Postgres/DuckDb presets now carry
    // `CommentSyntax::line_comment_ends_at_carriage_return`, so `\r` ends the comment and both
    // parsers reject. Measured dialect data, not an unconditional change: SQLite and MySQL end
    // a line comment at `\n` alone, reading a `\r` as comment content (they keep accepting).
    b"--!\r\r\rp",
];

/// Feed raw bytes to the public dialect parsers and assert they never panic.
///
/// Invalid UTF-8 and oversized buffers are not SQL inputs, so the target drops
/// them before reaching the parser. Parse errors are successful outcomes here:
/// this target exists to turn unchecked unwraps and span bugs into test failures.
/// [`MySql`] and [`Lenient`] carry their own lexical surface — backtick
/// identifiers, `#` comments, and backslash string escapes for MySQL; the
/// permissive multi-quote-style union for Lenient — so panic-freedom on [`Ansi`]/
/// [`Postgres`] alone does not cover them (ADR-0012/0014). [`DuckDb`] shares
/// PostgreSQL's lexical surface but not its *grammar* — the collection literals
/// (`[…]`/`{…}`/`MAP {…}`) parse only there — so it is fuzzed too. [`Sqlite`] carries
/// its own distinctive lexical + grammar surface (backtick/`[bracket]` quotes, `0x`
/// integers, the `?`/`:`/`@`/`$` placeholders, the `LIMIT o, c` comma form, and the
/// flag-gated misfeature family — see `properties::dialect_features`), none of it
/// reachable through the other four presets, so it is fuzzed for panic-freedom too
/// (oracle-parity-sqlite).
///
/// Every successful parse is additionally pushed through `assert_render_no_panic`,
/// so a render panic reachable only from a *parsed* (not `arbitrary`-generated)
/// shape is caught here too — the `roundtrip`/`differential` targets only ever
/// render trees the structured generator built.
pub fn parse_no_panic(input: &[u8]) {
    if input.len() > MAX_PARSE_INPUT_BYTES {
        return;
    }
    let Ok(sql) = std::str::from_utf8(input) else {
        return;
    };

    if let Ok(parsed) = parse_with(sql, Ansi) {
        assert_render_no_panic(&parsed);
    }
    if let Ok(parsed) = parse_with(sql, Postgres) {
        assert_render_no_panic(&parsed);
    }
    if let Ok(parsed) = parse_with(sql, MySql) {
        assert_render_no_panic(&parsed);
    }
    if let Ok(parsed) = parse_with(sql, Lenient) {
        assert_render_no_panic(&parsed);
    }
    if let Ok(parsed) = parse_with(sql, DuckDb) {
        assert_render_no_panic(&parsed);
    }
    if let Ok(parsed) = parse_with(sql, Sqlite) {
        assert_render_no_panic(&parsed);
    }
}

/// Render every statement of a successfully-parsed fuzz tree in all three
/// [`RenderMode`]s and discard the output, asserting only that rendering never
/// panics.
///
/// Cheap by construction: it runs only after a successful parse, and most fuzz
/// inputs fail to parse at all, so the common case pays nothing extra.
fn assert_render_no_panic(parsed: &Parsed) {
    for mode in [
        RenderMode::Canonical,
        RenderMode::Parenthesized,
        RenderMode::Redacted,
    ] {
        let _ = crate::render_statements(parsed, mode);
    }
}

/// Feed arbitrary bytes into the structured round-trip target.
///
/// Returns `true` when the bytes produced a complete structured input and the
/// oracle ran. Too-short arbitrary buffers are ignored, matching fuzz target
/// practice where not every byte string needs to be a valid structured value.
pub fn roundtrip_arbitrary_input(input: &[u8]) -> bool {
    if input.len() > MAX_ROUNDTRIP_INPUT_BYTES {
        return false;
    }

    let mut unstructured = Unstructured::new(input);
    let Ok(fuzz) = FuzzStatement::arbitrary(&mut unstructured) else {
        return false;
    };
    roundtrip_statement(&fuzz.into_statement());
    true
}

/// Render one generated statement, reparse it, and compare ASTs with shared symbols.
///
/// The generator can emit a few shapes the parser deliberately rejects to match
/// PostgreSQL (see `statement_outside_roundtrip_subset`). Those cannot round-trip, so
/// they are skipped here — the PostgreSQL accept/reject differential
/// ([`differential_statement`]) is what exercises them. Any *other* reparse failure is a
/// render/parse bug and still panics.
///
/// The reparsed tree is real source-backed output — unlike `statement`, whose spans
/// are all [`Span::SYNTHETIC`](squonk_ast::Span::SYNTHETIC) — so it must also hold
/// the whole-tree span invariants `spans::assert_parsed_span_invariants` checks over
/// the fixed/vendored corpora (ADR-0002): this wires the fuzzer's generated-and-
/// rendered inputs into that same regression guard, continuously.
pub fn roundtrip_statement(statement: &Statement<NoExt>) {
    let rendered = render_generated(statement, RenderMode::Parenthesized);
    let reparsed = match parse_with(&rendered, Ansi) {
        Ok(reparsed) => reparsed,
        Err(err) => {
            assert!(
                statement_outside_roundtrip_subset(statement),
                "rendered fuzz SQL did not parse: {rendered:?}: {err:?}",
            );
            return;
        }
    };

    let [reparsed_statement] = reparsed.statements() else {
        panic!("rendered fuzz SQL should parse to one statement: {rendered:?}");
    };
    spans::assert_parsed_span_invariants(&reparsed);

    let comparison = shared_interner::compare_statement_with_shared_symbols(
        statement,
        &GENERATED_RESOLVER,
        reparsed_statement,
        reparsed.resolver(),
    );
    if !comparison.structurally_equal() {
        let left_normalized = normalize_statement(statement, &GENERATED_RESOLVER);
        let right_normalized = normalize_statement(reparsed_statement, reparsed.resolver());
        panic!(
            "{}",
            comparison.failure_message(
                "fuzz generated round-trip structural mismatch",
                &[("rendered SQL", &rendered)],
                Some((&left_normalized, &right_normalized)),
            ),
        );
    }
}

/// Whether `statement` renders to SQL the parser deliberately rejects to match
/// PostgreSQL, placing it outside the render round-trip oracle's accepted subset.
///
/// Two shapes qualify, both PostgreSQL syntax errors the parser now rejects: a target
/// column list paired with a `DEFAULT VALUES` source, and `MATCH PARTIAL` on a
/// foreign-key reference. The accept/reject differential ([`differential_statement`])
/// exercises them; [`roundtrip_statement`] skips them because they cannot round-trip.
fn statement_outside_roundtrip_subset(statement: &Statement<NoExt>) -> bool {
    let columns_on_default_values = matches!(
        statement,
        Statement::Insert { insert, .. }
            if !insert.target.columns.is_empty()
                && matches!(insert.source, InsertSource::DefaultValues { .. })
    );
    columns_on_default_values || statement_has_partial_match(statement)
}

/// Whether any foreign-key reference anywhere in `statement` uses `MATCH PARTIAL`.
fn statement_has_partial_match(statement: &Statement<NoExt>) -> bool {
    use squonk_ast::generated::visit::Visit;

    struct PartialMatchFinder {
        found: bool,
    }
    impl<'ast> Visit<'ast, NoExt> for PartialMatchFinder {
        fn visit_foreign_key_match(&mut self, node: &'ast ForeignKeyMatch) {
            self.found |= matches!(node, ForeignKeyMatch::Partial);
        }
    }

    let mut finder = PartialMatchFinder { found: false };
    finder.visit_statement(statement);
    finder.found
}

/// Feed arbitrary bytes into the PostgreSQL differential target.
///
/// Returns `true` when the bytes decoded to a complete structured statement and the
/// differential ran. Like [`roundtrip_arbitrary_input`], too-short buffers are
/// skipped rather than treated as failures.
pub fn differential_arbitrary_input(input: &[u8]) -> bool {
    if input.len() > MAX_ROUNDTRIP_INPUT_BYTES {
        return false;
    }
    let mut unstructured = Unstructured::new(input);
    let Ok(fuzz) = FuzzStatement::arbitrary(&mut unstructured) else {
        return false;
    };
    differential_statement(&fuzz.into_statement());
    true
}

/// Render one generated statement and compare it against the real PostgreSQL parser
/// (ADR-0015: fuzz + differential = one loop).
///
/// Accept/reject parity is checked over the *full* generated surface — both parsers
/// must agree on whether the rendered SQL parses. Structural parity is checked only
/// over the `structurally_pg_comparable` subset, which is limited to the generated
/// constructs that are also in the PostgreSQL structural corpus. There are no
/// known-divergent class exclusions left: the one that existed — a unary minus over a
/// numeric literal, which PostgreSQL folds into a signed constant while we keep the
/// `UnaryOp` (ADR-0006 amendment) — is now normalized by the structural mapping
/// (`prod-pg-map-expressions`, ADR-0015 representation-equivalence).
/// `fuzz_excluded_divergence_classes_still_diverge` keeps any future exclusion honest.
///
/// # Panics
///
/// Panics on any divergence that is not named in the divergence allowlist.
pub fn differential_statement(statement: &Statement<NoExt>) {
    let sql = render_generated(statement, RenderMode::Canonical);

    if let Some(detail) = pg::pg_accept_reject_divergence(&sql) {
        assert!(
            pg::pg_divergence_allowlisted(pg::PgDivergenceKind::AcceptReject, &sql),
            "fuzz differential accept/reject divergence for {sql:?}: {detail}",
        );
        // A disagreement on acceptance makes structural comparison moot.
        return;
    }

    if structurally_pg_comparable(statement) {
        if let Some(detail) = pg::pg_structural_divergence(&sql) {
            assert!(
                pg::pg_divergence_allowlisted(pg::PgDivergenceKind::Structural, &sql),
                "fuzz differential structural divergence for {sql:?}: {detail}",
            );
        }
    }
}

/// Feed *raw bytes* (not a generated AST) to the PostgreSQL accept/reject oracle and
/// assert the two parsers agree on whether the SQL parses (ADR-0015).
///
/// This is the over-acceptance hunter [`differential_statement`] cannot be: that one
/// only ever renders trees the structured generator built — legal by construction —
/// so it searches under-acceptance on well-formed shapes. This one decodes arbitrary
/// bytes (dropping invalid UTF-8 and oversized buffers exactly as [`parse_no_panic`]
/// does) and drives the *same* [`pg::pg_accept_reject_divergence`] oracle over the
/// full raw-input space, so the validator-correctness class — accepting SQL
/// PostgreSQL rejects, or rejecting SQL it accepts — finally has a generative search.
///
/// Only accept/reject parity is checked: with no generated tree there is no neutral
/// shape to compare, so the structural half of the differential does not apply here.
///
/// Every divergence fails unless named in [`pg::PG_DIVERGENCE_ALLOWLIST`], in either
/// direction — an over-acceptance (we accept, PostgreSQL rejects; the
/// validator-correctness class only this target can hunt, none known) and an
/// under-acceptance (PostgreSQL accepts, we reject; a coverage gap) alike.
///
/// The `known_deferred_underacceptance` mute this target once carried — it swallowed
/// *every* we-reject input containing one of the glyph bytes `@ # ? ~ ! |`, citing the
/// enumerated PostgreSQL operator tail (unary `@`/`~`, `#`, `?`, factorial `!`, the
/// `|/`-family roots) — is RETIRED: its ticket
/// (`pg-operator-tail-unary-at-jsonb-path-existence-range`) is done and the general
/// custom-operator surface exists (`pg-operator-surface-regex-geometric-network`), so
/// per this target's own contract a class whose ticket closes must re-arm the fuzzer.
/// A directed accept/reject census over the whole glyph space
/// (`fuzz-mute-repoint-custom-operators`) confirmed the operator *infix* surface (the
/// jsonb `?`/`?|`/`?&`/`@>`/`<@`/`#>`/`#>>`/`@?`/`@@`/`#-` operators, regex
/// `~`/`~*`/`!~`/`!~*`, geometric `?#`/`?|`/`?-|`/`?||`, containment/text-search, `!=`,
/// and the once-deferred `-|-` range op), the known unary prefixes
/// (`@`/`~`/`|/`/`||/`/`@@`/`!!`), and custom multi-glyph prefix runs all both-accept,
/// zero over-acceptances; a bare positional `?` and the removed postfix factorial
/// `5 !` both-*reject*, so no `?`-parameter or factorial divergence exists to mute.
///
/// The census surfaced three residual under-acceptance classes, none owned by an open
/// ticket, left deliberately un-muted so the fuzzer re-arms and surfaces each as a
/// fresh, ticketable finding rather than a silently-swallowed gap: (1) a bare
/// single-glyph prefix operator that also has a dedicated infix token (`#`, `&`, `?`) —
/// since closed by pg-bare-prefix-operator-glyphs, which routed every prefix-valid `Op`
/// token (the single-glyph `#`/`&`/`|`/`?` the census named plus, on re-probe, the
/// multi-glyph dedicated `jsonb`/containment/arrow/shift/concat tokens `#>`/`?|`/`@>`/`->`/
/// `||`/`<<`/… the census had only tested in *infix* position) through the prefix path
/// under `custom_operators`, so the prefix seeds below both-accept and the `=`/`^`/`%`
/// exceptions both-reject; (2) the CREATE / ALTER / DROP OPERATOR (and OPERATOR CLASS /
/// FAMILY) object-DDL family; and (3) the PL/pgSQL `DO` statement (since closed by
/// pg-do-statement-body — the `DO` seeds below both-accept/both-reject now that the
/// statement parses). Only (1) is the expression-tail surface this mute covered; (2) and
/// (3) are statement-level surfaces it caught only incidentally, via a glyph in the
/// operator name or the dollar-quoted body. A future mute over any of these keys on its
/// own predicate and cites its own ticket, not on
/// the glyph set.
///
/// # Panics
///
/// Panics on any accept/reject divergence not named in [`pg::PG_DIVERGENCE_ALLOWLIST`]
/// — the triage signal this target exists to produce.
pub fn pg_differential_raw_bytes(input: &[u8]) {
    if input.len() > MAX_PARSE_INPUT_BYTES {
        return;
    }
    let Ok(sql) = std::str::from_utf8(input) else {
        return;
    };

    if let Some(detail) = pg::pg_accept_reject_divergence(sql) {
        assert!(
            pg::pg_divergence_allowlisted(pg::PgDivergenceKind::AcceptReject, sql),
            "raw-byte fuzz accept/reject divergence for {sql:?}: {detail}",
        );
    }
}

/// Seed + replay corpus for the raw-byte SQLite differential target.
///
/// Raw SQL bytes fed straight to the SQLite parse-only accept/reject + segmentation
/// oracle ([`crate::m2::sqlite_raw_bytes_divergence`]), doubling as libFuzzer seeds so
/// the mutator starts from real SQLite shapes. Every entry must be
/// accept/reject-and-count agreeing today (`None`-or-allowlisted) so
/// `sqlite_differential_raw_bytes_replays_committed_inputs` stays green; a fuzz-found,
/// minimized divergence lands here alongside its triage (an
/// [`M2_DIVERGENCE_ALLOWLIST`](crate::m2::M2_DIVERGENCE_ALLOWLIST) entry naming a
/// ticket, or a parser fix).
///
/// Seeded from the SQLite-distinctive shapes the flag-aware generator and
/// [`PARSE_NO_PANIC_REPLAYS`] already exercise (backtick / `[bracket]` quotes, the
/// typeless CREATE TABLE, bare `IS`, the `0x` integer, the `LIMIT o, c` comma form)
/// plus the schema-independent both-accept/both-reject shapes from the curated M2
/// corpus, and fresh multi-statement seeds that drive the segmentation
/// (statement-count) half — the SQLite arm of the splitter hunt.
#[cfg(feature = "oracle-engines")]
pub const SQLITE_DIFFERENTIAL_RAW_BYTES_REPLAYS: &[&[u8]] = &[
    // Schema-independent both-accept singles (curated M2 corpus shapes).
    b"SELECT 1",
    b"SELECT 1 + 2 * 3",
    b"VALUES (1), (2), (3)",
    b"SELECT 'it''s'",
    b"SELECT 1 IN (1, 2, 3)",
    // SQLite-distinctive lexical + grammar surface (both accept under the fitted
    // Sqlite preset; the other presets reject most of these).
    b"SELECT 1 AS `back_ticked`",
    b"SELECT 1 AS [bracketed]",
    b"CREATE TABLE t (a, b)",
    b"SELECT 1 IS 1",
    b"SELECT 0x1F",
    b"SELECT 1 LIMIT 2, 3",
    // Reject boundary: syntactic errors both reject (a genuine parse failure, not a
    // name-resolution reject — those read as accepts, see the module docs).
    b"SELECT",
    b"SELCT 1",
    b"SELECT 1 +",
    b"SELECT FROM",
    // Segmentation: `;`-separated statements both accept with agreeing counts, and the
    // separator-less abutment both reject — the statement-splitter surface.
    b"SELECT 1; SELECT 2",
    b"SELECT 1; VALUES (2); SELECT 3",
    b"SELECT 1;",
    b"SELECT 1 SELECT 2",
    // Vertical-tab (`0x0b`) whitespace-run continuation (whitespace-vertical-tab-sqlite-duckdb):
    // SQLite folds a `\v` that rides an open whitespace run but rejects one that would
    // start a run. The fuzzer's minimized accept was `[0x20, 0x0b]` (space + `\v`, an empty
    // statement) — accepted before the fix by SQLite only; a lone `[0x0b]` and a `\v`
    // starting a run stay rejected by both, so they belong in the reject-parity set.
    &[0x20, 0x0b],
    b"SELECT 1 \x0b",
    b"SELECT \x0b1",
    &[0x0b],
    b"SELECT\x0b1",
    // Numeric literal abutting identifier chars is one TK_ILLEGAL token in SQLite
    // (sqlite-numeric-trailing-junk-over-acceptance): the `reject_trailing_junk` +
    // `underscore_separators` flip makes the fitted Sqlite preset reject them too. The
    // minimized soak finds plus the interaction surface (bad hex body, junk exponent,
    // dotted-ident, misplaced/leading `_`), each `sqlite=reject`:
    b"SELECT 1SETECT",
    b"SELECT .122ualCvT",
    b"SELECT 2ES",
    b"SELECT 0x1g",
    b"SELECT 1e5x",
    b"SELECT 1.a",
    b"SELECT 1_",
    // `0x_1F`: a leading-underscore radix body. PG admits it, SQLite does not — the
    // dedicated `radix_leading_underscore` axis (off for Sqlite) keeps this a reject.
    b"SELECT 0x_1F",
    // Accept-side regression guards: `_` separators (SQLite 3.46+) stay one number, so
    // the junk flip must not newly reject them.
    b"SELECT 1_000_000",
    b"SELECT 0x1_F",
    // sqlite-lexer-under-acceptance-bundle: five SQLite-accepts / we-rejected lexer
    // boundaries, each measured on the live rusqlite oracle. Fail-before was proven by
    // reverting the dialect data (`$` continuation, empty quoted idents, the comment flags,
    // the bare string alias, numbered `?N`), which re-diverges these inputs.
    // (1) `$` is an identifier-*continuation* byte (`dollar_in_identifiers`); a leading `$name`
    // stays the placeholder and a lone `$` is a stray byte both engines reject.
    b"SELECT L$C3",
    b"CREATE TABLE t$x (a)",
    b"SELECT 1 AS a$b",
    b"SELECT $abc",
    // (2) empty quoted identifier in every style (`empty_quoted_identifiers`).
    b"SELECT ``",
    b"SELECT \"\"",
    b"SELECT []",
    b"SELECT 1 AS ``",
    b"CREATE TABLE `` (a)",
    // (3) unterminated `/*` at EOF silently closed + non-nesting (the two `CommentSyntax`
    // flags). A bare `/*` at EOF stays the slash operator (reject on both).
    b"SELECT 1/* eof",
    b"\t\t/*\t\t",
    b"/* a /* b */",
    b"SELECT /* a /* b */ 1",
    b"SELECT 1 /*",
    b"/*",
    // (4) bare (`AS`-less) string alias, and a bare parameter abutting one
    // (`bare_alias_string_literals` — the minimized fuzz shape was a param + empty string).
    b"SELECT 1 'x'",
    b"SELECT@z_va''",
    b"SELECT @z_va 'x'",
    b"SELECT :z_va''",
    // (5) numbered `?NNN` parameters (`numbered_question`), the `?1abc` maximal-munch boundary,
    // and the 1..=32766 range edges (`?0`/`?32767` reject on both, `?32766` accepts).
    b"SELECT ?1",
    b"SELECT ?123",
    b"SELECT ?1abc",
    b"SELECT ?1 + ?2",
    b"SELECT ?32766",
    b"SELECT ?0",
    b"SELECT ?32767",
];

/// Seed + replay corpus for the raw-byte DuckDB differential target.
///
/// Raw SQL bytes fed straight to the DuckDB parse-only accept/reject + segmentation
/// oracle ([`crate::m2::duckdb_raw_bytes_divergence`]), doubling as libFuzzer seeds.
/// Every entry must keep `duckdb_differential_raw_bytes_replays_committed_inputs`
/// green — either accept/reject-and-count agreeing, or a triaged divergence the target
/// body pre-filters. A fuzz-found, minimized divergence lands here alongside its
/// triage; the two non-ASCII-swallow exemplars below are pinned by the
/// `duckdb_nonascii_swallow` pre-filter (owner
/// `duckdb-extract-nonascii-swallow-allowlist`), so removing that pre-filter re-panics
/// this replay, deterministically regression-pinning the disposition.
///
/// Seeded from the DuckDB-distinctive collection literals (`[…]` list / `{…}` struct /
/// `MAP {…}`) the `duckdb-collection-literals` work armed and [`PARSE_NO_PANIC_REPLAYS`]
/// already carries, the `//` integer-division and `==` operator spellings, the curated
/// schema-independent both-accept/both-reject shapes, and fresh multi-statement seeds
/// for the segmentation half.
#[cfg(feature = "oracle-engines")]
pub const DUCKDB_DIFFERENTIAL_RAW_BYTES_REPLAYS: &[&[u8]] = &[
    // Schema-independent both-accept singles (curated M2 corpus shapes).
    b"SELECT 1",
    b"SELECT 1 + 2 * 3",
    b"VALUES (1), (2), (3)",
    b"SELECT 'it''s'",
    // DuckDB-distinctive collection literals + operators (both accept under the fitted
    // DuckDb preset; every other preset rejects the collection grammar).
    b"SELECT [1, 2, 3]",
    b"SELECT {'a': 42}",
    b"SELECT [{'a': 42}, {'b': 84}]",
    b"SELECT MAP {[1, 2]: {'k': []}}",
    b"SELECT 5 // 2",
    b"SELECT 1 == 1",
    // Reject boundary: syntactic errors both reject, including the unterminated
    // collection-literal brackets (`extract_statements` is parse-only, so an
    // unresolved *name* would instead read as an accept).
    b"SELECT",
    b"SELCT 1",
    b"SELECT [1, 2",
    b"SELECT {'a':",
    // Segmentation: `;`-separated statements both accept with agreeing counts; the
    // separator-less abutment both reject.
    b"SELECT 1; SELECT 2",
    b"VALUES (1); VALUES (2)",
    b"SELECT 1;",
    b"SELECT 1 SELECT 2",
    // Vertical-tab (`0x0b`) statement-boundary trim (whitespace-vertical-tab-sqlite-duckdb):
    // DuckDB folds a `\v` at a `;`-segment's leading/trailing edge but rejects one interior
    // to a statement. The fuzzer's minimized accepts were a lone `[0x0b]` (empty) and
    // `SELECT 1;\x0b` (a trailing `\v` after a real statement) — both accepted before the
    // fix by DuckDB only. Interior `\v` stays a reject-parity case on both engines.
    &[0x0b],
    b"SELECT 1;\x0b",
    b"\x0bSELECT 1",
    b"SELECT 1\x0b",
    b"SELECT\x0b1",
    // Non-ASCII-swallow exemplars (duckdb-extract-nonascii-swallow-allowlist): DuckDB's
    // extractor drops bare unrecognized non-ASCII, yielding zero statements with no
    // error (indistinguishable from empty), while our parser rejects. Both minimized by
    // the fuzz-differential-sqlite-duckdb maiden soak; `0xc7 0xa7` = 'ǧ', `0xd8 0xb4` =
    // 'ش'. Kept green by the target body's `duckdb_nonascii_swallow` pre-filter.
    &[0xc7, 0xa7],
    &[0xd8, 0xb4],
    // PG-style generalized operator spellings (duckdb-pg-operator-spelling-under-acceptance):
    // DuckDB inherits PostgreSQL's maximal-munch `Op`-class lexer and *parse*-accepts these
    // runs (bind-rejecting the ones with no backing function), while before the fix our DuckDb
    // preset rejected them at parse — the under-acceptance the soak surfaced. Both-accept after
    // arming `custom_operators` for DuckDb. The backtick-led `` `= `` and long `&&&&&@` run are
    // the minimized soak artifacts (`SELECT T,p&&&&&@Le`, `SELECT p`=`).
    b"SELECT T,p&&&&&@Le",
    b"SELECT 1 <<| 2",
    b"SELECT 1 <-> 2",
    b"SELECT 1 ~ 2",
    b"SELECT p`=q",
    b"SELECT @ 1",
    // Reject-parity for the charset boundary: DuckDB drops `#`/`?` from the `Op` class (its
    // `#1` positional-column and `?` parameter sigils), so a run stops at either — `1 @#@ 2`
    // is `@` then a stray `#`, and a lone `1 # 2` / `1 ? 2` reject on both engines. (These are
    // NOT PostgreSQL rejects — pg keeps `#`/`?` in its runs — but our DuckDb preset drops them
    // via `positional_column` / `anonymous_question`, matching DuckDB.)
    b"SELECT 1 @#@ 2",
    b"SELECT 1 # 2",
    b"SELECT 1 ? 2",
    // DuckDB postfix symbolic operators (duckdb-postfix-operator-dimension): DuckDB keeps the
    // generalized postfix reading PostgreSQL removed in 14 — a trailing `Op`-class operator with
    // no operand *parse*-accepts (`10!` binds via `!__postfix`; `1 ~`/`1 <->` bind-reject the
    // missing `~__postfix`/`<->__postfix`), while before the fix our DuckDb preset rejected the
    // trailing operator at parse. Both-accept after arming `postfix_operators`. Covers the
    // `Custom` residue (`<->`), the lone `!`/`~`, and the dedicated `&`/`<@`, plus the
    // precedence (`1 ! < 2` is `(1!) < 2`) and cast interaction (`1! :: INT`).
    b"SELECT 10!",
    b"SELECT 1 !",
    b"SELECT 1 ~",
    b"SELECT 1 <->",
    b"SELECT 1 &",
    b"SELECT 1 <@",
    b"SELECT 1 ! < 2",
    b"SELECT 1! :: INT",
    // Reject-parity for the postfix charset boundary: the JSON arrows `->`/`->>` are NOT
    // postfix-eligible (DuckDB syntax-errors a trailing `->`), so both engines reject.
    b"SELECT 1 ->",
];

/// Feed *raw bytes* to the SQLite parse-only accept/reject + segmentation oracle and
/// assert our fitted [`Sqlite`] preset and real SQLite agree (the SQLite analogue of
/// [`pg_differential_raw_bytes`]).
///
/// Never executes: the oracle counts statements with `sqlite3_prepare_v2` + `pzTail`,
/// finalizing each un-stepped ([`crate::sqlite_ffi`]); a SQLite name-resolution reject
/// reads as a parse accept so the differential compares *syntactic* acceptance, on the
/// same footing as our parse-only parser (see [`crate::m2::sqlite_raw_bytes_divergence`]).
/// The in-memory connection is a schema-less thread-local reused across inputs.
///
/// Behind `oracle-engines`: the default build links no SQLite. The libFuzzer target and
/// the stable Bolero gate share this body (one harness, two engines).
///
/// # Panics
///
/// Panics on any accept/reject or segmentation divergence not named in
/// [`M2_DIVERGENCE_ALLOWLIST`](crate::m2::M2_DIVERGENCE_ALLOWLIST) — the triage signal.
#[cfg(feature = "oracle-engines")]
pub fn sqlite_differential_raw_bytes(input: &[u8]) {
    use rusqlite::Connection as SqliteConnection;
    thread_local! {
        static CONN: SqliteConnection = SqliteConnection::open_in_memory()
            .expect("in-memory SQLite for the raw-byte differential");
    }
    if input.len() > MAX_PARSE_INPUT_BYTES {
        return;
    }
    let Ok(sql) = std::str::from_utf8(input) else {
        return;
    };
    CONN.with(|conn| {
        if let Some(detail) = crate::m2::sqlite_raw_bytes_divergence(conn, sql) {
            assert!(
                crate::m2::m2_divergence_allowlisted("sqlite", sql),
                "raw-byte fuzz accept/reject divergence (sqlite) for {sql:?}: {detail}",
            );
        }
    });
}

/// Feed *raw bytes* to the DuckDB parse-only accept/reject + segmentation oracle and
/// assert our fitted [`DuckDb`] preset and real DuckDB agree (the DuckDB analogue of
/// [`pg_differential_raw_bytes`]).
///
/// Never executes: the oracle counts statements with `duckdb_extract_statements` — the
/// parser, not the preparer — so it never trips DuckDB's "prepare executes all but the
/// last statement" hazard, and extraction is parse-only so an unresolved name still
/// parses (see [`crate::m2::duckdb_raw_bytes_divergence`]). The in-memory connection is
/// a thread-local reused across inputs; if `libduckdb` is unreachable at run time the
/// target skips (never a false crash).
///
/// Behind `oracle-engines`: the default build links no `libduckdb`. The libFuzzer
/// target and the stable Bolero gate share this body.
///
/// # Panics
///
/// Panics on any accept/reject or segmentation divergence not named in
/// [`M2_DIVERGENCE_ALLOWLIST`](crate::m2::M2_DIVERGENCE_ALLOWLIST).
#[cfg(feature = "oracle-engines")]
pub fn duckdb_differential_raw_bytes(input: &[u8]) {
    use crate::duckdb_ffi::Connection as DuckDbConnection;
    thread_local! {
        // `Option`: an unreachable engine is a skip (respecting the oracle contract),
        // never a false crash. In the `oracle-engines` build `libduckdb` is linked, so
        // this is `Some` in practice.
        //
        // `ManuallyDrop`: `duckdb_close` must NEVER run from this thread-local's
        // destructor. libduckdb 1.5.4's `~BlockAllocator` (via `~DatabaseInstance` →
        // `~DBConfig`) calls `GetBlockAllocatorThreadLocalState`, which re-initializes
        // DuckDB's own C++ `thread_local` — inside pthread TSD cleanup the C++ TLS
        // finalizers have already run, so that re-init touches freed TLS memory
        // (measured: intermittent EXC_BAD_ACCESS/SIGSEGV in
        // `BlockAllocatorThreadLocalState::Initialize` at test-thread exit, ticket
        // `duckdb-extract-nonascii-swallow-allowlist`). The per-thread connection is
        // deliberately leaked instead; the OS reclaims it at process exit. Dropping a
        // `Connection` on a live thread (mid-test) remains safe — only the
        // TLS-destructor context is hazardous.
        static CONN: std::mem::ManuallyDrop<Option<DuckDbConnection>> =
            std::mem::ManuallyDrop::new(DuckDbConnection::open_in_memory().ok());
    }
    if input.len() > MAX_PARSE_INPUT_BYTES {
        return;
    }
    let Ok(sql) = std::str::from_utf8(input) else {
        return;
    };
    CONN.with(|conn| {
        let Some(conn) = conn.as_ref() else {
            return;
        };
        if let Some(detail) = crate::m2::duckdb_raw_bytes_divergence(conn, sql) {
            // Pre-filter the DuckDB non-ASCII-swallow class: `duckdb_extract_statements`
            // silently drops unrecognized non-ASCII outside strings/identifiers/comments
            // (reading a bare run as empty, `duckdb=accept, squonk=reject`), an engine
            // API quirk, not a parser gap. Owner-named and proven per input, so real
            // unicode divergences — non-ASCII inside strings/identifiers, the `U&` escapes
            // — still surface (see `crate::m2::duckdb_nonascii_swallow`, owner
            // `duckdb-extract-nonascii-swallow-allowlist`).
            if crate::m2::duckdb_nonascii_swallow(conn, sql) {
                return;
            }
            assert!(
                crate::m2::m2_divergence_allowlisted("duckdb", sql),
                "raw-byte fuzz accept/reject divergence (duckdb) for {sql:?}: {detail}",
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_panic_replays_committed_inputs() {
        for input in PARSE_NO_PANIC_REPLAYS {
            parse_no_panic(input);
        }
    }

    /// Deterministic per-dialect anchors (mirrors [`fuzz_new_family_anchors_round_trip`]'s
    /// "anchor, don't just hope a random draw hits it" philosophy): each string is
    /// chosen to actually succeed under its dialect, so this proves `parse_no_panic`'s
    /// four-dialect parse and its render-on-success path both run — not merely compile
    /// — rather than relying on a random fuzz/Bolero draw to happen to land on
    /// parseable, dialect-specific SQL.
    #[test]
    fn parse_no_panic_exercises_every_dialect_and_the_render_path() {
        // Parses (and therefore renders) under all four dialects.
        parse_no_panic(b"SELECT a FROM t WHERE a = 1");
        // MySQL's own lexical surface: a backtick identifier and a `#` line comment
        // (the exact case `lenient.rs`'s own tests pin) — accepted by MySql and by
        // Lenient's permissive union, rejected by Ansi/Postgres.
        parse_no_panic(b"SELECT `a` # c\nFROM t");
        // Lenient-only: all three identifier quote styles at once (another case
        // `lenient.rs`'s own tests pin), accepted by no other dialect.
        parse_no_panic(br#"SELECT "a", `b`, [c] FROM t"#);
        // SQLite-only grammar: a typeless-column CREATE TABLE plus the bare-IS general
        // equality — accepted by the Sqlite preset, rejected by the other four — so the
        // Sqlite parse and its render-on-success path both run, not merely compile.
        parse_no_panic(b"CREATE TABLE t (a, b)");
        parse_no_panic(b"SELECT 1 IS 1");
    }

    #[test]
    fn roundtrip_replays_committed_inputs() {
        for input in ROUNDTRIP_REPLAYS {
            assert!(
                roundtrip_arbitrary_input(input),
                "round-trip replay did not decode as arbitrary input: {input:?}",
            );
        }
    }

    #[test]
    fn bolero_parse_no_panic_runs_under_cargo_test() {
        bolero::check!()
            .with_iterations(64)
            .with_max_len(MAX_PARSE_INPUT_BYTES)
            .for_each(|input: &[u8]| parse_no_panic(input));
    }

    #[test]
    fn bolero_roundtrip_runs_under_cargo_test() {
        // 256 (over the original 64): the generated statement surface now spans the
        // DDL/DML families, windows, and the literal forms, so a wider sweep keeps the
        // stable check a meaningful regression guard over the grown grammar.
        bolero::check!()
            .with_iterations(256)
            .with_max_len(MAX_ROUNDTRIP_INPUT_BYTES)
            .with_arbitrary::<FuzzStatement>()
            .for_each(|input| roundtrip_statement(&input.into_statement()));
    }

    #[test]
    fn differential_replays_committed_inputs() {
        for input in DIFFERENTIAL_REPLAYS {
            assert!(
                differential_arbitrary_input(input),
                "differential replay did not decode as arbitrary input: {input:?}",
            );
        }
    }

    #[test]
    fn bolero_differential_runs_under_cargo_test() {
        // 256 (over the original 64): every generated tree must also satisfy PostgreSQL
        // accept/reject parity now that the DDL/DML families flow through here, so the
        // wider sweep guards the larger surface (validated divergence-free at 200k).
        bolero::check!()
            .with_iterations(256)
            .with_max_len(MAX_ROUNDTRIP_INPUT_BYTES)
            .with_arbitrary::<FuzzStatement>()
            .for_each(|input| differential_statement(&input.into_statement()));
    }

    #[test]
    fn pg_differential_raw_bytes_replays_committed_inputs() {
        // Every committed seed must be accept/reject-agreeing under the current
        // allowlist; a diverging entry fails inside the body, pinning the regression.
        for input in PG_DIFFERENTIAL_RAW_BYTES_REPLAYS {
            pg_differential_raw_bytes(input);
        }
    }

    #[test]
    fn vertical_tab_parses_as_empty_and_agrees_with_postgres() {
        // Regression for pg-whitespace-class-control-bytes. PostgreSQL folds the vertical
        // tab (`0x0b`, `\v`) as ignorable whitespace — its flex `space` set is
        // `[ \t\n\r\f\v]` — so a lone `0x0b` is an empty statement it accepts. The
        // PostgreSQL preset now carries `0x0b` in its whitespace class, so our tokenizer
        // folds it too and the raw-byte differential (whose control-byte carve-out this
        // ticket deleted, re-arming this surface) sees no divergence.
        const VT: &str = "\u{0b}";
        assert!(
            parse_with(VT, Postgres).is_ok(),
            "our PostgreSQL tokenizer should fold a lone vertical tab as an empty statement",
        );
        assert!(
            pg::pg_accept_reject_divergence(VT).is_none(),
            "our PostgreSQL parser and libpg_query should agree on a lone vertical tab",
        );
        // Drive the exact re-armed fuzz target over the vertical tab in the positions the
        // raw mutator reaches it — alone, repeated, mixed with the rest of pg's `space`
        // set, and as an inter-token separator. Each panics if we and pg disagree.
        for input in [
            b"\x0b".as_slice(),
            b"\x0b\x0b\x0b",
            b" \x0b\t\r\n\x0c",
            b"SELECT\x0b1",
            b"SELECT 1\x0b",
        ] {
            pg_differential_raw_bytes(input);
        }
    }

    #[test]
    fn bolero_pg_differential_raw_bytes_runs_under_cargo_test() {
        // Raw bytes (like `parse_no_panic`), not a generated AST, so this shares that
        // target's modest budget: a random buffer almost never reaches parseable SQL,
        // and each iteration additionally pays the live pg_query oracle. The committed
        // seeds and the nightly libFuzzer soak carry the real over-acceptance search;
        // this only keeps a stable pulse on the body under `cargo nextest`.
        bolero::check!()
            .with_iterations(64)
            .with_max_len(MAX_PARSE_INPUT_BYTES)
            .for_each(|input: &[u8]| pg_differential_raw_bytes(input));
    }

    #[cfg(feature = "oracle-engines")]
    #[test]
    fn sqlite_differential_raw_bytes_replays_committed_inputs() {
        // Every committed SQLite seed must be accept/reject-and-count agreeing under
        // the current allowlist; a diverging entry fails inside the body, pinning the
        // regression (mirrors the pg replay test).
        for input in SQLITE_DIFFERENTIAL_RAW_BYTES_REPLAYS {
            sqlite_differential_raw_bytes(input);
        }
    }

    #[cfg(feature = "oracle-engines")]
    #[test]
    fn duckdb_differential_raw_bytes_replays_committed_inputs() {
        // As the SQLite replay test. Skips cleanly if `libduckdb` is unreachable (the
        // body's thread-local connection is `None`), never a false failure.
        for input in DUCKDB_DIFFERENTIAL_RAW_BYTES_REPLAYS {
            duckdb_differential_raw_bytes(input);
        }
    }

    /// The DuckDB non-ASCII-swallow class pre-filter (owner
    /// `duckdb-extract-nonascii-swallow-allowlist`): the class is suppressed while every
    /// other non-ASCII divergence stays visible. Skips cleanly if `libduckdb` is
    /// unreachable, like the replay tests.
    #[cfg(feature = "oracle-engines")]
    #[test]
    fn duckdb_nonascii_swallow_class_and_kept_unicode_coverage() {
        use crate::duckdb_ffi::Connection;
        let Some(conn) = Connection::open_in_memory().ok() else {
            return;
        };

        // Suppressed swallow class: bare non-ASCII across Unicode blocks, and non-ASCII
        // dropped between/after `;`-terminated statements (the two committed exemplars
        // 'ǧ'/'ش' among them). DuckDB accepts, our parser rejects — the engine-API quirk.
        for sql in [
            "\u{01e7}",                   // ǧ (exemplar 0xc7 0xa7)
            "\u{0634}",                   // ش (exemplar 0xd8 0xb4)
            "\u{00e9}",                   // é (Latin-1)
            "\u{4e2d}",                   // 中 (CJK)
            "\u{1f600}",                  // 😀 (emoji, 4-byte)
            "\u{0634}\u{0634}",           // multiple swallowed
            "SELECT 1;\u{0634}",          // trailing after a statement
            "SELECT 1;SELECT 2;\u{0634}", // trailing after two statements
            "\u{0634};SELECT 1",          // leading before a statement
            "SELECT 1;\u{0634};",         // between semicolons
        ] {
            assert!(
                crate::m2::duckdb_nonascii_swallow(&conn, sql),
                "expected the non-ASCII-swallow class: {sql:?}",
            );
        }

        // Kept coverage — the instrument must NOT swallow real unicode handling: non-ASCII
        // inside string literals, quoted identifiers, and comments (both engines accept),
        // and the `U&` unicode escapes (DuckDB rejects them unimplemented). Each stays a
        // visible (or non-) divergence rather than being pre-filtered.
        for sql in [
            "SELECT '\u{0634}'",         // string literal
            "SELECT '\u{4e2d}\u{6587}'", // multi-byte string literal
            "SELECT '\u{1f600}'",        // emoji string literal
            "SELECT \"\u{0634}\"",       // quoted identifier
            "SELECT 1 AS \"\u{4e2d}\"",  // quoted alias
            "SELECT 1 -- \u{0634}",      // line comment
            "SELECT 1 /* \u{0634} */",   // block comment
            "SELECT U&'\u{0634}'",       // U& string with raw non-ASCII (DuckDB reject)
        ] {
            assert!(
                !crate::m2::duckdb_nonascii_swallow(&conn, sql),
                "expected kept unicode coverage (not swallowed): {sql:?}",
            );
        }

        // The full target body exercises the kept string/identifier path as a both-accept
        // non-divergence without panicking — proof the pre-filter left that coverage live.
        duckdb_differential_raw_bytes("SELECT '\u{0634}'".as_bytes());
        duckdb_differential_raw_bytes("SELECT \"\u{4e2d}\"".as_bytes());
    }

    #[cfg(feature = "oracle-engines")]
    #[test]
    fn bolero_sqlite_differential_raw_bytes_runs_under_cargo_test() {
        // Raw bytes (like `pg_differential_raw_bytes`), sharing that target's modest
        // budget: a random buffer almost never reaches parseable SQL, and each
        // iteration additionally pays the live SQLite prepare-and-count oracle. The
        // committed seeds and the nightly libFuzzer soak carry the real search; this
        // keeps a stable pulse on the body under `cargo nextest --features oracle-engines`.
        bolero::check!()
            .with_iterations(64)
            .with_max_len(MAX_PARSE_INPUT_BYTES)
            .for_each(|input: &[u8]| sqlite_differential_raw_bytes(input));
    }

    #[cfg(feature = "oracle-engines")]
    #[test]
    fn bolero_duckdb_differential_raw_bytes_runs_under_cargo_test() {
        // As the SQLite gate; the body skips when `libduckdb` is unreachable.
        bolero::check!()
            .with_iterations(64)
            .with_max_len(MAX_PARSE_INPUT_BYTES)
            .for_each(|input: &[u8]| duckdb_differential_raw_bytes(input));
    }

    #[test]
    fn fuzz_excluded_divergence_classes_still_diverge() {
        // The structural differential excludes one known-divergent class. Guard
        // that it (a) is excluded by the comparable predicate, (b) still diverges
        // from PostgreSQL, and (c) carries a provenance label — so a silent fix flips
        // this test and forces re-including the class in the loop.
        let parse_one = |sql: &str| {
            parse_with(sql, Ansi)
                .expect("known-divergence case parses")
                .statements()[0]
                .clone()
        };

        // Known-divergent classes excluded from the structural differential, each
        // keyed to a stable provenance label. Currently empty: the only
        // class — a unary minus over a numeric literal — was resolved by the mapping
        // normalization in prod-pg-map-expressions (ADR-0015). Kept as a slice so
        // re-adding a class needs no restructuring.
        const EXCLUDED_DIVERGENCE_CLASSES: &[(&str, &str)] = &[];
        for &(sql, ticket) in EXCLUDED_DIVERGENCE_CLASSES {
            assert!(
                !structurally_pg_comparable(&parse_one(sql)),
                "{sql:?} must be excluded from the structural differential",
            );
            assert!(
                pg::pg_structural_divergence(sql).is_some(),
                "{sql:?} no longer diverges from PostgreSQL; re-include it in the \
                 structural differential and drop the exclusion",
            );
            assert!(
                !ticket.trim().is_empty(),
                "{sql:?} needs a provenance label"
            );
        }

        // The formerly-excluded signed numeric literal class is now normalized, so it
        // is structurally comparable with no remaining divergence
        // (prod-pg-map-expressions); a regression would resurrect the divergence here.
        assert!(structurally_pg_comparable(&parse_one("SELECT -1")));
        assert!(pg::pg_structural_divergence("SELECT -1").is_none());

        // Controls: ordinary mapped SQL and a single set operation stay comparable.
        assert!(structurally_pg_comparable(&parse_one(
            "SELECT a FROM t WHERE a > 1"
        )));
        assert!(structurally_pg_comparable(&parse_one(
            "SELECT 1 UNION SELECT 2"
        )));
        assert!(structurally_pg_comparable(&parse_one(
            "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3"
        )));
        assert!(
            pg::pg_structural_divergence("SELECT 1 UNION SELECT 2 INTERSECT SELECT 3").is_none()
        );
    }

    /// A fixed example per newly-generated family, built through the `Fuzz*` lowering
    /// and pushed through both the round-trip oracle ([`roundtrip_statement`]) and the
    /// PostgreSQL differential ([`differential_statement`]). A regression that breaks a
    /// family's legality, its render round-trip, or its PostgreSQL accept/reject parity
    /// then surfaces deterministically rather than only on a rare `arbitrary` draw.
    #[test]
    fn fuzz_new_family_anchors_round_trip() {
        let mut statements: Vec<Statement<NoExt>> = Vec::new();

        // The literal forms, including the typed temporal literals whose placeholder
        // values must survive the render round-trip and stay PostgreSQL-parseable.
        for literal in [
            FuzzLiteral::String,
            FuzzLiteral::BooleanTrue,
            FuzzLiteral::BooleanFalse,
            FuzzLiteral::Null,
            FuzzLiteral::Date,
            FuzzLiteral::Time(FuzzTimeZone::WithTimeZone),
            FuzzLiteral::Time(FuzzTimeZone::Unspecified),
            FuzzLiteral::Timestamp(FuzzTimeZone::WithoutTimeZone),
            FuzzLiteral::Interval(Some(FuzzIntervalFields::DayToSecond)),
            FuzzLiteral::Interval(None),
        ] {
            statements.push(query_of_select(minimal_select(
                literal_item(literal),
                FuzzNamedWindows::None,
            )));
        }

        // A plain (non-window) function call in scalar position: `f(DISTINCT a)`.
        statements.push(query_of_select(minimal_select(
            FuzzSelectItem::Expr {
                expr: scalar_atom(FuzzAtom::Function(FuzzPlainFunction::Distinct(
                    FuzzCallArgument::Column(FuzzColumnName::A),
                ))),
                alias: None,
            },
            FuzzNamedWindows::None,
        )));

        // A window function with an inline definition, frame, and exclusion.
        statements.push(query_of_select(minimal_select(
            FuzzSelectItem::Window(FuzzWindowFunction {
                call: FuzzWindowCall::Wildcard,
                over: FuzzWindowSpec::Inline(FuzzWindowDefinition::Framed {
                    partition: Some(FuzzColumnName::B),
                    order: order_by_integer(),
                    frame: FuzzWindowFrame {
                        shape: FuzzFrameShape::RowsUnboundedPrecedingToCurrentRow,
                        exclusion: Some(FuzzFrameExclusion::Ties),
                    },
                }),
            }),
            FuzzNamedWindows::None,
        )));

        // A SELECT carrying a two-entry `WINDOW` clause referenced by `OVER x` (sym 5).
        statements.push(query_of_select(minimal_select(
            FuzzSelectItem::Window(FuzzWindowFunction {
                call: FuzzWindowCall::Args {
                    arg0: FuzzCallArgument::Column(FuzzColumnName::B),
                    arg1: None,
                },
                over: FuzzWindowSpec::Named,
            }),
            FuzzNamedWindows::Two(
                FuzzWindowDefinition::NoFrame {
                    partition: Some(FuzzColumnName::A),
                    order: None,
                },
                FuzzWindowDefinition::NoFrame {
                    partition: None,
                    order: Some(order_by_integer()),
                },
            ),
        )));

        // USING and NATURAL constraints on an outer join (decoupled from the operator).
        for constraint in [
            FuzzJoinConstraint::Using {
                col0: FuzzColumnName::A,
                col1: Some(FuzzColumnName::B),
            },
            FuzzJoinConstraint::Natural,
        ] {
            let mut select = minimal_select(FuzzSelectItem::Wildcard, FuzzNamedWindows::None);
            select.from0 = Some(FuzzTableWithJoins {
                table: fuzz_table(),
                alias: None,
                join0: Some(FuzzJoin {
                    operator: FuzzJoinOperator::LeftOuter(constraint),
                    table: fuzz_table(),
                    alias: Some(FuzzName::X),
                }),
                join1: None,
            });
            statements.push(query_of_select(select));
        }

        // A top-level `VALUES` operand with a mixed expression/`DEFAULT` row.
        statements.push(
            FuzzStatement::Query(query_with_first(FuzzSetOperand::Values(FuzzValues {
                row0: FuzzValuesRow {
                    item0: FuzzValuesItem::Expr(scalar_atom(FuzzAtom::Literal(
                        FuzzLiteral::Integer,
                    ))),
                    item1: Some(FuzzValuesItem::Default),
                },
                row1: None,
            })))
            .into_statement(),
        );

        // Every `WITH` combination — RECURSIVE crossed with the materialization hint —
        // so the CTE accept/reject parity is exercised deterministically.
        for recursive in [false, true] {
            for materialized in [None, Some(true), Some(false)] {
                let mut query = query_with_first(FuzzSetOperand::Select(minimal_select(
                    FuzzSelectItem::Wildcard,
                    FuzzNamedWindows::None,
                )));
                query.with = Some(FuzzWith {
                    recursive,
                    materialized,
                });
                statements.push(FuzzStatement::Query(query).into_statement());
            }
        }

        // CREATE TABLE: an identity column, a named PRIMARY KEY table constraint, and
        // the WITH/TABLESPACE options.
        statements.push(
            FuzzStatement::CreateTable(FuzzCreateTable {
                temporary: FuzzTemporaryAndOnCommit::NotTemporary,
                if_not_exists: false,
                body: FuzzCreateTableBody::Definition {
                    element0: FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::A,
                        data_type: FuzzDataType::BigInt,
                        constraint: Some(FuzzColumnConstraint::Unnamed(
                            FuzzUnnamedColumnOption::Identity(FuzzIdentityColumn {
                                generation: FuzzIdentityGeneration::Always,
                                options: FuzzIdentityOptions {
                                    start: true,
                                    increment: false,
                                    min_value: Some(None),
                                    max_value: Some(Some(())),
                                    cache: false,
                                    cycle: Some(false),
                                },
                            }),
                        )),
                    }),
                    element1: Some(FuzzTableElement::Constraint(FuzzTableConstraintDef {
                        name: Some(FuzzColumnName::C),
                        constraint: FuzzTableConstraint::PrimaryKey(FuzzColumnList {
                            head: FuzzColumnName::A,
                            tail: None,
                        }),
                    })),
                    element2: None,
                },
                with_option: Some(FuzzWithOption {
                    param0: FuzzStorageParameter {
                        name: FuzzColumnName::B,
                        value: true,
                    },
                    param1: None,
                }),
                tablespace: true,
            })
            .into_statement(),
        );

        // CREATE TABLE: a DEFAULT literal column, a STORED generated column, a CHECK
        // table constraint, and a column-level REFERENCES.
        statements.push(
            FuzzStatement::CreateTable(FuzzCreateTable {
                temporary: FuzzTemporaryAndOnCommit::NotTemporary,
                if_not_exists: false,
                body: FuzzCreateTableBody::Definition {
                    element0: FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::A,
                        data_type: FuzzDataType::Integer,
                        constraint: Some(FuzzColumnConstraint::Unnamed(
                            FuzzUnnamedColumnOption::Default(FuzzLiteral::Integer),
                        )),
                    }),
                    element1: Some(FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::B,
                        data_type: FuzzDataType::Integer,
                        constraint: Some(FuzzColumnConstraint::Unnamed(
                            FuzzUnnamedColumnOption::Generated(scalar_atom(FuzzAtom::Column(
                                FuzzObjectName {
                                    head: FuzzName::A,
                                    tail: None,
                                },
                            ))),
                        )),
                    })),
                    element2: Some(FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::C,
                        data_type: FuzzDataType::Integer,
                        constraint: Some(FuzzColumnConstraint::Named {
                            name: None,
                            option: FuzzNamedColumnOption::References(FuzzForeignKeyRef {
                                col0: Some(FuzzColumnName::A),
                                col1: None,
                                match_type: Some(FuzzForeignKeyMatch::Full),
                                on_delete: Some(FuzzReferentialAction::Cascade),
                                on_update: Some(FuzzReferentialActionNoColumns::Restrict),
                            }),
                        }),
                    })),
                },
                with_option: None,
                tablespace: false,
            })
            .into_statement(),
        );

        // CREATE TEMP TABLE ... ON COMMIT DROP AS <query> WITH NO DATA.
        statements.push(
            FuzzStatement::CreateTable(FuzzCreateTable {
                temporary: FuzzTemporaryAndOnCommit::TemporaryOnCommit(
                    FuzzTemporaryKind::Temp,
                    FuzzOnCommitAction::Drop,
                ),
                if_not_exists: true,
                body: FuzzCreateTableBody::AsQuery {
                    col0: Some(FuzzColumnName::A),
                    col1: None,
                    query: FuzzEmbeddedQuery::Select(minimal_select(
                        literal_item(FuzzLiteral::Integer),
                        FuzzNamedWindows::None,
                    )),
                    with_data: Some(false),
                },
                with_option: None,
                tablespace: false,
            })
            .into_statement(),
        );

        // A table-level FOREIGN KEY whose `ON DELETE SET NULL (..)` carries a column
        // list (legal only on `ON DELETE`).
        statements.push(
            FuzzStatement::CreateTable(FuzzCreateTable {
                temporary: FuzzTemporaryAndOnCommit::NotTemporary,
                if_not_exists: false,
                body: FuzzCreateTableBody::Definition {
                    element0: FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::A,
                        data_type: FuzzDataType::Integer,
                        constraint: None,
                    }),
                    element1: Some(FuzzTableElement::Constraint(FuzzTableConstraintDef {
                        name: None,
                        constraint: FuzzTableConstraint::ForeignKey {
                            columns: FuzzColumnList {
                                head: FuzzColumnName::A,
                                tail: None,
                            },
                            references: FuzzForeignKeyRef {
                                col0: Some(FuzzColumnName::B),
                                col1: None,
                                match_type: None,
                                on_delete: Some(FuzzReferentialAction::SetNull(
                                    FuzzActionColumns {
                                        col0: Some(FuzzColumnName::A),
                                        col1: None,
                                    },
                                )),
                                on_update: None,
                            },
                        },
                    })),
                    element2: None,
                },
                with_option: None,
                tablespace: false,
            })
            .into_statement(),
        );

        // INSERT ... OVERRIDING SYSTEM VALUE VALUES (..), with a mixed expr/DEFAULT row.
        statements.push(
            FuzzStatement::Insert(FuzzInsert {
                with: None,
                target: FuzzInsertTarget {
                    alias: true,
                    col0: Some(FuzzColumnName::A),
                    col1: None,
                },
                source: FuzzInsertOverridingAndSource::Values {
                    overriding: Some(FuzzInsertOverriding::SystemValue),
                    values: FuzzInsertValues {
                        row0: FuzzInsertRow {
                            item0: FuzzInsertValue::Expr(scalar_atom(FuzzAtom::Literal(
                                FuzzLiteral::Integer,
                            ))),
                            item1: Some(FuzzInsertValue::Default),
                        },
                        row1: None,
                    },
                },
            })
            .into_statement(),
        );

        // WITH ... INSERT INTO t SELECT ... (the query source must render as a SELECT).
        statements.push(
            FuzzStatement::Insert(FuzzInsert {
                with: Some(FuzzWith {
                    recursive: false,
                    materialized: None,
                }),
                target: FuzzInsertTarget {
                    alias: false,
                    col0: None,
                    col1: None,
                },
                source: FuzzInsertOverridingAndSource::Query {
                    overriding: None,
                    select: minimal_select(
                        literal_item(FuzzLiteral::Integer),
                        FuzzNamedWindows::None,
                    ),
                },
            })
            .into_statement(),
        );

        // A foreign-key column reference using `MATCH PARTIAL`: the parser rejects it to
        // match PostgreSQL, so it lowers to `... REFERENCES t MATCH PARTIAL` and exercises
        // the accept/reject differential's rejection plus the round-trip oracle's skip
        // path (`statement_outside_roundtrip_subset`) deterministically.
        statements.push(
            FuzzStatement::CreateTable(FuzzCreateTable {
                temporary: FuzzTemporaryAndOnCommit::NotTemporary,
                if_not_exists: false,
                body: FuzzCreateTableBody::Definition {
                    element0: FuzzTableElement::Column(FuzzColumnDef {
                        name: FuzzColumnName::A,
                        data_type: FuzzDataType::Integer,
                        constraint: Some(FuzzColumnConstraint::Named {
                            name: None,
                            option: FuzzNamedColumnOption::References(FuzzForeignKeyRef {
                                col0: None,
                                col1: None,
                                match_type: Some(FuzzForeignKeyMatch::Partial),
                                on_delete: None,
                                on_update: None,
                            }),
                        }),
                    }),
                    element1: None,
                    element2: None,
                },
                with_option: None,
                tablespace: false,
            })
            .into_statement(),
        );

        // A target column list combined with `DEFAULT VALUES` is PostgreSQL-illegal: the
        // parser rejects it, so this lowers to `INSERT INTO t AS x (a) DEFAULT VALUES` and
        // exercises the accept/reject differential's rejection plus the round-trip oracle's
        // skip path (`statement_outside_roundtrip_subset`) deterministically.
        statements.push(
            FuzzStatement::Insert(FuzzInsert {
                with: None,
                target: FuzzInsertTarget {
                    alias: true,
                    col0: Some(FuzzColumnName::A),
                    col1: None,
                },
                source: FuzzInsertOverridingAndSource::DefaultValues,
            })
            .into_statement(),
        );

        // UPDATE t AS x SET a = 1, b = DEFAULT FROM t WHERE <predicate>.
        statements.push(
            FuzzStatement::Update(FuzzUpdate {
                with: None,
                target: FuzzDmlTarget { alias: true },
                assignment0: FuzzUpdateAssignment {
                    target: FuzzColumnName::A,
                    value: FuzzUpdateValue::Expr(scalar_atom(FuzzAtom::Literal(
                        FuzzLiteral::Integer,
                    ))),
                },
                assignment1: Some(FuzzUpdateAssignment {
                    target: FuzzColumnName::B,
                    value: FuzzUpdateValue::Default,
                }),
                from0: Some(FuzzTableWithJoins {
                    table: fuzz_table(),
                    alias: None,
                    join0: None,
                    join1: None,
                }),
                selection: Some(FuzzWhereSelection {
                    condition: predicate_eq(),
                }),
            })
            .into_statement(),
        );

        // WITH ... DELETE FROM t USING t WHERE <predicate>.
        statements.push(
            FuzzStatement::Delete(FuzzDelete {
                with: Some(FuzzWith {
                    recursive: false,
                    materialized: Some(false),
                }),
                target: FuzzDmlTarget { alias: false },
                using0: Some(FuzzTableWithJoins {
                    table: fuzz_table(),
                    alias: Some(FuzzName::X),
                    join0: None,
                    join1: None,
                }),
                selection: Some(FuzzWhereSelection {
                    condition: predicate_eq(),
                }),
            })
            .into_statement(),
        );

        for statement in &statements {
            // Both oracles the generator feeds: the structural render round-trip and
            // the PostgreSQL accept/reject (plus structural, where comparable) differential.
            roundtrip_statement(statement);
            differential_statement(statement);
        }
    }

    fn query_with_first(first: FuzzSetOperand) -> FuzzQuery {
        FuzzQuery {
            with: None,
            first,
            set0: None,
            set1: None,
            order0: None,
            order1: None,
            limit: FuzzLimit::None,
        }
    }

    fn query_of_select(select: FuzzSelect) -> Statement<NoExt> {
        FuzzStatement::Query(query_with_first(FuzzSetOperand::Select(select))).into_statement()
    }

    fn minimal_select(projection0: FuzzSelectItem, windows: FuzzNamedWindows) -> FuzzSelect {
        FuzzSelect {
            distinct: false,
            projection0,
            projection1: None,
            projection2: None,
            from0: None,
            from1: None,
            selection: None,
            group0: None,
            group1: None,
            having: None,
            windows,
        }
    }

    fn scalar_atom(first: FuzzAtom) -> FuzzScalar {
        FuzzScalar {
            first,
            step0: None,
            step1: None,
            step2: None,
            unary: None,
        }
    }

    fn literal_item(literal: FuzzLiteral) -> FuzzSelectItem {
        FuzzSelectItem::Expr {
            expr: scalar_atom(FuzzAtom::Literal(literal)),
            alias: None,
        }
    }

    fn order_by_integer() -> FuzzOrderBy {
        FuzzOrderBy {
            expr: scalar_atom(FuzzAtom::Literal(FuzzLiteral::Integer)),
            asc: None,
            nulls_first: None,
        }
    }

    fn predicate_eq() -> FuzzPredicate {
        FuzzPredicate {
            first: FuzzComparison {
                left: scalar_atom(FuzzAtom::Literal(FuzzLiteral::Integer)),
                op: FuzzComparisonOperator::Eq,
                right: scalar_atom(FuzzAtom::Literal(FuzzLiteral::Integer)),
            },
            step0: None,
            step1: None,
            negated: false,
        }
    }

    fn fuzz_table() -> FuzzObjectName {
        FuzzObjectName {
            head: FuzzName::T,
            tail: None,
        }
    }
}
