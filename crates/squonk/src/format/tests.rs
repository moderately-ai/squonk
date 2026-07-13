// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Integration tests for the pretty formatter: the parse-back oracle (structural
//! equality of `parse(format(x))` vs `parse(x)`), spelling fidelity, comment
//! survival, and the v1 style surface.

use super::{FormatOptions, KeywordCase, format_parsed, format_sql};
use crate::tokenizer::TokenKind;
use crate::{BuiltinDialect, ParseConfig, parse_builtin_with, tokenize_with_builtin};

const D: BuiltinDialect = BuiltinDialect::Ansi;

fn fmt(sql: &str) -> String {
    format_sql(sql, D, &FormatOptions::default()).expect("formats")
}

/// A representative corpus slice: every one must survive the parse-back oracle. Mixes
/// structured shapes (SELECT clauses, joins, CTEs, set ops, subqueries) with fallback
/// shapes (INSERT/UPDATE/DELETE/DDL) so both code paths are covered.
const CORPUS: &[&str] = &[
    "SELECT 1",
    "SELECT a, b, c FROM t",
    "SELECT DISTINCT a FROM t",
    "SELECT ALL a FROM t",
    "SELECT a FROM t WHERE a = 1 AND b > 2",
    "SELECT a, count(*) FROM t GROUP BY a HAVING count(*) > 1",
    "SELECT a FROM t ORDER BY a DESC, b ASC",
    "SELECT a FROM t LIMIT 10",
    "SELECT a FROM t WHERE a IN (1, 2, 3) ORDER BY a LIMIT 5",
    "SELECT a FROM t1 JOIN t2 ON t1.id = t2.id WHERE t1.x > 0",
    "SELECT a FROM t1 LEFT JOIN t2 ON t1.id = t2.id",
    "SELECT a, b FROM (SELECT a, b FROM inner_t WHERE a > 0) sub",
    "SELECT a FROM t WHERE a IN (SELECT id FROM u WHERE u.active)",
    "SELECT a FROM t WHERE EXISTS (SELECT 1 FROM u WHERE u.k = t.k)",
    "SELECT a FROM t WHERE NOT EXISTS (SELECT 1 FROM u WHERE u.k = t.k)",
    "SELECT a, (SELECT max(b) FROM u WHERE u.k = t.k) AS mx FROM t",
    "SELECT a FROM t WHERE a > (SELECT 1)",
    "SELECT x FROM (SELECT a AS x FROM u WHERE a IN (SELECT id FROM v WHERE v.ok)) s",
    "SELECT a FROM t JOIN (SELECT k, max(b) AS mb FROM u GROUP BY k) m ON m.k = t.k",
    // A string literal byte-identical to the subquery, earlier in the same predicate
    // fragment: the re-layout splice must skip the literal-interior match and structure
    // the real subquery (token-boundary verification in `relayout_fragment`).
    "SELECT a FROM t WHERE name = '(SELECT b FROM u WHERE b > 0)' AND a IN (SELECT b FROM u WHERE b > 0)",
    "SELECT a FROM t1 UNION SELECT a FROM t2",
    "SELECT a FROM t1 UNION ALL SELECT a FROM t2 INTERSECT SELECT a FROM t3",
    "WITH cte AS (SELECT a FROM t) SELECT a FROM cte",
    "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a, b",
    "SELECT CAST(a AS INT) FROM t",
    "SELECT a + b * c - d FROM t",
    "SELECT a FROM t WHERE a IS NOT NULL AND b IS NULL",
    "SELECT CASE WHEN a > 0 THEN 'pos' ELSE 'neg' END FROM t",
    "SELECT \"Quoted\", a FROM \"Table\"",
    "INSERT INTO t (a, b) VALUES (1, 2)",
    "UPDATE t SET a = 1 WHERE b = 2",
    "DELETE FROM t WHERE a = 1",
    "CREATE TABLE t (a INT, b VARCHAR(10))",
    "SELECT a FROM t; SELECT b FROM u",
];

fn parse_stmts(sql: &str) -> Vec<crate::ast::Statement> {
    parse_builtin_with(sql, ParseConfig::new(D))
        .expect("parses")
        .into_statements()
}

