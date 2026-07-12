// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Cross-dialect bitwise-operator precedence and parity
//! (`bitwise-operators-cross-dialect-gap`).
//!
//! The load-bearing risk of the bitwise family is precedence: the ranks diverge per
//! dialect, so getting one wrong silently reassociates `a | b + c`. These tests pin the
//! per-dialect grouping (rendered fully-parenthesized), round-trip each expression under
//! its own dialect, and — for PostgreSQL — compare our parse against `pg_query`'s tree so
//! the precedence is oracle-checked, not self-attested. The groupings were measured
//! against the live engines (rusqlite, libduckdb `json_serialize_sql`, `pg_query`); MySQL
//! is grammar-derived from the 8.0 operator-precedence manual (its oracle server was
//! unreachable — see the ticket's close note).

use squonk::dialect::{Ansi, DuckDb, MySql, Postgres, Sqlite};
use squonk::{Dialect, parse_with};
use squonk_ast::NoExt;
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderMode};

/// The first statement of `sql`, rendered fully parenthesized so operator grouping is
/// explicit. Panics if `sql` does not parse under `dialect`.
fn parenthesized<D: Dialect<Ext = NoExt>>(sql: &str, dialect: D) -> String {
    let parsed =
        parse_with(sql, dialect).unwrap_or_else(|err| panic!("expected {sql:?} to parse: {err:?}"));
    let config = RenderConfig {
        mode: RenderMode::Parenthesized,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    parsed.statements()[0].displayed(&ctx).to_string()
}

/// Assert `sql` round-trips under `dialect`: its fully-parenthesized render re-parses to a
/// tree that renders identically (an independent precedence oracle — ADR-0008/0014).
fn assert_roundtrips_under<D: Dialect<Ext = NoExt> + Copy>(sql: &str, dialect: D) {
    let once = parenthesized(sql, dialect);
    let twice = parenthesized(&once, dialect);
    assert_eq!(once, twice, "round-trip changed the tree for {sql:?}");
}

#[test]
fn sqlite_bitwise_precedence_groups_one_rank_with_tight_complement() {
    // SQLite (engine-measured via rusqlite): `& | << >>` share one rank between additive
    // and comparison, and prefix `~` binds tightly (above every binary operator).
    for (sql, expected) in [
        ("SELECT 1 << 2 + 3", "SELECT (1 << (2 + 3))"),
        ("SELECT 1 | 2 & 2", "SELECT ((1 | 2) & 2)"),
        ("SELECT 1 & 1 << 2", "SELECT ((1 & 1) << 2)"),
        ("SELECT 6 & 3 | 1", "SELECT ((6 & 3) | 1)"),
        // `~` tight: `~ 1 + 1` is `(~1) + 1`, and it does not fold the bitwise binary in.
        ("SELECT ~ 1 + 1", "SELECT ((~1) + 1)"),
        ("SELECT ~ 1 & 2", "SELECT ((~1) & 2)"),
        // Explicit grouping the parser cannot drop on render (equal-rank guard).
        ("SELECT ~ (1 & 2)", "SELECT (~(1 & 2))"),
    ] {
        assert_eq!(parenthesized(sql, Sqlite), expected, "{sql:?}");
        assert_roundtrips_under(sql, Sqlite);
    }
}

#[test]
fn postgres_bitwise_precedence_matches_any_other_operator_rank() {
    // PostgreSQL (engine-measured via pg_query): the binary bitwise operators — including
    // the `#` XOR — all bind at the "any other operator" rank (looser than additive), and
    // prefix `~` sits between arithmetic and that rank.
    for (sql, expected) in [
        ("SELECT 1 << 2 + 3", "SELECT (1 << (2 + 3))"),
        ("SELECT 1 | 2 & 2", "SELECT ((1 | 2) & 2)"),
        ("SELECT 2 # 3 * 2", "SELECT (2 # (3 * 2))"),
        ("SELECT 5 # 3 | 1", "SELECT ((5 # 3) | 1)"),
        // `~` looser than additive: `~ 1 + 1` is `~ (1 + 1)` — the opposite of SQLite.
        ("SELECT ~ 1 + 1", "SELECT (~(1 + 1))"),
        // `~` tighter-or-equal to the bitwise binaries: `~ 1 & 2` is `(~1) & 2`, yet the
        // explicit `~ (1 & 2)` keeps its parentheses (the strict-inequality break).
        ("SELECT ~ 1 & 2", "SELECT ((~1) & 2)"),
        ("SELECT ~ (1 & 2)", "SELECT (~(1 & 2))"),
    ] {
        assert_eq!(parenthesized(sql, Postgres), expected, "{sql:?}");
        assert_roundtrips_under(sql, Postgres);
    }
}

#[test]
fn mysql_bitwise_precedence_splits_the_family_across_four_ranks() {
    // MySQL (grammar-derived from the 8.0 manual): `| < & < << / >> < additive`, and
    // `^` (XOR) binds *tighter than* multiplicative — the divergence that makes
    // `1 | 2 & 3` group `1 | (2 & 3)` where PostgreSQL/SQLite group `(1 | 2) & 3`.
    for (sql, expected) in [
        ("SELECT 1 | 2 & 3", "SELECT (1 | (2 & 3))"),
        ("SELECT 1 & 1 << 2", "SELECT (1 & (1 << 2))"),
        ("SELECT 1 << 2 + 3", "SELECT (1 << (2 + 3))"),
        ("SELECT 2 ^ 3 * 2", "SELECT ((2 ^ 3) * 2)"),
        ("SELECT ~ 1 + 1", "SELECT ((~1) + 1)"),
    ] {
        assert_eq!(parenthesized(sql, MySql), expected, "{sql:?}");
        assert_roundtrips_under(sql, MySql);
    }

    // The same `1 | 2 & 3` text groups the *other* way in PostgreSQL/SQLite — the
    // divergence encoded as per-dialect binding-power data (ADR-0008).
    assert_eq!(
        parenthesized("SELECT 1 | 2 & 3", Postgres),
        "SELECT ((1 | 2) & 3)"
    );
    assert_eq!(
        parenthesized("SELECT 1 | 2 & 3", Sqlite),
        "SELECT ((1 | 2) & 3)"
    );
}

#[test]
fn bitwise_xor_spelling_is_dialect_specific() {
    // PostgreSQL spells XOR `#`; MySQL spells it `^`. Each dialect accepts its own spelling
    // as the operator.
    assert!(parse_with("SELECT 5 # 3", Postgres).is_ok());
    assert!(parse_with("SELECT 5 ^ 3", MySql).is_ok());
    // PostgreSQL's `^` is NOT XOR — it is arithmetic exponentiation
    // (`CaretOperator::Exponent`, `BinaryOperator::Exponent`), a distinct operator
    // at its own precedence tier. So `SELECT 5 ^ 3` parses under PostgreSQL, but as power, not
    // the `#` XOR: `5 # 3` and `5 ^ 3` are different operators there (verified by the
    // parenthesized renders below).
    assert!(parse_with("SELECT 5 ^ 3", Postgres).is_ok());
    // MySQL treats `#` as a line comment, *not* XOR, so `SELECT 5 # 3` parses to `SELECT 5`
    // (the `# 3` is stripped) — decisively not the `5 # 3` XOR tree PostgreSQL builds.
    assert_eq!(parenthesized("SELECT 5 # 3", MySql), "SELECT 5");

    // DuckDB has no bitwise XOR (it uses the `xor(a, b)` function), so `#` rejects. Its `^`
    // IS exponentiation, at the *same* precedence row PostgreSQL carries — probed identical on
    // DuckDB 1.5.4 (`duckdb-operator-surface-sweep`): `2^3*2 = 16` (tighter than `*`),
    // `2^3^2 = 64` (left-associative), `-2^2 = 4` (unary sign tighter than `^`) — so
    // `caret_operator` is `Exponent` and `5 ^ 3` parses as power.
    assert!(parse_with("SELECT 5 # 3", DuckDb).is_err());
    assert!(parse_with("SELECT 5 ^ 3", DuckDb).is_ok());
    assert_eq!(parenthesized("SELECT 5 ^ 3", DuckDb), "SELECT (5 ^ 3)");
    // The three probed precedence/associativity minimal pairs, as fully-parenthesized renders:
    // `^` tighter than `*`, left-associative, and (via the shared `prefix_sign` rank) tighter
    // than a unary sign — the DuckDB engine's own grouping.
    assert_eq!(
        parenthesized("SELECT 2 ^ 3 * 2", DuckDb),
        "SELECT ((2 ^ 3) * 2)"
    );
    assert_eq!(
        parenthesized("SELECT 2 ^ 3 ^ 2", DuckDb),
        "SELECT ((2 ^ 3) ^ 2)"
    );
    assert_eq!(parenthesized("SELECT -2 ^ 2", DuckDb), "SELECT ((-2) ^ 2)");
    assert_roundtrips_under("SELECT 2 ^ 3 * 2", DuckDb);

    // The XOR spelling round-trips exactly under its dialect (a load-bearing tag).
    assert_eq!(parenthesized("SELECT 5 # 3", Postgres), "SELECT (5 # 3)");
    assert_eq!(parenthesized("SELECT 5 ^ 3", MySql), "SELECT (5 ^ 3)");
    // PostgreSQL's `^` renders back as `^` too, but it is exponentiation, not XOR — it binds
    // tighter than `*` (its own precedence tier), so `5 ^ 3 * 2` groups `(5 ^ 3) * 2` and
    // `2 + 5 ^ 3` groups `2 + (5 ^ 3)`. Distinct from the additive-looser `#` XOR above.
    assert_eq!(parenthesized("SELECT 5 ^ 3", Postgres), "SELECT (5 ^ 3)");
    assert_eq!(
        parenthesized("SELECT 5 ^ 3 * 2", Postgres),
        "SELECT ((5 ^ 3) * 2)"
    );
    assert_eq!(
        parenthesized("SELECT 2 + 5 ^ 3", Postgres),
        "SELECT (2 + (5 ^ 3))"
    );
}

#[test]
fn duckdb_operator_surface_sweep_probe_matrix() {
    // The executable record of the DuckDB operator surface — the per-family verdicts probed
    // against DuckDB 1.5.4 (`duckdb-operator-surface-sweep`, then
    // `duckdb-pg-operator-spelling-under-acceptance`). `caret_operator` is `Exponent` (`^` is
    // power, precedence probed identical to PostgreSQL — see
    // `bitwise_xor_spelling_is_dialect_specific`); `custom_operators` is ON — DuckDB inherits
    // PostgreSQL's generalized maximal-munch lexer and *parse*-accepts the same `Op`-class runs
    // (bind-rejecting the ones with no backing function), so our parse-only parser folds the
    // unknown runs onto the generic named-operator surface. The preset comment on
    // `OperatorSyntax::DUCKDB` carries the full rationale. This test guards the flags and the
    // one lexical divergence (the `#`/`?` charset drop) against an accidental future flip.

    // caret_operator = Exponent: `^` and the whole exponent grammar accept.
    assert!(parse_with("SELECT 2 ^ 3", DuckDb).is_ok());

    // `**` is DuckDB's other exponent spelling (probed `2**3 = 8`). Not folded onto the
    // exponent operator here (an unmodelled synonym), but with the general surface on it now
    // parse-accepts as a generic `Op` run — matching DuckDB's parse verdict (`extract` accepts).
    assert!(parse_with("SELECT 2 ** 3", DuckDb).is_ok());

    // `&&` list/array/GEOMETRY overlap is its own dedicated operator, NOT the generic
    // custom-operator surface: `duckdb-geometry-type-and-overlaps-operator` routes it infix
    // under `double_ampersand: Overlaps` to `BinaryOperator::Overlap`, so `1 && 2` parses
    // (DuckDB's parser accepts it too; the binder then rejects "no function &&(INT, INT)" —
    // a bind reject outside a parse-only validator's scope).
    assert!(parse_with("SELECT 1 && 2", DuckDb).is_ok());

    // custom_operators ON: the general symbolic surface now parse-accepts, matching DuckDB —
    // infix regex `~` / `!~` (probed accept as `regexp_matches`) fold onto the named operator.
    assert!(parse_with("SELECT 'a' ~ 'b'", DuckDb).is_ok());
    assert!(parse_with("SELECT 'a' !~ 'b'", DuckDb).is_ok());

    // The one lexical divergence from PostgreSQL: DuckDB drops `#` and `?` from the `Op`
    // charset (its `#1` positional-column and `?` parameter sigils), so a run stops at either.
    // A bare leading `#` is the positional sigil, so `1 # 2` (no digit) stays a stray-byte
    // reject — matching DuckDB's "syntax error at or near #" — and an embedded `#` breaks a run
    // (`1 @#@ 2` is `@` then a stray `#`). This is why arming the shared `custom_operators`
    // charset would NOT over-accept the `#`/`?` runs: the lexer's `is_operator_char` drops them
    // under `positional_column` / `anonymous_question`.
    assert!(parse_with("SELECT 1 # 2", DuckDb).is_err());
    assert!(parse_with("SELECT 1 @#@ 2", DuckDb).is_err());
    assert!(parse_with("SELECT 1 ? 2", DuckDb).is_err());

    // Prefix `~` bitwise complement rides `bitwise_operators` (a separate flag) and keeps its
    // dedicated unary node — `~ 5` is `UnaryOperator::Not`, not a prefix named operator.
    assert!(parse_with("SELECT ~ 5", DuckDb).is_ok());
}

#[test]
fn duckdb_postfix_operator_precedence_is_engine_measured() {
    // DuckDB keeps the postfix operator reading PostgreSQL removed in 14
    // (`duckdb-postfix-operator-dimension`), binding at the "any other operator" left rank —
    // looser than the arithmetic operators, so a tighter operand groups first while the
    // postfix stays a complete unary token. The fully-parenthesized renders pin the grouping
    // (all engine-measured on DuckDB 1.5.4 via json_serialize_sql), and each round-trips.
    for (sql, expected) in [
        ("SELECT 10!", "SELECT (10 !)"),
        // Looser than `+`/`*`: the whole arithmetic operand groups under the postfix.
        ("SELECT 1 + 2!", "SELECT ((1 + 2) !)"),
        ("SELECT 2 * 3!", "SELECT ((2 * 3) !)"),
        // Tighter than comparison on the left: `1! < 2` is `(1!) < 2`.
        ("SELECT 1! < 2", "SELECT ((1 !) < 2)"),
        // A prefix sign is tighter, so it binds into the operand: `- 3!` is `(-3)!`.
        ("SELECT - 3!", "SELECT ((-3) !)"),
        // The postfix wraps under a tighter cast: `1! :: INT` is `(1!)::INT`.
        ("SELECT 1! :: INT", "SELECT ((1 !)::INT)"),
        ("SELECT 1! IS NULL", "SELECT ((1 !) IS NULL)"),
    ] {
        assert_eq!(parenthesized(sql, DuckDb), expected, "{sql:?}");
        assert_roundtrips_under(sql, DuckDb);
    }
}

#[test]
fn ansi_rejects_the_whole_bitwise_family() {
    // The bitwise operators are a dialect extension, not standard SQL: every spelling ends
    // the expression under ANSI, so the trailing operand is unexpected input.
    for sql in [
        "SELECT 1 | 2",
        "SELECT 1 & 2",
        "SELECT 1 << 2",
        "SELECT 1 >> 2",
        "SELECT ~ 1",
        "SELECT 5 # 3",
        "SELECT 5 ^ 3",
    ] {
        assert!(parse_with(sql, Ansi).is_err(), "ANSI must reject {sql:?}");
    }
}

#[test]
fn duckdb_shares_postgres_bitwise_precedence() {
    // DuckDB inherits PostgreSQL's shared bitwise rank and loose `~` (engine-measured via
    // json_serialize_sql), differing only in having no XOR operator.
    for (sql, expected) in [
        ("SELECT 1 | 2 & 3", "SELECT ((1 | 2) & 3)"),
        ("SELECT 1 << 2 + 3", "SELECT (1 << (2 + 3))"),
        ("SELECT ~ 1 + 1", "SELECT (~(1 + 1))"),
        ("SELECT ~ (1 & 2)", "SELECT (~(1 & 2))"),
    ] {
        assert_eq!(parenthesized(sql, DuckDb), expected, "{sql:?}");
        assert_roundtrips_under(sql, DuckDb);
    }
}

#[test]
fn postgres_bitwise_precedence_is_pg_query_verified() {
    // The precedence oracle: our PostgreSQL parse must map to the same neutral shape as
    // `pg_query`'s parse tree for the discriminating expressions, so the grouping is
    // checked against the real PostgreSQL parser rather than self-attested.
    use crate::oracle::{Comparison, structural_comparison};
    use crate::pg::PgStructuralOracle;

    for sql in [
        "SELECT 1 | 2 & 3",
        "SELECT 1 << 2 + 3",
        "SELECT 6 & 3 | 1",
        "SELECT 2 # 3 * 2",
        "SELECT 5 # 3 | 1",
        "SELECT ~ 1 + 1",
        "SELECT ~ 1 & 2",
        "SELECT 8 >> 1 + 1",
    ] {
        match structural_comparison(sql, Postgres, &PgStructuralOracle) {
            Comparison::Match => {}
            other => panic!("PostgreSQL structural parity failed for {sql:?}: {other:?}"),
        }
    }
}
