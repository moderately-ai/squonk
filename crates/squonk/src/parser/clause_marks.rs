// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Out-of-band clause-keyword offsets, recoverable by node and offset.
//!
//! A clause-introducing keyword (`WHERE`, `FROM`, `GROUP BY`, …) owns no AST node
//! and no [`Span`] of its own: it fills a *field* of the node it heads (a `WHERE`
//! fills [`Select::selection`](crate::ast::Select), an `ORDER BY` fills
//! [`Query::order_by`](crate::ast::Query)), and only the clause *body* is a node.
//! Pure span arithmetic therefore cannot place a comment that sits before such a
//! keyword — `WHERE x` / `-- c` / `GROUP BY` renders as `GROUP BY -- c` — which the
//! comment-attachment spike found to be the highest-visibility formatter miss and
//! the one piece of parser cooperation the formatter needs.
//!
//! This side-table records, per clause keyword, the [`NodeId`] of the node whose
//! field the clause fills ([`owner`](ClauseMark::owner)), the clause
//! [`kind`](ClauseMark::kind), and the source byte [`offset`](ClauseMark::offset)
//! of the keyword. It rides the parse root beside the
//! [`TriviaIndex`](crate::tokenizer::TriviaIndex), never on the hot AST nodes, so
//! node sizes and structural equality are unaffected — exactly as trivia does.
//!
//! ## Opt-in, zero-overhead-when-off
//!
//! Capture is gated behind the *same* opt-in as trivia
//! ([`ParseConfig::capture_trivia`](crate::parser::ParseConfig) / the
//! [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia) path): the default parse records
//! nothing, pays a single cold `bool` branch per clause keyword, and ships an empty
//! index that owns an empty `Vec` (no allocation). The pushes and the finalizing
//! owner-patch live in called-and-returned helper methods on the parser, off the
//! hot recursive frames, so recursion-depth headroom is unchanged.
//!
//! ## Sorted by construction
//!
//! The parser consumes tokens strictly left to right, so a clause keyword is
//! recorded at the moment it is eaten, in source order. The resulting index is
//! sorted by offset and each query is a binary search rather than a scan — the same
//! contract [`TriviaIndex`](crate::tokenizer::TriviaIndex) offers.

use crate::ast::{NodeId, Span};

/// The kind of clause a recorded keyword introduces.
///
/// A small parser-crate enum: `NodeId` already crosses the parser/AST boundary, so
/// this needs no AST surface change. Multi-word heads (`GROUP BY`, `ORDER BY`,
/// `CONNECT BY`, `START WITH`, `LATERAL VIEW`) record the byte offset of their first
/// keyword.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClauseKw {
    /// A `FROM` clause head ([`Select::from`](crate::ast::Select)).
    From,
    /// A `WHERE` clause head ([`Select::selection`](crate::ast::Select)).
    Where,
    /// A `CONNECT BY` hierarchical-clause head
    /// ([`Select::connect_by`](crate::ast::Select)).
    ConnectBy,
    /// A `START WITH` hierarchical-clause head
    /// ([`Select::connect_by`](crate::ast::Select)).
    StartWith,
    /// A `GROUP BY` clause head ([`Select::group_by`](crate::ast::Select)).
    GroupBy,
    /// A `HAVING` clause head ([`Select::having`](crate::ast::Select)).
    Having,
    /// A `WINDOW` clause head ([`Select::windows`](crate::ast::Select)).
    Window,
    /// A `QUALIFY` clause head ([`Select::qualify`](crate::ast::Select)).
    Qualify,
    /// A `LATERAL VIEW` clause head
    /// ([`Select::lateral_views`](crate::ast::Select)).
    LateralView,
    /// An `ORDER BY` query-tail clause head
    /// ([`Query::order_by`](crate::ast::Query)).
    OrderBy,
    /// A `LIMIT` query-tail clause head ([`Query::limit`](crate::ast::Query)).
    Limit,
    /// An `OFFSET` query-tail clause head ([`Query::limit`](crate::ast::Query)).
    Offset,
}

/// Owner stamped on a freshly-recorded mark before the owning node's [`NodeId`]
/// exists, patched to the real owner when that node finalizes.
///
/// A clause keyword is consumed *before* the node whose field it fills is built (a
/// `WHERE` is eaten at the top of the SELECT body; the [`Select`](crate::ast::Select)
/// and its id are minted only after the whole body is parsed), so the mark records
/// its kind and offset immediately and carries this placeholder until the owner
/// patches it. `u32::MAX` is unreachable as a real id — the source is `u32`-capped
/// and every node spans at least one byte, so a parse cannot mint `u32::MAX` ids
/// (the same argument the node-id counter relies on) — so a leftover placeholder is
/// a bug, caught by [`ClauseMarkIndex::new`]'s debug assertion.
const PENDING_OWNER: NodeId = match NodeId::new(u32::MAX) {
    Some(id) => id,
    None => panic!("u32::MAX is non-zero"),
};

/// One recorded clause keyword: the node whose field it fills, its kind, and its
/// source byte offset.
///
/// The offset is the start byte of the keyword (the first keyword for a multi-word
/// head), so the keyword text slices back out of the root's source and a comment's
/// attachment to the clause is recoverable by comparing against this offset — the
/// gap pure span arithmetic leaves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClauseMark {
    owner: NodeId,
    kind: ClauseKw,
    offset: u32,
}

