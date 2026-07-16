// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DuckDB `PIVOT` / `UNPIVOT` grammar — both surfaces of each operator.
//!
//! Three grammar positions feed the two canonical nodes ([`Pivot`]/[`Unpivot`]):
//!
//! - the **leading-keyword statement** (`PIVOT t ON year USING sum(x) GROUP BY city
//!   ORDER BY city`, `UNPIVOT t ON a, b INTO NAME n VALUE v`), dispatched from
//!   [`super::query`]'s statement dispatcher like the utility statements — including
//!   the `WITH`-prefixed form (DuckDB attaches CTEs to the pivot statement directly)
//!   and the trailing `ORDER BY`/`LIMIT` modifiers (engine-verified on 1.5.4);
//! - the **table-factor suffix** (`FROM t PIVOT (sum(x) FOR year IN (2000))`,
//!   `FROM t UNPIVOT [INCLUDE NULLS] (v FOR n IN (a, b))`), applied by
//!   [`parse_pivot_suffixes`](Parser::parse_pivot_suffixes) after every base table
//!   factor — before joins, matching the engine (a trailing `JOIN` takes the pivoted
//!   factor as its left side), and looping because factors chain
//!   (`t PIVOT (…) UNPIVOT (…)`);
//! - the **parenthesized statement as a table factor** (`FROM (PIVOT t ON …)`,
//!   including an inner `WITH`), read by
//!   [`try_parenthesized_pivot_factor`](Parser::try_parenthesized_pivot_factor) from
//!   the parenthesized-factor grammar in [`super::from`] and kept as the
//!   statement-spelled core inside a [`TableFactor::Pivot`], so the parentheses
//!   rederive from the spelling at render time.
//!
//! Both operators are gated on
//! [`TableFactorSyntax::pivot`](crate::ast::dialect::TableExpressionSyntax) /
//! [`unpivot`](crate::ast::dialect::TableExpressionSyntax) (DuckDb + Lenient). On a
//! *bare* factor the suffix additionally relies on the DuckDb preset reserving
//! `PIVOT`/`UNPIVOT` (`duckdb_keywords()` class `reserved`): without the reservation
//! the word is swallowed as the factor's alias first — under Lenient (which keeps the
//! ANSI reserved model) the suffix is therefore reachable only after an explicit
//! alias, exactly the `ASOF` precedent.
//!
//! The statement `ON` grammar reuses the expression parser: `ON col IN (v, …)` parses
//! as an [`Expr::InList`] predicate and is unfolded here into the column + value list
//! the [`PivotColumn`] shape carries, so the pivot grammar never re-implements — or
//! fights — the `IN` predicate the Pratt core already owns.

use crate::ast::{
    AliasSpelling, BinaryOperator, Expr, Ident, Keyword, NullInclusion, Pivot, PivotColumn,
    PivotExpr, PivotSpelling, PivotValueSource, RowExpr, Span, Spanned, Statement, TableFactor,
    UnaryOperator, Unpivot, UnpivotColumn, UnpivotSpelling, With,
};
use crate::error::ParseResult;
use crate::tokenizer::Punctuation;
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;

impl<'a, D: Dialect> Parser<'a, D> {
    // ---- statement dispatch -------------------------------------------------------

