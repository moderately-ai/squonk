// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Hand-written, zero-copy SQL tokenizer (byte-offset cursor + token buffer).
//!
//! The tokenizer turns source text into [`Token`]s, each a lexical [`TokenKind`]
//! plus the byte [`Span`] it occupies. Tokens are `Copy` and borrow nothing:
//! text is recovered as `&source[span]` on demand, so token buffers are
//! cache-dense and outlive the scan borrow.
//!
//! Why hand-written and not `logos`: the non-regular cases — nested `/* … */`
//! block comments and PostgreSQL `$tag$ … $tag$` dollar-quoting — cannot be
//! expressed by a DFA. The hot loop is driven by the shared `[u8; 256]`
//! lexer-class table from `squonk-ast`.
//!
//! ```
//! use squonk::ast::Keyword;
//! use squonk::tokenizer::{tokenize, TokenKind};
//!
//! let src = "SELECT 1";
//! let tokens = tokenize(src).expect("valid SQL lexes");
//! assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Select));
//! assert_eq!(&src[tokens[0].span.start() as usize..tokens[0].span.end() as usize], "SELECT");
//! ```
//!
//! ## Errors
//!
//! [`tokenize`] is fail-fast: the first lexical problem (unterminated string,
//! quoted identifier, block comment, or dollar-quote; a stray byte) stops the
//! scan and returns a [`LexError`]. That error type is intentionally local to
//! this module — `squonk::error` is built concurrently by `m1-parse-error`,
//! and the two are reconciled later in `m1-engine`.
//!
//! ## Trivia
//!
//! Whitespace and comments are skipped, never emitted, but stay offset-recoverable:
//! gaps between adjacent token spans are exactly the trivia. A tool that needs the
//! comments/whitespace themselves can *opt in* to capturing each trivia run's
//! [`Span`] into a [`TriviaIndex`] — [`tokenize_with_trivia`] returns one alongside
//! the tokens, and `ParseConfig::capture_trivia` homes one on the parse root. Capture is off
//! by default so the hot lexer path pays nothing for it.

mod cursor;
mod error;
mod scan;
mod token;
mod trivia;

pub use cursor::Cursor;
pub use error::{LexError, LexErrorKind};
pub use token::{Operator, Punctuation, Token, TokenKind};
pub use trivia::{TriviaIndex, TriviaKind, TriviaRange};

pub(crate) use trivia::{NoTrivia, TriviaSink};

use crate::ast::Span;
use crate::ast::dialect::FeatureSet;

const STREAM_BUFFER_MIN_INITIAL_CAPACITY: usize = 8;
const STREAM_BUFFER_MAX_INITIAL_CAPACITY: usize = 64;
const STREAM_BUFFER_RETAINED_CAPACITY: usize = 4096;

/// Tokenize `src` eagerly into a flat token buffer.
///
/// Trivia (whitespace and comments) is skipped. On the first lexical error the
/// scan stops and returns a [`LexError`]. An eager `Vec` is the M1 contract; a
/// lazy/streaming driver over the same cursor is a later optimization.
///
/// # Errors
///
/// Returns [`LexError`] for an unterminated string/quoted-identifier/block-
/// comment/dollar-quote, a stray byte, or a source longer than `u32::MAX` bytes
/// (whose offsets would not fit the `u32` spans this tokenizer uses).
pub fn tokenize(src: &str) -> Result<Vec<Token>, LexError> {
    tokenize_with(src, &FeatureSet::ANSI)
}

/// Tokenize `src` eagerly using `features` for dialect-owned lexical data.
///
/// This is the parser-facing entry point: keyword recognition is shared, while
/// byte-class dispatch comes from the dialect feature set. The parser itself
/// uses an internal buffered cursor to scan lazily; [`tokenize_with`] remains
/// the public eager tokenizer for callers that want a full token vector.
///
/// # Errors
///
/// Returns the same lexical errors as [`tokenize`].
pub fn tokenize_with(src: &str, features: &FeatureSet) -> Result<Vec<Token>, LexError> {
    // Spans are `u32`; a longer source could not be addressed. Huge inputs are
    // served by statement streaming (ADR-0005), not by widening spans (ADR-0002).
    if u32::try_from(src.len()).is_err() {
        return Err(LexError::new(LexErrorKind::SourceTooLong, Span::new(0, 0)));
    }

    let mut cursor = Cursor::new(src);
    // Most tokens are a few bytes wide; this hint avoids the early reallocations
    // without meaningfully over-allocating, and is cheap to revise later.
    let mut tokens = Vec::with_capacity(src.len() / 8 + 8);

    // The default sink discards trivia at compile time (zero overhead, ADR-0005);
    // callers wanting the spans use `tokenize_with_trivia`.
    let mut state = scan::LexState::default();
    while let Some(token) = scan::next_token(&mut cursor, features, &mut NoTrivia, &mut state)? {
        tokens.push(token);
    }

    Ok(tokens)
}

/// Tokenize `src` eagerly, also recovering an offset-sorted [`TriviaIndex`] of the
/// skipped comments and whitespace.
///
/// The tokenizer-output recovery path (the parse-root path is
/// [`ParseConfig::capture_trivia`](crate::ParseConfig::capture_trivia)): the returned tokens are
/// *identical* to [`tokenize_with`]'s — trivia stays out of the token stream — while
/// the [`TriviaIndex`] carries each `--`/`#`/`/* */` comment and whitespace run as a
/// queryable span. This pays the capture cost [`tokenize_with`] avoids, so reach for
/// it only when a formatter/linter/diagnostic actually needs the trivia.
///
/// # Errors
///
/// Returns the same lexical errors as [`tokenize`].
pub fn tokenize_with_trivia(
    src: &str,
    features: &FeatureSet,
) -> Result<(Vec<Token>, TriviaIndex), LexError> {
    if u32::try_from(src.len()).is_err() {
        return Err(LexError::new(LexErrorKind::SourceTooLong, Span::new(0, 0)));
    }

    let mut cursor = Cursor::new(src);
    let mut tokens = Vec::with_capacity(src.len() / 8 + 8);
    let mut trivia: Vec<TriviaRange> = Vec::new();

    let mut state = scan::LexState::default();
    while let Some(token) = scan::next_token(&mut cursor, features, &mut trivia, &mut state)? {
        tokens.push(token);
    }

    Ok((tokens, TriviaIndex::new(trivia)))
}

/// A parser-facing token cursor that grows on demand.
///
/// The public [`TokenCursor`] below remains the zero-allocation cursor over an
/// already-tokenized slice. `BufferedTokenCursor` is the streaming parser seam:
/// it drives the byte cursor only as grammar lookahead asks for tokens, while
/// retaining random access within the current statement so checkpoints can
/// rewind without rescanning.
#[derive(Debug)]
pub(crate) struct BufferedTokenCursor<'s> {
    source: BufferedTokenSource<'s>,
    features: Option<FeatureSet>,
    buffer: Vec<Token>,
    buffer_start: usize,
    pos: usize,
    /// Out-of-band trivia capture, opt-in: `Some` records each skipped
    /// comment/whitespace run, `None` (the default) discards them through the
    /// compile-time-dead [`NoTrivia`] sink. Unlike `buffer`, this is *not* drained
    /// between statements ([`discard_consumed`](Self::discard_consumed)) — a parse
    /// root keeps the whole source's trivia — which is why trivia capture is the
    /// collecting trivia-capture path, not the bounded streaming iterator.
    trivia: Option<Vec<TriviaRange>>,
}

#[derive(Debug)]
enum BufferedTokenSource<'s> {
    Slice(&'s [Token]),
    Stream {
        cursor: Cursor<'s>,
        reached_eof: bool,
        /// Cross-token lexical state (an open versioned-comment region) — owned
        /// here so it survives across `scan_loop` refills exactly as the cursor
        /// position does.
        lex: scan::LexState,
    },
}

impl<'s> BufferedTokenCursor<'s> {
    /// Create a cursor over an already-tokenized slice.
    pub(crate) fn from_tokens(tokens: &'s [Token]) -> Self {
        Self {
            source: BufferedTokenSource::Slice(tokens),
            features: None,
            buffer: Vec::new(),
            buffer_start: 0,
            pos: 0,
            trivia: None,
        }
    }

    /// Create a streaming cursor over `src` using `features` for lexical data.
    ///
    /// No token is scanned by construction; the first call to [`peek`](Self::peek),
    /// [`peek_nth`](Self::peek_nth), or [`advance`](Self::advance) grows the
    /// buffer just far enough to satisfy that operation. Trivia is discarded; use
    /// [`streaming_with_trivia`](Self::streaming_with_trivia) to capture it.
    pub(crate) fn streaming(src: &'s str, features: &FeatureSet) -> Result<Self, LexError> {
        Self::streaming_inner(src, features, None)
    }

    /// Like [`streaming`](Self::streaming), but records skipped comment/whitespace
    /// runs into an offset-sorted side-table, drained with [`take_trivia`](Self::take_trivia).
    pub(crate) fn streaming_with_trivia(
        src: &'s str,
        features: &FeatureSet,
    ) -> Result<Self, LexError> {
        Self::streaming_inner(src, features, Some(Vec::new()))
    }

    fn streaming_inner(
        src: &'s str,
        features: &FeatureSet,
        trivia: Option<Vec<TriviaRange>>,
    ) -> Result<Self, LexError> {
        if u32::try_from(src.len()).is_err() {
            return Err(LexError::new(LexErrorKind::SourceTooLong, Span::new(0, 0)));
        }

        Ok(Self {
            source: BufferedTokenSource::Stream {
                cursor: Cursor::new(src),
                reached_eof: false,
                lex: scan::LexState::default(),
            },
            features: Some(features.clone()),
            buffer: Vec::new(),
            buffer_start: 0,
            pos: 0,
            trivia,
        })
    }

    /// Take the captured trivia, leaving capture disabled.
    ///
    /// Yields an empty index when capture was off (the default). The scanner records
    /// in source order, so the ranges are already sorted by offset.
    pub(crate) fn take_trivia(&mut self) -> TriviaIndex {
        TriviaIndex::new(self.trivia.take().unwrap_or_default())
    }

    /// The current absolute token index.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// The token at the cursor without advancing, or `None` at end of input.
    pub(crate) fn peek(&mut self) -> Result<Option<Token>, LexError> {
        self.peek_nth(0)
    }

    /// The token `n` positions ahead without advancing.
    pub(crate) fn peek_nth(&mut self, n: usize) -> Result<Option<Token>, LexError> {
        let target = self.pos.saturating_add(n);
        match &self.source {
            BufferedTokenSource::Slice(tokens) => Ok(tokens.get(target).copied()),
            BufferedTokenSource::Stream { .. } => {
                // Fast path: a token already buffered needs no lexing, so skip the
                // `fill_through` feature borrow + scan-loop condition entirely. This
                // is the common case — grammar lookahead re-peeks the current token
                // several times per position (predicate/postfix/infix checks), and
                // each of those hits an already-buffered token.
                //
                // The cursor never rewinds below the retained buffer (`pos >=
                // buffer_start`, asserted in `seek`) and `n >= 0`, so `target >=
                // buffer_start`: `wrapping_sub` is exact and the single `get` bound
                // check both confirms the token is buffered and indexes it. This
                // collapses the old explicit-bound + `checked_sub` + `get` (three
                // branches) into one — the hot path is now a subtract and a bounds
                // check. A target before `buffer_start` (a broken invariant) wraps to
                // a huge offset, `get` misses, and the slow path returns `None`,
                // matching the previous `checked_sub` underflow behaviour.
                let offset = target.wrapping_sub(self.buffer_start);
                if let Some(&token) = self.buffer.get(offset) {
                    return Ok(Some(token));
                }
                self.fill_through(target)?;
                Ok(self.buffered_token(target))
            }
        }
    }

    /// Return the token at the cursor and advance past it, or `None` at end.
    pub(crate) fn advance(&mut self) -> Result<Option<Token>, LexError> {
        let token = self.peek()?;
        if token.is_some() {
            self.pos += 1;
        }
        Ok(token)
    }

    /// True once every token has been consumed.
    pub(crate) fn is_eof(&mut self) -> Result<bool, LexError> {
        Ok(self.peek()?.is_none())
    }

    /// Jump to an absolute token index for restoring a speculation checkpoint.
    pub(crate) fn seek(&mut self, pos: usize) {
        debug_assert!(
            pos >= self.buffer_start,
            "cannot rewind before the retained token buffer",
        );
        self.pos = pos;
    }

    /// The token immediately before the cursor, if it is still retained.
    pub(crate) fn preceding(&self) -> Option<Token> {
        let previous = self.pos.checked_sub(1)?;
        match &self.source {
            BufferedTokenSource::Slice(tokens) => tokens.get(previous).copied(),
            BufferedTokenSource::Stream { .. } => self.buffered_token(previous),
        }
    }

    /// Drop consumed streaming tokens once the parser is between statements.
    ///
    /// Checkpoints never survive across statement boundaries, so consumed tokens
    /// can be discarded there without breaking rewind. Keeping a small retained
    /// capacity avoids carrying a giant statement's token allocation through the
    /// rest of a long script.
    pub(crate) fn discard_consumed(&mut self) {
        if !matches!(&self.source, BufferedTokenSource::Stream { .. }) {
            return;
        }

        let consumed = self
            .pos
            .saturating_sub(self.buffer_start)
            .min(self.buffer.len());
        if consumed == 0 {
            return;
        }

        self.buffer.drain(..consumed);
        self.buffer_start += consumed;

        if self.buffer.is_empty() && self.buffer.capacity() > STREAM_BUFFER_RETAINED_CAPACITY {
            self.buffer.shrink_to(STREAM_BUFFER_MAX_INITIAL_CAPACITY);
        }
    }

    #[cfg(test)]
    pub(crate) fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    fn fill_through(&mut self, target: usize) -> Result<(), LexError> {
        // Move the trivia buffer out so the scan loop can borrow `self` for the
        // cursor/buffer while still recording into the (now-local) sink — `&mut
        // self.trivia` and `&mut self` could not coexist otherwise. Picking the sink
        // here, once, also keeps the per-token scan loop at one statically-known
        // monomorphization: the `None` arm hands `scan_loop` the compile-time-dead
        // `NoTrivia` sink, so the off path is the pre-trivia loop exactly (no branch,
        // no `Vec`, no push); the `Some` arm records into the side-table (ADR-0005).
        // Moving an `Option<Vec>` is three words and never touches the elements.
        let mut trivia = self.trivia.take();
        let result = match &mut trivia {
            Some(sink) => self.scan_loop(target, sink),
            None => self.scan_loop(target, &mut NoTrivia),
        };
        self.trivia = trivia;
        result
    }

    /// The per-token scan loop, generic over the trivia sink. Fills the buffer up to
    /// `target` (or end of input), recording any skipped trivia into `trivia`.
    fn scan_loop<S: TriviaSink>(&mut self, target: usize, trivia: &mut S) -> Result<(), LexError> {
        let BufferedTokenSource::Stream {
            cursor,
            reached_eof,
            lex,
        } = &mut self.source
        else {
            return Ok(());
        };
        let features = self
            .features
            .as_ref()
            .expect("streaming token cursors carry dialect features");

        while !*reached_eof && self.buffer_start + self.buffer.len() <= target {
            match scan::next_token(cursor, features, trivia, lex)? {
                Some(token) => {
                    if self.buffer.capacity() == 0 {
                        self.buffer
                            .reserve_exact(stream_buffer_initial_capacity(cursor.src()));
                    }
                    self.buffer.push(token);
                }
                None => *reached_eof = true,
            }
        }
        Ok(())
    }

    fn buffered_token(&self, index: usize) -> Option<Token> {
        let offset = index.checked_sub(self.buffer_start)?;
        self.buffer.get(offset).copied()
    }
}

