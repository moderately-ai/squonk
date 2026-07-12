// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The measured formatter coverage inventory (ticket
//! `formatter-structured-layout-and-comment-fidelity`).
//!
//! Where [`super::tests`] proves invariants (parse-back, spelling fidelity, no-drop),
//! this module *records what the formatter actually does today* on one fixture per
//! layout / comment / spelling family, via committed `insta` snapshots (the
//! conformance/bench snapshot idiom). The snapshots are the source of truth for the
//! v1 stable-vs-preview boundary written in [`crate::format`]: every documented
//! limitation has a fixture here, so its blast radius is concrete and any future fix
//! shows up as a reviewable snapshot diff rather than a silently changed behaviour.
//!
//! Reading the snapshots: `SUPPORTED` families are the v1 stable surface (structured
//! layout, style knobs, and the comment placements that render where a reader
//! expects). `PREVIEW` families carry a documented limitation and an owner ticket; the
//! recorded output is the *current* behaviour, warts included.
//!
//! The former tail-relocation families (statement-boundary, fragment-interior, and
//! comma-side comments) are now fixed: their fixtures below record the corrected,
//! position-stable placement, kept here as regression anchors. Subqueries in
//! structured positions now re-lay out recursively (`inventory_subquery_relayout`);
//! the fragment-fallback family records the remaining flat-fragment behaviour.
//! The former UESCAPE-identifier loss is now fixed too: both `U&'...'` string
//! *literals* and the `U&"..."` unicode *identifier* form round-trip verbatim (the
//! identifier carries [`QuoteStyle::UnicodeDouble`](crate::ast::QuoteStyle), whose
//! source spelling the canonical renderer replays), so the fixture below records the
//! preserved spelling as a regression anchor.

use super::{FormatOptions, KeywordCase, format_sql};
use crate::BuiltinDialect;

const ANSI: BuiltinDialect = BuiltinDialect::Ansi;
const PG: BuiltinDialect = BuiltinDialect::Postgres;

/// Format `sql`, or record the parse failure inline so a fixture that does not parse
/// still leaves a measured trace in the inventory instead of aborting the snapshot.
fn format_or_error(sql: &str, dialect: BuiltinDialect, opts: &FormatOptions) -> String {
    match format_sql(sql, dialect, opts) {
        Ok(out) => out,
        Err(err) => format!("«parse error: {err}»"),
    }
}

/// One inventory fixture: a labelled `input:` / `output:` block, both indented so
/// multi-line comment fixtures stay readable in the snapshot.
fn entry(label: &str, sql: &str, dialect: BuiltinDialect, opts: &FormatOptions) -> String {
    let out = format_or_error(sql, dialect, opts);
    let mut block = format!("[{label}]  ({dialect:?})\n  input:\n");
    for line in sql.lines() {
        block.push_str(&format!("    {line}\n"));
    }
    block.push_str("  output:\n");
    if out.is_empty() {
        block.push_str("    <empty>\n");
    } else {
        for line in out.lines() {
            block.push_str(&format!("    {line}\n"));
        }
    }
    block
}

/// Assemble one family's fixtures (all under `opts`) into a snapshot body.
fn inventory(fixtures: &[(&str, &str, BuiltinDialect)], opts: &FormatOptions) -> String {
    let mut body = String::new();
    for (label, sql, dialect) in fixtures {
        body.push_str(&entry(label, sql, *dialect, opts));
        body.push('\n');
    }
    body
}

// ===========================================================================
// SUPPORTED (v1 stable): structured layout the formatter fully re-lays out.
// ===========================================================================