/// The parse-back oracle: formatting is a pure layout change, so re-parsing the
/// formatted output yields a *structurally equal* tree. `Meta`'s `PartialEq` is
/// always-true, so `Statement` equality compares shape and spelling while ignoring
/// spans and node ids — exactly the structural-oracle notion of equality.
#[test]
fn format_output_parses_back_structurally_equal() {
    for &sql in CORPUS {
        let before = parse_stmts(sql);
        let formatted = fmt(sql);
        let after = parse_stmts(&formatted);
        assert_eq!(
            before, after,
            "structural drift for {sql:?}\n--- formatted ---\n{formatted}\n---"
        );
    }
}

/// The formatted output must always re-parse without error (a superset guard over the
/// structural check, and the property the differentiator rests on).
#[test]
fn format_output_always_reparses() {
    for &sql in CORPUS {
        let formatted = fmt(sql);
        assert!(
            parse_builtin_with(&formatted, ParseConfig::new(D)).is_ok(),
            "formatted output failed to reparse for {sql:?}:\n{formatted}"
        );
    }
}

/// The "significant" (spelling-bearing) token texts: identifiers, numbers, strings,
/// quoted identifiers. These carry `PreserveSource` spelling and must round-trip
/// byte-for-byte through a format (which only rearranges whitespace).
fn spelling_tokens(sql: &str) -> Vec<String> {
    tokenize_with_builtin(sql, D)
        .expect("tokenizes")
        .into_iter()
        .filter(|t| {
            matches!(
                t.kind,
                TokenKind::Word | TokenKind::Number | TokenKind::String | TokenKind::QuotedIdent
            )
        })
        .map(|t| sql[t.span.start() as usize..t.span.end() as usize].to_owned())
        .collect()
}

/// Spelling fidelity: identifier / literal / quote-style spellings survive a format
/// unchanged and in order (layout moves only whitespace). Keyword case is a separate,
/// deliberate knob, so keyword tokens are excluded here.
#[test]
fn format_preserves_spelling_tokens() {
    for &sql in CORPUS {
        let formatted = fmt(sql);
        assert_eq!(
            spelling_tokens(sql),
            spelling_tokens(&formatted),
            "spelling drift for {sql:?}\n{formatted}"
        );
    }
}

#[test]
fn simple_select_lays_out_one_clause_per_line() {
    let out = fmt("select a, b from t where a = 1 group by a");
    assert_eq!(out, "SELECT a, b\nFROM t\nWHERE a = 1\nGROUP BY a");
}

#[test]
fn wide_projection_breaks_one_item_per_line() {
    let sql = "SELECT alpha, bravo, charlie, delta, echo, foxtrot, golf, hotel, india FROM t";
    let out = format_sql(sql, D, &FormatOptions::default().with_max_line_length(40)).unwrap();
    assert!(out.contains("SELECT\n  alpha,\n  bravo,"), "got:\n{out}");
}

#[test]
fn indent_width_is_honoured() {
    let sql = "SELECT alpha, bravo, charlie, delta, echo, foxtrot, golf, hotel FROM t";
    let opts = FormatOptions::default()
        .with_max_line_length(30)
        .with_indent_width(4);
    let out = format_sql(sql, D, &opts).unwrap();
    assert!(out.contains("SELECT\n    alpha,"), "got:\n{out}");
}

#[test]
fn keyword_case_lower_recases_all_keywords() {
    let opts = FormatOptions::default().with_keyword_case(KeywordCase::Lower);
    let out = format_sql("SELECT a FROM t WHERE a IS NOT NULL", D, &opts).unwrap();
    assert_eq!(out, "select a\nfrom t\nwhere a is not null");
}

#[test]
fn keyword_case_preserve_follows_dominant_source_case() {
    let opts = FormatOptions::default().with_keyword_case(KeywordCase::Preserve);
    // Dominant source case is lower -> keywords render lower.
    let out = format_sql("select a from t where a = 1", D, &opts).unwrap();
    assert!(out.starts_with("select a"), "got:\n{out}");
    assert!(out.contains("from t"));
}

