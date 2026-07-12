// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! MySQL stored-program compound-statement body grammar (SQL/PSM).
//!
//! A SEPARATE statement dispatcher from the top-level `parse_statement`
//! (`super::query`): body context admits the `[<label>:] BEGIN … END` compound block
//! (with its strict `DECLARE` prefix), the flow-control statements
//! (`IF`/`CASE`/`LOOP`/`WHILE`/`REPEAT`/`LEAVE`/`ITERATE`/`RETURN`) and the cursor
//! operations (`OPEN`/`FETCH`/`CLOSE`), none of which are top-level statements. The two
//! dispatchers are deliberately disjoint: a bare top-level `BEGIN … END` stays
//! transaction-start (the top-level dispatcher never reaches this grammar), so a
//! top-level compound rejects — the spike's probe-proven boundary.
//!
//! # Recursion and the stack cliff
//!
//! Body nesting (a block inside a block, an `IF` arm, a loop body) is a DIFFERENT
//! recursion axis from the expression hot frames (`super::expr`), which sit near the
//! debug-stack cliff. [`parse_body_statement`](Parser::parse_body_statement) routes
//! every nesting construct through the shared [`enter_recursion`](Parser::enter_recursion)
//! guard, and the cold dispatcher [`parse_body_construct`](Parser::parse_body_construct)
//! is `#[inline(never)]` so it can never be folded into the expression frames and
//! inflate their per-level budget — the constraint the compound-nesting canary in
//! `super::recursion` pins.
//!
//! # The routine/trigger/event seam
//!
//! [`parse_body_statement`](Parser::parse_body_statement) is `pub(super)`: it is the
//! single entry point the routine/trigger/event DDL wrappers (sibling tickets) invoke to
//! parse a routine body, and the entry point the body-grammar tests drive directly. It is
//! feature-gated by
//! [`StatementDdlGates::compound_statements`](crate::ast::dialect::StatementDdlGates), on
//! for MySQL (and Lenient); off elsewhere it rejects (no dialect-identity branch — the
//! gate is read as data).

use crate::ast::{
    CaseStatement, CloseCursorStatement, CompoundStatement, ConditionValue, ConditionalBranch,
    Declaration, FetchCursorStatement, HandlerAction, HandlerCondition, Ident, IfStatement,
    IterateStatement, LeaveStatement, LoopStatement, Meta, OpenCursorStatement, RepeatStatement,
    ReturnStatement, Span, Statement, WhileStatement,
};
use crate::error::ParseResult;
use crate::tokenizer::{Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;

/// Whether a block/loop label names an iterable loop or a plain compound block, the
/// distinction `ITERATE` resolution needs: `LEAVE` targets any labelled block or loop,
/// but `ITERATE` targets only a loop (`LOOP`/`WHILE`/`REPEAT`) — `ITERATE`ing a
/// `BEGIN … END` block label is the server's `ER_SP_LILABEL_MISMATCH` (1308), the same
/// reject as an unresolved label.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelKind {
    /// A `LOOP`/`WHILE`/`REPEAT` label — resolvable by both `LEAVE` and `ITERATE`.
    Loop,
    /// A `BEGIN … END` block label — resolvable by `LEAVE` only.
    Block,
}

