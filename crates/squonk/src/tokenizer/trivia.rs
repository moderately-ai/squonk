// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Out-of-band trivia capture: comment and whitespace spans, recoverable by offset.
//!
//! Trivia (whitespace and comments) is *never* part of the token stream — the
//! grammar only ever sees real lexemes. But formatters, linters, and
//! diagnostics need to recover the comments and whitespace between tokens. This
//! module is the seam that keeps both true: the scanner discards trivia from the
//! token stream as before, and *optionally* records each trivia run's source
//! [`Span`] into a side-table that travels on the parse root, queryable by offset.
//!
//! ## Opt-in, zero-overhead-when-off
//!
//! Recording is opt-in because capturing every whitespace/comment span has a real
//! cost (a `Vec` plus a push per token gap) that the hot lexer path must not pay by
//! default. The cost is gated through a [`TriviaSink`] generic: the default
//! [`NoTrivia`] sink's `const RECORDING = false` folds the capture — and the
//! offset reads feeding it — away at compile time, so a parse that does not ask for
//! trivia lexes byte-for-byte as it did before this seam existed. A tool that wants
//! trivia opts in (`ParseConfig::capture_trivia` / `tokenize_with_trivia`), which swaps in a
//! recording sink and pays for what it captures.
//!
//! ## Sorted by construction
//!
//! The scanner advances the byte cursor monotonically, so trivia is recorded in
//! source order: the resulting [`TriviaIndex`] is sorted by offset and
//! non-overlapping *by construction*, which is what lets every query be a binary
//! search rather than a scan.

use crate::ast::Span;

/// The lexical category of a captured trivia run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TriviaKind {
    /// A `--` or `#` line comment, up to (but not including) the line's newline.
    LineComment,
    /// A `/* … */` block comment (nesting per dialect data). Also the
    /// comment-syntax pieces of a MySQL versioned comment: a wholly-discarded
    /// `/*!NNNNN … */` region is one run, while an *included* region records its
    /// `/*!NNNNN` opener and `*/` closer as separate runs with the live body
    /// tokens between them — so the version number is offset-recoverable even
    /// though it is not a token.
    BlockComment,
    /// A maximal run of whitespace bytes.
    Whitespace,
}

/// A single captured trivia run: its lexical [`kind`](TriviaKind) and the source
/// [`Span`] it occupies.
///
/// The span slices back to the exact trivia text (`&source[span]`), the same
/// zero-copy contract the token stream uses — the comment/whitespace
/// text is never copied, only its byte range is kept.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TriviaRange {
    span: Span,
    kind: TriviaKind,
}

impl TriviaRange {
    /// Pair a trivia `kind` with the source `span` it covers.
    pub fn new(kind: TriviaKind, span: Span) -> Self {
        Self { span, kind }
    }

    /// The source byte range this trivia occupies.
    pub fn span(&self) -> Span {
        self.span
    }

    /// The lexical category of this trivia.
    pub fn kind(&self) -> TriviaKind {
        self.kind
    }
}

/// An offset-sorted, non-overlapping side-table of captured trivia.
///
/// Built from the scanner's source-order recording, so the ranges are sorted by
/// [`Span::start`] and never overlap. That invariant makes every query a binary
/// search over the slice (see [`in_span`](Self::in_span) /
/// [`before`](Self::before)), so recovering the trivia around a token is `O(log n)`
/// rather than a linear scan.
///
/// A `TriviaIndex` is the recovery surface for both the parse-root path
/// ([`Parsed::trivia`](crate::Parsed::trivia)) and the eager tokenizer-output path
/// ([`tokenize_with_trivia`](crate::tokenizer::tokenize_with_trivia)). An empty
/// index (the default, off path) owns an empty `Vec` and so never allocates.
#[derive(Clone, Default, Debug)]
pub struct TriviaIndex {
    /// Sorted by `span.start()`, non-overlapping (scanner invariant).
    ranges: Vec<TriviaRange>,
}