/// Comments captured by the parse survive the format (none dropped) and the output
/// still parses.
#[test]
fn comments_survive_the_format() {
    let cases = [
        "-- leading\nSELECT 1",
        "SELECT a -- trailing\nFROM t",
        "SELECT a FROM t WHERE a = 1\n-- before group\nGROUP BY a",
        "SELECT count(/* inside */) FROM t",
        "SELECT /* block */ a FROM t",
    ];
    for sql in cases {
        let parsed =
            parse_builtin_with(sql, ParseConfig::new(D).capture_trivia(true)).expect("parses");
        let comment_count = parsed
            .trivia()
            .iter()
            .filter(|t| {
                matches!(
                    t.kind(),
                    crate::tokenizer::TriviaKind::LineComment
                        | crate::tokenizer::TriviaKind::BlockComment
                )
            })
            .count();
        let out = format_parsed(&parsed, D, &FormatOptions::default());
        let out_comments = tokenize_out_comment_count(&out);
        assert_eq!(
            comment_count, out_comments,
            "comment dropped for {sql:?}\n--- out ---\n{out}"
        );
        assert!(
            parse_builtin_with(&out, ParseConfig::new(D)).is_ok(),
            "output with comments failed to reparse for {sql:?}:\n{out}"
        );
    }
}

/// Count comment runs in formatted output by re-tokenizing with trivia capture.
fn tokenize_out_comment_count(sql: &str) -> usize {
    let (_tokens, trivia) = crate::tokenize_with_builtin_trivia(sql, D).expect("output tokenizes");
    trivia
        .all()
        .iter()
        .filter(|t| {
            matches!(
                t.kind(),
                crate::tokenizer::TriviaKind::LineComment
                    | crate::tokenizer::TriviaKind::BlockComment
            )
        })
        .count()
}

#[test]
fn comment_before_group_by_renders_before_the_keyword() {
    let sql = "SELECT a FROM t WHERE a = 1\n-- note\nGROUP BY a";
    let parsed = parse_builtin_with(sql, ParseConfig::new(D).capture_trivia(true)).expect("parses");
    let out = format_parsed(&parsed, D, &FormatOptions::default());
    // The comment must appear on its own line *before* GROUP BY, never after it.
    let note = out.find("-- note").expect("comment present");
    let group = out.find("GROUP BY").expect("group by present");
    assert!(note < group, "comment should precede GROUP BY:\n{out}");
}

#[test]
fn empty_input_formats_to_empty() {
    assert_eq!(fmt(""), "");
}

/// Comment fixtures spanning all three anchoring defect classes: statement-boundary
/// (leading / between / trailing), fragment-interior (operator / empty parens /
/// subquery / WHERE predicate / fallback statement), and comma-adjacent (both sides),
/// with line and block variants. Each is fed through `fmt` (which captures trivia).
const COMMENT_CORPUS: &[&str] = &[
    // Statement-boundary.
    "-- header\nSELECT a FROM t WHERE a = 1",
    "SELECT 1;\n-- divider\nSELECT 2",
    "SELECT a FROM t WHERE a = 1 -- filter\n",
    "SELECT a FROM t\n-- trailing note",
    "SELECT 1;\n-- after",
    "/* head */ SELECT a FROM t",
    // Fragment-interior.
    "SELECT a + /* mid */ b FROM t",
    "SELECT count(/* why */) FROM t",
    "SELECT x FROM (SELECT a /* inner */ FROM u) s",
    "INSERT INTO t (a) VALUES (1) /* tail */",
    "SELECT a FROM t WHERE b = /* mid */ 2",
    "SELECT a FROM t WHERE b = 2 AND c /* z */ IN (1, 2)",
    // Comma-adjacent, both sides, line and block.
    "SELECT a /* on a */\n, b FROM t",
    "SELECT a\n, /* on b */ b FROM t",
    "SELECT a, -- keep with a\nb FROM t",
    "SELECT a, b FROM t GROUP BY a, -- keep with a\nb",
    // Interior of a re-laid-out subquery (block and line variants, derived / IN /
    // EXISTS positions): the recursion gives these a real structured position.
    "SELECT x FROM (SELECT a /* pick */ FROM u WHERE a > 0) s",
    "SELECT x FROM (SELECT a, -- keep with a\nb FROM u WHERE a > 0) s",
    "SELECT a FROM t WHERE a IN (SELECT id /* only ids */ FROM u WHERE u.active)",
    "SELECT a FROM t WHERE EXISTS (SELECT 1 FROM u -- probe\nWHERE u.k = t.k)",
    // Interior of a subquery kept inline by the threshold: hoists adjacent.
    "SELECT a FROM t WHERE a > (SELECT 1 /* floor */)",
];