/// One label active in an enclosing block/loop while parsing a nested body statement.
///
/// The stack of these is the lexical label scope MySQL's stored-program parser resolves
/// `LEAVE`/`ITERATE` against during the parse itself: a target is in scope iff it names a
/// frame here (case-insensitively — labels are case-insensitive), so a forward reference,
/// a closed sibling's label, or an unknown name all fail to resolve. The `span` records
/// the label's source text for that case-insensitive comparison (and for the
/// redefinition check); the label body is never empty (the tokenizer rejects a
/// zero-length delimited identifier before it reaches here).
#[derive(Clone, Copy)]
struct LabelFrame {
    /// Source span of the label identifier, compared case-insensitively as text.
    span: Span,
    /// Whether `ITERATE` may also target this label.
    kind: LabelKind,
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse one MySQL stored-program body statement — the `pub(super)` seam the
    /// routine/trigger/event wrappers and the body-grammar tests consume.
    ///
    /// The compound sub-language is gated by
    /// [`StatementDdlGates::compound_statements`](crate::ast::dialect::StatementDdlGates): off,
    /// the dispatcher rejects (a dialect with an opaque/string routine body never reaches
    /// this grammar). A body statement is either a body-only construct (dispatched here,
    /// under the recursion guard) or a plain SQL statement (delegated to the
    /// recursion-guarded top-level [`parse_statement`](Self::parse_statement)).
    pub(super) fn parse_body_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        // A fresh label scope per top-level body: threaded (not a parser field) so an
        // error unwind discards it wholesale, leaving no stale state for a later parse.
        let mut labels = ThinVec::new();
        self.parse_body_statement_scoped(&mut labels)
    }

    /// [`parse_body_statement`](Self::parse_body_statement) within an active label scope
    /// (`labels`): the enclosing block/loop labels, for the redefinition check.
    fn parse_body_statement_scoped(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
    ) -> ParseResult<Statement<D::Ext>> {
        if !self.features().statement_ddl_gates.compound_statements {
            return Err(self.unexpected(
                "a statement (compound-statement bodies are a stored-program feature)",
            ));
        }
        if self.peek_starts_body_construct()? {
            // Guard the body-nesting axis (distinct from the expression hot frames); the
            // cold `parse_body_construct` never inlines into those frames.
            let span = self.current_span()?;
            let mut guard = self.enter_recursion(span)?;
            guard.parser().parse_body_construct(labels)
        } else {
            // A plain SQL statement (`SELECT`/`INSERT`/`SET`/`CALL`/…); it self-guards.
            self.parse_statement()
        }
    }

    /// True if the current token opens a body-only construct: a `<label>:` prefix or one
    /// of the compound/flow-control/cursor leading keywords. `pub(super)` so the routine DDL
    /// wrappers can detect a compound/flow-control routine body (vs a bare `RETURN <expr>`).
    pub(super) fn peek_starts_body_construct(&mut self) -> ParseResult<bool> {
        // `<ident> :` is a labelled block or loop; no plain statement begins that way.
        if self.peek_nth_is_punct(1, Punctuation::Colon)? {
            return Ok(true);
        }
        Ok(self.peek_is_contextual_keyword("BEGIN")?
            || self.peek_is_contextual_keyword("IF")?
            || self.peek_is_contextual_keyword("CASE")?
            || self.peek_is_contextual_keyword("LOOP")?
            || self.peek_is_contextual_keyword("WHILE")?
            || self.peek_is_contextual_keyword("REPEAT")?
            || self.peek_is_contextual_keyword("LEAVE")?
            || self.peek_is_contextual_keyword("ITERATE")?
            || self.peek_is_contextual_keyword("RETURN")?
            || self.peek_is_contextual_keyword("OPEN")?
            || self.peek_is_contextual_keyword("FETCH")?
            || self.peek_is_contextual_keyword("CLOSE")?)
    }

    /// The cold body-statement dispatcher: parse the optional `<label>:` prefix, enforce
    /// the label-redefinition check, then dispatch on the construct keyword.
    ///
    /// `#[inline(never)]` (and never called from the expression grammar) so it stays a
    /// cold frame off the expression hot path — the stack-headroom constraint.
    #[inline(never)]
    fn parse_body_construct(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
    ) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let open_label = if self.peek_nth_is_punct(1, Punctuation::Colon)? {
            let label = self.parse_ident()?;
            self.expect_punct(Punctuation::Colon, "`:` after a block or loop label")?;
            Some(label)
        } else {
            None
        };
        if let Some(label) = &open_label {
            // Redefinition check (server class 1309, ER_SP_DUP_LABEL): a label already
            // live in an enclosing block/loop cannot be reused. Compared case-insensitively
            // because MySQL labels are case-insensitive (`lbl:` and `LBL:` collide), the
            // same rule LEAVE/ITERATE resolution below applies.
            if self.label_in_scope(labels, label.meta.span, false) {
                let found = self.span_text(label.meta.span).to_owned();
                return Err(self.error_at(
                    label.meta.span,
                    "a block or loop label not already active in an enclosing scope",
                    found,
                ));
            }
            // A label is only ever followed by `BEGIN` (a block) or `LOOP`/`WHILE`/`REPEAT`
            // (a loop); a label before anything else is rejected by `dispatch_body_construct`,
            // so the frame it pushes here is discarded on that error and its kind unused.
            let kind = if self.peek_is_contextual_keyword("BEGIN")? {
                LabelKind::Block
            } else {
                LabelKind::Loop
            };
            labels.push(LabelFrame {
                span: label.meta.span,
                kind,
            });
        }
        let result = self.dispatch_body_construct(labels, start, open_label.clone());
        if open_label.is_some() {
            // Balanced on the success path; on an error the whole scope is discarded on
            // unwind, so the extra pop is harmless.
            labels.pop();
        }
        result
    }

    fn dispatch_body_construct(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
        label: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        if self.peek_is_contextual_keyword("BEGIN")? {
            self.parse_compound_statement(labels, start, label)
        } else if self.peek_is_contextual_keyword("LOOP")? {
            self.parse_loop_statement(labels, start, label)
        } else if self.peek_is_contextual_keyword("WHILE")? {
            self.parse_while_statement(labels, start, label)
        } else if self.peek_is_contextual_keyword("REPEAT")? {
            self.parse_repeat_statement(labels, start, label)
        } else if label.is_some() {
            // A label may only precede a block or a loop.
            Err(self.unexpected("`BEGIN`, `LOOP`, `WHILE`, or `REPEAT` after a label"))
        } else if self.peek_is_contextual_keyword("IF")? {
            self.parse_if_statement(labels, start)
        } else if self.peek_is_contextual_keyword("CASE")? {
            self.parse_case_statement(labels, start)
        } else if self.peek_is_contextual_keyword("LEAVE")? {
            self.parse_leave_statement(labels, start)
        } else if self.peek_is_contextual_keyword("ITERATE")? {
            self.parse_iterate_statement(labels, start)
        } else if self.peek_is_contextual_keyword("RETURN")? {
            self.parse_return_statement(start)
        } else if self.peek_is_contextual_keyword("OPEN")? {
            self.parse_open_cursor_statement(start)
        } else if self.peek_is_contextual_keyword("FETCH")? {
            self.parse_fetch_cursor_statement(start)
        } else if self.peek_is_contextual_keyword("CLOSE")? {
            self.parse_close_cursor_statement(start)
        } else {
            Err(self.unexpected("a compound-body statement"))
        }
    }

    /// Parse `[<label>:] BEGIN [<declarations>] [<statements>] END [<label>]`. The label
    /// was already consumed by [`parse_body_construct`](Self::parse_body_construct).
    fn parse_compound_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
        label: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("BEGIN")?;
        let declarations = self.parse_declaration_prefix(labels)?;
        let body = self.parse_block_body(labels, &["END"])?;
        self.expect_contextual_keyword("END")?;
        let end_label = self.parse_optional_end_label()?;
        self.check_label_match(&label, &end_label)?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Compound {
            compound: Box::new(CompoundStatement {
                label,
                declarations,
                body,
                end_label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Parse the leading `DECLARE …` prefix, enforcing the strict ordering rule with a
    /// phase counter that mirrors the server's own: {variables, conditions} (mutually
    /// order-free) → cursors → handlers. A variable/condition after a cursor/handler is
    /// server class 1337; a cursor after a handler is class 1338.
    fn parse_declaration_prefix(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
    ) -> ParseResult<ThinVec<Declaration<D::Ext>>> {
        let mut declarations = ThinVec::new();
        // 0 = {variable, condition}, 1 = cursor, 2 = handler.
        let mut max_phase: u8 = 0;
        while self.peek_is_contextual_keyword("DECLARE")? {
            let span = self.current_span()?;
            let declaration = self.parse_declaration(labels)?;
            let phase = match &declaration {
                Declaration::Variable { .. } | Declaration::Condition { .. } => 0,
                Declaration::Cursor { .. } => 1,
                Declaration::Handler { .. } => 2,
            };
            if phase < max_phase {
                let message = if phase == 0 {
                    // class 1337
                    "a variable or condition declaration to precede every cursor and handler \
                     declaration"
                } else {
                    // class 1338 (phase == 1, so max_phase == 2)
                    "a cursor declaration to precede every handler declaration"
                };
                let found = self.span_text(span).to_owned();
                return Err(self.error_at(span, message, found));
            }
            max_phase = max_phase.max(phase);
            declarations.push(declaration);
            self.expect_punct(Punctuation::Semicolon, "`;` after a declaration")?;
        }
        Ok(declarations)
    }

    fn parse_declaration(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
    ) -> ParseResult<Declaration<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DECLARE")?;
        if self.peek_is_contextual_keyword("CONTINUE")?
            || self.peek_is_contextual_keyword("EXIT")?
            || self.peek_is_contextual_keyword("UNDO")?
        {
            return self.parse_handler_declaration(labels, start);
        }
        let first = self.parse_ident()?;
        if self.eat_contextual_keyword("CONDITION")? {
            self.expect_contextual_keyword("FOR")?;
            let value = self.parse_condition_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Declaration::Condition {
                name: first,
                value,
                meta,
            });
        }
        if self.eat_contextual_keyword("CURSOR")? {
            self.expect_contextual_keyword("FOR")?;
            let query = self.parse_query()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Declaration::Cursor {
                name: first,
                query: Box::new(query),
                meta,
            });
        }
        // A variable declaration: one or more names sharing a type and optional default.
        let mut names = thin_vec![first];
        while self.eat_punct(Punctuation::Comma)? {
            names.push(self.parse_ident()?);
        }
        let data_type = self.parse_data_type()?;
        let default = if self.eat_contextual_keyword("DEFAULT")? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Declaration::Variable {
            names,
            data_type,
            default,
            meta,
        })
    }

    fn parse_handler_declaration(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
    ) -> ParseResult<Declaration<D::Ext>> {
        let action = if self.eat_contextual_keyword("CONTINUE")? {
            HandlerAction::Continue
        } else if self.eat_contextual_keyword("EXIT")? {
            HandlerAction::Exit
        } else {
            self.expect_contextual_keyword("UNDO")?;
            HandlerAction::Undo
        };
        self.expect_contextual_keyword("HANDLER")?;
        self.expect_contextual_keyword("FOR")?;
        let conditions = self.parse_comma_separated(Self::parse_handler_condition)?;
        // The handler body is a single body statement (often a compound block).
        let body = self.parse_body_statement_scoped(labels)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Declaration::Handler {
            action,
            conditions,
            body: Box::new(body),
            meta,
        })
    }

    /// Parse a `DECLARE … CONDITION FOR` value: `SQLSTATE [VALUE] '…'` or an error code.
    fn parse_condition_value(&mut self) -> ParseResult<ConditionValue> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SQLSTATE")? {
            let value_keyword = self.eat_contextual_keyword("VALUE")?;
            let sqlstate = self.expect_string_literal("a SQLSTATE string literal")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConditionValue::SqlState {
                value_keyword,
                sqlstate,
                meta,
            })
        } else {
            let code = self.expect_unsigned_integer_literal("a MySQL error code")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConditionValue::ErrorCode { code, meta })
        }
    }

    /// Parse one `HANDLER FOR` condition: `SQLSTATE [VALUE] '…'`, an error code,
    /// `SQLWARNING` / `NOT FOUND` / `SQLEXCEPTION`, or a declared condition name.
    fn parse_handler_condition(&mut self) -> ParseResult<HandlerCondition> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SQLSTATE")? {
            let value_keyword = self.eat_contextual_keyword("VALUE")?;
            let sqlstate = self.expect_string_literal("a SQLSTATE string literal")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(HandlerCondition::SqlState {
                value_keyword,
                sqlstate,
                meta,
            })
        } else if self.eat_contextual_keyword("SQLWARNING")? {
            Ok(HandlerCondition::SqlWarning {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("SQLEXCEPTION")? {
            Ok(HandlerCondition::SqlException {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("NOT")? {
            self.expect_contextual_keyword("FOUND")?;
            Ok(HandlerCondition::NotFound {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Number)
        ) {
            let code = self.expect_unsigned_integer_literal("a MySQL error code")?;
            Ok(HandlerCondition::ErrorCode {
                code,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else {
            let name = self.parse_ident()?;
            Ok(HandlerCondition::ConditionName {
                name,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        }
    }

    /// Parse `IF <cond> THEN … [ELSEIF <cond> THEN …] [ELSE …] END IF`.
    fn parse_if_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("IF")?;
        let mut branches =
            thin_vec![self.parse_conditional_branch(labels, &["ELSEIF", "ELSE", "END"])?];
        while self.eat_contextual_keyword("ELSEIF")? {
            branches.push(self.parse_conditional_branch(labels, &["ELSEIF", "ELSE", "END"])?);
        }
        let else_body = if self.eat_contextual_keyword("ELSE")? {
            Some(self.parse_block_body(labels, &["END"])?)
        } else {
            None
        };
        self.expect_contextual_keyword("END")?;
        self.expect_contextual_keyword("IF")?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::If {
            if_statement: Box::new(IfStatement {
                branches,
                else_body,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Parse `CASE [<operand>] WHEN … THEN … [ELSE …] END CASE` (simple when an operand
    /// is present, searched otherwise).
    fn parse_case_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("CASE")?;
        let operand = if self.peek_is_contextual_keyword("WHEN")? {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };
        self.expect_contextual_keyword("WHEN")?;
        let mut when_branches =
            thin_vec![self.parse_conditional_branch(labels, &["WHEN", "ELSE", "END"])?];
        while self.eat_contextual_keyword("WHEN")? {
            when_branches.push(self.parse_conditional_branch(labels, &["WHEN", "ELSE", "END"])?);
        }
        let else_body = if self.eat_contextual_keyword("ELSE")? {
            Some(self.parse_block_body(labels, &["END"])?)
        } else {
            None
        };
        self.expect_contextual_keyword("END")?;
        self.expect_contextual_keyword("CASE")?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Case {
            case_statement: Box::new(CaseStatement {
                operand,
                when_branches,
                else_body,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Parse one `<guard> THEN <body>` arm shared by `IF`/`ELSEIF` and searched/simple
    /// `CASE` `WHEN`; `terminators` are the keywords that end this arm's body.
    fn parse_conditional_branch(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        terminators: &[&'static str],
    ) -> ParseResult<ConditionalBranch<D::Ext>> {
        let start = self.current_span()?;
        let guard = self.parse_expr()?;
        self.expect_contextual_keyword("THEN")?;
        let body = self.parse_block_body(labels, terminators)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ConditionalBranch { guard, body, meta })
    }

    fn parse_loop_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
        label: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("LOOP")?;
        let body = self.parse_block_body(labels, &["END"])?;
        self.expect_contextual_keyword("END")?;
        self.expect_contextual_keyword("LOOP")?;
        let end_label = self.parse_optional_end_label()?;
        self.check_label_match(&label, &end_label)?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Loop {
            loop_statement: Box::new(LoopStatement {
                label,
                body,
                end_label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_while_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
        label: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("WHILE")?;
        let condition = self.parse_expr()?;
        self.expect_contextual_keyword("DO")?;
        let body = self.parse_block_body(labels, &["END"])?;
        self.expect_contextual_keyword("END")?;
        self.expect_contextual_keyword("WHILE")?;
        let end_label = self.parse_optional_end_label()?;
        self.check_label_match(&label, &end_label)?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::While {
            while_statement: Box::new(WhileStatement {
                label,
                condition: Box::new(condition),
                body,
                end_label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_repeat_statement(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        start: Span,
        label: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("REPEAT")?;
        let body = self.parse_block_body(labels, &["UNTIL"])?;
        self.expect_contextual_keyword("UNTIL")?;
        let condition = self.parse_expr()?;
        self.expect_contextual_keyword("END")?;
        self.expect_contextual_keyword("REPEAT")?;
        let end_label = self.parse_optional_end_label()?;
        self.check_label_match(&label, &end_label)?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Repeat {
            repeat: Box::new(RepeatStatement {
                label,
                body,
                condition: Box::new(condition),
                end_label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_leave_statement(
        &mut self,
        labels: &ThinVec<LabelFrame>,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("LEAVE")?;
        let label = self.parse_ident()?;
        // Resolve against the enclosing label scope (server class 1308,
        // ER_SP_LILABEL_MISMATCH): `LEAVE` targets any labelled block or loop lexically in
        // scope, so an unknown name, a forward reference, or a closed sibling's label all
        // reject here.
        if !self.label_in_scope(labels, label.meta.span, false) {
            let found = self.span_text(label.meta.span).to_owned();
            return Err(self.error_at(
                label.meta.span,
                "a LEAVE target naming a block or loop label active in an enclosing scope",
                found,
            ));
        }
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Leave {
            leave: Box::new(LeaveStatement {
                label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_iterate_statement(
        &mut self,
        labels: &ThinVec<LabelFrame>,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("ITERATE")?;
        let label = self.parse_ident()?;
        // Resolve against the enclosing label scope (server class 1308,
        // ER_SP_LILABEL_MISMATCH): `ITERATE` targets only a *loop* label in scope — an
        // unknown name and a `BEGIN … END` block label alike reject here (the loop-only
        // restriction is `ITERATE`'s sole difference from `LEAVE`).
        if !self.label_in_scope(labels, label.meta.span, true) {
            let found = self.span_text(label.meta.span).to_owned();
            return Err(self.error_at(
                label.meta.span,
                "an ITERATE target naming a loop label active in an enclosing scope",
                found,
            ));
        }
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Iterate {
            iterate: Box::new(IterateStatement {
                label,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Whether a label named by the source text at `name` is active in the enclosing
    /// scope `labels`, comparing case-insensitively (MySQL labels are case-insensitive).
    /// When `require_loop` (an `ITERATE` target), only a [`LabelKind::Loop`] frame
    /// matches; otherwise (a `LEAVE` target, or the redefinition check) any frame does.
    /// Active label names are unique (the class-1309 redefinition check enforces it), so
    /// at most one frame ever matches.
    fn label_in_scope(&self, labels: &ThinVec<LabelFrame>, name: Span, require_loop: bool) -> bool {
        let target = self.span_text(name);
        labels.iter().any(|frame| {
            (!require_loop || frame.kind == LabelKind::Loop)
                && self.span_text(frame.span).eq_ignore_ascii_case(target)
        })
    }

    fn parse_return_statement(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let return_span = self.current_span()?;
        self.expect_contextual_keyword("RETURN")?;
        // `RETURN` is a stored-**function** construct: the server rejects it in a procedure
        // (or trigger/event) body with `ER_SP_BADRETURN`. The routine wrapper narrows
        // `body_return_allowed` to `false` for a procedure body, so a `RETURN` anywhere within
        // it — top-level or nested in a block/loop/handler — rejects here.
        if !self.body_return_allowed {
            let found = self.span_text(return_span).to_owned();
            return Err(self.error_at(
                return_span,
                "a stored-function body (RETURN is not allowed outside a function)",
                found,
            ));
        }
        let value = self.parse_expr()?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::Return {
            return_statement: Box::new(ReturnStatement {
                value,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_open_cursor_statement(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("OPEN")?;
        let cursor = self.parse_ident()?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::OpenCursor {
            open: Box::new(OpenCursorStatement {
                cursor,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_close_cursor_statement(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("CLOSE")?;
        let cursor = self.parse_ident()?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::CloseCursor {
            close: Box::new(CloseCursorStatement {
                cursor,
                meta: inner_meta,
            }),
            meta,
        })
    }

    fn parse_fetch_cursor_statement(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("FETCH")?;
        let next_keyword = self.eat_contextual_keyword("NEXT")?;
        let from_keyword = self.eat_contextual_keyword("FROM")?;
        if next_keyword && !from_keyword {
            // The grammar is `FETCH [[NEXT] FROM]`: `NEXT` requires `FROM`.
            return Err(self.unexpected("`FROM` after `NEXT`"));
        }
        let cursor = self.parse_ident()?;
        self.expect_contextual_keyword("INTO")?;
        let targets = self.parse_comma_separated(Self::parse_ident)?;
        let (inner_meta, meta) = self.body_meta_pair(start);
        Ok(Statement::FetchCursor {
            fetch: Box::new(FetchCursorStatement {
                next_keyword,
                from_keyword,
                cursor,
                targets,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Parse a `;`-terminated body statement list up to (not including) any of
    /// `terminators` — the shared shape of a block, an `IF`/`CASE` arm, and a loop body.
    /// Each element routes through [`parse_body_statement_scoped`](Self::parse_body_statement_scoped),
    /// so a nested construct re-enters the recursion guard for its own level.
    fn parse_block_body(
        &mut self,
        labels: &mut ThinVec<LabelFrame>,
        terminators: &[&'static str],
    ) -> ParseResult<ThinVec<Statement<D::Ext>>> {
        let mut body = ThinVec::new();
        while self.peek()?.is_some() && !self.peek_is_any_contextual_keyword(terminators)? {
            let statement = self.parse_body_statement_scoped(labels)?;
            self.expect_punct(
                Punctuation::Semicolon,
                "`;` to terminate a compound-body statement",
            )?;
            body.push(statement);
        }
        Ok(body)
    }

    /// Parse an optional trailing block/loop label (`END … <label>`): present iff the next
    /// token is word-like — after a block/loop close only a `;` terminator or the label
    /// can follow, so a word there is unambiguously the label.
    fn parse_optional_end_label(&mut self) -> ParseResult<Option<Ident>> {
        let is_label = matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Word | TokenKind::QuotedIdent | TokenKind::Keyword(_))
        );
        if is_label {
            Ok(Some(self.parse_ident()?))
        } else {
            Ok(None)
        }
    }

    /// Reject an `END <label>` whose label does not match the opening label (MySQL
    /// surfaces this as a generic syntax error). Compared on source text,
    /// case-insensitively, so `outer` / `OUTER` match.
    fn check_label_match(
        &mut self,
        open: &Option<Ident>,
        close: &Option<Ident>,
    ) -> ParseResult<()> {
        if let (Some(open), Some(close)) = (open, close) {
            let matches = self
                .span_text(open.meta.span)
                .eq_ignore_ascii_case(self.span_text(close.meta.span));
            if !matches {
                let found = self.span_text(close.meta.span).to_owned();
                return Err(self.error_at(
                    close.meta.span,
                    "an end label matching the opening block label",
                    found,
                ));
            }
        }
        Ok(())
    }

    /// True if the current token is any of the given contextual keywords.
    fn peek_is_any_contextual_keyword(&mut self, words: &[&'static str]) -> ParseResult<bool> {
        for &word in words {
            if self.peek_is_contextual_keyword(word)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A pair of fresh [`Meta`] over `start..preceding` — one for a boxed body payload and
    /// one for its wrapping [`Statement`] variant, each a distinct node id (the
    /// boxed-statement idiom, as in `parse_alter_statement`).
    fn body_meta_pair(&mut self, start: Span) -> (Meta, Meta) {
        let span = start.union(self.preceding_span());
        (self.make_meta(span), self.make_meta(span))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::FeatureSet;
    use crate::ast::{
        CaseStatement, CompoundStatement, ConditionValue, Declaration, HandlerAction,
        HandlerCondition, IfStatement, Statement,
    };
    use crate::error::ParseErrorKind;
    use crate::parser::{FeatureDialect, Parser};
    use crate::render::Renderer;
    use crate::tokenizer::tokenize;

    /// The MySQL feature preset (`compound_statements` on) as a data-only test dialect.
    const MYSQL: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };
    /// ANSI (`compound_statements` off) — the gate-off reject dialect.
    const ANSI: FeatureDialect = FeatureDialect {
        features: &FeatureSet::ANSI,
    };

    /// Parse a body fragment as one body statement under the MySQL preset.
    fn parse_body(src: &str) -> Statement {
        let tokens = tokenize(src).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, MYSQL);
        parser
            .parse_body_statement()
            .unwrap_or_else(|err| panic!("{src}: {err:?}"))
    }

    /// Parse a body fragment, returning the reject for the negative pins.
    fn reject_body(src: &str, dialect: FeatureDialect) -> ParseErrorKind {
        let tokens = tokenize(src).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, dialect);
        parser
            .parse_body_statement()
            .expect_err(&format!("{src} must reject"))
            .kind
    }

    /// Parse, render, and re-parse a body fragment; the AST must be structurally stable
    /// across the round-trip (`Meta` is excluded from equality).
    fn assert_round_trips(src: &str) {
        let tokens = tokenize(src).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, MYSQL);
        let first = parser
            .parse_body_statement()
            .unwrap_or_else(|err| panic!("{src}: {err:?}"));
        let resolver = parser.finish();
        let rendered = Renderer::new(MYSQL)
            .render_statement(&first, &resolver, src)
            .unwrap_or_else(|err| panic!("{src}: render {err}"));
        let second = parse_body(&rendered);
        assert_eq!(
            first, second,
            "round-trip changed the AST\n  in:  {src}\n  out: {rendered}"
        );
    }

    #[test]
    fn empty_compound_block_parses() {
        let Statement::Compound { compound, .. } = parse_body("BEGIN END") else {
            panic!("expected a compound block");
        };
        assert!(compound.declarations.is_empty());
        assert!(compound.body.is_empty());
        assert!(compound.label.is_none() && compound.end_label.is_none());
    }

    #[test]
    fn compound_block_with_declarations_and_body_parses() {
        let src = "BEGIN DECLARE x INT DEFAULT 0; DECLARE y, z INT; SELECT x; SELECT y; END";
        let Statement::Compound { compound, .. } = parse_body(src) else {
            panic!("expected a compound block");
        };
        let CompoundStatement {
            declarations, body, ..
        } = *compound;
        assert_eq!(declarations.len(), 2, "two DECLARE items");
        assert_eq!(body.len(), 2, "two body statements");
        let Declaration::Variable { names, default, .. } = &declarations[0] else {
            panic!("first declaration is a variable");
        };
        assert_eq!(names.len(), 1);
        assert!(default.is_some(), "DEFAULT parsed");
        let Declaration::Variable { names, default, .. } = &declarations[1] else {
            panic!("second declaration is a variable");
        };
        assert_eq!(names.len(), 2, "two names share a type");
        assert!(default.is_none());
    }

    #[test]
    fn labelled_compound_block_parses() {
        let Statement::Compound { compound, .. } = parse_body("blk: BEGIN SELECT 1; END blk")
        else {
            panic!("expected a compound block");
        };
        assert!(compound.label.is_some());
        assert!(compound.end_label.is_some());
    }

    #[test]
    fn all_declaration_forms_parse_in_order() {
        let src = "BEGIN \
            DECLARE v INT; \
            DECLARE c CONDITION FOR SQLSTATE '42S02'; \
            DECLARE cur CURSOR FOR SELECT 1; \
            DECLARE CONTINUE HANDLER FOR NOT FOUND SET done = 1; \
            SELECT 1; END";
        let Statement::Compound { compound, .. } = parse_body(src) else {
            panic!("expected a compound block");
        };
        assert_eq!(compound.declarations.len(), 4);
        assert!(matches!(
            compound.declarations[0],
            Declaration::Variable { .. }
        ));
        assert!(matches!(
            compound.declarations[1],
            Declaration::Condition { .. }
        ));
        assert!(matches!(
            compound.declarations[2],
            Declaration::Cursor { .. }
        ));
        let Declaration::Handler {
            action, conditions, ..
        } = &compound.declarations[3]
        else {
            panic!("fourth declaration is a handler");
        };
        assert_eq!(*action, HandlerAction::Continue);
        assert!(matches!(conditions[0], HandlerCondition::NotFound { .. }));
    }

    #[test]
    fn condition_declaration_accepts_error_code_and_sqlstate() {
        let Statement::Compound { compound, .. } = parse_body(
            "BEGIN DECLARE a CONDITION FOR 1051; DECLARE b CONDITION FOR SQLSTATE VALUE '42S02'; SELECT 1; END",
        ) else {
            panic!("expected a compound block");
        };
        assert!(matches!(
            compound.declarations[0],
            Declaration::Condition {
                value: ConditionValue::ErrorCode { .. },
                ..
            }
        ));
        assert!(matches!(
            compound.declarations[1],
            Declaration::Condition {
                value: ConditionValue::SqlState {
                    value_keyword: true,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn cursor_ops_parse() {
        let src = "BEGIN OPEN cur; FETCH cur INTO x; FETCH NEXT FROM cur INTO x, y; CLOSE cur; END";
        let Statement::Compound { compound, .. } = parse_body(src) else {
            panic!("expected a compound block");
        };
        assert!(matches!(compound.body[0], Statement::OpenCursor { .. }));
        assert!(matches!(compound.body[1], Statement::FetchCursor { .. }));
        assert!(matches!(compound.body[3], Statement::CloseCursor { .. }));
        let Statement::FetchCursor { fetch, .. } = &compound.body[2] else {
            panic!("third body statement is a FETCH");
        };
        assert!(fetch.next_keyword && fetch.from_keyword);
        assert_eq!(fetch.targets.len(), 2);
    }

    #[test]
    fn if_elseif_else_parses() {
        let Statement::If { if_statement, .. } =
            parse_body("IF x > 0 THEN SELECT 1; ELSEIF x < 0 THEN SELECT 2; ELSE SELECT 3; END IF")
        else {
            panic!("expected an IF statement");
        };
        let IfStatement {
            branches,
            else_body,
            ..
        } = *if_statement;
        assert_eq!(branches.len(), 2, "IF + one ELSEIF");
        assert!(else_body.is_some());
    }

    #[test]
    fn simple_and_searched_case_parse() {
        let Statement::Case { case_statement, .. } = parse_body(
            "CASE x WHEN 1 THEN SELECT 1; WHEN 2 THEN SELECT 2; ELSE SELECT 3; END CASE",
        ) else {
            panic!("expected a CASE statement");
        };
        let CaseStatement {
            operand,
            when_branches,
            else_body,
            ..
        } = *case_statement;
        assert!(operand.is_some(), "simple CASE carries an operand");
        assert_eq!(when_branches.len(), 2);
        assert!(else_body.is_some());

        let Statement::Case { case_statement, .. } =
            parse_body("CASE WHEN x > 0 THEN SELECT 1; END CASE")
        else {
            panic!("expected a CASE statement");
        };
        assert!(
            case_statement.operand.is_none(),
            "searched CASE has no operand"
        );
    }

    #[test]
    fn loops_and_labels_parse() {
        assert!(matches!(
            parse_body("lp: LOOP LEAVE lp; END LOOP lp"),
            Statement::Loop { .. }
        ));
        assert!(matches!(
            parse_body("WHILE x > 0 DO SELECT 1; END WHILE"),
            Statement::While { .. }
        ));
        assert!(matches!(
            parse_body("REPEAT SELECT 1; UNTIL x > 0 END REPEAT"),
            Statement::Repeat { .. }
        ));
        assert!(matches!(
            parse_body("wl: WHILE x > 0 DO ITERATE wl; END WHILE wl"),
            Statement::While { .. }
        ));
    }

    #[test]
    fn nested_compound_blocks_parse() {
        let Statement::Compound { compound, .. } =
            parse_body("BEGIN BEGIN SELECT 1; END; SELECT 2; END")
        else {
            panic!("expected a compound block");
        };
        assert_eq!(compound.body.len(), 2);
        assert!(matches!(compound.body[0], Statement::Compound { .. }));
    }

    // --- Reject pins (spike-probed server behaviour) ---------------------------

    #[test]
    fn top_level_compound_is_not_a_statement() {
        // The top-level dispatcher never reaches the compound grammar (a bare top-level
        // `BEGIN` is transaction-start), so `BEGIN SELECT 1; END` as a whole script
        // rejects — the server's 1064-class boundary. `parse_with` walks the whole script:
        // `BEGIN` parses as a transaction start, `SELECT 1` as a query, and the trailing
        // `END` is an unknown statement.
        use crate::parser::parse_with;
        let err = parse_with("BEGIN SELECT 1; END", MYSQL)
            .expect_err("a top-level compound block must reject");
        assert_eq!(err.kind, ParseErrorKind::Syntax);
    }

    #[test]
    fn compound_body_is_gated_off_under_ansi() {
        // Without `compound_statements` (ANSI) the body dispatcher rejects the compound
        // grammar rather than branching on dialect identity.
        assert_eq!(
            reject_body("BEGIN SELECT 1; END", ANSI),
            ParseErrorKind::Syntax
        );
    }

    #[test]
    fn declare_ordering_is_enforced() {
        // Variable/condition after a cursor → class 1337; cursor after a handler → 1338.
        assert_eq!(
            reject_body(
                "BEGIN DECLARE cur CURSOR FOR SELECT 1; DECLARE x INT; SELECT 1; END",
                MYSQL
            ),
            ParseErrorKind::Syntax,
            "a variable after a cursor rejects (1337 class)"
        );
        assert_eq!(
            reject_body(
                "BEGIN DECLARE EXIT HANDLER FOR SQLWARNING SET x = 1; DECLARE cur CURSOR FOR SELECT 1; SELECT 1; END",
                MYSQL
            ),
            ParseErrorKind::Syntax,
            "a cursor after a handler rejects (1338 class)"
        );
        // {variable, condition} are mutually order-free — a condition before a variable
        // is accepted.
        assert!(matches!(
            parse_body("BEGIN DECLARE c CONDITION FOR 1051; DECLARE v INT; SELECT 1; END"),
            Statement::Compound { .. }
        ));
    }

    #[test]
    fn duplicate_label_in_scope_rejects() {
        // A label live in an enclosing loop cannot be redefined by a nested one (class
        // 1309).
        assert_eq!(
            reject_body(
                "lbl: LOOP lbl: LOOP LEAVE lbl; END LOOP lbl; END LOOP lbl",
                MYSQL
            ),
            ParseErrorKind::Syntax,
        );
    }

    #[test]
    fn leave_and_iterate_resolve_labels_across_nesting() {
        // A loop label is reachable by both LEAVE and ITERATE from its own body.
        assert!(matches!(
            parse_body("lp: LOOP LEAVE lp; END LOOP lp"),
            Statement::Loop { .. }
        ));
        assert!(matches!(
            parse_body("lp: LOOP ITERATE lp; END LOOP lp"),
            Statement::Loop { .. }
        ));
        // LEAVE resolves a BEGIN…END block label (not just loops).
        assert!(matches!(
            parse_body("blk: BEGIN LEAVE blk; END blk"),
            Statement::Compound { .. }
        ));
        // An enclosing label is visible from a deeper nested body: LEAVE the outer block
        // from inside a nested loop.
        assert!(matches!(
            parse_body("blk: BEGIN lp: LOOP LEAVE blk; END LOOP lp; END blk"),
            Statement::Compound { .. }
        ));
        // WHILE and REPEAT labels resolve for ITERATE too.
        assert!(matches!(
            parse_body("wl: WHILE x > 0 DO ITERATE wl; END WHILE wl"),
            Statement::While { .. }
        ));
        assert!(matches!(
            parse_body("rp: REPEAT ITERATE rp; UNTIL x > 0 END REPEAT rp"),
            Statement::Repeat { .. }
        ));
        // Labels are case-insensitive (`LP` resolves `lp:`).
        assert!(matches!(
            parse_body("lp: LOOP LEAVE LP; END LOOP lp"),
            Statement::Loop { .. }
        ));
        // The same label name is reusable across disjoint (closed) sibling scopes.
        assert!(matches!(
            parse_body("BEGIN l: LOOP LEAVE l; END LOOP l; l: LOOP LEAVE l; END LOOP l; END"),
            Statement::Compound { .. }
        ));
    }

    #[test]
    fn undeclared_leave_iterate_label_rejects() {
        // LEAVE / ITERATE a label that names no enclosing block or loop (server class 1308,
        // ER_SP_LILABEL_MISMATCH).
        assert_eq!(
            reject_body("BEGIN LEAVE nope; END", MYSQL),
            ParseErrorKind::Syntax,
            "LEAVE of an unknown label rejects",
        );
        assert_eq!(
            reject_body("BEGIN ITERATE nope; END", MYSQL),
            ParseErrorKind::Syntax,
            "ITERATE of an unknown label rejects",
        );
        // No forward reference: a label declared by a *later* sibling is not in scope.
        assert_eq!(
            reject_body("BEGIN LEAVE lp; lp: LOOP SELECT 1; END LOOP lp; END", MYSQL),
            ParseErrorKind::Syntax,
        );
        // A closed sibling loop's label is out of scope after it ends.
        assert_eq!(
            reject_body("BEGIN lp: LOOP SELECT 1; END LOOP lp; LEAVE lp; END", MYSQL),
            ParseErrorKind::Syntax,
        );
    }

    #[test]
    fn iterate_rejects_a_block_label() {
        // ITERATE targets only a loop; a BEGIN…END block label is not iterable (server
        // class 1308, the loop-only restriction — LEAVE of the same label accepts).
        assert_eq!(
            reject_body("blk: BEGIN ITERATE blk; END blk", MYSQL),
            ParseErrorKind::Syntax,
        );
        // The block is iterable-restricted even when referenced from a nested loop.
        assert_eq!(
            reject_body(
                "blk: BEGIN lp: LOOP ITERATE blk; END LOOP lp; END blk",
                MYSQL
            ),
            ParseErrorKind::Syntax,
        );
    }

    #[test]
    fn duplicate_label_check_is_case_insensitive() {
        // `LBL` shadowing `lbl` is a class-1309 redefinition — labels are case-insensitive,
        // so the nested reuse collides even across a case change.
        assert_eq!(
            reject_body(
                "lbl: LOOP LBL: LOOP LEAVE LBL; END LOOP LBL; END LOOP lbl",
                MYSQL
            ),
            ParseErrorKind::Syntax,
        );
    }

    #[test]
    fn mismatched_end_label_rejects() {
        // Non-reserved labels, so the reject is the label mismatch (not a reserved word in
        // identifier position).
        assert_eq!(
            reject_body("blk: BEGIN SELECT 1; END other", MYSQL),
            ParseErrorKind::Syntax,
        );
    }

    // --- Render round-trips ----------------------------------------------------

    #[test]
    fn body_statements_round_trip() {
        for src in [
            "BEGIN END",
            "BEGIN SELECT 1; SELECT 2; END",
            // Canonical type spellings so the round-trip isolates the compound structure
            // from the (orthogonal, existing) `INT`->`INTEGER` type-spelling canonicalization.
            "BEGIN DECLARE x INTEGER DEFAULT 0; SELECT x; END",
            "BEGIN DECLARE a, b INTEGER; SELECT 1; END",
            "BEGIN DECLARE c CONDITION FOR SQLSTATE '42S02'; SELECT 1; END",
            "BEGIN DECLARE cur CURSOR FOR SELECT 1; OPEN cur; CLOSE cur; END",
            // A `SELECT` handler body: canonical, so the round-trip isolates the handler
            // declaration's render from the (orthogonal) session-`SET` `=`->`TO` spelling.
            "BEGIN DECLARE CONTINUE HANDLER FOR NOT FOUND SELECT 1; SELECT 1; END",
            "IF x > 0 THEN SELECT 1; ELSEIF x < 0 THEN SELECT 2; ELSE SELECT 3; END IF",
            "CASE x WHEN 1 THEN SELECT 1; ELSE SELECT 2; END CASE",
            "CASE WHEN x > 0 THEN SELECT 1; END CASE",
            "lp: LOOP LEAVE lp; END LOOP lp",
            "wl: WHILE x > 0 DO ITERATE wl; END WHILE wl",
            "REPEAT SELECT 1; UNTIL x > 0 END REPEAT",
            "BEGIN FETCH NEXT FROM cur INTO x, y; END",
            "RETURN x + 1",
        ] {
            assert_round_trips(src);
        }
    }
}
