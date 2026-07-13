// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SELECT body grammar: projection, `FROM`, `WHERE`, `GROUP BY`, `HAVING`.
//!
//! [`parse_select`](Parser::parse_select) assembles one `SELECT` body in SQL
//! clause order. It is the operand the set-operation folder in [`super::query`]
//! repeats, and it recurses (through `parse_query`) for a derived-table subquery
//! in [`super::from`]. Every expression position — projection items, the `WHERE`
//! predicate, `GROUP BY` keys, and `HAVING` — defers to the Pratt core in
//! [`super::expr`], so operator precedence lives in exactly one place.

use crate::ast::{
    AliasSpelling, Expr, Extension, GroupByAllSpelling, GroupByItem, HierarchicalClause, Ident,
    IntoTarget, Keyword, LateralView, Literal, NamedWindow, ObjectName, OnlySyntax,
    RelationInheritance, RollupSpelling, SampleClause, SampleUnit, Select, SelectDistinct,
    SelectItem, SelectSpelling, SetQuantifier, Span, Spanned, TableFactor, TableWithJoins,
    WildcardOptions, WildcardRename, WildcardReplace,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::clause_marks::ClauseKw;
use super::engine::Parser;
use super::expr::number_literal_kind;

/// The clauses shared by the SELECT-first and FROM-first SELECT bodies, parsed after the
/// projection/`FROM` prefix (see [`Parser::parse_select_body_tail`]). Collected so both
/// the SELECT-first assembler and [`Parser::parse_select_from_first`] read this tail from
/// one grammar, differing only in the projection/`FROM` order that precedes it.
struct SelectBodyTail<X: Extension> {
    lateral_views: ThinVec<LateralView<X>>,
    selection: Option<Expr<X>>,
    connect_by: Option<Box<HierarchicalClause<X>>>,
    group_by: ThinVec<GroupByItem<X>>,
    group_by_quantifier: Option<SetQuantifier>,
    group_by_all: Option<GroupByAllSpelling>,
    having: Option<Expr<X>>,
    windows: ThinVec<NamedWindow<X>>,
    qualify: Option<Box<Expr<X>>>,
    sample: Option<Box<SampleClause>>,
}

/// The three outputs of parsing a `GROUP BY` body: the grouping items, PostgreSQL's
/// optional `DISTINCT`/`ALL` set-quantifier ([`Select::group_by_quantifier`]), and
/// DuckDB's `GROUP BY ALL` mode tag ([`Select::group_by_all`]). At most one of the
/// latter two is ever set (they are MECE — see [`Parser::parse_group_by_body`]).
type GroupByBody<X> = (
    ThinVec<GroupByItem<X>>,
    Option<SetQuantifier>,
    Option<GroupByAllSpelling>,
);

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse a `SELECT [DISTINCT] <projection> [FROM …] [WHERE …] [GROUP BY …]
    /// [HAVING …]` body.
    ///
    /// Each clause is optional and consumed in SQL order. The leading `SELECT`
    /// keyword is validated here (not asserted) because this is also reached
    /// speculatively as a set-operation operand and as a derived-table subquery,
    /// where a missing `SELECT` is a real parse error rather than a dispatch bug.
    pub(super) fn parse_select(&mut self) -> ParseResult<Select<D::Ext>> {
        // Saved before any clause keyword is eaten; the clauses recorded through the
        // body below carry a placeholder owner until this `Select`'s id is minted and
        // patched onto them at the end (their keywords are consumed before the node
        // exists). A nested subquery's clauses sit past its own later checkpoint, so
        // this patch never re-owns them.
        let clause_marks_start = self.clause_marks_checkpoint();
        if !self.peek_is_keyword(Keyword::Select)? {
            return Err(self.unexpected("`SELECT`"));
        }
        let keyword = self
            .advance()?
            .expect("peek_is_keyword confirmed a SELECT token is present");

        let distinct = self.parse_select_distinct()?;
        let straight_join = self.parse_straight_join_modifier()?;
        let projection = self.parse_projection(distinct.as_ref())?;
        let into = self.parse_select_into()?;
        let from = if self.peek_is_keyword(Keyword::From)? {
            if self.capturing_clause_marks() {
                let offset = self.current_span()?.start();
                self.record_clause_mark(ClauseKw::From, offset);
            }
            self.parse_from()?
        } else {
            ThinVec::new()
        };
        let tail = self.parse_select_body_tail(!from.is_empty())?;

        let span = keyword.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        if self.capturing_clause_marks() {
            self.patch_clause_marks(clause_marks_start, meta.node_id);
        }
        Ok(Select {
            distinct,
            straight_join,
            projection,
            into,
            from,
            lateral_views: tail.lateral_views,
            selection: tail.selection,
            connect_by: tail.connect_by,
            group_by: tail.group_by,
            group_by_quantifier: tail.group_by_quantifier,
            group_by_all: tail.group_by_all,
            having: tail.having,
            windows: tail.windows,
            qualify: tail.qualify,
            sample: tail.sample,
            spelling: SelectSpelling::Select,
            meta,
        })
    }

    /// Parse DuckDB's FROM-first SELECT body: `FROM <tables> [SELECT [DISTINCT]
    /// <projection>] <tail>`.
    ///
    /// The `FROM` clause leads; the projection, when written, sits immediately after it,
    /// or is omitted — the bare `FROM <tables>` form is an implicit `SELECT *`. DuckDB
    /// rejects a projection that trails any other clause (`FROM t WHERE x SELECT y` /
    /// `FROM t GROUP BY a SELECT a` are syntax errors; probed on 1.5.4), so the `SELECT`
    /// is read only in this one position and every following clause parses in its
    /// ordinary place through the shared
    /// [`parse_select_body_tail`](Self::parse_select_body_tail). The result is the
    /// canonical [`Select`] shape tagged [`SelectSpelling::FromFirst`] — one shape,
    /// surface recorded: DuckDB serializes `FROM t SELECT x` identically to
    /// `SELECT x FROM t`, and bare `FROM t` identically to `SELECT * FROM t`.
    /// `ORDER BY`/`LIMIT` bind on the enclosing [`Query`](crate::ast::Query), exactly as
    /// for a SELECT-first body.
    ///
    /// Gated at every caller by
    /// [`SelectSyntax::from_first`](crate::ast::dialect::SelectSyntax) (only reached at a
    /// leading `FROM` when the flag is on). The fields written only in the SELECT-first
    /// order — MySQL's [`straight_join`](Select::straight_join) and PostgreSQL's
    /// [`into`](Select::into) — stay at their defaults.
    pub(super) fn parse_select_from_first(&mut self) -> ParseResult<Select<D::Ext>> {
        let clause_marks_start = self.clause_marks_checkpoint();
        let start = self.current_span()?;
        if self.capturing_clause_marks() {
            self.record_clause_mark(ClauseKw::From, start.start());
        }
        let from = self.parse_from()?;
        let (distinct, projection) = if self.eat_keyword(Keyword::Select)? {
            let distinct = self.parse_select_distinct()?;
            let projection = self.parse_projection(distinct.as_ref())?;
            (distinct, projection)
        } else {
            // Bare `FROM <tables>` — the implicit `SELECT *`. The wildcard is synthetic
            // (no source token), so it borrows the FROM-clause span, mirroring how
            // `parse_table_command` anchors its synthesized wildcard.
            (
                None,
                thin_vec![SelectItem::Wildcard {
                    options: None,
                    alias: None,
                    alias_spelling: AliasSpelling::As,
                    meta: self.make_meta(start),
                }],
            )
        };
        // `parse_from` above guarantees a non-empty FROM list, so the lateral-view
        // position is reachable (`FROM t SELECT x LATERAL VIEW …` under Lenient, the
        // one preset that combines both gates).
        let tail = self.parse_select_body_tail(true)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        if self.capturing_clause_marks() {
            self.patch_clause_marks(clause_marks_start, meta.node_id);
        }
        Ok(Select {
            distinct,
            straight_join: false,
            projection,
            into: None,
            from,
            lateral_views: tail.lateral_views,
            selection: tail.selection,
            connect_by: tail.connect_by,
            group_by: tail.group_by,
            group_by_quantifier: tail.group_by_quantifier,
            group_by_all: tail.group_by_all,
            having: tail.having,
            windows: tail.windows,
            qualify: tail.qualify,
            sample: tail.sample,
            spelling: SelectSpelling::FromFirst,
            meta,
        })
    }

    /// Parse the clauses shared by the SELECT-first and FROM-first bodies, in SQL order
    /// after the projection/`FROM` prefix: `[LATERAL VIEW …]* [WHERE …]
    /// [[START WITH …] CONNECT BY [NOCYCLE] …] [GROUP BY … | GROUP BY ALL] [HAVING …]
    /// [WINDOW …] [QUALIFY …]`.
    ///
    /// `has_from` is whether the body wrote a (non-empty) `FROM` clause: Hive/Spark
    /// attach the lateral views inside the FROM clause (after its last relation), so a
    /// FROM-less body never reads them — its `LATERAL` is left unconsumed and rejects.
    fn parse_select_body_tail(&mut self, has_from: bool) -> ParseResult<SelectBodyTail<D::Ext>> {
        let lateral_views = if has_from {
            self.parse_lateral_views()?
        } else {
            ThinVec::new()
        };
        let selection = if self.eat_keyword(Keyword::Where)? {
            if self.capturing_clause_marks() {
                self.record_clause_mark(ClauseKw::Where, self.preceding_span().start());
            }
            Some(self.parse_expr()?)
        } else {
            None
        };
        let connect_by = self.parse_hierarchical_clause()?;
        let (group_by, group_by_quantifier, group_by_all) = if self.eat_keyword(Keyword::Group)? {
            if self.capturing_clause_marks() {
                self.record_clause_mark(ClauseKw::GroupBy, self.preceding_span().start());
            }
            self.expect_keyword(Keyword::By)?;
            self.parse_group_by_body()?
        } else {
            (ThinVec::new(), None, None)
        };
        let having = if self.eat_keyword(Keyword::Having)? {
            if self.capturing_clause_marks() {
                self.record_clause_mark(ClauseKw::Having, self.preceding_span().start());
            }
            Some(self.parse_expr()?)
        } else {
            None
        };
        let windows = self.parse_window_clause()?;
        let qualify = self.parse_qualify()?;
        let sample = self.parse_using_sample()?;
        Ok(SelectBodyTail {
            lateral_views,
            selection,
            connect_by,
            group_by,
            group_by_quantifier,
            group_by_all,
            having,
            windows,
            qualify,
            sample,
        })
    }

    /// Parse the Hive/Spark `LATERAL VIEW [OUTER] <generator>(args) <alias>
    /// [AS <col> [, …]]` clauses that follow the `FROM` clause
    /// ([`Select::lateral_views`]); empty when none lead. Repeatable — each iteration
    /// consumes one whole clause, so views chain (`… LATERAL VIEW explode(a) t1
    /// LATERAL VIEW explode(t1.x) t2 AS c`).
    ///
    /// Gated by [`SelectSyntax::lateral_view_clause`](crate::ast::dialect::SelectSyntax)
    /// — on for Hive/Databricks/Lenient; off the flag the `LATERAL` keyword is left
    /// unconsumed and surfaces as the usual trailing-token error. Claims a `LATERAL`
    /// only when `VIEW` follows, so the standard LATERAL derived-table/function factor
    /// ([`TableFactorSyntax::lateral`](crate::ast::dialect::TableExpressionSyntax)) —
    /// which reads its `LATERAL` at a table-factor *head*, a position this parser never
    /// occupies — can never race it for the shared lead, under any preset combination
    /// (Lenient enables both).
    fn parse_lateral_views(&mut self) -> ParseResult<ThinVec<LateralView<D::Ext>>> {
        let mut views = ThinVec::new();
        if !self.features().select_syntax.lateral_view_clause {
            return Ok(views);
        }
        while self.peek_is_keyword(Keyword::Lateral)?
            && self.peek_nth_is_keyword(1, Keyword::View)?
        {
            views.push(self.parse_lateral_view()?);
        }
        Ok(views)
    }

    /// Parse one `LATERAL VIEW` clause, cursor on its `LATERAL` (the caller confirmed
    /// the `VIEW` follow token).
    ///
    /// The generator must be a parenthesized function call (Hive's `function` / Spark's
    /// `qualifiedName '(' … ')'` productions) and the table alias is required (both
    /// grammars make it non-optional); the `AS` before the column list is optional —
    /// Spark's grammar spells `AS?` while Hive requires the keyword, a documented
    /// conservative-direction over-acceptance for the Hive preset (see [`LateralView`]).
    /// The alias and columns are `ColId`s, so a following clause keyword (`WHERE`,
    /// `GROUP`, another `LATERAL`, …) is never swallowed as a name.
    fn parse_lateral_view(&mut self) -> ParseResult<LateralView<D::Ext>> {
        let start = self.current_span()?;
        if self.capturing_clause_marks() {
            self.record_clause_mark(ClauseKw::LateralView, start.start());
        }
        self.expect_keyword(Keyword::Lateral)?;
        self.expect_keyword(Keyword::View)?;
        let outer = self.eat_keyword(Keyword::Outer)?;
        let name_start = self.current_span()?;
        let name = self.parse_object_name_with(self.features().reserved_function_name)?;
        if !self.peek_is_punct(Punctuation::LParen)? {
            return Err(self.unexpected("`(` opening the generator function's arguments"));
        }
        let function = self.parse_function_call(name, name_start)?;
        // A generator is `func_expr_windowless`-shaped: the windowed/aggregate wrapper
        // clauses are not a valid table-generating call in Hive or Spark (mirrors the
        // FROM table-function reject).
        if function.over.is_some() || function.filter.is_some() || function.within_group.is_some() {
            let span = function.meta.span;
            return Err(self.error_at(
                span,
                "a plain generator call: a LATERAL VIEW generator cannot carry an \
                 `OVER`, `FILTER`, or `WITHIN GROUP` clause",
                self.span_text(span).to_owned(),
            ));
        }
        let alias = self.parse_ident()?;
        let columns = if self.eat_keyword(Keyword::As)?
            || self
                .peek()?
                .is_some_and(|token| self.token_can_be_column_name(token))
        {
            self.parse_comma_separated(Self::parse_ident)?
        } else {
            ThinVec::new()
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LateralView {
            outer,
            function,
            alias,
            columns,
            meta,
        })
    }

    /// Parse the Oracle-style hierarchical query clause
    /// `[START WITH <cond>] CONNECT BY [NOCYCLE] <cond>` that follows the `WHERE` clause
    /// and precedes `GROUP BY` ([`Select::connect_by`]); `None` when absent.
    ///
    /// `START WITH` and `CONNECT BY` may be written in **either order** (Oracle admits
    /// both `START WITH … CONNECT BY …` and `CONNECT BY … START WITH …`); `CONNECT BY` is
    /// required and `START WITH` optional. The written order is recorded on
    /// [`HierarchicalClause::start_with_leads`] for an exact round-trip.
    ///
    /// Gated by [`SelectSyntax::connect_by_clause`](crate::ast::dialect::SelectSyntax) —
    /// on for Snowflake/Lenient; off the flag the `START`/`CONNECT` keyword is left
    /// unconsumed and surfaces as the usual trailing-token error.
    fn parse_hierarchical_clause(
        &mut self,
    ) -> ParseResult<Option<Box<HierarchicalClause<D::Ext>>>> {
        if !self.features().select_syntax.connect_by_clause {
            return Ok(None);
        }
        let start = self.current_span()?;
        if self.peek_is_keyword(Keyword::Start)? && self.peek_nth_is_keyword(1, Keyword::With)? {
            // START WITH leads; CONNECT BY is mandatory and follows.
            if self.capturing_clause_marks() {
                self.record_clause_mark(ClauseKw::StartWith, start.start());
            }
            let start_with = self.parse_start_with()?;
            if self.capturing_clause_marks() {
                let offset = self.current_span()?.start();
                self.record_clause_mark(ClauseKw::ConnectBy, offset);
            }
            let (nocycle, connect_by) = self.parse_connect_by()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(Box::new(HierarchicalClause {
                start_with: Some(start_with),
                nocycle,
                connect_by,
                start_with_leads: true,
                meta,
            })))
        } else if self.peek_is_keyword(Keyword::Connect)?
            && self.peek_nth_is_keyword(1, Keyword::By)?
        {
            // CONNECT BY leads; a trailing START WITH is optional.
            if self.capturing_clause_marks() {
                self.record_clause_mark(ClauseKw::ConnectBy, start.start());
            }
            let (nocycle, connect_by) = self.parse_connect_by()?;
            let start_with = if self.peek_is_keyword(Keyword::Start)?
                && self.peek_nth_is_keyword(1, Keyword::With)?
            {
                if self.capturing_clause_marks() {
                    let offset = self.current_span()?.start();
                    self.record_clause_mark(ClauseKw::StartWith, offset);
                }
                Some(self.parse_start_with()?)
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(Box::new(HierarchicalClause {
                start_with,
                nocycle,
                connect_by,
                start_with_leads: false,
                meta,
            })))
        } else {
            Ok(None)
        }
    }

    /// Parse a `START WITH <condition>` root-row seed. The condition is an ordinary
    /// predicate — `PRIOR` is meaningful only in the `CONNECT BY` walk, so it is *not*
    /// recognized here (a bare `prior` in `START WITH` stays an ordinary column name).
    fn parse_start_with(&mut self) -> ParseResult<Expr<D::Ext>> {
        self.expect_keyword(Keyword::Start)?;
        self.expect_keyword(Keyword::With)?;
        self.parse_expr()
    }

    /// Parse a `CONNECT BY [NOCYCLE] <condition>`, returning the `NOCYCLE` flag and the
    /// parent/child predicate.
    ///
    /// `NOCYCLE` is an Oracle-only word absent from the shared keyword inventory, so it is
    /// matched as a contextual keyword right after `CONNECT BY` (Snowflake's docs omit it —
    /// accepting it under the one atomic gate is a documented over-acceptance). The
    /// condition parses with [`Parser::in_connect_by`] armed, turning `PRIOR` into the
    /// [`UnaryOperator::Prior`](crate::ast::UnaryOperator) prefix operator; it is an
    /// ordinary expression otherwise, so it rides the guarded expression path and honours
    /// precedence. The flag is restored even on error.
    fn parse_connect_by(&mut self) -> ParseResult<(bool, Expr<D::Ext>)> {
        self.expect_keyword(Keyword::Connect)?;
        self.expect_keyword(Keyword::By)?;
        let nocycle = self.eat_contextual_keyword("NOCYCLE")?;
        let saved = self.in_connect_by;
        self.in_connect_by = true;
        let condition = self.parse_expr();
        self.in_connect_by = saved;
        Ok((nocycle, condition?))
    }

    /// Parse DuckDB's `USING SAMPLE <entry>` query-level sample clause; `None` when
    /// absent. Positioned after `QUALIFY` and before the enclosing query's `ORDER BY`
    /// (the reverse order is a DuckDB syntax error). Gated by
    /// [`QueryTailSyntax::using_sample`](crate::ast::dialect::SelectSyntax); off the flag the
    /// `USING` keyword is left unconsumed and surfaces as the usual trailing-token error.
    ///
    /// The entry is DuckDB's `tablesample_entry`, whose two equivalent surface shapes
    /// (count-first and method-first) fold to the one canonical [`SampleClause`]: a
    /// count-first `<size> [ROWS|PERCENT|%] [ '(' method [',' seed] ')' ]`, a method-first
    /// `method '(' <size> [unit] ')' [REPEATABLE '(' seed ')']`, and the parenthesized
    /// count `'(' <size> [unit] ')'`.
    fn parse_using_sample(&mut self) -> ParseResult<Option<Box<SampleClause>>> {
        if !(self.features().query_tail_syntax.using_sample
            && self.peek_is_keyword(Keyword::Using)?
            && self.peek_nth_is_contextual_keyword(1, "SAMPLE")?)
        {
            return Ok(None);
        }
        let start = self.current_span()?;
        self.advance()?; // USING
        self.expect_contextual_keyword("SAMPLE")?;
        let is_number = self
            .peek()?
            .is_some_and(|token| token.kind == TokenKind::Number);
        let (method, size, unit, seed) = if self.peek_is_punct(Punctuation::LParen)? {
            // `USING SAMPLE ( <size> [unit] )` — the parenthesized bare count.
            self.advance()?; // (
            let (size, unit) = self.parse_sample_size()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the sample size")?;
            (None, size, unit, None)
        } else if is_number {
            // Count-first: `<size> [unit] [ '(' method [',' seed] ')' ]`.
            let (size, unit) = self.parse_sample_size()?;
            let (method, seed) = if self.eat_punct(Punctuation::LParen)? {
                let method = self.parse_object_name()?;
                let seed = if self.eat_punct(Punctuation::Comma)? {
                    Some(self.parse_sample_literal()?)
                } else {
                    None
                };
                self.expect_punct(Punctuation::RParen, "`)` to close the sample method")?;
                (Some(method), seed)
            } else {
                (None, None)
            };
            (method, size, unit, seed)
        } else {
            // Method-first: `method '(' <size> [unit] ')' [REPEATABLE '(' seed ')']`.
            let method = self.parse_object_name()?;
            self.expect_punct(Punctuation::LParen, "`(` after the sample method")?;
            let (size, unit) = self.parse_sample_size()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the sample size")?;
            let seed = self.parse_sample_repeatable()?;
            (Some(method), size, unit, seed)
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(SampleClause {
            method,
            size,
            unit,
            seed,
            meta,
        })))
    }

    /// Parse a sample size: a numeric literal followed by an optional unit —
    /// `ROWS`, the `PERCENT` keyword, or the `%` sign; a bare number is a row count.
    fn parse_sample_size(&mut self) -> ParseResult<(Literal, SampleUnit)> {
        let size = self.parse_sample_literal()?;
        let unit = if self.eat_keyword(Keyword::Rows)? {
            SampleUnit::Rows
        } else if self.eat_keyword(Keyword::Percent)? {
            SampleUnit::Percent
        } else if self.eat_op(Operator::Percent)? {
            SampleUnit::PercentSign
        } else {
            SampleUnit::Count
        };
        Ok((size, unit))
    }

    /// Parse a bare numeric literal (a sample size or seed); DuckDB rejects a negative
    /// or general-expression value here, so only a `Number` token is admitted.
    fn parse_sample_literal(&mut self) -> ParseResult<Literal> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a numeric sample size"));
        };
        if token.kind != TokenKind::Number {
            return Err(self.unexpected("a numeric sample size"));
        }
        self.advance()?;
        let kind = number_literal_kind(self.span_text(token.span), self.float_as_decimal_enabled());
        Ok(Literal {
            kind,
            meta: self.make_meta(token.span),
        })
    }

    /// Parse the optional `REPEATABLE ( <seed> )` random-seed tail of a method-first
    /// sample entry; `None` when absent.
    fn parse_sample_repeatable(&mut self) -> ParseResult<Option<Literal>> {
        if !self.eat_contextual_keyword("REPEATABLE")? {
            return Ok(None);
        }
        self.expect_punct(Punctuation::LParen, "`(` after `REPEATABLE`")?;
        let seed = self.parse_sample_literal()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `REPEATABLE`")?;
        Ok(Some(seed))
    }

    /// Parse DuckDB's `QUALIFY <predicate>` post-window filter; `None` when absent.
    ///
    /// Positioned after the `WINDOW` clause per DuckDB's grammar order (`… HAVING …
    /// WINDOW … QUALIFY …`; the reverse is a DuckDB syntax error). Gated by
    /// [`SelectSyntax::qualify`](crate::ast::dialect::SelectSyntax): a dialect without
    /// it leaves `QUALIFY` unconsumed, so the trailing tokens surface as a clean parse
    /// error (the same reject mechanism the other SELECT gates use). Reaching this
    /// point at all under DuckDB relies on its reservation of `QUALIFY` (the
    /// `DUCKDB_RESERVED_*` sets): an unreserved word would already have been swallowed
    /// as a projection or FROM-relation bare alias. A predicate with no window
    /// function is accepted — DuckDB rejects that at bind time, past the parse-level
    /// contract.
    fn parse_qualify(&mut self) -> ParseResult<Option<Box<Expr<D::Ext>>>> {
        if !self.features().select_syntax.qualify || !self.eat_keyword(Keyword::Qualify)? {
            return Ok(None);
        }
        if self.capturing_clause_marks() {
            self.record_clause_mark(ClauseKw::Qualify, self.preceding_span().start());
        }
        Ok(Some(Box::new(self.parse_expr()?)))
    }

    /// Parse PostgreSQL's `INTO [TEMP | TEMPORARY] <table>` create-table target,
    /// written between the projection and `FROM`; `None` when absent.
    ///
    /// Gated by [`SelectSyntax::select_into`](crate::ast::dialect::SelectSyntax): a
    /// dialect without it leaves `INTO` unconsumed, so the trailing tokens surface as
    /// a clean parse error (the same reject mechanism the other SELECT gates use).
    /// This is the *materialize-into-a-new-table* form (equivalent to `CREATE TABLE
    /// … AS`); the SQL-standard `SELECT … INTO <variable>` (PSM variable assignment)
    /// is a different construct and is deliberately not parsed here.
    fn parse_select_into(&mut self) -> ParseResult<Option<Box<IntoTarget>>> {
        if !self.features().select_syntax.select_into {
            return Ok(None);
        }
        if !self.eat_keyword(Keyword::Into)? {
            return Ok(None);
        }
        let start = self.preceding_span();
        let temporary = self.parse_temporary_table_kind()?;
        // PostgreSQL's `OptTempTableName` admits an optional `TABLE` noise keyword after
        // the temporary marker (`INTO TABLE t`, `INTO TEMP TABLE t`); it carries no
        // meaning beyond the target already being a table, so — like a join side's
        // trailing `OUTER` — it is consumed and dropped rather than kept as a spelling.
        let _ = self.eat_keyword(Keyword::Table)?;
        let name = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(Box::new(IntoTarget {
            temporary,
            name,
            meta,
        })))
    }

    /// Parse the body after a consumed `GROUP BY`: an optional set-quantifier, then
    /// the grouping-item list (or DuckDB's `ALL` mode). Returns the grouping items, the
    /// PostgreSQL `DISTINCT`/`ALL` quantifier ([`Select::group_by_quantifier`]), and the
    /// DuckDB `GROUP BY ALL` mode tag ([`Select::group_by_all`], with its `ALL`/`*`
    /// spelling).
    ///
    /// Two constructs share the `ALL` keyword but are mutually exclusive by shape and
    /// kept MECE here:
    /// - PostgreSQL's quantifier ([`GroupingSyntax::group_by_set_quantifier`](crate::ast::dialect::SelectSyntax)):
    ///   `GROUP BY {DISTINCT | ALL} <items>`, a prefix on a *non-empty* item list
    ///   (PostgreSQL rejects a bare `GROUP BY ALL`/`GROUP BY DISTINCT`; probed on
    ///   pg_query PG-17).
    /// - DuckDB's mode ([`GroupingSyntax::group_by_all`](crate::ast::dialect::SelectSyntax)):
    ///   `GROUP BY ALL`, where `ALL` *is* the whole clause (empty item list; DuckDB
    ///   syntax-errors on anything trailing it, probed on 1.5.4). Its `GROUP BY *`
    ///   shorthand ([`GroupByAllSpelling::Star`]) opens the same mode from a bare
    ///   wildcard, disambiguated by the same end-of-clause lookahead.
    ///
    /// `ALL` therefore disambiguates by lookahead: it opens the DuckDB mode only when the
    /// mode gate is on and the grouping clause ends right after it (`at_group_by_end`);
    /// otherwise, under the quantifier gate, it is the quantifier prefixing the item list.
    /// Under Lenient (both gates on) this makes bare `GROUP BY ALL` the mode and
    /// `GROUP BY ALL <items>` the quantifier — the honest superset of the two dialects.
    fn parse_group_by_body(&mut self) -> ParseResult<GroupByBody<D::Ext>> {
        let features = self.features().grouping_syntax;
        // `DISTINCT` is only ever the PostgreSQL quantifier (it has no DuckDB-mode
        // reading), so a following item list is mandatory — a bare `GROUP BY DISTINCT`
        // falls into `parse_group_by_list` and rejects, matching PostgreSQL.
        if features.group_by_set_quantifier && self.eat_keyword(Keyword::Distinct)? {
            let items = self.parse_group_by_list()?;
            return Ok((items, Some(SetQuantifier::Distinct), None));
        }
        // DuckDB's `GROUP BY *` shorthand for the `ALL` mode: a bare wildcard standing
        // for the whole clause. Bare-only (DuckDB binder-rejects `GROUP BY *, x`, probed
        // on 1.5.4), so it opens the mode only when the clause ends right after the star;
        // otherwise the star falls through to the item grammar and errors, as every
        // dialect rejects a bare `*` grouping key.
        if features.group_by_all
            && self.peek_is_op(Operator::Star)?
            && self.at_group_by_end_after(1)?
        {
            self.advance()?; // *
            return Ok((ThinVec::new(), None, Some(GroupByAllSpelling::Star)));
        }
        if self.peek_is_keyword(Keyword::All)? {
            // DuckDB mode: `ALL` stands alone (the clause ends immediately after it).
            // The lookahead over the eaten `ALL` keeps the two constructs MECE without a
            // speculative parse.
            let all_is_mode = features.group_by_all && self.at_group_by_end_after(1)?;
            if all_is_mode {
                self.advance()?; // ALL
                return Ok((ThinVec::new(), None, Some(GroupByAllSpelling::Keyword)));
            }
            if features.group_by_set_quantifier {
                self.advance()?; // ALL
                let items = self.parse_group_by_list()?;
                return Ok((items, Some(SetQuantifier::All), None));
            }
            // Neither gate claims `ALL` here: fall through to the item grammar, where
            // every shipped dialect reserves it — a clean parse error (matching DuckDB's
            // reject of `GROUP BY ALL <items>` and PostgreSQL's of a bare `GROUP BY ALL`).
        }
        let items = self.parse_group_by_list()?;
        Ok((items, None, None))
    }

    /// Parse the comma-separated grouping-item list, folding MySQL's trailing
    /// `WITH ROLLUP` modifier. DuckDB tolerates a single trailing comma before the
    /// clause that follows the keys (`GROUP BY a, b,`); the follower is exactly
    /// [`at_group_by_end`](Self::at_group_by_end).
    fn parse_group_by_list(&mut self) -> ParseResult<ThinVec<GroupByItem<D::Ext>>> {
        let items =
            self.parse_comma_separated_trailing(Self::parse_group_by_item, Self::at_group_by_end)?;
        self.wrap_with_rollup(items)
    }

    /// Parse one `GROUP BY` item: an ordinary grouping expression or, when
    /// [`GroupingSyntax::grouping_sets`](crate::ast::dialect::SelectSyntax) is on, one
    /// of the SQL:1999 grouping-set constructs (`ROLLUP (…)`, `CUBE (…)`,
    /// `GROUPING SETS (…)`, or the empty grouping set `()`).
    ///
    /// PostgreSQL lowers those keyword forms in GROUP BY item position for any case
    /// spelling, so they are their own grammar node, not [`FunctionCall`](crate::ast::FunctionCall)s. With the
    /// gate off (MySQL) every item falls through to [`parse_expr`](Self::parse_expr),
    /// so an unquoted `rollup (a, b)` parses as an ordinary function call — MySQL's
    /// stored-function reading.
    fn parse_group_by_item(&mut self) -> ParseResult<GroupByItem<D::Ext>> {
        if self.features().grouping_syntax.grouping_sets {
            if let Some(item) = self.parse_grouping_set_construct()? {
                return Ok(item);
            }
        }
        let expr = self.parse_expr()?;
        let meta = self.make_meta(expr.span());
        Ok(GroupByItem::Expr { expr, meta })
    }

    /// Parse a grouping-set `GROUP BY` construct when the current tokens begin one;
    /// `None` when they do not (the caller then parses an ordinary expression).
    ///
    /// Only the *unquoted* keyword forms are constructs: a quoted `"rollup"(…)`
    /// tokenizes as a `QuotedIdent`, never `Keyword::Rollup`, so it stays a function
    /// call — matching PostgreSQL, which lowers only the bare keyword. Each keyword
    /// form additionally requires its opening `(`, so a bare `rollup` / `grouping`
    /// remains an ordinary column or function reference (PG's unreserved words).
    fn parse_grouping_set_construct(&mut self) -> ParseResult<Option<GroupByItem<D::Ext>>> {
        // `ROLLUP (expr_list)` / `CUBE (expr_list)`: a plain expression list.
        if self.peek_is_keyword(Keyword::Rollup)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            let start = self.advance_span()?;
            let exprs = self.parse_paren_expr_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(GroupByItem::Rollup {
                exprs,
                spelling: RollupSpelling::Function,
                meta,
            }));
        }
        if self.peek_is_keyword(Keyword::Cube)? && self.peek_nth_is_punct(1, Punctuation::LParen)? {
            let start = self.advance_span()?;
            let exprs = self.parse_paren_expr_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(GroupByItem::Cube { exprs, meta }));
        }
        // `GROUPING SETS (group_by_list)`: the members are themselves GROUP BY items,
        // so `ROLLUP`/`CUBE`/nested `GROUPING SETS`/`()` may appear inside.
        if self.peek_is_keyword(Keyword::Grouping)?
            && self.peek_nth_is_keyword(1, Keyword::Sets)?
            && self.peek_nth_is_punct(2, Punctuation::LParen)?
        {
            let start = self.advance_span()?; // GROUPING
            self.advance()?; // SETS
            self.expect_punct(Punctuation::LParen, "`(` after `GROUPING SETS`")?;
            let sets = self.parse_comma_separated_trailing(Self::parse_group_by_item, |p| {
                p.trailing_comma_at(Punctuation::RParen)
            })?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `GROUPING SETS` list")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(GroupByItem::GroupingSets { sets, meta }));
        }
        // The empty grouping set `()` — the grand total. Distinguished from a
        // parenthesized expression by the immediately following `)`.
        if self.peek_is_punct(Punctuation::LParen)?
            && self.peek_nth_is_punct(1, Punctuation::RParen)?
        {
            let start = self.advance_span()?; // (
            self.advance()?; // )
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(GroupByItem::Empty { meta }));
        }
        Ok(None)
    }

    /// Fold MySQL's trailing `GROUP BY <keys> WITH ROLLUP` modifier into the one
    /// canonical [`GroupByItem::Rollup`] shape, tagged [`RollupSpelling::WithRollup`]
    /// so rendering round-trips the surface. Returns `items` unchanged when
    /// the modifier is absent.
    ///
    /// Gated by [`GroupingSyntax::with_rollup`](crate::ast::dialect::SelectSyntax); when
    /// off (PostgreSQL/ANSI, which spell the super-aggregate `ROLLUP (…)`) the `WITH`
    /// is left unconsumed and surfaces as a trailing-input parse error. MySQL's
    /// `WITH ROLLUP` wraps *plain* grouping keys only, so each item must be a
    /// [`GroupByItem::Expr`]; a grouping-set item (reachable only under Lenient, which
    /// enables `grouping_sets` and this gate together) is a clean error.
    fn wrap_with_rollup(
        &mut self,
        items: ThinVec<GroupByItem<D::Ext>>,
    ) -> ParseResult<ThinVec<GroupByItem<D::Ext>>> {
        if !self.features().grouping_syntax.with_rollup
            || !self.peek_is_keyword(Keyword::With)?
            || !self.peek_nth_is_keyword(1, Keyword::Rollup)?
        {
            return Ok(items);
        }
        // The wrapping node spans the whole key list plus the trailing modifier: from
        // the first key to the consumed `ROLLUP`. `parse_comma_separated` yields at
        // least one item, so `first()` is present.
        let start = items
            .first()
            .map_or_else(|| self.preceding_span(), Spanned::span);
        let mut exprs = ThinVec::with_capacity(items.len());
        for item in items {
            match item {
                GroupByItem::Expr { expr, .. } => exprs.push(expr),
                other => {
                    return Err(self.error_at(
                        other.span(),
                        "a plain grouping expression before `WITH ROLLUP`",
                        "a grouping-set construct",
                    ));
                }
            }
        }
        self.advance()?; // WITH
        self.advance()?; // ROLLUP
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(thin_vec![GroupByItem::Rollup {
            exprs,
            spelling: RollupSpelling::WithRollup,
            meta,
        }])
    }

    /// Consume the current token and return its span. Used where a keyword or
    /// punctuation has already been confirmed by a peek, so its presence is an
    /// invariant.
    pub(super) fn advance_span(&mut self) -> ParseResult<Span> {
        let token = self
            .advance()?
            .expect("a preceding peek confirmed a token is present");
        Ok(token.span)
    }

    /// Parse a parenthesized comma-separated expression list `( expr, … )`, the
    /// `ROLLUP`/`CUBE` argument shape. The opening `(` is confirmed by the caller's
    /// peek, so it is expected here rather than re-peeked. DuckDB tolerates a single
    /// trailing comma before the closing `)` (`ROLLUP(a, b,)`; engine-probed on 1.5.4),
    /// so this is the trailing-tolerant analogue of
    /// [`parse_comma_separated_exprs`](Self::parse_comma_separated_exprs) — not that
    /// shared combinator, whose remaining callers (`PARTITION BY`, `PIVOT`, …) reject the
    /// trailing comma.
    fn parse_paren_expr_list(&mut self) -> ParseResult<ThinVec<Expr<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(`")?;
        let exprs = self.parse_comma_separated_trailing(Self::parse_expr, |p| {
            p.trailing_comma_at(Punctuation::RParen)
        })?;
        self.expect_punct(Punctuation::RParen, "`)` to close the grouping list")?;
        Ok(exprs)
    }

    /// Parse the optional `ALL` / `DISTINCT` / `DISTINCT ON (...)` set quantifier
    /// that follows the `SELECT` keyword; `None` when none is written.
    ///
    /// `ALL` and `DISTINCT` are the standard set quantifiers ([`SetQuantifier`]);
    /// `DISTINCT ON (<expr>, ...)` is the PostgreSQL extension, gated by
    /// [`SelectSyntax::distinct_on`](crate::ast::dialect::SelectSyntax) so a dialect
    /// without it leaves `ON` unconsumed and the projection parse rejects it.
    fn parse_select_distinct(&mut self) -> ParseResult<Option<SelectDistinct<D::Ext>>> {
        if self.eat_keyword(Keyword::All)? {
            let span = self.preceding_span();
            let meta = self.make_meta(span);
            return Ok(Some(SelectDistinct::Quantifier {
                quantifier: SetQuantifier::All,
                meta,
            }));
        }
        if self.eat_keyword(Keyword::Distinct)? {
            let start = self.preceding_span();
            if self.features().select_syntax.distinct_on && self.eat_keyword(Keyword::On)? {
                self.expect_punct(Punctuation::LParen, "`(` after `DISTINCT ON`")?;
                // DuckDB tolerates a single trailing comma in the key list
                // (`DISTINCT ON (a,)`; engine-probed on 1.5.4), so this list site opts into
                // the trailing-tolerant combinator rather than the shared
                // `parse_comma_separated_exprs` — whose other callers (`PARTITION BY`,
                // `PIVOT`, …) must keep rejecting the trailing comma.
                let exprs = self.parse_comma_separated_trailing(Self::parse_expr, |p| {
                    p.trailing_comma_at(Punctuation::RParen)
                })?;
                self.expect_punct(Punctuation::RParen, "`)` to close the `DISTINCT ON` list")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(SelectDistinct::On { exprs, meta }));
            }
            let meta = self.make_meta(start);
            return Ok(Some(SelectDistinct::Quantifier {
                quantifier: SetQuantifier::Distinct,
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse the optional MySQL `STRAIGHT_JOIN` SELECT modifier — the query-wide form
    /// of the join-order hint, written after the `DISTINCT`/`ALL` quantifier and
    /// before the projection. `true` when present.
    ///
    /// Gated by [`JoinSyntax::straight_join`](crate::ast::dialect::TableExpressionSyntax),
    /// the one flag that also admits the `STRAIGHT_JOIN` join operator. A dialect
    /// without it leaves the word unconsumed, so `STRAIGHT_JOIN` falls through to the
    /// projection grammar as an ordinary (non-reserved) identifier — the same reject
    /// mechanism the other SELECT gates use.
    fn parse_straight_join_modifier(&mut self) -> ParseResult<bool> {
        Ok(self.features().join_syntax.straight_join && self.eat_keyword(Keyword::StraightJoin)?)
    }

    /// Parse the SELECT projection list.
    ///
    /// Ordinarily a non-empty comma-separated list (no trailing comma). Under
    /// [`SelectSyntax::empty_target_list`](crate::ast::dialect::SelectSyntax)
    /// (PostgreSQL/Lenient) the list may be empty — libpg_query's raw grammar makes the
    /// projection optional before any clause (`SELECT`, `SELECT FROM t`, `SELECT;`) —
    /// so the first item is required only when a projection actually begins.
    ///
    /// The empty form is admitted only for a plain or explicit-`ALL` SELECT: PostgreSQL
    /// splits `SELECT opt_all_clause opt_target_list` (empty allowed) from
    /// `SELECT distinct_clause target_list` (required), so `SELECT DISTINCT` /
    /// `SELECT DISTINCT ON (…)` with no items is a syntax error there and here.
    fn parse_projection(
        &mut self,
        distinct: Option<&SelectDistinct<D::Ext>>,
    ) -> ParseResult<ThinVec<SelectItem<D::Ext>>> {
        let empty_allowed = self.features().select_syntax.empty_target_list
            && matches!(
                distinct,
                None | Some(SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::All,
                    ..
                })
            );
        if empty_allowed && self.at_empty_target_list()? {
            return Ok(ThinVec::new());
        }
        // DuckDB tolerates a trailing comma before the clause that follows the projection
        // (`SELECT a, b, FROM t`); the follower is exactly `at_empty_target_list`.
        let items = self
            .parse_comma_separated_trailing(Self::parse_select_item, Self::at_empty_target_list)?;
        Ok(items)
    }

    /// True when the projection is empty: the next token is a clause keyword that
    /// follows the target list, a set operator, or a statement terminator (`;`, `)`,
    /// end of input) — never a select item.
    ///
    /// A `target_el` begins with `*` or an expression, and every token here is instead
    /// a *reserved* clause keyword or punctuation, so it can never begin one — the
    /// disjointness that lets an empty list be recognized by its follower rather than
    /// by enumerating the (dialect-dependent, open) set of expression starts. The
    /// followers are exactly the clauses `parse_select` and `parse_query_after_with`
    /// consume after the projection; any other token routes to
    /// [`parse_select_item`](Self::parse_select_item), which reports the precise error.
    fn at_empty_target_list(&mut self) -> ParseResult<bool> {
        let Some(token) = self.peek()? else {
            return Ok(true); // end of input: a bare `SELECT`
        };
        Ok(match token.kind {
            // Statement terminator / subquery or paren close.
            TokenKind::Punctuation(Punctuation::Semicolon | Punctuation::RParen) => true,
            // The SELECT-body clauses (`INTO`/`FROM`/`WHERE`/`GROUP`/`HAVING`/`WINDOW`),
            // the query-tail clauses (`ORDER`/`LIMIT`/`OFFSET`/`FETCH`), and the set
            // operators (`UNION`/`INTERSECT`/`EXCEPT`) that may follow the target list.
            TokenKind::Keyword(keyword) => matches!(
                keyword,
                Keyword::Into
                    | Keyword::From
                    | Keyword::Where
                    | Keyword::Group
                    | Keyword::Having
                    | Keyword::Window
                    | Keyword::Order
                    | Keyword::Limit
                    | Keyword::Offset
                    | Keyword::Fetch
                    | Keyword::Union
                    | Keyword::Intersect
                    | Keyword::Except
            ),
            _ => false,
        })
    }

    /// True when the `GROUP BY` key list has ended: the next token is a clause keyword
    /// that follows the grouping keys, a set operator, or a statement terminator (`;`,
    /// `)`, end of input) — never a grouping item. The trailing-comma closer for the
    /// open-ended `GROUP BY` list (`GROUP BY a, b,`), the grouping-key analogue of
    /// [`at_empty_target_list`](Self::at_empty_target_list).
    ///
    /// A grouping item is a `ROLLUP`/`CUBE`/`GROUPING SETS` construct or a general
    /// expression, and every token here is instead a *reserved* follower keyword or a
    /// terminator, so it can never begin one — the same disjointness that lets
    /// `at_empty_target_list` recognize the empty projection by its follower rather than
    /// by enumerating the (open) set of expression starts. The followers are exactly the
    /// clauses [`parse_select_body_tail`](Self::parse_select_body_tail) and the enclosing
    /// query tail consume after the keys — `HAVING`/`WINDOW`/`QUALIFY`/`USING SAMPLE`
    /// (the last opened by its `USING` keyword) and then `ORDER`/`LIMIT`/`OFFSET`/`FETCH`
    /// and the set operators. Consulted only under
    /// [`SelectSyntax::trailing_comma`](crate::ast::dialect::SelectSyntax) and only after
    /// a comma (see [`parse_comma_separated_trailing`](Self::parse_comma_separated_trailing)),
    /// so a flag-off dialect leaves the dangling comma for `parse_group_by_item` to
    /// reject — the standard parse error. DuckDB accepts the comma before every one of
    /// these followers (engine-probed on 1.5.4).
    fn at_group_by_end(&mut self) -> ParseResult<bool> {
        self.at_group_by_end_after(0)
    }

    /// [`at_group_by_end`](Self::at_group_by_end) evaluated at the token `offset` places
    /// ahead of the cursor — used to disambiguate DuckDB's `GROUP BY ALL` mode (no
    /// grouping item follows `ALL`) from PostgreSQL's `ALL <items>` quantifier
    /// (`offset` = 1 looks past the not-yet-consumed `ALL`). Offset 0 is the plain
    /// current-token check.
    fn at_group_by_end_after(&mut self, offset: usize) -> ParseResult<bool> {
        let Some(token) = self.peek_nth(offset)? else {
            return Ok(true); // end of input: `GROUP BY a,` closing the statement
        };
        Ok(match token.kind {
            TokenKind::Punctuation(Punctuation::Semicolon | Punctuation::RParen) => true,
            TokenKind::Keyword(keyword) => matches!(
                keyword,
                Keyword::Having
                    | Keyword::Window
                    | Keyword::Qualify
                    | Keyword::Using
                    | Keyword::Order
                    | Keyword::Limit
                    | Keyword::Offset
                    | Keyword::Fetch
                    | Keyword::Union
                    | Keyword::Intersect
                    | Keyword::Except
            ),
            _ => false,
        })
    }

    /// Parse the `TABLE <relation_expr>` command into the canonical [`Select`] shape.
    ///
    /// `TABLE name` is the SQL `<explicit table>` short form for `SELECT * FROM name`
    /// (accepted by every shipped dialect, so it is ungated like `TRUNCATE`). It
    /// lowers to a wildcard projection over the one relation, tagged
    /// [`SelectSpelling::TableCommand`] so the renderer round-trips `TABLE name` (the
    /// star-projection shape PostgreSQL itself lowers `TABLE t` to, so the differential
    /// oracle compares one shape). Only a bare `relation_expr` follows — a qualified
    /// name with the optional PostgreSQL `ONLY`/`*` inheritance markers, and *no* alias
    /// or sample (`TABLE t x` is a syntax error) — while `ORDER BY`/`LIMIT` and set
    /// operations compose outside, on the enclosing query, exactly as for a `SELECT`.
    pub(super) fn parse_table_command(&mut self) -> ParseResult<Select<D::Ext>> {
        let keyword = self
            .advance()?
            .expect("parse_table_command is reached only at the TABLE keyword");
        let relation = self.parse_table_command_relation()?;
        let wildcard_meta = self.make_meta(keyword.span);
        let relation_span = relation.span();
        let table_meta = self.make_meta(relation_span);
        let span = keyword.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Select {
            distinct: None,
            straight_join: false,
            projection: thin_vec![SelectItem::Wildcard {
                options: None,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: wildcard_meta,
            }],
            into: None,
            from: thin_vec![TableWithJoins {
                relation,
                joins: ThinVec::new(),
                meta: table_meta,
            }],
            lateral_views: ThinVec::new(),
            connect_by: None,
            selection: None,
            group_by: ThinVec::new(),
            group_by_quantifier: None,
            group_by_all: None,
            having: None,
            windows: ThinVec::new(),
            qualify: None,
            sample: None,
            spelling: SelectSpelling::TableCommand,
            meta,
        })
    }

    /// Parse the `relation_expr` after `TABLE`: `[ONLY] qualified_name [*]` or
    /// `ONLY ( qualified_name )`, yielding a bare [`TableFactor::Table`] (no alias, no
    /// sample). The `ONLY`/`*` inheritance markers share the FROM-relation
    /// [`only`](crate::ast::dialect::TableExpressionSyntax::only) gate, so a dialect
    /// without PostgreSQL inheritance accepts only the plain `TABLE name`.
    fn parse_table_command_relation(&mut self) -> ParseResult<TableFactor<D::Ext>> {
        let start = self.current_span()?;
        // `ONLY` is the inheritance-suppression marker only when a name (or the
        // parenthesized form) follows; otherwise a bare `ONLY` is an ordinary relation
        // name (mirroring the FROM-clause `relation_expr` disambiguation).
        if self.peek_is_keyword(Keyword::Only)?
            && (self.peek_nth_is_punct(1, Punctuation::LParen)?
                || self
                    .peek_nth(1)?
                    .is_some_and(|token| self.token_can_be_column_name(token)))
        {
            if !self.features().table_expressions.only {
                return Err(self.unexpected("a table relation supported by this dialect"));
            }
            self.advance()?; // ONLY
            let (only, name) = if self.eat_punct(Punctuation::LParen)? {
                let name = self.parse_object_name()?;
                self.expect_punct(Punctuation::RParen, "`)` to close the `ONLY` table name")?;
                (OnlySyntax::Parenthesized, name)
            } else {
                (OnlySyntax::Bare, self.parse_object_name()?)
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(TableFactor::Table {
                name,
                inheritance: RelationInheritance::Only(only),
                // The `TABLE <name>` command takes a bare relation — no PartiQL path, no
                // version modifier, no MySQL tails.
                json_path: ThinVec::new(),
                version: None,
                partition: ThinVec::new(),
                alias: None,
                indexed_by: None,
                index_hints: ThinVec::new(),
                sample: None,
                table_hints: ThinVec::new(),
                meta,
            });
        }

        let name = self.parse_relation_name(start)?;
        let inheritance = self.parse_descendant_star()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableFactor::Table {
            name,
            inheritance,
            json_path: ThinVec::new(),
            version: None,
            partition: ThinVec::new(),
            alias: None,
            indexed_by: None,
            index_hints: ThinVec::new(),
            sample: None,
            table_hints: ThinVec::new(),
            meta,
        })
    }

    /// Parse a `qualified_name` in relation position, capped at the dialect's relation
    /// depth (`catalog.schema.table` for the catalog-qualified presets, `schema.table` for
    /// SQLite) exactly as the FROM-clause relation is
    /// ([`max_relation_name_parts`](Self::max_relation_name_parts)).
    fn parse_relation_name(&mut self, start: Span) -> ParseResult<ObjectName> {
        let head_reserved = self.features().reserved_column_name;
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a table name after `TABLE`"));
        };
        if !self.token_admissible(token, head_reserved) {
            return Err(self.unexpected("a table name after `TABLE`"));
        }
        let name = self.parse_object_name_with(head_reserved)?;
        if name.0.len() > self.max_relation_name_parts() {
            let span = start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(span, self.relation_name_depth_expected(), found));
        }
        Ok(name)
    }

    /// Parse one projection item: `*`, `<name>.*`, or `<expr> [[AS] alias]`.
    ///
    /// A bare `*` is the wildcard. Otherwise the item is parsed as an expression;
    /// a dotted name stops before a `.` that is not followed by a word (see
    /// [`parse_object_name`](Parser::parse_object_name)), so a trailing `.*` is
    /// still unconsumed and identifies a qualified wildcard. Anything else is an
    /// expression, optionally aliased.
    pub(super) fn parse_select_item(&mut self) -> ParseResult<SelectItem<D::Ext>> {
        // A leading `*COLUMNS(...)` is DuckDB's columns-unpack expression, not the bare
        // wildcard: `SELECT *COLUMNS('a') + 42` spreads the matched columns into an
        // ordinary value expression (`Expr::Columns` with `ColumnsSpelling::Unpack`), so
        // it must reach the expression parser rather than be consumed as `SELECT *`. The
        // primary-position arm already handles the unpack prefix; this guard only keeps
        // the wildcard branch from claiming the `*` first.
        if self.peek_is_op(Operator::Star)? && !self.peek_is_columns_unpack_prefix()? {
            let token = self
                .advance()?
                .expect("peek_is_op confirmed a wildcard token is present");
            let options = self.parse_gated_wildcard_options(token.span)?;
            let (alias, alias_spelling) = self.parse_gated_wildcard_alias()?;
            let span = token.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(SelectItem::Wildcard {
                options,
                alias,
                alias_spelling,
                meta,
            });
        }
        self.parse_projection_value_item()
    }

    /// Parse the value form of a projection item (a SELECT target or a `RETURNING`
    /// item), after any leading bare `*` has been ruled out: an expression, a
    /// qualified wildcard `<name>.*`, or a value-position composite star `(expr).*`,
    /// each with an optional alias.
    ///
    /// The target expression is parsed with the value-position `.*` star selector
    /// suppressed at its top level, so a bare-name `t.*` stays a
    /// [`SelectItem::QualifiedWildcard`] (and admits the DuckDB wildcard modifiers)
    /// rather than folding into a value expression. A trailing `.*` on a *non-name*
    /// base — `(func()).*`, `(a).b.*` — has no qualified-name form, so under
    /// [`ExpressionSyntax::field_wildcard`](crate::ast::dialect::ExpressionSyntax::field_wildcard)
    /// it folds into a value composite-star expression; with the flag off it is left
    /// unconsumed and rejects downstream, as before.
    pub(super) fn parse_projection_value_item(&mut self) -> ParseResult<SelectItem<D::Ext>> {
        // DuckDB's prefix colon alias: `<alias> : <expr>`, the alias written before the
        // value. It folds onto the ordinary trailing-alias field (DuckDB canonicalizes it
        // to `AS`), so once the prefix is read the item takes no trailing alias — a
        // following `AS y` / bare word is left unconsumed and rejects, matching the engine.
        if self.peek_starts_prefix_colon_alias()? {
            let alias = self.parse_bare_alias_ident()?;
            let start = alias.span();
            self.expect_punct(Punctuation::Colon, "`:` after a prefix alias")?;
            let expr = self.parse_expr()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(SelectItem::Expr {
                expr,
                alias: Some(alias),
                alias_spelling: AliasSpelling::PrefixColon,
                meta,
            });
        }
        let expr = self.parse_projection_target_expr()?;
        let qualified_wildcard =
            self.peek_is_punct(Punctuation::Dot)? && self.peek_nth_is_op(1, Operator::Star)?;
        if qualified_wildcard {
            // `<name>.*`: a dotted column name immediately followed by `.*` — the
            // select-list qualified wildcard, admitted under every preset.
            if let Expr::Column { name, .. } = expr {
                self.advance()?; // `.`
                let star = self
                    .advance()?
                    .expect("peek confirmed a `*` follows the dot");
                let options = self.parse_gated_wildcard_options(star.span)?;
                let (alias, alias_spelling) = self.parse_gated_qualified_wildcard_alias()?;
                let span = name.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(SelectItem::QualifiedWildcard {
                    name,
                    options,
                    alias,
                    alias_spelling,
                    meta,
                });
            }
            // A non-name base: `(func()).*` composite expansion in a target position.
            // The star suppression only defers the fold to here; it still needs the
            // value-star flag on.
            if self.features().expression_syntax.field_wildcard {
                self.advance()?; // `.`
                self.advance()?; // `*`
                let star = self.build_field_wildcard(expr);
                let start = star.span();
                let (alias, alias_spelling) = self.parse_optional_alias()?;
                let span = alias
                    .as_ref()
                    .map_or(start, |alias| start.union(alias.span()));
                let meta = self.make_meta(span);
                return Ok(SelectItem::Expr {
                    expr: star,
                    alias,
                    alias_spelling,
                    meta,
                });
            }
        }
        let start = expr.span();
        let (alias, alias_spelling) = self.parse_optional_alias()?;
        let span = alias
            .as_ref()
            .map_or(start, |alias| start.union(alias.span()));
        let meta = self.make_meta(span);
        Ok(SelectItem::Expr {
            expr,
            alias,
            alias_spelling,
            meta,
        })
    }

    /// Parse the DuckDB wildcard-modifier tail after a select-list/`RETURNING`
    /// `*`/`t.*`, but only when [`SelectSyntax::wildcard_modifiers`](crate::ast::dialect::SelectSyntax::wildcard_modifiers) is on. When off,
    /// a trailing `EXCLUDE`/`REPLACE`/`RENAME` is left unconsumed and surfaces as the
    /// usual downstream parse error — the over-acceptance guard for non-DuckDB
    /// dialects. `star_span` anchors the synthesized [`WildcardOptions`] span.
    pub(super) fn parse_gated_wildcard_options(
        &mut self,
        star_span: Span,
    ) -> ParseResult<Option<Box<WildcardOptions<D::Ext>>>> {
        if !self.features().select_syntax.wildcard_modifiers {
            return Ok(None);
        }
        self.parse_wildcard_modifier_tail(star_span)
    }

    /// Parse the optional alias on a select-list/`RETURNING` *bare* `*`, gated on the
    /// same DuckDB star axis as the modifiers
    /// ([`SelectSyntax::wildcard_modifiers`](crate::ast::dialect::SelectSyntax::wildcard_modifiers)).
    /// DuckDB admits `SELECT * AS idx` — the alias renames *every* star-expanded column
    /// (engine-probed on 1.5.4) — and the bare `SELECT * idx`, each written *after* any
    /// `EXCLUDE`/`REPLACE`/`RENAME` tail (`* EXCLUDE (a) AS idx`; `* AS idx EXCLUDE (a)`
    /// is a syntax error there and here, since the modifier tail is consumed first).
    /// When the gate is off, the word is left unconsumed so `SELECT * x` rejects
    /// downstream, exactly as before. Reuses
    /// [`parse_optional_alias`](Self::parse_optional_alias) — the same machinery a
    /// [`SelectItem::Expr`] projection uses — so the [`AliasSpelling`] tag (bare vs `AS`)
    /// rides for free; a star alias never reaches the `PrefixColon` form. The *qualified*
    /// `t.*` alias is a separate axis
    /// ([`parse_gated_qualified_wildcard_alias`](Self::parse_gated_qualified_wildcard_alias)):
    /// the bare-`*` rename-all is DuckDB-only, but a qualified wildcard's plain alias is a
    /// PostgreSQL surface too, so the two do not share a gate.
    pub(super) fn parse_gated_wildcard_alias(
        &mut self,
    ) -> ParseResult<(Option<Ident>, AliasSpelling)> {
        if !self.features().select_syntax.wildcard_modifiers {
            return Ok((None, AliasSpelling::As));
        }
        self.parse_optional_alias()
    }

    /// Parse the optional alias on a select-list/`RETURNING` *qualified* wildcard `t.*`,
    /// gated on
    /// [`SelectSyntax::qualified_wildcard_alias`](crate::ast::dialect::SelectSyntax::qualified_wildcard_alias).
    /// PostgreSQL reads `t.*` as an ordinary column-reference expression, so it takes the
    /// very same `[AS] label` projection alias an expression does (`SELECT t.* x` /
    /// `SELECT t.* AS x`; the bare form admits the `BareColLabel` reserved set, the `AS`
    /// form the full `ColLabel` set — libpg_query-measured, matching
    /// [`parse_optional_alias`](Self::parse_optional_alias) exactly, which is why it is
    /// reused). DuckDB admits it too (its qualified star aliases like the bare one). When
    /// the gate is off (ANSI/MySQL/SQLite, where `t.*` is a non-aliasable wildcard
    /// production — engine-measured reject), the word is left unconsumed and `SELECT t.* x`
    /// rejects downstream, exactly as before. Kept distinct from
    /// [`parse_gated_wildcard_alias`](Self::parse_gated_wildcard_alias) because the bare-`*`
    /// alias and this qualified one have different dialect boundaries.
    pub(super) fn parse_gated_qualified_wildcard_alias(
        &mut self,
    ) -> ParseResult<(Option<Ident>, AliasSpelling)> {
        if !self.features().select_syntax.qualified_wildcard_alias {
            return Ok((None, AliasSpelling::As));
        }
        self.parse_optional_alias()
    }

    /// Parse `[EXCLUDE …] [REPLACE …] [RENAME …]` in DuckDB's fixed surface order,
    /// each modifier optional and at most once. Parsing strictly in this sequence is
    /// what rejects an out-of-order or repeated modifier: a `REPLACE (…) EXCLUDE (…)`
    /// leaves the second keyword unconsumed for the caller to reject, exactly as
    /// DuckDB syntax-errors on it (probed on 1.5.4). Returns `None` when no modifier
    /// is present. Unconditional (no feature gate) so the `COLUMNS(*)` star form can
    /// reuse it directly; the select-list gate lives in
    /// [`parse_gated_wildcard_options`](Self::parse_gated_wildcard_options).
    pub(super) fn parse_wildcard_modifier_tail(
        &mut self,
        star_span: Span,
    ) -> ParseResult<Option<Box<WildcardOptions<D::Ext>>>> {
        let exclude = if self.eat_keyword(Keyword::Exclude)? {
            self.parse_wildcard_column_list()?
        } else {
            ThinVec::new()
        };
        let replace = if self.eat_keyword(Keyword::Replace)? {
            self.parse_wildcard_parenthesizable(Self::parse_wildcard_replace_item)?
        } else {
            ThinVec::new()
        };
        let rename = if self.eat_keyword(Keyword::Rename)? {
            self.parse_wildcard_parenthesizable(Self::parse_wildcard_rename_item)?
        } else {
            ThinVec::new()
        };
        if exclude.is_empty() && replace.is_empty() && rename.is_empty() {
            return Ok(None);
        }
        let span = star_span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(WildcardOptions {
            exclude,
            replace,
            rename,
            meta,
        })))
    }

    /// Parse the `EXCLUDE` operand: a parenthesized comma list `(a, t.b)` or a single
    /// bare column reference `a` (the bare form takes exactly one item — DuckDB reads
    /// `EXCLUDE a, b` as `EXCLUDE (a)` plus a second projection item).
    fn parse_wildcard_column_list(&mut self) -> ParseResult<ThinVec<ObjectName>> {
        self.parse_wildcard_parenthesizable(Self::parse_object_name)
    }

    /// Shared `( item [, item]* )`-or-bare-`item` shape behind all three modifiers:
    /// a parenthesized comma list, else a single bare item. The parenthesized form
    /// tolerates a single trailing comma before `)` under DuckDB's list tolerance
    /// (`* EXCLUDE (a,)`, `* REPLACE (e AS c,)`, `* RENAME (a AS b,)`; engine-probed on
    /// 1.5.4) — covering the select-list/`RETURNING`, `ORDER BY *`, and `COLUMNS(*)`
    /// modifier lists that all route through here. The bare (unparenthesized) form takes
    /// exactly one item, so no trailing comma is possible there.
    fn parse_wildcard_parenthesizable<T>(
        &mut self,
        parse_item: impl Fn(&mut Self) -> ParseResult<T> + Copy,
    ) -> ParseResult<ThinVec<T>> {
        if self.eat_punct(Punctuation::LParen)? {
            let items = self.parse_comma_separated_trailing(parse_item, |p| {
                p.trailing_comma_at(Punctuation::RParen)
            })?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the wildcard modifier list",
            )?;
            Ok(items)
        } else {
            Ok(thin_vec![parse_item(self)?])
        }
    }

    /// One `REPLACE` entry: `<expr> AS <col>` — the replacement expression and the
    /// output column it stands in for.
    fn parse_wildcard_replace_item(&mut self) -> ParseResult<WildcardReplace<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        self.expect_keyword(Keyword::As)?;
        let column = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(WildcardReplace { expr, column, meta })
    }

    /// One `RENAME` entry: `<col> AS <new>` — the source column (which DuckDB permits
    /// to be qualified) renamed to the unqualified output name.
    fn parse_wildcard_rename_item(&mut self) -> ParseResult<WildcardRename> {
        let start = self.current_span()?;
        let column = self.parse_object_name()?;
        self.expect_keyword(Keyword::As)?;
        let alias = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(WildcardRename {
            column,
            alias,
            meta,
        })
    }

    /// Parse a non-empty comma-separated list: one `f`, then `f` after each comma —
    /// the shared shape behind ~30 comma lists across the parser
    /// (`eval-parser-generic-comma-separated-combinator`). `#[inline]` is
    /// LOAD-BEARING: it collapses each monomorphization back into the caller, so the
    /// ~30 sites cost no more machine code than the inline loops they replaced
    /// (measured size-neutral).
    /// Dropping `#[inline]` re-emits ~30 standalone functions and regrows the artifact.
    #[inline]
    pub(super) fn parse_comma_separated<T>(
        &mut self,
        mut f: impl FnMut(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<ThinVec<T>> {
        let mut items = thin_vec![f(self)?];
        while self.eat_punct(Punctuation::Comma)? {
            items.push(f(self)?);
        }
        Ok(items)
    }

    /// Parse a comma-separated expression list (one or more; no trailing comma).
    ///
    /// The expression lists that reject a trailing comma (the window `PARTITION BY` list,
    /// the `PIVOT` grouping list, and similar positions); each item is a full Pratt
    /// expression. (A `GROUP BY` item is the richer
    /// [`GroupByItem`], parsed by [`parse_group_by_item`](Self::parse_group_by_item); the
    /// `ROLLUP`/`CUBE` argument lists and the `DISTINCT ON` key list tolerate a trailing
    /// comma under DuckDB and route through the trailing-aware
    /// [`parse_comma_separated_trailing`](Self::parse_comma_separated_trailing) instead.)
    pub(super) fn parse_comma_separated_exprs(&mut self) -> ParseResult<ThinVec<Expr<D::Ext>>> {
        self.parse_comma_separated(Self::parse_expr)
    }

    /// [`parse_comma_separated`](Self::parse_comma_separated) that, under
    /// [`SelectSyntax::trailing_comma`](crate::ast::dialect::SelectSyntax) — DuckDB's list
    /// tolerance — discards a single trailing comma before the list's closer. `at_close`
    /// reports whether the cursor sits at that closer; it is consulted only after a comma
    /// and only when the dialect tolerates the trailing comma, so a flag-off dialect
    /// parses exactly as `parse_comma_separated` (the dangling comma then fails in `f`, the
    /// standard reject). Applied per accepting list site rather than folded into
    /// `parse_comma_separated`, because DuckDB rejects the trailing comma in the
    /// function-argument, `ORDER BY`, and row-constructor lists that also route through the
    /// shared combinator (see the flag docs). `#[inline]` is load-bearing for the same
    /// size-neutral reason as `parse_comma_separated`.
    #[inline]
    pub(super) fn parse_comma_separated_trailing<T>(
        &mut self,
        mut f: impl FnMut(&mut Self) -> ParseResult<T>,
        mut at_close: impl FnMut(&mut Self) -> ParseResult<bool>,
    ) -> ParseResult<ThinVec<T>> {
        let mut items = thin_vec![f(self)?];
        while self.eat_punct(Punctuation::Comma)? {
            if self.features().select_syntax.trailing_comma && at_close(self)? {
                break;
            }
            items.push(f(self)?);
        }
        Ok(items)
    }

    /// The trailing-comma closer test for a list whose closing delimiter is a single
    /// punctuation (`)` / `]` / `}`): true when the dialect tolerates a trailing comma
    /// ([`SelectSyntax::trailing_comma`](crate::ast::dialect::SelectSyntax)) *and* the
    /// cursor sits at `close`. Used at the bespoke element loops (list / struct / map
    /// literals) whose shape does not route through
    /// [`parse_comma_separated_trailing`](Self::parse_comma_separated_trailing).
    pub(super) fn trailing_comma_at(&mut self, close: Punctuation) -> ParseResult<bool> {
        Ok(self.features().select_syntax.trailing_comma && self.peek_is_punct(close)?)
    }

    /// Parse an optional projection alias: `AS <ColLabel>` or a bare `<BareColLabel>`.
    ///
    /// The two positions reserve different keyword sets (prod-keyword-position-reserved-sets):
    /// an explicit `AS` introduces a `ColLabel`, which admits *every* keyword
    /// (`SELECT a AS select` is valid), while a bare alias is a `BareColLabel`,
    /// which rejects the `AS_LABEL` keywords — so `SELECT a over` / `SELECT a filter`
    /// are not aliases (and `FROM t WHERE …` still cannot read `WHERE` as an alias).
    pub(super) fn parse_optional_alias(&mut self) -> ParseResult<(Option<Ident>, AliasSpelling)> {
        if self.eat_keyword(Keyword::As)? {
            // MySQL admits a string literal as a projection alias (`SELECT 1 AS 'x'`);
            // this position is column-specific, so the string form does not leak into
            // table/schema-name aliases (which share `parse_as_alias_ident`).
            if self.features().select_syntax.alias_string_literals {
                if let Some(ident) = self.parse_string_alias_ident()? {
                    return Ok((Some(ident), AliasSpelling::As));
                }
            }
            // MySQL has no PostgreSQL `ColLabel` relaxation on the projection `AS` alias — a
            // reserved word is rejected there exactly as in the bare-alias position — so it
            // routes this one position to the stricter `reserved_bare_alias` set, while the
            // dotted-name continuation (which also flows through `parse_as_alias_ident`)
            // keeps the permissive `reserved_as_label` set.
            if self.features().select_syntax.as_alias_rejects_reserved {
                return Ok((Some(self.parse_bare_alias_ident()?), AliasSpelling::As));
            }
            return Ok((Some(self.parse_as_alias_ident()?), AliasSpelling::As));
        }
        // A FROM-less `SELECT <expr> SETTINGS name = …` reaches the query-tail settings
        // clause; the bare word `SETTINGS` here opens that clause, not a projection alias
        // named `settings`, so decline it (the same discriminator the table-alias
        // position uses).
        if self.peek_opens_settings_clause()? {
            return Ok((None, AliasSpelling::As));
        }
        // Likewise a FROM-less `SELECT <expr> FORMAT <name>` reaches the query-tail format
        // clause; the bare word `FORMAT` here opens that clause, not a projection alias
        // named `format`, so decline it (the same discriminator the table-alias position
        // uses).
        if self.peek_opens_format_clause()? {
            return Ok((None, AliasSpelling::As));
        }
        // A bare (`AS`-less) string-literal alias (`SELECT 1 'x'`), SQLite's and MySQL's
        // rule: a `String` token in bare-alias position is the column name. For MySQL the
        // adjacent-string-concat overlap is already resolved — a string primary folds its
        // following string continuations during expression parsing, so a string only reaches
        // here after a non-string expression. DuckDB accepts only the `AS 'x'` form, so this
        // rides its own axis rather than `alias_string_literals`.
        if self.features().select_syntax.bare_alias_string_literals {
            if let Some(ident) = self.parse_string_alias_ident()? {
                return Ok((Some(ident), AliasSpelling::Bare));
            }
        }
        if self.peek_can_start_bare_alias()? {
            Ok((Some(self.parse_bare_alias_ident()?), AliasSpelling::Bare))
        } else {
            Ok((None, AliasSpelling::As))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{
        FeatureDelta, FeatureSet, GroupingSyntax, QueryTailSyntax, SelectSyntax, TableFactorSyntax,
    };
    use crate::ast::{
        Expr, GroupByItem, NoExt, OnlySyntax, RelationInheritance, Resolver as _, RollupSpelling,
        Select, SelectDistinct, SelectItem, SelectSpelling, SetExpr, SetQuantifier, Span,
        Statement, TableFactor, TemporaryTableKind,
    };
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};

    /// ANSI plus the PostgreSQL SELECT-clause extensions, isolating the new forms
    /// from the rest of the PostgreSQL preset (casing, etc.).
    const PG_SELECT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .select_syntax(SelectSyntax::POSTGRES)
                .query_tail_syntax(QueryTailSyntax::POSTGRES)
                .grouping_syntax(GroupingSyntax::POSTGRES),
        );
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the `QUALIFY` clause flag alone, isolating the clause gate from the
    /// DuckDb preset's `QUALIFY` keyword *reservation* (tested with the preset in
    /// `crate::dialect::duckdb`).
    const QUALIFY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                qualify: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the `GROUP BY ALL` / `ORDER BY ALL` clause-mode flags alone,
    /// isolating the two gates from the rest of the DuckDb preset. Implements
    /// `RenderDialect` for the exact-text round-trip checks (the stock DuckDb
    /// preset has no Tier-1 render target yet).
    const BY_ALL_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.grouping_syntax(GroupingSyntax {
                group_by_all: true,
                order_by_all: true,
                ..GroupingSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the FROM-first flag alone (and the two `*_by_all` clause modes it
    /// co-occurs with in the corpus), isolating the `from_first` gate from the rest of
    /// the DuckDb preset. Implements `RenderDialect` for the exact-text round-trip checks.
    const FROM_FIRST_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .select_syntax(SelectSyntax {
                    from_first: true,
                    ..SelectSyntax::ANSI
                })
                .grouping_syntax(GroupingSyntax {
                    group_by_all: true,
                    order_by_all: true,
                    ..GroupingSyntax::ANSI
                }),
        );
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus DuckDB's prefix-colon-alias flag alone, isolating the gate from the rest
    /// of the DuckDb preset so the accept/reject boundary is attributed to this one flag.
    const COLON_ALIAS_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                prefix_colon_alias: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// DuckDB's prefix colon alias `SELECT j : 42` folds onto the ordinary alias field —
    /// the alias `j` on the value `42`, identical shape to `42 AS j` (no new node, no
    /// spelling tag). Gated by the flag: off under plain ANSI, a `:` head rejects.
    #[test]
    fn prefix_colon_alias_projection_folds_onto_alias_field() {
        let parsed = parse_with(
            "SELECT j : 42",
            crate::ParseConfig::new(COLON_ALIAS_DIALECT),
        )
        .expect("prefix colon alias parses");
        let [SelectItem::Expr { alias, .. }] = projection(&parsed) else {
            panic!("expected one aliased expression item");
        };
        let alias = alias.as_ref().expect("the prefix alias is captured");
        assert_eq!(parsed.resolver().resolve(alias.sym), "j");

        // Canonical render rewrites the colon to a trailing `AS` — matching DuckDB's own
        // json round-trip (`SELECT j : 42` → `SELECT 42 AS j`), the reuse this feature
        // rides: no new node, no spelling tag.
        let rendered = crate::render::Renderer::new(COLON_ALIAS_DIALECT)
            .render_parsed(&parsed)
            .expect("renders");
        assert_eq!(rendered, "SELECT 42 AS j");

        // The gate, honoured as data: the same text rejects with the flag off.
        assert!(parse_with("SELECT j : 42", crate::ParseConfig::new(TestDialect)).is_err());
    }

    /// The three alias spellings coexist in one projection list: prefix `j1 : 42`, trailing
    /// `42 AS j2`, and bare `42 j3` (a corpus line). Each item captures its own alias.
    #[test]
    fn prefix_colon_alias_coexists_with_as_and_bare_aliases() {
        let parsed = parse_with(
            "SELECT j1 : 42, 42 AS j2, 42 j3",
            crate::ParseConfig::new(COLON_ALIAS_DIALECT),
        )
        .expect("mixed alias spellings parse");
        let names: Vec<&str> = projection(&parsed)
            .iter()
            .map(|item| {
                let SelectItem::Expr { alias, .. } = item else {
                    panic!("expected an aliased expression item");
                };
                parsed
                    .resolver()
                    .resolve(alias.as_ref().expect("an alias").sym)
            })
            .collect();
        assert_eq!(names, ["j1", "j2", "j3"]);
    }

    /// The prefix alias is mutually exclusive with a trailing alias (`SELECT a : 42 AS b`
    /// rejects, probed on 1.5.4), and the `:` head fires only for a lone identifier that
    /// immediately abuts the colon — a qualified name (`a.b :`) and a call (`f() :`) are
    /// structurally rejected, and the alias never chains (`p : q : 42`). The admissible
    /// LHS follows the shared bare-alias reserved set (dialect data), so a quoted
    /// identifier is admitted.
    #[test]
    fn prefix_colon_alias_rejects_trailing_alias_and_non_identifier_lhs() {
        assert!(
            parse_with(
                "SELECT a : 42 AS b",
                crate::ParseConfig::new(COLON_ALIAS_DIALECT)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT a.b : 42",
                crate::ParseConfig::new(COLON_ALIAS_DIALECT)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT foo() : 42",
                crate::ParseConfig::new(COLON_ALIAS_DIALECT)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT p : q : 42",
                crate::ParseConfig::new(COLON_ALIAS_DIALECT)
            )
            .is_err()
        );
        // A quoted identifier LHS is admitted (`"my col" : 42`).
        assert!(
            parse_with(
                "SELECT \"my col\" : 42",
                crate::ParseConfig::new(COLON_ALIAS_DIALECT)
            )
            .is_ok()
        );
    }

    fn projection(parsed: &Parsed) -> &[SelectItem<NoExt>] {
        &select_of(parsed).projection
    }

    fn select_of(parsed: &Parsed) -> &Select<NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        select
    }

    #[test]
    fn explicit_as_alias_is_captured() {
        let parsed = parse_with("SELECT a AS x", crate::ParseConfig::new(TestDialect))
            .expect("aliased projection parses");
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &projection(&parsed)[0]
        else {
            panic!("expected an aliased expression item");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "x");
    }

    #[test]
    fn implicit_alias_without_as_is_captured() {
        // `a b` aliases the column `a` as `b` with the `AS` keyword elided.
        let parsed = parse_with("SELECT a b", crate::ParseConfig::new(TestDialect))
            .expect("implicit alias parses");
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &projection(&parsed)[0]
        else {
            panic!("expected an implicitly aliased item");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "b");
    }

    #[test]
    fn non_reserved_keyword_can_be_a_column_or_alias() {
        // `Range` is an unreserved keyword usable as a bare column (`ColId`); an
        // `AS` alias is a ColLabel that admits any keyword (`Desc`, reserved); and a
        // bare alias is a BareColLabel that admits non-`AS_LABEL` keywords (`Nulls`).
        let parsed = parse_with(
            "SELECT Range, a AS Desc, b Nulls",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("contextual keywords can be identifiers");
        let items = projection(&parsed);

        let SelectItem::Expr {
            expr: Expr::Column { name, .. },
            alias: None,
            ..
        } = &items[0]
        else {
            panic!("expected `Range` as a bare column");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "Range");

        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &items[1]
        else {
            panic!("expected `Desc` as an explicit alias");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "Desc");

        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &items[2]
        else {
            panic!("expected `Nulls` as an implicit alias");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "Nulls");
    }

    #[test]
    fn reserved_keyword_is_accepted_as_an_explicit_alias() {
        // D1 (`prod-keyword-position-reserved-sets`): an `AS` alias is a ColLabel,
        // which admits *every* keyword — including reserved ones like `FROM` — so
        // `SELECT a AS from` parses, matching PostgreSQL.
        let parsed = parse_with("SELECT a AS from", crate::ParseConfig::new(TestDialect))
            .expect("a reserved keyword is a valid AS alias");
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &projection(&parsed)[0]
        else {
            panic!("expected an aliased projection");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "from");
    }

    #[test]
    fn reserved_keyword_cannot_start_a_projection_expression() {
        let err = parse_with("SELECT FROM", crate::ParseConfig::new(TestDialect))
            .expect_err("reserved keyword is not a column expression");
        assert_eq!(err.span, Span::new(7, 11));
    }

    #[test]
    fn a_clause_keyword_is_not_taken_as_an_implicit_alias() {
        // `FROM` must not be read as the alias of the projection column `a`.
        let parsed = parse_with("SELECT a FROM t", crate::ParseConfig::new(TestDialect))
            .expect("FROM ends the projection");
        assert!(matches!(
            projection(&parsed)[0],
            SelectItem::Expr { alias: None, .. },
        ));
    }

    #[test]
    fn mysql_reserved_words_gate_identifiers_per_dialect() {
        use crate::dialect::{Ansi, MySql, Postgres};

        // Forward divergence (mysql-reserved-word-set): `RLIKE` is reserved in MySQL
        // 8.0 but free under ANSI/PostgreSQL. The word now lives in the shared
        // inventory, yet the dialect reject sets gate it differently — the same
        // mechanism the position sets already use, no new identifier logic.
        parse_with("SELECT rlike FROM t", crate::ParseConfig::new(MySql))
            .expect_err("MySQL reserves RLIKE as a column name");
        let ansi = parse_with("SELECT rlike FROM t", crate::ParseConfig::new(Ansi))
            .expect("ANSI leaves RLIKE free as an identifier");
        assert_eq!(
            ansi.resolver().resolve(match &projection(&ansi)[0] {
                SelectItem::Expr {
                    expr: Expr::Column { name, .. },
                    ..
                } => name.0[0].sym,
                _ => panic!("expected `rlike` as a bare column under ANSI"),
            }),
            "rlike",
        );
        parse_with("SELECT rlike FROM t", crate::ParseConfig::new(Postgres))
            .expect("PostgreSQL leaves RLIKE free as an identifier");

        // A MySQL bare alias is also gated: `STRAIGHT_JOIN` cannot alias under MySQL
        // but can under ANSI (it is non-reserved there).
        parse_with("SELECT a straight_join", crate::ParseConfig::new(MySql))
            .expect_err("MySQL reserves STRAIGHT_JOIN, so it is not a bare alias");
        parse_with("SELECT a straight_join", crate::ParseConfig::new(Ansi))
            .expect("ANSI admits STRAIGHT_JOIN as a bare alias");

        // Reverse divergence: PostgreSQL reserves `OFFSET`, MySQL does not, so the
        // gate swings the other way — MySQL admits it as a bare column, PostgreSQL
        // rejects it.
        parse_with("SELECT offset FROM t", crate::ParseConfig::new(MySql))
            .expect("MySQL leaves OFFSET free as a column name");
        parse_with("SELECT offset FROM t", crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL reserves OFFSET");
    }

    #[test]
    fn mysql_straight_join_modifier_rides_select() {
        use crate::dialect::{Ansi, MySql};

        // The MySQL `SELECT STRAIGHT_JOIN ...` modifier sets the `straight_join` flag
        // and is consumed before the projection, so `a` is the sole projection column.
        let parsed = parse_with(
            "SELECT STRAIGHT_JOIN a FROM t",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses the STRAIGHT_JOIN modifier");
        let select = select_of(&parsed);
        assert!(select.straight_join, "the modifier flag is set");
        assert_eq!(
            select.projection.len(),
            1,
            "STRAIGHT_JOIN is the modifier, not a projection item",
        );

        // Gated: under ANSI the flag is off, so `STRAIGHT_JOIN` is read as a column
        // reference aliased `a` (a non-reserved word there), not the modifier.
        let ansi = parse_with(
            "SELECT STRAIGHT_JOIN a FROM t",
            crate::ParseConfig::new(Ansi),
        )
        .expect("ANSI reads STRAIGHT_JOIN as an ordinary identifier");
        let ansi_select = select_of(&ansi);
        assert!(
            !ansi_select.straight_join,
            "ANSI sets no STRAIGHT_JOIN modifier"
        );
        assert!(matches!(
            ansi_select.projection[0],
            SelectItem::Expr { alias: Some(_), .. },
        ));
    }

    #[test]
    fn qualified_wildcard_keeps_its_object_name() {
        let parsed = parse_with("SELECT t.*", crate::ParseConfig::new(TestDialect))
            .expect("qualified wildcard parses");
        let SelectItem::QualifiedWildcard { name, .. } = &projection(&parsed)[0] else {
            panic!("expected a qualified wildcard");
        };
        assert_eq!(name.0.len(), 1, "the `t` prefix");
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "t");
    }

    #[test]
    fn multi_part_qualified_wildcard_keeps_the_whole_prefix() {
        let parsed = parse_with("SELECT a.b.*", crate::ParseConfig::new(TestDialect))
            .expect("dotted wildcard parses");
        let SelectItem::QualifiedWildcard { name, .. } = &projection(&parsed)[0] else {
            panic!("expected a qualified wildcard");
        };
        assert_eq!(name.0.len(), 2, "the `a.b` prefix");
    }

    #[test]
    fn bare_wildcard_is_unchanged() {
        let parsed = parse_with("SELECT *", crate::ParseConfig::new(TestDialect))
            .expect("bare wildcard parses");
        assert!(matches!(
            projection(&parsed)[0],
            SelectItem::Wildcard { .. }
        ));
    }

    #[test]
    fn qualified_wildcard_takes_a_projection_alias() {
        use crate::ast::AliasSpelling;
        use crate::dialect::{Ansi, MySql, Postgres, Sqlite};

        // PostgreSQL reads `t.*` as an ordinary columnref, so it takes the standard
        // `[AS] label` projection alias (engine-probed against libpg_query). The alias folds
        // onto the qualified-wildcard item's slot, carrying its bare-vs-`AS` spelling.
        let bare = parse_with("SELECT t.* a FROM t", crate::ParseConfig::new(Postgres))
            .expect("`t.* a` parses under PG");
        let SelectItem::QualifiedWildcard {
            name,
            alias: Some(alias),
            alias_spelling,
            ..
        } = &projection(&bare)[0]
        else {
            panic!("expected an aliased qualified wildcard");
        };
        assert_eq!(bare.resolver().resolve(name.0[0].sym), "t");
        assert_eq!(bare.resolver().resolve(alias.sym), "a");
        assert_eq!(*alias_spelling, AliasSpelling::Bare);

        let as_form = parse_with("SELECT t.* AS a FROM t", crate::ParseConfig::new(Postgres))
            .expect("`t.* AS a` parses under PG");
        let SelectItem::QualifiedWildcard {
            alias: Some(alias),
            alias_spelling,
            ..
        } = &projection(&as_form)[0]
        else {
            panic!("expected an aliased qualified wildcard");
        };
        assert_eq!(as_form.resolver().resolve(alias.sym), "a");
        assert_eq!(*alias_spelling, AliasSpelling::As);

        // A multi-part prefix aliases the same way (`s.t.* a`).
        parse_with("SELECT s.t.* a FROM s.t", crate::ParseConfig::new(Postgres))
            .expect("`s.t.* a` parses under PG");

        // The minimized fuzz reproducer (parse-qualified-wildcard-bare-alias): `hEE.*` then
        // the bare label `LC`, which PG accepts and we used to reject.
        parse_with("SELECT hEE.*LC;", crate::ParseConfig::new(Postgres))
            .expect("the fuzz reproducer parses under PG");

        // Asymmetry: a *bare* `*` is the non-aliasable `target_el: '*'` production, so a
        // trailing word rejects even under PG (matches libpg_query).
        assert!(
            parse_with("SELECT * a FROM t", crate::ParseConfig::new(Postgres)).is_err(),
            "PG rejects a bare-star alias",
        );

        // Gated: dialects whose `t.*` is a non-aliasable production keep the flag off, so the
        // trailing alias rejects (engine-measured Reject on ANSI/rusqlite/mysql:8).
        assert!(
            parse_with("SELECT t.* a FROM t", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects the qualified-wildcard alias (gate off)",
        );
        assert!(
            parse_with("SELECT t.* a FROM t", crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects the qualified-wildcard alias (gate off)",
        );
        assert!(
            parse_with("SELECT t.* a FROM t", crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects the qualified-wildcard alias (gate off)",
        );
    }

    #[test]
    fn no_quantifier_leaves_distinct_unset() {
        let parsed = parse_with("SELECT a", crate::ParseConfig::new(TestDialect))
            .expect("bare projection parses");
        assert!(select_of(&parsed).distinct.is_none());
    }

    #[test]
    fn explicit_all_quantifier_is_captured() {
        // `SELECT ALL` is the explicit spelling of the default; it is preserved as a
        // distinct AST state so it round-trips (mirroring `ORDER BY … ASC`).
        let parsed = parse_with("SELECT ALL a", crate::ParseConfig::new(TestDialect))
            .expect("SELECT ALL parses");
        assert!(matches!(
            select_of(&parsed).distinct,
            Some(SelectDistinct::Quantifier {
                quantifier: SetQuantifier::All,
                ..
            }),
        ));
    }

    #[test]
    fn distinct_quantifier_is_captured() {
        let parsed = parse_with("SELECT DISTINCT a", crate::ParseConfig::new(TestDialect))
            .expect("SELECT DISTINCT parses");
        assert!(matches!(
            select_of(&parsed).distinct,
            Some(SelectDistinct::Quantifier {
                quantifier: SetQuantifier::Distinct,
                ..
            }),
        ));
    }

    #[test]
    fn distinct_on_keeps_its_key_expressions() {
        let parsed = parse_with(
            "SELECT DISTINCT ON (a, b) c FROM t",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("DISTINCT ON parses under PostgreSQL");
        let Some(SelectDistinct::On { exprs, .. }) = &select_of(&parsed).distinct else {
            panic!("expected a DISTINCT ON quantifier");
        };
        assert_eq!(exprs.len(), 2, "both ON keys are retained");
        assert!(matches!(exprs[0], Expr::Column { .. }));
    }

    #[test]
    fn distinct_on_is_rejected_without_the_dialect_gate() {
        // ANSI does not gate `DISTINCT ON`, so `ON` is read as a projection token and
        // rejected (it is a reserved keyword, not a column expression).
        let err = parse_with(
            "SELECT DISTINCT ON (a) c",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI rejects DISTINCT ON");
        // `ON` sits at bytes 16..18, right after `SELECT DISTINCT `.
        assert_eq!(err.span, Span::new(16, 18));
    }

    #[test]
    fn select_into_captures_the_target_table() {
        // PostgreSQL's `SELECT … INTO <table>` create-table form: the target sits
        // between the projection and `FROM`. `INTO` is reserved as a bare alias, so it
        // is not swallowed as the alias of the projection column.
        let parsed = parse_with(
            "SELECT a INTO t FROM s",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("SELECT INTO parses");
        let select = select_of(&parsed);
        let into = select.into.as_ref().expect("the INTO target is captured");
        assert!(into.temporary.is_none(), "no TEMP marker was written");
        assert_eq!(into.name.0.len(), 1, "an unqualified target name");
        assert_eq!(parsed.resolver().resolve(into.name.0[0].sym), "t");
        // The projection column is not aliased `into` — `INTO` ended the projection.
        assert!(matches!(
            select.projection[0],
            SelectItem::Expr { alias: None, .. },
        ));
        // The `FROM` clause after the target still parses.
        assert_eq!(select.from.len(), 1, "the FROM source is retained");
    }

    #[test]
    fn select_into_temp_marks_a_temporary_target() {
        let parsed = parse_with(
            "SELECT a INTO TEMP t FROM s",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("SELECT INTO TEMP parses");
        let into = select_of(&parsed)
            .into
            .as_ref()
            .expect("the INTO target is captured");
        assert_eq!(into.temporary, Some(TemporaryTableKind::Temp));
    }

    #[test]
    fn select_into_temporary_keeps_the_long_spelling() {
        // The `TEMPORARY` long form is a distinct surface spelling from `TEMP`; it is
        // preserved (not folded to `TEMP`) so the target round-trips exactly, reusing
        // the same `TemporaryTableKind` shape `CREATE TABLE` uses.
        let parsed = parse_with(
            "SELECT a INTO TEMPORARY t FROM s",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("SELECT INTO TEMPORARY parses");
        let into = select_of(&parsed)
            .into
            .as_ref()
            .expect("the INTO target is captured");
        assert_eq!(into.temporary, Some(TemporaryTableKind::Temporary));
    }

    #[test]
    fn select_into_accepts_the_optional_table_noise_keyword() {
        // PostgreSQL's `OptTempTableName` admits an optional `TABLE` after the (optional)
        // temporary marker; it is pure noise, so it is consumed and dropped — the target and
        // its temporary axis are captured identically to the bare spelling.
        let plain = parse_with(
            "SELECT a INTO TABLE t FROM s",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("SELECT INTO TABLE parses");
        let into = select_of(&plain)
            .into
            .as_ref()
            .expect("INTO target captured");
        assert_eq!(into.temporary, None);

        let temp = parse_with(
            "SELECT a INTO TEMP TABLE t FROM s",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("SELECT INTO TEMP TABLE parses");
        let temp_into = select_of(&temp)
            .into
            .as_ref()
            .expect("INTO target captured");
        assert_eq!(temp_into.temporary, Some(TemporaryTableKind::Temp));
    }

    #[test]
    fn select_into_is_rejected_when_the_gate_is_off() {
        use crate::dialect::{Ansi, MySql};

        // ANSI's bare `SELECT … INTO` is PSM variable assignment, not the create-table
        // form, so the gate is off: `INTO` is left unconsumed and the trailing
        // `INTO t FROM s` is a parse error. MySQL has no such form and also rejects it.
        assert!(
            parse_with("SELECT a INTO t FROM s", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI does not accept the SELECT INTO create-table form",
        );
        assert!(
            parse_with("SELECT a INTO t FROM s", crate::ParseConfig::new(MySql)).is_err(),
            "MySQL has no SELECT INTO create-table form",
        );
    }

    fn group_by_of(parsed: &Parsed) -> &[GroupByItem<NoExt>] {
        &select_of(parsed).group_by
    }

    #[test]
    fn group_by_rollup_parses_as_a_grouping_construct() {
        // `ROLLUP (a, b)` is the grouping construct, not a call to a function named
        // `rollup` — PostgreSQL lowers the unquoted keyword in this position.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY ROLLUP (a, b)",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("ROLLUP grouping set parses");
        let group_by = group_by_of(&parsed);
        assert_eq!(group_by.len(), 1, "one GROUP BY item");
        let GroupByItem::Rollup { exprs, .. } = &group_by[0] else {
            panic!("expected a ROLLUP grouping item, got {:?}", group_by[0]);
        };
        assert_eq!(exprs.len(), 2, "both ROLLUP keys are retained");
        assert!(matches!(exprs[0], Expr::Column { .. }));
    }

    #[test]
    fn group_by_cube_parses_as_a_grouping_construct() {
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY CUBE (a, b)",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("CUBE grouping set parses");
        assert!(matches!(
            &group_by_of(&parsed)[0],
            GroupByItem::Cube { exprs, .. } if exprs.len() == 2,
        ));
    }

    #[test]
    fn group_by_grouping_sets_nests_grouping_items() {
        // GROUPING SETS nests PG's `group_by_list`, so ROLLUP / empty / plain-expr
        // items may appear inside (the recursive `group_by_item`).
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY GROUPING SETS (ROLLUP (a, b), (c), ())",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("GROUPING SETS parses");
        let GroupByItem::GroupingSets { sets, .. } = &group_by_of(&parsed)[0] else {
            panic!("expected a GROUPING SETS item");
        };
        assert_eq!(sets.len(), 3, "three nested grouping items");
        assert!(matches!(sets[0], GroupByItem::Rollup { .. }));
        assert!(matches!(sets[1], GroupByItem::Expr { .. }));
        assert!(matches!(sets[2], GroupByItem::Empty { .. }));
    }

    #[test]
    fn group_by_empty_grouping_set_parses() {
        // A bare `()` is the empty grouping set (grand total), admitted as a top-level
        // GROUP BY item per PG's `empty_grouping_set`.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY ()",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("empty grouping set parses");
        assert!(matches!(group_by_of(&parsed)[0], GroupByItem::Empty { .. }));
    }

    #[test]
    fn group_by_plain_expressions_stay_plain_items() {
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY a, b + c",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("plain GROUP BY keys parse");
        let group_by = group_by_of(&parsed);
        assert_eq!(group_by.len(), 2);
        assert!(
            group_by
                .iter()
                .all(|item| matches!(item, GroupByItem::Expr { .. }))
        );
    }

    #[test]
    fn quoted_rollup_in_group_by_stays_a_function_call() {
        // A quoted `"rollup"` is a delimited identifier, never `Keyword::Rollup`, so it
        // is a function call — matching PostgreSQL, which lowers only the bare keyword.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY \"rollup\" (a, b)",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("quoted rollup parses as a function call");
        let GroupByItem::Expr {
            expr: Expr::Function { call, .. },
            ..
        } = &group_by_of(&parsed)[0]
        else {
            panic!("expected a function call, not a grouping construct");
        };
        assert_eq!(parsed.resolver().resolve(call.name.0[0].sym), "rollup");
    }

    #[test]
    fn bare_rollup_column_falls_through_to_an_expression() {
        // `rollup` without a following `(` is an ordinary (unreserved in PG) column
        // reference, not the construct — the `(` is what admits ROLLUP as a grouping
        // set.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY rollup",
            crate::ParseConfig::new(PG_SELECT_DIALECT),
        )
        .expect("bare rollup is a column");
        let GroupByItem::Expr {
            expr: Expr::Column { name, .. },
            ..
        } = &group_by_of(&parsed)[0]
        else {
            panic!("expected a bare column");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "rollup");
    }

    #[test]
    fn group_by_rollup_is_a_function_call_when_the_gate_is_off() {
        use crate::dialect::MySql;

        // With `grouping_sets` off (MySQL), `ROLLUP (a, b)` falls through to the
        // expression grammar as an ordinary function call — MySQL resolves it as a
        // stored-function reference (`rollup` is non-reserved there). MySQL's own
        // grouping surface is the distinct trailing `WITH ROLLUP`, not modelled here.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY ROLLUP (a, b)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL reads rollup as a function call");
        assert!(matches!(
            &group_by_of(&parsed)[0],
            GroupByItem::Expr {
                expr: Expr::Function { .. },
                ..
            },
        ));
    }

    #[test]
    fn group_by_with_rollup_wraps_the_key_list_under_mysql() {
        use crate::dialect::MySql;

        // MySQL's trailing `WITH ROLLUP` applies to the whole key list, so it
        // canonicalizes into one `GroupByItem::Rollup` (spelling `WithRollup`) wrapping
        // every key — the same node `ROLLUP (…)` produces (ADR-0011), tagged so render
        // reproduces the surface.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY a, b WITH ROLLUP",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL `WITH ROLLUP` parses");
        let group_by = group_by_of(&parsed);
        assert_eq!(
            group_by.len(),
            1,
            "the key list folds into one wrapping item"
        );
        let GroupByItem::Rollup {
            exprs, spelling, ..
        } = &group_by[0]
        else {
            panic!("expected a wrapping ROLLUP item, got {:?}", group_by[0]);
        };
        assert_eq!(*spelling, RollupSpelling::WithRollup);
        assert_eq!(exprs.len(), 2, "both keys are retained inside the wrap");
        assert!(exprs.iter().all(|e| matches!(e, Expr::Column { .. })));
    }

    #[test]
    fn group_by_rollup_round_trips_each_spelling_byte_identically() {
        use crate::dialect::{Lenient, MySql, Postgres};
        use crate::render::Renderer;

        // MySQL's trailing modifier renders back as written (the `WithRollup` tag). MySQL
        // is not itself a render target, so the round-trip renders under Lenient — the
        // permissive superset that also accepts `WITH ROLLUP` — proving the tag, not the
        // target dialect, drives the trailing form.
        let mysql = "SELECT a FROM t GROUP BY a, b WITH ROLLUP";
        let parsed =
            parse_with(mysql, crate::ParseConfig::new(MySql)).expect("MySQL `WITH ROLLUP` parses");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("`WITH ROLLUP` renders"),
            mysql,
        );

        // The SQL:1999 item form is unchanged by the added tag (the `Function` default).
        let ansi = "SELECT a FROM t GROUP BY ROLLUP (a, b)";
        let parsed =
            parse_with(ansi, crate::ParseConfig::new(Postgres)).expect("`ROLLUP (…)` parses");
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("`ROLLUP (…)` renders"),
            ansi,
        );
    }

    #[test]
    fn group_by_with_rollup_is_rejected_where_the_gate_is_off() {
        use crate::dialect::{Ansi, Postgres};

        // PostgreSQL and ANSI spell the super-aggregate `ROLLUP (…)`; with `with_rollup`
        // off the trailing `WITH` is left unconsumed and surfaces as trailing input — a
        // clean parse error, not an over-acceptance. (MySQL-only surface: differential
        // pg parity is N/A, self-attested until the MySQL oracle lands.)
        assert!(
            parse_with(
                "SELECT a FROM t GROUP BY a, b WITH ROLLUP",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL rejects the MySQL `WITH ROLLUP` modifier",
        );
        assert!(
            parse_with(
                "SELECT a FROM t GROUP BY a, b WITH ROLLUP",
                crate::ParseConfig::new(Ansi)
            )
            .is_err(),
            "ANSI rejects the MySQL `WITH ROLLUP` modifier",
        );
    }

    #[test]
    fn group_by_all_parses_as_the_clause_mode() {
        use crate::dialect::DuckDb;

        let parsed = parse_with(
            "SELECT i, sum(j) FROM t GROUP BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("GROUP BY ALL parses under DuckDb");
        let select = select_of(&parsed);
        assert_eq!(
            select.group_by_all,
            Some(crate::ast::GroupByAllSpelling::Keyword),
            "the mode is set, spelled with the `ALL` keyword",
        );
        assert!(
            select.group_by.is_empty(),
            "the key list stays empty — ALL is a mode of the clause, never an item"
        );
    }

    #[test]
    fn group_by_star_parses_as_the_clause_mode() {
        use crate::dialect::DuckDb;

        // DuckDB's `GROUP BY *` shorthand opens the same mode as `GROUP BY ALL`
        // (probed on 1.5.4: identical result to `GROUP BY ALL`), tagged with its
        // `*` spelling so it round-trips.
        let parsed = parse_with(
            "SELECT i, sum(j) FROM t GROUP BY *",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("GROUP BY * parses under DuckDb");
        let select = select_of(&parsed);
        assert_eq!(
            select.group_by_all,
            Some(crate::ast::GroupByAllSpelling::Star),
            "the mode is set, spelled with the `*` shorthand",
        );
        assert!(
            select.group_by.is_empty(),
            "the key list stays empty — `*` is the mode, never an item"
        );
    }

    #[test]
    fn group_by_star_rejects_mixing_and_bind_only_forms() {
        use crate::dialect::DuckDb;

        // Probed on DuckDB 1.5.4: `GROUP BY *` is bare-only. The engine binder-rejects
        // `*` beside a sibling item ("STAR expression is not supported here"), and our
        // parse-level contract rejects the same shapes — the mode consumes only its
        // wildcard, so a leftover comma/item/paren fails as trailing input. No
        // over-acceptance beyond the bare form.
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY *, i",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY i, *",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY ROLLUP (i), *",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY *(i)",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
    }

    #[test]
    fn group_by_star_is_rejected_where_the_gate_is_off() {
        use crate::dialect::{Ansi, MySql, Postgres, Sqlite};

        // The `*` shorthand rides the same `group_by_all` gate as the keyword; with it
        // off, a bare `*` cannot open a grouping clause — the over-acceptance guard.
        let sql = "SELECT a, count(*) FROM t GROUP BY *";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects GROUP BY *"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects GROUP BY *"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects GROUP BY *"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects GROUP BY *"
        );
    }

    #[test]
    fn group_by_all_composes_with_having() {
        use crate::dialect::DuckDb;

        // Probed on DuckDB 1.5.4: `HAVING` follows the mode as it follows a key list.
        let parsed = parse_with(
            "SELECT i, sum(j) FROM t GROUP BY ALL HAVING sum(j) > 1",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("HAVING follows the mode");
        let select = select_of(&parsed);
        assert_eq!(
            select.group_by_all,
            Some(crate::ast::GroupByAllSpelling::Keyword)
        );
        assert!(select.having.is_some());
    }

    #[test]
    fn quoted_all_in_group_by_stays_a_column() {
        use crate::dialect::DuckDb;

        // The disambiguation trap: `ALL` is reserved under DuckDB, so a column named
        // `all` must be quoted — and the quoted spelling tokenizes as an identifier,
        // an ordinary grouping key, exactly as the engine reads it (a bare `all`
        // there is the mode; probed on 1.5.4).
        let parsed = parse_with(
            "SELECT \"all\" FROM t GROUP BY \"all\"",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("a quoted all is an ordinary key");
        let select = select_of(&parsed);
        assert!(select.group_by_all.is_none());
        assert_eq!(select.group_by.len(), 1);
    }

    #[test]
    fn group_by_all_rejects_mixing_with_keys_and_grouping_sets() {
        use crate::dialect::DuckDb;

        // Probed on DuckDB 1.5.4: `ALL` admits no sibling item in either order — the
        // mode consumes only its keyword, so the leftovers fail as trailing input,
        // the same verdict as the engine's syntax errors.
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY ALL, i",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY i, ALL",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY ROLLUP (i), ALL",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT i FROM t GROUP BY ALL (i)",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
    }

    #[test]
    fn group_by_all_is_rejected_where_the_gate_is_off() {
        use crate::dialect::{Ansi, MySql, Postgres, Sqlite};

        // Every shipped dialect reserves `ALL`, so with the gate off the keyword
        // cannot open a grouping expression — the over-acceptance guard on all four.
        let sql = "SELECT a, count(*) FROM t GROUP BY ALL";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects GROUP BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects GROUP BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects GROUP BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects GROUP BY ALL"
        );
    }

    #[test]
    fn group_by_all_round_trips_byte_identically() {
        use crate::render::Renderer;

        for sql in [
            "SELECT i, sum(j) FROM t GROUP BY ALL",
            "SELECT i, sum(j) FROM t GROUP BY ALL HAVING sum(j) > 1",
            "SELECT i, j FROM t GROUP BY ALL ORDER BY ALL",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(BY_ALL_DIALECT))
                .expect("GROUP BY ALL parses");
            assert_eq!(
                Renderer::new(BY_ALL_DIALECT)
                    .render_parsed(&parsed)
                    .expect("GROUP BY ALL renders"),
                sql,
            );
        }
    }

    #[test]
    fn group_by_star_normalizes_to_all_under_target_dialect() {
        use crate::render::Renderer;

        // The `*` shorthand is a spelling of the `ALL` mode: a target-dialect re-spell
        // (the `Renderer` path) canonicalizes it onto `GROUP BY ALL`, exactly as the
        // spelling-tag doctrine prescribes (byte-exact `*` replay is the source-fidelity
        // lane's job — see the ast render tests). The keyword form is already canonical.
        for (source, expected) in [
            (
                "SELECT i, sum(j) FROM t GROUP BY *",
                "SELECT i, sum(j) FROM t GROUP BY ALL",
            ),
            (
                "SELECT i, sum(j) FROM t GROUP BY * HAVING sum(j) > 1",
                "SELECT i, sum(j) FROM t GROUP BY ALL HAVING sum(j) > 1",
            ),
        ] {
            let parsed = parse_with(source, crate::ParseConfig::new(BY_ALL_DIALECT))
                .expect("GROUP BY * parses");
            assert_eq!(
                Renderer::new(BY_ALL_DIALECT)
                    .render_parsed(&parsed)
                    .expect("GROUP BY * renders"),
                expected,
            );
        }
    }

    #[test]
    fn group_by_distinct_quantifier_prefixes_the_key_list() {
        use crate::dialect::Postgres;

        // PostgreSQL's SQL:2016 grouping-set quantifier: `DISTINCT` prefixes a non-empty
        // grouping list, recorded on `group_by_quantifier` while the items parse normally
        // (probed on pg_query PG-17: `GROUP BY DISTINCT a, b` accepts).
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY DISTINCT a, b",
            crate::ParseConfig::new(Postgres),
        )
        .expect("GROUP BY DISTINCT parses under PostgreSQL");
        let select = select_of(&parsed);
        assert_eq!(select.group_by_quantifier, Some(SetQuantifier::Distinct));
        assert!(
            select.group_by_all.is_none(),
            "the quantifier is not the DuckDB mode"
        );
        assert_eq!(select.group_by.len(), 2, "both grouping keys are retained");
    }

    #[test]
    fn group_by_quantifier_governs_grouping_set_constructs() {
        use crate::dialect::Postgres;

        // The quantifier admits the grouping-set constructs after it (probed on pg_query
        // PG-17: `GROUP BY {ALL | DISTINCT} rollup(…) / grouping sets (…) / ()`).
        let all = parse_with(
            "SELECT a FROM t GROUP BY ALL ROLLUP (a, b)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("GROUP BY ALL over ROLLUP parses");
        let select = select_of(&all);
        assert_eq!(select.group_by_quantifier, Some(SetQuantifier::All));
        assert!(matches!(select.group_by[0], GroupByItem::Rollup { .. }));

        let distinct = parse_with(
            "SELECT a FROM t GROUP BY DISTINCT GROUPING SETS ((a), (b)), ()",
            crate::ParseConfig::new(Postgres),
        )
        .expect("GROUP BY DISTINCT over GROUPING SETS parses");
        let select = select_of(&distinct);
        assert_eq!(select.group_by_quantifier, Some(SetQuantifier::Distinct));
        assert!(matches!(
            select.group_by[0],
            GroupByItem::GroupingSets { .. }
        ));
        assert!(matches!(select.group_by[1], GroupByItem::Empty { .. }));
    }

    #[test]
    fn group_by_quantifier_requires_a_grouping_list() {
        use crate::dialect::Postgres;

        // The quantifier is a prefix, not a standalone clause: PostgreSQL rejects a bare
        // `GROUP BY {ALL | DISTINCT}`, a trailing quantifier, and a doubled one (all
        // "syntax error" on pg_query PG-17). This is exactly what keeps the quantifier
        // MECE with DuckDB's standalone `GROUP BY ALL` mode.
        for sql in [
            "SELECT a FROM t GROUP BY DISTINCT",
            "SELECT a FROM t GROUP BY ALL",
            "SELECT a FROM t GROUP BY a DISTINCT",
            "SELECT a FROM t GROUP BY DISTINCT ALL a",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects `{sql}` (the quantifier needs a following list)",
            );
        }
    }

    #[test]
    fn group_by_quantifier_is_rejected_where_the_gate_is_off() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};

        // Every non-PostgreSQL shipped dialect reserves `DISTINCT`/`ALL`, so with the
        // gate off the keyword cannot open a grouping expression — the over-acceptance
        // guard. (DuckDB's own `GROUP BY ALL` mode is the standalone bare form, tested
        // separately; `GROUP BY DISTINCT a` has no DuckDB reading.)
        let sql = "SELECT a FROM t GROUP BY DISTINCT a";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects the quantifier"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects the quantifier"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects the quantifier"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB has no GROUP BY DISTINCT quantifier"
        );
    }

    #[test]
    fn group_by_quantifier_and_duckdb_mode_disambiguate_under_lenient() {
        use crate::dialect::Lenient;

        // The crux of the two-flag design: Lenient enables BOTH the PostgreSQL quantifier
        // and DuckDB's `GROUP BY ALL` mode, and they stay MECE by lookahead — a bare
        // `GROUP BY ALL` is the mode (empty item list), while `GROUP BY ALL <items>` is
        // the quantifier prefixing the list. No genuine ambiguity, so the quantifier
        // ships on for Lenient (the honest superset) rather than being disabled.
        let mode = parse_with(
            "SELECT i, sum(j) FROM t GROUP BY ALL",
            crate::ParseConfig::new(Lenient),
        )
        .expect("bare GROUP BY ALL is the DuckDB mode under Lenient");
        let select = select_of(&mode);
        assert_eq!(
            select.group_by_all,
            Some(crate::ast::GroupByAllSpelling::Keyword),
            "bare ALL is the mode"
        );
        assert_eq!(select.group_by_quantifier, None);
        assert!(select.group_by.is_empty());

        let quantifier = parse_with(
            "SELECT a FROM t GROUP BY ALL a, b",
            crate::ParseConfig::new(Lenient),
        )
        .expect("GROUP BY ALL over a list is the PostgreSQL quantifier under Lenient");
        let select = select_of(&quantifier);
        assert!(
            select.group_by_all.is_none(),
            "ALL over a list is not the mode"
        );
        assert_eq!(select.group_by_quantifier, Some(SetQuantifier::All));
        assert_eq!(select.group_by.len(), 2);

        let distinct = parse_with(
            "SELECT a FROM t GROUP BY DISTINCT a",
            crate::ParseConfig::new(Lenient),
        )
        .expect("GROUP BY DISTINCT is the quantifier under Lenient");
        assert_eq!(
            select_of(&distinct).group_by_quantifier,
            Some(SetQuantifier::Distinct)
        );
    }

    #[test]
    fn group_by_quantifier_round_trips_byte_identically() {
        use crate::dialect::Postgres;
        use crate::render::Renderer;

        // The quantifier renders back before the item list; `None` (no quantifier) stays
        // absent, so an ordinary `GROUP BY a` is unchanged.
        for sql in [
            "SELECT a FROM t GROUP BY DISTINCT a, b",
            "SELECT a FROM t GROUP BY ALL a, b",
            "SELECT a FROM t GROUP BY DISTINCT ROLLUP (a, b)",
            "SELECT a FROM t GROUP BY a",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect("quantified GROUP BY parses");
            assert_eq!(
                Renderer::new(Postgres)
                    .render_parsed(&parsed)
                    .expect("quantified GROUP BY renders"),
                sql,
            );
        }
    }

    #[test]
    fn with_rollup_requires_plain_grouping_keys() {
        use crate::dialect::Lenient;

        // `WITH ROLLUP` wraps plain keys only. Lenient enables both `grouping_sets` and
        // `with_rollup`, so a grouping-set item can reach the wrap — it is a clean error
        // rather than a nonsensical rollup-of-a-rollup.
        let err = parse_with(
            "SELECT a FROM t GROUP BY ROLLUP (a, b) WITH ROLLUP",
            crate::ParseConfig::new(Lenient),
        )
        .expect_err("a grouping-set key before `WITH ROLLUP` is rejected");
        assert!(
            err.to_string().contains("WITH ROLLUP"),
            "the error names the offending modifier, got: {err}",
        );
    }

    /// The name + inheritance of the sole relation of a `TABLE`-command Select.
    fn table_command_relation(parsed: &Parsed) -> &TableFactor<NoExt> {
        let select = select_of(parsed);
        assert_eq!(
            select.spelling,
            SelectSpelling::TableCommand,
            "the body is tagged as a TABLE command",
        );
        assert_eq!(select.projection.len(), 1, "one wildcard projection item");
        assert!(matches!(select.projection[0], SelectItem::Wildcard { .. }));
        assert_eq!(select.from.len(), 1, "the one named relation");
        &select.from[0].relation
    }

    #[test]
    fn table_command_lowers_to_a_wildcard_star_projection() {
        // `TABLE t` is `SELECT * FROM t`, so it canonicalizes to a wildcard projection
        // over the one relation, tagged `TableCommand`. It is ungated (standard SQL
        // `<explicit table>`), so a bare `TABLE t` parses even under the ANSI test dialect.
        let parsed = parse_with("TABLE t", crate::ParseConfig::new(TestDialect))
            .expect("TABLE command parses");
        let TableFactor::Table {
            name,
            inheritance: RelationInheritance::Plain,
            alias: None,
            sample: None,
            ..
        } = table_command_relation(&parsed)
        else {
            panic!("expected a plain named relation with no alias or sample");
        };
        assert_eq!(name.0.len(), 1);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "t");
    }

    #[test]
    fn table_command_keeps_qualified_names() {
        let parsed = parse_with("TABLE s.t", crate::ParseConfig::new(TestDialect))
            .expect("qualified TABLE parses");
        let TableFactor::Table { name, .. } = table_command_relation(&parsed) else {
            panic!("expected a named relation");
        };
        assert_eq!(name.0.len(), 2, "schema-qualified");
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "s");
        assert_eq!(parsed.resolver().resolve(name.0[1].sym), "t");
    }

    #[test]
    fn table_command_carries_postgres_inheritance_markers() {
        use crate::dialect::Postgres;

        // The `ONLY`/`*` `relation_expr` markers ride the PostgreSQL inheritance gate,
        // so they parse under the full Postgres preset (not the ANSI-based test dialect).
        let only = parse_with("TABLE ONLY t", crate::ParseConfig::new(Postgres))
            .expect("TABLE ONLY parses");
        assert!(matches!(
            table_command_relation(&only),
            TableFactor::Table {
                inheritance: RelationInheritance::Only(OnlySyntax::Bare),
                ..
            },
        ));

        let only_paren = parse_with("TABLE ONLY (t)", crate::ParseConfig::new(Postgres))
            .expect("TABLE ONLY (t) parses");
        assert!(matches!(
            table_command_relation(&only_paren),
            TableFactor::Table {
                inheritance: RelationInheritance::Only(OnlySyntax::Parenthesized),
                ..
            },
        ));

        let star =
            parse_with("TABLE t *", crate::ParseConfig::new(Postgres)).expect("TABLE t * parses");
        assert!(matches!(
            table_command_relation(&star),
            TableFactor::Table {
                inheritance: RelationInheritance::Descendants,
                ..
            },
        ));
    }

    #[test]
    fn table_command_rejects_alias_and_trailing_clauses() {
        // `TABLE relation_expr` takes only the bare relation — no alias, no `WHERE`, no
        // parenthesized subquery, and at most three name parts — matching PostgreSQL.
        for sql in [
            "TABLE t x",
            "TABLE t AS x",
            "TABLE t WHERE a = 1",
            "TABLE (SELECT 1)",
            "TABLE a.b.c.d",
            "TABLE",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} must be rejected",
            );
        }
    }

    #[test]
    fn table_command_inheritance_markers_need_the_gate() {
        // Under a dialect without PostgreSQL inheritance (the ANSI test dialect), the
        // `ONLY`/`*` markers are rejected while the plain `TABLE t` still parses.
        assert!(parse_with("TABLE ONLY t", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("TABLE t *", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("TABLE t", crate::ParseConfig::new(TestDialect)).is_ok());
    }

    #[test]
    fn table_command_round_trips_each_spelling() {
        use crate::dialect::Postgres;
        use crate::render::Renderer;

        for sql in [
            "TABLE t",
            "TABLE s.t",
            "TABLE ONLY t",
            "TABLE ONLY (t)",
            "TABLE t *",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres)).expect("TABLE parses");
            assert_eq!(
                Renderer::new(Postgres)
                    .render_parsed(&parsed)
                    .expect("TABLE renders"),
                sql,
                "the TABLE command round-trips its short-form spelling",
            );
        }
    }

    #[test]
    fn empty_target_list_parses_under_the_gate() {
        // libpg_query's raw grammar makes the projection optional before any clause, so
        // a bare `SELECT` and each empty-projection-then-clause form parse under Postgres.
        for sql in [
            "SELECT",
            "SELECT FROM t",
            "SELECT WHERE a = 1",
            "SELECT GROUP BY a",
            "SELECT ORDER BY 1",
            "SELECT ALL FROM t",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(PG_SELECT_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            assert!(
                select_of(&parsed).projection.is_empty(),
                "{sql:?} has an empty projection",
            );
        }
    }

    #[test]
    fn empty_target_list_is_rejected_without_the_gate() {
        // ANSI/MySQL require ≥1 select item, so `FROM` — a reserved keyword, not an
        // expression — is a parse error where the projection is required.
        assert!(parse_with("SELECT FROM t", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("SELECT", crate::ParseConfig::new(TestDialect)).is_err());
    }

    #[test]
    fn empty_target_list_still_requires_a_list_after_distinct() {
        // PostgreSQL splits `SELECT opt_all_clause opt_target_list` (empty allowed) from
        // `SELECT distinct_clause target_list` (required), so a `DISTINCT` head with no
        // items is a syntax error even under the empty-target-list gate.
        assert!(
            parse_with(
                "SELECT DISTINCT",
                crate::ParseConfig::new(PG_SELECT_DIALECT)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT DISTINCT FROM t",
                crate::ParseConfig::new(PG_SELECT_DIALECT)
            )
            .is_err()
        );
        // A plain or explicit-`ALL` head still admits the empty list.
        assert!(parse_with("SELECT ALL", crate::ParseConfig::new(PG_SELECT_DIALECT)).is_ok());
    }

    #[test]
    fn empty_target_list_round_trips_as_a_bare_select() {
        use crate::dialect::Postgres;
        use crate::render::Renderer;

        for sql in ["SELECT", "SELECT FROM t", "SELECT WHERE a = 1"] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(Postgres)).expect("empty SELECT parses");
            assert_eq!(
                Renderer::new(Postgres)
                    .render_parsed(&parsed)
                    .expect("empty SELECT renders"),
                sql,
            );
        }
    }

    #[test]
    fn qualify_clause_parses_under_the_gate() {
        // After a GROUP BY key the word cannot be an alias, so the flag alone admits
        // the clause even without DuckDB's keyword reservation.
        let parsed = parse_with(
            "SELECT a FROM t GROUP BY a QUALIFY row_number() OVER () = 1",
            crate::ParseConfig::new(QUALIFY_DIALECT),
        )
        .expect("QUALIFY parses under the gate");
        let select = select_of(&parsed);
        assert!(
            select.qualify.is_some(),
            "the QUALIFY predicate is captured"
        );
        assert!(
            select.having.is_none(),
            "QUALIFY is its own slot, never folded into HAVING"
        );
    }

    #[test]
    fn qualify_is_rejected_without_the_gate() {
        // With the flag off (ANSI) the keyword is left unconsumed after the GROUP BY
        // key, so the trailing `QUALIFY …` is a clean parse error — the
        // over-acceptance guard.
        assert!(
            parse_with(
                "SELECT a FROM t GROUP BY a QUALIFY row_number() OVER () = 1",
                crate::ParseConfig::new(TestDialect),
            )
            .is_err(),
            "ANSI rejects the QUALIFY clause",
        );
    }

    // --- Hive/Spark LATERAL VIEW (planner-parity-select-lateral-view) ---------

    /// ANSI plus the `lateral_view_clause` flag alone, isolating the clause gate from
    /// the rest of the (feature-gated) Hive/Databricks presets. Renders for the
    /// exact-text round-trip checks.
    const LATERAL_VIEW_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                lateral_view_clause: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus BOTH the `lateral_view_clause` and derived-table `lateral` gates —
    /// the Lenient combination — used to prove the shared `LATERAL` lead partitions
    /// unambiguously between the two (position + `VIEW` follow token).
    const LATERAL_BOTH_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .select_syntax(SelectSyntax {
                    lateral_view_clause: true,
                    ..SelectSyntax::ANSI
                })
                .table_factor_syntax(TableFactorSyntax {
                    lateral: true,
                    // The lateral *function* factor (`FROM t, LATERAL f(x) v`) needs
                    // the table-function grammar on to exercise the factor side.
                    table_functions: true,
                    ..TableFactorSyntax::ANSI
                }),
        );
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn lateral_view_parses_and_round_trips_under_the_gate() {
        use crate::render::Renderer;

        for sql in [
            "SELECT a FROM t LATERAL VIEW explode(col) v",
            "SELECT a FROM t LATERAL VIEW OUTER explode(col) v",
            "SELECT a FROM t LATERAL VIEW explode(col) v AS c",
            "SELECT a FROM t LATERAL VIEW OUTER explode(col) v AS c1, c2",
            "SELECT a FROM t LATERAL VIEW json_tuple(j, 'k1', 'k2') jt AS k1, k2",
            "SELECT a FROM t LATERAL VIEW db.explode(col) v AS c",
            "SELECT a FROM t LATERAL VIEW stack(2, 'a', 1, 'b', 2) s AS key, value",
            // Repeatable, and a later view may reference an earlier one's output.
            "SELECT a FROM t LATERAL VIEW explode(m) kv AS k, vs LATERAL VIEW explode(vs) x AS v",
            // Position: after the whole FROM list (joins included), before WHERE.
            "SELECT a FROM t JOIN u ON t.id = u.id LATERAL VIEW explode(col) v AS c WHERE v.c = 1",
            "SELECT a FROM t, u LATERAL VIEW explode(col) v GROUP BY a",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(LATERAL_VIEW_DIALECT))
                .unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
            assert!(
                !select_of(&parsed).lateral_views.is_empty(),
                "{sql:?} populates lateral_views",
            );
            assert_eq!(
                Renderer::new(LATERAL_VIEW_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
                "{sql:?} round-trips",
            );
        }

        // Structural check: OUTER, the generator call, the alias, and the column
        // aliases land as typed fields.
        let parsed = parse_with(
            "SELECT a FROM t LATERAL VIEW OUTER explode(m) kv AS k, v",
            crate::ParseConfig::new(LATERAL_VIEW_DIALECT),
        )
        .expect("parses");
        let views = &select_of(&parsed).lateral_views;
        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert!(view.outer);
        assert_eq!(view.function.args.len(), 1);
        assert_eq!(view.columns.len(), 2);
    }

    #[test]
    fn lateral_view_as_keyword_is_optional_and_canonicalized() {
        use crate::render::Renderer;

        // Spark's grammar spells the column-alias `AS` as optional (`AS?`); the bare
        // spelling parses to the same shape and re-renders with the canonical `AS`
        // (a structural, not byte-exact, round-trip). Hive proper requires `AS` —
        // the documented conservative-direction over-acceptance.
        let bare = parse_with(
            "SELECT a FROM t LATERAL VIEW explode(m) kv k, v",
            crate::ParseConfig::new(LATERAL_VIEW_DIALECT),
        )
        .expect("the AS-less Spark spelling parses");
        assert_eq!(select_of(&bare).lateral_views[0].columns.len(), 2);
        assert_eq!(
            Renderer::new(LATERAL_VIEW_DIALECT)
                .render_parsed(&bare)
                .expect("renders"),
            "SELECT a FROM t LATERAL VIEW explode(m) kv AS k, v",
            "the bare spelling canonicalizes to AS",
        );
    }

    #[test]
    fn lateral_view_requires_a_generator_call_and_table_alias() {
        // The generator must be a parenthesized call…
        assert!(
            parse_with(
                "SELECT a FROM t LATERAL VIEW explode v",
                crate::ParseConfig::new(LATERAL_VIEW_DIALECT)
            )
            .is_err(),
            "a bare generator name without `(…)` rejects",
        );
        // …without the windowed/aggregate wrapper clauses…
        assert!(
            parse_with(
                "SELECT a FROM t LATERAL VIEW rank() OVER () v",
                crate::ParseConfig::new(LATERAL_VIEW_DIALECT),
            )
            .is_err(),
            "a windowed generator rejects",
        );
        // …and the table alias is required (both engine grammars make it
        // non-optional): a reserved clause keyword cannot fill the slot.
        assert!(
            parse_with(
                "SELECT a FROM t LATERAL VIEW explode(col) WHERE a = 1",
                crate::ParseConfig::new(LATERAL_VIEW_DIALECT),
            )
            .is_err(),
            "a missing table alias rejects",
        );
    }

    #[test]
    fn lateral_view_is_rejected_without_the_gate_and_without_from() {
        // With the flag off (ANSI) the post-FROM `LATERAL` is left unconsumed and the
        // trailing clause is a clean parse error — the over-acceptance guard.
        assert!(
            parse_with(
                "SELECT a FROM t LATERAL VIEW explode(col) v",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err(),
            "ANSI rejects the LATERAL VIEW clause",
        );
        // Hive/Spark attach lateral views inside the FROM clause, so a FROM-less body
        // never reads them even with the gate on.
        assert!(
            parse_with(
                "SELECT 1 LATERAL VIEW explode(col) v",
                crate::ParseConfig::new(LATERAL_VIEW_DIALECT)
            )
            .is_err(),
            "a FROM-less body rejects the clause",
        );
    }

    #[test]
    fn lateral_view_and_lateral_factor_partition_on_position_and_follow_token() {
        use crate::render::Renderer;

        // Under the Lenient combination both `LATERAL` gates are on. At a table-factor
        // head, `LATERAL` (no `VIEW` follow) is the derived-table/function factor…
        let factor = parse_with(
            "SELECT a FROM t, LATERAL f(t.x) v",
            crate::ParseConfig::new(LATERAL_BOTH_DIALECT),
        )
        .expect("the lateral function factor parses");
        let factor_select = select_of(&factor);
        assert_eq!(factor_select.from.len(), 2);
        assert!(
            matches!(
                factor_select.from[1].relation,
                TableFactor::Function { lateral: true, .. }
            ),
            "the factor grammar claims the factor-head LATERAL",
        );
        assert!(factor_select.lateral_views.is_empty());

        // …after the complete FROM list, `LATERAL VIEW` is the view clause…
        let clause = parse_with(
            "SELECT a FROM t LATERAL VIEW explode(col) v AS c",
            crate::ParseConfig::new(LATERAL_BOTH_DIALECT),
        )
        .expect("the view clause parses with both gates on");
        assert_eq!(select_of(&clause).lateral_views.len(), 1);

        // …and both compose on one query, each claiming its own `LATERAL`.
        let both = parse_with(
            "SELECT a FROM t, LATERAL f(t.x) v LATERAL VIEW explode(v.col) w AS c",
            crate::ParseConfig::new(LATERAL_BOTH_DIALECT),
        )
        .expect("factor + clause compose");
        let both_select = select_of(&both);
        assert!(matches!(
            both_select.from[1].relation,
            TableFactor::Function { lateral: true, .. }
        ));
        assert_eq!(both_select.lateral_views.len(), 1);
        assert_eq!(
            Renderer::new(LATERAL_BOTH_DIALECT)
                .render_parsed(&both)
                .expect("renders"),
            // The function factor canonicalizes its bare alias to `AS`; the view
            // clause's alias position has no `AS` in either engine grammar.
            "SELECT a FROM t, LATERAL f(t.x) AS v LATERAL VIEW explode(v.col) w AS c",
        );

        // A `LATERAL VIEW` head at a comma-item position is invalid in every engine
        // (Hive/Spark attach views to a preceding source, never after `,`), and the
        // factor grammar's reject keeps it a clean error rather than a silent misread.
        assert!(
            parse_with(
                "SELECT a FROM t, LATERAL VIEW explode(col) v",
                crate::ParseConfig::new(LATERAL_BOTH_DIALECT),
            )
            .is_err(),
            "a comma-position LATERAL VIEW rejects",
        );
    }

    // --- Oracle/Snowflake CONNECT BY / START WITH (planner-parity-select-connect-by) ---

    /// ANSI plus the `connect_by_clause` flag alone, isolating the hierarchical query
    /// clause gate from the rest of the (feature-gated) Snowflake preset. Renders for the
    /// exact-text round-trip checks.
    const CONNECT_BY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                connect_by_clause: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn connect_by_parses_and_round_trips_under_the_gate() {
        use crate::render::Renderer;

        for sql in [
            // CONNECT BY alone; PRIOR on either side of the equality.
            "SELECT a FROM t CONNECT BY PRIOR id = pid",
            "SELECT a FROM t CONNECT BY id = PRIOR pid",
            // START WITH leads (Snowflake's documented order).
            "SELECT a FROM t START WITH pid IS NULL CONNECT BY PRIOR id = pid",
            // START WITH trails (Oracle's other legal order) — the order round-trips.
            "SELECT a FROM t CONNECT BY PRIOR id = pid START WITH pid IS NULL",
            // NOCYCLE (Oracle; the documented over-acceptance under the one gate).
            "SELECT a FROM t CONNECT BY NOCYCLE PRIOR id = pid",
            "SELECT a FROM t START WITH id = 1 CONNECT BY NOCYCLE PRIOR id = pid",
            // Multi-conjunct condition; PRIOR is an ordinary prefix operator in it.
            "SELECT a FROM t CONNECT BY PRIOR id = pid AND lvl < 5",
            // Position: after WHERE and before GROUP BY.
            "SELECT a FROM t WHERE a > 0 CONNECT BY PRIOR id = pid",
            "SELECT dept, COUNT(*) FROM t CONNECT BY PRIOR id = pid GROUP BY dept",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(CONNECT_BY_DIALECT))
                .unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
            assert!(
                select_of(&parsed).connect_by.is_some(),
                "{sql:?} populates connect_by",
            );
            assert_eq!(
                Renderer::new(CONNECT_BY_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
                "{sql:?} round-trips",
            );
        }

        // Structural check: START WITH, NOCYCLE, the order tag, and the PRIOR operator
        // land as typed fields.
        let parsed = parse_with(
            "SELECT a FROM t START WITH pid IS NULL CONNECT BY NOCYCLE PRIOR id = pid",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("parses");
        let clause = select_of(&parsed)
            .connect_by
            .as_ref()
            .expect("connect_by is set");
        assert!(clause.start_with.is_some());
        assert!(clause.nocycle);
        assert!(clause.start_with_leads);
        assert!(matches!(
            clause.connect_by,
            Expr::BinaryOp { ref left, .. }
                if matches!(**left, Expr::UnaryOp { op: crate::ast::UnaryOperator::Prior, .. })
        ));
    }

    #[test]
    fn connect_by_start_with_order_is_preserved() {
        use crate::render::Renderer;

        // Both legal orders parse to distinct order tags and each re-renders in its own
        // written order (the spelling-fidelity round-trip).
        let leads = parse_with(
            "SELECT a FROM t START WITH id = 1 CONNECT BY PRIOR id = pid",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("START WITH first parses");
        assert!(
            select_of(&leads)
                .connect_by
                .as_ref()
                .unwrap()
                .start_with_leads
        );

        let trails = parse_with(
            "SELECT a FROM t CONNECT BY PRIOR id = pid START WITH id = 1",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("CONNECT BY first parses");
        assert!(
            !select_of(&trails)
                .connect_by
                .as_ref()
                .unwrap()
                .start_with_leads
        );

        for (parsed, expected) in [
            (
                &leads,
                "SELECT a FROM t START WITH id = 1 CONNECT BY PRIOR id = pid",
            ),
            (
                &trails,
                "SELECT a FROM t CONNECT BY PRIOR id = pid START WITH id = 1",
            ),
        ] {
            assert_eq!(
                Renderer::new(CONNECT_BY_DIALECT)
                    .render_parsed(parsed)
                    .expect("renders"),
                expected,
            );
        }
    }

    #[test]
    fn prior_binds_tighter_than_comparison() {
        // `PRIOR a = b` groups as `(PRIOR a) = b` (PRIOR at the unary-sign rank), so the
        // fully-parenthesized render brackets the operator, never the whole equality.
        use crate::render::{RenderConfig, RenderMode, Renderer};
        let parsed = parse_with(
            "SELECT a FROM t CONNECT BY PRIOR id = pid",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("parses");
        let renderer = Renderer::with_config(
            CONNECT_BY_DIALECT,
            RenderConfig {
                mode: RenderMode::Parenthesized,
                ..RenderConfig::default()
            },
        );
        assert_eq!(
            renderer.render_parsed(&parsed).expect("renders"),
            // Fully-parenthesized mode brackets every operator; `(PRIOR id)` nests inside
            // the equality, proving PRIOR binds tighter than `=`.
            "SELECT a FROM t CONNECT BY ((PRIOR id) = pid)",
        );
    }

    #[test]
    fn prior_is_scoped_to_the_connect_by_condition() {
        // Outside a CONNECT BY condition `PRIOR` stays an ordinary (non-reserved) column
        // name — in the projection, in START WITH, and inside a subquery of the condition.
        let projection = parse_with(
            "SELECT prior FROM t CONNECT BY PRIOR id = pid",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("bare `prior` in the projection is a column");
        assert!(select_of(&projection).connect_by.is_some());

        // START WITH does not recognize PRIOR — a bare `prior` there is a column.
        parse_with(
            "SELECT a FROM t START WITH prior = 1 CONNECT BY id = pid",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("bare `prior` in START WITH is a column");

        // A nested query inside the CONNECT BY condition resets the context, so its own
        // `prior` is a column, not the operator.
        parse_with(
            "SELECT a FROM t CONNECT BY id = (SELECT prior FROM u)",
            crate::ParseConfig::new(CONNECT_BY_DIALECT),
        )
        .expect("bare `prior` in a subquery of the condition is a column");
    }

    #[test]
    fn connect_by_is_rejected_without_the_gate() {
        // With the flag off (the stock ANSI TestDialect) the post-WHERE `CONNECT BY` is
        // left unconsumed and the trailing clause is a clean parse error — the
        // over-acceptance guard.
        assert!(
            parse_with(
                "SELECT a FROM t CONNECT BY PRIOR id = pid",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err(),
            "ANSI rejects the CONNECT BY clause",
        );
        assert!(
            parse_with(
                "SELECT a FROM t START WITH id = 1 CONNECT BY id = pid",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err(),
            "ANSI rejects the START WITH clause",
        );
    }

    // --- DuckDB FROM-first SELECT (duckdb-from-first-select) ------------------

    #[test]
    fn from_first_tags_the_spelling_and_keeps_the_canonical_body() {
        // `FROM t SELECT a` is the ordinary Select body written FROM-first: the projection
        // and FROM populate exactly as for `SELECT a FROM t`; only the surface tag differs.
        let from_first = parse_with(
            "FROM t SELECT a",
            crate::ParseConfig::new(FROM_FIRST_DIALECT),
        )
        .expect("FROM-first parses");
        let ff = select_of(&from_first);
        assert_eq!(ff.spelling, SelectSpelling::FromFirst);
        assert_eq!(ff.projection.len(), 1);
        assert!(matches!(ff.projection[0], SelectItem::Expr { .. }));
        assert_eq!(ff.from.len(), 1);
        assert!(ff.into.is_none() && !ff.straight_join);

        let select_first = parse_with(
            "SELECT a FROM t",
            crate::ParseConfig::new(FROM_FIRST_DIALECT),
        )
        .expect("SELECT-first parses");
        let sf = select_of(&select_first);
        assert_eq!(sf.spelling, SelectSpelling::Select);
        assert_eq!(ff.projection.len(), sf.projection.len());
        assert_eq!(ff.from.len(), sf.from.len());
    }

    #[test]
    fn bare_from_is_an_implicit_wildcard() {
        // Bare `FROM t` (no SELECT) canonicalizes to `SELECT * FROM t` plus the tag.
        let parsed = parse_with("FROM t", crate::ParseConfig::new(FROM_FIRST_DIALECT))
            .expect("bare FROM parses");
        let select = select_of(&parsed);
        assert_eq!(select.spelling, SelectSpelling::FromFirst);
        assert!(matches!(
            select.projection.as_slice(),
            [SelectItem::Wildcard { .. }]
        ));
        assert_eq!(select.from.len(), 1);
        assert!(select.selection.is_none() && select.distinct.is_none());
    }

    #[test]
    fn from_first_composes_the_full_tail() {
        // The projection sits right after FROM; WHERE/GROUP BY/HAVING then parse in
        // ordinary order after it.
        let parsed = parse_with(
            "FROM t SELECT a, sum(b) WHERE a > 1 GROUP BY a HAVING sum(b) > 2",
            crate::ParseConfig::new(FROM_FIRST_DIALECT),
        )
        .expect("FROM-first with a full tail parses");
        let select = select_of(&parsed);
        assert_eq!(select.spelling, SelectSpelling::FromFirst);
        assert!(select.selection.is_some(), "WHERE parsed");
        assert_eq!(select.group_by.len(), 1, "GROUP BY key parsed");
        assert!(select.having.is_some(), "HAVING parsed");

        // GROUP BY ALL co-occurs with FROM-first heavily in the corpus.
        let by_all = parse_with(
            "FROM t GROUP BY ALL",
            crate::ParseConfig::new(FROM_FIRST_DIALECT),
        )
        .expect("FROM t GROUP BY ALL parses");
        let by_all = select_of(&by_all);
        assert!(by_all.group_by_all.is_some() && by_all.group_by.is_empty());
    }

    #[test]
    fn from_first_rejected_when_flag_off() {
        use crate::dialect::{Ansi, MySql, Postgres, Sqlite};
        // A statement-position FROM must stay a clean parse error everywhere the gate is
        // off — the over-acceptance guard the differential oracle relies on.
        for sql in ["FROM t SELECT a", "FROM t", "FROM t WHERE a > 1"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "Ansi rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "Postgres rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite rejects {sql:?}"
            );
        }
    }

    #[test]
    fn from_first_projection_binds_only_immediately_after_from() {
        // DuckDB parses a projection only immediately after the FROM clause; a `SELECT`
        // that trails `WHERE`/`GROUP BY` does not join the FROM-first body (`FROM t WHERE x
        // SELECT y` / `FROM t GROUP BY a SELECT a` are single-statement syntax errors;
        // probed on 1.5.4). Separator-less, these now reject on our side too — the top-level
        // statement list is `;`-delimited (pg-do-statement-separator-divergence), so a trailing
        // `SELECT` with no `;` is a syntax error rather than being silently split off, matching
        // DuckDB's single-statement rejection.
        for sql in ["FROM t WHERE a > 1 SELECT a", "FROM t GROUP BY a SELECT a"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(FROM_FIRST_DIALECT)).is_err(),
                "separator-less juxtaposed statements reject in {sql:?}",
            );
        }
        // The projection-binding property, verified on the `;`-separated form: our from-first
        // body never binds a post-tail `SELECT` — the leading `FROM t` keeps its implicit `*`,
        // the tail consumes `WHERE`/`GROUP BY`, and the `SELECT` is the next statement rather
        // than the projection, so the FROM-first select still carries the wildcard, not `a`.
        for sql in [
            "FROM t WHERE a > 1; SELECT a",
            "FROM t GROUP BY a; SELECT a",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(FROM_FIRST_DIALECT))
                .expect("`;`-separated statements parse");
            let first = select_of(&parsed);
            assert_eq!(first.spelling, SelectSpelling::FromFirst);
            assert!(
                matches!(first.projection.as_slice(), [SelectItem::Wildcard { .. }]),
                "the trailing SELECT must not bind as the FROM-first projection in {sql:?}",
            );
            assert_eq!(
                parsed.statements().len(),
                2,
                "the trailing SELECT splits into its own statement in {sql:?}",
            );
        }
    }

    #[test]
    fn from_first_round_trips_byte_identically() {
        use crate::render::Renderer;
        for sql in [
            "FROM t",
            "FROM t SELECT a",
            "FROM t SELECT a, b",
            "FROM t SELECT DISTINCT a",
            "FROM t SELECT a WHERE a > 1 GROUP BY a HAVING a > 2",
            "FROM t GROUP BY ALL",
            "FROM t ORDER BY a",
            "FROM a SELECT x UNION FROM b SELECT y",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(FROM_FIRST_DIALECT))
                .expect("FROM-first parses");
            assert_eq!(
                Renderer::new(FROM_FIRST_DIALECT)
                    .render_parsed(&parsed)
                    .expect("FROM-first renders"),
                sql,
            );
        }
    }

    #[test]
    fn bare_star_from_first_normalizes_to_the_bare_form() {
        use crate::render::Renderer;
        // Explicit `FROM t SELECT *` carries the same shape as bare `FROM t` (a single
        // wildcard, no DISTINCT); the one canonical render is the bare form, so the
        // `SELECT *` normalizes away (ADR-0011, the single-tag-state decision).
        let parsed = parse_with(
            "FROM t SELECT *",
            crate::ParseConfig::new(FROM_FIRST_DIALECT),
        )
        .expect("FROM t SELECT * parses");
        assert_eq!(
            Renderer::new(FROM_FIRST_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "FROM t",
        );
    }

    #[test]
    fn from_first_composes_in_set_operations_and_subqueries() {
        // The FROM-first primary routes through the same query entry as SELECT, so it
        // composes as a set operand, a parenthesized operand, a scalar subquery, and a
        // CTE body — one gated choke point, every position for free.
        for sql in [
            "FROM a SELECT x UNION FROM b SELECT y",
            "(FROM a SELECT x) UNION SELECT y FROM b",
            "SELECT (FROM t SELECT max(a))",
            "WITH c AS (FROM t SELECT a) SELECT * FROM c",
            "WITH c AS (FROM t) FROM c",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(FROM_FIRST_DIALECT)).is_ok(),
                "{sql:?} parses"
            );
        }
    }

    /// ANSI plus the `wildcard_modifiers` flag alone, isolating the wildcard-tail gate
    /// from the rest of the DuckDb preset (the COLUMNS expression is the separate
    /// `columns_expression` gate, exercised with the preset in
    /// `crate::dialect::duckdb`).
    const STAR_MODIFIERS_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                wildcard_modifiers: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn wildcard_modifiers_ride_the_gate_not_the_preset() {
        // The flag alone (over ANSI) admits the tail on both the bare and the
        // qualified wildcard, and a plain `*` stays modifier-free (`options: None`).
        let parsed = parse_with(
            "SELECT * EXCLUDE (a) REPLACE (b + 1 AS b) RENAME (c AS d), t.* EXCLUDE (e), * FROM t",
            crate::ParseConfig::new(STAR_MODIFIERS_DIALECT),
        )
        .expect("the gated modifiers parse");
        let items = projection(&parsed);
        let SelectItem::Wildcard {
            options: Some(options),
            ..
        } = &items[0]
        else {
            panic!("expected the modifier-bearing wildcard");
        };
        assert_eq!(
            (
                options.exclude.len(),
                options.replace.len(),
                options.rename.len()
            ),
            (1, 1, 1),
        );
        assert!(matches!(
            &items[1],
            SelectItem::QualifiedWildcard {
                options: Some(_),
                ..
            }
        ));
        assert!(matches!(
            &items[2],
            SelectItem::Wildcard { options: None, .. }
        ));

        // The same text under plain ANSI leaves `EXCLUDE` unconsumed — a clean error.
        assert!(
            parse_with(
                "SELECT * EXCLUDE (a) FROM t",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err()
        );
    }

    #[test]
    fn wildcard_modifier_spans_cover_the_whole_item() {
        // The wildcard item's span stretches over its modifier tail (the meta is the
        // side-table key for diagnostics/slicing), and the options node anchors at the
        // star.
        let sql = "SELECT * EXCLUDE (a) FROM t";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(STAR_MODIFIERS_DIALECT)).expect("parses");
        let SelectItem::Wildcard {
            options: Some(options),
            meta,
            ..
        } = &projection(&parsed)[0]
        else {
            panic!("expected the modifier-bearing wildcard");
        };
        assert_eq!(
            meta.span,
            Span::new(7, 20),
            "item span covers `* EXCLUDE (a)`",
        );
        assert_eq!(options.meta.span, meta.span, "options anchor at the star");
    }
}
