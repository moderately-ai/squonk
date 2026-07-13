// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DML statement grammar.
//!
//! This module owns production DML surfaces. It keeps `DEFAULT` in statement-only
//! value positions out of the general expression grammar by parsing row items and
//! assignment values through DML-specific nodes.

use crate::ast::{
    AliasSpelling, ConflictAction, ConflictResolution, ConflictTarget, DefaultValue, Delete,
    DmlSelection, DmlTarget, FetchSpelling, Ident, Insert, InsertColumnMatching, InsertOverriding,
    InsertSource, InsertTarget, InsertValue, InsertValues, InsertVerb, Keyword, Limit, LimitSyntax,
    Merge, MergeAction, MergeMatchKind, MergeWhenClause, Meta, ObjectName, OnConflict, OnlySyntax,
    OrderByExpr, RelationInheritance, Returning, SelectItem, Span, Spanned, Statement, TableAlias,
    Update, UpdateAssignment, UpdateTupleSource, UpdateValue, Upsert, With,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;

/// The MySQL single-table `[ORDER BY <keys>] [LIMIT <count>]` mutation tails, shared by
/// `UPDATE` and `DELETE` (see [`Parser::parse_mutation_tails`]).
type MutationTails<X> = (ThinVec<OrderByExpr<X>>, Option<Limit<X>>);

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse an `INSERT` statement, optionally after a statement-level `WITH`.
    pub(super) fn parse_insert_statement_with(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        // MySQL admits a leading `WITH` before SELECT/UPDATE/DELETE but not before INSERT
        // (its insert CTE rides the `INSERT … SELECT` source instead), so a `WITH … INSERT`
        // is a syntax error there. The bare `INSERT` (no leading CTE) is unaffected.
        if with.is_some() && !self.features().mutation_syntax.cte_before_insert {
            let span = self.current_span()?;
            return Err(self.error_at(
                span,
                "no `WITH` clause before `INSERT` (its CTE rides the `INSERT … SELECT` source)",
                self.span_text(span).to_owned(),
            ));
        }
        self.expect_keyword(Keyword::Insert)?;
        let or_action = self.parse_or_action()?;
        self.expect_keyword(Keyword::Into)?;
        let target = self.parse_insert_target()?;
        let column_matching = self.parse_insert_column_matching(&target)?;
        let overriding = self.parse_insert_overriding()?;
        let source = self.parse_insert_source()?;
        let row_alias = self.parse_insert_row_alias()?;
        let upsert = self.parse_upsert()?;
        let returning = self.parse_returning()?;
        self.finish_insert(
            start,
            InsertVerb::Insert,
            or_action,
            with,
            target,
            column_matching,
            overriding,
            source,
            row_alias,
            upsert,
            returning,
        )
    }

    /// DuckDB `BY NAME` / `BY POSITION` after the insert target (probed 1.5.4).
    /// Engine rejects `BY NAME` with an explicit column list — match that at parse.
    fn parse_insert_column_matching(
        &mut self,
        target: &InsertTarget,
    ) -> ParseResult<Option<InsertColumnMatching>> {
        if !self.features().mutation_syntax.insert_column_matching {
            return Ok(None);
        }
        if !self.eat_keyword(Keyword::By)? {
            return Ok(None);
        }
        let mode = if self.eat_contextual_keyword("NAME")? {
            InsertColumnMatching::ByName
        } else if self.eat_contextual_keyword("POSITION")? {
            InsertColumnMatching::ByPosition
        } else {
            return Err(self.unexpected("`NAME` or `POSITION` after `BY`"));
        };
        if matches!(mode, InsertColumnMatching::ByName) && !target.columns.is_empty() {
            return Err(self.unexpected(
                "INSERT BY NAME without an explicit column list (DuckDB forbids both)",
            ));
        }
        Ok(Some(mode))
    }

    /// Parse an optional SQLite `OR <action>` conflict-resolution prefix on the mutation
    /// verb — `INSERT OR {REPLACE | IGNORE | ABORT | FAIL | ROLLBACK}` and the same tail
    /// on `UPDATE` — gated by
    /// [`MutationSyntax::or_conflict_action`](crate::ast::dialect::MutationSyntax). When
    /// off, the `OR` after the verb is left unconsumed and surfaces as a parse error (the
    /// ANSI/PostgreSQL/MySQL reject path). The five actions are matched contextually by
    /// spelling — `FAIL` is not a reserved keyword, and after `OR` in this position the
    /// action word is unambiguous — so no dialect-specific keyword row is needed.
    fn parse_or_action(&mut self) -> ParseResult<Option<ConflictResolution>> {
        if !(self.features().mutation_syntax.or_conflict_action
            && self.peek_is_keyword(Keyword::Or)?)
        {
            return Ok(None);
        }
        self.expect_keyword(Keyword::Or)?;
        let action = if self.eat_contextual_keyword("ROLLBACK")? {
            ConflictResolution::Rollback
        } else if self.eat_contextual_keyword("ABORT")? {
            ConflictResolution::Abort
        } else if self.eat_contextual_keyword("FAIL")? {
            ConflictResolution::Fail
        } else if self.eat_contextual_keyword("IGNORE")? {
            ConflictResolution::Ignore
        } else if self.eat_contextual_keyword("REPLACE")? {
            ConflictResolution::Replace
        } else {
            return Err(
                self.unexpected("`ROLLBACK`, `ABORT`, `FAIL`, `IGNORE`, or `REPLACE` after `OR`")
            );
        };
        Ok(Some(action))
    }

    /// Parse an optional MySQL 8.0.19+ row alias — `AS <alias>[(<col>, ...)]` — sitting
    /// between the insert source and the `ON DUPLICATE KEY UPDATE` clause. Gated by the
    /// same dialect data as that upsert
    /// ([`MutationSyntax::on_duplicate_key_update`](crate::ast::dialect::MutationSyntax)):
    /// the alias is the input side of MySQL's upsert surface, and no shipped dialect
    /// admits one without the other. When off, `AS` is left unconsumed and surfaces as a
    /// parse error (the ANSI/PostgreSQL reject path). It reuses the shared
    /// [`TableAlias`] shape — a correlation name plus an optional column-alias list.
    fn parse_insert_row_alias(&mut self) -> ParseResult<Option<TableAlias>> {
        if !(self.features().mutation_syntax.on_duplicate_key_update
            && self.eat_keyword(Keyword::As)?)
        {
            return Ok(None);
        }
        let start = self.preceding_span();
        let name = self.parse_ident()?;
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_insert_columns()?
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(TableAlias {
            name,
            columns,
            spelling: AliasSpelling::As,
            meta,
        }))
    }

    /// Parse a MySQL `REPLACE [INTO] <table> ...` statement.
    ///
    /// `REPLACE` is a delete-then-insert upsert that shares INSERT's tail grammar — the
    /// target column list and the `VALUES` / `SET` / `SELECT` sources — so it folds
    /// onto the one [`Insert`] shape tagged [`InsertVerb::Replace`] rather than
    /// a forked node. It carries none of the `OVERRIDING` / upsert / `RETURNING` tails
    /// (`REPLACE` *is* the conflict resolution, so an `ON DUPLICATE KEY UPDATE` on it is
    /// rejected as leftover input), and `INTO` is optional (MySQL). The dispatch gate
    /// ([`MutationSyntax::replace_into`](crate::ast::dialect::MutationSyntax)) is already
    /// checked by [`parse_statement`](Self::parse_statement).
    pub(super) fn parse_replace_statement(
        &mut self,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_keyword(Keyword::Replace)?;
        self.eat_keyword(Keyword::Into)?;
        let target = self.parse_insert_target()?;
        let source = self.parse_insert_source()?;
        // `REPLACE` *is* the conflict resolution, so it never carries an `OR <action>`
        // prefix (that is the `INSERT` spelling's slot).
        self.finish_insert(
            start,
            InsertVerb::Replace,
            None,
            None,
            target,
            None, // column_matching
            None,
            source,
            None,
            None,
            None,
        )
    }

    /// Assemble an [`Insert`] statement from its already-parsed parts, sharing the
    /// column-list-with-`DEFAULT VALUES` rejection and the span/meta wiring between the
    /// `INSERT` and `REPLACE` spellings. The verb tag and the absent
    /// `OVERRIDING`/upsert/`RETURNING` tails on `REPLACE` are the caller's.
    #[allow(clippy::too_many_arguments)]
    fn finish_insert(
        &mut self,
        start: Span,
        verb: InsertVerb,
        or_action: Option<ConflictResolution>,
        with: Option<With<D::Ext>>,
        target: InsertTarget,
        column_matching: Option<InsertColumnMatching>,
        overriding: Option<InsertOverriding>,
        source: InsertSource<D::Ext>,
        row_alias: Option<TableAlias>,
        upsert: Option<Box<Upsert<D::Ext>>>,
        returning: Option<Returning<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        // PostgreSQL attaches a target column list only to a `VALUES`/query source; a
        // column list on `DEFAULT VALUES` is a syntax error there, because the list names
        // columns that `DEFAULT VALUES` supplies no values for. Reject the combination at
        // parse time to match PostgreSQL rather than accept a list bound to nothing.
        if !target.columns.is_empty() && matches!(source, InsertSource::DefaultValues { .. }) {
            return Err(self.error_at(
                source.span(),
                "`VALUES` or a query to populate the column list",
                "`DEFAULT VALUES`",
            ));
        }
        let span = start.union(self.preceding_span());
        let insert = Insert {
            verb,
            or_action,
            with,
            target,
            column_matching,
            overriding,
            source,
            row_alias,
            upsert,
            returning,
            meta: self.make_meta(span),
        };
        Ok(Statement::Insert {
            insert: Box::new(insert),
            meta: self.make_meta(span),
        })
    }

    fn parse_insert_target(&mut self) -> ParseResult<InsertTarget> {
        let start = self.current_span()?;
        let name = self.parse_target_relation_name()?;
        let alias = if self.eat_keyword(Keyword::As)? {
            Some(self.parse_ident()?)
        } else {
            None
        };
        // A `(` here is ambiguous with the parenthesized query source
        // (`INSERT INTO x (SELECT …)`, no column list): a column list is
        // `(ColId, …)`, which — unlike a query — never opens with `SELECT`,
        // `VALUES`, or `WITH`, so one token of lookahead past the `(`
        // disambiguates without speculative backtracking. A query source is
        // left for `parse_insert_source` to consume.
        let columns = if self.peek_is_punct(Punctuation::LParen)?
            && !(self.peek_nth_is_keyword(1, Keyword::Select)?
                || self.peek_nth_is_keyword(1, Keyword::Values)?
                || self.peek_nth_is_keyword(1, Keyword::With)?)
        {
            self.parse_parenthesized_insert_columns()?
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(InsertTarget {
            name,
            alias,
            // The insert-target alias is only read after an explicit `AS`
            // (`INSERT INTO t AS x`); the bare form is not part of the grammar here.
            alias_spelling: AliasSpelling::As,
            columns,
            meta,
        })
    }

    fn parse_parenthesized_insert_columns(&mut self) -> ParseResult<ThinVec<Ident>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the INSERT column list")?;
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the INSERT column list")?;
        Ok(columns)
    }

    fn parse_insert_overriding(&mut self) -> ParseResult<Option<InsertOverriding>> {
        if !self.eat_contextual_keyword("OVERRIDING")? {
            return Ok(None);
        }
        let overriding = if self.eat_contextual_keyword("SYSTEM")? {
            InsertOverriding::SystemValue
        } else if self.eat_contextual_keyword("USER")? {
            InsertOverriding::UserValue
        } else {
            return Err(self.unexpected("`SYSTEM` or `USER`"));
        };
        self.expect_contextual_keyword("VALUE")?;
        Ok(Some(overriding))
    }

    fn parse_insert_source(&mut self) -> ParseResult<InsertSource<D::Ext>> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let start = self.current_span()?;
            self.advance()?;
            let default = DefaultValue {
                meta: self.make_meta(start),
            };
            self.expect_keyword(Keyword::Values)?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(InsertSource::DefaultValues { default, meta });
        }
        if self.peek_is_keyword(Keyword::Values)? {
            let values = self.parse_insert_values()?;
            let meta = self.make_meta(values.span());
            return Ok(InsertSource::Values {
                values: Box::new(values),
                meta,
            });
        }
        // MySQL `INSERT`/`REPLACE ... SET <col> = <value> [, ...]`: an assignment-list
        // source equivalent to a single VALUES row. Gated by dialect data — when off,
        // `SET` is left unconsumed and surfaces as the error below (the ANSI/PostgreSQL
        // reject path). It reuses the `UPDATE ... SET` assignment grammar.
        if self.features().mutation_syntax.insert_set && self.peek_is_contextual_keyword("SET")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("SET")?;
            let assignments = self.parse_update_assignments()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(InsertSource::Set { assignments, meta });
        }
        // A leading `(` is a parenthesized query source (`INSERT INTO t (SELECT
        // …)`), PostgreSQL `SelectStmt: select_no_parens | select_with_parens` —
        // `parse_query` already resolves it recursively (a parenthesized
        // operand, a set operation over parenthesized operands, a parenthesized
        // `WITH`-query), producing the identical shape the unparenthesized
        // spelling would, so the parens are pure grouping with no residue.
        if self.peek_starts_query()? || self.peek_is_punct(Punctuation::LParen)? {
            let query = self.parse_query()?;
            let meta = self.make_meta(query.span());
            return Ok(InsertSource::Query {
                query: Box::new(query),
                meta,
            });
        }
        let expected = if self.features().mutation_syntax.insert_set {
            "`DEFAULT VALUES`, `VALUES`, `SET`, or a query"
        } else {
            "`DEFAULT VALUES`, `VALUES`, or a query"
        };
        Err(self.unexpected(expected))
    }

    fn parse_insert_values(&mut self) -> ParseResult<InsertValues<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Values)?;
        // DuckDB tolerates a trailing comma after the row list (`VALUES (1), (2),`); a row
        // always opens with `(`, so a post-comma non-`(` token is that trailing comma.
        let rows = self.parse_comma_separated_trailing(Self::parse_insert_values_row, |p| {
            Ok(!p.peek_is_punct(Punctuation::LParen)?)
        })?;
        let span = start.union(self.preceding_span());
        // DuckDB rejects a ragged `INSERT ... VALUES` at parse too (measured on 1.5.4), so
        // the shared equal-arity gate covers this position as well as the query body.
        self.reject_ragged_values_rows(&rows, span)?;
        let meta = self.make_meta(span);
        Ok(InsertValues { rows, meta })
    }

    fn parse_insert_values_row(&mut self) -> ParseResult<ThinVec<InsertValue<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the INSERT VALUES row")?;
        // DuckDB tolerates a trailing comma inside a row (`VALUES (1, 2,)`); the closer is
        // the row's `)`. The INSERT *column* list keeps no such tolerance (DuckDB rejects
        // `INSERT INTO t (a, b,) …`; measured on 1.5.4).
        let row = self.parse_comma_separated_trailing(Self::parse_insert_value, |p| {
            p.peek_is_punct(Punctuation::RParen)
        })?;
        self.expect_punct(Punctuation::RParen, "`)` to close the INSERT VALUES row")?;
        Ok(row)
    }

    fn parse_insert_value(&mut self) -> ParseResult<InsertValue<D::Ext>> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let (default, meta) = self.parse_default_value()?;
            Ok(InsertValue::Default { default, meta })
        } else {
            let expr = self.parse_expr()?;
            let meta = self.make_meta(expr.span());
            Ok(InsertValue::Expr { expr, meta })
        }
    }

    /// Parse a confirmed `DEFAULT` value keyword into its `DefaultValue` child node
    /// and the wrapping value node's `Meta`. The caller has peeked `DEFAULT`. Both
    /// nodes claim a distinct `NodeId` — the child first — so the two `make_meta`
    /// calls preserve node-id ordering across the three DML value positions.
    fn parse_default_value(&mut self) -> ParseResult<(DefaultValue, Meta)> {
        let token = self
            .advance()?
            .expect("peek_is_contextual_keyword confirmed DEFAULT is present");
        let default = DefaultValue {
            meta: self.make_meta(token.span),
        };
        let meta = self.make_meta(token.span);
        Ok((default, meta))
    }

    /// Parse an `UPDATE` statement, optionally after a statement-level `WITH`.
    pub(super) fn parse_update_statement_with(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("UPDATE")?;
        let or_action = self.parse_or_action()?;
        let target = self.parse_dml_target(&["SET"])?;
        self.expect_contextual_keyword("SET")?;
        let assignments = self.parse_update_assignments()?;
        // MySQL has no `UPDATE … FROM` (its multi-table update lists the tables in the
        // target): with the gate off the `FROM` keyword is left as leftover input, rejected.
        let from =
            if self.features().mutation_syntax.update_from && self.eat_keyword(Keyword::From)? {
                self.parse_table_references()?
            } else {
                ThinVec::new()
            };
        let selection = self.parse_dml_selection()?;
        let (order_by, limit) = self.parse_mutation_tails()?;
        let returning = self.parse_returning()?;

        let span = start.union(self.preceding_span());
        let update_meta = self.make_meta(span);
        let update = Update {
            with,
            or_action,
            target,
            assignments,
            from,
            selection,
            order_by,
            limit,
            returning,
            meta: update_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::Update {
            update: Box::new(update),
            meta,
        })
    }

    pub(super) fn parse_update_assignments(
        &mut self,
    ) -> ParseResult<ThinVec<UpdateAssignment<D::Ext>>> {
        let assignments = self.parse_comma_separated(Self::parse_update_assignment)?;
        Ok(assignments)
    }

    /// Parse one `SET` assignment: a single `<col> = <value>` or, where dialect
    /// data enables it, a multiple-column `( <col> [, ...] ) = <source>`.
    ///
    /// A leading `(` can only begin the tuple form: a single set-target is a
    /// `ColId` and never opens with a parenthesis, so the branch is unambiguous.
    fn parse_update_assignment(&mut self) -> ParseResult<UpdateAssignment<D::Ext>> {
        if self.features().mutation_syntax.multi_column_assignment
            && self.peek_is_punct(Punctuation::LParen)?
        {
            return self.parse_tuple_assignment();
        }
        let start = self.current_span()?;
        let target = self.parse_object_name()?;
        if !self.features().mutation_syntax.update_set_qualified_column && target.0.len() > 1 {
            return Err(self.unexpected(
                "a bare column name as the SET target (qualified names are not admitted under this dialect)",
            ));
        }
        self.expect_assignment_eq()?;
        let value = self.parse_update_value()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(UpdateAssignment::Single {
            target,
            value,
            meta,
        })
    }

    /// Parse a multiple-column assignment `( <col> [, ...] ) = <source>` (the
    /// leading `(` is the current token, confirmed by the caller).
    fn parse_tuple_assignment(&mut self) -> ParseResult<UpdateAssignment<D::Ext>> {
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the column list")?;
        let targets = self.parse_comma_separated(Self::parse_object_name)?;
        if !self.features().mutation_syntax.update_set_qualified_column
            && targets.iter().any(|name| name.0.len() > 1)
        {
            return Err(self.unexpected(
                "bare column names as SET targets (qualified names are not admitted under this dialect)",
            ));
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the column list")?;
        self.expect_assignment_eq()?;
        let source = self.parse_update_tuple_source()?;
        // DuckDB (and the measured over-accept pins) reject arity mismatch on a value-row
        // RHS: `SET (a,b,c) = (1,2)` is a Parser Error "expected 3 values, got 2" (1.5.4).
        // Subquery RHS arity is bind-time; only the explicit value-row form is checked here.
        if self.features().mutation_syntax.update_tuple_value_row_arity {
            if let UpdateTupleSource::Row { values, .. } = &source {
                if values.len() != targets.len() {
                    return Err(self.error_at(
                        source.span(),
                        format!(
                            "a value row with {} element(s) (matching the column list)",
                            targets.len()
                        ),
                        format!("{} value(s)", values.len()),
                    ));
                }
            }
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(UpdateAssignment::Tuple {
            targets,
            source,
            meta,
        })
    }

    /// Consume the `=` separating an assignment's targets from its value.
    fn expect_assignment_eq(&mut self) -> ParseResult<()> {
        if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
            Ok(())
        } else {
            Err(self.unexpected("`=` in an UPDATE assignment"))
        }
    }

    /// Parse the right-hand side of a single assignment: `DEFAULT` or an expression.
    fn parse_update_value(&mut self) -> ParseResult<UpdateValue<D::Ext>> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let (default, meta) = self.parse_default_value()?;
            Ok(UpdateValue::Default { default, meta })
        } else {
            let expr = self.parse_expr()?;
            let meta = self.make_meta(expr.span());
            Ok(UpdateValue::Expr { expr, meta })
        }
    }

    /// Parse the source of a multiple-column assignment: a bare `DEFAULT`, an
    /// explicit `ROW( ... )` or parenthesized value row, or a row subquery.
    ///
    /// The parenthesized form disambiguates a subquery from a value row by the same
    /// `peek_starts_query` rule the expression grammar uses, so `(SELECT ...)` and
    /// `(VALUES ...)` map onto the subquery arm while `(1, DEFAULT)` is a value row.
    fn parse_update_tuple_source(&mut self) -> ParseResult<UpdateTupleSource<D::Ext>> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let (default, meta) = self.parse_default_value()?;
            return Ok(UpdateTupleSource::Default { default, meta });
        }
        if self.peek_is_keyword(Keyword::Row)? && self.peek_nth_is_punct(1, Punctuation::LParen)? {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::Row)?;
            let values = self.parse_update_value_row()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(UpdateTupleSource::Row {
                explicit: true,
                values,
                meta,
            });
        }
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the assigned row")?;
        if self.peek_starts_query()? {
            let query = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the row subquery")?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(UpdateTupleSource::Subquery {
                query: Box::new(query),
                meta,
            });
        }
        let values = self.parse_update_value_list()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the assigned row")?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(UpdateTupleSource::Row {
            explicit: false,
            values,
            meta,
        })
    }

    /// Parse a parenthesized `( <value> [, ...] )` row for an explicit `ROW(...)`.
    fn parse_update_value_row(&mut self) -> ParseResult<ThinVec<UpdateValue<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` after `ROW`")?;
        let values = self.parse_update_value_list()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the row constructor")?;
        Ok(values)
    }

    /// Parse a non-empty comma-separated list of assigned values (`<expr> | DEFAULT`).
    fn parse_update_value_list(&mut self) -> ParseResult<ThinVec<UpdateValue<D::Ext>>> {
        let values = self.parse_comma_separated(Self::parse_update_value)?;
        Ok(values)
    }

    /// Parse a `DELETE` statement, optionally after a statement-level `WITH`.
    pub(super) fn parse_delete_statement_with(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("DELETE")?;
        self.expect_keyword(Keyword::From)?;
        let target = self.parse_dml_target(&["USING", "WHERE", "RETURNING"])?;
        let using =
            if self.features().mutation_syntax.delete_using && self.eat_keyword(Keyword::Using)? {
                self.parse_table_references()?
            } else {
                // Dialects without a multi-relation delete (SQLite) leave `USING` unconsumed,
                // so the trailing relation list surfaces as a clean parse error.
                ThinVec::new()
            };
        // MySQL's `DELETE FROM tbl … USING …` names bare delete targets: an alias on the
        // target is a syntax error there (while a plain single-table `DELETE FROM t AS e
        // WHERE …` — no `USING` — is fine), so reject an aliased target once `USING` is
        // present under the gate.
        if !using.is_empty()
            && target.alias.is_some()
            && !self.features().mutation_syntax.delete_using_target_alias
        {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "no alias on the delete target of a `DELETE … USING` statement",
                self.span_text(span).to_owned(),
            ));
        }
        let selection = self.parse_dml_selection()?;
        let (order_by, limit) = self.parse_mutation_tails()?;
        let returning = self.parse_returning()?;

        let span = start.union(self.preceding_span());
        let delete_meta = self.make_meta(span);
        let delete = Delete {
            with,
            target,
            using,
            selection,
            order_by,
            limit,
            returning,
            meta: delete_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::Delete {
            delete: Box::new(delete),
            meta,
        })
    }

    fn parse_dml_target(&mut self, alias_stops: &[&'static str]) -> ParseResult<DmlTarget> {
        let start = self.current_span()?;
        let (inheritance, name) = self.parse_dml_target_name()?;
        let (alias, alias_spelling) = self.parse_optional_dml_alias(alias_stops)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(DmlTarget {
            name,
            inheritance,
            alias,
            alias_spelling,
            meta,
        })
    }

    /// Parse an `UPDATE`/`DELETE` target relation with its PostgreSQL `relation_expr`
    /// inheritance marker: `ONLY name`, `ONLY ( name )`, `name`, or `name *`.
    ///
    /// `ONLY` is a reserved keyword, so it can never be the target name itself; a
    /// leading `ONLY` therefore unambiguously begins the suppression form. The
    /// trailing `*` descendant marker (mutually exclusive with `ONLY`) attaches to
    /// a plain name. Both are gated by the same dialect-data flag as in a `FROM`
    /// clause, so a dialect without PostgreSQL inheritance syntax rejects them here.
    fn parse_dml_target_name(&mut self) -> ParseResult<(RelationInheritance, ObjectName)> {
        if !self.peek_is_keyword(Keyword::Only)? {
            let name = self.parse_target_relation_name()?;
            let inheritance = self.parse_descendant_star()?;
            return Ok((inheritance, name));
        }
        if !self.features().table_expressions.only {
            return Err(
                self.unexpected("a target table supported by this dialect (without `ONLY`)")
            );
        }
        self.expect_keyword(Keyword::Only)?;
        if self.eat_punct(Punctuation::LParen)? {
            let name = self.parse_target_relation_name()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `ONLY` target")?;
            Ok((RelationInheritance::Only(OnlySyntax::Parenthesized), name))
        } else {
            Ok((
                RelationInheritance::Only(OnlySyntax::Bare),
                self.parse_target_relation_name()?,
            ))
        }
    }

    fn parse_optional_dml_alias(
        &mut self,
        stop_keywords: &[&'static str],
    ) -> ParseResult<(Option<Ident>, AliasSpelling)> {
        if self.eat_keyword(Keyword::As)? {
            return Ok((Some(self.parse_ident()?), AliasSpelling::As));
        }
        let Some(token) = self.peek()? else {
            return Ok((None, AliasSpelling::As));
        };
        // A DML correlation alias is a `ColId` (PostgreSQL's `relation_alias`).
        if !self.token_can_be_column_name(token) {
            return Ok((None, AliasSpelling::As));
        }
        if stop_keywords
            .iter()
            .any(|keyword| self.token_is_contextual_keyword(token, keyword))
        {
            return Ok((None, AliasSpelling::As));
        }
        Ok((Some(self.parse_ident()?), AliasSpelling::Bare))
    }

    /// Parse the optional `WHERE` filter shared by `UPDATE` and `DELETE`.
    ///
    /// `WHERE CURRENT OF <cursor>` is the positioned form: it is the *entire* filter
    /// (PostgreSQL does not let it combine with a condition), so the two arms are
    /// mutually exclusive. It is gated by dialect data — when off, `CURRENT` is left
    /// unconsumed and `parse_expr` rejects it, which is the ANSI reject path.
    fn parse_dml_selection(&mut self) -> ParseResult<Option<DmlSelection<D::Ext>>> {
        if !self.eat_keyword(Keyword::Where)? {
            return Ok(None);
        }
        let start = self.preceding_span();
        if self.features().mutation_syntax.where_current_of
            && self.peek_is_keyword(Keyword::Current)?
            && self.peek_nth_is_keyword(1, Keyword::Of)?
        {
            self.expect_keyword(Keyword::Current)?;
            self.expect_keyword(Keyword::Of)?;
            let cursor = self.parse_ident()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(DmlSelection::CurrentOf { cursor, meta }));
        }
        let condition = self.parse_expr()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(DmlSelection::Where { condition, meta }))
    }

    /// Parse the MySQL single-table `[ORDER BY <keys>] [LIMIT <count>]` row-limiting
    /// tails shared by `UPDATE` and `DELETE`, gated by
    /// [`MutationSyntax::update_delete_tails`](crate::ast::dialect::MutationSyntax). When
    /// off, the leading `ORDER BY`/`LIMIT` is left unconsumed and surfaces as a
    /// trailing-input parse error (the ANSI/PostgreSQL reject path). The `ORDER BY` keys
    /// reuse the shared [`parse_order_by`](Self::parse_order_by) grammar.
    fn parse_mutation_tails(&mut self) -> ParseResult<MutationTails<D::Ext>> {
        if !self.features().mutation_syntax.update_delete_tails {
            return Ok((ThinVec::new(), None));
        }
        let order_by = self.parse_order_by()?;
        let limit = self.parse_mutation_limit()?;
        Ok((order_by, limit))
    }

    /// Parse an optional MySQL mutation `LIMIT <count>` tail — a bare row count with no
    /// offset and no `LIMIT <offset>, <count>` comma form (those belong to `SELECT`), so
    /// it folds into the canonical [`Limit`] tagged [`LimitSyntax::LimitOffset`] with a
    /// `None` offset. The caller has already gated this on `update_delete_tails`.
    fn parse_mutation_limit(&mut self) -> ParseResult<Option<Limit<D::Ext>>> {
        if !self.peek_is_keyword(Keyword::Limit)? {
            return Ok(None);
        }
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Limit)?;
        let count = self.parse_expr()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Limit {
            limit: Some(count),
            offset: None,
            syntax: LimitSyntax::LimitOffset,
            with_ties: None,
            percent: None,
            fetch_spelling: FetchSpelling::FirstRows,
            meta,
        }))
    }

    /// Parse an optional `RETURNING <output> [, ...]` clause, gated by dialect data.
    ///
    /// `RETURNING` is a PostgreSQL mutation extension, not ANSI, so a dialect that
    /// does not enable it leaves the keyword unconsumed; the trailing clause then
    /// surfaces as leftover input and a parse error, which is the reject side of the
    /// acceptance criteria. The output list is a SELECT projection, so it reuses the
    /// projection-item grammar via [`parse_returning_item`](Self::parse_returning_item).
    fn parse_returning(&mut self) -> ParseResult<Option<Returning<D::Ext>>> {
        if !self.features().mutation_syntax.returning {
            return Ok(None);
        }
        if !self.eat_keyword(Keyword::Returning)? {
            return Ok(None);
        }
        let start = self.preceding_span();
        let items = self.parse_comma_separated(Self::parse_returning_item)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Returning { items, meta }))
    }

    /// Parse one `RETURNING` output item: `*`, `<name>.*`, or `<expr> [[AS] alias]`.
    ///
    /// This mirrors a SELECT projection item: a leading `*` is the wildcard, a dotted
    /// column name followed by `.*` is a qualified wildcard, and anything else is an
    /// expression with an optional alias.
    fn parse_returning_item(&mut self) -> ParseResult<SelectItem<D::Ext>> {
        if self.peek_is_op(Operator::Star)? {
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
        // Shares the SELECT projection tail: `<name>.*` qualified wildcard, a
        // value-position `(func()).*` composite star, or an aliased expression.
        self.parse_projection_value_item()
    }

    /// Parse an optional upsert clause: PostgreSQL `ON CONFLICT ...` or MySQL
    /// `ON DUPLICATE KEY UPDATE ...`, each gated by its own dialect-data flag.
    ///
    /// Both spellings open with `ON`, so the keyword *after* it disambiguates them
    /// without committing: `CONFLICT` selects the PostgreSQL clause, `DUPLICATE` the
    /// MySQL one. When the follower matches neither enabled form, `ON` is left
    /// unconsumed so the trailing clause surfaces as a parse error — the reject path
    /// ANSI (and a single-upsert dialect on the other spelling) takes.
    fn parse_upsert(&mut self) -> ParseResult<Option<Box<Upsert<D::Ext>>>> {
        if !self.peek_is_keyword(Keyword::On)? {
            return Ok(None);
        }
        if self.features().mutation_syntax.on_conflict
            && self.peek_nth_is_keyword(1, Keyword::Conflict)?
        {
            let conflict = self.parse_on_conflict()?;
            let meta = self.make_meta(conflict.meta.span);
            return Ok(Some(Box::new(Upsert::OnConflict { conflict, meta })));
        }
        if self.features().mutation_syntax.on_duplicate_key_update
            && self.peek_nth_is_contextual_keyword(1, "DUPLICATE")?
        {
            return Ok(Some(Box::new(self.parse_on_duplicate_key_update()?)));
        }
        Ok(None)
    }

    /// Parse the PostgreSQL `ON CONFLICT [<target>] DO {NOTHING | UPDATE ...}` clause
    /// (the leading `ON CONFLICT` confirmed by [`parse_upsert`](Self::parse_upsert)).
    fn parse_on_conflict(&mut self) -> ParseResult<OnConflict<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::On)?;
        self.expect_keyword(Keyword::Conflict)?;
        let target = self.parse_conflict_target()?;
        let action = self.parse_conflict_action()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(OnConflict {
            target,
            action,
            meta,
        })
    }

    /// Parse the MySQL `ON DUPLICATE KEY UPDATE <col> = <expr> [, ...]` clause (the
    /// leading `ON DUPLICATE` confirmed by [`parse_upsert`](Self::parse_upsert)).
    ///
    /// MySQL infers the conflicting unique key, so there is no arbiter and no
    /// `DO NOTHING`: the clause is only the assignment list, which reuses the
    /// `UPDATE ... SET` shape via [`parse_update_assignments`](Self::parse_update_assignments).
    /// `KEY` and `UPDATE` are matched contextually (`DUPLICATE` is not a reserved
    /// keyword in the shared inventory), so no MySQL-specific keyword row is needed.
    fn parse_on_duplicate_key_update(&mut self) -> ParseResult<Upsert<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::On)?;
        self.expect_contextual_keyword("DUPLICATE")?;
        self.expect_contextual_keyword("KEY")?;
        self.expect_contextual_keyword("UPDATE")?;
        let assignments = self.parse_update_assignments()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Upsert::OnDuplicateKeyUpdate { assignments, meta })
    }

    /// Parse an optional `ON CONFLICT` arbiter: an index-element list (with an
    /// optional partial-index predicate) or `ON CONSTRAINT <name>`. `None` is the
    /// bare `ON CONFLICT DO ...` form, recognised when neither `(` nor `ON` follows.
    fn parse_conflict_target(&mut self) -> ParseResult<Option<ConflictTarget<D::Ext>>> {
        if self.peek_is_keyword(Keyword::On)? {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::On)?;
            self.expect_keyword(Keyword::Constraint)?;
            let name = self.parse_ident()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ConflictTarget::Constraint { name, meta }));
        }
        if self.peek_is_punct(Punctuation::LParen)? {
            let start = self.current_span()?;
            self.expect_punct(Punctuation::LParen, "`(` to open the ON CONFLICT target")?;
            let columns = self.parse_comma_separated_exprs()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the ON CONFLICT target")?;
            let predicate = if self.eat_keyword(Keyword::Where)? {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ConflictTarget::Index {
                columns,
                predicate,
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse the `ON CONFLICT` action: `DO NOTHING` or `DO UPDATE SET ... [WHERE ...]`.
    ///
    /// The `DO UPDATE` assignment list shares its shape with `UPDATE ... SET`, so it
    /// reuses [`parse_update_assignments`](Self::parse_update_assignments).
    fn parse_conflict_action(&mut self) -> ParseResult<ConflictAction<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Do)?;
        if self.eat_keyword(Keyword::Nothing)? {
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(ConflictAction::Nothing { meta });
        }
        self.expect_contextual_keyword("UPDATE")?;
        self.expect_contextual_keyword("SET")?;
        let assignments = self.parse_update_assignments()?;
        let selection = if self.eat_keyword(Keyword::Where)? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(ConflictAction::Update {
            assignments,
            selection,
            meta,
        })
    }

    /// Parse a `MERGE INTO <target> [AS alias] USING <source> ON <cond> <when>+
    /// [RETURNING <output>]` statement (SQL:2003 feature F312), optionally after a
    /// statement-level `WITH`; the dispatch gate
    /// ([`MutationSyntax::merge`](crate::ast::dialect::MutationSyntax)) already checked
    /// by [`parse_statement`](Self::parse_statement). The target reuses the shared
    /// `UPDATE`/`DELETE` relation shape ([`parse_dml_target`](Self::parse_dml_target),
    /// so `ONLY t` / `t *` ride the same inheritance gate), and the `USING` source
    /// reuses the one table-reference entry
    /// ([`parse_table_with_joins`](Self::parse_table_with_joins) — a table, a derived
    /// subquery, or a joined table, each carrying its own alias; a comma-separated
    /// list stays rejected at the following `ON`, as PostgreSQL rejects it).
    pub(super) fn parse_merge_statement(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Statement<D::Ext>> {
        // SQL:2016's `<merge statement>` takes no `<with clause>`; the leading `WITH`
        // is a PostgreSQL 15+/DuckDB extension, gated exactly as `cte_before_insert`
        // gates the `INSERT` form.
        if with.is_some() && !self.features().mutation_syntax.cte_before_merge {
            let span = self.current_span()?;
            return Err(self.error_at(
                span,
                "no `WITH` clause before `MERGE` (the standard merge statement takes none)",
                self.span_text(span).to_owned(),
            ));
        }
        self.expect_keyword(Keyword::Merge)?;
        self.expect_keyword(Keyword::Into)?;
        let target = self.parse_dml_target(&["USING"])?;
        self.expect_keyword(Keyword::Using)?;
        let using = self.parse_table_with_joins()?;
        self.expect_keyword(Keyword::On)?;
        let on = self.parse_expr()?;
        // The standard requires at least one WHEN clause; parse one, then any more.
        let mut clauses = thin_vec![self.parse_merge_when_clause()?];
        while self.peek_is_keyword(Keyword::When)? {
            clauses.push(self.parse_merge_when_clause()?);
        }
        // PostgreSQL 17+ and DuckDB (probed on 1.5.4) both admit the `RETURNING`
        // tail wherever they admit `RETURNING` on the other DML statements, so it
        // rides the same `returning` gate that `parse_returning` reads.
        let returning = self.parse_returning()?;
        let span = start.union(self.preceding_span());
        let merge_meta = self.make_meta(span);
        let merge = Merge {
            with,
            target,
            using,
            on,
            clauses,
            returning,
            meta: merge_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::Merge {
            merge: Box::new(merge),
            meta,
        })
    }

    /// Parse one `WHEN <match kind> [AND <predicate>] THEN <action>` arm.
    ///
    /// The `BY {SOURCE | TARGET}` qualifier after `NOT MATCHED` (PostgreSQL 17+,
    /// DuckDB — both probed) is gated by `merge_when_not_matched_by`; when off, `BY`
    /// is left unconsumed and the arm surfaces the ordinary `THEN` parse error (the
    /// ANSI reject path). A bare `NOT MATCHED` and `NOT MATCHED BY TARGET` are the
    /// same production (pg_query parses both to `MERGE_WHEN_NOT_MATCHED_BY_TARGET`),
    /// so both fold to [`MergeMatchKind::NotMatchedByTarget`] and render canonically
    /// without the qualifier. `BY` after a plain `MATCHED` stays rejected — PostgreSQL
    /// admits the qualifier only on the `NOT MATCHED` spelling (probed).
    fn parse_merge_when_clause(&mut self) -> ParseResult<MergeWhenClause<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::When)?;
        let match_kind = if self.eat_keyword(Keyword::Not)? {
            self.expect_keyword(Keyword::Matched)?;
            if self.features().mutation_syntax.merge_when_not_matched_by
                && self.eat_keyword(Keyword::By)?
            {
                if self.eat_contextual_keyword("SOURCE")? {
                    MergeMatchKind::NotMatchedBySource
                } else if self.eat_contextual_keyword("TARGET")? {
                    MergeMatchKind::NotMatchedByTarget
                } else {
                    return Err(self.unexpected("`SOURCE` or `TARGET` after `NOT MATCHED BY`"));
                }
            } else {
                MergeMatchKind::NotMatchedByTarget
            }
        } else {
            self.expect_keyword(Keyword::Matched)?;
            MergeMatchKind::Matched
        };
        let condition = if self.eat_keyword(Keyword::And)? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_keyword(Keyword::Then)?;
        let action = self.parse_merge_action(match_kind)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(MergeWhenClause {
            match_kind,
            condition,
            action,
            meta,
        })
    }

    /// Parse the `THEN` action, restricted by the arm's match kind.
    ///
    /// The grammar has separate per-arm specifications: `WHEN MATCHED` and `WHEN NOT
    /// MATCHED BY SOURCE` (both pair to an existing target row) take `UPDATE SET ...`
    /// or `DELETE`, `WHEN NOT MATCHED [BY TARGET]` takes `INSERT ...`, and `DO
    /// NOTHING` is valid in every arm (all probed on pg_query 17). Enforcing the
    /// pairing here rejects a `WHEN MATCHED ... THEN INSERT` (and the converse) with a
    /// clean parse error, matching PostgreSQL. `UPDATE`/`SET`/`DELETE` are matched
    /// contextually, mirroring [`parse_conflict_action`](Self::parse_conflict_action).
    fn parse_merge_action(
        &mut self,
        match_kind: MergeMatchKind,
    ) -> ParseResult<MergeAction<D::Ext>> {
        let start = self.current_span()?;
        if self.peek_is_keyword(Keyword::Do)? {
            self.expect_keyword(Keyword::Do)?;
            self.expect_keyword(Keyword::Nothing)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MergeAction::DoNothing { meta });
        }
        match match_kind {
            MergeMatchKind::Matched | MergeMatchKind::NotMatchedBySource => {
                if self.peek_is_contextual_keyword("UPDATE")? {
                    self.expect_contextual_keyword("UPDATE")?;
                    self.expect_contextual_keyword("SET")?;
                    if self.features().mutation_syntax.merge_update_set_star
                        && self.peek_is_op(Operator::Star)?
                    {
                        self.advance()?; // `*`
                        let meta = self.make_meta(start.union(self.preceding_span()));
                        return Ok(MergeAction::UpdateStar { meta });
                    }
                    let assignments = self.parse_update_assignments()?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    Ok(MergeAction::Update { assignments, meta })
                } else if self.features().mutation_syntax.merge_error_action
                    && self.peek_is_contextual_keyword("ERROR")?
                {
                    self.expect_contextual_keyword("ERROR")?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    Ok(MergeAction::Error { meta })
                } else if self.peek_is_contextual_keyword("DELETE")? {
                    self.expect_contextual_keyword("DELETE")?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    Ok(MergeAction::Delete { meta })
                } else if matches!(match_kind, MergeMatchKind::Matched) {
                    Err(self.unexpected(
                        "`UPDATE`, `DELETE`, or `DO NOTHING` after `WHEN MATCHED ... THEN`",
                    ))
                } else {
                    Err(self.unexpected(
                        "`UPDATE`, `DELETE`, or `DO NOTHING` after `WHEN NOT MATCHED BY SOURCE \
                         ... THEN`",
                    ))
                }
            }
            MergeMatchKind::NotMatchedByTarget => {
                if self.peek_is_keyword(Keyword::Insert)? {
                    self.parse_merge_insert_action(start)
                } else {
                    Err(self
                        .unexpected("`INSERT` or `DO NOTHING` after `WHEN NOT MATCHED ... THEN`"))
                }
            }
        }
    }

    /// Parse a `MERGE` insert action: `INSERT DEFAULT VALUES` or `INSERT [(<column>
    /// [, ...])] [OVERRIDING {SYSTEM | USER} VALUE] VALUES (<value>...)`.
    ///
    /// The single value row reuses the `INSERT ... VALUES` row grammar
    /// ([`parse_insert_values_row`](Self::parse_insert_values_row)), column list
    /// ([`parse_parenthesized_insert_columns`](Self::parse_parenthesized_insert_columns)),
    /// and identity override ([`parse_insert_overriding`](Self::parse_insert_overriding)),
    /// so a `DEFAULT` item, a column list, and an `OVERRIDING` clause parse exactly as
    /// in a top-level `INSERT`. `DEFAULT VALUES` is checked *before* the column list
    /// because it takes neither a column list nor an `OVERRIDING` clause — with either
    /// present the `DEFAULT` falls through to the `VALUES` expectation and rejects, as
    /// PostgreSQL does (probed). Each extension rides its own gate
    /// (`merge_insert_default_values` / `merge_insert_overriding` — DuckDB accepts the
    /// former but parse-rejects the latter, probed on 1.5.4); when off, the keyword is
    /// left unconsumed and surfaces the ordinary `VALUES` parse error.
    fn parse_merge_insert_action(&mut self, start: Span) -> ParseResult<MergeAction<D::Ext>> {
        self.expect_keyword(Keyword::Insert)?;
        // DuckDB `INSERT *` / `INSERT BY NAME [*]` (probed 1.5.4).
        if self.features().mutation_syntax.merge_insert_star_by_name {
            if self.peek_is_op(Operator::Star)? {
                self.advance()?; // `*`
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(MergeAction::InsertStar { meta });
            }
            if self.eat_contextual_keyword("BY")? {
                self.expect_contextual_keyword("NAME")?;
                let star = if self.peek_is_op(Operator::Star)? {
                    self.advance()?;
                    true
                } else {
                    false
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(MergeAction::InsertByName { star, meta });
            }
        }
        if self.features().mutation_syntax.merge_insert_default_values
            && self.peek_is_contextual_keyword("DEFAULT")?
        {
            let default_start = self.current_span()?;
            self.advance()?;
            let default = DefaultValue {
                meta: self.make_meta(default_start),
            };
            self.expect_keyword(Keyword::Values)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MergeAction::InsertDefault { default, meta });
        }
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_insert_columns()?
        } else {
            ThinVec::new()
        };
        let overriding = if self.features().mutation_syntax.merge_insert_overriding {
            self.parse_insert_overriding()?
        } else {
            None
        };
        self.expect_keyword(Keyword::Values)?;
        let values = self.parse_insert_values_row()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(MergeAction::Insert {
            columns,
            overriding,
            values,
            meta,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::FeatureSet;
    use crate::ast::{
        ConflictAction, ConflictResolution, ConflictTarget, Delete, DmlSelection, Expr, Insert,
        InsertOverriding, InsertSource, InsertValue, InsertVerb, Literal, LiteralKind, Merge,
        MergeAction, MergeMatchKind, NoExt, OnConflict, OnlySyntax, QuoteStyle,
        RelationInheritance, Resolver as _, SelectItem, SetExpr, Statement, TableFactor, Update,
        UpdateAssignment, UpdateTupleSource, UpdateValue, Upsert,
    };
    use crate::dialect::Postgres;
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};
    use crate::render::Renderer;

    /// Extract a single-column assignment's target name and value, panicking on a
    /// tuple assignment.
    fn single_assignment(
        assignment: &UpdateAssignment<NoExt>,
    ) -> (&crate::ast::ObjectName, &UpdateValue<NoExt>) {
        let UpdateAssignment::Single { target, value, .. } = assignment else {
            panic!("expected a single-column assignment");
        };
        (target, value)
    }

    fn insert_of(parsed: &Parsed) -> &Insert<NoExt> {
        let Statement::Insert { insert, .. } = &parsed.statements()[0] else {
            panic!("expected an INSERT statement");
        };
        insert
    }

    /// Extract an insert's PostgreSQL `ON CONFLICT` clause, panicking on the MySQL
    /// `ON DUPLICATE KEY UPDATE` arm or an absent upsert clause.
    fn on_conflict_of(insert: &Insert<NoExt>) -> &OnConflict<NoExt> {
        let Upsert::OnConflict { conflict, .. } =
            insert.upsert.as_deref().expect("an upsert clause")
        else {
            panic!("expected a PostgreSQL ON CONFLICT clause");
        };
        conflict
    }

    fn update_of(parsed: &Parsed) -> &Update<NoExt> {
        let Statement::Update { update, .. } = &parsed.statements()[0] else {
            panic!("expected an UPDATE statement");
        };
        update
    }

    fn delete_of(parsed: &Parsed) -> &Delete<NoExt> {
        let Statement::Delete { delete, .. } = &parsed.statements()[0] else {
            panic!("expected a DELETE statement");
        };
        delete
    }

    fn merge_of(parsed: &Parsed) -> &Merge<NoExt> {
        let Statement::Merge { merge, .. } = &parsed.statements()[0] else {
            panic!("expected a MERGE statement");
        };
        merge
    }

    /// The dispatch contract: the `INSERT`/`UPDATE`/`DELETE` leading keywords are
    /// routed by the central `parse_statement` to this module's three entries and
    /// yield the family's variants. The `*_of` helpers panic unless the dispatched
    /// statement is the expected variant, pinning the dispatch boundary.
    #[test]
    fn dispatch_routes_dml_keywords_to_this_family() {
        let _ = insert_of(
            &parse_with(
                "INSERT INTO t DEFAULT VALUES",
                crate::ParseConfig::new(TestDialect),
            )
            .expect("INSERT"),
        );
        let _ = update_of(
            &parse_with("UPDATE t SET a = 1", crate::ParseConfig::new(TestDialect))
                .expect("UPDATE"),
        );
        let _ = delete_of(
            &parse_with("DELETE FROM t", crate::ParseConfig::new(TestDialect)).expect("DELETE"),
        );
    }

    #[test]
    fn insert_values_parses_target_columns_and_default_items() {
        let parsed = parse_with(
            "INSERT INTO t (id, name) VALUES (1, DEFAULT), (2, 'b')",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("INSERT VALUES parses");
        let insert = insert_of(&parsed);

        assert!(insert.with.is_none());
        assert_eq!(parsed.resolver().resolve(insert.target.name.0[0].sym), "t");
        assert_eq!(insert.target.columns.len(), 2);
        assert_eq!(
            parsed.resolver().resolve(insert.target.columns[0].sym),
            "id"
        );
        assert_eq!(
            parsed.resolver().resolve(insert.target.columns[1].sym),
            "name",
        );

        let InsertSource::Values { values, .. } = &insert.source else {
            panic!("expected VALUES source");
        };
        assert_eq!(values.rows.len(), 2);
        assert!(matches!(values.rows[0][0], InsertValue::Expr { .. }));
        assert!(matches!(values.rows[0][1], InsertValue::Default { .. }));
        assert!(matches!(
            &values.rows[1][1],
            InsertValue::Expr {
                expr: Expr::Literal {
                    literal: Literal {
                        kind: LiteralKind::String,
                        ..
                    },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn insert_default_values_parses() {
        let parsed = parse_with(
            "INSERT INTO t DEFAULT VALUES",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("INSERT parses");
        let insert = insert_of(&parsed);
        assert!(matches!(insert.source, InsertSource::DefaultValues { .. }));
    }

    #[test]
    fn insert_column_list_on_default_values_is_rejected() {
        // PostgreSQL attaches a column list only to a `VALUES`/query source; pairing one
        // with `DEFAULT VALUES` is a syntax error there, which we now match. The bare
        // column-less `DEFAULT VALUES` form (above) and a column list on a `VALUES` source
        // stay accepted.
        for sql in [
            "INSERT INTO t (id) DEFAULT VALUES",
            "INSERT INTO t (id, name) DEFAULT VALUES",
            "INSERT INTO t AS x (id) DEFAULT VALUES",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err(&format!("should reject {sql:?}"));
        }
        parse_with(
            "INSERT INTO t (id) VALUES (1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("a column list on a VALUES source stays accepted");
    }

    #[test]
    fn insert_select_and_source_with_clause_parse() {
        let parsed = parse_with(
            "INSERT INTO t WITH src AS (SELECT 1) SELECT * FROM src",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("INSERT ... SELECT parses");
        let insert = insert_of(&parsed);
        let InsertSource::Query { query, .. } = &insert.source else {
            panic!("expected query source");
        };
        assert!(query.with.is_some());
        assert!(matches!(query.body, SetExpr::Select { .. }));
    }

    #[test]
    fn statement_level_with_insert_parses() {
        let parsed = parse_with(
            "WITH src AS (SELECT 1) INSERT INTO t SELECT * FROM src",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("WITH ... INSERT parses");
        let insert = insert_of(&parsed);
        let with = insert.with.as_ref().expect("statement-level WITH");
        assert_eq!(with.ctes.len(), 1);
        assert_eq!(parsed.resolver().resolve(with.ctes[0].name.sym), "src");
    }

    #[test]
    fn insert_alias_requires_as_and_overriding_is_preserved() {
        let parsed = parse_with(
            "INSERT INTO t AS target (id) OVERRIDING SYSTEM VALUE VALUES (1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("aliased INSERT parses");
        let insert = insert_of(&parsed);
        let alias = insert.target.alias.as_ref().expect("target alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "target");
        assert_eq!(insert.overriding, Some(InsertOverriding::SystemValue));

        parse_with(
            "INSERT INTO t target VALUES (1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("PostgreSQL rejects INSERT target aliases without AS");
    }

    #[test]
    fn update_parses_assignments_from_where_and_aliases() {
        let parsed = parse_with(
            "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("UPDATE parses");
        let update = update_of(&parsed);

        assert!(update.with.is_none());
        assert_eq!(parsed.resolver().resolve(update.target.name.0[0].sym), "t");
        let alias = update.target.alias.as_ref().expect("target alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "target");
        assert_eq!(update.assignments.len(), 2);
        let (target, value) = single_assignment(&update.assignments[0]);
        assert_eq!(parsed.resolver().resolve(target.0[0].sym), "a");
        assert!(matches!(value, UpdateValue::Expr { .. }));
        let (_, value) = single_assignment(&update.assignments[1]);
        assert!(matches!(value, UpdateValue::Default { .. }));
        assert_eq!(update.from.len(), 1);
        let TableFactor::Table { name, .. } = &update.from[0].relation else {
            panic!("expected a table reference in UPDATE FROM");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "u");
        assert!(matches!(update.selection, Some(DmlSelection::Where { .. })));
    }

    #[test]
    fn update_accepts_bare_alias_and_statement_with_clause() {
        let parsed = parse_with(
            "WITH src AS (SELECT 1) UPDATE t target SET a = 1 WHERE EXISTS (SELECT 1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("WITH ... UPDATE parses");
        let update = update_of(&parsed);
        assert!(update.with.is_some());
        let alias = update.target.alias.as_ref().expect("bare alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "target");
    }

    #[test]
    fn delete_parses_using_where_and_aliases() {
        let parsed = parse_with(
            "DELETE FROM t AS target USING u WHERE target.id = u.id",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("DELETE parses");
        let delete = delete_of(&parsed);

        assert!(delete.with.is_none());
        assert_eq!(parsed.resolver().resolve(delete.target.name.0[0].sym), "t");
        let alias = delete.target.alias.as_ref().expect("target alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "target");
        assert_eq!(delete.using.len(), 1);
        let TableFactor::Table { name, .. } = &delete.using[0].relation else {
            panic!("expected a table reference in DELETE USING");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "u");
        assert!(delete.selection.is_some());
    }

    #[test]
    fn delete_accepts_bare_alias_and_statement_with_clause() {
        let parsed = parse_with(
            "WITH src AS (SELECT 1) DELETE FROM t target WHERE EXISTS (SELECT 1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("WITH ... DELETE parses");
        let delete = delete_of(&parsed);
        assert!(delete.with.is_some());
        let alias = delete.target.alias.as_ref().expect("bare alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "target");
    }

    #[test]
    fn delete_requires_from_and_ansi_rejects_only() {
        parse_with("DELETE t", crate::ParseConfig::new(TestDialect))
            .expect_err("DELETE requires FROM");
        // `ONLY` is gated by dialect data, so the ANSI baseline rejects it rather
        // than misreading the reserved keyword as a table named `ONLY`.
        parse_with(
            "UPDATE ONLY t SET a = 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no ONLY-target inheritance suppression");
        parse_with("DELETE FROM ONLY t", crate::ParseConfig::new(TestDialect))
            .expect_err("ANSI has no ONLY-target inheritance suppression");
    }

    /// A PostgreSQL-featured dialect so the gated RETURNING / ON CONFLICT forms parse.
    const PG_DIALECT: FeatureDialect = FeatureDialect {
        features: &FeatureSet::POSTGRES,
    };

    /// A MySQL-featured dialect so the gated `ON DUPLICATE KEY UPDATE` upsert parses
    /// and renders to a MySQL target. `FeatureDialect` is both a parse [`Dialect`]
    /// and a [`RenderDialect`], so one value drives the round-trip.
    const MYSQL_DIALECT: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };

    /// A SQLite-featured dialect so the gated `INSERT OR`/`UPDATE OR <action>`
    /// conflict-resolution prefix parses and renders back to a SQLite target. Both a
    /// parse [`Dialect`] and a [`RenderDialect`], so one value drives the round-trip.
    const SQLITE_DIALECT: FeatureDialect = FeatureDialect {
        features: &FeatureSet::SQLITE,
    };

    #[test]
    fn sqlite_accepts_string_literal_relation_target_and_round_trips() {
        // SQLite's string-literal identifier misfeature (`string_literal_identifiers`): a
        // single-quoted `'name'` is read as the relation-target name in DML/DDL targets
        // (engine-verified parse-accept on rusqlite 3.53.2 — `DELETE FROM 'table1'`
        // binding-rejects "no such table", i.e. it parse-accepts the string as a name). The
        // folded name records `QuoteStyle::Single`, so the quotes render back verbatim.
        for sql in [
            "DELETE FROM 'table1' WHERE f1 = 3",
            "UPDATE 'table1' SET a = 1",
            "INSERT INTO 'table1' VALUES (1)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SQLITE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
        // The folded name carries the single-quote spelling.
        let parsed = parse_with(
            "DELETE FROM 'table1'",
            crate::ParseConfig::new(SQLITE_DIALECT),
        )
        .expect("DELETE FROM 'table1' parses");
        assert_eq!(
            delete_of(&parsed).target.name.0[0].quote,
            QuoteStyle::Single
        );

        // PostgreSQL (flag off) syntax-rejects a string literal in the relation-target
        // position.
        parse_with(
            "DELETE FROM 'table1' WHERE f1 = 3",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("PostgreSQL rejects a string literal as a relation-target name");
    }

    #[test]
    fn insert_on_conflict_do_update_and_returning_parse() {
        let parsed = parse_with(
            "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET n = excluded.n RETURNING *",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("INSERT ... ON CONFLICT DO UPDATE ... RETURNING parses");
        let insert = insert_of(&parsed);

        let on_conflict = on_conflict_of(insert);
        let ConflictTarget::Index {
            columns, predicate, ..
        } = on_conflict.target.as_ref().expect("a conflict target")
        else {
            panic!("expected an index-inference conflict target");
        };
        assert_eq!(columns.len(), 1);
        assert!(predicate.is_none(), "no index predicate was written");

        let ConflictAction::Update {
            assignments,
            selection,
            ..
        } = &on_conflict.action
        else {
            panic!("expected a DO UPDATE action");
        };
        assert_eq!(assignments.len(), 1);
        let (target, value) = single_assignment(&assignments[0]);
        assert_eq!(parsed.resolver().resolve(target.0[0].sym), "n");
        assert!(matches!(value, UpdateValue::Expr { .. }));
        assert!(selection.is_none(), "no DO UPDATE WHERE was written");

        let returning = insert.returning.as_ref().expect("RETURNING clause");
        assert_eq!(returning.items.len(), 1);
        assert!(matches!(returning.items[0], SelectItem::Wildcard { .. }));
    }

    #[test]
    fn insert_on_conflict_target_and_action_forms_parse() {
        // Bare `DO NOTHING`: no arbiter.
        let parsed = parse_with(
            "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("bare ON CONFLICT DO NOTHING parses");
        let on_conflict = on_conflict_of(insert_of(&parsed));
        assert!(on_conflict.target.is_none(), "no arbiter for the bare form");
        assert!(matches!(on_conflict.action, ConflictAction::Nothing { .. }));

        // Named constraint arbiter.
        let parsed = parse_with(
            "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ON CONFLICT ON CONSTRAINT parses");
        let target = on_conflict_of(insert_of(&parsed))
            .target
            .as_ref()
            .expect("a constraint arbiter");
        let ConflictTarget::Constraint { name, .. } = target else {
            panic!("expected a named-constraint conflict target");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "t_pkey");

        // Index arbiter with a partial-index predicate.
        let parsed = parse_with(
            "INSERT INTO t VALUES (1) ON CONFLICT (a, b) WHERE a > 0 DO NOTHING",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ON CONFLICT (cols) WHERE predicate parses");
        let target = on_conflict_of(insert_of(&parsed))
            .target
            .as_ref()
            .expect("an index arbiter");
        let ConflictTarget::Index {
            columns, predicate, ..
        } = target
        else {
            panic!("expected an index-inference conflict target");
        };
        assert_eq!(columns.len(), 2);
        assert!(predicate.is_some(), "the index predicate is captured");
    }

    #[test]
    fn update_and_delete_returning_parse() {
        let parsed = parse_with(
            "UPDATE t SET a = 1 WHERE id = 2 RETURNING a, id",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("UPDATE ... RETURNING parses");
        let returning = update_of(&parsed)
            .returning
            .as_ref()
            .expect("RETURNING clause");
        assert_eq!(returning.items.len(), 2);

        // A qualified wildcard and an aliased expression round-trip into SelectItem shapes.
        let parsed = parse_with(
            "DELETE FROM t RETURNING t.*, id AS removed",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("DELETE ... RETURNING parses");
        let returning = delete_of(&parsed)
            .returning
            .as_ref()
            .expect("RETURNING clause");
        assert_eq!(returning.items.len(), 2);
        assert!(matches!(
            returning.items[0],
            SelectItem::QualifiedWildcard { .. }
        ));
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &returning.items[1]
        else {
            panic!("expected an aliased RETURNING expression");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "removed");
    }

    #[test]
    fn returning_wildcard_carries_the_gated_modifiers() {
        // The RETURNING item shares the projection-item grammar, so the DuckDB
        // wildcard modifiers ride the same `wildcard_modifiers` gate here (corpus:
        // `RETURNING * EXCLUDE c1`). PG_DIALECT leaves the gate off, so the modifier
        // keyword stays unconsumed input there — the over-acceptance guard.
        use crate::dialect::DuckDb;
        let parsed = parse_with(
            "INSERT INTO v0 VALUES (1) RETURNING * EXCLUDE c1",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("RETURNING * EXCLUDE parses under DuckDb");
        let returning = insert_of(&parsed).returning.as_ref().expect("RETURNING");
        let SelectItem::Wildcard {
            options: Some(options),
            ..
        } = &returning.items[0]
        else {
            panic!("expected the modifier-bearing RETURNING wildcard");
        };
        assert_eq!(options.exclude.len(), 1);
        assert!(
            parse_with(
                "INSERT INTO v0 VALUES (1) RETURNING * EXCLUDE c1",
                crate::ParseConfig::new(PG_DIALECT)
            )
            .is_err(),
            "the modifiers stay rejected where the gate is off",
        );
    }

    #[test]
    fn ansi_rejects_returning_and_on_conflict() {
        // The mutation extensions are gated off under ANSI, so the trailing clause is
        // left unconsumed and surfaces as a parse error.
        for sql in [
            "INSERT INTO t VALUES (1) RETURNING *",
            "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
            "UPDATE t SET a = 1 RETURNING *",
            "DELETE FROM t RETURNING *",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no RETURNING / ON CONFLICT");
        }
    }

    #[test]
    fn update_and_delete_only_target_forms_parse() {
        // Bare `ONLY name`.
        let parsed = parse_with(
            "UPDATE ONLY t SET a = 1",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("UPDATE ONLY parses");
        let target = &update_of(&parsed).target;
        assert_eq!(
            target.inheritance,
            RelationInheritance::Only(OnlySyntax::Bare)
        );
        assert_eq!(parsed.resolver().resolve(target.name.0[0].sym), "t");

        // Parenthesized `ONLY ( name )`, with an alias after the close paren.
        let parsed = parse_with(
            "DELETE FROM ONLY (t) AS d WHERE d.id = 1",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("DELETE FROM ONLY (t) parses");
        let delete = delete_of(&parsed);
        assert_eq!(
            delete.target.inheritance,
            RelationInheritance::Only(OnlySyntax::Parenthesized)
        );
        let alias = delete.target.alias.as_ref().expect("target alias");
        assert_eq!(parsed.resolver().resolve(alias.sym), "d");
    }

    #[test]
    fn update_and_delete_descendant_star_target_forms_round_trip() {
        // `t *` is the explicit-descendants counterpart to `ONLY t`: it attaches a
        // trailing star to a plain target and round-trips through rendering.
        let parsed = parse_with("UPDATE t * SET a = 1", crate::ParseConfig::new(Postgres))
            .expect("UPDATE target `*` parses");
        assert_eq!(
            update_of(&parsed).target.inheritance,
            RelationInheritance::Descendants
        );
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("UPDATE target `*` renders"),
            "UPDATE t * SET a = 1"
        );

        let parsed = parse_with(
            "DELETE FROM t * WHERE id = 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect("DELETE target `*` parses");
        assert_eq!(
            delete_of(&parsed).target.inheritance,
            RelationInheritance::Descendants
        );
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("DELETE target `*` renders"),
            "DELETE FROM t * WHERE id = 1"
        );

        // The descendant `*` is gated like `ONLY`, so ANSI rejects it on a target too.
        parse_with("UPDATE t * SET a = 1", crate::ParseConfig::new(TestDialect))
            .expect_err("ANSI has no descendant-table `*` marker");
    }

    #[test]
    fn update_multi_column_assignment_forms_parse() {
        // A value row mixing an expression and a per-element DEFAULT.
        let parsed = parse_with(
            "UPDATE t SET (a, b) = (1, DEFAULT)",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("tuple value row parses");
        let assignments = &update_of(&parsed).assignments;
        assert_eq!(assignments.len(), 1);
        let UpdateAssignment::Tuple {
            targets, source, ..
        } = &assignments[0]
        else {
            panic!("expected a tuple assignment");
        };
        assert_eq!(targets.len(), 2);
        assert_eq!(parsed.resolver().resolve(targets[0].0[0].sym), "a");
        let UpdateTupleSource::Row {
            explicit, values, ..
        } = source
        else {
            panic!("expected a value-row source");
        };
        assert!(!explicit, "an implicit `( ... )` row is not `ROW( ... )`");
        assert_eq!(values.len(), 2);
        assert!(matches!(values[0], UpdateValue::Expr { .. }));
        assert!(matches!(values[1], UpdateValue::Default { .. }));

        // Explicit `ROW( ... )`.
        let parsed = parse_with(
            "UPDATE t SET (a, b) = ROW(1, 2)",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("explicit ROW source parses");
        let UpdateAssignment::Tuple { source, .. } = &update_of(&parsed).assignments[0] else {
            panic!("expected a tuple assignment");
        };
        assert!(matches!(
            source,
            UpdateTupleSource::Row { explicit: true, .. }
        ));

        // Row subquery.
        let parsed = parse_with(
            "UPDATE t SET (a, b) = (SELECT x, y FROM u)",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("row subquery source parses");
        let UpdateAssignment::Tuple { source, .. } = &update_of(&parsed).assignments[0] else {
            panic!("expected a tuple assignment");
        };
        assert!(matches!(source, UpdateTupleSource::Subquery { .. }));

        // Bare `DEFAULT` for every target.
        let parsed = parse_with(
            "UPDATE t SET (a, b) = DEFAULT",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("bare DEFAULT source parses");
        let UpdateAssignment::Tuple { source, .. } = &update_of(&parsed).assignments[0] else {
            panic!("expected a tuple assignment");
        };
        assert!(matches!(source, UpdateTupleSource::Default { .. }));

        // A single and a tuple assignment in one statement.
        let parsed = parse_with(
            "UPDATE t SET a = 1, (b, c) = (2, 3)",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("mixed single and tuple assignments parse");
        let assignments = &update_of(&parsed).assignments;
        assert_eq!(assignments.len(), 2);
        assert!(matches!(assignments[0], UpdateAssignment::Single { .. }));
        assert!(matches!(assignments[1], UpdateAssignment::Tuple { .. }));
    }

    #[test]
    fn multi_column_assignment_reaches_on_conflict_do_update() {
        let parsed = parse_with(
            "INSERT INTO t VALUES (1) ON CONFLICT (id) DO UPDATE SET (a, b) = (1, 2)",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ON CONFLICT DO UPDATE with a tuple assignment parses");
        let ConflictAction::Update { assignments, .. } = &on_conflict_of(insert_of(&parsed)).action
        else {
            panic!("expected a DO UPDATE action");
        };
        assert!(matches!(assignments[0], UpdateAssignment::Tuple { .. }));
    }

    /// Extract a MySQL `ON DUPLICATE KEY UPDATE` assignment list, panicking on the
    /// PostgreSQL `ON CONFLICT` arm or an absent upsert clause.
    fn on_duplicate_assignments(insert: &Insert<NoExt>) -> &[UpdateAssignment<NoExt>] {
        let Upsert::OnDuplicateKeyUpdate { assignments, .. } =
            insert.upsert.as_deref().expect("an upsert clause")
        else {
            panic!("expected a MySQL ON DUPLICATE KEY UPDATE clause");
        };
        assignments
    }

    #[test]
    fn insert_on_duplicate_key_update_parses_and_round_trips() {
        let sql =
            "INSERT INTO t (id, n) VALUES (1, 2) ON DUPLICATE KEY UPDATE a = 1, b = VALUES(c)";
        let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT))
            .expect("ON DUPLICATE KEY UPDATE parses");
        let insert = insert_of(&parsed);

        let assignments = on_duplicate_assignments(insert);
        assert_eq!(assignments.len(), 2);
        let (target, value) = single_assignment(&assignments[0]);
        assert_eq!(parsed.resolver().resolve(target.0[0].sym), "a");
        assert!(matches!(value, UpdateValue::Expr { .. }));

        // `b = VALUES(c)`: the legacy proposed-value reference parses as a canonical
        // function call named `VALUES`, so it reuses `Expr::Function` rather than a
        // bespoke node.
        let (target, value) = single_assignment(&assignments[1]);
        assert_eq!(parsed.resolver().resolve(target.0[0].sym), "b");
        let UpdateValue::Expr {
            expr: Expr::Function { call, .. },
            ..
        } = value
        else {
            panic!("expected `VALUES(c)` to parse as a function call");
        };
        assert_eq!(parsed.resolver().resolve(call.name.0[0].sym), "VALUES");

        // The clause round-trips through the MySQL render target verbatim.
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("ON DUPLICATE KEY UPDATE renders"),
            sql,
        );
    }

    #[test]
    fn ansi_and_postgres_reject_on_duplicate_key_update() {
        // The clause is gated by `on_duplicate_key_update`, which only MySQL enables;
        // ANSI and PostgreSQL leave `ON` unconsumed, so the trailing clause is leftover
        // input and the parse fails.
        let sql = "INSERT INTO t VALUES (1) ON DUPLICATE KEY UPDATE a = 1";
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect_err("ANSI has no ON DUPLICATE KEY UPDATE");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL spells its upsert ON CONFLICT");
    }

    #[test]
    fn insert_row_alias_parses_and_round_trips() {
        // MySQL 8.0.19+ row alias between the VALUES source and `ON DUPLICATE KEY UPDATE`,
        // with an optional column-alias list; it reuses the shared `TableAlias` shape.
        let sql = "INSERT INTO t VALUES (1) AS new_row(c) ON DUPLICATE KEY UPDATE a = new_row.c";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT)).expect("row alias parses");
        let insert = insert_of(&parsed);
        let alias = insert.row_alias.as_ref().expect("a row alias");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "new_row");
        assert_eq!(alias.columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(alias.columns[0].sym), "c");
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("row alias renders"),
            sql,
        );
    }

    #[test]
    fn ansi_and_postgres_reject_insert_row_alias() {
        // The row alias rides the `on_duplicate_key_update` gate (only MySQL enables it),
        // so elsewhere the `AS new_row` after the source is leftover input -> reject.
        let sql = "INSERT INTO t VALUES (1) AS new_row";
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect_err("ANSI has no INSERT row alias");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL has no INSERT row alias");
    }

    #[test]
    fn update_order_by_and_limit_tails_parse_and_round_trip() {
        let sql = "UPDATE t SET a = 1 WHERE b = 2 ORDER BY c DESC LIMIT 5";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT)).expect("UPDATE tails parse");
        let update = update_of(&parsed);
        assert_eq!(update.order_by.len(), 1);
        assert_eq!(update.order_by[0].asc, Some(false));
        let limit = update.limit.as_ref().expect("a LIMIT tail");
        assert!(limit.limit.is_some() && limit.offset.is_none());
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("UPDATE tails render"),
            sql,
        );
    }

    #[test]
    fn delete_order_by_and_limit_tails_parse_and_round_trip() {
        let sql = "DELETE FROM t WHERE b = 2 ORDER BY c LIMIT 5";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT)).expect("DELETE tails parse");
        let delete = delete_of(&parsed);
        assert_eq!(delete.order_by.len(), 1);
        let limit = delete.limit.as_ref().expect("a LIMIT tail");
        assert!(limit.limit.is_some() && limit.offset.is_none());
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("DELETE tails render"),
            sql,
        );
    }

    #[test]
    fn ansi_and_postgres_reject_update_and_delete_tails() {
        // The row-limiting tails are gated by `update_delete_tails` (only MySQL/Lenient);
        // elsewhere the trailing `ORDER BY`/`LIMIT` is leftover input -> reject.
        for sql in [
            "UPDATE t SET a = 1 ORDER BY a LIMIT 1",
            "DELETE FROM t ORDER BY a LIMIT 1",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no UPDATE/DELETE tails");
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err("PostgreSQL has no UPDATE/DELETE tails");
        }
    }

    /// Extract an insert's MySQL `SET` assignment-list source, panicking on any other
    /// source kind.
    fn set_source_assignments(insert: &Insert<NoExt>) -> &[UpdateAssignment<NoExt>] {
        let InsertSource::Set { assignments, .. } = &insert.source else {
            panic!("expected a SET source");
        };
        assignments
    }

    #[test]
    fn mysql_replace_statement_sources_parse_and_round_trip() {
        // REPLACE folds onto the canonical `Insert` shape tagged `InsertVerb::Replace`,
        // sharing the VALUES / SET / SELECT tail grammar, and round-trips verbatim under
        // the MySQL render target. It carries none of the upsert/returning tails.
        for sql in [
            "REPLACE INTO t VALUES (1)",
            "REPLACE INTO t (a, b) VALUES (1, 2)",
            "REPLACE INTO t SET a = 1, b = 2",
            "REPLACE INTO t SELECT 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT))
                .expect("REPLACE parses under MySQL");
            let insert = insert_of(&parsed);
            assert_eq!(insert.verb, InsertVerb::Replace);
            assert!(insert.upsert.is_none(), "REPLACE has no upsert tail");
            assert!(insert.returning.is_none(), "REPLACE has no RETURNING tail");
            assert_eq!(
                Renderer::new(MYSQL_DIALECT)
                    .render_parsed(&parsed)
                    .expect("REPLACE renders"),
                sql,
            );
        }

        // The SET source reuses the shared `UpdateAssignment` shape, so a `DEFAULT` in
        // value position parses exactly as it does in `UPDATE ... SET`.
        let parsed = parse_with(
            "REPLACE INTO t SET a = 1, b = DEFAULT",
            crate::ParseConfig::new(MYSQL_DIALECT),
        )
        .expect("REPLACE ... SET parses");
        let assignments = set_source_assignments(insert_of(&parsed));
        assert_eq!(assignments.len(), 2);
        let (_, value) = single_assignment(&assignments[1]);
        assert!(matches!(value, UpdateValue::Default { .. }));
    }

    #[test]
    fn mysql_replace_optional_into_normalizes_to_into() {
        // `INTO` is optional after `REPLACE` (MySQL); the bare form parses and renders
        // canonically with `INTO`, mirroring how a bare DML alias normalizes to `AS`.
        let parsed = parse_with(
            "REPLACE t VALUES (1)",
            crate::ParseConfig::new(MYSQL_DIALECT),
        )
        .expect("REPLACE without INTO parses");
        assert_eq!(insert_of(&parsed).verb, InsertVerb::Replace);
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("REPLACE renders"),
            "REPLACE INTO t VALUES (1)",
        );
    }

    #[test]
    fn mysql_replace_rejects_on_duplicate_key_update() {
        // REPLACE *is* the conflict resolution, so it has no upsert tail: a trailing
        // `ON DUPLICATE KEY UPDATE` is leftover input even under MySQL.
        parse_with(
            "REPLACE INTO t VALUES (1) ON DUPLICATE KEY UPDATE a = 1",
            crate::ParseConfig::new(MYSQL_DIALECT),
        )
        .expect_err("REPLACE has no ON DUPLICATE KEY UPDATE tail");
    }

    #[test]
    fn ansi_and_postgres_reject_replace_statement() {
        // `replace_into` is off in ANSI/PostgreSQL, so `REPLACE` is never dispatched and
        // surfaces as an unknown statement.
        for sql in ["REPLACE INTO t VALUES (1)", "REPLACE INTO t SET a = 1"] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no REPLACE statement");
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err("PostgreSQL has no REPLACE statement");
        }
    }

    #[test]
    fn insert_or_action_forms_parse_and_round_trip() {
        // SQLite `INSERT OR <action>`: the conflict-resolution algorithm on the verb,
        // parsed into `Insert.or_action` and round-tripped verbatim under the SQLite
        // render target. All five actions reach the slot; `FAIL` (not a reserved keyword)
        // is matched contextually exactly like the others.
        for (sql, action) in [
            (
                "INSERT OR ROLLBACK INTO t VALUES (1)",
                ConflictResolution::Rollback,
            ),
            (
                "INSERT OR ABORT INTO t VALUES (1)",
                ConflictResolution::Abort,
            ),
            ("INSERT OR FAIL INTO t VALUES (1)", ConflictResolution::Fail),
            (
                "INSERT OR IGNORE INTO t VALUES (1)",
                ConflictResolution::Ignore,
            ),
            (
                "INSERT OR REPLACE INTO t VALUES (1)",
                ConflictResolution::Replace,
            ),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_DIALECT))
                .expect("INSERT OR <action> parses");
            let insert = insert_of(&parsed);
            assert_eq!(insert.verb, InsertVerb::Insert);
            assert_eq!(insert.or_action, Some(action));
            assert_eq!(
                Renderer::new(SQLITE_DIALECT)
                    .render_parsed(&parsed)
                    .expect("INSERT OR <action> renders"),
                sql,
            );
        }

        // A plain `INSERT` under the same preset leaves the slot empty.
        assert!(
            insert_of(
                &parse_with(
                    "INSERT INTO t VALUES (1)",
                    crate::ParseConfig::new(SQLITE_DIALECT)
                )
                .expect("plain INSERT")
            )
            .or_action
            .is_none()
        );
    }

    #[test]
    fn update_or_action_parses_and_round_trips() {
        // `UPDATE OR <action>` reuses the same slot on the `Update` node and round-trips.
        let sql = "UPDATE OR REPLACE t SET a = 2 WHERE a = 1";
        let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_DIALECT))
            .expect("UPDATE OR REPLACE parses");
        assert_eq!(
            update_of(&parsed).or_action,
            Some(ConflictResolution::Replace)
        );
        assert_eq!(
            Renderer::new(SQLITE_DIALECT)
                .render_parsed(&parsed)
                .expect("UPDATE OR REPLACE renders"),
            sql,
        );

        for (sql, action) in [
            (
                "UPDATE OR ROLLBACK t SET a = 1",
                ConflictResolution::Rollback,
            ),
            ("UPDATE OR ABORT t SET a = 1", ConflictResolution::Abort),
            ("UPDATE OR FAIL t SET a = 1", ConflictResolution::Fail),
            ("UPDATE OR IGNORE t SET a = 1", ConflictResolution::Ignore),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_DIALECT))
                .expect("UPDATE OR <action> parses");
            assert_eq!(update_of(&parsed).or_action, Some(action));
            assert_eq!(
                Renderer::new(SQLITE_DIALECT)
                    .render_parsed(&parsed)
                    .expect("UPDATE OR <action> renders"),
                sql,
            );
        }
    }

    #[test]
    fn insert_or_replace_and_replace_into_are_distinct_surfaces() {
        // `INSERT OR REPLACE` and `REPLACE INTO` are equivalent SQLite semantics written
        // as different source texts, so each keeps its own representation (the `ConflictResolution`
        // slot vs the `InsertVerb::Replace` tag) and round-trips through it — one is never
        // folded onto the other.
        let or_replace = parse_with(
            "INSERT OR REPLACE INTO t VALUES (1)",
            crate::ParseConfig::new(SQLITE_DIALECT),
        )
        .expect("INSERT OR REPLACE parses");
        let insert = insert_of(&or_replace);
        assert_eq!(insert.verb, InsertVerb::Insert);
        assert_eq!(insert.or_action, Some(ConflictResolution::Replace));

        let replace_into = parse_with(
            "REPLACE INTO t VALUES (1)",
            crate::ParseConfig::new(SQLITE_DIALECT),
        )
        .expect("REPLACE INTO parses");
        let replace = insert_of(&replace_into);
        assert_eq!(replace.verb, InsertVerb::Replace);
        assert!(
            replace.or_action.is_none(),
            "the REPLACE statement carries no OR-action prefix",
        );

        assert_eq!(
            Renderer::new(SQLITE_DIALECT)
                .render_parsed(&or_replace)
                .expect("renders"),
            "INSERT OR REPLACE INTO t VALUES (1)",
        );
        assert_eq!(
            Renderer::new(SQLITE_DIALECT)
                .render_parsed(&replace_into)
                .expect("renders"),
            "REPLACE INTO t VALUES (1)",
        );
    }

    #[test]
    fn or_action_is_rejected_where_the_gate_is_off() {
        // `or_conflict_action` is off in ANSI/PostgreSQL/MySQL, so the `OR` after the verb
        // is left unconsumed: on `INSERT` the expected `INTO` then fails against the stray
        // `OR`, on `UPDATE` the reserved `OR` cannot be the target name — either way a clean
        // parse error. The MySQL rejects also pin that MySQL's `INSERT IGNORE` is a
        // *different* surface (a bare post-verb `IGNORE`, no `OR`), deliberately not
        // absorbed here.
        for sql in [
            "INSERT OR REPLACE INTO t VALUES (1)",
            "INSERT OR IGNORE INTO t VALUES (1)",
            "UPDATE OR REPLACE t SET a = 1",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no INSERT OR / UPDATE OR");
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err("PostgreSQL has no INSERT OR / UPDATE OR");
            parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT))
                .expect_err("MySQL has no OR-prefixed conflict action (INSERT IGNORE differs)");
        }
    }

    #[test]
    fn mysql_insert_set_source_parses_and_round_trips() {
        // The `SET` source is shared by INSERT and REPLACE under `insert_set`; INSERT
        // accepts it under MySQL and round-trips verbatim, with the standard verb tag.
        let sql = "INSERT INTO t SET a = 1, b = 2";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(MYSQL_DIALECT)).expect("INSERT ... SET parses");
        let insert = insert_of(&parsed);
        assert_eq!(insert.verb, InsertVerb::Insert);
        assert_eq!(set_source_assignments(insert).len(), 2);
        assert_eq!(
            Renderer::new(MYSQL_DIALECT)
                .render_parsed(&parsed)
                .expect("INSERT ... SET renders"),
            sql,
        );
    }

    #[test]
    fn ansi_and_postgres_reject_insert_set_source() {
        // `insert_set` is off in ANSI/PostgreSQL, so `SET` after the target is leftover
        // input and the parse fails.
        parse_with(
            "INSERT INTO t SET a = 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI INSERT has no SET source");
        parse_with("INSERT INTO t SET a = 1", crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL INSERT has no SET source");
    }

    #[test]
    fn on_duplicate_key_update_reuses_the_update_set_assignment_shape() {
        // The MySQL upsert assignment list is the same `UpdateAssignment::Single`
        // shape an `UPDATE ... SET` and an `ON CONFLICT DO UPDATE SET` produce, so the
        // three clauses share one assignment representation (ADR-0011), differing only
        // in their clause lead-in.
        let parsed = parse_with(
            "INSERT INTO t VALUES (1) ON DUPLICATE KEY UPDATE n = 2",
            crate::ParseConfig::new(MYSQL_DIALECT),
        )
        .expect("ON DUPLICATE KEY UPDATE parses");
        let on_duplicate = on_duplicate_assignments(insert_of(&parsed));

        let pg = parse_with(
            "INSERT INTO t VALUES (1) ON CONFLICT (id) DO UPDATE SET n = 2",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ON CONFLICT DO UPDATE parses");
        let Upsert::OnConflict { conflict, .. } =
            insert_of(&pg).upsert.as_deref().expect("an upsert clause")
        else {
            panic!("expected a PostgreSQL ON CONFLICT clause");
        };
        let ConflictAction::Update {
            assignments: on_conflict,
            ..
        } = &conflict.action
        else {
            panic!("expected a DO UPDATE action");
        };

        // Same variant, same resolved target, same value shape across both spellings.
        let (mysql_target, mysql_value) = single_assignment(&on_duplicate[0]);
        let (pg_target, pg_value) = single_assignment(&on_conflict[0]);
        assert_eq!(
            parsed.resolver().resolve(mysql_target.0[0].sym),
            pg.resolver().resolve(pg_target.0[0].sym),
        );
        assert!(matches!(mysql_value, UpdateValue::Expr { .. }));
        assert!(matches!(pg_value, UpdateValue::Expr { .. }));
    }

    #[test]
    fn update_and_delete_where_current_of_parse() {
        let parsed = parse_with(
            "UPDATE t SET a = 1 WHERE CURRENT OF c",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("UPDATE ... WHERE CURRENT OF parses");
        let DmlSelection::CurrentOf { cursor, .. } = update_of(&parsed)
            .selection
            .as_ref()
            .expect("a WHERE clause")
        else {
            panic!("expected a positioned WHERE CURRENT OF");
        };
        assert_eq!(parsed.resolver().resolve(cursor.sym), "c");

        // A quoted cursor name round-trips through the identifier path.
        let parsed = parse_with(
            "DELETE FROM t WHERE CURRENT OF \"My Cursor\"",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("DELETE ... WHERE CURRENT OF parses");
        let DmlSelection::CurrentOf { cursor, .. } = delete_of(&parsed)
            .selection
            .as_ref()
            .expect("a WHERE clause")
        else {
            panic!("expected a positioned WHERE CURRENT OF");
        };
        assert_eq!(parsed.resolver().resolve(cursor.sym), "My Cursor");

        // A plain condition stays a `Where`, not a positioned form.
        let parsed = parse_with(
            "UPDATE t SET a = 1 WHERE id = 2",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("UPDATE ... WHERE condition parses");
        assert!(matches!(
            update_of(&parsed).selection,
            Some(DmlSelection::Where { .. })
        ));
    }

    #[test]
    fn ansi_rejects_advanced_update_delete_forms() {
        // Each advanced form is gated, so the ANSI baseline rejects it.
        for sql in [
            "UPDATE ONLY t SET a = 1",
            "DELETE FROM ONLY t",
            "UPDATE t SET (a, b) = (1, 2)",
            "UPDATE t SET a = 1 WHERE CURRENT OF c",
            "DELETE FROM t WHERE CURRENT OF c",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no ONLY / multi-column SET / WHERE CURRENT OF");
        }
    }

    /// The full SQL:2003 `MERGE` shape: an aliased target and source, an `ON`
    /// condition, and every action across MATCHED/NOT-MATCHED arms (with and without
    /// an `AND` predicate). It parses into the canonical [`Merge`] node and renders
    /// back byte-for-byte, proving the round-trip the ticket requires.
    #[test]
    fn merge_full_form_parses_and_round_trips() {
        let sql = "MERGE INTO accounts AS t USING transactions AS s ON t.id = s.account_id \
                   WHEN MATCHED AND s.amount < 0 THEN DELETE \
                   WHEN MATCHED THEN UPDATE SET balance = t.balance + s.amount \
                   WHEN NOT MATCHED THEN INSERT (id, balance) VALUES (s.account_id, s.amount) \
                   WHEN NOT MATCHED AND s.flag THEN DO NOTHING";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(Postgres)).expect("the full MERGE parses");
        let merge = merge_of(&parsed);

        assert_eq!(
            parsed.resolver().resolve(merge.target.name.0[0].sym),
            "accounts"
        );
        assert_eq!(
            parsed
                .resolver()
                .resolve(merge.target.alias.as_ref().expect("a target alias").sym),
            "t",
        );
        let TableFactor::Table { name, .. } = &merge.using.relation else {
            panic!("expected a plain-table USING source");
        };
        assert!(merge.using.joins.is_empty());
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "transactions");
        assert!(matches!(merge.on, Expr::BinaryOp { .. }));

        assert_eq!(merge.clauses.len(), 4);

        // WHEN MATCHED AND ... THEN DELETE
        assert_eq!(merge.clauses[0].match_kind, MergeMatchKind::Matched);
        assert!(merge.clauses[0].condition.is_some());
        assert!(matches!(
            merge.clauses[0].action,
            MergeAction::Delete { .. }
        ));

        // WHEN MATCHED THEN UPDATE SET ...
        assert_eq!(merge.clauses[1].match_kind, MergeMatchKind::Matched);
        assert!(merge.clauses[1].condition.is_none());
        let MergeAction::Update { assignments, .. } = &merge.clauses[1].action else {
            panic!("expected an UPDATE action on the second arm");
        };
        assert_eq!(assignments.len(), 1);

        // WHEN NOT MATCHED THEN INSERT (...) VALUES (...)
        assert_eq!(
            merge.clauses[2].match_kind,
            MergeMatchKind::NotMatchedByTarget
        );
        let MergeAction::Insert {
            columns, values, ..
        } = &merge.clauses[2].action
        else {
            panic!("expected an INSERT action on the third arm");
        };
        assert_eq!(columns.len(), 2);
        assert_eq!(values.len(), 2);

        // WHEN NOT MATCHED AND ... THEN DO NOTHING
        assert_eq!(
            merge.clauses[3].match_kind,
            MergeMatchKind::NotMatchedByTarget
        );
        assert!(merge.clauses[3].condition.is_some());
        assert!(matches!(
            merge.clauses[3].action,
            MergeAction::DoNothing { .. }
        ));

        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );
    }

    /// The `USING` source reuses the table-factor grammar, so a derived subquery (with
    /// its own alias) is a valid source and round-trips.
    #[test]
    fn merge_using_subquery_source_parses_and_round_trips() {
        let sql = "MERGE INTO t USING (SELECT id FROM staging) AS s ON t.id = s.id \
                   WHEN MATCHED THEN DELETE";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("a subquery USING source parses");
        assert!(matches!(
            merge_of(&parsed).using.relation,
            TableFactor::Derived { .. }
        ));
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );
    }

    /// `merge` is on in the ANSI baseline — `MERGE` is the SQL:2016 standard upsert, not
    /// a dialect extension — so the standard-only `TestDialect` accepts it.
    #[test]
    fn ansi_accepts_merge() {
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("ANSI accepts the standard MERGE statement");
    }

    /// MySQL has no `MERGE`, so `merge` is off and the leading keyword is never
    /// dispatched — the statement is rejected with a clean parse error.
    #[test]
    fn mysql_rejects_merge() {
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
            crate::ParseConfig::new(MYSQL_DIALECT),
        )
        .expect_err("MySQL has no MERGE statement");
    }

    /// The standard pairs each branch with its own actions: a `WHEN MATCHED` arm takes
    /// `UPDATE`/`DELETE`, a `WHEN NOT MATCHED` arm takes `INSERT`. The parser enforces
    /// the pairing, rejecting a cross-matched action with a clean error (matching
    /// PostgreSQL). `DO NOTHING` is the one action valid in both branches.
    #[test]
    fn merge_action_pairing_is_enforced() {
        for sql in [
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN INSERT (id) VALUES (s.id)",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN UPDATE SET a = 1",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN DELETE",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("mismatched MERGE action: {sql:?}"));
        }

        // `DO NOTHING` is valid in both branches.
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DO NOTHING",
            crate::ParseConfig::new(Postgres),
        )
        .expect("DO NOTHING is a valid MATCHED action");
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN DO NOTHING",
            crate::ParseConfig::new(Postgres),
        )
        .expect("DO NOTHING is a valid NOT MATCHED action");
    }

    /// `WHEN NOT MATCHED BY {SOURCE | TARGET}` (PostgreSQL 17+; DuckDB, probed on
    /// 1.5.4): `BY SOURCE` is the unpaired-target arm (admitting `UPDATE`/`DELETE`/
    /// `DO NOTHING`, exactly as `MATCHED` — engine-verified) and `BY TARGET` is the
    /// same production as the bare `NOT MATCHED` (pg_query folds both to one
    /// `matchKind`), so it parses to the same node and renders the canonical bare
    /// spelling. The standard-only `TestDialect` leaves `BY` unconsumed
    /// (`merge_when_not_matched_by` off — SQL:2016 has only the two-way match).
    #[test]
    fn merge_not_matched_by_source_target_parses_and_round_trips() {
        let sql = "MERGE INTO t USING s ON t.id = s.id \
                   WHEN NOT MATCHED BY SOURCE AND t.stale THEN DELETE \
                   WHEN NOT MATCHED BY SOURCE THEN UPDATE SET archived = TRUE \
                   WHEN NOT MATCHED THEN INSERT VALUES (s.id)";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("BY SOURCE arms parse under PostgreSQL");
        let merge = merge_of(&parsed);
        assert_eq!(
            merge.clauses[0].match_kind,
            MergeMatchKind::NotMatchedBySource
        );
        assert_eq!(
            merge.clauses[1].match_kind,
            MergeMatchKind::NotMatchedBySource
        );
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );

        // `BY TARGET` folds onto the bare `NOT MATCHED` production and renders bare.
        let by_target = "MERGE INTO t USING s ON t.id = s.id \
                         WHEN NOT MATCHED BY TARGET THEN INSERT VALUES (s.id)";
        let parsed =
            parse_with(by_target, crate::ParseConfig::new(Postgres)).expect("BY TARGET parses");
        assert_eq!(
            merge_of(&parsed).clauses[0].match_kind,
            MergeMatchKind::NotMatchedByTarget
        );
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT VALUES (s.id)",
        );

        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("the standard merge has no BY SOURCE arm");
    }

    /// The `BY` qualifier's reject boundaries, matching pg_query 17 (all probed):
    /// `BY` attaches only to `NOT MATCHED` (never a plain `MATCHED`), takes only
    /// `SOURCE`/`TARGET`, and the arm's actions follow its pairing — `BY SOURCE`
    /// rejects `INSERT` (it has no source row to insert), `BY TARGET` rejects
    /// `UPDATE`/`DELETE`.
    #[test]
    fn merge_not_matched_by_reject_boundaries() {
        for sql in [
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED BY SOURCE THEN DELETE",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY grp THEN DELETE",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN INSERT VALUES (1)",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY TARGET THEN UPDATE SET a = 1",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY TARGET THEN DELETE",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("invalid BY arm must reject: {sql:?}"));
        }
        // `DO NOTHING` stays valid on the BY SOURCE arm (probed).
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DO NOTHING",
            crate::ParseConfig::new(Postgres),
        )
        .expect("DO NOTHING is a valid BY SOURCE action");
    }

    /// The merge `INSERT DEFAULT VALUES` action (PostgreSQL; DuckDB, probed on 1.5.4)
    /// parses to its own variant and round-trips; it takes neither a column list nor
    /// an `OVERRIDING` clause (both engine-rejected, so both parse-reject here), and
    /// the standard-only `TestDialect` rejects the whole form
    /// (`merge_insert_default_values` off — not SQL:2016 surface).
    #[test]
    fn merge_insert_default_values_parses_and_round_trips() {
        let sql = "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT DEFAULT VALUES";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("INSERT DEFAULT VALUES parses");
        assert!(matches!(
            merge_of(&parsed).clauses[0].action,
            MergeAction::InsertDefault { .. }
        ));
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );

        for sql in [
            // A column list on DEFAULT VALUES is a PostgreSQL syntax error (probed).
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT (a) DEFAULT VALUES",
            // As is an OVERRIDING clause before it (probed).
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT \
             OVERRIDING USER VALUE DEFAULT VALUES",
            // And the MATCHED arm never takes an INSERT.
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN INSERT DEFAULT VALUES",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("must reject: {sql:?}"));
        }
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect_err("the standard merge insert has no DEFAULT VALUES form");
    }

    /// The merge insert `OVERRIDING {SYSTEM | USER} VALUE` override is SQL:2016
    /// standard surface, so it parses under PostgreSQL *and* the standard-only
    /// `TestDialect`, reusing the top-level `INSERT` node and keyword order (the
    /// clause sits between the column list and `VALUES`; a malformed variant
    /// rejects).
    #[test]
    fn merge_insert_overriding_parses_and_round_trips() {
        let sql = "MERGE INTO t USING s ON t.id = s.id \
                   WHEN NOT MATCHED THEN INSERT (a) OVERRIDING USER VALUE VALUES (s.a)";
        for dialect_sql in [
            sql,
            "MERGE INTO t USING s ON t.id = s.id \
             WHEN NOT MATCHED THEN INSERT OVERRIDING SYSTEM VALUE VALUES (s.a)",
        ] {
            let parsed = parse_with(dialect_sql, crate::ParseConfig::new(Postgres))
                .expect("OVERRIDING parses");
            let MergeAction::Insert { overriding, .. } = &merge_of(&parsed).clauses[0].action
            else {
                panic!("expected an INSERT action");
            };
            assert!(overriding.is_some());
            assert_eq!(
                Renderer::new(Postgres)
                    .render_parsed(&parsed)
                    .expect("the MERGE renders"),
                dialect_sql,
            );
        }
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect("the override clause is SQL:2016 standard surface");
        // Only SYSTEM/USER name an override source (probed).
        parse_with(
            "MERGE INTO t USING s ON t.id = s.id \
             WHEN NOT MATCHED THEN INSERT (a) OVERRIDING grp VALUE VALUES (1)",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("only SYSTEM or USER can be overridden");
    }

    /// `MERGE INTO ONLY t` / `MERGE INTO t *` reuse the shared `DmlTarget` relation
    /// shape, so the PostgreSQL inheritance markers ride the same
    /// `table_expressions.only` gate as `UPDATE`/`DELETE` targets: both spellings
    /// parse and round-trip under PostgreSQL, and the ANSI-only `TestDialect`
    /// rejects them.
    #[test]
    fn merge_into_only_target_parses_and_round_trips() {
        let sql = "MERGE INTO ONLY t AS m USING s ON m.id = s.id WHEN MATCHED THEN DELETE";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(Postgres)).expect("MERGE INTO ONLY parses");
        assert!(matches!(
            merge_of(&parsed).target.inheritance,
            RelationInheritance::Only(OnlySyntax::Bare)
        ));
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );

        let starred = "MERGE INTO t * USING s ON t.id = s.id WHEN MATCHED THEN DELETE";
        let parsed = parse_with(starred, crate::ParseConfig::new(Postgres))
            .expect("the descendant `*` parses");
        assert!(matches!(
            merge_of(&parsed).target.inheritance,
            RelationInheritance::Descendants
        ));

        for sql in [sql, starred] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("ANSI has no inheritance markers on a MERGE target");
        }
    }

    /// The `USING` source is SQL:2016's `<table reference>`, so a joined table is a
    /// valid source (PostgreSQL and DuckDB, both probed): the join chain parses onto
    /// the source's `TableWithJoins` and the *second* `ON` is the merge condition. A
    /// comma-separated source list stays rejected (probed — PostgreSQL takes one
    /// table reference).
    #[test]
    fn merge_using_joined_source_parses_and_round_trips() {
        let sql = "MERGE INTO t USING a JOIN b ON a.k = b.k ON t.id = a.id \
                   WHEN MATCHED THEN DELETE";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("a joined USING source parses");
        let merge = merge_of(&parsed);
        assert_eq!(merge.using.joins.len(), 1);
        assert!(matches!(merge.on, Expr::BinaryOp { .. }));
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );

        parse_with(
            "MERGE INTO t USING a, b ON t.id = a.id WHEN MATCHED THEN DELETE",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("the merge source is one table reference, never a list");
    }

    /// A leading `WITH` before `MERGE` (PostgreSQL 15+; DuckDB, probed on 1.5.4)
    /// parses onto the statement's own `with` slot and round-trips; the
    /// standard-only `TestDialect` rejects it (`cte_before_merge` off — SQL:2016's
    /// merge statement takes no `WITH` clause).
    #[test]
    fn merge_with_leading_cte_parses_and_round_trips() {
        let sql = "WITH src AS (SELECT 1 AS id) MERGE INTO t USING src ON t.id = src.id \
                   WHEN MATCHED THEN DELETE";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("WITH … MERGE parses under PostgreSQL");
        assert!(merge_of(&parsed).with.is_some());
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect_err("the standard merge statement takes no leading WITH");
    }

    /// `MERGE … RETURNING` (PostgreSQL 17+; DuckDB, probed on 1.5.4) rides the shared
    /// `returning` gate: it parses and round-trips under PostgreSQL, and the
    /// standard-only `TestDialect` (`returning` off) leaves the tail unconsumed.
    /// (PG 17's `merge_action()` output expression is a separate EXPR-family
    /// special form, not yet modelled.)
    #[test]
    fn merge_returning_parses_and_round_trips() {
        let sql = "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE \
                   RETURNING t.id, t.balance";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("MERGE … RETURNING parses under PostgreSQL");
        assert!(merge_of(&parsed).returning.is_some());
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the MERGE renders"),
            sql,
        );
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .expect_err("RETURNING is gated off in the ANSI baseline");
    }

    #[test]
    fn duckdb_rejects_qualified_update_set_targets() {
        use crate::dialect::{DuckDb, Postgres};
        parse_with("UPDATE t SET t.i = 1", crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB rejects qualified SET targets");
        parse_with("UPDATE t SET i = 1", crate::ParseConfig::new(DuckDb))
            .expect("DuckDB admits bare SET targets");
        parse_with("UPDATE t SET t.i = 1", crate::ParseConfig::new(Postgres))
            .expect("PostgreSQL admits qualified SET targets");
    }

    #[test]
    fn merge_duckdb_extensions_parse_and_gate() {
        use crate::ast::{MergeAction, Statement};
        use crate::dialect::{Ansi, DuckDb, Postgres};

        let cases = [
            (
                "MERGE INTO t USING s ON t.i = s.i WHEN MATCHED THEN UPDATE SET *",
                "UpdateStar",
            ),
            (
                "MERGE INTO t USING s ON t.i = s.i WHEN NOT MATCHED THEN INSERT *",
                "InsertStar",
            ),
            (
                "MERGE INTO t USING s ON t.i = s.i WHEN NOT MATCHED THEN INSERT BY NAME",
                "InsertByName",
            ),
            (
                "MERGE INTO t USING s ON t.i = s.i WHEN NOT MATCHED THEN INSERT BY NAME *",
                "InsertByNameStar",
            ),
            (
                "MERGE INTO t USING s ON t.i = s.i WHEN MATCHED THEN ERROR",
                "Error",
            ),
        ];
        for (sql, label) in cases {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|e| panic!("DuckDB parses {label}: {e:?}"));
            let Statement::Merge { merge, .. } = &parsed.statements()[0] else {
                panic!("expected Merge for {label}");
            };
            let action = &merge.clauses[0].action;
            match label {
                "UpdateStar" => assert!(matches!(action, MergeAction::UpdateStar { .. })),
                "InsertStar" => assert!(matches!(action, MergeAction::InsertStar { .. })),
                "InsertByName" => {
                    assert!(matches!(
                        action,
                        MergeAction::InsertByName { star: false, .. }
                    ))
                }
                "InsertByNameStar" => {
                    assert!(matches!(
                        action,
                        MergeAction::InsertByName { star: true, .. }
                    ))
                }
                "Error" => assert!(matches!(action, MergeAction::Error { .. })),
                _ => unreachable!(),
            }
            // Postgres/ANSI reject the DuckDB-only spellings
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("Postgres rejects {label}"));
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI rejects {label}"));
        }
    }

    #[test]
    fn duckdb_tranche2_small_surface_matches_engine() {
        use crate::ast::{ConflictResolution, InsertColumnMatching, Statement};
        use crate::dialect::{DuckDb, Postgres};

        // INSERT OR REPLACE (engine-accept)
        let sql = "INSERT OR REPLACE INTO t VALUES (1)";
        let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("INSERT OR REPLACE");
        let Statement::Insert { insert, .. } = &parsed.statements()[0] else {
            panic!("insert");
        };
        assert_eq!(insert.or_action, Some(ConflictResolution::Replace));
        parse_with(sql, crate::ParseConfig::new(Postgres)).expect_err("PG has no INSERT OR");

        // INSERT BY NAME (engine-accept; no column list)
        let sql = "INSERT INTO t BY NAME SELECT 1 AS a";
        let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("BY NAME");
        let Statement::Insert { insert, .. } = &parsed.statements()[0] else {
            panic!("insert");
        };
        assert_eq!(insert.column_matching, Some(InsertColumnMatching::ByName));
        parse_with(
            "INSERT INTO t (a) BY NAME SELECT 1 AS a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("BY NAME + column list rejected like DuckDB");

        // INSERT BY POSITION
        parse_with(
            "INSERT INTO t BY POSITION VALUES (1)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("BY POSITION");

        // Multi-col SET matching arity (engine Parser Error on mismatch)
        parse_with(
            "UPDATE t SET (a, b, c) = (1, 2, 3)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("matching arity");
        parse_with(
            "UPDATE t SET (a, b, c) = (1, 2)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("arity mismatch must reject like DuckDB");
        parse_with(
            "UPDATE t SET (a, b, c) = (1, 2, 3, 4)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("arity mismatch high");
        // PostgreSQL raw parsing accepts this surface; arity is semantic analysis there.
        parse_with(
            "UPDATE t SET (a, b, c) = (1, 2)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL parses value-row arity mismatches");
    }

    #[test]
    fn duckdb_glob_and_starts_with_operators() {
        use crate::ast::{BinaryOperator, Expr, SelectItem, SetExpr, Statement};
        use crate::dialect::{Ansi, DuckDb};

        let sql = "SELECT 'hello' GLOB 'h*'";
        let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("GLOB");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!();
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!();
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!();
        };
        let Expr::BinaryOp { op, .. } = expr else {
            panic!("{expr:?}");
        };
        assert_eq!(*op, BinaryOperator::Glob);
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no GLOB keyword op");

        let sql = "SELECT 'hello' ^@ 'he'";
        let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("^@");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!();
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!();
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!();
        };
        let Expr::BinaryOp { op, .. } = expr else {
            panic!("{expr:?}");
        };
        assert_eq!(*op, BinaryOperator::StartsWith);
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no ^@");
    }
}
