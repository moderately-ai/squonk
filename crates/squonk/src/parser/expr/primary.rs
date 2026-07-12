// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The prefix/primary dispatch of the Pratt core: the token-class router
//! [`parse_prefix`](crate::parser::Parser::parse_prefix), the prefix-operator class,
//! the parenthesized group / scalar subquery, and the `AexprConst` constant subset.
//!
//! STACK-FRAME RULE: `parse_prefix` sits on the per-nesting-level recursive frame
//! (with `parse_expr_bp_inner` and `parse_grouped`), so it stays a thin router —
//! every token class's guard and arm scratch lives in a called-and-returned
//! `#[inline(never)]` per-class dispatcher ([`parse_keyword_prefix`] in
//! `keyword_forms.rs`, [`parse_word_or_literal_prefix`] in `literals.rs`,
//! [`parse_operator_prefix`] here), and the paren/collection openers route straight
//! to their parsers. Inlining an arm body back into the router (or into
//! `parse_expr_bp_inner`) regresses the per-level stack budget the
//! `high_but_safe_nesting` canary and the 1.6 MB drift sentinel in
//! `parser::recursion` guard.
//!
//! [`parse_keyword_prefix`]: crate::parser::Parser::parse_keyword_prefix
//! [`parse_word_or_literal_prefix`]: crate::parser::Parser::parse_word_or_literal_prefix
//! [`parse_operator_prefix`]: crate::parser::Parser::parse_operator_prefix

