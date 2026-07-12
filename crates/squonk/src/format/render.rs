// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The pretty render path: walk the validated AST, emit a [`Doc`] layout tree.
//!
//! Layout-bearing constructs — the statement list, `WITH`/CTEs, set operations, and
//! `SELECT` with its clauses and list bodies — are structured into breakable groups.
//! Everything else (expressions, table refs, joins, exotic clauses, and non-`SELECT`
//! statements) is rendered through the **canonical** [`Render`](crate::ast::render::Render)
//! path as a single-line text fragment and embedded as [`Doc::Text`]. This has two
//! payoffs the design leans on:
//!
//! - **Completeness / parse-back safety.** Any construct the structured path does not
//!   recognize falls back to the canonical renderer, which is exact and total, so the
//!   formatted output always re-parses to a structurally equal tree — the formatter
//!   never drops or reshapes a clause it does not understand.
//! - **Spelling fidelity for free.** Fragments come from the `PreserveSource`
//!   canonical renderer, so every preserved spelling (types, literals, quote styles)
//!   round-trips exactly; layout only rearranges whitespace between fragments.
//!
//! Subqueries reachable from a structured position — a `FROM` derived table, a
//! scalar / `IN` / `EXISTS` / quantified predicate in `WHERE`/`HAVING`, a scalar
//! subquery in the projection, and a CTE body — are themselves re-laid out
//! recursively (clause-per-line, indented one level) by the same structured pass,
//! not embedded flat; see [`PrettyRenderer::structured_subquery`] for the
//! re-layout threshold. Deep re-layout of *non-subquery* nested expressions (long
//! arithmetic / boolean chains) inside a flat fragment remains a documented v1
//! limitation (see [`crate::format`]).

use std::collections::HashSet;

use super::comments::CommentAttachments;
use super::doc::{self, Doc};
use crate::ast::Span;
use crate::ast::render::{RenderConfig, RenderCtx, RenderExt};
use crate::ast::{
    Cte, CteBody, DerivedSpelling, Expr, GroupByItem, NoExt, OrderByExpr, Query, Resolver, Select,
    SelectDistinct, SelectItem, SetExpr, SetOperator, SetQuantifier, SourceStore, Spanned,
    Statement, TableFactor, TableWithJoins, With,
};
use crate::dialect::BuiltinDialect;
use crate::parser::Parsed;
use crate::tokenizer::{Punctuation, TokenKind, TriviaKind, TriviaRange};

/// Builds a [`Doc`] from a parse tree, weaving in attached comments and tracking
/// which comments were emitted (for the no-drop safety net).
pub(super) struct PrettyRenderer<'a> {
    resolver: &'a dyn Resolver,
    source: &'a str,
    config: RenderConfig,
    comments: &'a CommentAttachments,
    indent: usize,
    /// The dialect the tree was parsed under, used to re-tokenize a canonical fragment
    /// when locating a subquery's parenthesized substring for re-layout (see
    /// [`Self::relayout_fragment`]).
    dialect: BuiltinDialect,
    /// Start offsets of comments already emitted, so the safety net can append any
    /// that no structured position claimed.
    emitted: HashSet<u32>,
}

