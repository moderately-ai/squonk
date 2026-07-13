// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Pretty-printing SQL formatter (feature `document-render`).
//!
//! A layout-IR renderer that formats *from the validated AST*, not by re-tokenizing
//! and relaying out source text. That is the differentiator (ticket
//! `render-pretty-print-layout-ir`): the output is guaranteed to re-parse, layout is
//! dialect-aware, and it composes with the crate's other render modes. It is built
//! from four pieces:
//!
//! - [`doc`](crate::format::doc) — a Wadler/Prettier `Doc` combinator IR (`text` / `line` / `softline` /
//!   `group` / `nest`) plus a width-aware layouter.
//! - [`comments`](crate::format::comments) — the trivia -> node [`CommentAttachments`](crate::format::comments::CommentAttachments) pass, so comments
//!   captured out of band survive a format (leading / trailing / dangling), anchored
//!   with the parser's opt-in clause marks.
//! - [`keyword_case`](crate::format::keyword_case) — the `keywordCase` post-pass (`upper` / `lower` / `preserve`).
//! - `render` — the pretty render path that emits the `Doc` tree, structuring the
//!   statement list, `WITH`/CTEs, set operations, and `SELECT` clauses while
//!   delegating expressions and exotic constructs to the canonical renderer.
//!
//! # Example
//!
//! ```
//! use squonk::BuiltinDialect;
//! use squonk::format::{format_sql, FormatOptions};
//!
//! let sql = "select a,b from t where a=1 and b=2";
//! let out = format_sql(sql, BuiltinDialect::Ansi, &FormatOptions::default()).unwrap();
//! assert!(out.contains("SELECT"));
//! ```
//!
//! # v1 support tier: stable surface vs preview
//!
//! The formatter ships as a **documented preview**, not a full-fidelity formatter.
//! One fixture per family in the `format::coverage` test module measures the exact
//! current behaviour of everything below; that inventory is the source of truth and any
//! future fix lands as a reviewable snapshot diff.
//!
//! ## Stable in v1 (guaranteed)
//!
//! - **Parse-back.** Formatted output always re-parses to a structurally equal tree —
//!   unrecognised constructs fall back to the total canonical renderer, so nothing is
//!   dropped or reshaped.
//! - **Spelling fidelity.** Every `PreserveSource`-tagged spelling round-trips (types,
//!   literals — including `U&'…' UESCAPE '…'` string literals — and quote styles,
//!   including the `U&"…" [UESCAPE '…']` Unicode-escaped *identifier* form):
//!   fragments are canonical renders, so layout only moves whitespace.
//! - **Minimal-paren normalization.** The formatter emits the minimal parentheses the
//!   binding-power table requires (the AST records no parenthesization; the structural
//!   oracle proves semantic safety; prettier precedent).
//! - **Structured layout** of the statement list, `WITH`/CTEs, set operations, and the
//!   `SELECT` clause skeleton (`SELECT`/`DISTINCT`, `FROM`, `WHERE`, `GROUP BY`,
//!   `HAVING`, `ORDER BY`, `LIMIT`) — one clause per line, width-aware list breaking.
//! - **Recursive subquery re-layout.** A subquery in a structured position — a `FROM`
//!   derived table, a scalar / `IN` / `EXISTS` / quantified predicate reachable in
//!   `WHERE`/`HAVING` or the projection, and a CTE body — is re-laid out with the same
//!   clause-per-line treatment, indented one level. A subquery whose structured layout
//!   would still be a single line (`(SELECT 1)`) stays inline; one with an exotic body
//!   falls back to a flat fragment (the completeness net). The threshold is structural
//!   (does the layout carry a forced break?), not a size heuristic — see
//!   `format::render`.
//! - **Style knobs:** indent width, max line length, keyword case
//!   (`upper` / `lower` / `preserve`).
//! - **Position-stable comments.** Every captured comment renders at a stable position
//!   near its source, and formatting is idempotent (`format(format(x)) == format(x)`):
//!   - an end-of-line comment trailing a non-final list item stays with that item;
//!   - a comment before a clause keyword anchors before the keyword (parser clause marks);
//!   - a statement-boundary comment (leading a statement, between statements, or
//!     trailing the last one) holds its boundary;
//!   - a comment inside a re-laid-out subquery renders at its exact structured
//!     position (e.g. trailing the projection item it followed);
//!   - a comment anchored inside a *flattened* fragment (across an operator, in empty
//!     parens, inside an inline-kept subquery, or inside a fallback statement) hoists
//!     adjacent to the fragment that owns it rather than relocating to the output tail;
//!   - a list-item comment written before its separating comma renders before the comma.
//!
//! ## Preview in v1 (NOT full-fidelity; each limitation has an owner ticket)
//!
//! Measured against `format::coverage`. The pretty path never drops a comment (the
//! no-drop safety net re-emits any it cannot place) and no longer relocates one to the
//! tail; the remaining gap is layout-depth:
//!
//! 1. **Deep expression re-layout** — nested *expressions* (long arithmetic / boolean
//!    chains, function arguments, `CASE` arms) are embedded as single-line canonical
//!    fragments, never re-laid out; this includes a subquery buried in a position the
//!    re-layout pass does not reach (a function argument, a `CASE` arm, an `ORDER
//!    BY`/`GROUP BY` item). A comment buried inside such a *flat* fragment renders
//!    *adjacent* to the fragment (trailing it), not re-injected at its exact interior
//!    column — adjacency is the fidelity ceiling for the unstructured fragment path,
//!    while a comment inside a re-laid-out subquery now places exactly. Owner ticket
//!    `formatter-structured-subquery-relayout` (subquery re-layout itself shipped
//!    there; the expression-depth remainder stays with it).
//!
//! Comments are rendered from the out-of-band attachment table and never mutate the
//! AST, so structural equality is preserved regardless of placement.

