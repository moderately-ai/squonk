// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! ADR-0011 canonical-shape + surface-tag proofs (prod-dialect-canonical-tags-audit).
//!
//! For a construct dialects spell differently, the *same* semantics parses to one canonical AST
//! shape, and the spelling that matters for round-trip rides a compact surface tag (data) — never
//! a dialect-specific shape fork. Acceptance of a spelling is gated by `FeatureSet`, not encoded as
//! a distinct shape. The audited inventory and per-construct verdict live in the `squonk_ast::ast`
//! module docs; these are the executable proofs. Evicted from the `coverage` module (it proves
//! ADR-0011, not the ADR-0015 coverage gate).

use crate::coverage::feature_set_with;
use crate::coverage::harness::{AdHocDialect, query_body};
use squonk::ast::dialect::{FeatureDelta, FeatureSet, QueryTailSyntax};
use squonk::ast::{
    BinaryOperator, CastSyntax, Expr, JoinOperator, Limit, LimitSyntax, NoExt, NotEqSpelling,
    SelectItem, SetExpr, Statement,
};
use squonk::dialect::{Ansi, DuckDb, MySql, Postgres};
use squonk::{Parsed, parse_with};

// --- ADR-0011 canonical-shape + surface-tag proofs ------------------------
//
// The audit acceptance evidence (prod-dialect-canonical-tags-audit): for a
// construct dialects spell differently, the *same* semantics parses to one
// canonical AST shape, and the spelling that matters for round-trip rides a
// compact surface tag (data) — never a dialect-specific shape fork. Acceptance
// of a spelling is gated by `FeatureSet`, not encoded as a distinct shape. The
// audited inventory and per-construct verdict live in the `squonk_ast::ast`
// module docs; these are the executable proofs.

/// Postgres without the SQL:2008 fetch-first row-limiting spelling, to prove
/// the `FETCH FIRST`/`LIMIT` acceptance fork is `FeatureSet` data, not a shape.
const NO_FETCH_FIRST_FEATURES: FeatureSet =
    FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
        fetch_first: false,
        ..QueryTailSyntax::POSTGRES
    }));

fn projection_exprs(parsed: &Parsed) -> Vec<&Expr<NoExt>> {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        panic!("expected a SELECT body");
    };
    select
        .projection
        .iter()
        .map(|item| match item {
            SelectItem::Expr { expr, .. } => expr,
            other => panic!("expected a projection expression, got {other:?}"),
        })
        .collect()
}

fn sole_limit(parsed: &Parsed) -> &Limit<NoExt> {
    let [Statement::Query { query, .. }] = parsed.statements() else {
        panic!("expected one query statement");
    };
    query
        .limit
        .as_ref()
        .expect("expected a row-limiting clause")
}

fn first_join_operator(parsed: &Parsed) -> &JoinOperator<NoExt> {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        panic!("expected a SELECT body");
    };
    let [table] = select.from.as_slice() else {
        panic!("expected one FROM relation");
    };
    let [join] = table.joins.as_slice() else {
        panic!("expected one join");
    };
    &join.operator
}