impl<'a> PrettyRenderer<'a> {
    pub(super) fn new<S: SourceStore>(
        parsed: &'a Parsed<S, NoExt>,
        comments: &'a CommentAttachments,
        indent: usize,
        dialect: BuiltinDialect,
    ) -> PrettyRenderer<'a> {
        PrettyRenderer {
            resolver: parsed.resolver(),
            source: parsed.source(),
            config: RenderConfig::default(),
            comments,
            indent,
            dialect,
            emitted: HashSet::new(),
        }
    }

    /// Render every statement, joined by `;` and a blank line, then append any comment
    /// no structured position claimed (the no-drop guarantee).
    pub(super) fn document(&mut self, statements: &[Statement<NoExt>]) -> Doc {
        let stmts: Vec<Doc> = statements.iter().map(|s| self.statement(s)).collect();
        let sep = doc::concat([Doc::text(";"), doc::hardline(), doc::hardline()]);
        let body = doc::join(sep, stmts);
        let leftover = self.leftover_comments();
        doc::concat([body, leftover])
    }

    /// The canonical single-line fragment for a node.
    fn frag<T: RenderExt>(&self, node: &T) -> String {
        let ctx = RenderCtx::new(self.resolver, self.source, &self.config);
        format!("{}", node.displayed(&ctx))
    }

    /// The source text of a trivia run.
    fn text_of(&self, range: TriviaRange) -> String {
        let span = range.span();
        self.source[span.start() as usize..span.end() as usize].to_owned()
    }

    /// Leading comments for the node at `span`, each on its own line before the node.
    fn leading(&mut self, span: Span) -> Doc {
        let ranges: Vec<TriviaRange> = self.comments.leading_for(span).to_vec();
        let mut parts = Vec::new();
        for range in ranges {
            self.emitted.insert(range.span().start());
            parts.push(doc::concat([
                Doc::text(self.text_of(range)),
                doc::hardline(),
            ]));
        }
        doc::concat(parts)
    }

    /// Trailing comments for the node at `span`: ` <comment>` after the node. Returns
    /// the doc and whether any is a line comment (which forces an enclosing list to
    /// break, so following syntax never lands on the commented-out line).
    fn trailing(&mut self, span: Span) -> (Doc, bool) {
        let ranges: Vec<TriviaRange> = self.comments.trailing_for(span).to_vec();
        let mut parts = Vec::new();
        let mut has_line = false;
        for range in ranges {
            self.emitted.insert(range.span().start());
            has_line |= matches!(range.kind(), TriviaKind::LineComment);
            parts.push(Doc::text(format!(" {}", self.text_of(range))));
        }
        (doc::concat(parts), has_line)
    }

    /// Comments whose source position falls *inside* `span` and that no more-specific
    /// structured position has emitted, rendered adjacent (trailing) to the fragment
    /// that owns `span`.
    ///
    /// The structured path flattens a node's interior into a single canonical fragment
    /// (across an operator, inside empty parens, inside a subquery, or inside a
    /// non-`SELECT` fallback statement). A comment anchored to one of those swallowed
    /// sub-nodes has no structured position of its own, so instead of falling through
    /// to the tail-relocating no-drop net it hoists here to trail the enclosing
    /// fragment, keeping it near its source. A line comment gets a trailing hardline so
    /// it can never comment out whatever follows on the same line (the parse-back
    /// guarantee); its exact interior column is unrecoverable from a flattened
    /// fragment, so adjacency is the fidelity ceiling for the fragment path.
    fn interior(&mut self, span: Span) -> Doc {
        // `all_comments` is source-ordered (trivia is captured in order), so the
        // collected ranges need no re-sort.
        let ranges: Vec<TriviaRange> = self
            .comments
            .all_comments()
            .iter()
            .filter(|r| {
                let s = r.span().start();
                !self.emitted.contains(&s) && s >= span.start() && s < span.end()
            })
            .copied()
            .collect();
        let mut parts = Vec::new();
        for range in ranges {
            self.emitted.insert(range.span().start());
            let piece = Doc::text(format!(" {}", self.text_of(range)));
            if matches!(range.kind(), TriviaKind::LineComment) {
                parts.push(doc::concat([piece, doc::hardline()]));
            } else {
                parts.push(piece);
            }
        }
        doc::concat(parts)
    }

    /// The offset of the separating comma between two adjacent list items, scanning the
    /// source gap `[from, to)` and skipping any comma that sits inside a comment run.
    fn separator_comma(&self, from: u32, to: u32) -> Option<u32> {
        let bytes = self.source.as_bytes();
        let end = (to as usize).min(bytes.len());
        let mut i = from as usize;
        while i < end {
            if let Some(run) = self.comments.all_comments().iter().find(|c| {
                let s = c.span();
                (s.start() as usize) <= i && i < (s.end() as usize)
            }) {
                i = run.span().end() as usize;
                continue;
            }
            if bytes[i] == b',' {
                return Some(i as u32);
            }
            i += 1;
        }
        None
    }

    /// The comments to render *with* a list item, split into the part that renders
    /// before the item's separating comma and the part that renders after it, plus
    /// whether any is a line comment (which forces the enclosing list to break).
    ///
    /// Two disjoint sources: comments the fragment flattened (offset inside the item
    /// span) and comments attached as trailing of the item itself (offset after it).
    /// A comment written before the comma in the source renders before the comma, so it
    /// stays on the author's side of the separator — except a line comment, which would
    /// swallow the comma if placed before it on the same line. Such a line comment
    /// routes after the comma where the forced break keeps it safe: this honours the
    /// no-comment-out-the-separator invariant that puts the comma there to begin with,
    /// at the cost of crossing the comma for the line-comment-before-comma case only.
    fn item_comments(&mut self, span: Span, next_start: Option<u32>) -> (Doc, Doc, bool) {
        let mut ranges: Vec<TriviaRange> = self
            .comments
            .all_comments()
            .iter()
            .filter(|r| {
                let s = r.span().start();
                !self.emitted.contains(&s) && s >= span.start() && s < span.end()
            })
            .copied()
            .collect();
        ranges.extend(
            self.comments
                .trailing_for(span)
                .iter()
                .filter(|r| !self.emitted.contains(&r.span().start()))
                .copied(),
        );
        if ranges.is_empty() {
            return (doc::nil(), doc::nil(), false);
        }
        ranges.sort_by_key(|r| r.span().start());

        let comma_off = next_start.and_then(|ns| self.separator_comma(span.end(), ns));
        let mut pre = Vec::new();
        let mut post = Vec::new();
        let mut has_line = false;
        for range in ranges {
            self.emitted.insert(range.span().start());
            let is_line = matches!(range.kind(), TriviaKind::LineComment);
            has_line |= is_line;
            let before_comma = comma_off.is_some_and(|c| range.span().start() < c);
            let piece = Doc::text(format!(" {}", self.text_of(range)));
            if before_comma && !is_line {
                pre.push(piece);
            } else {
                post.push(piece);
            }
        }
        (doc::concat(pre), doc::concat(post), has_line)
    }

    /// Any captured comment no structured position emitted, appended at the end so a
    /// comment is never silently dropped even when its anchor sits inside a fragment.
    fn leftover_comments(&mut self) -> Doc {
        let mut ranges: Vec<TriviaRange> = self
            .comments
            .all_comments()
            .iter()
            .filter(|r| !self.emitted.contains(&r.span().start()))
            .copied()
            .collect();
        if ranges.is_empty() {
            return doc::nil();
        }
        ranges.sort_by_key(|r| r.span().start());
        let mut parts = vec![doc::hardline()];
        for range in ranges {
            self.emitted.insert(range.span().start());
            parts.push(doc::concat([
                doc::hardline(),
                Doc::text(self.text_of(range)),
            ]));
        }
        doc::concat(parts)
    }

    /// A statement: leading comments, then its body (structured for a query, canonical
    /// fragment otherwise), then trailing comments.
    fn statement(&mut self, stmt: &Statement<NoExt>) -> Doc {
        let span = stmt.span();
        let lead = self.leading(span);
        let body = match stmt {
            Statement::Query { query, .. } => self.query(query),
            // A fallback statement is one flat fragment; sweep its interior comments so
            // they render adjacent instead of relocating to the output tail.
            other => doc::concat([Doc::text(self.frag(other)), self.interior(span)]),
        };
        let (trail, _) = self.trailing(span);
        doc::concat([lead, body, trail])
    }

    /// A query: `WITH`, body, `ORDER BY`, `LIMIT` — each on its own line — when its
    /// shape is one v1 structures; otherwise the whole query as a canonical fragment.
    fn query(&mut self, query: &Query<NoExt>) -> Doc {
        match self.try_query(query) {
            Some(doc) => doc,
            None => doc::concat([Doc::text(self.frag(query)), self.interior(query.span())]),
        }
    }

    /// `Some` structured layout when `query` uses only clauses v1 lays out, else `None`
    /// (caller falls back to a canonical fragment, preserving completeness).
    fn try_query(&mut self, query: &Query<NoExt>) -> Option<Doc> {
        // Exotic query tails are rendered flat via the fragment fallback.
        if query.order_by_all.is_some()
            || query.limit_by.is_some()
            || !query.settings.is_empty()
            || query.format.is_some()
            || !query.locking.is_empty()
            || !query.pipe_operators.is_empty()
            || query.for_clause.is_some()
        {
            return None;
        }

        let mut clauses: Vec<Doc> = Vec::new();
        if let Some(with) = &query.with {
            clauses.push(self.with_clause(with)?);
        }
        clauses.push(self.set_expr(&query.body)?);
        if !query.order_by.is_empty() {
            clauses.push(self.order_by_clause(&query.order_by));
        }
        if let Some(limit) = &query.limit {
            clauses.push(Doc::text(self.frag(limit)));
        }
        Some(doc::join(doc::hardline(), clauses))
    }

    /// A `WITH` block: each CTE `name AS ( <query> )`, comma-separated, then the body
    /// on the next line. `None` for a CTE shape v1 does not structure (SEARCH/CYCLE, or
    /// a non-query body) so the caller falls back to a fragment.
    fn with_clause(&mut self, with: &With<NoExt>) -> Option<Doc> {
        let keyword = if with.recursive {
            "WITH RECURSIVE "
        } else {
            "WITH "
        };
        let mut cte_docs: Vec<Doc> = Vec::new();
        for cte in &with.ctes {
            cte_docs.push(self.cte(cte)?);
        }
        let joined = doc::join(doc::concat([Doc::text(","), doc::hardline()]), cte_docs);
        Some(doc::concat([Doc::text(keyword), joined]))
    }

    /// One CTE. `None` unless it is a plain query body with no SEARCH/CYCLE tail.
    fn cte(&mut self, cte: &Cte<NoExt>) -> Option<Doc> {
        if cte.search.is_some() || cte.cycle.is_some() {
            return None;
        }
        let CteBody::Query { query, .. } = &cte.body else {
            return None;
        };
        let inner = self.try_query(query)?;

        let mut head = self.frag(&cte.name);
        if !cte.columns.is_empty() {
            let cols: Vec<String> = cte.columns.iter().map(|c| self.frag(c)).collect();
            head.push_str(&format!(" ({})", cols.join(", ")));
        }
        head.push_str(" AS ");
        match cte.materialized {
            Some(true) => head.push_str("MATERIALIZED "),
            Some(false) => head.push_str("NOT MATERIALIZED "),
            None => {}
        }
        head.push('(');
        Some(doc::concat([
            Doc::text(head),
            doc::nest(self.indent, doc::concat([doc::hardline(), inner])),
            doc::hardline(),
            Doc::text(")"),
        ]))
    }

    /// A query body: a structured `SELECT`, a recursive set operation, or a nested
    /// query; `None` for an exotic body (VALUES, PIVOT, …) that falls back to a fragment.
    fn set_expr(&mut self, body: &SetExpr<NoExt>) -> Option<Doc> {
        match body {
            SetExpr::Select { select, .. } => self.select(select),
            SetExpr::Query { query, .. } => self.try_query(query),
            SetExpr::SetOperation {
                op,
                all,
                by_name,
                left,
                right,
                ..
            } => {
                let left_doc = self.set_expr(left)?;
                let right_doc = self.set_expr(right)?;
                let word = set_op_word(op, *all, *by_name);
                Some(doc::join(
                    doc::hardline(),
                    [left_doc, Doc::text(word), right_doc],
                ))
            }
            _ => None,
        }
    }

    /// A `SELECT`: one clause per line. `None` when the SELECT uses a clause outside
    /// the v1 structured set (see the predicate), so the caller renders it flat.
    fn select(&mut self, select: &Select<NoExt>) -> Option<Doc> {
        if !is_simple_select(select) {
            return None;
        }

        let mut clauses: Vec<Doc> = Vec::new();

        // SELECT [DISTINCT] <projection>
        let mut select_kw = String::from("SELECT");
        if let Some(distinct) = &select.distinct {
            select_kw.push(' ');
            select_kw.push_str(&self.distinct_text(distinct));
        }
        let proj_items: Vec<ListItem> = select
            .projection
            .iter()
            .map(|item| ListItem {
                span: item.span(),
                doc: self.projection_item_doc(item),
            })
            .collect();
        clauses.push(self.list_clause(select_kw, &proj_items));

        // FROM <tables>
        if !select.from.is_empty() {
            let from_items: Vec<ListItem> = select
                .from
                .iter()
                .map(|twj| ListItem {
                    span: twj.span(),
                    doc: self.table_item_doc(twj),
                })
                .collect();
            clauses.push(self.list_clause(String::from("FROM"), &from_items));
        }

        // WHERE <predicate>
        if let Some(selection) = &select.selection {
            clauses.push(self.expr_clause("WHERE", selection));
        }

        // GROUP BY <items>
        if !select.group_by.is_empty() {
            let group_items: Vec<ListItem> = select
                .group_by
                .iter()
                .map(|item| ListItem {
                    span: group_by_item_span(item),
                    doc: Doc::text(self.frag(item)),
                })
                .collect();
            clauses.push(self.list_clause(String::from("GROUP BY"), &group_items));
        }

        // HAVING <predicate>
        if let Some(having) = &select.having {
            clauses.push(self.expr_clause("HAVING", having));
        }

        Some(doc::join(doc::hardline(), clauses))
    }

    /// The `ALL` / `DISTINCT` / `DISTINCT ON (...)` quantifier text.
    fn distinct_text(&self, distinct: &SelectDistinct<NoExt>) -> String {
        match distinct {
            SelectDistinct::Quantifier { quantifier, .. } => match quantifier {
                SetQuantifier::All => "ALL".to_owned(),
                SetQuantifier::Distinct => "DISTINCT".to_owned(),
            },
            SelectDistinct::On { exprs, .. } => {
                let keys: Vec<String> = exprs.iter().map(|e| self.frag(e)).collect();
                format!("DISTINCT ON ({})", keys.join(", "))
            }
        }
    }

    /// An `ORDER BY` list clause.
    fn order_by_clause(&mut self, items: &[OrderByExpr<NoExt>]) -> Doc {
        let list: Vec<ListItem> = items
            .iter()
            .map(|item| ListItem {
                span: item.span(),
                doc: Doc::text(self.frag(item)),
            })
            .collect();
        self.list_clause(String::from("ORDER BY"), &list)
    }

    /// A single-expression clause (`WHERE`/`HAVING`): keyword, a space, the fragment.
    /// Leading comments anchored to the predicate render before the keyword (the
    /// clause-mark before-keyword fix); trailing after.
    fn expr_clause(&mut self, keyword: &'static str, expr: &Expr<NoExt>) -> Doc {
        let span = expr.span();
        let lead = self.leading(span);
        // Re-lay out any subquery reachable in the predicate (`WHERE a IN (SELECT …)`,
        // `WHERE a > (SELECT …)`, `WHERE EXISTS (SELECT …)`); non-subquery structure
        // stays a flat fragment. A comment placed inside a re-laid-out subquery gets a
        // real structured position from the recursion, so it renders in place there.
        let body = self.expr_body_doc(expr);
        // Any remaining comment inside the predicate fragment (`WHERE b = /* c */ 2`,
        // i.e. not inside a structured subquery) still has no structured position, so
        // hoist it adjacent to the clause fragment.
        let interior = self.interior(span);
        let (trail, _) = self.trailing(span);
        doc::concat([
            lead,
            Doc::text(keyword),
            Doc::text(" "),
            body,
            interior,
            trail,
        ])
    }

    /// The layout doc for one projection item: its canonical fragment with any scalar
    /// subquery re-laid out in place (`SELECT (SELECT max(x) FROM u) AS m`).
    fn projection_item_doc(&mut self, item: &SelectItem<NoExt>) -> Doc {
        let whole = self.frag(item);
        let mut subs: Vec<&Query<NoExt>> = Vec::new();
        if let SelectItem::Expr { expr, .. } = item {
            collect_expr_subqueries(expr, &mut subs);
        }
        self.relayout_fragment(whole, &subs)
    }

    /// The layout doc for one `FROM` item: its canonical fragment with any derived-table
    /// subquery (in the leading relation or a join) re-laid out in place.
    fn table_item_doc(&mut self, twj: &TableWithJoins<NoExt>) -> Doc {
        let whole = self.frag(twj);
        let mut subs: Vec<&Query<NoExt>> = Vec::new();
        collect_factor_subqueries(&twj.relation, &mut subs);
        for join in &twj.joins {
            collect_factor_subqueries(&join.relation, &mut subs);
        }
        self.relayout_fragment(whole, &subs)
    }

    /// The layout doc for a clause-position expression: its canonical fragment with any
    /// reachable subquery re-laid out in place.
    fn expr_body_doc(&mut self, expr: &Expr<NoExt>) -> Doc {
        let whole = self.frag(expr);
        let mut subs: Vec<&Query<NoExt>> = Vec::new();
        collect_expr_subqueries(expr, &mut subs);
        self.relayout_fragment(whole, &subs)
    }

    /// Splice the structured layout of each re-laid-out subquery into `whole`, the
    /// enclosing node's flat canonical fragment.
    ///
    /// The canonical renderer is compositional: a parenthesized subquery's fragment is
    /// exactly `(` + the subquery rendered as a standalone query + `)` (a
    /// parenthesized query never takes precedence-dependent extra parens), so that
    /// fragment appears verbatim as a substring of `whole`, in render (left-to-right)
    /// order — the same order `subs` is collected in. Each such substring is replaced
    /// with the subquery's structured multi-line doc, leaving every other byte of the
    /// canonical fragment (operators, aliases, minimal parens) untouched, so all
    /// precedence-sensitive rendering stays the canonical renderer's job.
    ///
    /// A candidate match is accepted only where a real `(` *token* begins (`whole` is
    /// re-tokenized under the parse dialect to find those offsets): the same byte
    /// sequence can also occur inside a string literal — `SELECT '(SELECT 1 FROM t)'
    /// || (SELECT 1 FROM t)` — where a blind splice would rewrite the literal's
    /// contents and break spelling fidelity and parse-back.
    ///
    /// A subquery kept inline (trivial or an exotic body [`try_query`](Self::try_query)
    /// declines), one whose fragment is not located (the compositionality invariant
    /// failing, which should not happen), or a fragment that fails to re-tokenize is
    /// left as fragment text — always a correct fallback.
    fn relayout_fragment(&mut self, whole: String, subs: &[&Query<NoExt>]) -> Doc {
        if subs.is_empty() {
            return Doc::text(whole);
        }
        let Ok(tokens) = crate::tokenize_with_builtin(&whole, self.dialect) else {
            return Doc::text(whole);
        };
        let lparen_offsets: HashSet<u32> = tokens
            .iter()
            .filter(|t| matches!(t.kind, TokenKind::Punctuation(Punctuation::LParen)))
            .map(|t| t.span.start())
            .collect();
        let mut parts: Vec<Doc> = Vec::new();
        let mut cursor = 0usize;
        for &query in subs {
            let needle = format!("({})", self.frag(query));
            let mut search = cursor;
            let found = loop {
                let Some(rel) = whole[search..].find(&needle) else {
                    break None;
                };
                let at = search + rel;
                if lparen_offsets.contains(&(at as u32)) {
                    break Some(at);
                }
                // Matched inside a wider token (a string literal); resume just past
                // the `(` byte, which is ASCII so `at + 1` is a char boundary.
                search = at + 1;
            };
            let Some(at) = found else {
                continue;
            };
            let Some(sub_doc) = self.structured_subquery(query) else {
                continue;
            };
            if at > cursor {
                parts.push(Doc::text(whole[cursor..at].to_owned()));
            }
            parts.push(sub_doc);
            cursor = at + needle.len();
        }
        if cursor < whole.len() {
            parts.push(Doc::text(whole[cursor..].to_owned()));
        }
        if parts.is_empty() {
            return Doc::text(whole);
        }
        doc::concat(parts)
    }

    /// The structured, indented `( … )` layout for a subquery, or `None` to keep it
    /// inline as a fragment.
    ///
    /// Re-layout threshold: a subquery is laid out across lines exactly when (a) its
    /// body is one the structured pass recognizes ([`try_query`](Self::try_query)
    /// succeeds — otherwise the flat fragment is the completeness fallback) and (b)
    /// structuring it actually spans more than one line (its layout carries a forced
    /// break). Condition (b) keeps a trivial single-clause subquery (`(SELECT 1)`,
    /// `(SELECT count(*))`) inline, where exploding it across lines would add
    /// parens-on-their-own-lines noise with no readability gain; a subquery with real
    /// clause structure (a `FROM`/`WHERE`/`GROUP BY`, a `WITH`, set-op arms, or a
    /// projection long enough to force its own break) is re-laid out, one clause per
    /// line at the same rules as the top-level query. The threshold counts forced
    /// breaks in the candidate layout, so it is a stable structural policy, not a
    /// character-count heuristic.
    ///
    /// `try_query` emits any comments interior to the subquery as a side effect; when
    /// the subquery is kept inline the emission is rolled back, so those comments stay
    /// available to the enclosing fragment's interior sweep (and are never dropped).
    fn structured_subquery(&mut self, query: &Query<NoExt>) -> Option<Doc> {
        let snapshot = self.emitted.clone();
        match self.try_query(query) {
            Some(inner) if doc_has_hardline(&inner) => Some(doc::concat([
                Doc::text("("),
                doc::nest(self.indent, doc::concat([doc::hardline(), inner])),
                doc::hardline(),
                Doc::text(")"),
            ])),
            _ => {
                self.emitted = snapshot;
                None
            }
        }
    }

    /// A list clause: `KEYWORD item, item, …` when it fits, one item per line when it
    /// does not. Commas ride the item they follow so a trailing comment never
    /// comments out the separator; any leading or line-comment-trailing item forces
    /// the clause to break.
    fn list_clause(&mut self, keyword: String, items: &[ListItem]) -> Doc {
        // Leading of the first item anchors before the keyword (clause-mark fix).
        let keyword_lead = items
            .first()
            .map(|it| self.leading(it.span))
            .unwrap_or_else(doc::nil);

        let mut force_break = !matches!(keyword_lead, Doc::Nil);
        let mut pieces: Vec<Doc> = Vec::with_capacity(items.len());
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            // First item's leading already went before the keyword.
            let lead = if i == 0 {
                doc::nil()
            } else {
                self.leading(item.span)
            };
            if !matches!(lead, Doc::Nil) {
                force_break = true;
            }
            let comma = if i < last { "," } else { "" };
            let next_start = items.get(i + 1).map(|it| it.span.start());
            // `pre` renders between the item and its comma (a source-side-trailing
            // comment stays on the author's side of the separator); `post` renders
            // after the comma. Interior comments the fragment flattened are folded in.
            let (pre, post, has_line) = self.item_comments(item.span, next_start);
            force_break |= has_line;
            pieces.push(doc::concat([
                lead,
                item.doc.clone(),
                pre,
                Doc::text(comma),
                post,
            ]));
        }

        let sep = if force_break {
            doc::hardline()
        } else {
            doc::line()
        };
        let body = doc::nest(
            self.indent,
            doc::concat([doc::line(), doc::join(sep, pieces)]),
        );
        let clause = doc::concat([Doc::text(keyword), body]);
        doc::concat([
            keyword_lead,
            if force_break {
                clause
            } else {
                doc::group(clause)
            },
        ])
    }
}

