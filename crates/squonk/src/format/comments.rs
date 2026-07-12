// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The trivia -> node comment-attachment pass (formatter spike design).
//!
//! Comments are captured out of band as a flat, source-ordered side-table
//! ([`Parsed::trivia`](crate::Parsed::trivia)); they are never part of the AST and
//! never consulted by the canonical renderer. To survive a *format*, each comment
//! must be re-attached to a node and a position (leading / trailing / dangling), the
//! industry vocabulary Prettier and rustfmt share. This module computes that
//! attachment table post-parse, over the typed node tree, and hands it to the pretty
//! renderer — it never mutates the AST, so structural equality is preserved.
//!
//! ## The design (per `spike-formatter-comment-attachment`)
//!
//! Pure span arithmetic is deterministic but mis-renders three hard cases because a
//! clause keyword / operator / bracket owns no node. This pass therefore combines:
//!
//! 1. **Span-arithmetic tree walk.** The node inventory comes from
//!    [`NodeIdWalk`] (pre-order `Meta`); span
//!    containment reconstructs the parent/child tree. For each comment: the *deepest*
//!    node whose span contains it is the enclosing node; among that node's direct
//!    children, the nearest one ending before is `preceding` and the nearest starting
//!    after is `following`; a newline in the gap chooses leading(following) vs
//!    trailing(preceding).
//! 2. **Clause-mark anchoring.** When a clause-introducing keyword (`WHERE`,
//!    `GROUP BY`, …) sits between the comment and the following child, the comment
//!    belongs *before that keyword*. It is recorded as `leading` of the clause-body
//!    node; the renderer emits leading comments before the clause keyword, so the
//!    highest-visibility miss (`GROUP BY -- c`) is closed. Uses the parser's opt-in
//!    [`clause_marks`](crate::Parsed::clause_marks).
//! 3. **Kind-aware dangling.** A comment inside an otherwise-empty bracketed construct
//!    (`count(/*c*/)`) must dangle *inside* the construct, not trail the last child
//!    before the `(`. A small typed walk flags empty-argument function calls so the
//!    classifier can dangle rather than mis-trail.
//!
//! ## Storage / lookup agreement
//!
//! The renderer looks comments up by a node's [`Spanned::span`](crate::ast::Spanned),
//! resolved through the deepest-wins [`span_to_id`](CommentAttachments) index. Every
//! attachment — interior (`classify`) and statement-boundary (`attach_top_level`) alike
//! — therefore anchors to that same deepest-wins id, so a stored comment is never missed
//! by the span-keyed lookup and forced into the renderer's tail-relocating no-drop net.
//!
//! ## Placement fidelity
//!
//! A comment anchored to a sub-node the renderer flattens into a single-line canonical
//! fragment (operator-crossing `a + /*c*/ b`, dangling in empty parens) keeps a correct
//! *attachment* here; the renderer hoists it adjacent to the enclosing fragment (see
//! [`crate::format`]). Re-injecting a comment at its exact column inside a flattened
//! fragment is out of reach until the fragment interior is structured.

use std::collections::HashMap;

use crate::ast::generated::NodeIdWalk;
use crate::ast::generated::visit::{self, Visit};
use crate::ast::{FunctionCall, NoExt, NodeId, SourceStore, Span};
use crate::parser::Parsed;
use crate::tokenizer::{TriviaKind, TriviaRange};

/// The comments attached to one node, in source order within each position.
#[derive(Clone, Debug, Default)]
pub struct NodeComments {
    /// Comments rendered before the node (or, for a clause body, before its keyword).
    pub leading: Vec<TriviaRange>,
    /// Comments rendered after the node.
    pub trailing: Vec<TriviaRange>,
    /// Comments rendered inside an otherwise-empty construct the node heads.
    pub dangling: Vec<TriviaRange>,
}