use super::ParsedExpr;
use super::number_literal_kind;
use crate::ast::{
    ColumnsSpelling, Expr, Keyword, Literal, LiteralKind, PrefixOperatorExpr, Query, RowExpr,
    Spanned, UnaryOperator,
};
use crate::error::ParseResult;
use crate::parser::engine::Parser;
use crate::parser::{Dialect, HookResult};
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use thin_vec::thin_vec;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse a prefix unary chain or a primary expression.
    ///
    /// A thin token-class router (see the module doc's STACK-FRAME RULE): the dialect
    /// hooks run first, then the leading token's class selects a per-class
    /// `#[inline(never)]` dispatcher. The paren/collection openers route straight to
    /// their parsers so the hot parenthesized path never carries the keyword classes'
    /// arm scratch.
    pub(super) fn parse_prefix(&mut self) -> ParseResult<ParsedExpr<D::Ext>> {
        match D::parse_prefix_expr_hook(self) {
            HookResult::Handled(expr) => return Ok(ParsedExpr::bare(expr)),
            HookResult::NotHandled => {}
            HookResult::Err(error) => return Err(error),
        }

        // A dialect may define a custom prefix operator (as opposed to a whole
        // custom primary, above): the parser climbs its operand at the hook-provided
        // binding power, so the operand honours precedence by construction (ADR-0008).
        if let Some(prefix_bp) = D::peek_prefix_operator_hook(self)? {
            return self
                .parse_hook_prefix_operator(prefix_bp)
                .map(ParsedExpr::bare);
        }

        // `peek` yields a `Copy` token, so matching on it borrows nothing from
        // `self` and the arms are free to drive the cursor.
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("an expression"));
        };

        match token.kind {
            TokenKind::Punctuation(Punctuation::LParen) => self.parse_grouped(),
            // DuckDB collection literals: a primary-position `[` opens a list and `{`
            // a struct. Postfix subscript never reaches here (it needs a base, so its
            // `[` is consumed by the climb loop), and under a dialect with the gate
            // off both bytes fall through to the punctuation reject arm below.
            TokenKind::Punctuation(Punctuation::LBracket)
                if self.features().expression_syntax.collection_literals =>
            {
                self.parse_list_literal().map(ParsedExpr::bare)
            }
            TokenKind::Punctuation(Punctuation::LBrace)
                if self.features().expression_syntax.collection_literals =>
            {
                self.parse_struct_literal().map(ParsedExpr::bare)
            }
            TokenKind::Keyword(keyword) => self
                .parse_keyword_prefix(token, keyword)
                .map(ParsedExpr::bare),
            TokenKind::Operator(op) => self.parse_operator_prefix(token, op).map(ParsedExpr::bare),
            TokenKind::Word
            | TokenKind::QuotedIdent
            | TokenKind::Number
            | TokenKind::String
            | TokenKind::Parameter
            | TokenKind::PositionalColumn
            | TokenKind::Variable => self
                .parse_word_or_literal_prefix(token)
                .map(ParsedExpr::bare),
            // `::` is a postfix typecast and `:` an array-slice separator; neither —
            // nor any other punctuation (including `[`/`{` with the collection gate
            // off) — can begin a primary expression. Stage references are COPY INTO
            // endpoints, not expression primaries.
            TokenKind::Punctuation(_) | TokenKind::Unknown | TokenKind::StageReference => {
                Err(self.unexpected("an expression"))
            }
        }
    }
    /// Parse a dialect-hook custom prefix operator applied to its operand, the hook's
    /// binding power already peeked by the router. Called-and-returned so the hook
    /// path's locals stay off the recursive frame.
    #[inline(never)]
    fn parse_hook_prefix_operator(&mut self, prefix_bp: u8) -> ParseResult<Expr<D::Ext>> {
        let op_token = self
            .advance()?
            .expect("peek_prefix_operator_hook confirmed an operator token is present");
        let operand = self.parse_expr_bp(prefix_bp)?;
        D::build_prefix_operator(self, op_token, operand)
    }
    /// Per-class prefix dispatcher for an operator token in primary position: the
    /// sign/complement unary operators, the DuckDB `*COLUMNS(...)` unpack prefix, the
    /// PostgreSQL named prefix operators, and the (exhaustive) reject residue — a
    /// binary-only operator cannot open a primary.
    ///
    /// `#[inline(never)]`: called-and-returned from the [`parse_prefix`](Self::parse_prefix)
    /// router so this class's scratch stays off the hot recursive frame (the
    /// `high_but_safe_nesting` stack canary budget).
    #[inline(never)]
    fn parse_operator_prefix(&mut self, token: Token, op: Operator) -> ParseResult<Expr<D::Ext>> {
        match op {
            Operator::Minus => self.parse_unary(UnaryOperator::Minus),
            Operator::Plus => self.parse_unary(UnaryOperator::Plus),
            // Prefix `~` bitwise complement, under a dialect that admits the bitwise family.
            // With the gate off it falls through to the reject arm below (a lone `~` is not
            // an expression).
            Operator::Tilde if self.features().operator_syntax.bitwise_operators => {
                self.parse_unary(UnaryOperator::BitwiseNot)
            }
            // DuckDB's `*COLUMNS(<selector>)` unpack prefix: a primary-position `*`
            // immediately followed by `COLUMNS(` spreads the selected columns into the
            // enclosing call / `IN`-list argument list. The `*` is unpack only here in
            // prefix position; an infix `a * COLUMNS(*)` never reaches this arm (the
            // climb loop consumes the `*` as multiplication after its left operand), so
            // the two never collide (probed on 1.5.4). With the gate off, or a `*` not
            // followed by `COLUMNS(`, the token falls through to the reject arm below.
            Operator::Star if self.peek_is_columns_unpack_prefix()? => {
                let star = self
                    .advance()?
                    .expect("peek confirmed the `*` unpack prefix is present");
                self.parse_columns_selector(star.span, ColumnsSpelling::Unpack)
            }
            // General prefix symbolic operators under `custom_operators`. PostgreSQL admits
            // *any* `Op`-class token in prefix position (`qual_Op a_expr %prec Op`), and this
            // gate is a PostgreSQL-only preset (only `Postgres` sets `custom_operators`), so
            // this arm mirrors that grammar: every operator token whose lexeme is an `Op` —
            // the general residue (`Custom`, e.g. `@#@`/`|/`/`||/`/`!!`/`<->`/`-|-`), the lone
            // `@`, `~`, `!`, and every *dedicated* built-in operator token whose primary
            // meaning is infix — opens a primary here. Engine-probed on pg_query 17: each of
            //   `#` `&` `|` `?`  (single-glyph, dedicated infix token)
            //   `@@` `@?` `?|` `?&`  (the `jsonb` family)
            //   `#>` `#>>` `#-`  (`jsonb` path)
            //   `@>` `<@`  (containment)   `->` `->>`  (json arrow)
            //   `||` `&&` `<<` `>>`  (concat / logical-and / shifts)
            // accepts as `SELECT <op> 3` and deparses to the same bare-prefix form.
            //
            // The enumerated *exceptions* — the special single-char and comparison grammar
            // tokens that are NOT `Op` — stay rejected below and PostgreSQL rejects them in
            // prefix position too: `=`/`==`, `<`/`<=`/`>`/`>=`/`<>`/`!=`/`<=>`, `^`, `%`,
            // `*`, `/`/`//`, and the non-operator separators `=>`/`:=` (named args) and `|>`
            // (query pipe, a non-PostgreSQL token anyway). `+`/`-` keep their own dedicated
            // unary arms above.
            //
            // The prefix reading never disturbs the infix partition: these tokens reach this
            // arm only as the *leading* token of a primary (no left operand), while their
            // infix reading is driven by the climb loop after a left operand exists — so
            // `a # b` stays infix bitwise-XOR while `# b` is this prefix operator (both
            // engine-confirmed), and a bare `?` with no operand rejects here exactly as
            // PostgreSQL rejects `SELECT ?` (the operand climb fails). Binds at the "any
            // other operator" rank.
            Operator::Custom
            | Operator::AtAt
            | Operator::Bang
            | Operator::Hash
            | Operator::Amp
            | Operator::Pipe
            | Operator::Question
            | Operator::QuestionPipe
            | Operator::QuestionAmp
            | Operator::AtQuestion
            | Operator::HashGt
            | Operator::HashGtGt
            | Operator::HashMinus
            | Operator::AtGt
            | Operator::LtAt
            | Operator::MinusGt
            | Operator::MinusGtGt
            | Operator::Concat
            | Operator::AmpAmp
            | Operator::ShiftLeft
            | Operator::ShiftRight
                if self.features().operator_syntax.custom_operators =>
            {
                self.parse_prefix_named_operator(token)
            }
            // The reject residue. Two kinds of token land here. First, the genuine
            // never-a-prefix exceptions — the special single-char/comparison grammar tokens
            // that are NOT `Op`, which PostgreSQL also rejects in prefix position (probed):
            // `*`, `/`/`//`, `%`, `=`/`==`, `<`/`<=`/`>`/`>=`/`<>`/`!=`/`<=>`, `^`, plus the
            // non-operator separators `=>`/`:=` (named-argument, recognised only between a
            // name and its value in `parse_function_arg`) and `|>` (the query-pipe tail
            // token, a non-PostgreSQL lexeme). Second, the *gate-off fallthrough* of the
            // Op-class tokens the guarded arm above claims under `custom_operators` — a lone
            // `~` (when the bitwise gate is off), and `#`/`&`/`|`/`?`/the `jsonb`
            // (`@@`/`@?`/`?|`/`?&`/`#>`/`#>>`/`#-`)/containment (`@>`/`<@`)/arrow
            // (`->`/`->>`)/`||`/`&&`/`<<`/`>>`/`Custom` tokens (when `custom_operators` is
            // off) — which have no prefix reading without the gate, so they are an expression
            // error here exactly as before. Both kinds share this exhaustive arm.
            Operator::Star
            | Operator::Slash
            | Operator::SlashSlash
            | Operator::Percent
            | Operator::Eq
            | Operator::EqEq
            | Operator::Lt
            | Operator::LtEq
            | Operator::Gt
            | Operator::GtEq
            | Operator::NotEq
            | Operator::LtEqGt
            | Operator::Concat
            | Operator::AmpAmp
            | Operator::Bang
            | Operator::Pipe
            | Operator::Amp
            | Operator::Caret
            | Operator::CaretAt
            | Operator::Tilde
            | Operator::ShiftLeft
            | Operator::ShiftRight
            | Operator::Hash
            | Operator::Arrow
            | Operator::ColonEquals
            | Operator::AtGt
            | Operator::LtAt
            | Operator::MinusGt
            | Operator::MinusGtGt
            | Operator::PipeArrow
            | Operator::Question
            | Operator::QuestionPipe
            | Operator::QuestionAmp
            | Operator::AtQuestion
            | Operator::AtAt
            | Operator::HashGt
            | Operator::HashGtGt
            | Operator::HashMinus
            | Operator::Custom => Err(self.unexpected("an expression")),
        }
    }
    /// Parse a PostgreSQL `AexprConst` — a bare constant, never a general expression.
    ///
    /// The value grammar of the recursive-query `CYCLE … TO value DEFAULT default` mark:
    /// a numeric / string / bit-string literal, `NULL` / `TRUE` / `FALSE`, a temporal
    /// literal (`DATE '…'`), or a typed-string constant (`point '(1,1)'`, `float8 '1.5'`).
    /// It deliberately never enters the `(` / prefix-operator / column / function paths of
    /// `parse_primary`: parentheses are transparent in this AST
    /// (ADR-0008), so a `(5)` could not be accepted-then-rejected, and PostgreSQL rejects
    /// `TO (5)`, `TO -1`, `TO x`, `TO f()`, and `TO CAST(…)` alike (probed on pg_query 17)
    /// — each is a non-constant opener rejected here at dispatch. The literal arms reuse
    /// the same builders as `parse_primary`, restricted to the constant subset.
    pub(in crate::parser) fn parse_aexpr_const(&mut self) -> ParseResult<Expr<D::Ext>> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a constant value"));
        };
        match token.kind {
            TokenKind::Number => {
                self.advance()?;
                let kind =
                    number_literal_kind(self.span_text(token.span), self.parse_float_as_decimal());
                let literal = Literal {
                    kind,
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(token.span);
                Ok(Expr::Literal { literal, meta })
            }
            TokenKind::String => self.parse_string_literal(token),
            TokenKind::Keyword(Keyword::Null) => self.parse_literal_keyword(LiteralKind::Null),
            TokenKind::Keyword(Keyword::True) => {
                self.parse_literal_keyword(LiteralKind::Boolean(true))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.parse_literal_keyword(LiteralKind::Boolean(false))
            }
            // A temporal literal (`DATE '…'`, `INTERVAL '90' DAY`) — only when a string
            // constant follows the type keyword; a bare temporal word is not a constant.
            TokenKind::Keyword(
                keyword @ (Keyword::Date | Keyword::Time | Keyword::Timestamp | Keyword::Interval),
            ) => match self.try_parse_temporal_literal(keyword)? {
                Some(kind) => {
                    let span = token.span.union(self.preceding_span());
                    let literal = Literal {
                        kind,
                        meta: self.make_meta(span),
                    };
                    let meta = self.make_meta(span);
                    Ok(Expr::Literal { literal, meta })
                }
                None => Err(self.unexpected("a constant value")),
            },
            // A generalized typed-string constant (`point '(1,1)'`): a type-name prefix
            // followed by a string. A non-match rewinds and is not a constant here.
            TokenKind::Word | TokenKind::Keyword(_) | TokenKind::QuotedIdent => {
                match self.try_parse_typed_literal()? {
                    Some(expr) => Ok(expr),
                    None => Err(self.unexpected("a constant value")),
                }
            }
            _ => Err(self.unexpected("a constant value")),
        }
    }
    /// Parse a prefix unary operator (the current token) applied to its operand.
    ///
    /// Recurses at the operator's prefix binding power, so the operand binds
    /// everything tighter than the operator: `- a * b` is `(- a) * b`, while
    /// `NOT a = b` is `NOT (a = b)` (`NOT` is looser than comparison).
    pub(super) fn parse_unary(&mut self, op: UnaryOperator) -> ParseResult<Expr<D::Ext>> {
        let token = self
            .advance()?
            .expect("parse_unary is reached only with a current operator token");
        let operand = self.parse_expr_bp(self.features().prefix_binding_power(&op))?;
        let span = token.span.union(operand.span());
        let meta = self.make_meta(span);
        Ok(Expr::UnaryOp {
            op,
            expr: Box::new(operand),
            meta,
        })
    }
    /// Parse a PostgreSQL prefix symbolic operator (`@`, `|/`, `@@`, `@#@`) applied to its
    /// operand. The operator's exact spelling is taken from `token`'s span and interned so it
    /// round-trips; the operand is climbed at the "any other operator" right power so it
    /// captures everything tighter (`@ a + b` is `@ (a + b)`), matching PostgreSQL's
    /// `qual_Op a_expr %prec Op` prefix production.
    fn parse_prefix_named_operator(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // the operator token
        let op = self.intern_text(self.span_text(token.span));
        let right_bp = self.features().binding_powers.any_operator.right;
        let operand = self.parse_expr_bp(right_bp)?;
        let span = token.span.union(operand.span());
        let prefix_operator = PrefixOperatorExpr {
            op,
            operand,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::PrefixOperator {
            prefix_operator: Box::new(prefix_operator),
            meta,
        })
    }
    /// Parse a parenthesized expression.
    ///
    /// Parens are grouping only and are not stored: the inner
    /// expression is returned directly, re-entering the climb at the lowest
    /// precedence so the parentheses fully reset binding inside them.
    fn parse_grouped(&mut self) -> ParseResult<ParsedExpr<D::Ext>> {
        let open = self
            .advance()?
            .expect("parse_grouped is reached only at an open parenthesis");
        // `(SELECT …)` / `(WITH …)` / `(VALUES (…))` open a scalar subquery, but a
        // bare `values` is a non-reserved column name, so `(values + 1)` stays a
        // grouped column reference (see `peek_starts_subquery_in_parens`).
        if self.peek_starts_subquery_in_parens()? {
            let query = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the subquery")?;
            let span = open.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(ParsedExpr::bare(Expr::Subquery {
                query: Box::new(query),
                meta,
            }));
        }
        // A nested `(` is the genuinely ambiguous case: `((SELECT …) UNION …)` is
        // a scalar subquery over a parenthesized-operand set operation, while
        // `((a + b), c)` is a row constructor whose first field happens to be
        // parenthesized and `((a + b))` a doubly-grouped expression — they
        // diverge only *after* the inner group, so no bounded prefix tells them
        // apart. Mirrors `from::try_parenthesized_query_factor` (ADR-0005
        // backtracking, reusing the `query` set-op climb rather than
        // re-deriving precedence): the query reading wins exactly when it
        // consumes the group whole (the closing `)` immediately follows this
        // parenthesized position), otherwise the cursor rewinds to the
        // row-constructor/grouping path below.
        if self.peek_is_punct(Punctuation::LParen)? {
            let checkpoint = self.checkpoint();
            if let Some(query) = self.try_parenthesized_query()? {
                let span = open.span.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(ParsedExpr::bare(Expr::Subquery {
                    query: Box::new(query),
                    meta,
                }));
            }
            self.rewind(checkpoint);
        }
        // A parenthesized group is PostgreSQL's `c_expr: '(' a_expr ')'` — the inner
        // expression resets to the full `a_expr` grammar, so `parse_expr` (not the raw
        // `parse_expr_bp`) is used to clear any `b_expr` restriction for the group.
        let inner = self.parse_expr()?;
        // `(a, b, …)` with at least two elements is the implicit row constructor;
        // a single parenthesized expression stays a bare grouping (the parens are
        // not a node, ADR-0008). Gated by dialect data like the explicit `ROW(...)`.
        if self.features().expression_syntax.row_constructor
            && self.peek_is_punct(Punctuation::Comma)?
        {
            let mut fields = thin_vec![inner];
            while self.eat_punct(Punctuation::Comma)? {
                fields.push(self.parse_expr()?);
            }
            self.expect_punct(Punctuation::RParen, "`)` to close the row constructor")?;
            let span = open.span.union(self.preceding_span());
            let row = RowExpr {
                fields,
                explicit: false,
                meta: self.make_meta(span),
            };
            let meta = self.make_meta(span);
            return Ok(ParsedExpr::bare(Expr::Row {
                row: Box::new(row),
                meta,
            }));
        }
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Punctuation(Punctuation::RParen) => {
                self.advance()?; // consume `)`
                Ok(ParsedExpr::grouped(inner))
            }
            _ => Err(self.unexpected("`)`")),
        }
    }
    /// Speculatively read a `(`-opening parenthesized position — one whose
    /// first inner token is itself `(` — as a complete query (PostgreSQL
    /// `select_with_parens`, with a parenthesized leading set-operation
    /// operand, e.g. `((SELECT 1) UNION (SELECT 2) ORDER BY 1 OFFSET 1)`).
    ///
    /// Shared by every expression-grammar position that can open a scalar
    /// subquery over a parenthesized operand: a grouped expression
    /// ([`parse_grouped`](Self::parse_grouped), `((SELECT …) UNION …)` as a
    /// bare scalar subquery) and an `IN (…)` operand
    /// ([`parse_predicate`](Self::parse_predicate), `x IN ((SELECT …) UNION
    /// …)`). Both diverge from their non-query reading
    /// (a row constructor / plain grouping; an expression list) only *after*
    /// the inner group, so no bounded prefix tells them apart — this mirrors
    /// `from::try_parenthesized_query_factor`'s identical ambiguity for a
    /// derived table in `FROM` position (backtracking, reusing the
    /// `query` set-op climb rather than re-deriving precedence).
    ///
    /// The outer `(` must already be consumed by the caller. Returns `Some`
    /// when the set-op-aware query grammar consumes the whole group (a closing
    /// `)` immediately follows), and `None` — leaving the cursor for the caller
    /// to rewind — otherwise. A query parse *error* is likewise a non-match:
    /// under fail-fast the reported error is inert (only the returned `Result`
    /// drives control flow), so the rewound path surfaces the real
    /// diagnostic.
    pub(super) fn try_parenthesized_query(&mut self) -> ParseResult<Option<Query<D::Ext>>> {
        // Arm the grouping context: the inner leading `(` is an expression-position
        // scalar-subquery grouping (`SELECT ((SELECT 1))`), a complete standalone primary
        // SQLite accepts with `parenthesized_query_operands` off — not a bare operand.
        self.set_paren_query_grouping(true);
        let Ok(query) = self.parse_query() else {
            return Ok(None);
        };
        if !self.peek_is_punct(Punctuation::RParen)? {
            return Ok(None);
        }
        self.advance()?; // `)`
        Ok(Some(query))
    }
}