/// A rendered list entry: its source span (for comment lookup) and layout doc. The
/// doc is usually a single-line [`Doc::Text`] fragment, but a projection or `FROM`
/// item that reaches a re-laid-out subquery carries a structured multi-line doc.
struct ListItem {
    span: Span,
    doc: Doc,
}

/// Collect, in canonical render order, every re-layoutable subquery reachable from
/// `expr` through the paren-safe carrier shapes: a scalar [`Expr::Subquery`], the
/// right side of `IN`/`EXISTS`/quantified predicates, and the operands of binary and
/// unary operators (so a subquery buried in `a > (SELECT …)` or `NOT EXISTS (…)` is
/// reached). Recursion stops *at* each subquery — its own nested subqueries are
/// re-laid out when that subquery is structured, not collected here. Shapes that would
/// need precedence-dependent reconstruction to split (function args, `CASE`, …) are
/// left whole; their subqueries stay inline, a documented fragment-path limitation.
fn collect_expr_subqueries<'e>(expr: &'e Expr<NoExt>, out: &mut Vec<&'e Query<NoExt>>) {
    match expr {
        Expr::Subquery { query, .. } | Expr::Exists { query, .. } => out.push(query),
        Expr::InSubquery { expr, subquery, .. } => {
            collect_expr_subqueries(expr, out);
            out.push(subquery);
        }
        Expr::QuantifiedComparison { left, subquery, .. } => {
            collect_expr_subqueries(left, out);
            out.push(subquery);
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_expr_subqueries(left, out);
            collect_expr_subqueries(right, out);
        }
        Expr::UnaryOp { expr, .. } => collect_expr_subqueries(expr, out),
        _ => {}
    }
}

