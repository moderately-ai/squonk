// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The ADR-0005 `logos` reference lexer, shared by the two comparison harnesses
//! and the parity/evidence test:
//!
//! - `tokenizer_logos.rs`      — codspeed-criterion wall-clock, cursor vs logos.
//! - `tokenizer_logos_heap.rs` — dhat transient + peak heap, cursor vs logos.
//! - `tests/logos_reference.rs` — asserts token-count parity on regular SQL and
//!   demonstrates the non-regular cases logos cannot express (via `#[path]`, the
//!   same cross-target include `tests/upstream_gate.rs` uses for `upstream/`).
//!
//! Why this exists. ADR-0005 chose a hand-written byte cursor over `logos` and
//! kept `logos` only as a perf reference (ADR-0017: a rejected dep stays a
//! *measured* alternative, never a production dep — like phf/lasso/text-size). The
//! gap this fills is a concrete measurement: on the REGULAR token subset a DFA can
//! express, is the cursor actually competitive on throughput and allocation? If it
//! is, the cursor's extra expressiveness (below) is free; if it is not, the trade
//! is at least quantified.
//!
//! What logos covers here — the regular subset only: words/keywords, numbers
//! (int/float/leading-dot/exponent), single-quoted strings and double-quoted
//! identifiers (with doubled-delimiter escapes), the operator and punctuation
//! bytes, `--` line comments, and *non-nested* `/* … */` block comments.
//!
//! What logos CANNOT express, and why it stays owned by the hand cursor:
//!
//! - PostgreSQL `$tag$ … $tag$` dollar quoting — the close delimiter must equal the (arbitrary, run-time) open tag, and a DFA has no memory of the tag it opened, so this is not a regular language.
//! - Nested `/* /* */ */` block comments — balanced nesting needs a depth counter that a regular grammar cannot keep, so the regex below stops at the first `*/` and leaks whatever follows; the production cursor (`skip_block_comment`) tracks depth and treats the whole nest as one skipped unit.
//!
//! These remain covered by the production tokenizer's own tests in `crates/squonk/src/tokenizer/mod.rs` (`dollar_quoting_is_a_single_string_token`, `nested_block_comment_is_one_skipped_unit`, `other_unterminated_constructs_report_their_kinds`); this crate adds no production dependency on logos.
//!
//! Each consumer uses a different slice of this module (the benches never call
//! `lex_tokens`; the test never builds the report), so module-level
//! `allow(dead_code)` keeps `-D warnings` green without scattering attributes —
//! the same convention `benches/upstream/mod.rs` uses.
#![allow(dead_code)]

use logos::Logos;
use squonk::ast::Span;
use squonk_ast::dialect::{Keyword, lookup_keyword};
use std::fmt::Write as _;
use std::hint::black_box;

/// The regular SQL token subset, as a `logos` DFA.
///
/// Operators and punctuation collapse into one variant each: the head-to-head
/// metric is *token boundaries* (count, span) and shared post-classification, not
/// the operator sub-kind, which the production cursor recovers from the span the
/// same way. Multi-byte operators (`<=`, `<>`, `||`, …) get their own `#[token]`
/// so logos's longest-match picks them over their one-byte prefixes, matching the
/// cursor's maximal munch.
#[derive(Logos, Clone, Copy, PartialEq, Eq, Debug)]
// Trivia, skipped exactly like the cursor's `skip_trivia`. Whitespace mirrors
// `u8::is_ascii_whitespace` (space, tab, LF, form-feed, CR — not vertical tab).
#[logos(skip r"[ \t\r\n\f]+")]
// `allow_greedy`: a line comment runs to end of line, so the unbounded
// to-EOL match is the intended behaviour, not the accidental `.*` logos warns about.
#[logos(skip("--[^\n]*", allow_greedy = true))]
// NON-nested block comment: matches up to the FIRST `*/`. This is the regular
// approximation; the cursor's nesting-aware version is the ADR-0005 point.
#[logos(skip r"/\*([^*]|\*+[^*/])*\*+/")]
pub enum LogosToken {
    /// An identifier or keyword; keyword classification happens in `tokenize_logos`
    /// via the shared `lookup_keyword`, exactly as the cursor does inline.
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Word,
    /// `42`, `3.14`, `1.`, `.5`, `1e10`, `2.5E-3` — the ANSI numeric forms (radix
    /// prefixes and `_` separators are dialect-gated, not part of this subset).
    #[regex(r"[0-9]+(\.[0-9]*)?([eE][+-]?[0-9]+)?")]
    #[regex(r"\.[0-9]+([eE][+-]?[0-9]+)?")]
    Number,
    /// A single-quoted string, doubled `''` as an in-literal escape.
    #[regex(r"'([^']|'')*'")]
    String,
    /// A double-quoted delimited identifier, doubled `""` as an escape.
    #[regex(r#""([^"]|"")*""#)]
    QuotedIdent,
    /// Any operator lexeme the cursor recognises (`+ - * / % = < > ! | & ^ ~` and
    /// the fixed two-byte forms).
    #[token("<=")]
    #[token(">=")]
    #[token("<>")]
    #[token("!=")]
    #[token("||")]
    #[token("&&")]
    #[token("+")]
    #[token("-")]
    #[token("*")]
    #[token("/")]
    #[token("%")]
    #[token("=")]
    #[token("<")]
    #[token(">")]
    #[token("!")]
    #[token("|")]
    #[token("&")]
    #[token("^")]
    #[token("~")]
    Operator,
    /// Structural punctuation (`( ) , ; . [ ] { }`).
    #[token("(")]
    #[token(")")]
    #[token(",")]
    #[token(";")]
    #[token(".")]
    #[token("[")]
    #[token("]")]
    #[token("{")]
    #[token("}")]
    Punctuation,
}