    /// Parse a leading-keyword `PIVOT` statement into [`Statement::Pivot`], reached
    /// from the statement dispatcher (and, `WITH`-prefixed, from the `WITH`
    /// dispatcher) under [`TableFactorSyntax::pivot`](crate::ast::dialect::TableExpressionSyntax).
    pub(super) fn parse_pivot_statement(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        let pivot = self.parse_pivot_operator(start, with)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Pivot {
            pivot: Box::new(pivot),
            meta,
        })
    }

    /// Parse a leading-keyword `UNPIVOT` statement into [`Statement::Unpivot`] — the
    /// [`parse_pivot_statement`](Self::parse_pivot_statement) mirror.
    pub(super) fn parse_unpivot_statement(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        let unpivot = self.parse_unpivot_operator(start, with)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Unpivot {
            unpivot: Box::new(unpivot),
            meta,
        })
    }

    // ---- the statement-form cores -------------------------------------------------

    /// Parse the statement-spelled `PIVOT` operator, cursor on the `PIVOT` keyword:
    /// `PIVOT <source> [ON <col> [IN (…)], …] [USING <agg> [AS a], …] [GROUP BY …]
    /// [ORDER BY …] [LIMIT …]`. Every clause after the source is optional (DuckDB
    /// auto-detects the omitted ones at bind time), but their order is fixed.
    ///
    /// Shared by the leading-keyword statement, the parenthesized table factor, and the
    /// query-body position (`SetExpr::Pivot`, from [`super::query`]'s `parse_set_operand`).
    pub(super) fn parse_pivot_operator(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Pivot<D::Ext>> {
        self.expect_keyword(Keyword::Pivot)?;
        let source = Box::new(self.parse_table_factor()?);
        let pivot_on = if self.eat_keyword(Keyword::On)? {
            self.parse_comma_separated(Self::parse_pivot_on_entry)?
        } else {
            ThinVec::new()
        };
        let aggregates = if self.eat_keyword(Keyword::Using)? {
            self.parse_comma_separated(Self::parse_pivot_expr)?
        } else {
            ThinVec::new()
        };
        let group_by = self.parse_pivot_group_by()?;
        let (order_by, order_by_all) = self.parse_order_by_clause()?;
        let limit = self.parse_limit()?.map(Box::new);
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Pivot {
            source,
            aggregates,
            pivot_on,
            group_by,
            with,
            order_by,
            order_by_all,
            limit,
            // DuckDB's statement PIVOT has no `DEFAULT ON NULL` (Snowflake table-factor only).
            default_on_null: None,
            spelling: PivotSpelling::Statement,
            meta,
        })
    }

    /// Parse the statement-spelled `UNPIVOT` operator, cursor on the `UNPIVOT`
    /// keyword: `UNPIVOT <source> ON <cols> [INTO NAME <n> VALUE <v>, …] [ORDER BY …]
    /// [LIMIT …]`. Unlike `PIVOT`, the `ON` list is mandatory, and the statement form
    /// admits no `INCLUDE`/`EXCLUDE NULLS` marker (engine parse-rejects it; probed on
    /// 1.5.4), so [`Unpivot::null_inclusion`] is always `None` here.
    ///
    /// Shared with the query-body position ([`SetExpr::Unpivot`](crate::ast::SetExpr::Unpivot)) exactly as
    /// [`parse_pivot_operator`](Self::parse_pivot_operator) is.
    pub(super) fn parse_unpivot_operator(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Unpivot<D::Ext>> {
        self.expect_keyword(Keyword::Unpivot)?;
        let source = Box::new(self.parse_table_factor()?);
        self.expect_keyword(Keyword::On)?;
        let columns = self.parse_comma_separated(Self::parse_unpivot_column)?;
        let (name, value) = if self.eat_keyword(Keyword::Into)? {
            self.expect_keyword(Keyword::Name)?;
            let name = thin_vec![self.parse_ident()?];
            // `VALUE` and `VALUES` are engine synonyms here (probed on 1.5.4);
            // canonical render emits `VALUE`, the `TRUNCATE [TABLE]` noise-word
            // precedent. The list form pairs with multi-column ON groups
            // (`INTO NAME month VALUE sales, tickets`).
            if !self.eat_keyword(Keyword::Value)? {
                self.expect_keyword(Keyword::Values)?;
            }
            let value = self.parse_comma_separated(Self::parse_ident)?;
            (name, value)
        } else {
            (ThinVec::new(), ThinVec::new())
        };
        let (order_by, order_by_all) = self.parse_order_by_clause()?;
        let limit = self.parse_limit()?.map(Box::new);
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Unpivot {
            source,
            value,
            name,
            columns,
            // The statement form admits no `INCLUDE`/`EXCLUDE NULLS` marker (engine
            // parse-rejects it; probed on 1.5.4).
            null_inclusion: None,
            with,
            order_by,
            order_by_all,
            limit,
            spelling: UnpivotSpelling::Statement,
            meta,
        })
    }

    /// One statement `ON` entry: a full expression whose `IN (…)` predicate — the
    /// inline value list — is unfolded into the [`PivotColumn`] shape.
    ///
    /// The engine's entry production is narrower than a full expression
    /// (`b_expr`-like: the boolean/predicate operators belong to the entry list, not
    /// the entry), so the parsed column is validated against the probed reject
    /// classes ([`pivot_on_column_admissible`]) rather than accepted wholesale —
    /// `PIVOT t ON NULL`/`ON 'str'`/`ON (SELECT …)`/`ON NOT c`/`ON c IS NULL` are
    /// DuckDB syntax errors while `ON c || d`/`ON 1 + c`/`ON (a = b)` parse (probed
    /// on 1.5.4).
    fn parse_pivot_on_entry(&mut self) -> ParseResult<PivotColumn<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        let column = match expr {
            Expr::InList {
                expr,
                list,
                negated: false,
                ..
            } => {
                let values = list
                    .into_iter()
                    .map(|value| {
                        let meta = self.make_meta(value.span());
                        PivotExpr {
                            expr: value,
                            alias: None,
                            alias_spelling: AliasSpelling::As,
                            meta,
                        }
                    })
                    .collect();
                PivotColumn {
                    expr: *expr,
                    values,
                    enum_source: None,
                    value_source: None,
                    meta,
                }
            }
            expr => PivotColumn {
                expr,
                values: ThinVec::new(),
                enum_source: None,
                value_source: None,
                meta,
            },
        };
        if !pivot_on_column_admissible(&column.expr) {
            return Err(self.unexpected("a pivot column expression"));
        }
        Ok(column)
    }

    /// The optional `GROUP BY <expr>, …` clause shared by both pivot surfaces.
    fn parse_pivot_group_by(&mut self) -> ParseResult<ThinVec<Expr<D::Ext>>> {
        if self.eat_keyword(Keyword::Group)? {
            self.expect_keyword(Keyword::By)?;
            self.parse_comma_separated_exprs()
        } else {
            Ok(ThinVec::new())
        }
    }

    // ---- the table-factor suffixes ------------------------------------------------

    /// Apply every `PIVOT (…)` / `UNPIVOT (…)` suffix chained onto an already-parsed
    /// table factor. The `(`-lookahead (or `INCLUDE`/`EXCLUDE` for `UNPIVOT`) keeps
    /// the commit unambiguous: the parenthesized body is the only suffix surface, so
    /// a bare trailing `PIVOT` is left for the enclosing grammar to reject.
    pub(super) fn parse_pivot_suffixes(
        &mut self,
        start: Span,
        mut factor: TableFactor<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        loop {
            // Dual-require on every entry path: PIVOT is admitted when either the DuckDB-style
            // `pivot` gate or the BigQuery/Snowflake-style `pivot_value_sources` gate is on
            // (those presets keep `pivot: false` and use `pivot_value_sources` as the sole
            // primary). Same disjunction for UNPIVOT. Pipe paths must use this same check
            // — `pipe_syntax` alone must not admit the surface.
            if (self.features().table_factor_syntax.pivot
                || self.features().table_factor_syntax.pivot_value_sources)
                && self.peek_is_keyword(Keyword::Pivot)?
                && self.peek_nth_is_punct(1, Punctuation::LParen)?
            {
                factor = self.parse_pivot_table_factor(start, factor)?;
            } else if (self.features().table_factor_syntax.unpivot
                || self.features().table_factor_syntax.pivot_value_sources)
                && self.peek_is_keyword(Keyword::Unpivot)?
                && (self.peek_nth_is_punct(1, Punctuation::LParen)?
                    || self.peek_nth_is_keyword(1, Keyword::Include)?
                    || self.peek_nth_is_keyword(1, Keyword::Exclude)?)
            {
                factor = self.parse_unpivot_table_factor(start, factor)?;
            } else if self.features().table_factor_syntax.match_recognize
                && self.peek_is_keyword(Keyword::MatchRecognize)?
                && self.peek_nth_is_punct(1, Punctuation::LParen)?
            {
                // The SQL:2016 `<source> MATCH_RECOGNIZE (…)` row-pattern table factor —
                // another `FROM`-suffix operator that binds tighter than joins, so it
                // chains here alongside PIVOT/UNPIVOT (parsed in its own module).
                factor = self.parse_match_recognize_suffix(start, factor)?;
            } else {
                return Ok(factor);
            }
        }
    }

    /// Parse one `PIVOT (<aggs> FOR <col> IN (<values>) [GROUP BY …])` suffix onto
    /// `source`. Exactly one `FOR` column, and the aggregate list is mandatory —
    /// both engine-enforced (a bare `PIVOT (FOR …)` and a second `FOR` are DuckDB
    /// syntax errors; probed on 1.5.4) — so the grammar here is deliberately
    /// narrower than the statement form's.
    fn parse_pivot_table_factor(
        &mut self,
        start: Span,
        source: TableFactor<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::Pivot)?;
        self.expect_punct(Punctuation::LParen, "`(` after `PIVOT`")?;
        let aggregates = self.parse_comma_separated(Self::parse_pivot_expr)?;
        self.expect_keyword(Keyword::For)?;
        let mut pivot_on = thin_vec![self.parse_pivot_for_column()?];
        // Additional column heads are written bare — `FOR y IN (…) m IN (…)` — a
        // second `FOR` is an engine syntax error (both probed on 1.5.4); the `IN`
        // lookahead keeps a following `GROUP` out of head position. The bare-chained
        // multi-column form is DuckDB's alone: the standard PIVOT
        // (Snowflake/BigQuery/Oracle) has exactly one `FOR` column, so the chain is
        // gated on the DuckDB `pivot` flag and a standard-only pivot stops after the
        // first head (leaving a following `DEFAULT`/`)` to the tail).
        while self.features().table_factor_syntax.pivot
            && !self.peek_is_punct(Punctuation::RParen)?
            && !self.peek_is_keyword(Keyword::Group)?
            // A trailing `DEFAULT ON NULL` is the standard tail, not a bare column head
            // (`DEFAULT` is reserved and never a column name) — leave it to the tail.
            && !self.peek_is_keyword(Keyword::Default)?
        {
            pivot_on.push(self.parse_pivot_for_column()?);
        }
        let group_by = self.parse_pivot_group_by()?;
        let default_on_null = self.parse_pivot_default_on_null()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `PIVOT`")?;
        let span = start.union(self.preceding_span());
        let pivot_meta = self.make_meta(span);
        let pivot = Box::new(Pivot {
            source: Box::new(source),
            aggregates,
            pivot_on,
            group_by,
            with: None,
            order_by: ThinVec::new(),
            order_by_all: None,
            limit: None,
            default_on_null,
            spelling: PivotSpelling::TableFactor,
            meta: pivot_meta,
        });
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Pivot { pivot, alias, meta })
    }

    /// Parse one `UNPIVOT [INCLUDE|EXCLUDE NULLS] (<value> FOR <name> IN (<cols>))`
    /// suffix onto `source`. The `NULLS` marker is this surface's alone (the
    /// statement form parse-rejects it); the name column is always a single
    /// identifier, while the value side may be a parenthesized list for the
    /// multi-column unpivot. This is the shared DuckDB/BigQuery/Snowflake table
    /// factor — reachable both under the DuckDB
    /// [`unpivot`](crate::ast::dialect::TableFactorSyntax) flag and, off it, under
    /// [`pivot_value_sources`](crate::ast::dialect::TableFactorSyntax) (the standard
    /// reachability gate the PIVOT table factor rides; PIVOT/UNPIVOT co-travel in these
    /// engines' grammars).
    fn parse_unpivot_table_factor(
        &mut self,
        start: Span,
        source: TableFactor<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::Unpivot)?;
        // Three states so a written marker round-trips: `INCLUDE NULLS`, an explicit
        // `EXCLUDE NULLS`, or the unwritten default (`None`; `EXCLUDE NULLS` semantics).
        let null_inclusion = if self.eat_keyword(Keyword::Include)? {
            self.expect_keyword(Keyword::Nulls)?;
            Some(NullInclusion::IncludeNulls)
        } else if self.eat_keyword(Keyword::Exclude)? {
            self.expect_keyword(Keyword::Nulls)?;
            Some(NullInclusion::ExcludeNulls)
        } else {
            None
        };
        self.expect_punct(Punctuation::LParen, "`(` after `UNPIVOT`")?;
        let value = self.parse_unpivot_name_list()?;
        self.expect_keyword(Keyword::For)?;
        let name = thin_vec![self.parse_ident()?];
        self.expect_keyword(Keyword::In)?;
        self.expect_punct(Punctuation::LParen, "`(` after `IN`")?;
        let columns = self.parse_comma_separated(Self::parse_unpivot_column)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the UNPIVOT column list")?;
        self.expect_punct(Punctuation::RParen, "`)` to close `UNPIVOT`")?;
        let span = start.union(self.preceding_span());
        let unpivot_meta = self.make_meta(span);
        let unpivot = Box::new(Unpivot {
            source: Box::new(source),
            value,
            name,
            columns,
            null_inclusion,
            with: None,
            order_by: ThinVec::new(),
            order_by_all: None,
            limit: None,
            spelling: UnpivotSpelling::TableFactor,
            meta: unpivot_meta,
        });
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Unpivot {
            unpivot,
            alias,
            meta,
        })
    }

    /// The table factor's `FOR <col> IN (<values>)` clause: a (possibly qualified)
    /// column name — or a parenthesized name row for the multi-column pivot
    /// (`FOR (a, b) IN ((1, 2))`) — followed by the mandatory value list. Parsed as
    /// names rather than expressions because `IN` must terminate the column here
    /// (the full expression grammar would swallow it as a predicate), matching the
    /// engine, whose `FOR` side admits only column references.
    pub(super) fn parse_pivot_for_column(&mut self) -> ParseResult<PivotColumn<D::Ext>> {
        let start = self.current_span()?;
        let expr = if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let fields = self.parse_comma_separated(|parser| {
                let field_start = parser.current_span()?;
                let name = parser.parse_object_name()?;
                let span = field_start.union(parser.preceding_span());
                let meta = parser.make_meta(span);
                Ok(Expr::Column { name, meta })
            })?;
            self.expect_punct(Punctuation::RParen, "`)` to close the pivot column list")?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            Expr::Row {
                row: Box::new(RowExpr {
                    fields,
                    explicit: false,
                    meta: self.make_meta(span),
                }),
                meta,
            }
        } else {
            let name = self.parse_object_name()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            Expr::Column { name, meta }
        };
        self.expect_keyword(Keyword::In)?;
        // `IN (<values>)` is the written list; `IN <name>` names an ENUM type whose
        // labels supply the values (a single unqualified name — the engine rejects a
        // qualified one; probed on 1.5.4). Under
        // [`pivot_value_sources`](crate::ast::dialect::TableFactorSyntax) the
        // parenthesized form additionally admits the standard `IN (ANY [ORDER BY …])`
        // and `IN (<subquery>)` sources (Snowflake/BigQuery/Oracle).
        let (values, enum_source, value_source) = if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            if let Some(source) = self.try_parse_pivot_value_source()? {
                self.expect_punct(Punctuation::RParen, "`)` to close the pivot IN source")?;
                (ThinVec::new(), None, Some(Box::new(source)))
            } else {
                let values = self.parse_comma_separated(Self::parse_pivot_expr)?;
                self.expect_punct(Punctuation::RParen, "`)` to close the pivot IN list")?;
                (values, None, None)
            }
        } else {
            (ThinVec::new(), Some(self.parse_ident()?), None)
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(PivotColumn {
            expr,
            values,
            enum_source,
            value_source,
            meta,
        })
    }

    /// Read a standard PIVOT non-list value source at the cursor just inside the `IN (`
    /// — `ANY [ORDER BY <keys>]` or a `SELECT`/`WITH` subquery — returning `None` (cursor
    /// unmoved) when the gate is off or the content is an ordinary value list, so the
    /// caller reads the list instead. Only reachable under
    /// [`pivot_value_sources`](crate::ast::dialect::TableFactorSyntax); DuckDB's IN list
    /// and `IN <enum>` forms never see it.
    fn try_parse_pivot_value_source(&mut self) -> ParseResult<Option<PivotValueSource<D::Ext>>> {
        if !self.features().table_factor_syntax.pivot_value_sources {
            return Ok(None);
        }
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Any)? {
            // A bare `ANY` leaves the key list empty; `ANY ORDER BY <keys>` fills it.
            let order_by = self.parse_order_by()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(PivotValueSource::Any { order_by, meta }));
        }
        if self.peek_is_keyword(Keyword::Select)? || self.peek_is_keyword(Keyword::With)? {
            let query = Box::new(self.parse_query()?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(PivotValueSource::Subquery { query, meta }));
        }
        Ok(None)
    }

    /// Read the Snowflake table-factor `DEFAULT ON NULL (<expr>)` tail, cursor after the
    /// pivot column list — `None` when the gate is off or the clause is absent. Only
    /// reachable under [`pivot_value_sources`](crate::ast::dialect::TableFactorSyntax).
    fn parse_pivot_default_on_null(&mut self) -> ParseResult<Option<Box<Expr<D::Ext>>>> {
        if !self.features().table_factor_syntax.pivot_value_sources
            || !self.eat_keyword(Keyword::Default)?
        {
            return Ok(None);
        }
        self.expect_keyword(Keyword::On)?;
        self.expect_keyword(Keyword::Null)?;
        self.expect_punct(Punctuation::LParen, "`(` after `DEFAULT ON NULL`")?;
        let expr = self.parse_expr()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `DEFAULT ON NULL`")?;
        Ok(Some(Box::new(expr)))
    }

    /// An aliased pivot expression — a `USING` aggregate or an `IN`-list value:
    /// `<expr> [[AS] <alias>]`.
    pub(super) fn parse_pivot_expr(&mut self) -> ParseResult<PivotExpr<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        let (alias, alias_spelling) = self.parse_pivot_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(PivotExpr {
            expr,
            alias,
            alias_spelling,
            meta,
        })
    }

    /// One `UNPIVOT` column entry — a statement `ON` item or a table-factor `IN`
    /// entry: a column expression or a parenthesized group, with an optional alias
    /// (`(a, b) AS ab`). The entries are expressions rather than bare names so the
    /// `COLUMNS('re')` star-expansion node and DuckDB's bare `*` / `t.*` all-columns
    /// star (`ON *`, `ON * EXCLUDE (id)`, `IN (*)`; probed on 1.5.4) parse here exactly
    /// as they do in any other expression position.
    pub(super) fn parse_unpivot_column(&mut self) -> ParseResult<UnpivotColumn<D::Ext>> {
        let start = self.current_span()?;
        // Expression-first: a leading `(` is genuinely ambiguous between a column
        // group `(a, b)` and a parenthesized expression `(c + 100)::VARCHAR` (both
        // engine-accepted), and the expression grammar already resolves it — the
        // group parses as an implicit row, unfolded here into the column list. A
        // leading `*` / `t.*` is the bare-star all-columns form (`parse_star_or_expr`).
        let columns = match self.parse_star_or_expr()? {
            Expr::Row { row, .. } if !row.explicit => row.fields,
            expr => thin_vec![expr],
        };
        let (alias, alias_spelling) = self.parse_pivot_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(UnpivotColumn {
            columns,
            alias,
            alias_spelling,
            meta,
        })
    }

    /// The `UNPIVOT` table factor's value-name side: a single identifier or a
    /// parenthesized list (`(v1, v2)`) for the multi-column form.
    pub(super) fn parse_unpivot_name_list(&mut self) -> ParseResult<ThinVec<Ident>> {
        if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let names = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the value-name list")?;
            Ok(names)
        } else {
            Ok(thin_vec![self.parse_ident()?])
        }
    }

    /// An optional pivot alias: `AS <ident>`, `AS '<string>'`, or a bare ident.
    ///
    /// The string form is unconditional here — DuckDB admits it in every pivot alias
    /// position (`(Q1, Q2) AS 'sem1'`; probed on 1.5.4) — unlike the projection
    /// alias, whose string spelling is the MySQL-gated
    /// [`SelectSyntax::alias_string_literals`](crate::ast::dialect::SelectSyntax).
    /// Reuses that feature's [`parse_string_alias_ident`](Self::parse_string_alias_ident)
    /// so the `QuoteStyle::Single` round-trip machinery stays one implementation.
    fn parse_pivot_alias(&mut self) -> ParseResult<(Option<Ident>, AliasSpelling)> {
        if self.eat_keyword(Keyword::As)? {
            if let Some(ident) = self.parse_string_alias_ident()? {
                return Ok((Some(ident), AliasSpelling::As));
            }
            return Ok((Some(self.parse_as_alias_ident()?), AliasSpelling::As));
        }
        if self.peek_can_start_bare_alias()? {
            return Ok((Some(self.parse_bare_alias_ident()?), AliasSpelling::Bare));
        }
        Ok((None, AliasSpelling::As))
    }

    // ---- the parenthesized statement as a table factor ----------------------------

    /// Read a parenthesized statement-spelled pivot as a table factor —
    /// `FROM (PIVOT t ON …)`, optionally with an inner `WITH` — with the cursor just
    /// after the opening `(`. Returns `None` (cursor position unspecified; the caller
    /// rewinds) when the content is not a pivot statement, so the caller's original
    /// diagnosis stands.
    pub(super) fn try_parenthesized_pivot_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<Option<TableFactor<D::Ext>>> {
        let inner_start = self.current_span()?;
        let with = if self.peek_is_keyword(Keyword::With)? {
            match self.parse_with_clause() {
                Ok(with) => with,
                // A malformed WITH cannot introduce a pivot statement; report as a
                // non-match so the caller's (query-grammar) error is the one surfaced.
                Err(_) => return Ok(None),
            }
        } else {
            None
        };
        let is_pivot =
            self.features().table_factor_syntax.pivot && self.peek_is_keyword(Keyword::Pivot)?;
        let is_unpivot = self.features().table_factor_syntax.unpivot
            && self.peek_is_keyword(Keyword::Unpivot)?;
        if !is_pivot && !is_unpivot {
            return Ok(None);
        }
        if lateral {
            // The statement form has nothing for LATERAL to correlate against
            // (mirrors the plain-table reject).
            return Err(self.unexpected("a query after `LATERAL (`"));
        }
        let factor = if is_pivot {
            let pivot = Box::new(self.parse_pivot_operator(inner_start, with)?);
            self.expect_punct(Punctuation::RParen, "`)` to close the pivot statement")?;
            let alias = self.parse_optional_table_alias()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            TableFactor::Pivot { pivot, alias, meta }
        } else {
            let unpivot = Box::new(self.parse_unpivot_operator(inner_start, with)?);
            self.expect_punct(Punctuation::RParen, "`)` to close the unpivot statement")?;
            let alias = self.parse_optional_table_alias()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            TableFactor::Unpivot {
                unpivot,
                alias,
                meta,
            }
        };
        Ok(Some(factor))
    }
}