/// Collect the derived-table subquery of a `FROM` table factor, if it is the standard
/// parenthesized form. DuckDB's bare `FROM VALUES` ([`DerivedSpelling::BareValues`])
/// has no parentheses and a `VALUES` body the structured pass declines, so it is left
/// inline.
fn collect_factor_subqueries<'e>(factor: &'e TableFactor<NoExt>, out: &mut Vec<&'e Query<NoExt>>) {
    if let TableFactor::Derived {
        subquery, spelling, ..
    } = factor
    {
        if matches!(spelling, DerivedSpelling::Parenthesized) {
            out.push(subquery);
        }
    }
}

/// Whether laying `doc` out yields more than one line, i.e. it carries a forced break
/// ([`Doc::HardLine`]) at any depth. The re-layout threshold in
/// [`PrettyRenderer::structured_subquery`] uses this to keep a single-line subquery
/// inline.
fn doc_has_hardline(doc: &Doc) -> bool {
    match doc {
        Doc::HardLine => true,
        Doc::Concat(parts) => parts.iter().any(doc_has_hardline),
        Doc::Nest(_, inner) => doc_has_hardline(inner),
        Doc::Group(inner) => doc_has_hardline(inner),
        Doc::Nil | Doc::Text(_) | Doc::Line | Doc::SoftLine => false,
    }
}

