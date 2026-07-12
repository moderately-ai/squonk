// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The parser engine state and the low-level seams the grammar drives.
//!
//! [`Parser`] holds everything a parse needs: a token-index cursor,
//! the source (to recover token text and intern it), the live interner, the
//! error sink, the dialect, and the per-parse [`NodeId`] counter. The grammar
//! families (`m1-pratt-expr`, `m1-select-grammar`) are written entirely in terms
//! of the methods on this type, so the methods here — not the grammar — are the
//! deliberate, stable API. Each is a thin, single-purpose seam; the grammar
//! composes them.

use crate::ast::dialect::FeatureSet;
use crate::ast::{Meta, NodeId, Span, Symbol};
use crate::error::{ErrorSink, Expected, FailFastSink, Found, ParseError, ParseResult};
use crate::interner::Interner;
use crate::tokenizer::{BufferedTokenCursor, LexError, Token, TriviaIndex};

use super::Dialect;
use super::clause_marks::{ClauseKw, ClauseMark, ClauseMarkIndex};

/// The default recursive-descent depth limit (DoS-safety).
///
/// Deeply nested untrusted SQL is a denial-of-service vector: unbounded recursive
/// descent overflows the stack and aborts the process. The recursion guard rejects
/// such input with a clean [`ParseError`] instead, and this is the depth it trips
/// at unless [`Parser::with_recursion_limit`] overrides it.
///
/// The value is chosen for generous headroom below the empirically measured crash
/// depths: the from-scratch stress spike overflowed an 8 MB stack at ~721 nested
/// parentheses and ~406 nested subqueries. Capping recursion at 128 entries was
/// then measured (in a debug build, which spends *more* stack per frame than the
/// shipped release build) to complete every nesting vector — parentheses, the
/// several subquery forms, derived tables — within a **2 MiB stack**, the size
/// Rust gives a spawned thread by default and a sound "smallest commonly supported
/// stack". That is ~5–6× below the 8 MB crash depth and still far more than any
/// legitimate query needs (real SQL rarely nests past ~20). It mirrors
/// `sqlparser-rs` (default 50) with the extra headroom our higher measured crash
/// points allow. A deployment that parses untrusted SQL on threads smaller than
/// 2 MiB should lower the limit, and machine-generated deeply nested SQL on a large
/// stack can raise it, via [`Parser::with_recursion_limit`] or
/// [`ParseOptions`](super::ParseOptions).
pub const DEFAULT_RECURSION_LIMIT: usize = 128;

/// A saved cursor position for speculative parsing.
///
/// Returned by [`Parser::checkpoint`] and consumed by [`Parser::rewind`]. It
/// captures *only* the token position on purpose: interning is idempotent (a
/// re-interned word yields the same [`Symbol`]), and [`NodeId`]s are side-table
/// keys whose only requirement is per-parse uniqueness — discarded speculation
/// simply leaves a gap in the id sequence. Neither needs rolling back, so a
/// checkpoint is a single index, and backtracking costs nothing but a `seek`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pos: usize,
}