/// The attachment table: every captured comment anchored to a node and a position.
///
/// Keyed by [`NodeId`] (stable, on every node's `Meta`) so the render walk looks up
/// `attachments.get(id)` at each node. Empty when the parse captured no trivia (the
/// default parse path) — the formatter then runs pure layout with no comment work.
#[derive(Clone, Debug, Default)]
pub struct CommentAttachments {
    by_anchor: HashMap<NodeId, NodeComments>,
    /// Exact-span -> node id, so the renderer can look comments up from a node's
    /// [`Spanned::span`](crate::ast::Spanned) without a per-enum node-id accessor.
    /// On an equal-span wrapper chain (`Statement -> Query -> Select`) the deepest
    /// (last in pre-order) node wins — the innermost node at that position, which is
    /// where a comment should render.
    span_to_id: HashMap<Span, NodeId>,
    /// Every captured comment run, in source order — the renderer's no-drop safety
    /// net iterates these and appends any it did not otherwise emit.
    comments: Vec<TriviaRange>,
    /// Total comments the pass attached, for the renderer's no-drop safety net.
    attached: usize,
}

impl CommentAttachments {
    /// Comments to render before `id` (before its clause keyword, for a clause body).
    pub fn leading(&self, id: NodeId) -> &[TriviaRange] {
        self.by_anchor.get(&id).map_or(&[], |c| &c.leading)
    }

    /// Comments to render after `id`.
    pub fn trailing(&self, id: NodeId) -> &[TriviaRange] {
        self.by_anchor.get(&id).map_or(&[], |c| &c.trailing)
    }

    /// Comments to render inside the empty construct `id` heads.
    pub fn dangling(&self, id: NodeId) -> &[TriviaRange] {
        self.by_anchor.get(&id).map_or(&[], |c| &c.dangling)
    }

    /// Whether any comment was attached.
    pub fn is_empty(&self) -> bool {
        self.attached == 0
    }

    /// The number of attached comments (== the number of captured comment runs).
    pub fn len(&self) -> usize {
        self.attached
    }

    /// Every captured comment run, in source order (the renderer's no-drop net).
    pub fn all_comments(&self) -> &[TriviaRange] {
        &self.comments
    }

    /// Leading comments for the node with exactly `span` (via [`Spanned::span`]).
    ///
    /// [`Spanned::span`]: crate::ast::Spanned::span
    pub fn leading_for(&self, span: Span) -> &[TriviaRange] {
        self.span_to_id
            .get(&span)
            .map_or(&[], |id| self.leading(*id))
    }

    /// Trailing comments for the node with exactly `span`.
    pub fn trailing_for(&self, span: Span) -> &[TriviaRange] {
        self.span_to_id
            .get(&span)
            .map_or(&[], |id| self.trailing(*id))
    }

    /// Dangling comments for the node with exactly `span`.
    pub fn dangling_for(&self, span: Span) -> &[TriviaRange] {
        self.span_to_id
            .get(&span)
            .map_or(&[], |id| self.dangling(*id))
    }

    fn entry(&mut self, id: NodeId) -> &mut NodeComments {
        self.by_anchor.entry(id).or_default()
    }

    /// Compute the attachment table for `parsed`. Empty when no trivia was captured.
    pub fn compute<S: SourceStore>(parsed: &Parsed<S, NoExt>) -> CommentAttachments {
        let comments: Vec<TriviaRange> = parsed
            .trivia()
            .iter()
            .copied()
            .filter(|t| matches!(t.kind(), TriviaKind::LineComment | TriviaKind::BlockComment))
            .collect();
        if comments.is_empty() {
            return CommentAttachments::default();
        }

        // Node inventory: pre-order `Meta`, source-backed only.
        let mut walk = NodeIdWalk::default();
        for statement in parsed.statements() {
            walk.visit_statement(statement);
        }
        let nodes: Vec<NodeInfo> = walk
            .metas
            .iter()
            .filter(|m| !m.span.is_synthetic())
            .map(|m| NodeInfo {
                id: m.node_id,
                span: m.span,
            })
            .collect();

        // Parent/child tree by span containment over the pre-order inventory.
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        let mut roots: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            while let Some(&top) = stack.last() {
                if contains(nodes[top].span, node.span) {
                    break;
                }
                stack.pop();
            }
            match stack.last() {
                Some(&top) => children[top].push(i),
                None => roots.push(i),
            }
            stack.push(i);
        }