#[test]
fn cast_spellings_share_one_canonical_shape() {
    // `CAST(expr AS type)` (standard), `expr::type` (PostgreSQL), and the prefixed
    // typed string constant `type 'string'` (PostgreSQL) are one canonical
    // `Expr::Cast`; only the `CastSyntax` surface tag records which spelling was
    // written. The prefix form's operand is always a string constant, so all three
    // cast the same `'42'` constant here. Parsing them in one statement shares one
    // interner, so the operand and target type compare by value.
    let parsed = parse_with("SELECT CAST('42' AS int4), '42'::int4, int4 '42'", Postgres)
        .expect("postgres parses all three spellings");
    let exprs = projection_exprs(&parsed);
    let cast = |expr: &'static str, e: &Expr<NoExt>| match e {
        Expr::Cast {
            expr,
            data_type,
            syntax,
            ..
        } => (expr.clone(), data_type.clone(), *syntax),
        other => panic!("{expr} is an Expr::Cast: {other:?}"),
    };
    let (call_operand, call_type, call_syntax) = cast("CAST(...)", exprs[0]);
    let (colon_operand, colon_type, colon_syntax) = cast("'42'::int4", exprs[1]);
    let (prefix_operand, prefix_type, prefix_syntax) = cast("int4 '42'", exprs[2]);
    // Same canonical shape: identical operand and target type, the surface tag the
    // only difference (`Meta` is structural-equality-neutral, ADR-0002).
    assert_eq!(
        call_operand, colon_operand,
        "all spellings cast one operand"
    );
    assert_eq!(
        call_operand, prefix_operand,
        "all spellings cast one operand"
    );
    assert_eq!(call_type, colon_type, "all spellings cast to one type");
    assert_eq!(call_type, prefix_type, "all spellings cast to one type");
    assert_eq!(call_syntax, CastSyntax::Call);
    assert_eq!(colon_syntax, CastSyntax::DoubleColon);
    assert_eq!(prefix_syntax, CastSyntax::PrefixTyped);

    // Acceptance is a `FeatureSet` gate (`expression_syntax.typecast_operator`),
    // not a shape: ANSI rejects `::` while accepting the standard `CAST(...)`.
    assert!(
        parse_with("SELECT a::INT", Ansi).is_err(),
        "ANSI lacks the `::` typecast operator",
    );
    assert!(
        parse_with("SELECT CAST(a AS INT)", Ansi).is_ok(),
        "ANSI accepts the standard CAST(...) call",
    );
}

#[test]
fn limit_and_fetch_first_share_one_canonical_shape() {
    // `LIMIT <n>` and the SQL:2008 `FETCH FIRST <n> ROWS ONLY` are one canonical
    // `Limit`; the `LimitSyntax` tag records the spelling. The row count is a
    // literal (its value rides the source span, not the interner), so the canonical
    // operands compare by value across the two parses.
    let limit = parse_with("SELECT 1 LIMIT 10", Postgres).expect("postgres parses LIMIT");
    let fetch = parse_with("SELECT 1 FETCH FIRST 10 ROWS ONLY", Postgres)
        .expect("postgres parses FETCH FIRST");
    let limit = sole_limit(&limit);
    let fetch = sole_limit(&fetch);
    assert_eq!(
        limit.limit, fetch.limit,
        "both spellings carry one row count"
    );
    assert_eq!(
        limit.offset, fetch.offset,
        "neither spelling sets an offset"
    );
    assert_eq!(limit.syntax, LimitSyntax::LimitOffset);
    assert_eq!(fetch.syntax, LimitSyntax::FetchFirst);

    // Acceptance is a `FeatureSet` gate (`query_tail_syntax.fetch_first`), not a shape:
    // a dialect with it off rejects `FETCH FIRST` but still accepts `LIMIT`.
    let no_fetch = AdHocDialect(&NO_FETCH_FIRST_FEATURES);
    assert!(
        parse_with("SELECT 1 FETCH FIRST 10 ROWS ONLY", no_fetch).is_err(),
        "a fetch_first-off dialect rejects FETCH FIRST",
    );
    assert!(
        parse_with("SELECT 1 LIMIT 10", no_fetch).is_ok(),
        "a fetch_first-off dialect still accepts LIMIT",
    );
}