/// The parser engine: cursor + source + interner + sink + dialect + id counter.
///
/// `'a` is the lifetime of the tokenized input and its source; it never escapes
/// into the AST — [`span_text`](Self::span_text) hands out `&'a str`
/// only so the grammar can intern or classify token text during the parse.
pub struct Parser<'a, D: Dialect> {
    cursor: BufferedTokenCursor<'a>,
    source: &'a str,
    interner: Interner,
    errors: FailFastSink,
    dialect: D,
    /// One-based counter for the next [`NodeId`]. Starts at 1 so the very first
    /// id is non-zero (a [`NodeId`] is a `NonZeroU32`).
    next_node_id: u32,
    /// Memoized current token: the result of [`peek`](Self::peek) (= `peek_nth(0)`)
    /// together with the cursor position it was computed at. The token at a given
    /// absolute position never changes, so this hit is valid whenever the position
    /// is unchanged; `advance`/`rewind` move the position and the key simply misses,
    /// recomputing. The grammar re-peeks the current token several times per
    /// position (the Pratt loop's predicate/postfix/infix checks), so caching it
    /// turns those into a field read instead of a cursor descent. Both the result
    /// and the key are `Copy` (`LexError` is `Copy`), so a hit copies, never clones.
    peeked: Option<(usize, Result<Option<Token>, LexError>)>,
    /// Current recursive-descent nesting depth — the number of live
    /// [`RecursionGuard`]s. Bumped on entry to each genuine self-recursion point
    /// (nested expressions, subqueries, parenthesized join factors, nested
    /// statements) and restored on the guard's drop, so it tracks the live stack
    /// depth even across error/backtrack unwinds. A `usize` field read against
    /// `recursion_limit` is the whole per-entry cost (DoS-safety).
    recursion_depth: usize,
    /// Pending type-position `>` closes produced by consuming a `>>` (ShiftRight)
    /// under BigQuery angle-bracket types — see [`expect_angle_gt`](crate::parser::ty).
    pub(super) angle_gt_pending: u32,
    /// The depth at which [`enter_recursion`](Self::enter_recursion) refuses to go
    /// deeper and returns a clean error instead of overflowing the stack. Defaults
    /// to [`DEFAULT_RECURSION_LIMIT`]; override with
    /// [`with_recursion_limit`](Self::with_recursion_limit).
    recursion_limit: usize,
    /// Off-by-default consumer request (the parser's
    /// [`ParseOptions::parse_float_as_decimal`](crate::parser::ParseOptions::parse_float_as_decimal)):
    /// when set, a fractional/scientific numeric literal is classified
    /// [`LiteralKind::Decimal`](crate::ast::LiteralKind::Decimal) rather than
    /// [`LiteralKind::Float`](crate::ast::LiteralKind::Float), the sole effect the flag
    /// has. Read at every numeric-literal classification site through
    /// [`number_literal_kind`](super::expr::number_literal_kind); when unset (always in
    /// the default parse paths) the classification is byte-for-byte the historical one.
    parse_float_as_decimal: bool,
    /// One-shot flag: the query about to be parsed sits in a *grouping* position where a
    /// leading parenthesized query is a table-or-subquery / scalar-subquery grouping (a
    /// complete standalone primary), not a compound operand. Set by the two grouping
    /// helpers (`from::try_parenthesized_query_factor`, `expr::try_parenthesized_query`)
    /// immediately before their `parse_query`, consumed by the body's `parse_set_expr_bp`,
    /// and propagated one level down when a grouping paren directly nests another
    /// (`((SELECT 1))`). Only load-bearing when
    /// [`SelectSyntax::parenthesized_query_operands`](crate::ast::dialect::SelectSyntax)
    /// is off (SQLite); the other presets accept the paren operand outright and ignore it.
    paren_query_grouping: bool,
    /// Set when the query body currently being parsed is exactly a grouping
    /// parenthesized-query operand (SQLite): its enclosing query must not extend it with a
    /// set operator or an `ORDER BY`/`LIMIT` tail. Read by `parse_set_expr_bp` (skip the
    /// compound climb) and taken by `parse_query_after_with` (skip the clause tail), so
    /// any following token is left for the grouping helper's closing `)` — which then
    /// rejects `((SELECT 1) UNION …)` / `((SELECT 1) LIMIT 1)` as SQLite does.
    grouped_query_complete: bool,
    /// Set while parsing an expression restricted to PostgreSQL's `b_expr` grammar class
    /// (a column-constraint `DEFAULT` under
    /// [`ColumnDefinitionSyntax::column_default_requires_b_expr`](crate::ast::dialect::ColumnDefinitionSyntax)).
    /// It gates the Pratt loop's `a_expr`-only dispatch points (the boolean/predicate
    /// operators, quantified comparison, and `AT TIME ZONE`) and the prefix `NOT`. Set by
    /// [`parse_b_expr`](Self::parse_b_expr); cleared by [`parse_expr`](Self::parse_expr) —
    /// the `a_expr` reset boundary every `c_expr` sub-expression (parens, call args, CASE,
    /// array, subquery) re-enters through — while the operator-spine recursion keeps it, so
    /// the restriction covers the top-level default spine only, exactly as `b_expr` nests.
    pub(in crate::parser) restrict_b_expr: bool,
    /// Set only while parsing the *top level* of a select-list / `RETURNING` projection
    /// target, to suppress the value-position `.*` star selector
    /// ([`ExpressionSyntax::field_wildcard`](crate::ast::dialect::ExpressionSyntax::field_wildcard))
    /// at that one spot: a target `tbl.*` must stay a
    /// [`SelectItem::QualifiedWildcard`](crate::ast::SelectItem::QualifiedWildcard)
    /// (and admit the DuckDB wildcard modifiers), so the postfix loop leaves its `.*`
    /// unconsumed for the projection-item parser to claim. Cleared by
    /// [`parse_expr`](Self::parse_expr) — the `a_expr` reset boundary every nested
    /// `c_expr` (parens, call args, `ROW(...)`, a cast operand) re-enters through — so a
    /// `(func()).*` or a whole-row `tbl.*` written inside a value still folds normally.
    pub(in crate::parser) suppress_value_star: bool,
    /// Set only while parsing an Oracle/Snowflake `CONNECT BY` condition
    /// ([`Select::connect_by`](crate::ast::Select)), where it turns the `PRIOR` keyword
    /// into the [`UnaryOperator::Prior`](crate::ast::UnaryOperator) prefix operator. Off
    /// everywhere else, so `PRIOR` stays an ordinary column name — the `CONNECT BY`
    /// condition is the only place the operator is meaningful, so this scopes it there
    /// without any global expression-grammar change. Unlike
    /// [`restrict_b_expr`](Self::restrict_b_expr) it is *not* cleared at the `parse_expr`
    /// nesting boundary — `PRIOR` stays live through the condition's own parentheses and
    /// sub-expressions — but it *is* cleared when a nested query begins
    /// ([`parse_query`](Self::parse_query)), so a scalar subquery inside the condition
    /// reads its own `PRIOR` as a plain identifier.
    pub(in crate::parser) in_connect_by: bool,
    /// Off-by-default opt-in mirroring [`ParseOptions::capture_trivia`](crate::parser::ParseOptions):
    /// when set, each clause-introducing keyword (`WHERE`/`FROM`/`GROUP BY`/…) is
    /// recorded into [`clause_marks`](Self::clause_marks) as the parser eats it. The
    /// gate is a single cold branch read at each clause site (the field is already
    /// loaded), so the default parse — where the flag is `false` — records nothing
    /// and pays only that branch, the zero-overhead-when-off symmetry with trivia
    /// capture.
    capture_clause_marks: bool,
    /// Source-order buffer of recorded clause keywords, drained into the parse root's
    /// [`ClauseMarkIndex`] by [`take_clause_marks`](Self::take_clause_marks). Empty
    /// (and unallocated) unless [`capture_clause_marks`](Self::capture_clause_marks)
    /// is set. Each keyword is recorded with a placeholder owner at consume time and
    /// patched to the real [`NodeId`] when the owning node finalizes (see
    /// [`patch_clause_marks`](Self::patch_clause_marks)).
    clause_marks: Vec<ClauseMark>,
    /// Whether a `RETURN <expr>` statement is legal in the MySQL stored-program body
    /// currently being parsed. `true` in a stored-**function** body, `false` in a
    /// **procedure** body — where the server rejects `RETURN` (`ER_SP_BADRETURN`). Set by the
    /// routine DDL wrappers (`parse_routine_body_statement`) around the body
    /// parse and read by the body dispatcher's `RETURN` arm; `true` by default so the
    /// body-grammar seam (and its direct tests) admit `RETURN` unless a procedure wrapper
    /// narrows it. A single cold branch at the one `RETURN` site — zero cost off the routine
    /// path.
    pub(in crate::parser) body_return_allowed: bool,
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// Build a parser over an already-tokenized `source`.
    ///
    /// `tokens` must be the result of tokenizing `source`, so token spans index
    /// back into it. As with the byte cursor, spans are `u32`; `source` must fit
    /// in `u32` bytes, which [`tokenize`](crate::tokenizer::tokenize) guarantees
    /// before producing the tokens passed here.
    pub fn new(source: &'a str, tokens: &'a [Token], dialect: D) -> Self {
        debug_assert!(
            u32::try_from(source.len()).is_ok(),
            "Parser source length must fit in u32; tokenize() guards this",
        );
        Self::from_cursor(source, dialect, BufferedTokenCursor::from_tokens(tokens))
    }

    /// Build a parser over a source-backed lazy token cursor.
    ///
    /// This is the production parse path. Tokens are scanned only as grammar
    /// lookahead asks for them, then discarded between top-level statements.
    pub(crate) fn streaming(
        source: &'a str,
        dialect: D,
    ) -> Result<Self, crate::tokenizer::LexError> {
        let cursor = BufferedTokenCursor::streaming(source, dialect.features())?;
        Ok(Self::from_cursor(source, dialect, cursor))
    }

    /// Like [`streaming`](Self::streaming), but the cursor records skipped
    /// comments/whitespace as out-of-band trivia, drained with
    /// [`take_trivia`](Self::take_trivia) once the parse finishes.
    pub(crate) fn streaming_with_trivia(
        source: &'a str,
        dialect: D,
    ) -> Result<Self, crate::tokenizer::LexError> {
        let cursor = BufferedTokenCursor::streaming_with_trivia(source, dialect.features())?;
        Ok(Self::from_cursor(source, dialect, cursor))
    }

    fn from_cursor(source: &'a str, dialect: D, cursor: BufferedTokenCursor<'a>) -> Self {
        // The single choke-point every parse passes through (`new`, `streaming`,
        // `streaming_with_trivia` all funnel here), so it is where the dialect's
        // FeatureSet is validated against all three self-consistency registries
        // (ADR-0011): lexical conflicts, unsatisfied grammar-flag dependencies, and
        // grammar-position conflicts. A conflicted custom set otherwise ships a silent
        // mis-parse (a lexical or grammar conflict shadows one reading by fixed
        // precedence) or a silently inert flag (an unsatisfied dependency) with no
        // debug-time verdict from the registries built to give one; these catch it in the
        // consumer's own debug/test runs. `debug_assert!` compiles out in release — zero
        // cost — and for a const-preset dialect `features()` const-folds each predicate to
        // a known-clean constant (the per-dialect `const _: () = assert!(…)` ratchets
        // prove all three fold), so the whole check is dead code there. Builders wanting a
        // verdict as a value use `FeatureSet::try_with` (lexical) or read
        // `feature_dependencies`/`grammar_conflict` on the built set.
        debug_assert!(
            dialect.features().is_lexically_consistent(),
            "dialect feature set is lexically inconsistent: {:?} — two features claim one \
             tokenizer trigger, so a fixed lex precedence silently shadows one reading (see \
             the FeatureSet::lexical_conflict registry); enable only one claimant of the trigger.",
            dialect.features().lexical_conflict(),
        );
        debug_assert!(
            dialect.features().has_satisfied_feature_dependencies(),
            "dialect feature set has an unsatisfied grammar-flag dependency: {:?} — a refinement \
             flag is enabled without the base flag it rides on, leaving it inert (see the \
             FeatureSet::feature_dependencies registry); enable the named base flag or drop the \
             dependent one.",
            dialect.features().feature_dependencies(),
        );
        debug_assert!(
            dialect.features().has_no_grammar_conflict(),
            "dialect feature set has a grammar-position conflict: {:?} — two features read the \
             same parser-position head with no lookahead to tell them apart, so a fixed branch \
             order silently shadows one reading (see the FeatureSet::grammar_conflict registry); \
             enable only one of the contending features.",
            dialect.features().grammar_conflict(),
        );
        Self {
            cursor,
            source,
            interner: Interner::new(),
            errors: FailFastSink::new(),
            dialect,
            next_node_id: 1,
            peeked: None,
            recursion_depth: 0,
            angle_gt_pending: 0,
            recursion_limit: DEFAULT_RECURSION_LIMIT,
            parse_float_as_decimal: false,
            paren_query_grouping: false,
            grouped_query_complete: false,
            restrict_b_expr: false,
            suppress_value_star: false,
            in_connect_by: false,
            capture_clause_marks: false,
            clause_marks: Vec::new(),
            body_return_allowed: true,
        }
    }

    /// Set the recursive-descent depth limit, consuming and returning the parser.
    ///
    // Builder convention (uniform across the crate's `by value -> Self` builders —
    // `ParseOptions::with_*`, these `Parser::with_*`, and `Renderer::{new,with_config}`):
    // every consuming builder carries `#[must_use]`, because it returns a configured value
    // rather than mutating in place, so discarding the result (`parser.with_recursion_limit(5);`)
    // silently no-ops. This targets that build-and-discard hazard only; plain getters stay
    // unmarked (per the clippy `must_use_candidate` consensus — no blanket getter spray).
    /// The limit is the maximum number of nested recursive-descent entries (nested
    /// expressions, subqueries, parenthesized join factors, and nested statements)
    /// before parsing fails with a
    /// [`ParseErrorKind::RecursionLimitExceeded`](crate::error::ParseErrorKind::RecursionLimitExceeded)
    /// error rather than recursing further. See
    /// [`DEFAULT_RECURSION_LIMIT`] for how the default is chosen; lower it on small
    /// stacks, raise it for deeply nested machine-generated SQL on large stacks.
    #[must_use]
    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    /// Set the float-as-decimal classification request, consuming and returning the
    /// parser.
    ///
    /// When `enabled`, a fractional or scientific numeric literal is classified
    /// [`LiteralKind::Decimal`](crate::ast::LiteralKind::Decimal) rather than
    /// [`LiteralKind::Float`](crate::ast::LiteralKind::Float); every other classification
    /// (integer, radix integer, money) and the source spelling are unchanged. Off by
    /// default. Backs [`ParseOptions::parse_float_as_decimal`](crate::parser::ParseOptions::parse_float_as_decimal).
    #[must_use]
    pub fn with_parse_float_as_decimal(mut self, enabled: bool) -> Self {
        self.parse_float_as_decimal = enabled;
        self
    }

    /// Whether floating numeric literals are being classified as
    /// [`LiteralKind::Decimal`](crate::ast::LiteralKind::Decimal); read at each numeric
    /// literal classification site.
    pub(in crate::parser) fn parse_float_as_decimal(&self) -> bool {
        self.parse_float_as_decimal
    }

    /// Enable clause-mark capture, consuming and returning the parser.
    ///
    /// When `enabled`, each clause-introducing keyword is recorded into the
    /// [`ClauseMarkIndex`] the root carries, drained by
    /// [`take_clause_marks`](Self::take_clause_marks). Off by default; enabled on the
    /// same opt-in path as trivia capture (see the crate's `parse_with_trivia`).
    pub(crate) fn with_clause_mark_capture(mut self, enabled: bool) -> Self {
        self.capture_clause_marks = enabled;
        self
    }

    // --- Clause marks (formatter comment anchoring) ------------------------

    /// Whether clause-mark capture is on. The cold gate every clause site branches on
    /// before recording; a field read that folds to nothing on the default (off)
    /// path, so a non-capturing parse pays only this one branch per clause keyword.
    #[inline(always)]
    pub(in crate::parser) fn capturing_clause_marks(&self) -> bool {
        self.capture_clause_marks
    }

    /// The current length of the clause-mark buffer, saved by a clause-owning node at
    /// entry and passed back to [`patch_clause_marks`](Self::patch_clause_marks) at
    /// exit so the patch touches only the marks that node (and its clauses) recorded.
    #[inline(always)]
    pub(in crate::parser) fn clause_marks_checkpoint(&self) -> usize {
        self.clause_marks.len()
    }

    /// Drop every clause mark recorded since `checkpoint`, restoring the buffer for a
    /// speculative parse that rewound its cursor (the infix/postfix operand probe). Without
    /// this, a rewound `parse_prefix` that recorded a nested clause keyword would leave a
    /// dangling mark the formatter later anchors to a discarded parse.
    #[inline(always)]
    pub(in crate::parser) fn truncate_clause_marks(&mut self, checkpoint: usize) {
        self.clause_marks.truncate(checkpoint);
    }

    /// Record a clause keyword's `kind` and byte `offset` with a placeholder owner.
    ///
    /// Callers gate this on [`capturing_clause_marks`](Self::capturing_clause_marks),
    /// so it only runs on the opt-in path. Kept `#[inline(never)]` so the `ClauseMark`
    /// construction and any `Vec` growth stay in this frame rather than bloating the
    /// hot recursive grammar frames that call it (the stack-canary discipline).
    #[inline(never)]
    pub(in crate::parser) fn record_clause_mark(&mut self, kind: ClauseKw, offset: u32) {
        self.clause_marks.push(ClauseMark::pending(kind, offset));
    }

    /// Stamp `owner` onto every still-pending clause mark recorded since `checkpoint`.
    ///
    /// A clause-owning node calls this after minting its own [`Meta`] with the
    /// checkpoint it saved at entry: the marks in `[checkpoint, len)` that a nested
    /// owner has not already claimed (still pending) are this node's own clause
    /// keywords, so they take its id. Callers gate on
    /// [`capturing_clause_marks`](Self::capturing_clause_marks); `#[inline(never)]`
    /// keeps the patch loop off the hot recursive frames.
    #[inline(never)]
    pub(in crate::parser) fn patch_clause_marks(&mut self, checkpoint: usize, owner: NodeId) {
        for mark in &mut self.clause_marks[checkpoint..] {
            if mark.owner_is_pending() {
                mark.set_owner(owner);
            }
        }
    }

    /// Drain the recorded clause marks into a queryable index.
    ///
    /// Empty unless the parser was built with
    /// [`with_clause_mark_capture`](Self::with_clause_mark_capture). Taken by `&mut`
    /// before [`finish`](Self::finish) (which consumes the parser) so a
    /// clause-mark-capturing root can carry the resolver, the trivia, and this index.
    pub(crate) fn take_clause_marks(&mut self) -> ClauseMarkIndex {
        ClauseMarkIndex::new(std::mem::take(&mut self.clause_marks))
    }

    // --- Token stream ------------------------------------------------------

    /// The token at the cursor without consuming it, or `None` at end of input.
    ///
    /// Returns the cursor's narrow [`LexError`] (a lexical fault is the only way a
    /// peek fails); a grammar method's `?` widens it to [`ParseError`] via [`From`]
    /// only on the error path, keeping the success path's `Result` 16 bytes rather
    /// than 56 on this — the hottest — path. Memoized per cursor position (see the
    /// `peeked` field).
    pub fn peek(&mut self) -> Result<Option<Token>, LexError> {
        let pos = self.cursor.pos();
        if let Some((cached_pos, cached)) = self.peeked {
            if cached_pos == pos {
                return cached;
            }
        }
        let result = self.cursor.peek();
        self.peeked = Some((pos, result));
        result
    }

    /// The token `n` positions ahead, without consuming anything.
    ///
    /// `peek_nth(0)` is [`peek`](Self::peek) (and shares its cache). Multi-token
    /// lookahead is how the grammar disambiguates productions (e.g. `name(` vs
    /// `name`). Widening of its [`LexError`] is as for [`peek`](Self::peek).
    pub fn peek_nth(&mut self, n: usize) -> Result<Option<Token>, LexError> {
        if n == 0 {
            return self.peek();
        }
        self.cursor.peek_nth(n)
    }

    /// Consume and return the token at the cursor, or `None` at end of input.
    pub fn advance(&mut self) -> Result<Option<Token>, LexError> {
        // The cache is position-keyed and self-validating, so the move past this
        // token simply makes the next `peek` miss and recompute — no explicit
        // invalidation needed.
        self.cursor.advance()
    }

    /// True once every token has been consumed.
    pub fn is_eof(&mut self) -> Result<bool, LexError> {
        // Delegated rather than `self.peek()?.is_none()`: `is_eof` is consulted
        // once per statement (not in the per-token hot loop), so it needs no cache,
        // and delegating keeps the cursor's own `is_eof` the single end-of-input
        // definition.
        self.cursor.is_eof()
    }

    // --- Spans -------------------------------------------------------------

    /// The span of the token at the cursor, or an empty end-of-input span.
    pub fn current_span(&mut self) -> crate::error::ParseResult<Span> {
        Ok(match self.peek()? {
            Some(token) => token.span,
            None => self.eof_span(),
        })
    }

    /// The span of the most recently consumed token.
    ///
    /// The grammar marks the end of a construct with this after eating the
    /// construct's last token (`start.union(self.preceding_span())`). Before any
    /// token is consumed there is no predecessor, so it anchors at the source
    /// start.
    pub fn preceding_span(&self) -> Span {
        self.cursor
            .preceding()
            .map_or_else(|| Span::new(0, 0), |token| token.span)
    }

    /// An empty span at the end of the source.
    ///
    /// End-of-input diagnostics point here so they still carry a real byte
    /// offset (the byte just past the last token) rather than a synthetic span.
    pub fn eof_span(&self) -> Span {
        // The source-length invariant (see `new`) guarantees this fits `u32`.
        let end = self.source.len() as u32;
        Span::new(end, end)
    }

    // --- Source text + interning -------------------------------------------

    /// Recover the source text covered by `span` as a source-lifetime slice.
    ///
    /// Returns `&'a str` (tied to the input, not to `&self`) so the grammar can
    /// classify or intern token text while still mutating the parser.
    pub fn span_text(&self, span: Span) -> &'a str {
        // Copy the `&'a str` out of `&self` first so the returned slice borrows
        // the input for `'a`, independent of this `&self` borrow.
        let source: &'a str = self.source;
        &source[span.start() as usize..span.end() as usize]
    }

    /// Intern the text of an identifier-position `token`, preserving exact source
    /// spelling and reusing the keyword identity the lexer already settled.
    ///
    /// The lexer ran `lookup_keyword` once while scanning this word, so
    /// its kind already carries the answer. Re-deriving it through the full
    /// [`Interner::intern`](Interner::intern) would repeat that lookup on the same
    /// text; the settled kind instead selects the interning primitive directly:
    /// - a `Word` was proved to match no keyword, so it skips the lookup entirely;
    /// - a `Keyword(kw)` needs only `text == kw.as_str()` to choose between `kw`'s
    ///   fixed slot (canonical spelling) and a verbatim dynamic symbol (any other
    ///   case, e.g. `Asc`), which round-trips its source case.
    ///
    /// Either primitive returns the same [`Symbol`] the full
    /// `intern` would — the skip changes cost, not identity. A non-word/keyword token
    /// (never produced by the identifier grammar) falls back to the full
    /// keyword-checking `intern`, sound for any text.
    pub fn intern_identifier(&mut self, token: Token) -> Symbol {
        use crate::tokenizer::TokenKind;

        let text = self.span_text(token.span);
        match token.kind {
            TokenKind::Word => self.interner.intern_nonkeyword(text),
            TokenKind::Keyword(kw) => self.interner.intern_keyword_ident(kw, text),
            _ => self.interner.intern(text),
        }
    }

    /// Intern arbitrary identifier text with the full keyword check, preserving its
    /// exact characters.
    ///
    /// This is the path for text the lexer did **not** settle as a single
    /// `Word`/`Keyword` token whose kind [`intern_identifier`](Self::intern_identifier)
    /// could reuse: a quoted identifier's body (only known after its delimiters are
    /// stripped and a doubled close delimiter is collapsed — the unescape is deferred
    /// to materialization, which can require an owned copy), or a name carved
    /// out of a larger token (a `:name`/`@name` parameter, a `@@scope.name` variable,
    /// an `OPERATOR(...)` operator run). The keyword check is load-bearing here and
    /// must stay: a quoted `"select"` is lexed as `QuotedIdent` with no keyword
    /// classification, yet its text must still resolve to `Keyword::Select`'s fixed
    /// slot, or the same-text-same-symbol identity invariant breaks (a bare `select`
    /// keyword-token and a quoted `"select"` would diverge).
    pub fn intern_text(&mut self, text: &str) -> Symbol {
        self.interner.intern(text)
    }

    // --- Node identity + metadata ------------------------------------------

    /// Allocate the next per-parse [`NodeId`].
    pub fn next_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id)
            .expect("node-id counter starts at 1 and never reaches 0");
        // Overflow is unreachable in practice: the source is capped at `u32::MAX`
        // bytes and every node spans at least one byte, so a parse cannot create
        // `u32::MAX` nodes.
        self.next_node_id += 1;
        id
    }

    /// Build [`Meta`] for a node: its `span` plus a fresh [`NodeId`].
    pub fn make_meta(&mut self, span: Span) -> Meta {
        Meta::new(span, self.next_node_id())
    }

    // --- Errors ------------------------------------------------------------

    /// Hand an already-built error to the sink.
    ///
    /// The sink owns error *policy*; call sites only report. v1's
    /// [`FailFastSink`] keeps the first; a resilient sink is a future drop-in.
    pub fn report(&mut self, error: ParseError) {
        self.errors.report(error);
    }

    /// Build an error at `span`, report it, and return it.
    ///
    /// The dual report-and-return is deliberate: the *return* drives fail-fast
    /// unwinding through `?`, while the *report* routes the same error through
    /// the sink seam so a future recovering sink needs no call-site change.
    pub fn error_at(
        &mut self,
        span: Span,
        expected: impl Into<Expected>,
        found: impl Into<Found>,
    ) -> ParseError {
        let error = ParseError::new(span, expected, found);
        self.report(error.clone());
        error
    }

    /// Build, report, and return an error against the *current* token.
    ///
    /// The offending token's text becomes the `found`; at end of input the span
    /// collapses to [`eof_span`](Self::eof_span) and `found` is
    /// [`Found::EndOfInput`]. The caller supplies what was `expected`.
    pub fn unexpected(&mut self, expected: impl Into<Expected>) -> ParseError {
        let expected = expected.into();
        match self.peek() {
            Ok(Some(token)) => {
                let found = self.span_text(token.span).to_owned();
                self.error_at(token.span, expected, found)
            }
            Ok(None) => {
                let span = self.eof_span();
                self.error_at(span, expected, Found::EndOfInput)
            }
            Err(error) => {
                // A lexical fault at the cursor: widen it to the unified error,
                // report it through the sink, and surface it as the "unexpected"
                // outcome so the grammar's fail-fast path is uniform.
                let error = ParseError::from(error);
                self.report(error.clone());
                error
            }
        }
    }

    // --- Recursion guard ---------------------------------------------------

    /// Enter one level of recursive descent, or fail cleanly if that would exceed
    /// the [recursion limit](Self::with_recursion_limit) (DoS-safety).
    ///
    /// Returns an RAII [`RecursionGuard`] that has *already* bumped the depth;
    /// dropping it restores the depth, so every exit path — a normal return, a `?`
    /// short-circuit, or a backtracking rewind that discards a partial parse —
    /// unwinds the count correctly and a leak is impossible. `span` is the location
    /// blamed when the limit is hit (the token that would have opened one level too
    /// many). The whole success-path cost is a `usize` compare and increment, so
    /// the guard stays off the parser's critical-path budget.
    pub(super) fn enter_recursion(&mut self, span: Span) -> ParseResult<RecursionGuard<'_, 'a, D>> {
        if self.recursion_depth >= self.recursion_limit {
            return Err(self.recursion_limit_exceeded(span));
        }
        self.recursion_depth += 1;
        Ok(RecursionGuard { parser: self })
    }

    /// Build, report, and return a recursion-limit error at `span`.
    ///
    /// Mirrors [`error_at`](Self::error_at): the error is routed through the sink
    /// (so a future recovering sink observes it) *and* returned to drive fail-fast
    /// unwinding through `?`.
    fn recursion_limit_exceeded(&mut self, span: Span) -> ParseError {
        let error = ParseError::recursion_limit_exceeded(span);
        self.report(error.clone());
        error
    }

    // --- Speculation -------------------------------------------------------

    /// Snapshot the cursor for later backtracking.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.cursor.pos(),
        }
    }

    /// Restore a [`checkpoint`](Self::checkpoint), undoing token consumption.
    pub fn rewind(&mut self, checkpoint: Checkpoint) {
        self.cursor.seek(checkpoint.pos);
    }

    /// Release tokens consumed before the next top-level statement.
    pub(super) fn discard_consumed_tokens(&mut self) {
        self.cursor.discard_consumed();
    }

    /// Panic-mode recovery: discard the rest of a broken statement, up to and
    /// including the next `;` boundary, or to end of input.
    ///
    /// Backs the resilient [`parse_recovering`](super::parse_recovering) path; the
    /// default fail-fast parse never calls it. The statement separator is the single
    /// resynchronization point: after a statement fails,
    /// skipping to the next `;` lets parsing resume at a known statement start, so
    /// one run reports every statement's error — "all errors in the file" — instead
    /// of stopping at the first. This is statement-level only; clause/expression-level
    /// resync is deliberately deferred.
    ///
    /// Returns `Ok(true)` if it resynced at a `;` (a known statement start, so more
    /// statements may follow) or `Ok(false)` if it consumed to end of input (recovery
    /// is complete). A lexical fault while skipping cannot be stepped over, so it
    /// surfaces as `Err` (the caller stops recovering on it).
    ///
    /// The `Ok(false)` end-of-input signal is load-bearing — the caller must NOT
    /// re-enter the parse on it. An unterminated construct (string / quoted-ident /
    /// block-comment / dollar-quote) at end of input leaves the byte cursor *pinned*:
    /// its scan consumes to EOF and only then errors, so the failing `peek` reports the
    /// fault (and is memoized at that position) while a *subsequent* `advance` reports
    /// `None` (EOF). Resyncing then makes no token progress and `pos` does not move, so
    /// re-parsing would re-fail at the same position forever — `false` tells the caller
    /// to stop instead.
    pub(crate) fn recover_to_statement_boundary(&mut self) -> ParseResult<bool> {
        use crate::tokenizer::{Punctuation, TokenKind};

        while let Some(token) = self.advance()? {
            if matches!(token.kind, TokenKind::Punctuation(Punctuation::Semicolon)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // --- Dialect data ------------------------------------------------------

    /// The dialect's const [`FeatureSet`]; reads const-fold under `Parser<D>`.
    pub fn features(&self) -> &FeatureSet {
        self.dialect.features()
    }

    // --- Parenthesized-query grouping context (see the field docs) ---------

    /// Consume the one-shot [`paren_query_grouping`](Self::paren_query_grouping) flag.
    pub(super) fn take_paren_query_grouping(&mut self) -> bool {
        std::mem::take(&mut self.paren_query_grouping)
    }

    /// Arm/disarm the grouping context for the next query body's leading operand.
    pub(super) fn set_paren_query_grouping(&mut self, grouping: bool) {
        self.paren_query_grouping = grouping;
    }

    /// Mark the current query body as a complete grouping paren-operand (no compound /
    /// clause tail may follow), read by `parse_set_expr_bp` and `parse_query_after_with`.
    pub(super) fn mark_grouped_query_complete(&mut self) {
        self.grouped_query_complete = true;
    }

    /// Peek [`grouped_query_complete`](Self::grouped_query_complete) without clearing it.
    pub(super) fn grouped_query_complete(&self) -> bool {
        self.grouped_query_complete
    }

    /// Consume [`grouped_query_complete`](Self::grouped_query_complete).
    pub(super) fn take_grouped_query_complete(&mut self) -> bool {
        std::mem::take(&mut self.grouped_query_complete)
    }

    // --- Finishing ---------------------------------------------------------

    /// A resolver view over the still-live interner.
    ///
    /// Interning is append-only and never reassigns a [`Symbol`],
    /// so symbols in an already-parsed statement resolve correctly here even as later
    /// statements intern more text. This is what lets the streaming
    /// [`statements`](super::statements) iterator resolve each statement before
    /// parsing the next, without freezing the interner.
    pub(crate) fn live_resolver(&self) -> &Interner {
        &self.interner
    }

    /// Drain the out-of-band trivia captured during the parse.
    ///
    /// Empty unless the parser was built with
    /// [`streaming_with_trivia`](Self::streaming_with_trivia). Taken by `&mut`
    /// before [`finish`](Self::finish) (which consumes the parser) so a
    /// `parse_with_trivia` root can carry both the resolver and the trivia.
    pub(crate) fn take_trivia(&mut self) -> TriviaIndex {
        self.cursor.take_trivia()
    }

    /// Consume the parser and freeze its interner into the shippable resolver.
    ///
    /// The terminal step of a parse: once no more text will be interned, the
    /// mutable interner collapses to the compact, `Send + Sync` resolver carried
    /// on the [`Parsed`](super::Parsed) root.
    pub(crate) fn finish(self) -> crate::interner::FrozenResolver {
        self.interner.freeze()
    }
}

/// RAII recursion-depth guard (DoS-safety).
///
/// Constructed by [`enter_recursion`](Parser::enter_recursion), which has already
/// bumped the parser's depth; [`Drop`] restores it. The guard owns the parser
/// borrow for the duration of the guarded production and lends it back through
/// [`parser`](Self::parser), so the increment/decrement bracket the *entire*
/// recursive call — normal return, `?` unwind, and backtracking rewind alike —
/// without the grammar method having to remember to decrement on every path. This
/// is why a deeply nested but ultimately rejected parse cannot leak depth and
/// spuriously trip the limit on later, shallower input.
pub(super) struct RecursionGuard<'p, 'a, D: Dialect> {
    parser: &'p mut Parser<'a, D>,
}

impl<'a, D: Dialect> RecursionGuard<'_, 'a, D> {
    /// The guarded parser, to drive the recursive production one level deeper.
    pub(super) fn parser(&mut self) -> &mut Parser<'a, D> {
        self.parser
    }
}

impl<D: Dialect> Drop for RecursionGuard<'_, '_, D> {
    fn drop(&mut self) {
        // `enter_recursion` incremented before this guard existed, so the depth is
        // always positive here; restoring it can never underflow.
        self.parser.recursion_depth -= 1;
    }
}