        // Empty-bracket nodes (kind-aware dangling), by NodeId.
        let mut brackets = EmptyBracketWalk::default();
        for statement in parsed.statements() {
            brackets.visit_statement(statement);
        }

        let source = parsed.source();
        // Exact-span index (deepest wins: pre-order visits outer before inner). Built
        // separately so `classify` can resolve an anchor node to the deepest node id
        // sharing its span — the same id the renderer's span-keyed lookup resolves to
        // (an equal-span wrapper like `GroupByItem`/its inner `Expr` must agree).
        let mut span_to_id: HashMap<Span, NodeId> = HashMap::new();
        for node in &nodes {
            span_to_id.insert(node.span, node.id);
        }

        let mut out = CommentAttachments {
            comments: comments.clone(),
            ..CommentAttachments::default()
        };
        for comment in &comments {
            out.attached += 1;
            classify(
                &mut out,
                *comment,
                &nodes,
                &children,
                &roots,
                &brackets.ids,
                &span_to_id,
                source,
                parsed,
            );
        }
        out.span_to_id = span_to_id;
        out
    }
}

/// One id-bearing, source-backed node in the pre-order inventory.
struct NodeInfo {
    id: NodeId,
    span: Span,
}

/// Whether `outer` fully contains `inner` (`outer.start <= inner.start` and
/// `inner.end <= outer.end`), both source-backed. Equal spans count as containment,
/// so an equal-span wrapper chain (`Statement -> Query -> Select`) nests.
fn contains(outer: Span, inner: Span) -> bool {
    !outer.is_synthetic()
        && !inner.is_synthetic()
        && outer.start() <= inner.start()
        && inner.end() <= outer.end()
}

/// Whether `source[a..b]` contains a newline. A missing/invalid slice reads as no
/// newline (conservative: a same-line trailing decision).
fn newline_between(source: &str, a: u32, b: u32) -> bool {
    if a >= b {
        return false;
    }
    source
        .get(a as usize..b as usize)
        .is_some_and(|s| s.contains('\n'))
}