/// Whether a parsed statement-`ON` column is inside the engine's entry production —
/// a `b_expr`-like class whose primary excludes constants and subqueries.
///
/// Probe-fitted (DuckDB 1.5.4): a bare constant (`NULL`, `'str'`, `42`, `true`), a
/// scalar subquery/`EXISTS`, a boolean/predicate top (`NOT c`, `c IS NULL`,
/// `a AND b`, `c NOT IN (…)`, `BETWEEN`, `LIKE`) each syntax-error, while
/// operator/call/`CASE`/grouped forms (`c || d`, `1 + c`, `-c`, `lower(c)`,
/// `(a = b)`) parse — and so does `col IN (SELECT …)`, whose subquery is a value
/// *source* the entry grammar admits (the whole `InSubquery` stays the column
/// expression). The check is a top-node test: the boolean operators belong to the
/// *entry list* grammar there, not the entry, so they can only appear at the top of
/// an over-wide parse. `UNPIVOT` entries are deliberately unrestricted — the engine
/// parses `ON 'jan'`/`ON 42` and rejects them at bind time.
fn pivot_on_column_admissible<X: crate::ast::Extension>(expr: &Expr<X>) -> bool {
    !matches!(
        expr,
        Expr::Literal { .. }
            | Expr::Subquery { .. }
            | Expr::Exists { .. }
            | Expr::IsNull { .. }
            | Expr::IsTruth { .. }
            | Expr::IsNormalized { .. }
            | Expr::Between { .. }
            | Expr::Like { .. }
            | Expr::InList { .. }
            | Expr::UnaryOp {
                op: UnaryOperator::Not,
                ..
            }
            | Expr::BinaryOp {
                op: BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::IsDistinctFrom(_)
                    | BinaryOperator::IsNotDistinctFrom(_),
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        Expr, NullInclusion, PivotSpelling, PivotValueSource, QuoteStyle, Resolver as _, SetExpr,
        Statement, TableFactor, UnpivotSpelling,
    };
    use crate::dialect::{BigQuery, DuckDb, Lenient, Snowflake};
    use crate::parser::{Parsed, parse_with};

    /// The pivot core of a single-query statement's first FROM table factor.
    fn factor_pivot(parsed: &Parsed) -> &crate::ast::Pivot<crate::ast::NoExt> {
        let TableFactor::Pivot { pivot, .. } = relation_of(parsed) else {
            panic!("expected a pivot table factor");
        };
        pivot
    }

    /// The unpivot core of a single-query statement's first FROM table factor.
    fn factor_unpivot(parsed: &Parsed) -> &crate::ast::Unpivot<crate::ast::NoExt> {
        let TableFactor::Unpivot { unpivot, .. } = relation_of(parsed) else {
            panic!("expected an unpivot table factor");
        };
        unpivot
    }

    /// The pivot core of a single [`Statement::Pivot`].
    fn pivot_of(parsed: &Parsed) -> &crate::ast::Pivot<crate::ast::NoExt> {
        let Statement::Pivot { pivot, .. } = &parsed.statements()[0] else {
            panic!("expected a PIVOT statement");
        };
        pivot
    }

    /// The first FROM relation of a single-query statement.
    fn relation_of(parsed: &Parsed) -> &TableFactor<crate::ast::NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        &select.from[0].relation
    }

    #[test]
    fn statement_pivot_unfolds_the_inline_in_list() {
        // `ON Year IN (2000, 2010)` parses through the expression grammar as an
        // `InList` predicate and unfolds into the column + value list; a bare `ON
        // Year` keeps an empty value list (bind-time auto-detection).
        let parsed = parse_with(
            "PIVOT Cities ON Year IN (2000, 2010), Country USING sum(Population) GROUP BY Name",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the statement form parses");
        let pivot = pivot_of(&parsed);
        assert!(matches!(pivot.spelling, PivotSpelling::Statement));
        assert_eq!(pivot.pivot_on.len(), 2);
        assert_eq!(pivot.pivot_on[0].values.len(), 2, "the IN list unfolds");
        assert!(matches!(pivot.pivot_on[0].expr, Expr::Column { .. }));
        assert!(pivot.pivot_on[1].values.is_empty(), "bare ON column");
        assert_eq!(pivot.aggregates.len(), 1);
        assert_eq!(pivot.group_by.len(), 1);
        assert!(pivot.with.is_none());
    }

    #[test]
    fn statement_pivot_carries_with_order_by_and_limit() {
        let parsed = parse_with(
            "WITH c AS (SELECT 1 AS x, 2 AS y) PIVOT c ON x USING sum(y) ORDER BY x LIMIT 3",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the WITH-prefixed statement parses");
        let pivot = pivot_of(&parsed);
        assert!(
            pivot.with.is_some(),
            "the WITH clause attaches to the pivot"
        );
        assert_eq!(pivot.order_by.len(), 1);
        assert!(pivot.limit.is_some());
    }

    #[test]
    fn statement_unpivot_captures_the_into_clause() {
        let parsed = parse_with(
            "UNPIVOT monthly_sales ON jan, feb, mar INTO NAME month VALUE sales",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the UNPIVOT statement parses");
        let Statement::Unpivot { unpivot, .. } = &parsed.statements()[0] else {
            panic!("expected an UNPIVOT statement");
        };
        assert!(matches!(unpivot.spelling, UnpivotSpelling::Statement));
        assert_eq!(unpivot.columns.len(), 3);
        assert_eq!(
            parsed.resolver().resolve(unpivot.name[0].sym),
            "month",
            "INTO NAME",
        );
        assert_eq!(
            parsed.resolver().resolve(unpivot.value[0].sym),
            "sales",
            "INTO … VALUE",
        );
        assert_eq!(
            unpivot.null_inclusion, None,
            "the statement form has no marker"
        );
    }

    #[test]
    fn table_factor_pivot_captures_aggregates_for_column_and_alias() {
        let parsed = parse_with(
            "SELECT * FROM Cities PIVOT (sum(Population) AS total, count(*) FOR Year \
             IN (2000 AS y2000, 2010) GROUP BY Country) AS p",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the table-factor form parses");
        let TableFactor::Pivot { pivot, alias, .. } = relation_of(&parsed) else {
            panic!("expected a pivot table factor");
        };
        assert!(matches!(pivot.spelling, PivotSpelling::TableFactor));
        assert!(matches!(*pivot.source, TableFactor::Table { .. }));
        assert_eq!(pivot.aggregates.len(), 2);
        assert!(pivot.aggregates[0].alias.is_some());
        assert_eq!(pivot.pivot_on.len(), 1, "exactly one FOR column");
        let values = &pivot.pivot_on[0].values;
        assert_eq!(values.len(), 2);
        assert!(values[0].alias.is_some(), "IN-value alias");
        assert_eq!(pivot.group_by.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias").name.sym),
            "p",
        );
    }

    #[test]
    fn table_factor_unpivot_captures_nulls_groups_and_string_alias() {
        let parsed = parse_with(
            "SELECT * FROM t UNPIVOT INCLUDE NULLS \
             ((v1, v2) FOR n IN ((a, b) AS 'g1', (c, d)))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the multi-column UNPIVOT parses");
        let TableFactor::Unpivot { unpivot, .. } = relation_of(&parsed) else {
            panic!("expected an unpivot table factor");
        };
        assert!(matches!(unpivot.spelling, UnpivotSpelling::TableFactor));
        assert_eq!(unpivot.null_inclusion, Some(NullInclusion::IncludeNulls));
        assert_eq!(unpivot.value.len(), 2, "multi-column value names");
        assert_eq!(unpivot.name.len(), 1);
        assert_eq!(unpivot.columns.len(), 2);
        assert_eq!(unpivot.columns[0].columns.len(), 2, "grouped entry");
        let alias = unpivot.columns[0].alias.as_ref().expect("group alias");
        assert_eq!(alias.quote, QuoteStyle::Single, "string alias spelling");
        assert_eq!(parsed.resolver().resolve(alias.sym), "g1");
    }

    #[test]
    fn duckdb_unpivot_exclude_nulls_round_trips() {
        // An explicit `EXCLUDE NULLS` is DuckDB's default semantically, but the written
        // marker is preserved (`Some(ExcludeNulls)`) so it round-trips rather than
        // eliding to the bare default; an omitted marker stays `None`.
        let written = parse_with(
            "SELECT * FROM t UNPIVOT EXCLUDE NULLS (v FOR n IN (a, b))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("explicit EXCLUDE NULLS parses");
        assert_eq!(
            factor_unpivot(&written).null_inclusion,
            Some(NullInclusion::ExcludeNulls),
        );
        let bare = parse_with(
            "SELECT * FROM t UNPIVOT (v FOR n IN (a, b))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the bare default parses");
        assert_eq!(factor_unpivot(&bare).null_inclusion, None);
        // Lenient is the render target and shares the surface; the explicit marker is
        // reproduced rather than elided to the bare default.
        let rendered = crate::render::Renderer::new(Lenient)
            .render_parsed(&written)
            .expect("renders");
        assert_eq!(
            rendered, "SELECT * FROM t UNPIVOT EXCLUDE NULLS (v FOR n IN (a, b))",
            "the explicit EXCLUDE NULLS round-trips",
        );
    }

    // ---- the standard UNPIVOT table factor (BigQuery/Snowflake) ---------------------
    //
    // The table-factor UNPIVOT grammar is fully shared with DuckDB; `pivot_value_sources`
    // only reaches it where the DuckDB `unpivot` flag is off. BigQuery/Snowflake
    // position-reserve `UNPIVOT` on the `ColId` axis (the PIVOT precedent above), so the
    // suffix fires on a bare factor as well as after an explicit alias; the tests here use
    // the explicit-alias form to pin the shared fields.

    #[test]
    fn bigquery_unpivot_reachable_with_shared_fields() {
        let parsed = parse_with(
            "SELECT * FROM sales AS s UNPIVOT INCLUDE NULLS (amount FOR quarter IN (q1 AS 'Q1', q2))",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("BigQuery reaches the standard UNPIVOT after an explicit alias");
        let unpivot = factor_unpivot(&parsed);
        assert!(matches!(unpivot.spelling, UnpivotSpelling::TableFactor));
        assert_eq!(unpivot.null_inclusion, Some(NullInclusion::IncludeNulls));
        assert_eq!(unpivot.value.len(), 1);
        assert_eq!(unpivot.name.len(), 1);
        assert_eq!(unpivot.columns.len(), 2);
        assert!(
            unpivot.columns[0].alias.is_some(),
            "the per-column alias is typed",
        );
    }

    #[test]
    fn snowflake_unpivot_reachable() {
        let parsed = parse_with(
            "SELECT * FROM sales AS s UNPIVOT (amount FOR quarter IN (q1, q2, q3, q4))",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake reaches the standard UNPIVOT after an explicit alias");
        let unpivot = factor_unpivot(&parsed);
        assert_eq!(unpivot.null_inclusion, None, "no marker written");
        assert_eq!(unpivot.columns.len(), 4);
    }

    #[test]
    fn standard_unpivot_round_trips_under_lenient() {
        // Lenient shares the gate and is the render target; each null-marker form
        // round-trips through parse -> render unchanged.
        for sql in [
            "SELECT * FROM sales AS s UNPIVOT (amount FOR quarter IN (q1, q2))",
            "SELECT * FROM sales AS s UNPIVOT INCLUDE NULLS (amount FOR quarter IN (q1, q2))",
            "SELECT * FROM sales AS s UNPIVOT EXCLUDE NULLS (amount FOR quarter IN (q1 AS a, q2))",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = crate::render::Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn standard_unpivot_rejected_off_the_gate() {
        use crate::dialect::Postgres;
        // Off the `unpivot`/`pivot_value_sources` gate PostgreSQL has no UNPIVOT table
        // factor, so the suffix is a clean parse divergence.
        assert!(
            parse_with(
                "SELECT * FROM sales AS s UNPIVOT (amount FOR quarter IN (q1, q2))",
                crate::ParseConfig::new(Postgres),
            )
            .is_err(),
            "PostgreSQL rejects the standard UNPIVOT table factor",
        );
    }

    #[test]
    fn pivot_suffixes_chain_and_bind_before_joins() {
        // `t PIVOT (…) UNPIVOT (…)` nests (the unpivot's source is the pivot), and a
        // trailing JOIN takes the whole pivoted factor as its left side —
        // engine-verified composition (DuckDB 1.5.4).
        let parsed = parse_with(
            "SELECT * FROM test PIVOT (sum(x) FOR y IN ('z')) UNPIVOT (x FOR y IN (z)) JOIN u ON true",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the chained suffixes parse");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        let from = &select.from[0];
        assert_eq!(from.joins.len(), 1, "the JOIN attaches outside the chain");
        let TableFactor::Unpivot { unpivot, .. } = &from.relation else {
            panic!("expected the unpivot as the outermost factor");
        };
        assert!(
            matches!(*unpivot.source, TableFactor::Pivot { .. }),
            "the pivot nests as the unpivot's source",
        );
    }

    #[test]
    fn parenthesized_statement_pivot_reads_as_a_factor() {
        // `FROM (PIVOT …)` keeps the statement-spelled core inside the factor (the
        // parentheses rederive from the spelling at render time), including the
        // inner-WITH form recovered by the ADR-0005 backtrack.
        let parsed = parse_with(
            "SELECT * FROM (PIVOT Cities ON Year USING sum(Population)) AS p",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the parenthesized statement parses");
        let TableFactor::Pivot { pivot, alias, .. } = relation_of(&parsed) else {
            panic!("expected a pivot table factor");
        };
        assert!(matches!(pivot.spelling, PivotSpelling::Statement));
        assert!(alias.is_some());

        let parsed = parse_with(
            "SELECT * FROM (WITH c AS (SELECT 1 AS a, 2 AS b) PIVOT c ON a USING sum(b))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the inner-WITH parenthesized statement parses");
        let TableFactor::Pivot { pivot, .. } = relation_of(&parsed) else {
            panic!("expected a pivot table factor");
        };
        assert!(pivot.with.is_some(), "the inner WITH attaches to the pivot");
    }

    #[test]
    fn lenient_reads_a_bare_factor_pivot_as_the_alias() {
        // LENIENT enables the grammar flags but keeps the ANSI reserved model, so on
        // a bare factor the alias reading wins and the `(…)` body fails as an alias
        // column list; after an explicit alias the word cannot alias, so the suffix
        // fires — the ASOF split, applied to PIVOT.
        assert!(
            parse_with(
                "SELECT * FROM t PIVOT (sum(x) FOR y IN (1))",
                crate::ParseConfig::new(Lenient),
            )
            .is_err(),
            "the alias reading swallows the bare-factor PIVOT under Lenient",
        );
        let parsed = parse_with(
            "SELECT * FROM t a PIVOT (sum(x) FOR y IN (1))",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses the suffix after an explicit alias");
        assert!(matches!(relation_of(&parsed), TableFactor::Pivot { .. }));
        // The statement form has no alias ambiguity, so it parses directly.
        assert!(
            parse_with(
                "PIVOT t ON y USING sum(x)",
                crate::ParseConfig::new(Lenient)
            )
            .is_ok()
        );
    }

    #[test]
    fn unpivot_on_columns_call_parses_as_the_columns_expression() {
        // `COLUMNS('re')` is the star-expression node (`duckdb-select-star-modifiers`)
        // under the DuckDb preset, and unpivot columns are expressions rather than
        // bare names — so the entry reads as `Expr::Columns`, exactly as it does in
        // any other expression position.
        let parsed = parse_with(
            "UNPIVOT monthly_sales ON COLUMNS('month_.*') INTO NAME month VALUE sales",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the COLUMNS spelling parses as the columns expression");
        let Statement::Unpivot { unpivot, .. } = &parsed.statements()[0] else {
            panic!("expected an UNPIVOT statement");
        };
        assert!(matches!(
            unpivot.columns[0].columns[0],
            Expr::Columns { .. },
        ));
    }

    // ---- PIVOT/UNPIVOT as a query body (`duckdb-statement-in-query-position`) --------

    #[test]
    fn pivot_as_a_cte_body_is_a_setexpr_pivot() {
        // DuckDB admits `PIVOT` as a CTE body (`A CTE needs a SELECT` is *not* raised
        // for pivot; probed on 1.5.4), so the CTE query's body is a `SetExpr::Pivot`
        // reusing the statement-spelled core — the query-body representation the
        // pivot ticket deferred.
        let parsed = parse_with(
            "WITH pivoted AS (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) \
             SELECT * FROM pivoted ORDER BY empid DESC",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("PIVOT as a CTE body parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let cte = &query.with.as_ref().expect("a WITH clause").ctes[0];
        let cte_query = cte.body.as_query().expect("a query CTE body");
        let SetExpr::Pivot { pivot, .. } = &cte_query.body else {
            panic!("expected the CTE body to be a SetExpr::Pivot");
        };
        assert!(matches!(pivot.spelling, PivotSpelling::Statement));
        assert!(
            pivot.with.is_none(),
            "the outer WITH is the CTE list, not the pivot's"
        );
        // The materialization hint rides the CTE, not the pivot body.
        let materialized = parse_with(
            "WITH p AS MATERIALIZED (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) \
             SELECT * FROM p",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("MATERIALIZED PIVOT CTE parses");
        let Statement::Query { query, .. } = &materialized.statements()[0] else {
            panic!("expected a query statement");
        };
        assert_eq!(
            query.with.as_ref().unwrap().ctes[0].materialized,
            Some(true)
        );
    }

    #[test]
    fn unpivot_as_a_cte_body_is_a_setexpr_unpivot() {
        let parsed = parse_with(
            "WITH u AS (UNPIVOT monthly_sales ON jan, feb INTO NAME month VALUE sales) SELECT * FROM u",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("UNPIVOT as a CTE body parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        assert!(matches!(
            query.with.as_ref().unwrap().ctes[0]
                .body
                .as_query()
                .expect("a query CTE body")
                .body,
            SetExpr::Unpivot { .. },
        ));
    }

    #[test]
    fn pivot_as_a_create_view_body_is_a_setexpr_pivot() {
        // `CREATE VIEW … AS PIVOT …` (with explicit `IN` values — DuckDB parse-rejects a
        // dynamic pivot in a view) puts the pivot at the view's query body.
        let parsed = parse_with(
            "CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH IN ('1-JAN', '2-FEB') \
             USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("CREATE VIEW AS PIVOT parses");
        let Statement::CreateView { view, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE VIEW statement");
        };
        assert!(matches!(view.query.body, SetExpr::Pivot { .. }));
    }

    #[test]
    fn pivot_as_a_create_table_as_body_is_a_setexpr_pivot() {
        let parsed = parse_with(
            "CREATE TABLE ct AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("CREATE TABLE AS PIVOT parses");
        let Statement::CreateTable { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE TABLE statement");
        };
        let crate::ast::CreateTableBody::AsQuery { query, .. } = &create.body else {
            panic!("expected a CTAS body");
        };
        assert!(matches!(query.body, SetExpr::Pivot { .. }));
    }

    #[test]
    fn query_body_pivot_round_trips() {
        // DuckDb is not itself a render target (see the `WITH ROLLUP` precedent in
        // `select.rs`); the round-trip renders under Lenient, the permissive superset
        // that also accepts the query-body pivot, proving the shape round-trips.
        for sql in [
            "WITH p AS (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) SELECT * FROM p",
            "CREATE VIEW v AS PIVOT t ON id IN ('a') USING SUM(feb)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = crate::render::Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn query_body_pivot_is_rejected_where_the_gate_is_off() {
        use crate::dialect::Postgres;
        // Off the `pivot`/`unpivot` gate the query body cannot be a pivot: PostgreSQL
        // sees `PIVOT` where a query is required and rejects it (a clean parse error).
        assert!(
            parse_with(
                "WITH p AS (PIVOT t ON x USING sum(y)) SELECT * FROM p",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL rejects PIVOT in a CTE body",
        );
        assert!(
            parse_with(
                "CREATE VIEW v AS PIVOT t ON x IN (1) USING sum(y)",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL rejects PIVOT as a view body",
        );
        // Lenient (which shares the gate) accepts it.
        assert!(
            parse_with(
                "WITH p AS (PIVOT t ON x USING sum(y)) SELECT * FROM p",
                crate::ParseConfig::new(Lenient)
            )
            .is_ok(),
            "Lenient accepts PIVOT in a CTE body",
        );
    }

    // ---- the standard PIVOT table factor (BigQuery/Snowflake/Oracle) ----------------
    //
    // BigQuery/Snowflake position-reserve `PIVOT`/`UNPIVOT` on the `ColId` axis (their
    // `*_RESERVED_COLUMN_NAME` sets — the `DUCKDB_PIVOT_RESERVATION` mechanism, minus the
    // function/type/projection axes those engines keep open), so a bare `FROM t PIVOT (…)`
    // reaches the operator directly — see `bigquery_pivot_reachable_on_a_bare_factor` below.
    // The explicit-alias form (`FROM t AS s PIVOT (…)`) stays reachable too; the tests here
    // exercise it to pin the shared value-source fields. Lenient is the render target and
    // keeps `PIVOT` unreserved (permissive), so under Lenient the suffix still needs an
    // explicit alias — the `pivot`/`ASOF` precedent.

    #[test]
    fn bigquery_pivot_captures_the_value_list_with_aliases() {
        // BigQuery's PIVOT uses only the explicit value list, with per-value and
        // aggregate aliases — the fields the standard shares with DuckDB.
        let parsed = parse_with(
            "SELECT * FROM sales AS s PIVOT (sum(amount) AS total FOR quarter IN ('Q1' AS q1, 'Q2'))",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("BigQuery parses the standard PIVOT after an explicit alias");
        let pivot = factor_pivot(&parsed);
        assert!(matches!(pivot.spelling, PivotSpelling::TableFactor));
        assert_eq!(pivot.aggregates.len(), 1);
        assert!(pivot.aggregates[0].alias.is_some(), "aggregate alias");
        assert_eq!(pivot.pivot_on.len(), 1, "the standard has one FOR column");
        let column = &pivot.pivot_on[0];
        assert_eq!(column.values.len(), 2);
        assert!(column.values[0].alias.is_some(), "per-value alias");
        assert!(
            column.value_source.is_none(),
            "an explicit list, not ANY/subquery"
        );
        assert!(pivot.default_on_null.is_none());
    }

    #[test]
    fn snowflake_pivot_captures_any_order_by_value_source() {
        let parsed = parse_with(
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN (ANY ORDER BY quarter))",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake parses `IN (ANY ORDER BY …)`");
        let pivot = factor_pivot(&parsed);
        let column = &pivot.pivot_on[0];
        assert!(column.values.is_empty(), "no explicit list");
        let Some(source) = &column.value_source else {
            panic!("expected an ANY value source");
        };
        let PivotValueSource::Any { order_by, .. } = source.as_ref() else {
            panic!("expected the ANY variant");
        };
        assert_eq!(order_by.len(), 1, "the ORDER BY key is typed");
    }

    #[test]
    fn snowflake_pivot_captures_subquery_value_source_and_default() {
        let parsed = parse_with(
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter \
             IN (SELECT DISTINCT quarter FROM q) DEFAULT ON NULL (0))",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake parses the subquery source and `DEFAULT ON NULL`");
        let pivot = factor_pivot(&parsed);
        let column = &pivot.pivot_on[0];
        let Some(source) = &column.value_source else {
            panic!("expected a subquery value source");
        };
        assert!(
            matches!(source.as_ref(), PivotValueSource::Subquery { .. }),
            "the subquery is a typed value source",
        );
        assert!(
            pivot.default_on_null.is_some(),
            "the DEFAULT ON NULL expression is typed",
        );
    }

    // ---- bare-factor reachability under BigQuery/Snowflake --------------------------
    //
    // `bigquery-snowflake-pivot-keyword-reservation`: the two presets position-reserve
    // `PIVOT`/`UNPIVOT` (and, for Snowflake, `MATCH_RECOGNIZE`) on the `ColId` axis, so a
    // *bare* factor no longer swallows the keyword as a correlation alias and the operator
    // is reachable without an explicit `AS` — the canonical Snowflake/BigQuery spelling.

    #[test]
    fn bigquery_pivot_reachable_on_a_bare_factor() {
        // The documented BigQuery form has no alias between the table and `PIVOT`.
        let parsed = parse_with(
            "SELECT * FROM sales PIVOT (sum(amount) FOR quarter IN ('Q1', 'Q2'))",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("BigQuery reaches the bare-factor PIVOT via the ColId reservation");
        let pivot = factor_pivot(&parsed);
        assert!(matches!(pivot.spelling, PivotSpelling::TableFactor));
        assert_eq!(pivot.pivot_on.len(), 1);
        assert_eq!(pivot.pivot_on[0].values.len(), 2);
    }

    #[test]
    fn bigquery_unpivot_reachable_on_a_bare_factor() {
        let parsed = parse_with(
            "SELECT * FROM sales UNPIVOT (amount FOR quarter IN (q1, q2))",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("BigQuery reaches the bare-factor UNPIVOT via the ColId reservation");
        assert!(matches!(
            factor_unpivot(&parsed).spelling,
            UnpivotSpelling::TableFactor
        ));
    }

    #[test]
    fn snowflake_pivot_and_unpivot_reachable_on_a_bare_factor() {
        let pivot = parse_with(
            "SELECT * FROM sales PIVOT (sum(amount) FOR quarter IN (ANY))",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake reaches the bare-factor PIVOT via the ColId reservation");
        assert!(matches!(
            factor_pivot(&pivot).spelling,
            PivotSpelling::TableFactor
        ));
        let unpivot = parse_with(
            "SELECT * FROM sales UNPIVOT (amount FOR quarter IN (q1, q2, q3, q4))",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake reaches the bare-factor UNPIVOT via the ColId reservation");
        assert!(matches!(
            factor_unpivot(&unpivot).spelling,
            UnpivotSpelling::TableFactor
        ));
    }

    #[test]
    fn pivot_reservation_is_confined_to_the_colid_axis() {
        // The reservation deliberately does not spill into the function/type/projection
        // bare-label axes: BigQuery/Snowflake do not class `pivot`/`unpivot` as reserved
        // keywords, so those positions keep parsing the bare word (faithfulness bound —
        // the direction opposite the ColId over-reservation below). The dialect presets are
        // distinct types, so each is exercised explicitly.
        for label in ["pivot(…) call", "projection bare label", "type name"] {
            let sql = match label {
                "pivot(…) call" => "SELECT pivot(x) FROM t",
                "projection bare label" => "SELECT 1 pivot FROM t",
                _ => "SELECT CAST(1 AS pivot) FROM t",
            };
            parse_with(sql, crate::ParseConfig::new(BigQuery))
                .unwrap_or_else(|e| panic!("BigQuery keeps `pivot` a {label}: {e:?}"));
            parse_with(sql, crate::ParseConfig::new(Snowflake))
                .unwrap_or_else(|e| panic!("Snowflake keeps `pivot` a {label}: {e:?}"));
        }
    }

    #[test]
    fn pivot_is_rejected_as_a_colid_the_reachability_cost() {
        // The unavoidable cost of the ColId reservation (shared with the DuckDB precedent):
        // an unquoted `pivot` can no longer be a column/table name or table alias — bare
        // *or* explicit-`AS` — under these presets. This over-reserves relative to the
        // engines (which keep the word non-reserved), and is the honest direction pinned by
        // the `bigquery-snowflake-pivot-keyword-reservation` ticket: quote it (`` `pivot` ``
        // / `"pivot"`) to use it as an identifier.
        for sql in [
            "SELECT * FROM t AS pivot", // explicit-AS table alias
            "SELECT * FROM t pivot",    // bare table alias
            "SELECT pivot FROM t",      // bare column reference
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(BigQuery)).is_err(),
                "BigQuery rejects the unquoted `pivot` ColId: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Snowflake)).is_err(),
                "Snowflake rejects the unquoted `pivot` ColId: {sql:?}",
            );
        }
        // The escape hatch: a quoted alias sidesteps the keyword classification.
        parse_with(
            "SELECT * FROM t AS `pivot`",
            crate::ParseConfig::new(BigQuery),
        )
        .expect("BigQuery admits a backtick-quoted alias named pivot");
        parse_with(
            "SELECT * FROM t AS \"pivot\"",
            crate::ParseConfig::new(Snowflake),
        )
        .expect("Snowflake admits a double-quoted alias named pivot");
    }

    #[test]
    fn standard_pivot_value_sources_round_trip_under_lenient() {
        // Lenient shares the `pivot_value_sources` gate and is the render target; each
        // form round-trips through parse -> render unchanged.
        for sql in [
            "SELECT * FROM sales AS s PIVOT (sum(amount) AS total FOR quarter IN ('Q1' AS q1, 'Q2'))",
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN (ANY ORDER BY quarter))",
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN (ANY))",
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN (SELECT quarter FROM q))",
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN ('Q1') DEFAULT ON NULL (0))",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = crate::render::Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn standard_pivot_value_sources_are_rejected_off_the_gate() {
        use crate::dialect::Postgres;
        // Off the `pivot`/`pivot_value_sources` gate PostgreSQL has no PIVOT table factor,
        // so the standard forms are a clean parse divergence rather than lost fields.
        for sql in [
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN (ANY))",
            "SELECT * FROM sales AS s PIVOT (sum(amount) FOR quarter IN ('Q1') DEFAULT ON NULL (0))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects the standard PIVOT: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_pivot_rejects_the_standard_value_sources() {
        // The DuckDB `pivot` flag is on but `pivot_value_sources` is off, so the standard
        // `ANY [ORDER BY …]` / subquery / `DEFAULT ON NULL` forms are not DuckDB grammar:
        // the `IN` list parses them as ordinary value expressions and rejects. DuckDB's
        // own shape (`IN (1, 2)`, `IN <enum>`) is untouched.
        for sql in [
            "SELECT * FROM t AS a PIVOT (sum(x) FOR y IN (ANY ORDER BY y))",
            "SELECT * FROM t AS a PIVOT (sum(x) FOR y IN (SELECT y FROM q))",
            "SELECT * FROM t AS a PIVOT (sum(x) FOR y IN (1) DEFAULT ON NULL (0))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB has no standard PIVOT value source: {sql:?}",
            );
        }
        // The DuckDB value-list form still parses, carrying no standard value source.
        let parsed = parse_with(
            "SELECT * FROM t AS a PIVOT (sum(x) FOR y IN (1, 2))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the DuckDB value list is untouched");
        assert!(factor_pivot(&parsed).pivot_on[0].value_source.is_none());
    }
}