impl TriviaIndex {
    /// Wrap source-order trivia into a queryable index.
    ///
    /// Crate-internal: the only producers are the scanner-driven capture paths,
    /// which record in source order, so the sorted/non-overlapping invariant the
    /// query methods rely on holds. The debug assertion pins that contract.
    pub(crate) fn new(ranges: Vec<TriviaRange>) -> Self {
        debug_assert!(
            ranges.windows(2).all(|pair| {
                let (prev, next) = (pair[0].span, pair[1].span);
                prev.start() <= next.start() && prev.end() <= next.start()
            }),
            "trivia must be recorded sorted and non-overlapping",
        );
        Self { ranges }
    }

    /// Every captured trivia run, in source order.
    pub fn all(&self) -> &[TriviaRange] {
        &self.ranges
    }

    /// The number of captured trivia runs.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// True when no trivia was captured (the default, off path).
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Every trivia run fully contained in `span`, as a sub-slice.
    ///
    /// "Contained" is `start >= span.start()` and `end <= span.end()`; a run that
    /// merely straddles a boundary is excluded. A synthetic span recovers nothing.
    /// Two binary searches bound the contiguous block, so no run is copied.
    pub fn in_span(&self, span: Span) -> &[TriviaRange] {
        if span.is_synthetic() {
            return &[];
        }
        // `lo`: first run starting at/after the span start. `hi`: first run ending
        // past the span end. Both predicates are monotonic over the sorted ranges
        // (start and end are each non-decreasing), so `[lo, hi)` is exactly the
        // fully-contained block.
        let lo = self
            .ranges
            .partition_point(|r| r.span.start() < span.start());
        let hi = self.ranges.partition_point(|r| r.span.end() <= span.end());
        if lo >= hi {
            return &[];
        }
        &self.ranges[lo..hi]
    }

    /// The contiguous run of trivia immediately preceding `offset` — the "leading
    /// trivia" of a token that starts at `offset`.
    ///
    /// Walks back from `offset` over trivia that abuts it with no gap (the scanner
    /// emits the whitespace/comments between two tokens as a back-to-back chain), so
    /// the result is every comment and whitespace run attached to the front of the
    /// token at `offset`. Empty when a real token (not trivia) directly precedes
    /// `offset`.
    pub fn before(&self, offset: u32) -> &[TriviaRange] {
        // The runs ending at or before `offset` are a prefix `[0, end)`; the leading
        // chain is the suffix of that prefix whose ranges meet end-to-start back to
        // `offset`.
        let end = self.ranges.partition_point(|r| r.span.end() <= offset);
        let mut start = end;
        let mut boundary = offset;
        while start > 0 {
            let span = self.ranges[start - 1].span;
            if span.end() != boundary {
                break;
            }
            boundary = span.start();
            start -= 1;
        }
        &self.ranges[start..end]
    }
}

/// A destination for trivia spans the scanner discards from the token stream.
///
/// The generic seam behind the opt-in/zero-overhead-when-off contract (see the
/// module docs): the scanner is monomorphized over the sink, so the off path
/// ([`NoTrivia`]) compiles to the pre-trivia lexer with no branch, `Vec`, or push,
/// while a recording sink captures spans. `RECORDING` lets the scanner drop the
/// whole capture — including the offset reads feeding it — at compile time on the
/// off path, rather than relying on the optimizer to prove the no-op dead.
pub(crate) trait TriviaSink {
    /// Whether this sink records; `false` makes capture compile-time dead.
    const RECORDING: bool;

    /// Record one trivia run. A no-op for [`NoTrivia`].
    fn record(&mut self, range: TriviaRange);
}

/// The default sink: discards trivia, recording nothing.
///
/// A zero-sized type whose `record` is an inlinable no-op and whose
/// `RECORDING = false` deletes the scanner's capture block outright, so the default
/// parse path is exactly the pre-trivia hot loop.
pub(crate) struct NoTrivia;