/// Attach one `comment` to a node and position.
#[allow(clippy::too_many_arguments)]
fn classify<S: SourceStore>(
    out: &mut CommentAttachments,
    comment: TriviaRange,
    nodes: &[NodeInfo],
    children: &[Vec<usize>],
    roots: &[usize],
    brackets: &std::collections::HashSet<NodeId>,
    span_to_id: &HashMap<Span, NodeId>,
    source: &str,
    parsed: &Parsed<S, NoExt>,
) {
    // The deepest node id sharing a node's span — the id the renderer's span lookup
    // resolves to. Anchoring to it keeps storage and lookup in agreement.
    let anchor = |idx: usize| -> NodeId {
        span_to_id
            .get(&nodes[idx].span)
            .copied()
            .unwrap_or(nodes[idx].id)
    };
    let cspan = comment.span();

    // Enclosing = deepest (smallest span, then latest in pre-order) node containing it.
    let mut enclosing: Option<usize> = None;
    for (i, node) in nodes.iter().enumerate() {
        if !contains(node.span, cspan) {
            continue;
        }
        enclosing = Some(match enclosing {
            None => i,
            Some(best) => {
                let (bl, nl) = (nodes[best].span.len(), node.span.len());
                if nl < bl || (nl == bl && i > best) {
                    i
                } else {
                    best
                }
            }
        });
    }

    let Some(enc) = enclosing else {
        // Top-level: attach to a statement.
        attach_top_level(out, comment, nodes, roots, span_to_id);
        return;
    };

    // Nearest direct children bracketing the comment.
    let kids = &children[enc];
    let mut preceding: Option<usize> = None;
    let mut following: Option<usize> = None;
    for &k in kids {
        let ks = nodes[k].span;
        if ks.end() <= cspan.start() && preceding.is_none_or(|p| ks.end() > nodes[p].span.end()) {
            preceding = Some(k);
        }
        if ks.start() >= cspan.end() && following.is_none_or(|f| ks.start() < nodes[f].span.start())
        {
            following = Some(k);
        }
    }

    let enc_id = anchor(enc);
    // Clause-mark anchoring: a clause-introducing keyword (WHERE, GROUP BY, …) sits
    // between the preceding child and the comment. Then the comment falls *inside the
    // new clause* — `GROUP BY /* c */ a` reads as leading the grouping item, even on
    // one line, not trailing whatever ended the previous clause. This is the parser
    // cooperation the spike found load-bearing: the keyword owns no node, so pure span
    // arithmetic would mis-trail it. Recorded as leading of the following clause body,
    // which the renderer emits before the keyword.
    let preceding_end = preceding.map_or(nodes[enc].span.start(), |p| nodes[p].span.end());
    let clause_kw_before = parsed
        .clause_marks_in(nodes[enc].span)
        .iter()
        .any(|mark| mark.offset() >= preceding_end && mark.offset() < cspan.start());

    match (preceding, following) {
        (Some(p), Some(f)) => {
            // A comment on its own line (a newline separates it from the preceding
            // child), or one sitting after a clause keyword, leads the following node.
            // A same-line comment with no intervening clause keyword trails the
            // preceding child (the EOL-trailing case).
            let own_line = newline_between(source, nodes[p].span.end(), cspan.start());
            if own_line || clause_kw_before {
                out.entry(anchor(f)).leading.push(comment);
            } else {
                out.entry(anchor(p)).trailing.push(comment);
            }
        }
        (Some(p), None) => {
            // Kind-aware dangling: inside an empty bracketed construct, dangle rather
            // than trail the last child before the `(`.
            if brackets.contains(&enc_id) {
                out.entry(enc_id).dangling.push(comment);
            } else {
                out.entry(anchor(p)).trailing.push(comment);
            }
        }
        (None, Some(f)) => {
            out.entry(anchor(f)).leading.push(comment);
        }
        (None, None) => {
            out.entry(enc_id).dangling.push(comment);
        }
    }
}

/// Attach a comment that lies outside every node: before/between/after statements.
///
/// A statement-boundary comment must anchor to the SAME node id the renderer's
/// span-keyed lookup resolves to. The renderer queries a statement's leading/trailing
/// by its [`Spanned::span`](crate::ast::Spanned), and `span_to_id` is deepest-wins: a
/// bare statement's span (`Statement -> Query -> Select`) resolves to the innermost
/// co-span node, not the outer `Statement`. Storing against the raw root id would miss
/// that lookup and the comment would fall through to the renderer's no-drop net (which
/// relocates it to the output tail). Resolving through `span_to_id` keeps storage and
/// lookup in agreement, exactly as `classify`'s `anchor` does for interior comments.
fn attach_top_level(
    out: &mut CommentAttachments,
    comment: TriviaRange,
    nodes: &[NodeInfo],
    roots: &[usize],
    span_to_id: &HashMap<Span, NodeId>,
) {
    let anchor = |idx: usize| -> NodeId {
        span_to_id
            .get(&nodes[idx].span)
            .copied()
            .unwrap_or(nodes[idx].id)
    };
    let cspan = comment.span();
    let mut preceding: Option<usize> = None;
    let mut following: Option<usize> = None;
    for &r in roots {
        let rs = nodes[r].span;
        if rs.end() <= cspan.start() && preceding.is_none_or(|p| rs.end() > nodes[p].span.end()) {
            preceding = Some(r);
        }
        if rs.start() >= cspan.end() && following.is_none_or(|f| rs.start() < nodes[f].span.start())
        {
            following = Some(r);
        }
    }
    match (following, preceding) {
        // A following statement claims the comment as leading (before-statement /
        // between-statements). Otherwise it trails the last statement.
        (Some(f), _) => out.entry(anchor(f)).leading.push(comment),
        (None, Some(p)) => out.entry(anchor(p)).trailing.push(comment),
        (None, None) => {}
    }
}