pub mod comments;
pub mod doc;
pub mod keyword_case;
mod render;

pub use comments::{CommentAttachments, NodeComments};
pub use keyword_case::KeywordCase;

use crate::ast::NoExt;
use crate::ast::SourceStore;
use crate::dialect::BuiltinDialect;
use crate::error::ParseError;
use crate::parser::{ParseConfig, Parsed};

/// The v1 style surface. Deliberately small (ticket point 4): indent width, max line
/// length, keyword case — explicitly **not** sql-formatter's full option catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatOptions {
    /// Spaces per indent level. Default 2.
    pub indent_width: usize,
    /// The target line width groups try to fit within before breaking. Default 80.
    pub max_line_length: usize,
    /// How keyword tokens are cased. Default [`KeywordCase::Upper`].
    pub keyword_case: KeywordCase,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            indent_width: 2,
            max_line_length: 80,
            keyword_case: KeywordCase::Upper,
        }
    }
}

impl FormatOptions {
    /// Set the indent width (spaces per level).
    pub fn with_indent_width(mut self, width: usize) -> Self {
        self.indent_width = width;
        self
    }

    /// Set the target line width.
    pub fn with_max_line_length(mut self, width: usize) -> Self {
        self.max_line_length = width;
        self
    }

    /// Set the keyword-casing policy.
    pub fn with_keyword_case(mut self, case: KeywordCase) -> Self {
        self.keyword_case = case;
        self
    }
}

/// Format an already-parsed tree, applying `dialect` for the keyword-casing token
/// pass and for re-tokenizing fragments during subquery re-layout — pass the dialect
/// the tree was parsed under. Comments are formatted only when the parse captured
/// trivia ([`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia)).
///
/// The tree must be the standard (`NoExt`) AST; the layouter reuses the canonical
/// renderer for expression fragments, so any spelling the canonical path preserves is
/// preserved here.
pub fn format_parsed<S: SourceStore>(
    parsed: &Parsed<S, NoExt>,
    dialect: BuiltinDialect,
    options: &FormatOptions,
) -> String {
    let attachments = CommentAttachments::compute(parsed);
    let mut renderer =
        render::PrettyRenderer::new(parsed, &attachments, options.indent_width, dialect);
    let doc = renderer.document(parsed.statements());
    let laid_out = doc::layout(&doc, options.max_line_length);
    keyword_case::apply(laid_out, parsed.source(), dialect, options.keyword_case)
}

/// Parse `sql` under a runtime-selected built-in `dialect` (capturing trivia so
/// comments format) and pretty-print it. The one-shot string-to-string convenience
/// the language bindings wrap.
pub fn format_sql(
    sql: &str,
    dialect: BuiltinDialect,
    options: &FormatOptions,
) -> Result<String, ParseError> {
    let parsed = crate::parse_builtin_with(sql, ParseConfig::new(dialect).capture_trivia(true))?;
    Ok(format_parsed(&parsed, dialect, options))
}

#[cfg(test)]
mod coverage;
#[cfg(test)]
mod tests;
