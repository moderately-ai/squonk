// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Window-function grammar: the `OVER` clause on a call and the SELECT-level
//! `WINDOW` clause.
//!
//! All window keywords (`OVER`, `PARTITION`, `RANGE`, `GROUPS`, `UNBOUNDED`,
//! `PRECEDING`, `FOLLOWING`, `CURRENT`, `ROW`, `EXCLUDE`, `TIES`, `OTHERS`, `NO`,
//! `WINDOW`) are non-reserved, so they stay usable as identifiers outside this
//! grammar; they are recognized here positionally. The entry points are
//! [`parse_over_clause`](Parser::parse_over_clause) (called by the function-call
//! grammar) and [`parse_window_clause`](Parser::parse_window_clause) (called by the
//! SELECT grammar); both share [`parse_window_definition`](Parser::parse_window_definition).

use crate::ast::{
    Keyword, NamedWindow, Span, Spanned, WindowDefinition, WindowFrame, WindowFrameBound,
    WindowFrameExclusion, WindowFrameUnits, WindowSpec,
};
use crate::error::ParseResult;
use crate::tokenizer::Punctuation;
use thin_vec::ThinVec;

use super::Dialect;
use super::clause_marks::ClauseKw;
use super::engine::Parser;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse an optional `OVER name` / `OVER ( … )` clause after a function call.
    ///
    /// `OVER` is non-reserved, so its absence simply means a non-window call. A
    /// following `(` is an inline window definition; anything else is a reference
    /// to a name defined in the query's `WINDOW` clause.
    pub(super) fn parse_over_clause(&mut self) -> ParseResult<Option<WindowSpec<D::Ext>>> {
        let start = self.current_span()?;
        if !self.eat_keyword(Keyword::Over)? {
            return Ok(None);
        }
        if self.peek_is_punct(Punctuation::LParen)? {
            let definition = self.parse_window_definition()?;
            let span = start.union(definition.span());
            let meta = self.make_meta(span);
            Ok(Some(WindowSpec::Inline {
                definition: Box::new(definition),
                meta,
            }))
        } else {
            let name = self.parse_ident()?;
            let span = start.union(name.span());
            let meta = self.make_meta(span);
            Ok(Some(WindowSpec::Named { name, meta }))
        }
    }

    /// Parse a `WINDOW name AS ( … ) [, …]` clause; empty when `WINDOW` is absent.
    pub(super) fn parse_window_clause(&mut self) -> ParseResult<ThinVec<NamedWindow<D::Ext>>> {
        if !self.eat_keyword(Keyword::Window)? {
            return Ok(ThinVec::new());
        }
        if self.capturing_clause_marks() {
            self.record_clause_mark(ClauseKw::Window, self.preceding_span().start());
        }
        let windows = self.parse_comma_separated(Self::parse_named_window)?;
        Ok(windows)
    }

    /// Parse one `name AS ( <definition> )` entry of a `WINDOW` clause.
    fn parse_named_window(&mut self) -> ParseResult<NamedWindow<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        self.expect_keyword(Keyword::As)?;
        let definition = self.parse_window_definition()?;
        let span = start.union(self.preceding_span());
        Ok(NamedWindow {
            name,
            definition,
            meta: self.make_meta(span),
        })
    }

    /// Parse a parenthesized window definition:
    /// `( [base_window] [PARTITION BY …] [ORDER BY …] [<frame>] )`.
    ///
    /// A leading identifier that is not the start of a `PARTITION`/`ORDER`/frame
    /// clause (and not the closing `)`) is a base-window name this definition
    /// extends, matching PostgreSQL's `OVER (w PARTITION BY …)` form.
    fn parse_window_definition(&mut self) -> ParseResult<WindowDefinition<D::Ext>> {
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the window definition")?;

        let existing = if self.peek_starts_window_body()? {
            None
        } else {
            Some(self.parse_ident()?)
        };

        let partition_by = if self.eat_keyword(Keyword::Partition)? {
            self.expect_keyword(Keyword::By)?;
            self.parse_comma_separated_exprs()?
        } else {
            ThinVec::new()
        };

        let order_by = self.parse_order_by()?;
        let frame = self.parse_window_frame()?;

        self.expect_punct(Punctuation::RParen, "`)` to close the window definition")?;
        let span = start.union(self.preceding_span());
        Ok(WindowDefinition {
            existing,
            partition_by,
            order_by,
            frame,
            meta: self.make_meta(span),
        })
    }

    /// True when the next token begins a window-definition body (`PARTITION`,
    /// `ORDER`, a frame unit) or closes the definition — i.e. is not a base-window
    /// name.
    fn peek_starts_window_body(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_punct(Punctuation::RParen)?
            || self.peek_is_keyword(Keyword::Partition)?
            || self.peek_is_keyword(Keyword::Order)?
            || self.peek_is_keyword(Keyword::Rows)?
            || self.peek_is_keyword(Keyword::Range)?
            || self.peek_is_keyword(Keyword::Groups)?)
    }

    /// Parse an optional frame clause: `{ ROWS | RANGE | GROUPS } <extent>
    /// [ <exclusion> ]`. `None` when no frame unit starts here.
    fn parse_window_frame(&mut self) -> ParseResult<Option<WindowFrame<D::Ext>>> {
        let start = self.current_span()?;
        let units = if self.eat_keyword(Keyword::Rows)? {
            WindowFrameUnits::Rows
        } else if self.eat_keyword(Keyword::Range)? {
            WindowFrameUnits::Range
        } else if self.eat_keyword(Keyword::Groups)? {
            WindowFrameUnits::Groups
        } else {
            return Ok(None);
        };

        let (frame_start, end) = if self.eat_keyword(Keyword::Between)? {
            let frame_start = self.parse_window_frame_bound()?;
            self.expect_keyword(Keyword::And)?;
            let end = self.parse_window_frame_bound()?;
            (frame_start, Some(end))
        } else {
            (self.parse_window_frame_bound()?, None)
        };

        let exclusion = self.parse_window_frame_exclusion()?;
        let span = start.union(self.preceding_span());
        self.reject_invalid_frame_bound_order(&frame_start, end.as_ref(), span)?;
        Ok(Some(WindowFrame {
            units,
            start: frame_start,
            end,
            exclusion,
            meta: self.make_meta(span),
        }))
    }

    /// Reject a window frame whose bounds are ordered impossibly, at *parse* time.
    ///
    /// The SQL-standard frame bounds have a fixed position order (earliest to latest):
    /// `UNBOUNDED PRECEDING` < `<expr> PRECEDING` < `CURRENT ROW` < `<expr> FOLLOWING` <
    /// `UNBOUNDED FOLLOWING`. Three constraints follow and are all decidable on the parse
    /// tree (no offset *value* is compared — two `PRECEDING`/`FOLLOWING` bounds share a
    /// rank, so `2 FOLLOWING AND 1 FOLLOWING` and `1 PRECEDING AND 2 PRECEDING` stay
    /// accepted, their real order settled at execution): the start may not be `UNBOUNDED
    /// FOLLOWING` (nothing follows the last row), the end may not be `UNBOUNDED PRECEDING`
    /// (nothing precedes the first), and the start's rank may not exceed the end's. This is
    /// a shape-level well-formedness rule every engine enforces at parse — DuckDB (`frame
    /// start cannot be UNBOUNDED FOLLOWING` / `frame end cannot be UNBOUNDED PRECEDING`),
    /// SQLite, and PostgreSQL (libpg_query) all parse-reject the same set and accept the
    /// same valid frames (probed) — so it is applied unconditionally, like the sibling
    /// `WITHIN GROUP` combination checks, not gated per dialect. A bare single bound (`end`
    /// is `None`) is only checked against the start-cannot-be-UNBOUNDED-FOLLOWING rule.
    fn reject_invalid_frame_bound_order(
        &mut self,
        start: &WindowFrameBound<D::Ext>,
        end: Option<&WindowFrameBound<D::Ext>>,
        span: Span,
    ) -> ParseResult<()> {
        /// Position of a bound in the earliest-to-latest frame order.
        fn rank<X: crate::ast::Extension>(bound: &WindowFrameBound<X>) -> u8 {
            match bound {
                WindowFrameBound::UnboundedPreceding { .. } => 0,
                WindowFrameBound::Preceding { .. } => 1,
                WindowFrameBound::CurrentRow { .. } => 2,
                WindowFrameBound::Following { .. } => 3,
                WindowFrameBound::UnboundedFollowing { .. } => 4,
            }
        }
        let found = || self.span_text(span).to_owned();
        if matches!(start, WindowFrameBound::UnboundedFollowing { .. }) {
            return Err(self.error_at(
                span,
                "a frame start bound before UNBOUNDED FOLLOWING: the frame start cannot be \
                 UNBOUNDED FOLLOWING",
                found(),
            ));
        }
        if let Some(end) = end {
            if matches!(end, WindowFrameBound::UnboundedPreceding { .. }) {
                return Err(self.error_at(
                    span,
                    "a frame end bound after UNBOUNDED PRECEDING: the frame end cannot be \
                     UNBOUNDED PRECEDING",
                    found(),
                ));
            }
            if rank(start) > rank(end) {
                return Err(self.error_at(
                    span,
                    "a frame end bound at or after the start bound: the frame start cannot \
                     come after the frame end",
                    found(),
                ));
            }
        }
        Ok(())
    }

    /// Parse one frame bound: `UNBOUNDED PRECEDING|FOLLOWING`, `CURRENT ROW`, or
    /// `<offset> PRECEDING|FOLLOWING`.
    ///
    /// The offset is a full expression; `PRECEDING`/`FOLLOWING` are non-operator
    /// keywords, so the Pratt climb stops before them and they delimit the bound.
    ///
    /// `UNBOUNDED` and `CURRENT` are non-reserved, so they also lead ordinary offset
    /// expressions (`unbounded(1)`, `current.x`, `unbounded + 1`). The sentinel
    /// productions are therefore committed only when the following token completes them
    /// — `UNBOUNDED PRECEDING|FOLLOWING` and `CURRENT ROW`; any other continuation folds
    /// the keyword into a value-offset `a_expr`. This mirrors PostgreSQL's LALR shift
    /// preference: `UNBOUNDED`/`CURRENT` reduce to a bare column ref exactly when the
    /// two-token sentinel cannot apply.
    fn parse_window_frame_bound(&mut self) -> ParseResult<WindowFrameBound<D::Ext>> {
        if self.peek_is_keyword(Keyword::Unbounded)?
            && (self.peek_nth_is_keyword(1, Keyword::Preceding)?
                || self.peek_nth_is_keyword(1, Keyword::Following)?)
        {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::Unbounded)?;
            if self.eat_keyword(Keyword::Preceding)? {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(WindowFrameBound::UnboundedPreceding { meta });
            }
            self.expect_keyword(Keyword::Following)?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(WindowFrameBound::UnboundedFollowing { meta });
        }
        if self.peek_is_keyword(Keyword::Current)? && self.peek_nth_is_keyword(1, Keyword::Row)? {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::Current)?;
            self.expect_keyword(Keyword::Row)?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(WindowFrameBound::CurrentRow { meta });
        }
        let offset = self.parse_expr()?;
        if self.eat_keyword(Keyword::Preceding)? {
            let span = offset.span().union(self.preceding_span());
            let meta = self.make_meta(span);
            Ok(WindowFrameBound::Preceding {
                offset: Box::new(offset),
                meta,
            })
        } else {
            self.expect_keyword(Keyword::Following)?;
            let span = offset.span().union(self.preceding_span());
            let meta = self.make_meta(span);
            Ok(WindowFrameBound::Following {
                offset: Box::new(offset),
                meta,
            })
        }
    }

    /// Parse an optional `EXCLUDE { CURRENT ROW | GROUP | TIES | NO OTHERS }`.
    fn parse_window_frame_exclusion(&mut self) -> ParseResult<Option<WindowFrameExclusion>> {
        if !self.eat_keyword(Keyword::Exclude)? {
            return Ok(None);
        }
        if self.eat_keyword(Keyword::Current)? {
            self.expect_keyword(Keyword::Row)?;
            return Ok(Some(WindowFrameExclusion::CurrentRow));
        }
        if self.eat_keyword(Keyword::Group)? {
            return Ok(Some(WindowFrameExclusion::Group));
        }
        if self.eat_keyword(Keyword::Ties)? {
            return Ok(Some(WindowFrameExclusion::Ties));
        }
        self.expect_keyword(Keyword::No)?;
        self.expect_keyword(Keyword::Others)?;
        Ok(Some(WindowFrameExclusion::NoOthers))
    }
}