fn stream_buffer_initial_capacity(src: &str) -> usize {
    (src.len() / 4 + 8).clamp(
        STREAM_BUFFER_MIN_INITIAL_CAPACITY,
        STREAM_BUFFER_MAX_INITIAL_CAPACITY,
    )
}

/// A random-access cursor over a tokenized buffer.
///
/// This is the eager slice cursor: it can [`peek`](Self::peek) and
/// [`advance`](Self::advance) like a stream, but also look ahead with
/// [`peek_nth`](Self::peek_nth) and rewind with [`seek`](Self::seek). It borrows
/// the buffer and copies the `Copy` tokens out, so it allocates nothing. The
/// parser's production path uses the internal lazy cursor above.
#[derive(Clone, Copy, Debug)]
pub struct TokenCursor<'t> {
    tokens: &'t [Token],
    pos: usize,
}

impl<'t> TokenCursor<'t> {
    /// Create a cursor at the first token of `tokens`.
    pub fn new(tokens: &'t [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// The current token index.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Number of tokens not yet consumed.
    pub fn remaining(&self) -> usize {
        self.tokens.len() - self.pos.min(self.tokens.len())
    }

    /// True once every token has been consumed.
    pub fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// The token at the cursor without advancing, or `None` at end of buffer.
    pub fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    /// The token `n` positions ahead without advancing.
    pub fn peek_nth(&self, n: usize) -> Option<Token> {
        self.pos
            .checked_add(n)
            .and_then(|index| self.tokens.get(index))
            .copied()
    }

    /// Return the token at the cursor and advance past it, or `None` at end.
    pub fn advance(&mut self) -> Option<Token> {
        let token = self.peek();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Jump to an absolute token index (for restoring a speculation checkpoint).
    ///
    /// Indices past the end are allowed and simply leave the cursor at EOF.
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Keyword;
    use crate::ast::dialect::lex_class::{CLASS_IDENTIFIER_CONTINUE, CLASS_IDENTIFIER_START};
    use crate::ast::dialect::{
        ByteClasses, CommentSyntax, ExpressionSyntax, FeatureDelta, FeatureSet, IdentifierQuote,
        IdentifierSyntax, NumericLiteralSyntax, OperatorSyntax, ParameterSyntax, QueryTailSyntax,
        SessionVariableSyntax, StringLiteralSyntax,
    };

    /// Vertical tab (`0x0b`) is per-dialect whitespace data with three distinct,
    /// engine-measured shapes (`whitespace-vertical-tab-sqlite-duckdb`), so one
    /// parametric matrix pins the class across every preset. Each cell is whether
    /// `tokenize_with` accepts (`Ok`) — the level at which the `0x0b` accept/reject is
    /// decided — under that preset. Oracle-verified: PostgreSQL/MySQL fold it as
    /// ordinary whitespace; SQLite folds it only as a run *continuation* (rides an open
    /// whitespace run, cannot start one); DuckDB folds it only as statement *trim*
    /// (leading/trailing of a `;`-segment, a hard error interior to a statement);
    /// ANSI keeps it strict everywhere.
    #[test]
    fn vertical_tab_whitespace_class_is_per_dialect() {
        struct Case {
            input: &'static str,
            ansi: bool,
            postgres: bool,
            mysql: bool,
            sqlite: bool,
            duckdb: bool,
        }
        // Columns: ANSI, PostgreSQL, MySQL, SQLite, DuckDB (accept = tokenizes clean).
        const CASES: &[Case] = &[
            // Lone `\v`: PG/MySQL empty; SQLite can't start a run (reject); DuckDB trims
            // it as an empty statement (accept).
            Case {
                input: "\x0b",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: true,
            },
            // Leading `\v` before a statement: SQLite still can't start a run; DuckDB
            // trims the leading edge.
            Case {
                input: "\x0bSELECT 1",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: true,
            },
            // Trailing `\v` after a statement: DuckDB trims the trailing edge; SQLite
            // rejects (the `\v` follows `1`, a non-whitespace byte).
            Case {
                input: "SELECT 1\x0b",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: true,
            },
            // DuckDB treats comments as content for boundary trim: the middle `\v`
            // lies between two comments and is therefore not at a segment edge.
            Case {
                input: ";\x0b/**/\x0b/***/\n;\x0b",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: false,
            },
            // Trailing `\v` after a `;`: a leading edge of the (empty) next statement.
            Case {
                input: "SELECT 1;\x0b",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: true,
            },
            // `\v` between two content tokens: interior — DuckDB rejects; SQLite rejects
            // (the `\v` would start a run after `SELECT`); PG/MySQL fold it.
            Case {
                input: "SELECT\x0b1",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: false,
            },
            // `\v` riding an open whitespace run (space then `\v`): the SQLite-distinctive
            // accept — a run continuation. DuckDB still rejects (interior, adjacency to a
            // real space does not rescue it).
            Case {
                input: "SELECT \x0b1",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: true,
                duckdb: false,
            },
            // `\v` then a space, interior: SQLite rejects (the `\v` starts the run), and so
            // does DuckDB (interior boundary byte).
            Case {
                input: "SELECT\x0b 1",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: false,
                duckdb: false,
            },
            // Whole-input blank run reducing to empty: SQLite rides the run from the space;
            // DuckDB trims. Both accept.
            Case {
                input: " \x0b ",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: true,
                duckdb: true,
            },
            // Interior `\v` flanked by real spaces: DuckDB rejects (boundary byte with
            // content on both sides); SQLite accepts (rides the run).
            Case {
                input: "SELECT 1 \x0b FROM t",
                ansi: false,
                postgres: true,
                mysql: true,
                sqlite: true,
                duckdb: false,
            },
        ];
        for case in CASES {
            let expect = [
                (FeatureSet::ANSI, case.ansi, "ANSI"),
                (FeatureSet::POSTGRES, case.postgres, "PostgreSQL"),
                (FeatureSet::MYSQL, case.mysql, "MySQL"),
                (FeatureSet::SQLITE, case.sqlite, "SQLite"),
                (FeatureSet::DUCKDB, case.duckdb, "DuckDB"),
            ];
            for (features, accepts, name) in expect {
                assert_eq!(
                    tokenize_with(case.input, &features).is_ok(),
                    accepts,
                    "{name} tokenization of {:?} should {}",
                    case.input,
                    if accepts { "accept" } else { "reject" },
                );
            }
        }
    }

    #[test]
    fn token_is_a_twelve_byte_copy_value() {
        // ADR-0005: a `Token` is a borrow-free {kind, span} pair; pinning it at 12
        // bytes keeps the token stream cache-dense. `size_of::<Token>()` was
        // unpinned anywhere in the nextest suite, so this is its governed pin.
        assert_eq!(std::mem::size_of::<Token>(), 12);
    }

    /// Slice a token's span back out of the source.
    fn text<'s>(src: &'s str, token: &Token) -> &'s str {
        &src[token.span.start() as usize..token.span.end() as usize]
    }

    /// `(kind, sliced text)` for every token, the spine of most assertions.
    fn lexed(src: &str) -> Vec<(TokenKind, &str)> {
        tokenize(src)
            .expect("expected a clean tokenization")
            .iter()
            .map(|token| (token.kind, text(src, token)))
            .collect()
    }

    #[test]
    fn tokenizes_a_representative_select() {
        use Operator::{NotEq, Plus};
        use Punctuation::Comma;

        // `d`, `b`, `u`, `x` are not keywords in the full inventory, so they lex as
        // `Word`; `a`/`t` would now lex as the SQL:2016 single-letter keywords.
        let src = "SELECT d, b + 1 FROM u WHERE x <> 'it''s'";
        let tokens = tokenize(src).expect("clean tokenization");

        let expected = [
            (TokenKind::Keyword(Keyword::Select), "SELECT"),
            (TokenKind::Word, "d"),
            (TokenKind::Punctuation(Comma), ","),
            (TokenKind::Word, "b"),
            (TokenKind::Operator(Plus), "+"),
            (TokenKind::Number, "1"),
            (TokenKind::Keyword(Keyword::From), "FROM"),
            (TokenKind::Word, "u"),
            (TokenKind::Keyword(Keyword::Where), "WHERE"),
            (TokenKind::Word, "x"),
            (TokenKind::Operator(NotEq), "<>"),
            (TokenKind::String, "'it''s'"),
        ];

        assert_eq!(tokens.len(), expected.len());
        for (token, (kind, slice)) in tokens.iter().zip(expected) {
            assert_eq!(token.kind, kind);
            assert_eq!(text(src, token), slice);
        }

        // Spell out a couple of spans numerically to pin down offsets directly.
        assert_eq!(tokens[0].span, Span::new(0, 6)); // SELECT
        assert_eq!(tokens.last().expect("non-empty").span, Span::new(34, 41)); // 'it''s'
    }

    #[test]
    fn every_span_slices_back_to_exact_source() {
        // Mixes words, numbers, operators, punctuation, both quote forms, a
        // dollar-quote, and trivia between every pair of tokens.
        let src = "select  c1 ,\t3.14 -- note\n+ \"Odd\"\nfrom /* x */ $$body$$ ;";
        let tokens =
            tokenize_with(src, &FeatureSet::POSTGRES).expect("clean PostgreSQL tokenization");

        assert!(!tokens.is_empty());
        for token in &tokens {
            let slice = text(src, token);
            assert_eq!(slice.len() as u32, token.span.len());
            assert!(!slice.is_empty(), "real tokens are never empty");
        }

        // Trivia is out-of-band: the gap before `from` is the comment + newline.
        let from = tokens
            .iter()
            .find(|t| text(src, t) == "from")
            .expect("from present");
        assert_eq!(from.kind, TokenKind::Keyword(Keyword::From));
    }

    #[test]
    fn keyword_lookup_is_case_insensitive_but_ascii_only() {
        assert_eq!(
            lexed("SeLeCt asc café FROM"),
            [
                (TokenKind::Keyword(Keyword::Select), "SeLeCt"),
                (TokenKind::Keyword(Keyword::Asc), "asc"),
                (TokenKind::Word, "café"),
                (TokenKind::Keyword(Keyword::From), "FROM"),
            ],
        );
    }

    #[test]
    fn nested_block_comment_is_one_skipped_unit() {
        let tokens = tokenize("/* a /* b */ c */").expect("balanced nesting");
        assert!(tokens.is_empty(), "a fully nested comment yields no tokens");

        // And it does not swallow what follows it.
        assert_eq!(
            lexed("/* a /* b */ c */x"),
            [(TokenKind::Word, "x")],
            "the token after the comment survives"
        );
    }

    #[test]
    fn line_comment_runs_to_end_of_line_only() {
        assert_eq!(
            lexed("d -- comment to EOL\nb"),
            [(TokenKind::Word, "d"), (TokenKind::Word, "b")],
        );
        // A line comment at EOF with no trailing newline is fine.
        assert!(tokenize("d --tail").expect("ok").len() == 1);
    }