#[test]
fn limit_offset_comma_shares_one_canonical_shape() {
    // MySQL's `LIMIT <offset>, <count>` is the *same* canonical `Limit` as
    // `LIMIT <count> OFFSET <offset>` (offset first, count second) — one shape, never a
    // dialect-specific node (ADR-0011) — carrying a `CommaOffset` surface tag so a
    // source-fidelity render replays the comma spelling. The counts are literals whose
    // value rides the source span, so the canonical operands compare by value across
    // the two parses; only the spelling tag differs.
    let comma = parse_with("SELECT 1 LIMIT 5, 10", MySql).expect("mysql parses LIMIT a, b");
    let explicit =
        parse_with("SELECT 1 LIMIT 10 OFFSET 5", MySql).expect("mysql parses LIMIT/OFFSET");
    let comma = sole_limit(&comma);
    let explicit = sole_limit(&explicit);
    assert_eq!(
        comma.limit, explicit.limit,
        "the count is the second comma argument",
    );
    assert_eq!(
        comma.offset, explicit.offset,
        "the offset is the first comma argument",
    );
    assert_eq!(comma.syntax, LimitSyntax::CommaOffset);
    assert_eq!(explicit.syntax, LimitSyntax::LimitOffset);

    // Both surfaces round-trip exactly under the source-fidelity render.
    for sql in ["SELECT 1 LIMIT 5, 10", "SELECT 1 LIMIT 10 OFFSET 5"] {
        match crate::corpus_roundtrip::roundtrip(sql, MySql) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("mysql rejected its own limit surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }

    // Acceptance is a `FeatureSet` gate (`query_tail_syntax.limit_offset_comma`), not a
    // shape: with it off the comma is trailing input, but `LIMIT 10` still parses.
    let no_comma = feature_set_with("limit_offset_comma", &FeatureSet::MYSQL, false);
    assert!(
        parse_with("SELECT 1 LIMIT 5, 10", AdHocDialect(&no_comma)).is_err(),
        "a limit_offset_comma-off dialect rejects the comma form",
    );
    assert!(
        parse_with("SELECT 1 LIMIT 10", AdHocDialect(&no_comma)).is_ok(),
        "a limit_offset_comma-off dialect still accepts plain LIMIT",
    );
}

#[test]
fn not_equal_spellings_share_one_operator_with_a_surface_tag() {
    // `<>` (standard) and `!=` (synonym) are one canonical `BinaryOperator::NotEq`
    // operator carrying a `NotEqSpelling` surface tag: the same variant, differing
    // only in the tag (like `=`/`==` on `Eq`), so a source-fidelity render replays the
    // exact spelling. Parsing both in one statement, they share the operator but *not*
    // the tag.
    let parsed = parse_with("SELECT a <> b, a != b", Postgres).expect("postgres parses both");
    let exprs = projection_exprs(&parsed);
    let (angle, bang) = (exprs[0], exprs[1]);
    assert!(
        matches!(
            angle,
            Expr::BinaryOp {
                op: BinaryOperator::NotEq(NotEqSpelling::AngleBracket),
                ..
            }
        ),
        "`<>` is a NotEq(AngleBracket): {angle:?}",
    );
    assert!(
        matches!(
            bang,
            Expr::BinaryOp {
                op: BinaryOperator::NotEq(NotEqSpelling::Bang),
                ..
            }
        ),
        "`!=` is a NotEq(Bang): {bang:?}",
    );
    assert_ne!(
        angle, bang,
        "`<>` and `!=` differ only in the `NotEqSpelling` surface tag",
    );

    // Each spelling round-trips exactly under the source-fidelity render.
    for sql in ["SELECT a <> b", "SELECT a != b"] {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("postgres rejected its own inequality surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }

    // No acceptance fork: both spellings are universally accepted (the `!` byte is
    // operator-class in the shared lexer), so ANSI accepts both too.
    assert!(
        parse_with("SELECT a <> b", Ansi).is_ok(),
        "ANSI accepts `<>`"
    );
    assert!(
        parse_with("SELECT a != b", Ansi).is_ok(),
        "ANSI accepts `!=`"
    );
}

#[test]
fn join_keyword_noise_words_share_one_operator_with_a_surface_tag() {
    // INNER and OUTER are optional noise words: `JOIN` ≡ `INNER JOIN` and
    // `LEFT JOIN` ≡ `LEFT OUTER JOIN`. One canonical `JoinOperator` variant per join
    // kind carrying an `inner`/`outer` bool surface tag — the same variant, differing
    // only in the tag, so a source-fidelity render replays the written keyword.
    let bare_inner =
        parse_with("SELECT 1 FROM a JOIN b ON a = b", Postgres).expect("bare JOIN parses");
    let spelled_inner =
        parse_with("SELECT 1 FROM a INNER JOIN b ON a = b", Postgres).expect("INNER JOIN parses");
    assert!(matches!(
        first_join_operator(&bare_inner),
        JoinOperator::Inner { inner: false, .. }
    ));
    assert!(
        matches!(
            first_join_operator(&spelled_inner),
            JoinOperator::Inner { inner: true, .. }
        ),
        "`INNER JOIN` is the same Inner variant with the `inner` tag set",
    );

    let bare_left =
        parse_with("SELECT 1 FROM a LEFT JOIN b ON a = b", Postgres).expect("LEFT JOIN parses");
    let spelled_left = parse_with("SELECT 1 FROM a LEFT OUTER JOIN b ON a = b", Postgres)
        .expect("LEFT OUTER JOIN parses");
    assert!(matches!(
        first_join_operator(&bare_left),
        JoinOperator::LeftOuter { outer: false, .. }
    ));
    assert!(
        matches!(
            first_join_operator(&spelled_left),
            JoinOperator::LeftOuter { outer: true, .. }
        ),
        "`LEFT OUTER JOIN` is the same LeftOuter variant with the `outer` tag set",
    );

    // Each written keyword round-trips exactly under the source-fidelity render.
    for sql in [
        "SELECT 1 FROM a JOIN b ON a = b",
        "SELECT 1 FROM a INNER JOIN b ON a = b",
        "SELECT 1 FROM a LEFT JOIN b ON a = b",
        "SELECT 1 FROM a LEFT OUTER JOIN b ON a = b",
    ] {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("postgres rejected its own join surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }
}

#[test]
fn straight_join_shares_inner_join_shape_with_a_surface_tag() {
    // MySQL `STRAIGHT_JOIN` is an inner join that also fixes the table read order.
    // Per ADR-0011 it is the canonical `JoinOperator::Inner` shape carrying the
    // `straight` surface tag — never a new variant — so it is the same shape as a
    // bare `JOIN` save for the tag (which, unlike INNER/OUTER, is preserved so the
    // spelling round-trips).
    let straight = parse_with("SELECT 1 FROM a STRAIGHT_JOIN b ON a = b", MySql)
        .expect("MySQL parses STRAIGHT_JOIN");
    let plain =
        parse_with("SELECT 1 FROM a JOIN b ON a = b", MySql).expect("MySQL parses a bare JOIN");
    assert!(matches!(
        first_join_operator(&straight),
        JoinOperator::Inner { straight: true, .. },
    ));
    assert!(
        matches!(
            first_join_operator(&plain),
            JoinOperator::Inner {
                straight: false,
                ..
            },
        ),
        "a bare JOIN is the same canonical Inner shape with the tag unset",
    );

    // The `SELECT STRAIGHT_JOIN ...` modifier rides `Select` as a flag — the
    // query-wide form of the same hint, mirroring how `distinct` rides `Select`.
    let modifier = parse_with("SELECT STRAIGHT_JOIN a FROM t", MySql)
        .expect("MySQL parses the SELECT STRAIGHT_JOIN modifier");
    let SetExpr::Select { select, .. } = query_body(&modifier) else {
        panic!("expected a SELECT body");
    };
    assert!(select.straight_join, "the SELECT STRAIGHT_JOIN flag is set");

    // Both surfaces round-trip exactly (the join keyword and the select modifier).
    for sql in [
        "SELECT 1 FROM a STRAIGHT_JOIN b ON a = b",
        "SELECT STRAIGHT_JOIN a FROM t",
    ] {
        match crate::corpus_roundtrip::roundtrip(sql, MySql) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("MySQL rejected its own STRAIGHT_JOIN surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }

    // Acceptance is a `FeatureSet` gate (`join_syntax.straight_join`), not a
    // shape: ANSI/PostgreSQL reject the join form (there `STRAIGHT_JOIN` is a
    // non-reserved word the table factor takes as an alias, leaving `b ON ...` as
    // leftover input).
    assert!(
        parse_with("SELECT 1 FROM a STRAIGHT_JOIN b ON a = b", Ansi).is_err(),
        "ANSI lacks the STRAIGHT_JOIN join operator",
    );
    assert!(
        parse_with("SELECT 1 FROM a STRAIGHT_JOIN b ON a = b", Postgres).is_err(),
        "PostgreSQL lacks the STRAIGHT_JOIN join operator",
    );
}

#[test]
fn nonstandard_joins_are_new_operators_under_duckdb() {
    // DuckDB `ASOF [side] JOIN` / `POSITIONAL JOIN` change matching semantics
    // (nearest-match / row-position), so unlike `STRAIGHT_JOIN` they are new
    // `JoinOperator` variants, not surface tags on `Inner` (the ticket's
    // new-canonical-shape resolution). The bare-factor spellings exercise the
    // DuckDb-only `asof`/`positional` ColId reservation: the words cannot be read
    // as the left factor's alias.
    use squonk::ast::{AsOfJoinKind, JoinConstraint};

    let asof = parse_with("SELECT 1 FROM a ASOF JOIN b ON a.t >= b.t", DuckDb)
        .expect("DuckDb parses ASOF JOIN");
    assert!(matches!(
        first_join_operator(&asof),
        JoinOperator::AsOf {
            kind: AsOfJoinKind::Inner,
            ..
        },
    ));
    // `ASOF INNER JOIN` and `ASOF LEFT OUTER JOIN` collapse onto the canonical
    // side (INNER/OUTER are noise words here exactly as in the standard joins).
    let spelled_inner = parse_with("SELECT 1 FROM a ASOF INNER JOIN b ON a.t >= b.t", DuckDb)
        .expect("DuckDb parses ASOF INNER JOIN");
    assert!(matches!(
        first_join_operator(&spelled_inner),
        JoinOperator::AsOf {
            kind: AsOfJoinKind::Inner,
            ..
        },
    ));
    for (sql, want) in [
        (
            "SELECT 1 FROM a ASOF LEFT JOIN b ON a.t >= b.t",
            AsOfJoinKind::Left,
        ),
        (
            "SELECT 1 FROM a ASOF LEFT OUTER JOIN b ON a.t >= b.t",
            AsOfJoinKind::Left,
        ),
        (
            "SELECT 1 FROM a ASOF RIGHT JOIN b ON a.t >= b.t",
            AsOfJoinKind::Right,
        ),
        (
            "SELECT 1 FROM a ASOF FULL JOIN b ON a.t >= b.t",
            AsOfJoinKind::Full,
        ),
    ] {
        let parsed = parse_with(sql, DuckDb).expect("DuckDb parses every ASOF side");
        match first_join_operator(&parsed) {
            JoinOperator::AsOf { kind, .. } => assert_eq!(*kind, want, "{sql:?}"),
            other => panic!("{sql:?} is an AsOf join: {other:?}"),
        }
    }
    // The USING form carries the ordinary `Using` constraint; POSITIONAL carries
    // none at all (the variant has no constraint field, like `Cross`).
    let using = parse_with("SELECT 1 FROM a ASOF JOIN b USING (t)", DuckDb)
        .expect("DuckDb parses ASOF JOIN USING");
    assert!(matches!(
        first_join_operator(&using),
        JoinOperator::AsOf {
            constraint: JoinConstraint::Using { .. },
            ..
        },
    ));
    let positional = parse_with("SELECT 1 FROM a POSITIONAL JOIN b", DuckDb)
        .expect("DuckDb parses POSITIONAL JOIN");
    assert!(matches!(
        first_join_operator(&positional),
        JoinOperator::Positional { .. },
    ));

    // Engine-mirrored parse-level rejects (probed on DuckDB 1.5.4): a bare `ASOF
    // JOIN` needs its constraint, POSITIONAL takes no constraint and no side, and
    // ASOF composes with neither NATURAL nor CROSS. (An *equality* `ON` still
    // parses — DuckDB's inequality requirement is bind-time, not parse-time.)
    for sql in [
        "SELECT 1 FROM a ASOF JOIN b",
        "SELECT 1 FROM a POSITIONAL JOIN b ON a.t = b.t",
        "SELECT 1 FROM a POSITIONAL JOIN b USING (t)",
        "SELECT 1 FROM a POSITIONAL LEFT JOIN b",
        "SELECT 1 FROM a NATURAL ASOF JOIN b",
        "SELECT 1 FROM a ASOF CROSS JOIN b",
    ] {
        assert!(
            parse_with(sql, DuckDb).is_err(),
            "DuckDb must reject (engine parse-rejects): {sql:?}",
        );
    }
    assert!(
        parse_with("SELECT 1 FROM a ASOF JOIN b ON a.t = b.t", DuckDb).is_ok(),
        "an equality ON parses (the inequality check is DuckDB bind-time)",
    );

    // Round-trip both operators, the sweep's derived-table example among them.
    for sql in [
        "SELECT 1 FROM a ASOF JOIN b ON a.t >= b.t",
        "SELECT 1 FROM a ASOF LEFT JOIN (SELECT 1 AS x) b ON a.x >= b.x",
        "SELECT 1 FROM a ASOF JOIN b USING (t)",
        "SELECT 1 FROM a POSITIONAL JOIN b",
    ] {
        match crate::corpus_roundtrip::roundtrip(sql, DuckDb) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("DuckDb rejected its own nonstandard-join surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }

    // The DuckDB *meaning* needs the DuckDb preset (flag + reservation). Under
    // ANSI/PostgreSQL the words are non-reserved and — unlike `STRAIGHT_JOIN`,
    // where the absorbed alias leaves `b ON ...` as leftover input — the very
    // next word is `JOIN`, so the text still parses, as an aliased *plain*
    // join: a different, dialect-correct tree, not a reject.
    for parse in [
        parse_with("SELECT 1 FROM a ASOF JOIN b ON a.t >= b.t", Ansi),
        parse_with("SELECT 1 FROM a ASOF JOIN b ON a.t >= b.t", Postgres),
        parse_with("SELECT 1 FROM a POSITIONAL JOIN b", Ansi),
        parse_with("SELECT 1 FROM a POSITIONAL JOIN b", Postgres),
    ] {
        let parsed = parse.expect("the alias reading parses outside DuckDb");
        assert!(
            matches!(
                first_join_operator(&parsed),
                JoinOperator::Inner {
                    straight: false,
                    ..
                },
            ),
            "outside DuckDb the word aliases the factor and the join stays Inner",
        );
    }
}

#[test]
fn semi_anti_joins_are_new_operators_under_duckdb() {
    // DuckDB `[ASOF|NATURAL] SEMI JOIN` / `ANTI JOIN` keep/drop each left row by
    // right-match membership — a `join_type` (never a side), so new `JoinOperator`
    // variants (the ticket's new-canonical-shape resolution), mirroring
    // ASOF/POSITIONAL. The bare-factor spellings exercise the DuckDb-only
    // `semi`/`anti` ColId reservation: the words cannot be read as the left alias.
    use squonk::ast::JoinConstraint;

    // Bare SEMI/ANTI (REGULAR ref-type): the ON and USING constraint forms.
    let semi = parse_with("SELECT 1 FROM a SEMI JOIN b ON a.i = b.i", DuckDb)
        .expect("DuckDb parses SEMI JOIN");
    assert!(matches!(
        first_join_operator(&semi),
        JoinOperator::Semi {
            asof: false,
            constraint: JoinConstraint::On { .. },
            ..
        },
    ));
    let anti = parse_with("SELECT 1 FROM a ANTI JOIN b ON a.i = b.i", DuckDb)
        .expect("DuckDb parses ANTI JOIN");
    assert!(matches!(
        first_join_operator(&anti),
        JoinOperator::Anti {
            asof: false,
            constraint: JoinConstraint::On { .. },
            ..
        },
    ));
    let using = parse_with("SELECT 1 FROM a SEMI JOIN b USING (i)", DuckDb)
        .expect("DuckDb parses SEMI JOIN USING");
    assert!(matches!(
        first_join_operator(&using),
        JoinOperator::Semi {
            asof: false,
            constraint: JoinConstraint::Using { .. },
            ..
        },
    ));
    // NATURAL SEMI/ANTI: the shared-column match is the constraint (no ON/USING).
    let natural_semi = parse_with("SELECT 1 FROM a NATURAL SEMI JOIN b", DuckDb)
        .expect("DuckDb parses NATURAL SEMI JOIN");
    assert!(matches!(
        first_join_operator(&natural_semi),
        JoinOperator::Semi {
            asof: false,
            constraint: JoinConstraint::Natural { .. },
            ..
        },
    ));
    let natural_anti = parse_with("SELECT 1 FROM a NATURAL ANTI JOIN b", DuckDb)
        .expect("DuckDb parses NATURAL ANTI JOIN");
    assert!(matches!(
        first_join_operator(&natural_anti),
        JoinOperator::Anti {
            asof: false,
            constraint: JoinConstraint::Natural { .. },
            ..
        },
    ));
    // ASOF SEMI/ANTI: the `asof: true` composition, constraint mandatory.
    let asof_semi = parse_with("SELECT 1 FROM a ASOF SEMI JOIN b ON a.t >= b.t", DuckDb)
        .expect("DuckDb parses ASOF SEMI JOIN");
    assert!(matches!(
        first_join_operator(&asof_semi),
        JoinOperator::Semi {
            asof: true,
            constraint: JoinConstraint::On { .. },
            ..
        },
    ));
    let asof_anti = parse_with("SELECT 1 FROM a ASOF ANTI JOIN b USING (t)", DuckDb)
        .expect("DuckDb parses ASOF ANTI JOIN USING");
    assert!(matches!(
        first_join_operator(&asof_anti),
        JoinOperator::Anti {
            asof: true,
            constraint: JoinConstraint::Using { .. },
            ..
        },
    ));

    // Engine-mirrored parse-level rejects (probed on DuckDB 1.5.4): SEMI/ANTI are a
    // join_type, so a bare form needs its ON/USING constraint (unless NATURAL
    // supplies it) and takes no side (LEFT/INNER/…), no CROSS/POSITIONAL
    // composition, and `ASOF` must *precede*, never follow, SEMI/ANTI — and never
    // co-occur with NATURAL. (An equality `ON` still parses; the predicate rules are
    // DuckDB bind-time.)
    for sql in [
        "SELECT 1 FROM a SEMI JOIN b",
        "SELECT 1 FROM a ANTI JOIN b",
        "SELECT 1 FROM a LEFT SEMI JOIN b ON a.i = b.i",
        "SELECT 1 FROM a INNER SEMI JOIN b ON a.i = b.i",
        "SELECT 1 FROM a CROSS SEMI JOIN b",
        "SELECT 1 FROM a POSITIONAL SEMI JOIN b",
        "SELECT 1 FROM a SEMI ASOF JOIN b ON a.t >= b.t",
        "SELECT 1 FROM a ASOF LEFT SEMI JOIN b ON a.t >= b.t",
        "SELECT 1 FROM a NATURAL ASOF SEMI JOIN b",
        "SELECT 1 FROM a ASOF SEMI JOIN b",
    ] {
        assert!(
            parse_with(sql, DuckDb).is_err(),
            "DuckDb must reject (engine parse-rejects): {sql:?}",
        );
    }
    assert!(
        parse_with("SELECT 1 FROM a SEMI JOIN b ON a.i = b.i", DuckDb).is_ok(),
        "an equality ON parses",
    );

    // Round-trip every accepted surface (parser-only; the NATURAL prefix and ASOF
    // composition must survive render -> re-parse).
    for sql in [
        "SELECT 1 FROM a SEMI JOIN b ON a.i = b.i",
        "SELECT 1 FROM a ANTI JOIN b ON a.i = b.i",
        "SELECT 1 FROM a SEMI JOIN b USING (i)",
        "SELECT 1 FROM a NATURAL SEMI JOIN b",
        "SELECT 1 FROM a NATURAL ANTI JOIN b",
        "SELECT 1 FROM a ASOF SEMI JOIN b ON a.t >= b.t",
        "SELECT 1 FROM a ASOF ANTI JOIN b USING (t)",
    ] {
        match crate::corpus_roundtrip::roundtrip(sql, DuckDb) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("DuckDb rejected its own semi/anti-join surface: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }
    }

    // The DuckDB *meaning* needs the DuckDb preset (flag + reservation). Under
    // ANSI/PostgreSQL the words are non-reserved and — the next word being `JOIN` —
    // the text still parses, as an aliased *plain* join: a different, dialect-correct
    // tree, not a reject (the ASOF/POSITIONAL precedent).
    for parse in [
        parse_with("SELECT 1 FROM a SEMI JOIN b ON a.i = b.i", Ansi),
        parse_with("SELECT 1 FROM a SEMI JOIN b ON a.i = b.i", Postgres),
        parse_with("SELECT 1 FROM a ANTI JOIN b ON a.i = b.i", Ansi),
        parse_with("SELECT 1 FROM a ANTI JOIN b ON a.i = b.i", Postgres),
    ] {
        let parsed = parse.expect("the alias reading parses outside DuckDb");
        assert!(
            matches!(
                first_join_operator(&parsed),
                JoinOperator::Inner {
                    straight: false,
                    ..
                },
            ),
            "outside DuckDb the word aliases the factor and the join stays Inner",
        );
    }
}