/// Formatting is idempotent: the formatted output is a fixed point, so
/// `format(format(x)) == format(x)` byte-for-byte. This is the load-bearing guarantee
/// for the comment-anchoring model — a comment that renders in a stable position on the
/// first pass must not drift on the second.
#[test]
fn comment_formatting_is_byte_stable_idempotent() {
    for &sql in COMMENT_CORPUS {
        let once = fmt(sql);
        let twice = fmt(&once);
        assert_eq!(
            once, twice,
            "format is not idempotent for {sql:?}\n--- once ---\n{once}\n--- twice ---\n{twice}\n---"
        );
    }
}

/// Byte-stable idempotence over the layout corpus too: re-laid-out subqueries (and
/// every other structured or fallback shape) must be a fixed point of the formatter —
/// a multi-line subquery re-parsed from formatted output re-lays out identically.
#[test]
fn corpus_formatting_is_byte_stable_idempotent() {
    for &sql in CORPUS {
        let once = fmt(sql);
        let twice = fmt(&once);
        assert_eq!(
            once, twice,
            "format is not idempotent for {sql:?}\n--- once ---\n{once}\n--- twice ---\n{twice}\n---"
        );
    }
}

/// Every comment in the corpus survives the format (none dropped) and the output still
/// re-parses — the no-drop and parse-back invariants hold across all three anchoring
/// defect classes now that comments render at structured positions rather than the tail.
#[test]
fn comment_corpus_never_drops_and_reparses() {
    for &sql in COMMENT_CORPUS {
        let parsed =
            parse_builtin_with(sql, ParseConfig::new(D).capture_trivia(true)).expect("parses");
        let comment_count = parsed
            .trivia()
            .iter()
            .filter(|t| {
                matches!(
                    t.kind(),
                    crate::tokenizer::TriviaKind::LineComment
                        | crate::tokenizer::TriviaKind::BlockComment
                )
            })
            .count();
        let out = format_parsed(&parsed, D, &FormatOptions::default());
        assert_eq!(
            comment_count,
            tokenize_out_comment_count(&out),
            "comment dropped for {sql:?}\n--- out ---\n{out}"
        );
        assert!(
            parse_builtin_with(&out, ParseConfig::new(D)).is_ok(),
            "output failed to reparse for {sql:?}:\n{out}"
        );
    }
}

#[test]
fn statement_boundary_comments_hold_position() {
    // Leading stays at the head, a divider stays between the statements, a trailing
    // comment stays with the statement it follows — none relocate to the tail.
    assert!(fmt("-- header\nSELECT a FROM t").starts_with("-- header\nSELECT"));
    let between = fmt("SELECT 1;\n-- divider\nSELECT 2");
    assert!(
        between.contains("-- divider\nSELECT 2"),
        "divider must precede the second statement:\n{between}"
    );
    assert!(
        fmt("SELECT a FROM t -- tail\n").contains("FROM t -- tail"),
        "trailing comment stays with the statement"
    );
}

#[test]
fn fragment_interior_comment_hoists_adjacent() {
    assert!(fmt("SELECT a + /* c */ b FROM t").contains("SELECT a + b /* c */"));
    assert!(fmt("SELECT count(/* c */) FROM t").contains("count() /* c */"));
    assert!(fmt("SELECT a FROM t WHERE b = /* c */ 2").contains("WHERE b = 2 /* c */"));
}

#[test]
fn item_trailing_comment_renders_before_the_comma() {
    // A block comment written after the item and before the comma stays on the item's
    // side of the separator.
    assert_eq!(
        fmt("SELECT a /* on a */\n, b FROM t"),
        "SELECT a /* on a */, b\nFROM t"
    );
}