/// The classified kind stored per token, mirroring the production `TokenKind`
/// granularity for words/keywords so the keyword lookup is real work, not elided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LexedKind {
    Word,
    Keyword(Keyword),
    Number,
    String,
    QuotedIdent,
    Operator,
    Punctuation,
    /// A byte the regular subset cannot lex (e.g. a PostgreSQL `$`): logos yields
    /// `Err(())`, which the cursor would instead reject as a `LexError`.
    Error,
}

/// Tokenize `sql` with the logos reference, returning the token count.
///
/// Deliberately apples-to-apples with the production `tokenize` (ADR-0005): it
/// collects into a `Vec` pre-sized with the *same* `src.len() / 8 + 8` capacity
/// hint, records each token's `Span`, and runs the *same* `lookup_keyword` on
/// words — so the comparison isolates the scan engine (DFA vs hand cursor), not the
/// surrounding buffer-growth or keyword-recognition work.
pub fn tokenize_logos(sql: &str) -> usize {
    let mut lexer = LogosToken::lexer(sql);
    let mut tokens: Vec<(LexedKind, Span)> = Vec::with_capacity(sql.len() / 8 + 8);
    while let Some(result) = lexer.next() {
        let range = lexer.span();
        // Bench inputs are far under u32::MAX; the production tokenizer's spans are
        // u32 (ADR-0002), so the reference uses the same width.
        let span = Span::new(range.start as u32, range.end as u32);
        let kind = match result {
            Ok(LogosToken::Word) => match lookup_keyword(lexer.slice()) {
                Some(keyword) => LexedKind::Keyword(keyword),
                None => LexedKind::Word,
            },
            Ok(LogosToken::Number) => LexedKind::Number,
            Ok(LogosToken::String) => LexedKind::String,
            Ok(LogosToken::QuotedIdent) => LexedKind::QuotedIdent,
            Ok(LogosToken::Operator) => LexedKind::Operator,
            Ok(LogosToken::Punctuation) => LexedKind::Punctuation,
            Err(()) => LexedKind::Error,
        };
        tokens.push((kind, span));
    }
    // The token count alone does not depend on the keyword classification, so a
    // hot inliner could otherwise drop the `lookup_keyword` work and flatter logos
    // unfairly. Force the full classified buffer to be materialized first.
    black_box(&tokens);
    tokens.len()
}

/// The raw logos token stream (each `Ok`/`Err` with no post-classification), for
/// the non-regular evidence test. An `Err(())` marks a byte the regular subset
/// cannot lex.
pub fn lex_tokens(sql: &str) -> Vec<Result<LogosToken, ()>> {
    LogosToken::lexer(sql).collect()
}

/// The fixed context block both harnesses print, so a raw bench log is
/// self-describing: what is compared, what the ratio means, and which non-regular
/// cases logos cannot express (ADR-0005) — stated, never silent (ADR-0017).
pub fn report_header() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# tokenizer comparison: hand-written cursor vs logos reference"
    );
    let _ = writeln!(
        out,
        "#   ours   : squonk::tokenizer::tokenize (ANSI, ADR-0005)"
    );
    let _ = writeln!(
        out,
        "#   logos  : bench-only regular-subset DFA (benches/logos_ref)"
    );
    let _ = writeln!(
        out,
        "#   ratio  = ours / logos  (> 1.0 ⇒ the cursor is heavier/slower)"
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# both pre-size the token Vec identically and run the shared"
    );
    let _ = writeln!(
        out,
        "# lookup_keyword on words, so the comparison isolates the scan engine."
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# fairness caveat: the cursor is dialect-parameterized (a per-byte"
    );
    let _ = writeln!(
        out,
        "# feature-set/class-table dispatch, ADR-0011) whereas this reference is a"
    );
    let _ = writeln!(
        out,
        "# hardcoded ANSI DFA. Part of any throughput gap is the price of that"
    );
    let _ = writeln!(
        out,
        "# cross-dialect configurability and the open, extensible token set, not"
    );
    let _ = writeln!(
        out,
        "# pure scan-loop overhead — read the ratio through that."
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# logos covers the REGULAR subset only. It CANNOT express, so these stay"
    );
    let _ = writeln!(
        out,
        "# owned by the hand cursor and are NOT in this comparison (ADR-0005):"
    );
    let _ = writeln!(
        out,
        "#   - PostgreSQL $tag$…$tag$ dollar quoting (tag-matched close: not regular)"
    );
    let _ = writeln!(
        out,
        "#   - nested /* /* */ */ block comments (depth counting: not regular)"
    );
    let _ = writeln!(
        out,
        "# Production tests in crates/squonk/src/tokenizer/mod.rs still cover them."
    );
    out
}