/// The rendered keyword for a set operation, e.g. `UNION ALL`, `UNION ALL BY NAME`.
fn set_op_word(op: &SetOperator, all: bool, by_name: bool) -> String {
    let base = match op {
        SetOperator::Union => "UNION",
        SetOperator::Intersect => "INTERSECT",
        SetOperator::Except => "EXCEPT",
    };
    let mut word = String::from(base);
    if all {
        word.push_str(" ALL");
    }
    if by_name {
        word.push_str(" BY NAME");
    }
    word
}

/// The span of a `GROUP BY` item across its variants.
fn group_by_item_span(item: &GroupByItem<NoExt>) -> Span {
    item.span()
}

/// Whether a `SELECT` uses only the clauses v1 lays out structurally. Any exotic
/// clause routes the whole SELECT to the flat canonical fragment, keeping the
/// structured path small and always complete.
fn is_simple_select(select: &Select<NoExt>) -> bool {
    matches!(select.spelling, crate::ast::SelectSpelling::Select)
        && !select.straight_join
        && select.into.is_none()
        && select.lateral_views.is_empty()
        && select.connect_by.is_none()
        && select.group_by_quantifier.is_none()
        && select.group_by_all.is_none()
        && select.windows.is_empty()
        && select.qualify.is_none()
        && select.sample.is_none()
}