#[test]
fn inventory_mainstream_structured_layout() {
    // The clause / statement-list / CTE / set-op shapes the pretty path structures
    // into breakable groups (see `format::render`). These are the v1 guarantee.
    let fixtures = &[
        (
            "select-clauses",
            "select a, b from t where a = 1 group by a having count(*) > 1",
            ANSI,
        ),
        ("distinct", "SELECT DISTINCT a FROM t", ANSI),
        ("distinct-on", "SELECT DISTINCT ON (a) a, b FROM t", PG),
        (
            "inner-join",
            "SELECT a FROM t1 JOIN t2 ON t1.id = t2.id WHERE t1.x > 0",
            ANSI,
        ),
        (
            "left-join",
            "SELECT a FROM t1 LEFT JOIN t2 ON t1.id = t2.id",
            ANSI,
        ),
        (
            "order-by-limit",
            "SELECT a FROM t ORDER BY a DESC, b ASC LIMIT 10",
            ANSI,
        ),
        (
            "single-cte",
            "WITH cte AS (SELECT a FROM t) SELECT a FROM cte",
            ANSI,
        ),
        (
            "multi-cte",
            "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT x FROM a, b",
            ANSI,
        ),
        (
            "recursive-cte",
            "WITH RECURSIVE r AS (SELECT 1) SELECT n FROM r",
            ANSI,
        ),
        (
            "union-all",
            "SELECT a FROM t1 UNION ALL SELECT a FROM t2",
            ANSI,
        ),
        (
            "intersect-chain",
            "SELECT a FROM t1 UNION SELECT a FROM t2 INTERSECT SELECT a FROM t3",
            ANSI,
        ),
        ("statement-list", "SELECT a FROM t; SELECT b FROM u", ANSI),
    ];
    insta::assert_snapshot!(
        "inventory_mainstream_structured_layout",
        inventory(fixtures, &FormatOptions::default())
    );
}

#[test]
fn inventory_width_and_style_knobs() {
    // The v1 style surface: max line length drives group breaks, indent width and
    // keyword case are honoured. Wide projection at width 40 must break one item/line.
    let wide = "SELECT alpha, bravo, charlie, delta, echo, foxtrot, golf, hotel, india FROM t";
    let mut body = String::new();
    body.push_str(&entry(
        "width-40-breaks-projection",
        wide,
        ANSI,
        &FormatOptions::default().with_max_line_length(40),
    ));
    body.push('\n');
    body.push_str(&entry(
        "indent-width-4",
        wide,
        ANSI,
        &FormatOptions::default()
            .with_max_line_length(30)
            .with_indent_width(4),
    ));
    body.push('\n');
    body.push_str(&entry(
        "keyword-case-lower",
        "SELECT a FROM t WHERE a IS NOT NULL",
        ANSI,
        &FormatOptions::default().with_keyword_case(KeywordCase::Lower),
    ));
    body.push('\n');
    body.push_str(&entry(
        "keyword-case-preserve-lower-dominant",
        "select a from t where a = 1",
        ANSI,
        &FormatOptions::default().with_keyword_case(KeywordCase::Preserve),
    ));
    insta::assert_snapshot!("inventory_width_and_style_knobs", body);
}

// ===========================================================================
// SUPPORTED (v1 stable): comment placements that render where a reader expects.
// ===========================================================================