/// Records the [`NodeId`]s of empty-argument function calls — the kind-aware dangling
/// set. `count(/*c*/)` dangles the comment inside the call rather than trailing the
/// `count` name node.
#[derive(Default)]
struct EmptyBracketWalk {
    ids: std::collections::HashSet<NodeId>,
}

impl<'ast> Visit<'ast, NoExt> for EmptyBracketWalk {
    fn visit_function_call(&mut self, node: &'ast FunctionCall<NoExt>) {
        if node.args.is_empty() {
            self.ids.insert(node.meta.node_id);
        }
        visit::walk_function_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Ansi;
    use crate::parse_with_trivia;

    fn attach(sql: &str) -> (CommentAttachments, crate::Parsed) {
        let parsed = parse_with_trivia(sql, Ansi).expect("parses");
        let attachments = CommentAttachments::compute(&parsed);
        (attachments, parsed)
    }

    /// The comment text for a range, sliced from the source.
    fn texts(ranges: &[TriviaRange], source: &str) -> Vec<String> {
        ranges
            .iter()
            .map(|r| source[r.span().start() as usize..r.span().end() as usize].to_owned())
            .collect()
    }

    #[test]
    fn empty_when_no_trivia_captured() {
        let parsed = crate::parse_with("SELECT 1", Ansi).expect("parses");
        assert!(CommentAttachments::compute(&parsed).is_empty());
    }

    #[test]
    fn leading_comment_before_statement() {
        let (a, p) = attach("-- hi\nSELECT 1");
        // Attaches to some node as leading; the comment is not lost.
        assert_eq!(a.len(), 1);
        let all: Vec<String> = a
            .by_anchor
            .values()
            .flat_map(|c| texts(&c.leading, p.source()))
            .collect();
        assert_eq!(all, vec!["-- hi".to_string()]);
    }

    #[test]
    fn comment_before_clause_keyword_anchors_leading_not_trailing() {
        // `-- note` sits before GROUP BY: it must be leading (rendered before the
        // keyword), never trailing the WHERE predicate.
        let sql = "SELECT a FROM t WHERE a = 1\n-- note\nGROUP BY a";
        let (a, p) = attach(sql);
        let leading_all: Vec<String> = a
            .by_anchor
            .values()
            .flat_map(|c| texts(&c.leading, p.source()))
            .collect();
        assert!(
            leading_all.contains(&"-- note".to_string()),
            "clause-keyword comment should anchor leading; got {leading_all:?}"
        );
        // And it is not trailing anything.
        let trailing_all: Vec<String> = a
            .by_anchor
            .values()
            .flat_map(|c| texts(&c.trailing, p.source()))
            .collect();
        assert!(!trailing_all.contains(&"-- note".to_string()));
    }

    #[test]
    fn empty_call_dangles_comment_inside() {
        let (a, p) = attach("SELECT count(/* c */)");
        let dangling_all: Vec<String> = a
            .by_anchor
            .values()
            .flat_map(|c| texts(&c.dangling, p.source()))
            .collect();
        assert_eq!(dangling_all, vec!["/* c */".to_string()]);
    }

    #[test]
    fn trailing_eol_comment_stays_trailing() {
        let (a, p) = attach("SELECT a -- tag\nFROM t");
        let trailing_all: Vec<String> = a
            .by_anchor
            .values()
            .flat_map(|c| texts(&c.trailing, p.source()))
            .collect();
        assert_eq!(trailing_all, vec!["-- tag".to_string()]);
    }
}