impl TriviaSink for NoTrivia {
    const RECORDING: bool = false;

    #[inline(always)]
    fn record(&mut self, _range: TriviaRange) {}
}

/// The recording sink. Trivia arrives in source order, so a plain push keeps the
/// `Vec` sorted and non-overlapping — the [`TriviaIndex`] invariant.
impl TriviaSink for Vec<TriviaRange> {
    const RECORDING: bool = true;

    #[inline]
    fn record(&mut self, range: TriviaRange) {
        self.push(range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an index from `(kind, start, end)` triples, in the source order the
    /// scanner would record them.
    fn index(runs: &[(TriviaKind, u32, u32)]) -> TriviaIndex {
        TriviaIndex::new(
            runs.iter()
                .map(|&(kind, start, end)| TriviaRange::new(kind, Span::new(start, end)))
                .collect(),
        )
    }

    #[test]
    fn empty_index_allocates_nothing_and_queries_empty() {
        let empty = TriviaIndex::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.all(), &[]);
        assert!(empty.in_span(Span::new(0, 100)).is_empty());
        assert!(empty.before(0).is_empty());
    }

    #[test]
    fn in_span_returns_only_fully_contained_runs() {
        use TriviaKind::{BlockComment, LineComment, Whitespace};
        // `a /*c*/ b -- line\n d`: whitespace and comments tiling the gaps.
        let idx = index(&[
            (Whitespace, 1, 2),    // after `a`
            (BlockComment, 2, 7),  // /*c*/
            (Whitespace, 7, 8),    // before `b`
            (Whitespace, 9, 10),   // after `b`
            (LineComment, 10, 17), // -- line
            (Whitespace, 17, 19),  // newline + space
        ]);

        // A span covering the middle keeps only runs wholly inside it.
        let inner = idx.in_span(Span::new(2, 10));
        assert_eq!(
            inner.iter().map(TriviaRange::kind).collect::<Vec<_>>(),
            [BlockComment, Whitespace, Whitespace],
        );

        // Boundary-straddling runs are excluded, not clipped: `[3, 7)` cuts through
        // the block comment `[2, 7)` (starts before 3) and the whitespace `[7, 8)`
        // (ends after 7), so neither is fully contained.
        assert!(idx.in_span(Span::new(3, 7)).is_empty());

        // The whole source returns everything; a synthetic span returns nothing.
        assert_eq!(idx.in_span(Span::new(0, 19)).len(), 6);
        assert!(idx.in_span(Span::SYNTHETIC).is_empty());
    }

    #[test]
    fn before_collects_the_contiguous_leading_chain() {
        use TriviaKind::{BlockComment, LineComment, Whitespace};
        // Two tokens with a whitespace+comment+whitespace chain between them, then a
        // token that directly abuts the next (no leading trivia).
        let idx = index(&[
            (Whitespace, 5, 6),    // after first token (ends at 6)
            (BlockComment, 6, 11), // /* … */
            (Whitespace, 11, 12),  // up to the token at 12
            (LineComment, 20, 27), // a detached later comment
        ]);

        // The token at 12 has the full back-to-back chain as its leading trivia.
        let leading = idx.before(12);
        assert_eq!(
            leading.iter().map(TriviaRange::kind).collect::<Vec<_>>(),
            [Whitespace, BlockComment, Whitespace],
        );

        // An offset a real token directly precedes (a gap before it) has no leading
        // trivia: 20 is preceded by the token ending the chain at 12, not trivia.
        assert!(idx.before(20).is_empty());

        // The detached comment is itself leading trivia for offset 27.
        assert_eq!(
            idx.before(27),
            &[TriviaRange::new(LineComment, Span::new(20, 27))]
        );

        // Nothing ends at 5, so there is no leading chain there.
        assert!(idx.before(5).is_empty());
    }
}
