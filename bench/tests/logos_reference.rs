// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Guards that the ADR-0005 `logos` comparison stays meaningful, and pins the
//! evidence for *why* the non-regular cases stay owned by the hand-written cursor.
//!
//! Two things are asserted:
//! 1. Parity — on the regular-SQL corpus the logos reference and the production
//!    cursor agree on token boundaries (count). Without this the throughput /
//!    allocation ratios in `examples/compare_tokenizer_logos.rs` would compare different work.
//! 2. Expressiveness — concrete demonstrations that the regular subset CANNOT
//!    keep a PostgreSQL dollar quote whole, nor treat a nested block comment as
//!    one unit, while the cursor does. This is the measured backing for ADR-0005.
//!
//! The reference lexer is shared via `#[path]`, the same cross-target include
//! `tests/upstream_gate.rs` uses for `benches/upstream/`. logos is a dev-dep, so
//! no production dependency is introduced (ADR-0017).
//!
//! Quarantined from the default `cargo nextest run` — all four are `#[ignore]`d.
//! This is a pure dev-time comparison, not a correctness gate, and nextest
//! intermittently reports the binary as `leaky`: the tests are ~10ms of in-process
//! string tokenization that spawn no thread/child/socket, so the flag is a
//! `--leak-timeout` race under heavy parallel gate load, not a real
//! leaked resource — there is nothing to join or close. The production behaviours
//! these reference (dollar quoting, nested block comments) stay covered by the
//! tokenizer's own tests in `crates/squonk/src/tokenizer/mod.rs`. Run them on
//! demand with `cargo nextest run -p squonk-bench logos_reference --run-ignored all`.

#[path = "../benches/logos_ref/mod.rs"]
mod logos_ref;

use logos_ref::{LogosToken, lex_tokens, tokenize_logos};
use squonk::ast::dialect::FeatureSet;
use squonk::tokenizer::{TokenKind, tokenize, tokenize_with};
use squonk_bench::{TOKENIZER_CASES, tokenize_sql};

#[test]
#[ignore = "dev-only logos-vs-ours comparison (ADR-0005); excluded from the default gate — nextest intermittently reports it leaky under parallel load. Run with --run-ignored all."]
fn logos_reference_matches_cursor_token_count_on_regular_sql() {
    for case in TOKENIZER_CASES {
        assert_eq!(
            tokenize_logos(case.sql),
            tokenize_sql(case.sql),
            "logos and the cursor must agree on token count for `{}` \
             (else the benches compare different work)",
            case.name,
        );
    }
}

#[test]
#[ignore = "dev-only logos-vs-ours comparison (ADR-0005); excluded from the default gate — nextest intermittently reports it leaky under parallel load. Run with --run-ignored all."]
fn logos_handles_simple_line_and_block_comments_like_the_cursor() {
    // The regular comment forms ARE expressible: `--` to end of line and a single
    // (non-nested) `/* … */`. Both lexers skip them and yield `SELECT a FROM t`.
    let src = "SELECT a -- trailing\nFROM /* simple */ t";
    assert_eq!(tokenize_logos(src), tokenize_sql(src));
    assert_eq!(tokenize_sql(src), 4, "SELECT a FROM t");
}

#[test]
#[ignore = "dev-only logos-vs-ours comparison (ADR-0005); excluded from the default gate — nextest intermittently reports it leaky under parallel load. Run with --run-ignored all."]
fn logos_cannot_keep_a_postgres_dollar_quote_whole() {
    // The hand cursor lexes the whole `$tag$ … $tag$` as one String: the close
    // must equal the run-time open tag, which a DFA cannot remember (ADR-0005).
    let src = "$tag$body$tag$";
    let ours = tokenize_with(src, &FeatureSet::POSTGRES).expect("cursor lexes the dollar quote");
    assert_eq!(ours.len(), 1, "the cursor keeps the dollar quote whole");
    assert_eq!(ours[0].kind, TokenKind::String);

    // The regular subset has no rule for `$`, so logos errors on it and the tag /
    // body leak as separate words — it cannot express tag-matched quoting.
    let theirs = lex_tokens(src);
    assert!(
        theirs.len() > 1,
        "logos splits the dollar quote instead of keeping it whole: {theirs:?}",
    );
    assert!(
        theirs.iter().any(Result::is_err),
        "logos errors on the `$` it cannot lex: {theirs:?}",
    );
}

#[test]
#[ignore = "dev-only logos-vs-ours comparison (ADR-0005); excluded from the default gate — nextest intermittently reports it leaky under parallel load. Run with --run-ignored all."]
fn logos_cannot_treat_a_nested_block_comment_as_one_unit() {
    // The hand cursor counts `/*`/`*/` depth, so a fully nested comment is one
    // skipped unit and yields no tokens.
    let src = "/* a /* b */ c */";
    assert!(
        tokenize(src)
            .expect("cursor skips the nested comment")
            .is_empty(),
        "the cursor treats the whole nested comment as trivia",
    );

    // The regular block-comment rule stops at the FIRST `*/`, so ` c */` leaks:
    // depth counting is not a regular language (ADR-0005).
    let theirs = lex_tokens(src);
    assert!(
        !theirs.is_empty(),
        "logos leaks tokens after the inner close instead of skipping the nest",
    );
    assert!(
        theirs.iter().any(|t| matches!(t, Ok(LogosToken::Word))),
        "the `c` after the inner `*/` leaks as a word: {theirs:?}",
    );
}