impl ClauseMark {
    /// Record a keyword's kind and offset with the placeholder owner, to be patched
    /// when the owning node finalizes.
    pub(super) fn pending(kind: ClauseKw, offset: u32) -> Self {
        Self {
            owner: PENDING_OWNER,
            kind,
            offset,
        }
    }

    /// Whether this mark still carries the placeholder owner (unpatched).
    ///
    /// A finalizing owner patches only the still-pending marks in its own range, so
    /// a mark a nested owner already claimed is skipped rather than re-owned.
    pub(super) fn owner_is_pending(&self) -> bool {
        self.owner == PENDING_OWNER
    }

    /// Stamp the real owner onto a pending mark.
    pub(super) fn set_owner(&mut self, owner: NodeId) {
        self.owner = owner;
    }

    /// The [`NodeId`] of the node whose field this clause fills.
    pub fn owner(&self) -> NodeId {
        self.owner
    }

    /// The kind of clause this keyword introduces.
    pub fn kind(&self) -> ClauseKw {
        self.kind
    }

    /// The source byte offset of the clause keyword (its first keyword for a
    /// multi-word head).
    pub fn offset(&self) -> u32 {
        self.offset
    }
}

/// An offset-sorted side-table of recorded clause keywords.
///
/// Built from the parser's source-order recording, so the marks are sorted by
/// [`offset`](ClauseMark::offset), which makes every offset/span query a binary
/// search (see [`in_span`](Self::in_span)). A `ClauseMarkIndex` is the recovery
/// surface on the parse root ([`Parsed::clause_marks`](crate::Parsed::clause_marks));
/// an empty index (the default, off path) owns an empty `Vec` and never allocates.
#[derive(Clone, Default, Debug)]
pub struct ClauseMarkIndex {
    /// Sorted by `offset` (parser records in source order).
    marks: Vec<ClauseMark>,
}

impl ClauseMarkIndex {
    /// Wrap source-order clause marks into a queryable index.
    ///
    /// Crate-internal: the only producer is the parser's finalizing drain, which
    /// records in source order and patches every mark's owner before this wraps
    /// them, so both the sorted invariant the query methods rely on and the
    /// no-placeholder-leaks invariant hold. The debug assertion pins both.
    pub(super) fn new(marks: Vec<ClauseMark>) -> Self {
        debug_assert!(
            marks
                .windows(2)
                .all(|pair| pair[0].offset <= pair[1].offset),
            "clause marks must be recorded sorted by offset",
        );
        debug_assert!(
            marks.iter().all(|mark| !mark.owner_is_pending()),
            "every clause mark must have its owner patched before finalizing",
        );
        Self { marks }
    }

    /// Every recorded clause mark, in source (offset) order.
    pub fn all(&self) -> &[ClauseMark] {
        &self.marks
    }

    /// The number of recorded clause marks.
    pub fn len(&self) -> usize {
        self.marks.len()
    }

    /// True when no clause mark was recorded (the default, off path).
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// The clause marks whose keyword offset lies within `span`
    /// (`span.start() <= offset < span.end()`), as a sub-slice.
    ///
    /// Two binary searches bound the contiguous block, so no mark is copied. A
    /// synthetic span recovers nothing. Passing a node's own span recovers exactly
    /// the clause keywords that node heads — the query a formatter runs to place a
    /// comment sitting before an otherwise node-less clause keyword.
    pub fn in_span(&self, span: Span) -> &[ClauseMark] {
        if span.is_synthetic() {
            return &[];
        }
        let lo = self
            .marks
            .partition_point(|mark| mark.offset < span.start());
        let hi = self.marks.partition_point(|mark| mark.offset < span.end());
        if lo >= hi {
            return &[];
        }
        &self.marks[lo..hi]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an index from `(kind, offset)` pairs in the source order the parser
    /// would record them, with a distinct real owner stamped on each (so the
    /// no-placeholder invariant holds).
    fn index(marks: &[(ClauseKw, u32)]) -> ClauseMarkIndex {
        let marks = marks
            .iter()
            .enumerate()
            .map(|(i, &(kind, offset))| {
                let mut mark = ClauseMark::pending(kind, offset);
                mark.set_owner(NodeId::new(i as u32 + 1).expect("non-zero owner"));
                mark
            })
            .collect();
        ClauseMarkIndex::new(marks)
    }

    #[test]
    fn empty_index_allocates_nothing_and_queries_empty() {
        let empty = ClauseMarkIndex::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.all(), &[]);
        assert!(empty.in_span(Span::new(0, 100)).is_empty());
    }

    #[test]
    fn in_span_returns_only_contained_marks() {
        use ClauseKw::{From, GroupBy, Where};
        // `SELECT * FROM t WHERE a GROUP BY b`: FROM@9, WHERE@16, GROUP BY@24.
        let idx = index(&[(From, 9), (Where, 16), (GroupBy, 24)]);

        // A span covering the WHERE..GROUP gap keeps only the marks inside it.
        let inner = idx.in_span(Span::new(16, 24));
        assert_eq!(
            inner.iter().map(ClauseMark::kind).collect::<Vec<_>>(),
            [Where],
        );

        // The whole statement returns every mark; a synthetic span returns nothing.
        assert_eq!(idx.in_span(Span::new(0, 33)).len(), 3);
        assert!(idx.in_span(Span::SYNTHETIC).is_empty());

        // The end bound is exclusive: a mark exactly at the span end is excluded.
        assert!(idx.in_span(Span::new(0, 9)).is_empty());
        assert_eq!(idx.in_span(Span::new(0, 10)).len(), 1);
    }
}