#[test]
fn inventory_comment_placement_supported() {
    // The two comment families the structured path renders in place: an end-of-line
    // comment trailing a list item stays with that item (and forces the list to
    // break), and a comment before a clause keyword anchors before the keyword via the
    // parser's clause marks. Anything attaching to a bare statement or a fragment does
    // NOT land here — see the two relocation families below.
    let fixtures = &[
        (
            "trailing-eol-projection-item",
            "SELECT a, -- keep with a\nb FROM t",
            ANSI,
        ),
        (
            "trailing-eol-group-item",
            "SELECT a, b FROM t GROUP BY a, -- keep with a\nb",
            ANSI,
        ),
        (
            "trailing-eol-order-item",
            "SELECT a FROM t ORDER BY a, -- then\nb",
            ANSI,
        ),
        (
            "leading-before-clause-keyword",
            "SELECT a FROM t WHERE a = 1\n-- note\nGROUP BY a",
            ANSI,
        ),
        (
            "leading-before-where-predicate",
            "SELECT a FROM t\n-- filter\nWHERE a = 1",
            ANSI,
        ),
    ];
    insta::assert_snapshot!(
        "inventory_comment_placement_supported",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// SUPPORTED (v1 stable): recursive subquery re-layout.
// ===========================================================================

#[test]
fn inventory_subquery_relayout() {
    // A subquery in a structured position — a FROM derived table (leading relation or
    // joined), a scalar / IN / EXISTS / quantified predicate in WHERE/HAVING or the
    // projection — is re-laid out clause-per-line, indented one level, recursively.
    // The threshold is structural: a subquery whose structured layout would still be
    // one line (`(SELECT 1)`) stays inline (see `PrettyRenderer::structured_subquery`).
    let fixtures = &[
        (
            "exists-subquery",
            "SELECT a FROM t WHERE EXISTS (SELECT 1 FROM u WHERE u.k = t.k)",
            ANSI,
        ),
        (
            "not-exists-subquery",
            "SELECT a FROM t WHERE NOT EXISTS (SELECT 1 FROM u WHERE u.k = t.k)",
            ANSI,
        ),
        (
            "scalar-subquery-projection",
            "SELECT a, (SELECT max(b) FROM u WHERE u.k = t.k) AS mx FROM t",
            ANSI,
        ),
        (
            "quantified-comparison-subquery",
            "SELECT a FROM t WHERE a > ALL (SELECT b FROM u WHERE u.k = t.k)",
            ANSI,
        ),
        (
            "join-derived-subquery",
            "SELECT a FROM t JOIN (SELECT k, max(b) AS mb FROM u GROUP BY k) m ON m.k = t.k",
            ANSI,
        ),
        (
            "nested-two-level-subquery",
            "SELECT x FROM (SELECT a AS x FROM u WHERE a IN (SELECT id FROM v WHERE v.ok)) s",
            ANSI,
        ),
        (
            "set-op-derived-subquery",
            "SELECT x FROM (SELECT a FROM t1 UNION ALL SELECT a FROM t2) s",
            ANSI,
        ),
        (
            "trivial-subquery-stays-inline",
            "SELECT a FROM t WHERE a > (SELECT 1)",
            ANSI,
        ),
    ];
    insta::assert_snapshot!(
        "inventory_subquery_relayout",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// PREVIEW: deep expression re-layout — flat fragments stay single-line.
// ===========================================================================

#[test]
fn inventory_fragment_fallback_no_deep_relayout() {
    // Everything the structured path does not recognise renders through the canonical
    // renderer as a single-line fragment: nested expressions are never re-laid out,
    // and a non-SELECT statement / exotic body collapses to one line. Subqueries in
    // structured positions are the exception — they re-lay out recursively (the
    // derived / scalar-WHERE / IN fixtures below record that layout; see also
    // `inventory_subquery_relayout`).
    let fixtures = &[
        ("nested-arithmetic", "SELECT a + b * c - d FROM t", ANSI),
        (
            "case-expression",
            "SELECT CASE WHEN a > 0 THEN 'p' ELSE 'n' END FROM t",
            ANSI,
        ),
        (
            "derived-subquery",
            "SELECT x FROM (SELECT a AS x FROM inner_t WHERE a > 0) sub",
            ANSI,
        ),
        (
            "scalar-subquery-where",
            "SELECT a FROM t WHERE a > (SELECT max(b) FROM u WHERE u.k = t.k)",
            ANSI,
        ),
        (
            "in-subquery",
            "SELECT a FROM t WHERE a IN (SELECT id FROM u WHERE u.active AND u.n > 3)",
            ANSI,
        ),
        (
            "insert-fallback",
            "INSERT INTO t (a, b) VALUES (1, 2), (3, 4)",
            ANSI,
        ),
        (
            "update-fallback",
            "UPDATE t SET a = 1, b = 2 WHERE c = 3",
            ANSI,
        ),
        (
            "create-table-fallback",
            "CREATE TABLE t (a INT NOT NULL, b VARCHAR(10), PRIMARY KEY (a))",
            ANSI,
        ),
        ("values-body-fallback", "VALUES (1, 2), (3, 4)", ANSI),
    ];
    insta::assert_snapshot!(
        "inventory_fragment_fallback_no_deep_relayout",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// FIXED: comment anchored inside a canonical fragment -> hoisted adjacent.
// ===========================================================================

#[test]
fn inventory_comment_relocation_fragment() {
    // A comment whose anchor sits inside a *flat* canonical fragment (an operator, an
    // empty arg-list, or any fallback statement) has no structured position of its own.
    // It hoists adjacent to the fragment that flattened it (trailing) rather than
    // relocating to the output tail; exact interior placement stays out of reach for
    // the flat fragment path. A comment inside a *re-laid-out subquery* is the
    // structured exception: the recursion gives it a real position, so the
    // inside-subquery fixture below records exact interior placement.
    let fixtures = &[
        ("operator-crossing", "SELECT a + /* mid */ b FROM t", ANSI),
        (
            "empty-parens-dangling",
            "SELECT count(/* why */) FROM t",
            ANSI,
        ),
        (
            "inside-subquery-fragment",
            "SELECT x FROM (SELECT a /* inner */ FROM u) s",
            ANSI,
        ),
        (
            "inside-insert-fallback",
            "INSERT INTO t (a) VALUES (1) /* tail */",
            ANSI,
        ),
        (
            "inside-where-predicate-expr",
            "SELECT a FROM t WHERE b = /* mid */ 2",
            ANSI,
        ),
    ];
    insta::assert_snapshot!(
        "inventory_comment_relocation_fragment",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// FIXED: statement-boundary comment -> holds its boundary position.
// ===========================================================================

#[test]
fn inventory_comment_relocation_statement_boundary() {
    // A comment anchored to a whole statement (before the first statement, between
    // statements, or trailing after a statement's last token) now holds its boundary:
    // `attach_top_level` anchors it to the same deepest-wins id the renderer's
    // span-keyed lookup resolves to, so the statement's leading/trailing lookup finds it
    // instead of the no-drop net relocating it to the tail. Leading stays at the head,
    // a between-statement comment stays between, a trailing comment stays with its
    // statement.
    let fixtures = &[
        (
            "leading-before-statement",
            "-- header\nSELECT a FROM t WHERE a = 1",
            ANSI,
        ),
        (
            "between-statements",
            "SELECT 1;\n-- divider\nSELECT 2",
            ANSI,
        ),
        (
            "trailing-after-last-token",
            "SELECT a FROM t WHERE a = 1 -- filter\n",
            ANSI,
        ),
        (
            "stmt-final-line-comment",
            "SELECT a FROM t\n-- trailing note",
            ANSI,
        ),
        ("stmt-final-after-semicolon", "SELECT 1;\n-- after", ANSI),
    ];
    insta::assert_snapshot!(
        "inventory_comment_relocation_statement_boundary",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// FIXED: comma-side comment placement.
// ===========================================================================

#[test]
fn inventory_comment_comma_side_reordering() {
    // A block comment written after a list item and before its separating comma now
    // renders on the item's side of the comma (`a /* on a */, b`). A line comment
    // written before the comma still routes after it, because placing a line comment
    // before a same-line comma would swallow the separator — the no-comment-out-the-
    // separator invariant. The leading-side case (`, /* on b */ b`) was already correct.
    let fixtures = &[
        (
            "comma-leading-side",
            "SELECT a\n, /* on b */ b FROM t",
            ANSI,
        ),
        (
            "comma-trailing-side",
            "SELECT a /* on a */\n, b FROM t",
            ANSI,
        ),
    ];
    insta::assert_snapshot!(
        "inventory_comment_comma_side_reordering",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// SUPPORTED (formerly PREVIEW): UESCAPE / unicode-identifier surface-spelling fidelity.
// ===========================================================================

#[test]
fn inventory_spelling_uescape_limitation() {
    // The formatter's fragments ride the canonical renderer, so any spelling the
    // canonical path round-trips is preserved here too. Both the `U&'...'` string-literal
    // form (with or without an explicit UESCAPE) and the `U&"..."` unicode-identifier form
    // now round-trip verbatim — the identifier keeps its `U&` prefix and UESCAPE clause via
    // `QuoteStyle::UnicodeDouble`'s source-spelling replay.
    let fixtures = &[
        (
            "uescape-string-custom-escape",
            r"SELECT U&'d!0061t!0061' UESCAPE '!' FROM t",
            PG,
        ),
        (
            "uescape-string-default-escape",
            r"SELECT U&'d\0061t\0061' FROM t",
            PG,
        ),
        (
            "uescape-unicode-identifier",
            r#"SELECT U&"d!0061ta" UESCAPE '!' FROM t"#,
            PG,
        ),
    ];
    insta::assert_snapshot!(
        "inventory_spelling_uescape_limitation",
        inventory(fixtures, &FormatOptions::default())
    );
}

// ===========================================================================
// Focused characterization assertions: pin the boundary-defining behaviours the
// v1 stable-vs-preview split rests on (these are the anchors a future fix inverts).
// ===========================================================================

/// A comment inside an expression fragment now hoists adjacent to the fragment that
/// flattened it, instead of relocating to the statement tail.
/// (Owner: formatter-comment-fragment-relocation.)
#[test]
fn operator_crossing_comment_renders_adjacent() {
    let out = format_sql(
        "SELECT a + /* c */ b FROM t",
        ANSI,
        &FormatOptions::default(),
    )
    .unwrap();
    // The fragment path cannot re-inject the comment at its exact interior column, so
    // it renders trailing the projection item — adjacent, not relocated to the tail.
    assert!(
        out.contains("SELECT a + b /* c */"),
        "comment must render adjacent to its fragment:\n{out}"
    );
    assert!(
        !out.trim_end().ends_with("/* c */"),
        "comment must not relocate to the tail:\n{out}"
    );
}

/// A leading comment before a statement is kept at the head, not relocated to the tail.
/// (Owner: formatter-statement-boundary-comment-anchoring.)
#[test]
fn statement_boundary_leading_comment_renders_at_head() {
    let out = format_sql(
        "-- header\nSELECT a FROM t",
        ANSI,
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(
        out.starts_with("-- header"),
        "leading comment must stay at the head:\n{out}"
    );
    assert!(
        !out.trim_end().ends_with("-- header"),
        "leading comment must not relocate to the tail:\n{out}"
    );
}

/// A compound subquery in FROM is re-laid out across lines (clause-per-line, indented),
/// not embedded as a single-line fragment. (Owner: formatter-structured-subquery-relayout.)
#[test]
fn derived_subquery_is_relaid_out_multiline() {
    let out = format_sql(
        "SELECT x FROM (SELECT a AS x FROM u WHERE a > 0) sub",
        ANSI,
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(
        out.contains("SELECT a AS x\n"),
        "the derived subquery's clauses must each land on their own line:\n{out}"
    );
    // The alias rides the closing paren line, and the inner clauses are indented one
    // level under the derived-table parentheses.
    assert!(
        out.contains("  ) sub"),
        "the derived table's alias must trail the re-laid-out subquery:\n{out}"
    );
    assert!(
        !out.contains("(SELECT a AS x FROM u WHERE a > 0)"),
        "the subquery must not stay a single-line fragment:\n{out}"
    );
}

/// A trivial single-clause subquery stays inline as a fragment — the re-layout
/// threshold only structures a subquery whose layout spans more than one line.
/// (Owner: formatter-structured-subquery-relayout.)
#[test]
fn trivial_subquery_stays_inline() {
    let out = format_sql(
        "SELECT a FROM t WHERE a > (SELECT 1)",
        ANSI,
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(
        out.contains("WHERE a > (SELECT 1)"),
        "a single-clause subquery must stay inline, not explode across lines:\n{out}"
    );
}

/// A comment interior to a re-laid-out subquery now lands at its exact structured
/// position (trailing the projection item it followed), instead of hoisting adjacent to
/// the whole flattened fragment — the comment-placement unlock this ticket delivers.
/// (Owner: formatter-structured-subquery-relayout / formatter-comment-fragment-relocation.)
#[test]
fn comment_inside_relaid_out_subquery_places_exactly() {
    let out = format_sql(
        "SELECT x FROM (SELECT a /* pick */ FROM u WHERE a > 0) s",
        ANSI,
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(
        out.contains("SELECT a /* pick */\n"),
        "the interior comment must render at its exact position inside the subquery:\n{out}"
    );
    assert!(
        !out.trim_end().ends_with("/* pick */"),
        "the comment must not hoist to the fragment tail / statement tail:\n{out}"
    );
}

/// A `U&'...'` string literal (with an explicit UESCAPE) round-trips verbatim.
#[test]
fn uescape_string_literal_round_trips() {
    let out = format_sql(
        r"SELECT U&'d!0061t!0061' UESCAPE '!' FROM t",
        PG,
        &FormatOptions::default(),
    )
    .unwrap();
    assert!(
        out.contains(r"U&'d!0061t!0061' UESCAPE '!'"),
        "the UESCAPE string literal must round-trip verbatim:\n{out}"
    );
}

/// A `U&"..."` unicode *identifier* now round-trips verbatim — both the default-escape
/// (`\XXXX` / `\+XXXXXX`) form and an explicit `UESCAPE 'c'` override keep the `U&` prefix
/// and the whole UESCAPE clause through the formatter (which rides the canonical renderer).
#[test]
fn uescape_unicode_identifier_round_trips() {
    for src in [
        r#"SELECT U&"d\0061t\+000061" FROM t"#,
        r#"SELECT U&"d!0061ta" UESCAPE '!' FROM t"#,
    ] {
        let out = format_sql(src, PG, &FormatOptions::default()).unwrap();
        assert!(
            out.contains(&src["SELECT ".len()..src.len() - " FROM t".len()]),
            "the UESCAPE unicode identifier must round-trip verbatim:\ninput:  {src}\noutput: {out}"
        );
    }
}