    /// A MySQL-like preset that recognises `#` line comments.
    const HASH_COMMENT_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
            line_comment_hash: true,
            ..CommentSyntax::ANSI
        }));

    #[test]
    fn hash_line_comment_is_skipped_only_when_enabled() {
        // Enabled: `#` runs to end of line, exactly like `--`.
        let tokens =
            tokenize_with("d # comment to EOL\nb", &HASH_COMMENT_FEATURES).expect("hash comment");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Word, TokenKind::Word],
        );
        // A `#` comment at EOF with no trailing newline is fine.
        assert_eq!(
            tokenize_with("d #tail", &HASH_COMMENT_FEATURES)
                .expect("ok")
                .len(),
            1,
        );

        // Disabled (ANSI): `#` is in no lexical class, so it is a stray byte.
        let err = tokenize("d # b").expect_err("# is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(2, 3));
    }

    #[test]
    fn hash_comment_and_hash_identifier_are_mutually_exclusive() {
        // The documented either/or: a dialect uses `#` for comments OR for
        // identifiers, never both. The comment branch is checked before byte-class
        // identifier dispatch, so with `line_comment_hash` on, `#temp` to EOL is a
        // comment — only `x` on the next line survives.
        let src = "#temp\nx";
        let comment = tokenize_with(src, &HASH_COMMENT_FEATURES).expect("hash comment wins");
        assert_eq!(comment.len(), 1);
        assert_eq!(comment[0].kind, TokenKind::Word);
        assert_eq!(text(src, &comment[0]), "x");

        // The other side of the either/or: a T-SQL-like preset marks `#` an
        // identifier byte (and leaves hash-comments off), so `#temp` is one word.
        const TEMP_IDENT_FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.byte_classes(
                ByteClasses::STANDARD
                    .with_class(b'#', CLASS_IDENTIFIER_START | CLASS_IDENTIFIER_CONTINUE),
            ),
        );
        let ident = tokenize_with("#temp", &TEMP_IDENT_FEATURES).expect("hash identifier");
        assert_eq!(ident.len(), 1);
        assert_eq!(ident[0].kind, TokenKind::Word);
        assert_eq!(text("#temp", &ident[0]), "#temp");
    }

    /// DuckDB's `#n` positional column reference, gated by
    /// `ExpressionSyntax::positional_column`.
    const POSITIONAL_COLUMN_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
            positional_column: true,
            ..ExpressionSyntax::ANSI
        }));

    #[test]
    fn hash_positional_column_lexes_only_when_enabled() {
        // Enabled (DuckDB): `#` + digits is one token spanning the sigil and the digits.
        let tokens = tokenize_with("#12", &POSITIONAL_COLUMN_FEATURES).expect("positional column");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::PositionalColumn);
        assert_eq!(text("#12", &tokens[0]), "#12");

        // In an `ORDER BY` tail each `#n` lexes as its own token beside the comma.
        let src = "ORDER BY #2 , #1";
        let kinds: Vec<_> = tokenize_with(src, &POSITIONAL_COLUMN_FEATURES)
            .expect("clean tokenization")
            .iter()
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == TokenKind::PositionalColumn)
                .count(),
            2,
        );

        // A `#` with no following digit stays a stray byte even with the feature on,
        // matching DuckDB's own "syntax error at or near #".
        let bare = tokenize_with("#a", &POSITIONAL_COLUMN_FEATURES).expect_err("bare # is stray");
        assert_eq!(bare.kind, LexErrorKind::StrayByte);
        assert_eq!(bare.span, Span::new(0, 1));

        // Disabled (ANSI): `#` is in no lexical class, so `#1` is a stray byte.
        let err = tokenize("#1").expect_err("# is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
    }

    /// The MySQL comment surface — `/*!…*/` conditional inclusion gated at
    /// `CommentSyntax::MYSQL_8_VERSION_BOUND` (80499) and non-nesting block
    /// comments — applied to the ANSI base so these tests pin the comment shape
    /// in isolation. Every expectation below was verified against a live mysql:8
    /// (8.4.10) server.
    const VERSIONED_COMMENT_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax::MYSQL));

    fn lexed_versioned(src: &str) -> Vec<(TokenKind, &str)> {
        tokenize_with(src, &VERSIONED_COMMENT_FEATURES)
            .expect("clean tokenization")
            .iter()
            .map(|t| (t.kind, text(src, t)))
            .collect()
    }

    #[test]
    fn versioned_comment_body_lexes_as_live_tokens_only_when_enabled() {
        use crate::ast::Keyword;
        // Enabled: the wrapper is trivia, the body is live input — the engine
        // *executes* `/*!…*/` content, so `SELECT /*!40101 1 */` is `SELECT 1`.
        assert_eq!(
            lexed_versioned("SELECT /*!40101 1 */"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1")
            ],
        );

        // Disabled (every non-MySQL dialect): the whole construct stays an
        // ordinary skipped block comment — the pre-existing behaviour.
        assert_eq!(
            lexed("SELECT /*!40101 1 */"),
            [(TokenKind::Keyword(Keyword::Select), "SELECT")],
        );
    }

    #[test]
    fn versioned_comment_version_gate_and_digit_rule_follow_the_engine() {
        // (input, live tokens after SELECT) — the engine's digit rule: exactly
        // five or six abutting digits form the version (include iff <= 80499);
        // 0-4 digits are not a version (digits stay body tokens, unconditional
        // include); from >=7 digits the first five are the version.
        let cases: &[(&str, &[&str])] = &[
            ("SELECT /*! 1 */", &["1"]),              // no version: include
            ("SELECT /*!80499 1 */", &["1"]),         // at the bound: include
            ("SELECT /*!80500 1 */", &[]),            // above the bound: skip
            ("SELECT /*!99999 1 */", &[]),            // future 5-digit: skip
            ("SELECT /*!1234 9 */", &["1234", "9"]),  // 4 digits: not a version
            ("SELECT /*!123456 9 */", &[]),           // 6-digit 123456 > bound: skip
            ("SELECT /*!012345 9 */", &["9"]),        // 6-digit 012345 = 12345: include
            ("SELECT /*!1234567 9 */", &["67", "9"]), // first five 12345: include, 67 leaks
            ("SELECT /*! 40101 */", &["40101"]),      // space breaks abutment: no version
        ];
        for (src, expected) in cases {
            let body: Vec<&str> = lexed_versioned(src)[1..]
                .iter()
                .map(|(_, text)| *text)
                .collect();
            assert_eq!(&body, expected, "digit-rule divergence for {src:?}");
        }
    }

    #[test]
    fn versioned_region_flag_semantics_match_the_engine() {
        use crate::ast::Keyword;
        // An inner plain comment consumes its own terminator (non-nesting), so
        // the region continues past it.
        assert_eq!(
            lexed_versioned("SELECT /*!40101 1 /* c */ + 2 */"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1"),
                (TokenKind::Operator(Operator::Plus), "+"),
                (TokenKind::Number, "2"),
            ],
        );

        // A passing inner `/*!NNNNN` is a no-op marker — the state is a flag,
        // not a depth — so the first region-level `*/` closes the (single)
        // region and the second leaks as the `*` `/` operator tokens.
        assert_eq!(
            lexed_versioned("SELECT /*!40101 1 /*!40101 + 10 */ */ x"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1"),
                (TokenKind::Operator(Operator::Plus), "+"),
                (TokenKind::Number, "10"),
                (TokenKind::Operator(Operator::Star), "*"),
                (TokenKind::Operator(Operator::Slash), "/"),
                (TokenKind::Word, "x"),
            ],
        );

        // A failing inner marker discards only up to the next `*/`; the outer
        // region stays open (engine: `SELECT /*!40101 1 /*!99999 + 10 */ + 2 */`
        // evaluates to 3 on mysql:8).
        assert_eq!(
            lexed_versioned("SELECT /*!40101 1 /*!99999 + 10 */ + 2 */"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1"),
                (TokenKind::Operator(Operator::Plus), "+"),
                (TokenKind::Number, "2"),
            ],
        );
    }

    #[test]
    fn versioned_region_string_protection_and_raw_discard() {
        use crate::ast::Keyword;
        // Included body: a `*/` inside a string literal is string content — the
        // close is recognised only where a token would start, which is exactly
        // the engine's protection.
        assert_eq!(
            lexed_versioned("SELECT /*!40101 '*/' */"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::String, "'*/'"),
            ],
        );

        // Discarded body: raw bytes, NOT string-aware — an unbalanced quote is
        // harmless and the first `*/` closes (engine-verified).
        assert_eq!(
            lexed_versioned("SELECT /*!99999 ' */ 1"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1"),
            ],
        );
    }

    #[test]
    fn versioned_region_unterminated_is_a_lex_error() {
        // Both the included and the discarded form reject at EOF, like an
        // unterminated block comment, with the span anchored at the opener.
        for src in ["SELECT /*!40101 1", "SELECT /*!99999 x"] {
            let err = tokenize_with(src, &VERSIONED_COMMENT_FEATURES)
                .expect_err("an unterminated region must not lex");
            assert_eq!(err.kind, LexErrorKind::UnterminatedBlockComment, "{src:?}");
            assert_eq!(err.span, Span::new(7, src.len() as u32), "{src:?}");
        }
    }

    #[test]
    fn block_comment_nesting_is_dialect_data() {
        use crate::ast::Keyword;
        // MySQL: the first `*/` closes a block comment — `/* a /* b */` is a
        // complete comment and the `1` is live (the engine parses this).
        assert_eq!(
            lexed_versioned("SELECT /* a /* b */ 1"),
            [
                (TokenKind::Keyword(Keyword::Select), "SELECT"),
                (TokenKind::Number, "1")
            ],
        );
        // The balanced-nesting spelling leaks its tail under MySQL instead.
        assert_eq!(
            lexed_versioned("SELECT /* a /* b */ zz */ 1")
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Keyword(Keyword::Select),
                TokenKind::Word,
                TokenKind::Operator(Operator::Star),
                TokenKind::Operator(Operator::Slash),
                TokenKind::Number,
            ],
        );

        // ANSI keeps the permissive nesting baseline: the same prefix is an
        // unterminated (still-nested) comment.
        let err = tokenize("SELECT /* a /* b */ 1").expect_err("ANSI nests");
        assert_eq!(err.kind, LexErrorKind::UnterminatedBlockComment);
    }

    #[test]
    fn versioned_comment_markers_are_recorded_as_trivia() {
        // An included region records its opener (with the version digits) and its
        // closer as separate BlockComment runs — the version number is
        // offset-recoverable for tooling even though it is not a token.
        let src = "SELECT /*!40101 1 */ x";
        let (tokens, trivia) =
            tokenize_with_trivia(src, &VERSIONED_COMMENT_FEATURES).expect("clean tokenization");
        assert_eq!(tokens.len(), 3);
        let comments: Vec<&str> = trivia
            .all()
            .iter()
            .filter(|run| run.kind() == TriviaKind::BlockComment)
            .map(|run| &src[run.span().start() as usize..run.span().end() as usize])
            .collect();
        assert_eq!(comments, ["/*!40101", "*/"]);

        // A discarded region is one whole BlockComment run, like the comment the
        // engine treats it as.
        let src = "SELECT /*!99999 1 */ x";
        let (tokens, trivia) =
            tokenize_with_trivia(src, &VERSIONED_COMMENT_FEATURES).expect("clean tokenization");
        assert_eq!(tokens.len(), 2);
        let comments: Vec<&str> = trivia
            .all()
            .iter()
            .filter(|run| run.kind() == TriviaKind::BlockComment)
            .map(|run| &src[run.span().start() as usize..run.span().end() as usize])
            .collect();
        assert_eq!(comments, ["/*!99999 1 */"]);
    }

    #[test]
    fn double_ampersand_lexes_as_one_operator_token() {
        // The `&&` lexeme always tokenizes (its *meaning* is dialect data resolved
        // in the parser); a lone `&` stays `Amp`.
        assert_eq!(
            lexed("d && b"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Operator(Operator::AmpAmp), "&&"),
                (TokenKind::Word, "b"),
            ],
        );
        assert_eq!(
            lexed("d & b"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Operator(Operator::Amp), "&"),
                (TokenKind::Word, "b"),
            ],
        );
    }

    #[test]
    fn postgres_at_family_operators_lex_as_single_tokens() {
        use Operator::{AtGt, LtAt, MinusGt, MinusGtGt};
        // Under PostgreSQL each containment / JSON lexeme is one operator token.
        let cases: &[(&str, Operator)] = &[
            ("@>", AtGt),
            ("<@", LtAt),
            ("->", MinusGt),
            ("->>", MinusGtGt),
        ];
        for (src, op) in cases {
            let tokens = tokenize_with(src, &FeatureSet::POSTGRES)
                .unwrap_or_else(|err| panic!("{src:?} lexes: {err:?}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Operator(*op));
            assert_eq!(text(src, &tokens[0]), *src);
        }
        // A bare `@` (not opening `@>`) is the prefix absolute-value operator under the
        // general operator surface (`custom_operators`): `@ x` lexes as one `Custom` operator
        // token spanning just the `@`, then the word `x`.
        let tokens = tokenize_with("@ x", &FeatureSet::POSTGRES).expect("bare `@` is the operator");
        assert_eq!(tokens[0].kind, TokenKind::Operator(Operator::Custom));
        assert_eq!(text("@ x", &tokens[0]), "@");

        // Contiguous munch even without surrounding spaces: `d@>b` and `d->>b` (`d`/`b`
        // are non-keyword words; a single letter like `a` is a SQL:2016 keyword here).
        assert_eq!(
            tokenize_with("d@>b", &FeatureSet::POSTGRES)
                .expect("contiguous lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [TokenKind::Word, TokenKind::Operator(AtGt), TokenKind::Word],
        );
        assert_eq!(
            tokenize_with("d->>b", &FeatureSet::POSTGRES)
                .expect("contiguous lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(MinusGtGt),
                TokenKind::Word
            ],
        );
    }

    #[test]
    fn pipe_arrow_lexes_only_under_pipe_syntax() {
        // The BigQuery/ZetaSQL `|>` separator is a feature-gated maximal munch. Build an
        // ANSI dialect with only `pipe_syntax` flipped on so the test isolates this gate.
        let pipe = FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
            pipe_syntax: true,
            ..QueryTailSyntax::ANSI
        }));

        // Gate on: `|>` is one PipeArrow token, contiguous-munch even without spaces.
        // `d`/`b` are non-keyword words (a lone `a` is a SQL:2016 keyword here).
        let tokens = tokenize_with("d |> b", &pipe).expect("`|>` lexes under pipe syntax");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::PipeArrow),
                TokenKind::Word
            ],
        );
        assert_eq!(
            tokenize_with("d|>b", &pipe)
                .expect("contiguous `|>` lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::PipeArrow),
                TokenKind::Word
            ],
        );
        // `||` is untouched by the new arm — still one Concat token under the same dialect.
        assert_eq!(
            tokenize_with("d || b", &pipe)
                .expect("`||` still lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::Concat),
                TokenKind::Word
            ],
        );

        // Gate off (ANSI): the bytes stay `|` then `>`, so no dialect's `|`/`||` lexing
        // shifts. (Under POSTGRES the `|>` bytes are one `Custom` operator instead — the
        // general operator surface's maximal munch — which is a different not-PipeArrow
        // reading; ANSI, with neither `pipe_syntax` nor `custom_operators`, isolates the
        // plain `|` `>` split this gate is about.)
        assert_eq!(
            tokenize_with("d |> b", &FeatureSet::ANSI)
                .expect("`|` `>` lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::Pipe),
                TokenKind::Operator(Operator::Gt),
                TokenKind::Word
            ],
        );

        // Maximal munch is contiguous only: a space breaks `| >` even with the gate on.
        assert_eq!(
            tokenize_with("d | > b", &pipe)
                .expect("spaced `| >` lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::Pipe),
                TokenKind::Operator(Operator::Gt),
                TokenKind::Word
            ],
        );
    }

    #[test]
    fn postgres_operator_munch_is_contiguous_only() {
        // Maximal munch is over abutting bytes only: a space breaks `->`, so `d - > b`
        // stays a binary minus then a greater-than rather than a JSON arrow.
        assert_eq!(
            tokenize_with("d - > b", &FeatureSet::POSTGRES)
                .expect("spaced lexes")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Operator(Operator::Minus),
                TokenKind::Operator(Operator::Gt),
                TokenKind::Word,
            ],
        );
    }

    #[test]
    fn mysql_at_forms_are_unaffected_by_the_pg_operators() {
        // The MySQL session-variable preset leaves the containment operators off, so
        // `@name`/`@@name` keep lexing as single Variable tokens — the PostgreSQL `@`
        // operator scanner does not regress the MySQL `@`/`@@` boundary the token-prefix
        // work established.
        for src in ["@x", "@@version", "@@global.time_zone"] {
            let tokens = tokenize_with(src, &SESSION_VARIABLE_FEATURES)
                .unwrap_or_else(|err| panic!("{src:?} lexes: {err:?}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one Variable token");
            assert_eq!(tokens[0].kind, TokenKind::Variable);
            assert_eq!(text(src, &tokens[0]), src);
        }
    }

    #[test]
    fn dollar_quoting_is_a_single_string_token() {
        for src in ["$$x$$", "$tag$y$tag$", "$$$$", "$tag$$tag$"] {
            let tokens = tokenize_with(src, &FeatureSet::POSTGRES).expect("balanced dollar-quote");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::String);
            assert_eq!(text(src, &tokens[0]), src, "the whole literal is the span");
        }

        // A differently-tagged inner delimiter is body text, not a close.
        let src = "$a$ inner $b$ still body $a$";
        let tokens = tokenize_with(src, &FeatureSet::POSTGRES).expect("balanced");
        assert_eq!(tokens.len(), 1);
        assert_eq!(text(src, &tokens[0]), src);
    }

    #[test]
    fn dollar_quote_near_miss_candidates_do_not_skip_the_true_terminator() {
        // Regression for the position-scan-to-`$` rewrite of `scan_dollar_quote`:
        // each body below plants a `$` that opens a candidate close but fails to
        // match the full delimiter. A resume that skips the whole failed-match
        // width (`delim.len()` bytes) instead of just the one candidate byte
        // could step past a true terminator hiding inside the near miss.
        let cases: &[&str] = &[
            // An embedded '$' with no tag following at all.
            "$tag$a$b$tag$",
            // Two sequential near-misses (`$ta$` shares a 3-byte prefix with
            // `$tag$`, then its own close is also not `tag$`) ahead of the close.
            "$tag$x$ta$y$tag$",
            // The tightest overlap: `$ta` immediately precedes the real `$tag$`,
            // so the byte that breaks the near-miss comparison is itself the
            // opening '$' of the true terminator.
            "$tag$x$ta$tag$",
            // Empty tag (`$$`) with a lone embedded '$' that never doubles up.
            "$$a$b$$",
        ];
        for src in cases {
            let tokens = tokenize_with(src, &FeatureSet::POSTGRES)
                .unwrap_or_else(|err| panic!("{src:?} should lex cleanly: {err:?}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::String);
            assert_eq!(
                text(src, &tokens[0]),
                *src,
                "{src:?} span covers the whole literal"
            );
        }

        // Unterminated: a trailing lone '$' is one last failing candidate, and
        // the scan must still run to end of input and report the whole span —
        // identical to the byte-at-a-time scan this replaces.
        let src = "$tag$abc$";
        let err = tokenize_with(src, &FeatureSet::POSTGRES).expect_err("never closes");
        assert_eq!(err.kind, LexErrorKind::UnterminatedDollarQuote);
        assert_eq!(err.span, Span::new(0, src.len() as u32));
    }

    /// A preset enabling both positional placeholder forms (`$n` and `?`) plus
    /// dollar-quoting, so the `$`-disambiguation is exercised with both `$` paths live.
    const PARAMETER_FEATURES: FeatureSet = FeatureSet::POSTGRES
        .with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            positional_dollar: true,
            anonymous_question: true,
            ..ParameterSyntax::POSTGRES
        }))
        // The anonymous `?` placeholder and the `jsonb` `?` operator claim the same
        // trigger; enabling the placeholder here vacates the operators so the preset
        // stays lexically consistent (the `?` still lexes as the placeholder either way).
        .with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            jsonb_operators: false,
            ..OperatorSyntax::POSTGRES
        }));

    /// A preset enabling the named placeholder forms (`:name` and `@name`) on top of
    /// PostgreSQL, so the `:`/`::` and `@`/`@@` disambiguations are exercised with the
    /// typecast and dollar-quoting paths also live.
    const NAMED_PARAMETER_FEATURES: FeatureSet =
        FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            named_colon: true,
            named_at: true,
            ..ParameterSyntax::POSTGRES
        }));

    const SESSION_VARIABLE_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.session_variables(SessionVariableSyntax::MYSQL));

    #[test]
    fn parameter_placeholders_lex_as_one_token_only_when_enabled() {
        // Positional `$1` and anonymous `?` each lex as a single Parameter token,
        // with the span covering the whole placeholder.
        for src in ["$1", "$42", "?"] {
            let tokens = tokenize_with(src, &PARAMETER_FEATURES).expect("placeholder lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Parameter);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // Disabled (ANSI): `$` and `?` are both stray bytes.
        let err = tokenize("$1").expect_err("`$1` is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
        let err = tokenize("?").expect_err("`?` is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
    }

    #[test]
    fn positional_parameter_does_not_collide_with_dollar_quoting() {
        // The `$`-disambiguation: with both positional parameters and dollar-quoting
        // enabled, `$` + digit is a parameter while `$tag$`/`$$` open a string.
        let tokens = tokenize_with("$1 $$x$$", &PARAMETER_FEATURES).expect("both `$` forms lex");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Parameter, TokenKind::String],
        );
        assert_eq!(text("$1 $$x$$", &tokens[0]), "$1");
        assert_eq!(text("$1 $$x$$", &tokens[1]), "$$x$$");

        // With dollar-quoting on but positional off, `$1` is not a valid dollar-quote
        // opener (a tag cannot start with a digit), so it stays a stray byte.
        const DOLLAR_QUOTE_ONLY: FeatureSet =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.parameters(ParameterSyntax::ANSI));
        let err = tokenize_with("$1", &DOLLAR_QUOTE_ONLY)
            .expect_err("`$1` rejects when positional parameters are off");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));

        // `$$x$$` is still one dollar-quoted String when positional is on.
        let tokens = tokenize_with("$$x$$", &PARAMETER_FEATURES).expect("dollar quote still lexes");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
    }

    /// A T-SQL-like preset recognising `$1234.56` money literals on top of ANSI.
    /// ANSI-based (no positional `$n`, no dollar-quoting), so the `$` byte belongs to
    /// money alone — the dialect-disjoint arrangement a real money dialect has.
    const MONEY_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
            money_literals: true,
            ..NumericLiteralSyntax::ANSI
        }));

    #[test]
    fn money_literals_lex_as_one_number_only_when_enabled() {
        // `$1234.56`, `$100`, leading-dot `$.5`, and trailing-dot `$1.` each lex as a
        // single Number token spanning the `$` (the money type is recovered from the
        // prefix at parse time, ADR-0006).
        for src in ["$1234.56", "$100", "$.5", "$1."] {
            let tokens = tokenize_with(src, &MONEY_FEATURES).expect("money literal lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Number);
            assert_eq!(text(src, &tokens[0]), src, "the whole literal is the span");
        }

        // A signed money value is a separate `-` operator then the money Number; signs
        // are never folded into the literal token (ADR-0006).
        let src = "-$1000";
        let tokens = tokenize_with(src, &MONEY_FEATURES).expect("signed money lexes");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Operator(Operator::Minus), TokenKind::Number],
        );
        assert_eq!(text(src, &tokens[1]), "$1000");

        // Disabled (ANSI): `$` is in no lexical class, so it is a stray byte.
        let err = tokenize("$100").expect_err("`$100` is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn money_resolves_the_dollar_dispatch_deterministically() {
        // `$.` with no following digit is not money: it falls through the `$` arms. Under
        // money-only ANSI there is no further `$` form, so it is a stray byte.
        let err =
            tokenize_with("$.x", &MONEY_FEATURES).expect_err("`$.` without a digit is not money");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));

        // All three `$` forms live at once (a synthetic config — no shipped dialect
        // enables money *and* the PostgreSQL `$` forms): money claims `$.5`, dollar
        // quoting claims `$$x$$`, and the documented money-before-parameter priority
        // makes `$1234.56` one money Number rather than the `$1234` parameter + `.56`.
        const ALL_DOLLAR_FORMS: FeatureSet =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
                money_literals: true,
                ..NumericLiteralSyntax::POSTGRES
            }));
        let src = "$1234.56 $.5 $$x$$";
        let tokens = tokenize_with(src, &ALL_DOLLAR_FORMS).expect("all `$` forms lex");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Number, TokenKind::Number, TokenKind::String],
        );
        assert_eq!(text(src, &tokens[0]), "$1234.56");
        assert_eq!(text(src, &tokens[1]), "$.5");
        assert_eq!(text(src, &tokens[2]), "$$x$$");
    }

    #[test]
    fn named_parameters_lex_as_one_token_only_when_enabled() {
        // `:name` / `@name` each lex as a single Parameter token spanning sigil + name.
        for src in [":name", "@name", ":x1", "@_v"] {
            let tokens =
                tokenize_with(src, &NAMED_PARAMETER_FEATURES).expect("named placeholder lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Parameter);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // Disabled (PostgreSQL has neither named form): the `:` stays a lone-colon
        // punctuation token rather than absorbing the following name, so `:name` is `:` then a
        // separate name — never one Parameter token.
        let tokens = tokenize_with(":name", &FeatureSet::POSTGRES).expect("`:` is punctuation");
        assert_eq!(tokens.len(), 2, "`:name` is `:` then a separate name");
        assert_eq!(tokens[0].kind, TokenKind::Punctuation(Punctuation::Colon));
        assert_eq!(text(":name", &tokens[0]), ":");
        // `@name` under PostgreSQL is NOT a named parameter either: with `named_at` off, the
        // `@` lexes as the prefix absolute-value operator (`Custom`, the general operator
        // surface) spanning just the `@`, then the name as a separate token — two tokens, not
        // one Parameter. (The name itself may lex as a keyword, which is irrelevant here.)
        let tokens = tokenize_with("@name", &FeatureSet::POSTGRES).expect("`@name` lexes");
        assert_eq!(
            tokens.len(),
            2,
            "`@name` is the `@` operator then a separate name"
        );
        assert_eq!(tokens[0].kind, TokenKind::Operator(Operator::Custom));
        assert_eq!(text("@name", &tokens[0]), "@");
        assert_ne!(tokens[0].kind, TokenKind::Parameter);
    }

    #[test]
    fn session_variables_lex_as_one_token_only_when_enabled() {
        // `@name`, `@@name`, and `@@scope.name` each lex as a single Variable token
        // spanning the sigil, any scope prefix, and the name (the scope `.` never
        // escapes as separate punctuation).
        for src in [
            "@user_count",
            "@@max_connections",
            "@@global.time_zone",
            "@@session.sql_mode",
        ] {
            let tokens =
                tokenize_with(src, &SESSION_VARIABLE_FEATURES).expect("session variable lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Variable);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // `@@global` with no abutting `.name` is one token (a system variable literally
        // named `global`); a trailing `.` with no identifier is left as punctuation.
        let tokens =
            tokenize_with("@@global", &SESSION_VARIABLE_FEATURES).expect("bare `@@global` lexes");
        assert_eq!(tokens.len(), 1);
        assert_eq!(text("@@global", &tokens[0]), "@@global");

        // Disabled (ANSI): `@` is a lexical stray byte, never a variable token.
        let err = tokenize_with("@x", &FeatureSet::ANSI).expect_err("`@` is stray under ANSI");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn named_colon_parameter_does_not_collide_with_cast_or_slice() {
        // The `:`-disambiguation: with `:name` on, the sigil only binds an
        // identifier-start byte, so `::` stays the typecast and `:` before a digit
        // stays the array-slice separator — both unchanged.
        let tokens = tokenize_with("x::y", &NAMED_PARAMETER_FEATURES).expect("cast still lexes");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Punctuation(Punctuation::DoubleColon),
                TokenKind::Word,
            ],
            "`::` is still the typecast, never a `:`-parameter",
        );

        let tokens = tokenize_with("x[1:2]", &NAMED_PARAMETER_FEATURES).expect("slice still lexes");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [
                TokenKind::Word,
                TokenKind::Punctuation(Punctuation::LBracket),
                TokenKind::Number,
                TokenKind::Punctuation(Punctuation::Colon),
                TokenKind::Number,
                TokenKind::Punctuation(Punctuation::RBracket),
            ],
            "a lone `:` before a digit is still the slice separator",
        );

        // `@@x`: the second `@` is not an identifier byte, so the `@name` arm never fires on
        // the first `@`. This preset inherits PostgreSQL's `jsonb` operators, whose `@@` match
        // operator munches the two `@` bytes, leaving `x` a word — so `@@` stays disjoint from
        // the `@name` form by its second byte.
        let tokens = tokenize_with("@@x", &NAMED_PARAMETER_FEATURES)
            .expect("`@@` lexes as the jsonb match operator, not the `@name` form");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Operator(Operator::AtAt), TokenKind::Word],
            "`@@` is the jsonb match operator, then the `x` word",
        );
    }

    #[test]
    fn postgres_escape_string_is_a_single_string_token() {
        let src = "E'can\\'t' e'line\\n'";
        let tokens = tokenize_with(src, &FeatureSet::POSTGRES).expect("escape strings lex");

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(text(src, &tokens[0]), "E'can\\'t'");
        assert_eq!(tokens[1].kind, TokenKind::String);
        assert_eq!(text(src, &tokens[1]), "e'line\\n'");
    }

    #[test]
    fn postgres_escape_string_rejects_malformed_escapes_at_lex_time() {
        // PostgreSQL validates escape content in its scanner; a short/invalid Unicode
        // escape, an escape decoding to NUL, or a byte escape that breaks UTF-8 is
        // rejected while lexing (matching libpg_query), not deferred to the accessor.
        for src in [
            "E'\\u12'",
            "E'\\uD800'",
            "E'\\U00110000'",
            "E'\\0'",
            "E'\\x00'",
            "E'\\xff'",
            "E'\\xc3'",
        ] {
            let err = tokenize_with(src, &FeatureSet::POSTGRES)
                .expect_err("malformed escape is rejected at lex time");
            assert_eq!(err.kind, LexErrorKind::InvalidEscapeSequence, "for {src:?}");
            // The error covers the whole escape-string token.
            assert_eq!(err.span, Span::new(0, src.len() as u32), "for {src:?}");
        }
    }

    #[test]
    fn postgres_escape_string_accepts_unknown_and_valid_escapes() {
        // An unknown escape (`\q`), a bare `\x`, and well-formed byte/Unicode escapes
        // all lex as one string token — the boundary the real parser draws; the value
        // is recovered later via `as_str` (ADR-0006).
        for src in [
            "E'\\q'",
            "E'\\x'",
            "E'\\q\\x'",
            "E'\\xc3\\xa9'",
            "E'line\\n\\t'",
            "E'\\141\\x62c'",
        ] {
            let tokens = tokenize_with(src, &FeatureSet::POSTGRES)
                .unwrap_or_else(|err| panic!("{src:?} should lex: {err:?}"));
            assert_eq!(tokens.len(), 1, "for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::String, "for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "for {src:?}");
        }
    }

    #[test]
    fn ansi_tokenizer_does_not_accept_dollar_quoted_strings() {
        let err = tokenize("$$x$$").expect_err("dollar quotes are PostgreSQL-specific");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn string_literal_rejects_a_raw_nul_byte_at_lex_time() {
        // PG parity: a raw NUL byte (0x00) in a string literal's source is rejected
        // while lexing in every form (confirmed against the libpg_query oracle), with
        // the error spanning the whole literal. This is the raw-byte case, distinct
        // from an escape that *decodes* to NUL (`E'\\x00'`, an InvalidEscapeSequence).
        let cases: &[(&str, &FeatureSet)] = &[
            ("'a\0b'", &FeatureSet::ANSI),             // ordinary
            ("'\0'", &FeatureSet::ANSI),               // ordinary, lone NUL
            ("E'a\0b'", &FeatureSet::POSTGRES),        // escape string, raw NUL byte
            ("$$a\0b$$", &FeatureSet::POSTGRES),       // dollar-quoted
            ("$tag$a\0b$tag$", &FeatureSet::POSTGRES), // tagged dollar-quote
            ("N'a\0b'", &NATIONAL_STRING_FEATURES),    // national string
            ("U&'a\0b'", &UNICODE_STRING_FEATURES),    // unicode-escape string
            ("B'1\0'", &BIT_STRING_FEATURES),          // bit-string constant
            // Backslash-escape dialect: a NUL right after `\` is still rejected — the
            // check scans the finished token, so escaping cannot smuggle a NUL past it.
            ("'a\\\0b'", &MYSQL_STRING_FEATURES),
        ];
        for (src, features) in cases {
            let err = tokenize_with(src, features)
                .expect_err("a raw NUL in a string literal is rejected at lex time");
            assert_eq!(err.kind, LexErrorKind::NulByteInString, "for {src:?}");
            assert_eq!(
                err.span,
                Span::new(0, src.len() as u32),
                "the error spans the whole literal for {src:?}",
            );
        }

        // Sanity: a non-NUL control byte (0x01) is accepted — PostgreSQL accepts it
        // too, so the check is specific to NUL, not control bytes generally — and a
        // clean string still lexes as one token.
        for src in ["'a\x01b'", "'ab'"] {
            let tokens = tokenize_with(src, &FeatureSet::ANSI).expect("non-NUL string lexes");
            assert_eq!(tokens.len(), 1, "for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::String, "for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "for {src:?}");
        }
    }

    #[test]
    fn quoted_identifier_rejects_a_raw_nul_byte_at_lex_time() {
        // PG parity: a raw NUL byte (0x00) in a quoted identifier's source is rejected
        // while lexing, in every configured quote form, with the error spanning the
        // whole identifier. PostgreSQL rejects a NUL in all lexable text via its
        // NUL-terminated-C-string boundary (confirmed against the libpg_query oracle for
        // the `"…"` form it enables). This is its own error kind, not `NulByteInString`:
        // a quoted identifier is interned, never materialised through `Literal::as_str`.

        // A SQL Server-like preset whose only identifier quote is the asymmetric `[…]`.
        const BRACKET_IDENT_FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_quotes(&[
                IdentifierQuote::Asymmetric {
                    open: '[',
                    close: ']',
                },
            ]));

        let cases: &[(&str, &FeatureSet)] = &[
            ("\"a\0b\"", &FeatureSet::ANSI), // standard double-quoted identifier
            ("\"\0\"", &FeatureSet::ANSI),   // lone NUL
            ("[a\0b]", &BRACKET_IDENT_FEATURES), // T-SQL bracket identifier
            ("`a\0b`", &MYSQL_STRING_FEATURES), // MySQL backtick identifier
        ];
        for (src, features) in cases {
            let err = tokenize_with(src, features)
                .expect_err("a raw NUL in a quoted identifier is rejected at lex time");
            assert_eq!(err.kind, LexErrorKind::NulByteInIdentifier, "for {src:?}");
            assert_eq!(
                err.span,
                Span::new(0, src.len() as u32),
                "the error spans the whole identifier for {src:?}",
            );
        }

        // Sanity: a non-NUL control byte (0x01) is accepted (the check is specific to
        // NUL), and a clean quoted identifier still lexes as one token.
        for src in ["\"a\x01b\"", "\"ab\""] {
            let tokens = tokenize_with(src, &FeatureSet::ANSI).expect("non-NUL identifier lexes");
            assert_eq!(tokens.len(), 1, "for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::QuotedIdent, "for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "for {src:?}");
        }
    }

    #[test]
    fn comment_rejects_a_raw_nul_byte_at_lex_time() {
        // PG parity: a raw NUL byte (0x00) inside a comment is rejected while lexing. A
        // comment is skipped as trivia — the scanner consumes it to end-of-line / `*/`
        // without inspecting the bytes — so it is the one lexable context the value-bearing
        // string/identifier NUL gates cannot see. PostgreSQL rejects a NUL in *all* lexable
        // text via its NUL-terminated-C-string boundary (confirmed against the libpg_query
        // oracle), so this closes the gap; the minimized fuzz reproducer is the bare `--\0-`
        // line comment (fuzz-pg-differential-crash-2b8d66f9). A raw NUL cannot be written in
        // a Rust raw string, so these use `\0` escapes.
        let cases: &[(&str, &FeatureSet)] = &[
            ("--\0-", &FeatureSet::POSTGRES), // the 4-byte fuzz reproducer, comment to EOF
            ("-- a\0b", &FeatureSet::ANSI),   // `--` line comment, NUL mid-body
            ("-- a\0b\nSELECT 1", &FeatureSet::ANSI), // NUL before the comment-ending newline
            ("# a\0b", &HASH_COMMENT_FEATURES), // MySQL `#` line comment
            ("/* a\0b */", &FeatureSet::ANSI), // `/* … */` block comment
            ("/* a /* \0 */ b */", &FeatureSet::POSTGRES), // NUL in a nested block comment
            ("SELECT/* \0 */1", &FeatureSet::ANSI), // block comment between real tokens
        ];
        for (src, features) in cases {
            let err = tokenize_with(src, features)
                .expect_err("a raw NUL in a comment is rejected at lex time");
            assert_eq!(err.kind, LexErrorKind::NulByteInComment, "for {src:?}");
        }
        // The error spans the comment run (here the whole 4-byte reproducer).
        let err = tokenize_with("--\0-", &FeatureSet::POSTGRES).expect_err("rejected");
        assert_eq!(err.span, Span::new(0, 4));

        // Sanity: a non-NUL control byte (0x01) in a comment is accepted — PostgreSQL
        // accepts it, so the check is specific to NUL, not control bytes — and a
        // comment-only input still lexes to zero tokens (the comment is pure trivia).
        for src in ["-- a\x01b", "/* a\x01b */"] {
            assert!(
                tokenize_with(src, &FeatureSet::ANSI)
                    .expect("a non-NUL control byte in a comment lexes")
                    .is_empty(),
                "for {src:?}",
            );
        }
    }

    #[test]
    fn line_comment_terminator_set_is_dialect_data() {
        // The line-comment terminator set is dialect data (tokenizer-line-comment-terminator-set).
        // A `\n` ends a `--`/`#` line comment in every dialect; a bare `\r` ends it only where
        // `CommentSyntax::line_comment_ends_at_carriage_return` is set — Postgres and DuckDb,
        // whose flex scanner's comment body is `[^\n\r]*` — while Ansi/Sqlite/MySql/Lenient read
        // a `\r` as ordinary comment content and end only at `\n`. A `\x0b`/`\x0c` (vertical tab /
        // form feed, in the flex `space` set but not its newline set) is never a terminator. All
        // four axes measured against pg_query, rusqlite, libduckdb, and live mysql; the
        // oracle-parity half is pinned in the conformance suite.
        //
        // Probe: `<marker> x<TERM>Y`. If TERM ends the comment, `Y` is a live word token; if not,
        // the whole tail is trivia and there are zero tokens.

        // `\r` drives the `#` comment through the same code path, but no shipped dialect pairs `#`
        // comments with the CR terminator, so an ad-hoc preset exercises that combination.
        const HASH_CR_FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
                line_comment_hash: true,
                line_comment_ends_at_carriage_return: true,
                ..CommentSyntax::ANSI
            }));

        // Presets where `\r` ends a line comment (`--` for Postgres/DuckDb, `#` for the ad-hoc
        // preset), and presets where it does not (`--` for Ansi, `#` for the plain hash preset).
        let cr_on: &[(&str, &FeatureSet)] = &[
            ("--", &FeatureSet::POSTGRES),
            ("--", &FeatureSet::DUCKDB),
            ("#", &HASH_CR_FEATURES),
        ];
        let cr_off: &[(&str, &FeatureSet)] =
            &[("--", &FeatureSet::ANSI), ("#", &HASH_COMMENT_FEATURES)];

        // (terminator, ends the comment where CR is on?, ends where CR is off?)
        let terminators: &[(&str, bool, bool)] = &[
            ("\n", true, true),     // newline: the universal terminator
            ("\r", true, false),    // carriage return: only where the flag is on
            ("\r\n", true, true),   // CRLF: the trailing `\n` ends it even without CR-termination
            ("\x0b", false, false), // vertical tab: never a terminator
            ("\x0c", false, false), // form feed: never a terminator
        ];

        let check = |marker: &str, features: &FeatureSet, term: &str, ends: bool| {
            let src = format!("{marker} x{term}Y");
            let tokens =
                tokenize_with(&src, features).unwrap_or_else(|e| panic!("lex {src:?}: {e:?}"));
            if ends {
                assert_eq!(tokens.len(), 1, "{src:?} should end the comment");
                assert_eq!(text(&src, &tokens[0]), "Y", "live token after {src:?}");
            } else {
                assert!(tokens.is_empty(), "{src:?} is one comment run to EOF");
            }
        };
        for (term, ends_on, ends_off) in terminators {
            for (marker, features) in cr_on {
                check(marker, features, term, *ends_on);
            }
            for (marker, features) in cr_off {
                check(marker, features, term, *ends_off);
            }
        }

        // Trivia-span decision: the terminating `\r` is left for the whitespace scan, so it joins
        // the following whitespace run, never the comment run — matching PG, whose `[^\n\r]*` body
        // excludes the `\r` (which is `CLASS_WHITESPACE` in every preset). For `-- x\rY` under
        // Postgres: comment `[0,4)`, whitespace `\r` `[4,5)`, then `Y` `[5,6)`.
        let src = "-- x\rY";
        let (tokens, trivia) = tokenize_with_trivia(src, &FeatureSet::POSTGRES).expect("clean");
        assert_eq!(text(src, &tokens[0]), "Y");
        assert_eq!(tokens[0].span, Span::new(5, 6));
        let runs: Vec<(TriviaKind, Span)> =
            trivia.all().iter().map(|r| (r.kind(), r.span())).collect();
        assert_eq!(
            runs,
            [
                (TriviaKind::LineComment, Span::new(0, 4)),
                (TriviaKind::Whitespace, Span::new(4, 5)),
            ],
            "the terminating `\\r` belongs to the following whitespace, not the comment run",
        );

        // Flag off (Ansi): the same bytes are one comment run to EOF — the `\r` stays comment
        // content, so there is no token and no separate whitespace run.
        let (tokens, trivia) = tokenize_with_trivia(src, &FeatureSet::ANSI).expect("clean");
        assert!(tokens.is_empty());
        let runs: Vec<(TriviaKind, Span)> =
            trivia.all().iter().map(|r| (r.kind(), r.span())).collect();
        assert_eq!(runs, [(TriviaKind::LineComment, Span::new(0, 6))]);
    }

    #[test]
    fn zero_length_delimited_identifier_is_rejected_at_lex_time() {
        // SQL's `<delimited identifier body>` requires at least one character.
        // PostgreSQL rejects an empty delimited identifier while scanning
        // ("zero-length delimited identifier", `scan.l`) and MySQL rejects an empty
        // backtick the same way — unconditionally (no dialect legitimately accepts
        // one), so every configured identifier-quote style is covered here: symmetric
        // `"..."`, symmetric backtick, and asymmetric `[...]`.
        const BRACKET_IDENT_FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_quotes(&[
                IdentifierQuote::Asymmetric {
                    open: '[',
                    close: ']',
                },
            ]));

        let cases: &[(&str, &FeatureSet)] = &[
            ("\"\"", &FeatureSet::ANSI),     // standard double-quoted identifier
            ("``", &MYSQL_STRING_FEATURES),  // MySQL backtick identifier
            ("[]", &BRACKET_IDENT_FEATURES), // T-SQL bracket identifier
        ];
        for (src, features) in cases {
            let err = tokenize_with(src, features)
                .expect_err("an empty delimited identifier body is rejected at lex time");
            assert_eq!(
                err.kind,
                LexErrorKind::ZeroLengthDelimitedIdentifier,
                "for {src:?}"
            );
            // The span covers both delimiters (PG parity), not a zero-width point.
            assert_eq!(err.span, Span::new(0, src.len() as u32), "for {src:?}");
        }

        // `U&""` is scanned by the dedicated `U&"..."` Unicode-escaped *identifier* arm
        // (UNICODE_STRING_FEATURES enables `unicode_strings`), so the zero-length body is
        // rejected there over the whole `U&""` lexeme — matching PostgreSQL, whose `U&"..."`
        // scanner arm raises "zero-length delimited identifier" spanning the full `U&""`.
        let err = tokenize_with("U&\"\"", &UNICODE_STRING_FEATURES)
            .expect_err("an empty U&\"\" body is a zero-length delimited identifier");
        assert_eq!(err.kind, LexErrorKind::ZeroLengthDelimitedIdentifier);
        assert_eq!(
            err.span,
            Span::new(0, 4),
            "the span covers the whole `U&\"\"`, the leading `U&` included"
        );

        // The doubled-close escape is not a zero-length body: `""""` / ```` ```` ```` /
        // `[]]]` each carry exactly one *literal* close character, so all four bytes
        // stay one valid QuotedIdent — the critical case the empty-body check must not
        // regress, for every style above.
        for (src, features) in [
            ("\"\"\"\"", &FeatureSet::ANSI),
            ("````", &MYSQL_STRING_FEATURES),
            ("[]]]", &BRACKET_IDENT_FEATURES),
        ] {
            let tokens = tokenize_with(src, features)
                .expect("a doubled-close escape is one valid identifier, not an empty body");
            assert_eq!(tokens.len(), 1, "for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::QuotedIdent, "for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "for {src:?}");
        }

        // Non-regression: an empty *string* is ordinary valid SQL and must never trip
        // the identifier-only check, in every scan that shares this loop — standard
        // `''`, and MySQL `""` (a string, not an identifier, whenever
        // `double_quoted_strings` is on — the identifier-vs-string asymmetry this fix
        // must respect).
        for (src, features) in [("''", &FeatureSet::ANSI), ("\"\"", &MYSQL_STRING_FEATURES)] {
            let tokens = tokenize_with(src, features).expect("an empty string literal is valid");
            assert_eq!(tokens.len(), 1, "for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::String, "for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "for {src:?}");
        }
    }

    #[test]
    fn numbers_cover_int_float_leading_dot_and_exponent() {
        assert_eq!(lexed("42"), [(TokenKind::Number, "42")]);
        assert_eq!(lexed("3.14"), [(TokenKind::Number, "3.14")]);
        assert_eq!(lexed("1."), [(TokenKind::Number, "1.")]);
        assert_eq!(lexed(".5"), [(TokenKind::Number, ".5")]);
        assert_eq!(lexed("1e10"), [(TokenKind::Number, "1e10")]);
        assert_eq!(lexed("2.5E-3"), [(TokenKind::Number, "2.5E-3")]);

        // A bare `.` is the member dot, not a number.
        assert_eq!(
            lexed("d.b"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Punctuation(Punctuation::Dot), "."),
                (TokenKind::Word, "b"),
            ],
        );

        // `1e` is not a valid exponent: the number ends, `e` is a word.
        assert_eq!(
            lexed("1e+"),
            [
                (TokenKind::Number, "1"),
                (TokenKind::Word, "e"),
                (TokenKind::Operator(Operator::Plus), "+"),
            ],
        );
    }

    #[test]
    fn operators_and_punctuation_pick_the_right_subkinds() {
        use Operator::{Concat, Eq, GtEq, LtEq, NotEq};

        assert_eq!(
            lexed("d||b"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Operator(Concat), "||"),
                (TokenKind::Word, "b"),
            ],
        );
        assert_eq!(
            lexed("<= >= <> != ="),
            [
                (TokenKind::Operator(LtEq), "<="),
                (TokenKind::Operator(GtEq), ">="),
                (TokenKind::Operator(NotEq), "<>"),
                (TokenKind::Operator(NotEq), "!="),
                (TokenKind::Operator(Eq), "="),
            ],
        );
        assert_eq!(
            lexed("();,"),
            [
                (TokenKind::Punctuation(Punctuation::LParen), "("),
                (TokenKind::Punctuation(Punctuation::RParen), ")"),
                (TokenKind::Punctuation(Punctuation::Semicolon), ";"),
                (TokenKind::Punctuation(Punctuation::Comma), ","),
            ],
        );
    }

    #[test]
    fn colon_and_double_colon_lex_as_punctuation() {
        use Punctuation::{Colon, DoubleColon, LBracket, RBracket};

        // `::` munches as one DoubleColon (the typecast operator). `d`/`b` are not
        // keywords in the full inventory, so they lex as plain words.
        assert_eq!(
            lexed("d::b"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Punctuation(DoubleColon), "::"),
                (TokenKind::Word, "b"),
            ],
        );
        // A lone `:` is the array-slice separator; the surrounding numbers still lex
        // as separate tokens (the `:` does not begin a numeric literal).
        assert_eq!(
            lexed("d[1:2]"),
            [
                (TokenKind::Word, "d"),
                (TokenKind::Punctuation(LBracket), "["),
                (TokenKind::Number, "1"),
                (TokenKind::Punctuation(Colon), ":"),
                (TokenKind::Number, "2"),
                (TokenKind::Punctuation(RBracket), "]"),
            ],
        );
        // Maximal munch: `:::` is `::` followed by a lone `:`.
        assert_eq!(
            lexed(":::"),
            [
                (TokenKind::Punctuation(DoubleColon), "::"),
                (TokenKind::Punctuation(Colon), ":"),
            ],
        );
    }

    #[test]
    fn quoted_identifier_handles_doubled_quote_escape() {
        let src = r#""Odd""Name""#;
        let tokens = tokenize(src).expect("balanced quotes");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::QuotedIdent);
        assert_eq!(text(src, &tokens[0]), src);
    }

    #[test]
    fn tokenize_with_reads_dialect_byte_classes_and_identifier_quote() {
        const FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .identifier_quotes(&[IdentifierQuote::Symmetric('`')])
                .byte_classes(
                    ByteClasses::STANDARD
                        .with_class(b'@', CLASS_IDENTIFIER_START | CLASS_IDENTIFIER_CONTINUE),
                ),
        );

        let tokens = tokenize_with("@name `Odd``Name`", &FEATURES).expect("custom dialect lexes");

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Word);
        assert_eq!(text("@name `Odd``Name`", &tokens[0]), "@name");
        assert_eq!(tokens[1].kind, TokenKind::QuotedIdent);
        assert_eq!(text("@name `Odd``Name`", &tokens[1]), "`Odd``Name`");
    }

    #[test]
    fn asymmetric_and_multiple_identifier_quote_styles_lex() {
        // A SQL Server-like preset: asymmetric `[...]` (doubled `]]` escape) plus `"..."`.
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_quotes(&[
                IdentifierQuote::Asymmetric {
                    open: '[',
                    close: ']',
                },
                IdentifierQuote::Symmetric('"'),
            ]));

        let src = r#"[a b] "c d" [a]]b]"#;
        let tokens = tokenize_with(src, &FEATURES).expect("multi-style dialect lexes");

        assert_eq!(tokens.len(), 3);
        assert!(tokens.iter().all(|t| t.kind == TokenKind::QuotedIdent));
        assert_eq!(text(src, &tokens[0]), "[a b]");
        assert_eq!(text(src, &tokens[1]), r#""c d""#);
        // The doubled close `]]` escapes a literal `]`, so all of `[a]]b]` is one token.
        assert_eq!(text(src, &tokens[2]), "[a]]b]");
    }

    #[test]
    fn brackets_stay_punctuation_when_bracket_quoting_is_off() {
        // ANSI does not bracket-quote, so `[` remains `LBracket` punctuation.
        let tokens = tokenize("[d]").expect("ANSI lexes brackets as punctuation");
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Punctuation(Punctuation::LBracket),
                TokenKind::Word,
                TokenKind::Punctuation(Punctuation::RBracket),
            ],
        );
    }

    /// A SQL Server-like string preset: `N'...'` national strings on; the rest off.
    const NATIONAL_STRING_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            national_strings: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// A MySQL-like string preset: `"..."` is a string, `'...'` honours backslash
    /// escapes, and the identifier quote moves to the backtick.
    const MYSQL_STRING_FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .string_literals(StringLiteralSyntax {
                double_quoted_strings: true,
                backslash_escapes: true,
                ..StringLiteralSyntax::ANSI
            })
            .identifier_quotes(&[IdentifierQuote::Symmetric('`')]),
    );

    /// A unicode-escape string preset: `U&'...'` on.
    const UNICODE_STRING_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            unicode_strings: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// A bit-string preset: `B'...'` / `X'...'` on.
    const BIT_STRING_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            bit_string_literals: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// A SQLite-like blob preset: eager even-hex `x'...'` on, bit strings off.
    const BLOB_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            blob_literals: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// The MySQL layout: blob and bit-string both on, so `x`/`X` resolves to the eager
    /// blob arm by scan precedence and `B`/`b` keeps the deferred bit-string.
    const BLOB_AND_BIT_STRING_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            blob_literals: true,
            bit_string_literals: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// A MySQL-like charset-introducer preset: `_charset'...'` on (with backslash
    /// escapes, as MySQL has them) — the rest of the string surface left off. `"` keeps
    /// its ANSI role as a symmetric identifier quote (`double_quoted_strings` off), which
    /// is exactly the `ANSI_QUOTES`-on layout where the double-quoted introducer must not
    /// apply.
    const CHARSET_INTRODUCER_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
            charset_introducers: true,
            backslash_escapes: true,
            ..StringLiteralSyntax::ANSI
        }));

    /// A MySQL-like charset-introducer preset with double-quoted strings on: both
    /// `_charset'...'` and `_charset"..."` introduce a string, because `"..."` is itself
    /// a string here (`double_quoted_strings` on — the MySQL `ANSI_QUOTES`-off layout,
    /// with the identifier quote moved to the backtick so `"` is unambiguously a string).
    const CHARSET_INTRODUCER_DQ_FEATURES: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .string_literals(StringLiteralSyntax {
                charset_introducers: true,
                double_quoted_strings: true,
                backslash_escapes: true,
                ..StringLiteralSyntax::ANSI
            })
            .identifier_quotes(&[IdentifierQuote::Symmetric('`')]),
    );

    #[test]
    fn national_string_lexes_as_one_string_token_only_when_enabled() {
        // Enabled: `N'x'` is a single String spanning the `N` prefix.
        let tokens =
            tokenize_with("N'x'", &NATIONAL_STRING_FEATURES).expect("national strings lex");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(text("N'x'", &tokens[0]), "N'x'");
        // Lower-case `n` is accepted too (mirrors `E'`/`e'`).
        assert_eq!(
            tokenize_with("n'x'", &NATIONAL_STRING_FEATURES).expect("ok")[0].kind,
            TokenKind::String,
        );

        // Disabled (ANSI): `N` is a word, `'x'` a separate string.
        assert_eq!(
            lexed("N'x'"),
            [(TokenKind::Word, "N"), (TokenKind::String, "'x'")],
        );
    }

    #[test]
    fn charset_introducer_lexes_as_one_string_token_only_when_enabled() {
        // Enabled: `_utf8mb4'x'` is a single String spanning the `_charset` introducer,
        // exactly as `N'x'` spans the `N` prefix.
        for src in ["_utf8mb4'x'", "_latin1'x'", "_utf8'caf\\'e'"] {
            let tokens =
                tokenize_with(src, &CHARSET_INTRODUCER_FEATURES).expect("charset introducer lexes");
            assert_eq!(tokens.len(), 1, "one token for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::String, "kind for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "span for {src:?}");
        }

        // A bare `_name` with no abutting quote stays an ordinary identifier — the
        // introducer never steals a leading-`_` word.
        assert_eq!(
            tokenize_with("_utf8 'x'", &CHARSET_INTRODUCER_FEATURES).expect("ok")[0].kind,
            TokenKind::Word,
        );
        // A lone `_` before the quote is not a charset name (the name must be non-empty),
        // so `_` lexes as a word and `'x'` as a separate string.
        let tokens = tokenize_with("_'x'", &CHARSET_INTRODUCER_FEATURES).expect("ok");
        assert_eq!(
            (tokens[0].kind, tokens[1].kind),
            (TokenKind::Word, TokenKind::String),
        );

        // Disabled (ANSI): `_utf8` is a word, `'x'` a separate string.
        assert_eq!(
            lexed("_utf8'x'"),
            [(TokenKind::Word, "_utf8"), (TokenKind::String, "'x'")],
        );
    }

    #[test]
    fn charset_introducer_spans_a_double_quoted_body_only_when_double_quotes_string() {
        // With `"..."` a string (MySQL `ANSI_QUOTES` off), the introducer spans a
        // double-quoted body exactly as it spans a single-quoted one: the whole
        // `_charset"..."` is one String token covering the introducer prefix (ADR-0006),
        // the mirror of `_latin1'x'`. The doubled `""` and a backslash-escaped `\"` are
        // in-body, not terminators.
        for src in [r#"_latin1"x""#, r#"_utf8"a""b""#, r#"_utf8mb4"caf\"e""#] {
            let tokens = tokenize_with(src, &CHARSET_INTRODUCER_DQ_FEATURES)
                .expect("double-quoted charset introducer lexes");
            assert_eq!(tokens.len(), 1, "one token for {src:?}");
            assert_eq!(tokens[0].kind, TokenKind::String, "kind for {src:?}");
            assert_eq!(text(src, &tokens[0]), src, "span for {src:?}");
        }

        // The single-quote form still lexes as one String under the same preset.
        let tokens = tokenize_with(r#"_latin1'x'"#, &CHARSET_INTRODUCER_DQ_FEATURES).expect("ok");
        assert_eq!((tokens.len(), tokens[0].kind), (1, TokenKind::String));

        // `ANSI_QUOTES`-on semantics: where `"..."` is an *identifier* (double-quoted
        // strings off, `"` a symmetric identifier quote), the introducer is NOT applied —
        // `_latin1` stays an ordinary word and `"x"` is a separate quoted identifier, so
        // the two never fuse into one charset-tagged string.
        let src = r#"_latin1"x""#;
        let tokens = tokenize_with(src, &CHARSET_INTRODUCER_FEATURES).expect("ok");
        assert_eq!(
            (tokens[0].kind, text(src, &tokens[0])),
            (TokenKind::Word, "_latin1"),
        );
        assert_eq!(
            (tokens[1].kind, text(src, &tokens[1])),
            (TokenKind::QuotedIdent, r#""x""#),
        );
    }

    #[test]
    fn double_quoted_string_takes_precedence_over_identifier_quoting() {
        // `"a b"` is a String, `` `c d` `` is the identifier quote, and `'x'` still
        // works — the MySQL ANSI_QUOTES-off layout.
        let src = r#""a b" `c d` 'x'"#;
        let tokens = tokenize_with(src, &MYSQL_STRING_FEATURES).expect("mysql strings lex");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(text(src, &tokens[0]), r#""a b""#);
        assert_eq!(tokens[1].kind, TokenKind::QuotedIdent);
        assert_eq!(tokens[2].kind, TokenKind::String);

        // ANSI still lexes `"a b"` as a quoted identifier (behaviour unchanged).
        assert_eq!(
            tokenize(r#""a b""#).expect("ansi")[0].kind,
            TokenKind::QuotedIdent,
        );
    }

    #[test]
    fn backslash_escapes_keep_an_escaped_quote_inside_the_string() {
        // With escapes on, `\'` does not terminate, so `'a\'b'` is one token.
        let tokens =
            tokenize_with(r"'a\'b'", &MYSQL_STRING_FEATURES).expect("backslash-escaped quote");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(text(r"'a\'b'", &tokens[0]), r"'a\'b'");

        // ANSI (no backslash escapes): `'a\'` closes, then `b`, then an unterminated `'`.
        assert_eq!(
            tokenize(r"'a\'b'")
                .expect_err("trailing quote never closes")
                .kind,
            LexErrorKind::UnterminatedString,
        );
    }

    /// A preset enabling every radix prefix plus underscore separators (PG 14-like),
    /// with `reject_trailing_junk` off so the loose split-token behaviour (`0xZ` -> `0`
    /// then `xZ`, a trailing `1_` -> number then word) stays observable — the strict
    /// PostgreSQL reject is exercised separately under `FeatureSet::POSTGRES`.
    const RADIX_NUMBER_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
            hex_integers: true,
            octal_integers: true,
            binary_integers: true,
            underscore_separators: true,
            // Loose fixture: the leading-underscore radix opener stays off, so `0x_1F`
            // keeps the `0` + word split the reject-off tests below observe.
            radix_leading_underscore: false,
            money_literals: false,
            reject_trailing_junk: false,
        }));

    #[test]
    fn radix_prefixed_integers_lex_as_one_number_only_when_enabled() {
        for src in ["0x1F", "0Xbeef", "0o17", "0b1010"] {
            let tokens = tokenize_with(src, &RADIX_NUMBER_FEATURES).expect("radix integer lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Number);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // Disabled (ANSI): `0x1F` splits into the number `0` and the word `x1F`.
        assert_eq!(
            lexed("0x1F"),
            [(TokenKind::Number, "0"), (TokenKind::Word, "x1F")],
        );

        // A prefix with no valid radix digit falls through to `0` + word.
        assert_eq!(
            tokenize_with("0xZ", &RADIX_NUMBER_FEATURES).expect("ok")[0].kind,
            TokenKind::Number,
        );
        assert_eq!(
            text(
                "0xZ",
                &tokenize_with("0xZ", &RADIX_NUMBER_FEATURES).expect("ok")[0]
            ),
            "0",
        );
    }

    #[test]
    fn underscore_separators_join_digit_groups_only_when_enabled() {
        // Enabled: `_` between digits is part of the number, across decimal,
        // fractional, exponent, and radix forms.
        for src in ["1_500_000", "1_000.000_5", "1_0e1_0", "0xFF_FF"] {
            let tokens =
                tokenize_with(src, &RADIX_NUMBER_FEATURES).expect("separated number lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Number);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // A trailing `_` is not a separator: the number stops, `_000` is a word.
        let src = "1_000";
        let tokens = tokenize_with(src, &RADIX_NUMBER_FEATURES).expect("ok");
        assert_eq!(
            tokens.len(),
            1,
            "separator between digits stays in the number"
        );
        let trailing = tokenize_with("1_", &RADIX_NUMBER_FEATURES).expect("ok");
        assert_eq!(
            trailing.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Number, TokenKind::Word],
        );

        // Disabled (ANSI): `_` is an identifier byte, so `1_500` is `1` + `_500`.
        assert_eq!(
            lexed("1_500"),
            [(TokenKind::Number, "1"), (TokenKind::Word, "_500")],
        );
    }

    /// PostgreSQL over the same radix/separator surface as [`RADIX_NUMBER_FEATURES`] but
    /// with `reject_trailing_junk` forced *off* — the dialect override that proves the
    /// trailing-junk reject and strict-`_` placement are honoured off the shared table,
    /// not baked into the scanner (test both the default and an override).
    const POSTGRES_LOOSE_NUMBERS: FeatureSet =
        FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
            reject_trailing_junk: false,
            ..NumericLiteralSyntax::POSTGRES
        }));

    /// The 18 malformed numeric forms `numerology.sql` proves PostgreSQL parse-rejects
    /// (pg-numeric-literal-lexer-validation): an identifier glued to a number, a radix
    /// prefix with missing/invalid digits, junk/empty exponents, and misplaced `_`
    /// separators. Every one decomposes to "trailing junk after a numeric literal".
    const TRAILING_JUNK_NUMBERS: &[&str] = &[
        "123abc", "0x", "1x", "0x0y", "0x0o", "0b", "1b", "0b0x", "0o", "1o", "0o0x", "0.a",
        "0.0a", ".0a", "0.0e1a", "0.0e", "100_", "100__000",
    ];

    #[test]
    fn postgres_rejects_trailing_junk_after_numeric_literals() {
        // The reject direction (PostgreSQL, `reject_trailing_junk` on): each malformed
        // form is a single `TrailingJunkAfterNumber` lex error whose span covers the
        // whole bad lexeme (the number *and* the abutting identifier run), mirroring
        // PG's `{decinteger}{identifier}` munch.
        for &src in TRAILING_JUNK_NUMBERS {
            let err = tokenize_with(src, &FeatureSet::POSTGRES)
                .expect_err(&format!("{src:?} is trailing junk under PostgreSQL"));
            assert_eq!(err.kind, LexErrorKind::TrailingJunkAfterNumber, "{src:?}");
            assert_eq!(
                &src[err.span.start() as usize..err.span.end() as usize],
                src,
                "{src:?} error span covers the whole malformed lexeme",
            );
        }

        // Strict `_` placement is the same knob: a `_` not flanked by two digits stops the
        // number and surfaces as junk, so these misplaced-separator forms reject too.
        for &src in &["1_000_.5", "1_000._5", "1_000.5_", "1_000.5e_1"] {
            let err = tokenize_with(src, &FeatureSet::POSTGRES).expect_err(&format!(
                "{src:?} has a misplaced separator under PostgreSQL"
            ));
            assert_eq!(err.kind, LexErrorKind::TrailingJunkAfterNumber, "{src:?}");
        }
    }

    #[test]
    fn trailing_junk_reject_is_dialect_data_not_baked_in() {
        // The accept direction, proven by a *same-dialect override*: flip
        // `reject_trailing_junk` off on PostgreSQL and every malformed form lexes cleanly
        // again — the number, then its trailing text as an ordinary word (the loose
        // DuckDB/MySQL behaviour) — so the reject is dialect data a future dialect cannot
        // bypass, not a hard-wired scanner rule.
        for &src in TRAILING_JUNK_NUMBERS {
            let tokens = tokenize_with(src, &POSTGRES_LOOSE_NUMBERS)
                .unwrap_or_else(|err| panic!("{src:?} must lex loosely with the flag off: {err}"));
            assert!(
                tokens.len() >= 2,
                "{src:?} splits into a number and trailing text"
            );
            assert_eq!(
                tokens[0].kind,
                TokenKind::Number,
                "{src:?} opens with a number"
            );
        }

        // `123abc` is the canonical split: number `123`, then the word `abc`.
        assert_eq!(
            tokenize_with("123abc", &POSTGRES_LOOSE_NUMBERS)
                .expect("loose")
                .iter()
                .map(|t| (t.kind, text("123abc", t)))
                .collect::<Vec<_>>(),
            [(TokenKind::Number, "123"), (TokenKind::Word, "abc")],
        );
    }

    #[test]
    fn postgres_lexes_valid_radix_and_separator_numbers_as_one_token() {
        // Zero new over-rejection: every valid PG-17 numeric form — radix integers, `_`
        // grouping (including a radix body that opens with `_`, `0x_…`), the plain
        // decimal/scientific forms, and a bare `1.`/`.5` — stays a single `Number`
        // spanning its whole text, under the same `reject_trailing_junk`-on dialect.
        for src in [
            "0x42F",
            "0o273",
            "0b100101",
            "0x1F",
            "1_000_000",
            "1_2_3",
            "0b_10_0101",
            "0x1EEE_FFFF",
            "0o2_73",
            "1_000.000_005",
            "1_000.",
            ".000_005",
            "1_000.5e0_1",
            "1e10",
            ".5",
            "5.",
            "1.5",
        ] {
            let tokens = tokenize_with(src, &FeatureSet::POSTGRES)
                .unwrap_or_else(|err| panic!("{src:?} is a valid PostgreSQL number: {err}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Number, "{src:?}");
            assert_eq!(text(src, &tokens[0]), src, "{src:?} spans its whole text");
        }
    }

    #[test]
    fn sqlite_numeric_boundary_matches_the_engine() {
        // The measured rusqlite 3.53.2 boundary (sqlite-numeric-trailing-junk-over-acceptance):
        // SQLite lexes a numeric literal abutting identifier chars as one TK_ILLEGAL token,
        // rejects a misplaced/leading `_`, but accepts interior `_` separators (3.46+). The
        // fitted Sqlite preset now carries `reject_trailing_junk` + `underscore_separators`
        // (with `radix_leading_underscore` OFF), so it reproduces the boundary exactly.
        for &src in &[
            // trailing junk after a numeric literal
            "1SETECT",
            ".122ualCvT",
            "2ES",
            "123abc",
            "1a",
            "1.a",
            ".1a",
            "1.5x",
            // bad radix body / junk exponent
            "0x1g",
            "0xG",
            "0x",
            "0x1Fz",
            "1e5x",
            "1e",
            // misplaced or leading `_`
            "1_",
            "1__0",
            "1._5",
            // leading-underscore radix body: PG accepts `0x_1F`, SQLite rejects it
            "0x_1F",
        ] {
            let err = tokenize_with(src, &FeatureSet::SQLITE)
                .expect_err(&format!("{src:?} is trailing junk under SQLite"));
            assert_eq!(err.kind, LexErrorKind::TrailingJunkAfterNumber, "{src:?}");
        }

        // The accept side stays one Number token: valid radix, interior `_` across decimal,
        // fraction, exponent, and radix bodies (the `1_000_000` regression the flip could
        // otherwise have caused).
        for src in [
            "0x1F",
            "1e5",
            "1.5",
            ".5",
            "1_000",
            "1_000_000",
            "0x1_F",
            "1.5_5",
            "1e1_0",
        ] {
            let tokens = tokenize_with(src, &FeatureSet::SQLITE)
                .unwrap_or_else(|err| panic!("{src:?} is a valid SQLite number: {err}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Number, "{src:?}");
            assert_eq!(text(src, &tokens[0]), src, "{src:?} spans its whole text");
        }
    }

    #[test]
    fn radix_leading_underscore_is_dialect_data_not_baked_in() {
        // Prove the axis rides the shared table, not a hard-wired scanner rule,
        // by toggling it on both a default (SQLite: off) and an override. `0x_1F` — a radix
        // body opening with `_` — rejects as trailing junk when off, and lexes as one hex
        // Number when the same dialect flips the flag on.
        let src = "0x_1F";
        let off = tokenize_with(src, &FeatureSet::SQLITE)
            .expect_err("SQLite rejects a leading-underscore radix body");
        assert_eq!(off.kind, LexErrorKind::TrailingJunkAfterNumber);

        let sqlite_with_leading =
            FeatureSet::SQLITE.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
                radix_leading_underscore: true,
                ..FeatureSet::SQLITE.numeric_literals
            }));
        let tokens = tokenize_with(src, &sqlite_with_leading)
            .expect("the flag flip admits the leading-underscore radix body");
        assert_eq!(tokens.len(), 1, "one Number token");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(text(src, &tokens[0]), src);
    }

    #[test]
    fn sqlite_dollar_is_an_identifier_continuation_byte() {
        // The measured rusqlite boundary (sqlite-lexer-under-acceptance-bundle): SQLite's
        // IdChar set includes `$` as a *continuation* byte, so `L$C3`/`a$b`/`t$x`/`a$` are one
        // identifier each, while a *leading* `$name` is the dollar-named placeholder and a lone
        // `$` a stray byte. The fitted Sqlite preset carries `dollar_in_identifiers`.
        for src in ["L$C3", "a$b", "t$x", "a$"] {
            let tokens = tokenize_with(src, &FeatureSet::SQLITE)
                .unwrap_or_else(|err| panic!("{src:?} is one SQLite identifier: {err}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Word, "{src:?}");
            assert_eq!(text(src, &tokens[0]), src, "{src:?} spans its whole text");
        }
        // Leading `$name` stays the placeholder; a lone `$` is a stray byte.
        let param = tokenize_with("$abc", &FeatureSet::SQLITE).expect("$abc is a placeholder");
        assert_eq!(param[0].kind, TokenKind::Parameter);
        assert_eq!(text("$abc", &param[0]), "$abc");
        assert_eq!(
            tokenize_with("$", &FeatureSet::SQLITE)
                .expect_err("a lone $ is a stray byte")
                .kind,
            LexErrorKind::StrayByte,
        );
        // Dialect data, not baked in: the same SQLite preset with the flag off stops the word
        // at the `$`, so `a$b` is the word `a` then the `$b` dollar-named placeholder.
        let off =
            FeatureSet::SQLITE.with(FeatureDelta::EMPTY.identifier_syntax(IdentifierSyntax {
                dollar_in_identifiers: false,
                ..FeatureSet::SQLITE.identifier_syntax
            }));
        let toks = tokenize_with("a$b", &off).expect("`a` then `$b`");
        assert_eq!(text("a$b", &toks[0]), "a", "the word stops at `$` when off");
        assert_eq!(
            text("a$b", &toks[1]),
            "$b",
            "`$b` is a placeholder when off"
        );
        assert_eq!(toks[1].kind, TokenKind::Parameter);
    }

    #[test]
    fn sqlite_empty_quoted_identifier_lexes_in_every_style() {
        // The measured rusqlite boundary: SQLite admits a zero-length quoted identifier in
        // every quote style (`empty_quoted_identifiers`), unique among the shipped engines.
        for src in ["``", "\"\"", "[]"] {
            let tokens = tokenize_with(src, &FeatureSet::SQLITE)
                .unwrap_or_else(|err| panic!("{src:?} is an empty SQLite quoted ident: {err}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::QuotedIdent, "{src:?}");
            assert_eq!(text(src, &tokens[0]), src, "{src:?} spans its delimiters");
        }
        // Dialect data, not baked in: the same SQLite preset with the flag off restores the
        // universal zero-length reject for every style.
        let off =
            FeatureSet::SQLITE.with(FeatureDelta::EMPTY.identifier_syntax(IdentifierSyntax {
                empty_quoted_identifiers: false,
                ..FeatureSet::SQLITE.identifier_syntax
            }));
        for src in ["``", "\"\"", "[]"] {
            assert_eq!(
                tokenize_with(src, &off)
                    .expect_err(&format!("{src:?} rejects with the flag off"))
                    .kind,
                LexErrorKind::ZeroLengthDelimitedIdentifier,
                "{src:?}",
            );
        }
    }

    #[test]
    fn sqlite_block_comment_eof_and_nesting_match_the_engine() {
        // The measured rusqlite boundary: SQLite silently closes an unterminated `/* …` at EOF
        // (as long as a byte follows the `/*`) and does NOT nest block comments. The fitted
        // Sqlite preset carries `unterminated_block_comment_at_eof` and `nested_block_comments`
        // off. `1/*x` -> just the `1` (comment is trailing trivia); `/*x`/`/* a /* b */` are all
        // trivia (empty token stream).
        for src in ["/*x", "/* a /* b */", "/* x ", "/**"] {
            let tokens = tokenize_with(src, &FeatureSet::SQLITE)
                .unwrap_or_else(|err| panic!("{src:?}: {err}"));
            assert!(tokens.is_empty(), "{src:?} is all trivia, got {tokens:?}");
        }
        let one = tokenize_with("1/*x", &FeatureSet::SQLITE).expect("1 then a closed comment");
        assert_eq!(one.len(), 1, "just the `1`");
        assert_eq!(one[0].kind, TokenKind::Number);
        // A *bare* `/*` at EOF (no byte after `*`) is the `/` slash operator, not a comment.
        assert_eq!(
            tokenize_with("/*", &FeatureSet::SQLITE)
                .expect("bare /* at EOF lexes as operators")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TokenKind::Operator(Operator::Slash),
                TokenKind::Operator(Operator::Star),
            ],
        );
        // Non-nesting leaves the tail after the first `*/`: `/* a /* b */ c */` -> `c` then `*/`.
        let tail = tokenize_with("/* a /* b */ c */", &FeatureSet::SQLITE).expect("tail leaks");
        assert_eq!(
            text("/* a /* b */ c */", &tail[0]),
            "c",
            "first `*/` closes the comment"
        );
        // Dialect data, not baked in: with the EOF flag off the same input is the hard error.
        let off = FeatureSet::SQLITE.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
            unterminated_block_comment_at_eof: false,
            ..FeatureSet::SQLITE.comment_syntax
        }));
        assert_eq!(
            tokenize_with("1/*x", &off)
                .expect_err("unterminated at EOF errors with the flag off")
                .kind,
            LexErrorKind::UnterminatedBlockComment,
        );
    }

    #[test]
    fn sqlite_numbered_question_parameter_matches_the_engine() {
        // The measured rusqlite boundary: SQLite numbered `?NNN` parameters (`numbered_question`).
        // The number is a maximal digit munch, so `?1abc` is `?1` then the identifier `abc`.
        for src in ["?1", "?123", "?32766"] {
            let tokens = tokenize_with(src, &FeatureSet::SQLITE)
                .unwrap_or_else(|err| panic!("{src:?} is a numbered parameter: {err}"));
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::Parameter, "{src:?}");
            assert_eq!(text(src, &tokens[0]), src, "{src:?} spans the whole `?NNN`");
        }
        let split = tokenize_with("?1abc", &FeatureSet::SQLITE).expect("?1abc lexes");
        assert_eq!(
            text("?1abc", &split[0]),
            "?1",
            "the number is a maximal munch"
        );
        assert_eq!(split[0].kind, TokenKind::Parameter);
        assert_eq!(text("?1abc", &split[1]), "abc");
        // Dialect data, not baked in: with numbering off but the anonymous `?` still on, `?1`
        // splits into a bare `?` placeholder and the number `1`.
        let off = FeatureSet::SQLITE.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            numbered_question: false,
            ..FeatureSet::SQLITE.parameters
        }));
        let anon = tokenize_with("?1", &off).expect("?1 lexes as `?` then `1`");
        assert_eq!(
            text("?1", &anon[0]),
            "?",
            "the `?` is the anonymous placeholder"
        );
        assert_eq!(anon[0].kind, TokenKind::Parameter);
        assert_eq!(text("?1", &anon[1]), "1");
    }

    #[test]
    fn unicode_escape_string_lexes_only_when_enabled() {
        // Enabled: `U&'d\0061t\+000061'` is one String including the `U&` prefix;
        // the `\` is body, not a quote escape.
        let src = r"U&'a\0062''c'";
        let tokens = tokenize_with(src, &UNICODE_STRING_FEATURES).expect("unicode strings lex");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(text(src, &tokens[0]), src, "doubled '' stays in the body");

        // Disabled (ANSI): `U` word, `&` operator, `'...'` string — three tokens.
        assert_eq!(
            lexed("U&'x'"),
            [
                (TokenKind::Word, "U"),
                (TokenKind::Operator(Operator::Amp), "&"),
                (TokenKind::String, "'x'"),
            ],
        );
    }

    #[test]
    fn unicode_escape_identifier_lexes_only_on_the_full_trigger() {
        // Enabled: `U&"d\0061ta"` is one QuotedIdent spanning the `U&` prefix; the `\`
        // is body (no escape decoding at lex time), and the case-insensitive `u&"` lead
        // works too. Disjoint from the `U&'...'` string arm by the third byte.
        for src in [r#"U&"d\0061ta""#, r#"u&"x""#, r#"U&"""""#] {
            let tokens = tokenize_with(src, &UNICODE_STRING_FEATURES).expect("unicode ident lexes");
            assert_eq!(tokens.len(), 1, "{src}");
            assert_eq!(tokens[0].kind, TokenKind::QuotedIdent, "{src}");
            assert_eq!(text(src, &tokens[0]), src, "{src} spans the `U&` prefix");
        }

        // The full `U&"` trigger is required: a bare `U&` before a non-quote stays a `U`
        // word, a `&` operator, and the following token — `U` is identifier-start, so this
        // arm must never steal a plain identifier followed by the `&` operator.
        let bare = tokenize_with("U&1", &UNICODE_STRING_FEATURES).expect("U&1 lexes");
        assert_eq!(
            bare.iter()
                .map(|t| (t.kind, text("U&1", t)))
                .collect::<Vec<_>>(),
            [
                (TokenKind::Word, "U"),
                (TokenKind::Operator(Operator::Amp), "&"),
                (TokenKind::Number, "1"),
            ],
        );

        // Disabled (ANSI): `U&"x"` decomposes into a `U` word, `&` operator, and a plain
        // quoted identifier — three tokens.
        assert_eq!(
            lexed(r#"U&"x""#),
            [
                (TokenKind::Word, "U"),
                (TokenKind::Operator(Operator::Amp), "&"),
                (TokenKind::QuotedIdent, r#""x""#),
            ],
        );
    }

    #[test]
    fn bit_string_lexes_as_one_string_token_only_when_enabled() {
        // Enabled: `B'1010'` / `X'1FF'` are single String tokens spanning the marker,
        // in either case. A malformed body (`X'1FG'`) still lexes — the digit check is
        // deferred to the accessor, mirroring PostgreSQL.
        for src in ["B'1010'", "b'1010'", "X'1FF'", "x'1ff'", "X'1FG'"] {
            let tokens = tokenize_with(src, &BIT_STRING_FEATURES).expect("bit strings lex");
            assert_eq!(tokens.len(), 1, "{src}");
            assert_eq!(tokens[0].kind, TokenKind::String, "{src}");
            assert_eq!(text(src, &tokens[0]), src, "{src} spans the marker");
        }

        // The quote must abut the marker: `B '1010'` is a word then a string.
        assert_eq!(
            tokenize_with("B '1010'", &BIT_STRING_FEATURES)
                .expect("ok")
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [TokenKind::Word, TokenKind::String],
        );

        // Disabled (ANSI): `B` is a word, `'1010'` a separate string.
        assert_eq!(
            lexed("B'1010'"),
            [(TokenKind::Word, "B"), (TokenKind::String, "'1010'")],
        );
    }

    #[test]
    fn blob_literal_lexes_eagerly_validated_even_hex_only_when_enabled() {
        // Enabled (SQLite): an even count of hex digits — including the zero-byte `x''`
        // — is a single String token spanning the marker, in either marker case.
        for src in ["x'53514C'", "X'53514c'", "x'1A'", "x''"] {
            let tokens = tokenize_with(src, &BLOB_FEATURES).expect("even-hex blobs lex");
            assert_eq!(tokens.len(), 1, "{src}");
            assert_eq!(tokens[0].kind, TokenKind::String, "{src}");
            assert_eq!(text(src, &tokens[0]), src, "{src} spans the marker");
        }

        // Zero new over-acceptance: an odd digit count or a non-hex body is an eager
        // lex-time reject — SQLite's "unrecognized token" / MySQL's ER_PARSE_ERROR
        // (probed) — with the error span covering the whole malformed lexeme.
        for src in ["x'ABC'", "X'1FF'", "x'0'", "x'XY'"] {
            let err = tokenize_with(src, &BLOB_FEATURES)
                .expect_err(&format!("{src:?} is a malformed blob"));
            assert_eq!(err.kind, LexErrorKind::MalformedBlobLiteral, "{src:?}");
            assert_eq!(
                err.span,
                Span::new(0, src.len() as u32),
                "{src:?} spans the whole lexeme",
            );
        }

        // An unterminated body stays the unterminated-string reject, not a malformed
        // blob (there is no closing quote to validate against).
        assert_eq!(
            tokenize_with("x'1A", &BLOB_FEATURES)
                .expect_err("unterminated")
                .kind,
            LexErrorKind::UnterminatedString,
        );

        // The quote must abut the marker (`x '1A'` is a word then a string), and `B`
        // never triggers the blob arm — with bit strings off it stays a word.
        for src in ["x '1A'", "B'1010'"] {
            assert_eq!(
                tokenize_with(src, &BLOB_FEATURES)
                    .expect("ok")
                    .iter()
                    .map(|t| t.kind)
                    .collect::<Vec<_>>(),
                [TokenKind::Word, TokenKind::String],
                "{src}",
            );
        }

        // Disabled (ANSI): `x` is a word, `'1A'` a separate string.
        assert_eq!(
            lexed("x'1A'"),
            [(TokenKind::Word, "x"), (TokenKind::String, "'1A'")],
        );

        // The MySQL layout (both flags on): `x`/`X` resolves to the eager blob arm — the
        // odd-hex body that a deferred bit-string would tolerate rejects — while `B'…'`
        // keeps the deferred bit-string scan (`B'1FG'`-style bodies still lex; the digit
        // check stays at the accessor).
        assert_eq!(
            tokenize_with("x'ABC'", &BLOB_AND_BIT_STRING_FEATURES)
                .expect_err("the eager blob arm claims x")
                .kind,
            LexErrorKind::MalformedBlobLiteral,
        );
        let tokens =
            tokenize_with("B'1012'", &BLOB_AND_BIT_STRING_FEATURES).expect("B stays deferred");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
    }

    #[test]
    fn custom_identifier_start_need_not_also_be_continue() {
        const FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .byte_classes(ByteClasses::STANDARD.with_class(b'@', CLASS_IDENTIFIER_START)),
        );

        let tokens = tokenize_with("@name", &FEATURES).expect("custom start byte lexes");

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Word);
        assert_eq!(text("@name", &tokens[0]), "@name");
    }

    #[test]
    fn non_ascii_identifier_round_trips_through_its_span() {
        // A word may start with and contain multi-byte UTF-8; the span must land
        // on char boundaries so slicing does not panic.
        let src = "café = δ";
        let tokens = tokenize(src).expect("clean");
        assert_eq!(tokens[0].kind, TokenKind::Word);
        assert_eq!(text(src, &tokens[0]), "café");
        assert_eq!(tokens[2].kind, TokenKind::Word);
        assert_eq!(text(src, &tokens[2]), "δ");
    }

    #[test]
    fn unicode_letters_start_and_continue_identifiers() {
        // The identifier-start/continue policy is the Unicode *letter* property
        // (`char::is_alphabetic`), not ASCII-only: accented Latin, Greek, and CJK
        // letters all begin and continue an unquoted identifier as one Word.
        for src in ["café", "naïve", "δ", "Ünüm", "表", "数据", "_underscore"] {
            let tokens = tokenize(src).expect("Unicode-letter identifier lexes");
            assert_eq!(tokens.len(), 1, "{src:?} is one word");
            assert_eq!(tokens[0].kind, TokenKind::Word);
            assert_eq!(text(src, &tokens[0]), src, "the whole word is the span");
        }
    }

    #[test]
    fn non_letter_code_points_are_not_identifier_starts() {
        // A non-letter code point does not begin an identifier under the Unicode
        // policy — deliberately stricter than a raw "any high byte" rule, which would
        // silently fold an emoji or symbol into a word. The stray-byte span covers the
        // whole character, so the error offset stays on a char boundary.
        let err = tokenize("🎉").expect_err("an emoji is not an identifier start");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 4), "span is the whole 4-byte char");

        // A symbol mid-stream is likewise rejected (here U+00B0 DEGREE SIGN, 2 bytes,
        // after a clean word and a space).
        let err = tokenize("a °").expect_err("a symbol is not an identifier char");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(2, 4));
    }

    #[test]
    fn digits_continue_but_never_start_an_identifier() {
        // An ASCII digit never starts an identifier: a leading digit is the number,
        // and the trailing letters are a separate word.
        assert_eq!(
            lexed("1abc"),
            [(TokenKind::Number, "1"), (TokenKind::Word, "abc")],
        );

        // A Unicode digit (Arabic-Indic `٣`, which has the numeric property) continues
        // an identifier but does not start one — mirroring ASCII digits.
        let cont = tokenize("a٣").expect("a Unicode digit continues an identifier");
        assert_eq!(cont.len(), 1);
        assert_eq!(cont[0].kind, TokenKind::Word);
        assert_eq!(text("a٣", &cont[0]), "a٣");

        // Leading, that same digit neither starts an identifier nor (being non-ASCII)
        // a number, so it is a stray byte.
        let err = tokenize("٣").expect_err("a Unicode digit does not start an identifier");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
    }

    /// An ANSI-based preset with only the `$`-in-identifier policy flipped on, so the
    /// effect is attributable to that one dialect-data knob and nothing else.
    const DOLLAR_IDENT_FEATURES: FeatureSet =
        FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_syntax(IdentifierSyntax {
            dollar_in_identifiers: true,
            ..IdentifierSyntax::ANSI
        }));

    #[test]
    fn dollar_in_identifiers_is_dialect_data() {
        // PostgreSQL accepts `$` as an identifier-continue character, so `foo$bar` is a
        // single Word; `a$1` shows `$` and a digit both continuing after a letter start.
        for src in ["foo$bar", "a$1"] {
            let tokens =
                tokenize_with(src, &FeatureSet::POSTGRES).expect("PG accepts `$` in identifiers");
            assert_eq!(tokens.len(), 1, "{src:?} is one word under PostgreSQL");
            assert_eq!(tokens[0].kind, TokenKind::Word);
            assert_eq!(text(src, &tokens[0]), src);
        }

        // Strict ANSI forbids `$` in identifiers: `foo` ends at the `$`, which is then a
        // stray byte (ANSI has no `$` lexeme). Same source, different acceptance — the
        // difference is dialect *data*, not tree shape.
        let err = tokenize("foo$bar").expect_err("ANSI forbids `$` in identifiers");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(
            err.span,
            Span::new(3, 4),
            "the `$` after `foo` is the stray byte"
        );

        // Flipping only the one knob on an ANSI base reproduces the PostgreSQL
        // acceptance, proving it is exactly that knob doing the work.
        let tokens =
            tokenize_with("foo$bar", &DOLLAR_IDENT_FEATURES).expect("the flag alone enables `$`");
        assert_eq!(tokens.len(), 1);
        assert_eq!(text("foo$bar", &tokens[0]), "foo$bar");
    }

    #[test]
    fn quoted_identifier_bypasses_the_unicode_identifier_policy() {
        // Quoting accepts characters the unquoted policy rejects — an emoji, `$`,
        // punctuation, spaces — as one QuotedIdent spanning the delimiters. The policy
        // governs *unquoted* identifiers only.
        for src in ["\"🎉\"", "\"a b$%\"", "\"δ\""] {
            let tokens = tokenize(src).expect("a quoted identifier accepts anything");
            assert_eq!(tokens.len(), 1, "{src:?} is one token");
            assert_eq!(tokens[0].kind, TokenKind::QuotedIdent);
            assert_eq!(text(src, &tokens[0]), src);
        }
    }

    #[test]
    fn empty_and_trivia_only_inputs_yield_no_tokens() {
        assert!(tokenize("").expect("ok").is_empty());
        assert!(tokenize("   \t\n  ").expect("ok").is_empty());
        assert!(tokenize("-- just a comment").expect("ok").is_empty());
        assert!(
            tokenize("/* only */ /* comments */")
                .expect("ok")
                .is_empty()
        );
    }

    #[test]
    fn unterminated_string_reports_a_lex_error_with_the_full_span() {
        let src = "SELECT 'it''s";
        let err = tokenize(src).expect_err("the literal never closes");
        assert_eq!(err.kind, LexErrorKind::UnterminatedString);
        // Span runs from the opening quote to end of input.
        assert_eq!(err.span, Span::new(7, 13));
        assert_eq!(
            &src[err.span.start() as usize..err.span.end() as usize],
            "'it''s"
        );
    }

    #[test]
    fn other_unterminated_constructs_report_their_kinds() {
        assert_eq!(
            tokenize(r#""abc"#).expect_err("no close").kind,
            LexErrorKind::UnterminatedQuotedIdent,
        );
        assert_eq!(
            tokenize("/* open").expect_err("no close").kind,
            LexErrorKind::UnterminatedBlockComment,
        );
        assert_eq!(
            tokenize("/* a /* b */")
                .expect_err("inner closes, outer does not")
                .kind,
            LexErrorKind::UnterminatedBlockComment,
        );
        assert_eq!(
            tokenize_with("$tag$ body", &FeatureSet::POSTGRES)
                .expect_err("no close")
                .kind,
            LexErrorKind::UnterminatedDollarQuote,
        );
    }

    #[test]
    fn stray_byte_reports_an_error_at_its_offset() {
        // `@` is in no lexical class.
        let err = tokenize("a @ b").expect_err("@ is stray");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(2, 3));

        // A `$` that opens no valid dollar-quote is also stray.
        let err = tokenize("$ x").expect_err("lone $ is stray");
        assert_eq!(err.kind, LexErrorKind::StrayByte);
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn token_cursor_peeks_advances_and_seeks_for_backtracking() {
        let tokens = tokenize("d , b").expect("clean");
        let mut cursor = TokenCursor::new(&tokens);

        assert_eq!(cursor.remaining(), 3);
        assert_eq!(cursor.peek().map(|t| t.kind), Some(TokenKind::Word));
        assert_eq!(
            cursor.peek_nth(1).map(|t| t.kind),
            Some(TokenKind::Punctuation(Punctuation::Comma)),
        );

        let checkpoint = cursor.pos();
        assert_eq!(cursor.advance().map(|t| t.kind), Some(TokenKind::Word));
        assert_eq!(
            cursor.advance().map(|t| t.kind),
            Some(TokenKind::Punctuation(Punctuation::Comma))
        );
        assert_eq!(cursor.pos(), 2);

        // Rewind to the checkpoint and re-read — the speculation seam.
        cursor.seek(checkpoint);
        assert_eq!(cursor.pos(), 0);
        assert_eq!(cursor.advance().map(|t| t.kind), Some(TokenKind::Word));

        // Drain to the end.
        cursor.seek(tokens.len());
        assert!(cursor.is_eof());
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(cursor.advance(), None);
    }

    #[test]
    fn buffered_token_cursor_grows_for_lookahead_and_rewinds() {
        let mut cursor = BufferedTokenCursor::streaming("d.*", &FeatureSet::ANSI)
            .expect("streaming cursor starts");

        assert_eq!(cursor.buffered_len(), 0, "no token is scanned up front");

        let checkpoint = cursor.pos();
        assert_eq!(
            cursor
                .peek_nth(2)
                .expect("lookahead scans")
                .map(|token| token.kind),
            Some(TokenKind::Operator(Operator::Star)),
            "lookahead can cross the current buffer boundary",
        );
        assert_eq!(cursor.pos(), checkpoint, "lookahead does not consume");
        assert_eq!(cursor.buffered_len(), 3, "lookahead grew the buffer");

        assert_eq!(
            cursor
                .advance()
                .expect("first token")
                .map(|token| token.kind),
            Some(TokenKind::Word),
        );
        assert_eq!(
            cursor
                .advance()
                .expect("second token")
                .map(|token| token.kind),
            Some(TokenKind::Punctuation(Punctuation::Dot)),
        );
        cursor.seek(checkpoint);
        assert_eq!(
            cursor
                .advance()
                .expect("rewound token")
                .map(|token| token.kind),
            Some(TokenKind::Word),
            "checkpoint rewind reuses the retained lazy buffer",
        );
    }

    #[test]
    fn buffered_token_cursor_discards_consumed_statement_tokens() {
        let mut cursor = BufferedTokenCursor::streaming("SELECT 1; SELECT 2", &FeatureSet::ANSI)
            .expect("streaming cursor starts");

        assert_eq!(
            cursor.advance().expect("select").map(|token| token.kind),
            Some(TokenKind::Keyword(Keyword::Select)),
        );
        assert_eq!(
            cursor.advance().expect("literal").map(|token| token.kind),
            Some(TokenKind::Number),
        );
        assert_eq!(
            cursor.advance().expect("semicolon").map(|token| token.kind),
            Some(TokenKind::Punctuation(Punctuation::Semicolon)),
        );

        assert_eq!(
            cursor
                .peek()
                .expect("next statement")
                .map(|token| token.kind),
            Some(TokenKind::Keyword(Keyword::Select)),
            "separator scanning may already hold one next-statement lookahead",
        );
        cursor.discard_consumed();
        assert_eq!(
            cursor.buffered_len(),
            1,
            "only the next statement lookahead remains buffered",
        );
        assert_eq!(
            cursor
                .peek()
                .expect("retained token")
                .map(|token| token.kind),
            Some(TokenKind::Keyword(Keyword::Select)),
        );
    }

    #[test]
    fn lex_error_displays_its_kind_and_range() {
        let err = LexError::new(LexErrorKind::StrayByte, Span::new(2, 3));
        assert_eq!(err.to_string(), "stray byte at bytes 2..3");
    }

    #[test]
    fn opt_in_trivia_capture_records_kinds_and_spans() {
        use TriviaKind::{BlockComment, LineComment, Whitespace};

        // Whitespace, a block comment, and a line comment between the lexemes, each a
        // distinct trivia run with its exact source span. `d`/`b` are not keywords in
        // the inventory, so they lex as plain words.
        let src = "d /*c*/ b -- tail\n";
        let (tokens, trivia) =
            tokenize_with_trivia(src, &FeatureSet::ANSI).expect("clean tokenization");

        // Token stream is exactly the two words — no trivia leaked in.
        assert_eq!(
            tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            [TokenKind::Word, TokenKind::Word],
        );

        // Every trivia run, in source order, with kind and the text its span slices.
        let recorded: Vec<_> = trivia
            .all()
            .iter()
            .map(|r| {
                let span = r.span();
                (r.kind(), &src[span.start() as usize..span.end() as usize])
            })
            .collect();
        assert_eq!(
            recorded,
            [
                (Whitespace, " "),
                (BlockComment, "/*c*/"),
                (Whitespace, " "),
                (Whitespace, " "),
                (LineComment, "-- tail"),
                (Whitespace, "\n"),
            ],
        );
    }

    #[test]
    fn trivia_capture_leaves_the_token_stream_identical() {
        // The opt-in capture must not perturb the token stream: the tokens are the
        // same with capture on as with it off (trivia stays strictly out-of-band).
        let src = "SELECT /* x */ a,\n  -- note\n  b FROM t";
        let plain = tokenize_with(src, &FeatureSet::ANSI).expect("plain lex");
        let (with_capture, trivia) =
            tokenize_with_trivia(src, &FeatureSet::ANSI).expect("captured lex");

        assert_eq!(
            plain, with_capture,
            "capture must not change the token stream"
        );
        assert!(!trivia.is_empty(), "the comments/whitespace were captured");
    }

    #[test]
    fn tokens_and_trivia_tile_the_source_without_overlap() {
        // Strong invariant: every source byte belongs either to exactly one token or
        // to exactly one trivia run — tokens and trivia partition the input. This is
        // the precise sense in which trivia is "out-of-band yet offset-recoverable".
        let src = "  SELECT /*c*/ a, -- z\n 'lit'  ";
        let (tokens, trivia) =
            tokenize_with_trivia(src, &FeatureSet::ANSI).expect("clean tokenization");

        // Merge token and trivia spans, sort by start, and walk: each must begin
        // exactly where the previous ended, covering [0, len) with no gap or overlap.
        let mut spans: Vec<Span> = tokens.iter().map(|t| t.span).collect();
        spans.extend(trivia.all().iter().map(|r| r.span()));
        spans.sort_by_key(|s| (s.start(), s.end()));

        let mut cursor = 0u32;
        for span in &spans {
            assert_eq!(span.start(), cursor, "no gap or overlap before {span:?}");
            cursor = span.end();
        }
        assert_eq!(cursor, src.len() as u32, "spans cover the whole source");
    }

    #[test]
    fn trivia_free_source_captures_an_empty_index() {
        // Capture on, but no comments/whitespace to record between the tokens: the
        // index is empty (and an empty index owns an empty, unallocated `Vec`).
        let (tokens, trivia) =
            tokenize_with_trivia("a+b", &FeatureSet::ANSI).expect("clean tokenization");
        assert_eq!(tokens.len(), 3);
        assert!(trivia.is_empty(), "no trivia between adjacent lexemes");
    }
}
