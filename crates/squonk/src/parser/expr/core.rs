// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use super::ParsedExpr;
use crate::ast::dialect::FeatureSet;
use crate::ast::precedence::{Assoc, BindingPower, OVERLAPS_PREDICATE, UNPARENTHESIZED_IN_LIST};
use crate::ast::{
    ArgSyntax, AtTimeZoneExpr, BinaryOperator, BitwiseXorSpelling, CastSyntax, CollateExpr,
    EqualsSpelling, Expr, Extension, FieldSelectionExpr, FieldSelector, FunctionArg, Ident,
    IntegerDivideSpelling, IsDistinctFromSpelling, IsNotDistinctFromSpelling, Keyword, LambdaExpr,
    LambdaParamSpelling, LikeSpelling, ModuloSpelling, NamedOperatorExpr, NamedOperatorSpelling,
    NormalizationForm, NotEqSpelling, NullTestSpelling, ObjectName, PostfixOperatorExpr,
    Quantifier, SemiStructuredAccessExpr, SemiStructuredPathSegment, Spanned, SubscriptExpr,
    SubscriptKind, Symbol, TruthValue, UnaryOperator,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use thin_vec::{ThinVec, thin_vec};

/// A PostgreSQL postfix operator recognised in the precedence-climbing loop.
///
/// These all attach to an already-parsed left operand and bind tighter than the
/// binary operators; each is gated by [`ExpressionSyntax`](crate::ast::dialect::ExpressionSyntax)
/// dialect data and carries its own binding power from the dialect table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PostfixOp {
    /// `expr::type`.
    Typecast,
    /// `base[index]` / `base[lower:upper]`.
    Subscript,
    /// `base:key[0].field`.
    SemiStructuredAccess,
    /// `expr COLLATE collation`.
    Collate,
    /// `expr AT TIME ZONE zone`.
    AtTimeZone,
    /// `(expr).field`.
    FieldSelection,
    /// `(expr).*` composite expansion or a value-position whole-row `tbl.*`.
    FieldWildcard,
}

/// The parsed contents of an array subscript `[...]`: an index `[i]`, a two-bound slice
/// `[lower:upper]` (either bound optional), or a DuckDB three-bound slice `[lower:upper:step]`.
struct SubscriptBounds<X: Extension> {
    /// The index ([`SubscriptKind::Index`]) or the slice lower bound, if present.
    lower: Option<Expr<X>>,
    /// The slice upper bound, if present. `None` under [`SubscriptKind::SliceWithStep`] is
    /// the `-` open-upper placeholder.
    upper: Option<Expr<X>>,
    /// The slice step, if present ([`SubscriptKind::SliceWithStep`] only).
    step: Option<Expr<X>>,
    /// Which bracketed form was read.
    kind: SubscriptKind,
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse a full expression: the Pratt core entered at the lowest precedence.
    ///
    /// This `a_expr` entry is the reset boundary for the `b_expr` restriction: every
    /// `c_expr` sub-expression (a parenthesized group, a call argument, a `CASE` arm, an
    /// array element, a scalar subquery) re-enters through here, so it clears
    /// [`restrict_b_expr`](Parser) for its whole extent and restores it after — leaving the
    /// restriction to cover only the enclosing `b_expr` operator spine, exactly as
    /// PostgreSQL's `c_expr: '(' a_expr ')'` resets to the full grammar inside parentheses.
    pub(crate) fn parse_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        let saved = self.restrict_b_expr;
        // A nested `c_expr` (parens, call/ROW args, a cast operand) re-admits the
        // value-position `.*` star selector suppressed only at a projection-target top
        // level, so this reset boundary clears it just as it clears `restrict_b_expr`.
        let saved_star = self.suppress_value_star;
        self.restrict_b_expr = false;
        self.suppress_value_star = false;
        let result = self.parse_expr_bp(0);
        self.restrict_b_expr = saved;
        self.suppress_value_star = saved_star;
        result
    }

    /// Parse a projection target expression (a SELECT target or `RETURNING` item) with
    /// the value-position `.*` star selector suppressed at its top level, so a bare-name
    /// `t.*` stays a select-list qualified wildcard rather than folding into a value
    /// composite-star. A nested `c_expr` (parens, call/`ROW` args, a cast operand) clears
    /// the suppression via [`parse_expr`](Self::parse_expr), so a `(func()).*` or a
    /// whole-row `tbl.*` written *inside* a value still folds. Like the `a_expr` entry
    /// this clears [`restrict_b_expr`](Parser) for the target's extent.
    pub(crate) fn parse_projection_target_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        let saved_restrict = self.restrict_b_expr;
        let saved_star = self.suppress_value_star;
        self.restrict_b_expr = false;
        self.suppress_value_star = true;
        let result = self.parse_expr_bp(0);
        self.restrict_b_expr = saved_restrict;
        self.suppress_value_star = saved_star;
        result
    }

    /// Parse a PostgreSQL `b_expr` — the restricted expression grammar a column-constraint
    /// `DEFAULT` uses under
    /// [`ColumnDefinitionSyntax::column_default_requires_b_expr`](crate::ast::dialect::ColumnDefinitionSyntax).
    /// The same Pratt core, entered with [`restrict_b_expr`](Parser) armed so the loop leaves
    /// the `a_expr`-only operators (`AND`/`OR`/`NOT`, `IN`/`BETWEEN`/`LIKE`, `IS NULL`, a
    /// quantified comparison, `AT TIME ZONE`) unconsumed at the spine — where the caller's
    /// context then rejects them — while keeping arithmetic, comparison, `IS [NOT] DISTINCT
    /// FROM`, `COLLATE`, `::`, subscripts, and `OPERATOR(...)`.
    pub(crate) fn parse_b_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        let saved = self.restrict_b_expr;
        self.restrict_b_expr = true;
        let result = self.parse_expr_bp(0);
        self.restrict_b_expr = saved;
        result
    }

    /// Parse a PostgreSQL `c_expr` (a primary/prefix) and return just the expression,
    /// discarding the grouped/bare tag. Used by the `XMLTABLE` table factor
    /// ([`super::super::from`]), whose row and document operands are `c_expr` (a bare `a || b`
    /// rejects — the operator tail is left unconsumed for the enclosing clause). Resets the
    /// [`restrict_b_expr`](Parser) flag so a parenthesized `(a_expr)` re-admits the full
    /// grammar, matching PostgreSQL's `c_expr: '(' a_expr ')'`.
    pub(crate) fn parse_c_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        let saved = self.restrict_b_expr;
        self.restrict_b_expr = false;
        let parsed = self.parse_prefix();
        self.restrict_b_expr = saved;
        Ok(parsed?.expr)
    }

    /// Parse a partition-key element head (PostgreSQL `part_elem`): a bare column (`a`), a bare
    /// `func_expr_windowless` (`lower(a)`), or a parenthesized `(a_expr)` — the *primary* only,
    /// stopping before any postfix operator. This deliberately leaves `COLLATE`, `::`, `[…]`, and
    /// a trailing operator-class name unconsumed, because in a `part_elem` the `COLLATE` and
    /// operator-class are *separate clauses* (not expression postfixes) and `::`/`[…]` are
    /// outright rejected (matching PostgreSQL, which parses only `part_elem`'s three key forms
    /// here). Returns the parsed expression and whether it was source-parenthesized (the
    /// `'(' a_expr ')'` form), which the caller records so the exact key form round-trips.
    pub(crate) fn parse_partition_key_head(&mut self) -> ParseResult<(Expr<D::Ext>, bool)> {
        let saved = self.restrict_b_expr;
        self.restrict_b_expr = false;
        let parsed = self.parse_prefix();
        self.restrict_b_expr = saved;
        let parsed = parsed?;
        Ok((parsed.expr, parsed.grouped))
    }

    /// Parse the operand of DuckDB's `%` percentage-`LIMIT` marker: a prefix/primary with
    /// its tighter-than-arithmetic postfix operators (`::`, `[]`, `COLLATE`, `AT TIME
    /// ZONE`, field selection) folded in, but no binary-infix fold. DuckDB reduces the
    /// `%` marker only once its operand is complete at *multiplicative-or-tighter*
    /// precedence, so `LIMIT (30-10) %` / `LIMIT RANDOM() %` / `LIMIT ?::VARCHAR %` parse
    /// while `LIMIT 1+2 %` is a DuckDB parser error (the `%` shifts as a modulo needing a
    /// right operand). Reading at the multiplicative *right* binding power reproduces that
    /// boundary: every binary operator (top rank multiplicative) is left unfolded — so a
    /// trailing modulo `%` is never consumed and stays the marker — while the postfix
    /// operators, which all bind tighter, still fold. The caller eats the `%`.
    pub(crate) fn parse_limit_percent_operand(&mut self) -> ParseResult<Expr<D::Ext>> {
        self.parse_expr_bp(self.features().binding_powers.multiplicative.right)
    }
    /// Precedence-climbing core: parse an expression that binds at least `min_bp`.
    ///
    /// The recursion-guarded entry to the Pratt core: every nested
    /// expression — a parenthesized group, a unary operand, an operator's
    /// right-hand side, `CASE`/`CAST`/array/row sub-expressions — re-enters here,
    /// so guarding this one method bounds *all* expression nesting. A scalar
    /// subquery also passes through here (its enclosing grouping) on top of
    /// [`parse_query`](Self::parse_query), so it counts against the limit at both
    /// points. The real climb is [`parse_expr_bp_inner`](Self::parse_expr_bp_inner).
    ///
    /// `pub(crate)` (like [`parse_expr`](Self::parse_expr)) so a caller outside the `expr`
    /// module can parse an operand bounded above a precedence level — the `FROM`-clause
    /// `FOR SYSTEM_TIME BETWEEN … AND …` endpoints parse at the range-predicate power so the
    /// separating `AND` stays a delimiter.
    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8) -> ParseResult<Expr<D::Ext>> {
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard.parser().parse_expr_bp_inner(min_bp)
    }
    /// The precedence climb itself, one level deep under the recursion guard.
    ///
    /// Parse a prefix/primary, then fold in each following infix operator whose
    /// *left* binding power is at least `min_bp`, recursing the right operand at
    /// the operator's *own* right binding power. An operator that binds
    /// looser than `min_bp` belongs to an outer expression and ends this one.
    fn parse_expr_bp_inner(&mut self, min_bp: u8) -> ParseResult<Expr<D::Ext>> {
        let ParsedExpr {
            expr: mut lhs,
            grouped: mut lhs_grouped,
        } = self.parse_prefix()?;

        loop {
            // DuckDB's unparenthesized `<expr> [NOT] IN <c_expr>` list-membership binds
            // tighter than the comparison-level `IN (list)` predicate (between comparison
            // and string-concat), so it is matched here at its own binding power ahead of
            // the predicate path — which still owns the parenthesized `IN (list)` /
            // `IN (subquery)` forms (the peek declines a `(` RHS) and the reject on a
            // missing `(` after a disallowed leading token.
            if let Some(negated) = self.peek_unparenthesized_in_list()? {
                // An `IN` list is an `a_expr`-only predicate — outside `b_expr`.
                if self.restrict_b_expr || UNPARENTHESIZED_IN_LIST.left < min_bp {
                    break;
                }
                lhs = self.parse_unparenthesized_in_list(lhs, negated)?;
                lhs_grouped = false;
                continue;
            }
            if self.peek_starts_predicate()? {
                // `b_expr` keeps only `IS [NOT] DISTINCT FROM` from the predicate family;
                // `IN`/`BETWEEN`/`LIKE`/`IS NULL`/`OVERLAPS`/… are `a_expr`-only, so leave
                // them unconsumed (the caller's context rejects the residual keyword).
                if self.restrict_b_expr && !self.peek_starts_is_distinct_predicate()? {
                    break;
                }
                // NOTE this branch's body is deliberately lean: `parse_expr_bp_inner` is the
                // per-nesting-level recursive frame, so predicate dispatch (including the
                // tighter-binding `OVERLAPS` period predicate) lives in the called helpers,
                // whose frames pop before any recursion — inlining it here tripped the
                // `high_but_safe_nesting` stack canary.
                let bp = self.predicate_binding_power()?;
                if bp.left < min_bp {
                    break;
                }
                self.reject_nonassoc_chain(&lhs, bp, lhs_grouped, "the end of the comparison")?;
                lhs = self.parse_predicate(lhs, lhs_grouped)?;
                lhs_grouped = false;
                continue;
            }

            // The PostgreSQL postfix operators (`::`, `[]`, `COLLATE`, `AT TIME
            // ZONE`) and composite field selection bind tighter than the binary
            // operators; each is gated by dialect data and its own binding power.
            // A postfix token whose left binding power is below `min_bp` belongs to
            // an outer expression, so it is left for the infix check (which ends the
            // climb), matching how a looser infix operator returns out of the loop.
            if let Some(postfix) = self.peek_postfix_operator(min_bp)? {
                // `AT TIME ZONE` is `a_expr`-only — outside `b_expr` — so leave it for the
                // caller to reject; the other postfixes (`::`, `[]`, `COLLATE`, field
                // selection) are `b_expr` members and fold as usual.
                if self.restrict_b_expr && postfix == PostfixOp::AtTimeZone {
                    break;
                }
                // PostgreSQL's subscript indirection applies to a `c_expr`, and a bare
                // `CASE … END` is not one: `CASE … END[i]` is a syntax error, requiring
                // the parenthesized `(CASE … END)[i]` (a grouped `c_expr`). A grouped
                // operand carries no `Expr::Nested` node (ADR-0008), so the `grouped`
                // flag is what tells the two apart here.
                if postfix == PostfixOp::Subscript
                    && !lhs_grouped
                    && matches!(lhs, Expr::Case { .. })
                {
                    return Err(
                        self.unexpected("`(` around the `CASE` expression before subscripting it")
                    );
                }
                lhs = self.parse_postfix(lhs, postfix)?;
                lhs_grouped = false;
                continue;
            }

            // DuckDB postfix symbolic operators: a general `Op`-class operator with no operand
            // following it folds as an `Expr::PostfixOperator` — `10!`, `1 ~`, `1 <->`, `1 &`
            // (the postfix reading PostgreSQL removed in 14; `OperatorSyntax::postfix_operators`).
            // It shares the "any other operator" left rank with the infix reading, and the infix
            // reading still wins whenever an operand DOES follow (`1 ! + 2` is infix `!`), so this
            // claims the operator only in the operand-absent position. A postfix below `min_bp`
            // belongs to an outer expression: the `left` gate skips it, and the loop's later infix
            // checks then break on the same rank, deferring the operator to the outer frame (so
            // `2 * 3 !` groups `(2 * 3)!`).
            if self.features().operator_syntax.postfix_operators
                && self.features().binding_powers.any_operator.left >= min_bp
                && self.peek_postfix_symbolic_operator()?
            {
                lhs = self.parse_postfix_symbolic_operator(lhs)?;
                lhs_grouped = false;
                continue;
            }

            // The PostgreSQL `a OPERATOR(schema.op) b` explicit-operator infix form
            // binds at the "any other operator" rank (the `string_concat` level, the
            // same `%left Op OPERATOR` precedence as `||`, ADR-0008); like `||` it is
            // left-associative, so no non-associative chain check applies. A construct
            // below `min_bp` belongs to an outer expression and ends the climb.
            if let Some(bp) = self.peek_operator_construct()? {
                if bp.left < min_bp {
                    break;
                }
                lhs = self.parse_operator_construct(lhs, bp.right)?;
                lhs_grouped = false;
                continue;
            }

            // The general PostgreSQL bare symbolic operator — regex `~`/`!~`/`~*`/`!~*`, a
            // geometric/network/text-search op, or a fully user-defined operator — binds at
            // the "any other operator" rank (`%left Op`, left-associative like `||`, so no
            // chain check), building an `Expr::NamedOperator` with the bare spelling. This is
            // the infix twin of `peek_operator_construct`; it claims the bare operator tokens
            // ahead of `peek_infix_operator` (which returns `None` for them).
            if let Some(bp) = self.peek_bare_infix_operator()? {
                if bp.left < min_bp {
                    break;
                }
                lhs = self.parse_bare_infix_operator(lhs, bp.right)?;
                lhs_grouped = false;
                continue;
            }

            let Some(op) = self.peek_infix_operator()? else {
                // No built-in infix operator. A dialect may still define a custom
                // one over a lexeme the core grammar leaves free in infix position.
                // The parser keeps ownership of the precedence climb: it gates on
                // the hook's left binding power, then climbs the right operand at the
                // hook's right binding power itself, so a custom operator cannot
                // mis-bind by ignoring its own precedence (ADR-0008).
                if let Some(bp) = D::peek_infix_operator_hook(self)? {
                    if bp.left < min_bp {
                        break;
                    }
                    lhs = self.fold_hook_infix(lhs, lhs_grouped, bp)?;
                    lhs_grouped = false;
                    continue;
                }
                break;
            };
            // `AND`/`OR` are the boolean operators `b_expr` excludes; leave the keyword for
            // the caller (a column-constraint `DEFAULT`) to reject.
            if self.restrict_b_expr && matches!(op, BinaryOperator::And | BinaryOperator::Or) {
                break;
            }
            let bp = self.features().binding_power(&op);
            if bp.left < min_bp {
                break;
            }
            lhs = self.fold_infix(lhs, lhs_grouped, op, bp)?;
            lhs_grouped = false;
        }

        Ok(lhs)
    }
    /// Fold one already-peeked built-in infix operator (its `bp` already cleared
    /// `min_bp`) into its left operand: the non-associative chain check, the DuckDB
    /// arrow-lambda and quantified-operand special shapes, and the ordinary binary
    /// fold. Called-and-returned from the climb loop so the fold's locals (the right
    /// operand, spans, the lambda scratch) stay off the per-nesting-level recursive
    /// frame (the `high_but_safe_nesting` stack canary; see the NOTE above). The
    /// no-inline hint is debug-gated: it rides the hottest release path, where the
    /// stack budget does not bind and the optimizer keeps its freedom.
    #[cfg_attr(debug_assertions, inline(never))]
    fn fold_infix(
        &mut self,
        lhs: Expr<D::Ext>,
        lhs_grouped: bool,
        op: BinaryOperator,
        bp: BindingPower,
    ) -> ParseResult<Expr<D::Ext>> {
        // A non-associative operator (the comparisons) may not chain: `a < b < c`
        // is a clean error, not a silently left-associated parse. Climbing alone
        // would accept it — the second operator's `left` still clears `min_bp` —
        // so reject it explicitly when the left operand is already a comparison
        // at this same precedence level.
        self.reject_nonassoc_chain(&lhs, bp, lhs_grouped, "the end of the comparison")?;
        let op_token = self
            .advance()?
            .expect("peek_infix_operator confirmed an operator token is present");
        // The one `Operator::NotEq` token spells both `<>` and `!=`; recover which the
        // source wrote from the token text so the fidelity tag round-trips. Every other
        // operator's spelling is already fixed by its distinct token, so this is a no-op
        // for them. Runs before the quantified/plain folds so `a != ANY (…)` keeps it too.
        let op = if let BinaryOperator::NotEq(_) = op {
            let spelling = if self.span_text(op_token.span).starts_with('!') {
                NotEqSpelling::Bang
            } else {
                NotEqSpelling::AngleBracket
            };
            BinaryOperator::NotEq(spelling)
        } else {
            op
        };
        // DuckDB's single-arrow lambda shares its token, binding power, and
        // associativity with the JSON `->` accessor — DuckDB itself parses every
        // `->` as one construct and splits lambda from JSON at *bind* time, by
        // requiring a lambda argument's left side to be an unqualified name list.
        // Applying that same shape test here picks the node label without
        // changing acceptance or precedence: a param-shaped left operand builds
        // the lambda, anything else falls through to the ordinary `JsonGet`
        // binary fold below (see `OperatorSyntax::lambda_expressions`).
        if matches!(op, BinaryOperator::JsonGet)
            && self.features().operator_syntax.lambda_expressions
        {
            if let Some((params, spelling)) = lambda_params(&lhs, lhs_grouped) {
                let body = self.parse_expr_bp(bp.right)?;
                let span = lhs.span().union(op_token.span).union(body.span());
                let lambda = LambdaExpr {
                    params,
                    spelling,
                    body,
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                return Ok(Expr::Lambda {
                    lambda: Box::new(lambda),
                    meta,
                });
            }
        }
        // A quantified `<op> {ANY|ALL|SOME} (...)` is an `a_expr`-only form; under
        // `b_expr` the operator stays plain, and the reserved `ANY`/`ALL`/`SOME` head
        // then surfaces as a clean reject when the right operand is parsed. The six
        // comparison operators quantify in every dialect that admits the construct;
        // PostgreSQL extends it to any operator except the boolean keywords `AND`/`OR`
        // (`quantified_arbitrary_operator`, engine-probed).
        if self.is_quantifiable_operator(&op) && !self.restrict_b_expr {
            if let Some(quantifier) = self.parse_quantifier()? {
                return self.parse_quantified_operator_tail(lhs, op, op_token, quantifier);
            }
        }
        let right = self.parse_expr_bp(bp.right)?;
        let span = lhs.span().union(op_token.span).union(right.span());
        let meta = self.make_meta(span);
        Ok(Expr::BinaryOp {
            left: Box::new(lhs),
            op,
            right: Box::new(right),
            meta,
        })
    }
    /// Fold one dialect-hook infix operator (its `bp` already cleared `min_bp`) into
    /// its left operand. Split out of the climb loop for the same stack-frame reason
    /// as [`fold_infix`](Self::fold_infix), with the same debug-gated no-inline hint.
    #[cfg_attr(debug_assertions, inline(never))]
    fn fold_hook_infix(
        &mut self,
        lhs: Expr<D::Ext>,
        lhs_grouped: bool,
        bp: BindingPower,
    ) -> ParseResult<Expr<D::Ext>> {
        // A non-associative custom operator may not chain (`a ~ b ~ c`),
        // exactly like the built-in comparisons: reject when the left
        // operand is already a non-associative operator at this same level.
        // The reported `bp` (with `left < right`) is what routed the second
        // operator here to the enclosing climb instead of the inner one.
        self.reject_nonassoc_chain(&lhs, bp, lhs_grouped, "the end of the operator chain")?;
        let op_token = self
            .advance()?
            .expect("peek_infix_operator_hook confirmed an operator token is present");
        let right = self.parse_expr_bp(bp.right)?;
        D::build_infix_operator(self, op_token, lhs, right)
    }
    /// Reject a non-associative operator that would chain onto an equal-precedence
    /// non-associative left operand (`a < b < c`): the climb alone would silently
    /// left-associate it, so this makes it a clean error. A parenthesized left
    /// operand (`grouped`) forms a barrier that breaks the chain and is allowed
    /// (`(a < b) < c`). `expected` names what should have followed instead.
    fn reject_nonassoc_chain(
        &mut self,
        lhs: &Expr<D::Ext>,
        bp: BindingPower,
        grouped: bool,
        expected: &'static str,
    ) -> ParseResult<()> {
        if bp.assoc == Assoc::NonAssoc && !grouped && self.lhs_chains_nonassoc(lhs, bp.left) {
            return Err(self.unexpected(expected));
        }
        Ok(())
    }
    /// Peek the PostgreSQL postfix operator at the cursor, if one applies here.
    ///
    /// Returns `Some` only when the dialect enables the form, the operator token is
    /// present, and the operator's left binding power is at least `min_bp` — so a
    /// postfix that binds looser than the current climb is declined (left to end the
    /// loop) rather than mis-folded. The cursor is never advanced.
    fn peek_postfix_operator(&mut self, min_bp: u8) -> ParseResult<Option<PostfixOp>> {
        let syntax = self.features().expression_syntax;
        let powers = self.features().binding_powers;
        if syntax.typecast_operator
            && powers.typecast.left >= min_bp
            && self.peek_is_punct(Punctuation::DoubleColon)?
        {
            return Ok(Some(PostfixOp::Typecast));
        }
        if syntax.subscript
            && powers.subscript.left >= min_bp
            && self.peek_is_punct(Punctuation::LBracket)?
        {
            return Ok(Some(PostfixOp::Subscript));
        }
        if syntax.semi_structured_access
            && powers.subscript.left >= min_bp
            && self.peek_is_punct(Punctuation::Colon)?
            && self
                .peek_nth(1)?
                .is_some_and(|token| self.token_can_be_label(token))
        {
            return Ok(Some(PostfixOp::SemiStructuredAccess));
        }
        // `COLLATE` is the operator only when a collation *name* follows; a bare
        // `COLLATE` at the end of an expression is a column label (`SELECT a collate`
        // aliases `a` as `collate`, matching PostgreSQL's `BareColLabel`). The
        // collation head is a `ColId`, so a reserved keyword (`FROM`, …) is not one
        // and leaves `COLLATE` to be read as the label instead.
        if syntax.collate
            && powers.collate.left >= min_bp
            && self.peek_is_keyword(Keyword::Collate)?
            && self
                .peek_nth(1)?
                .is_some_and(|token| self.token_can_be_column_name(token))
        {
            return Ok(Some(PostfixOp::Collate));
        }
        // `AT` is the operator only when the full `AT TIME ZONE` phrase follows;
        // otherwise a bare `AT` is a column label (PostgreSQL's `BareColLabel`).
        if syntax.at_time_zone
            && powers.at_time_zone.left >= min_bp
            && self.peek_is_keyword(Keyword::At)?
            && self.peek_nth_is_keyword(1, Keyword::Time)?
            && self.peek_nth_is_keyword(2, Keyword::Zone)?
        {
            return Ok(Some(PostfixOp::AtTimeZone));
        }
        // Field selection only triggers on `.` *followed by a label*: a `.*` is a
        // qualified wildcard the projection grammar owns, and a bare `.` ends the
        // expression. The dot is otherwise consumed during name parsing, so it
        // only reaches here after a non-column primary (`(x).y`, `f(x).y`).
        if syntax.field_selection
            && powers.field_selection.left >= min_bp
            && self.peek_is_punct(Punctuation::Dot)?
            && self
                .peek_nth(1)?
                .is_some_and(|token| self.token_can_be_label(token))
        {
            return Ok(Some(PostfixOp::FieldSelection));
        }
        // The `.*` star selector — composite expansion `(expr).*` and a value-position
        // whole-row `tbl.*` — binds at the same rank as `.field`. Suppressed at a
        // projection-target top level (`suppress_value_star`) so a select-list `tbl.*`
        // stays a qualified wildcard; a nested value `.*` (in `ROW(...)`, a call
        // argument, a cast) still folds because the `c_expr` reset clears the flag.
        if syntax.field_wildcard
            && !self.suppress_value_star
            && powers.field_selection.left >= min_bp
            && self.peek_is_punct(Punctuation::Dot)?
            && self.peek_nth_is_op(1, Operator::Star)?
        {
            return Ok(Some(PostfixOp::FieldWildcard));
        }
        Ok(None)
    }
    /// Fold a peeked postfix operator into its left operand `lhs`.
    fn parse_postfix(&mut self, lhs: Expr<D::Ext>, op: PostfixOp) -> ParseResult<Expr<D::Ext>> {
        match op {
            PostfixOp::Typecast => {
                self.advance()?; // `::`
                let data_type = self.parse_data_type()?;
                let span = lhs.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                Ok(Expr::Cast {
                    expr: Box::new(lhs),
                    data_type: Box::new(data_type),
                    syntax: CastSyntax::DoubleColon,
                    try_cast: false,
                    meta,
                })
            }
            PostfixOp::Subscript => {
                self.advance()?; // `[`
                let bounds = self.parse_subscript_bounds()?;
                self.expect_punct(Punctuation::RBracket, "`]` to close the subscript")?;
                let span = lhs.span().union(self.preceding_span());
                let subscript = SubscriptExpr {
                    base: lhs,
                    lower: bounds.lower,
                    upper: bounds.upper,
                    step: bounds.step,
                    kind: bounds.kind,
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                Ok(Expr::Subscript {
                    subscript: Box::new(subscript),
                    meta,
                })
            }
            PostfixOp::SemiStructuredAccess => self.parse_semi_structured_access(lhs),
            PostfixOp::Collate => {
                self.advance()?; // `COLLATE`
                let collation = self.parse_object_name()?;
                let span = lhs.span().union(self.preceding_span());
                let collate = CollateExpr {
                    expr: lhs,
                    collation,
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                Ok(Expr::Collate {
                    collate: Box::new(collate),
                    meta,
                })
            }
            PostfixOp::AtTimeZone => {
                self.advance()?; // `AT`
                self.expect_keyword(Keyword::Time)?;
                self.expect_keyword(Keyword::Zone)?;
                // The zone binds at the operator's right binding power, so a looser
                // operator after it (`a AT TIME ZONE z + 1`) closes the zone first.
                let zone_bp = self.features().binding_powers.at_time_zone.right;
                let zone = self.parse_expr_bp(zone_bp)?;
                let span = lhs.span().union(zone.span());
                let at_time_zone = AtTimeZoneExpr {
                    expr: lhs,
                    zone,
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                Ok(Expr::AtTimeZone {
                    at_time_zone: Box::new(at_time_zone),
                    meta,
                })
            }
            PostfixOp::FieldSelection => {
                self.advance()?; // `.`
                // The field is an attribute name (`ColLabel`), which admits every
                // keyword — `(x).order` is valid PostgreSQL.
                let field = self.parse_as_alias_ident()?;
                // DuckDB dot-method chaining: `<receiver>.<method>(<args>)` desugars to
                // the ordinary call `<method>(<receiver>, <args>)` (ADR-0011 canonical
                // shape — the method spelling is not preserved because the desugared form
                // round-trips structurally). Fires only when the flag is on and `(`
                // immediately follows the method name; a `name.method(args)` on a bare-name
                // receiver is instead the schema-qualified call the object-name grammar
                // reads before any expression exists, so this postfix reaches only a
                // non-name receiver (a call result, a parenthesized expression) — there is
                // no ambiguity with a qualified call.
                if self.features().call_syntax.method_chaining
                    && self.peek_is_punct(Punctuation::LParen)?
                {
                    let lhs_span = lhs.span();
                    let name = ObjectName(thin_vec![field]);
                    let mut call = self.parse_function_call(name, lhs_span)?;
                    let receiver = FunctionArg {
                        name: None,
                        variadic: false,
                        syntax: ArgSyntax::Positional,
                        value: lhs,
                        meta: self.make_meta(lhs_span),
                    };
                    call.args.insert(0, receiver);
                    let meta = self.make_meta(call.meta.span);
                    return Ok(Expr::Function {
                        call: Box::new(call),
                        meta,
                    });
                }
                let field_meta = self.make_meta(field.span());
                let span = lhs.span().union(self.preceding_span());
                let field_selection = FieldSelectionExpr {
                    base: lhs,
                    selector: FieldSelector::Field {
                        field,
                        meta: field_meta,
                    },
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                Ok(Expr::FieldSelection {
                    field_selection: Box::new(field_selection),
                    meta,
                })
            }
            PostfixOp::FieldWildcard => {
                self.advance()?; // `.`
                self.advance()?; // `*`
                Ok(self.build_field_wildcard(lhs))
            }
        }
    }

    /// Build a `.*` composite/whole-row star selection off an already-parsed `base`,
    /// spanning from the base through the just-consumed `*`. Shared by the postfix loop
    /// and the projection-item parser, which folds a non-column `(expr).*` target.
    pub(crate) fn build_field_wildcard(&mut self, base: Expr<D::Ext>) -> Expr<D::Ext> {
        let star_meta = self.make_meta(self.preceding_span());
        let span = base.span().union(self.preceding_span());
        let field_selection = FieldSelectionExpr {
            base,
            selector: FieldSelector::Star { meta: star_meta },
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Expr::FieldSelection {
            field_selection: Box::new(field_selection),
            meta,
        }
    }

    fn parse_semi_structured_access(&mut self, base: Expr<D::Ext>) -> ParseResult<Expr<D::Ext>> {
        self.expect_punct(Punctuation::Colon, "`:` before the semi-structured path")?;
        let first = self.parse_semi_structured_key_segment()?;
        let mut path = thin_vec![first];
        while let Some(segment) = self.parse_semi_structured_path_suffix()? {
            path.push(segment);
        }
        let span = base.span().union(self.preceding_span());
        let access = SemiStructuredAccessExpr {
            base,
            path,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::SemiStructuredAccess {
            semi_structured_access: Box::new(access),
            meta,
        })
    }

    /// Parse one `.key` / `[index]` suffix of a semi-structured path, or `None` when the
    /// next token opens neither. Shared with the table-position PartiQL / SUPER path
    /// (`crate::parser::from`), whose bracket-rooted `FROM src[0].a` is a sequence of these
    /// suffixes with no leading `:`.
    pub(in crate::parser) fn parse_semi_structured_path_suffix(
        &mut self,
    ) -> ParseResult<Option<SemiStructuredPathSegment<D::Ext>>> {
        if self.peek_is_punct(Punctuation::Dot)?
            && self
                .peek_nth(1)?
                .is_some_and(|token| self.token_can_be_label(token))
        {
            self.advance()?; // `.`
            return Ok(Some(self.parse_semi_structured_key_segment()?));
        }
        if self.peek_is_punct(Punctuation::LBracket)? {
            self.advance()?; // `[`
            let index = self.parse_expr()?;
            self.expect_punct(
                Punctuation::RBracket,
                "`]` to close the semi-structured path index",
            )?;
            let meta = self.make_meta(index.span());
            return Ok(Some(SemiStructuredPathSegment::Index {
                index: Box::new(index),
                meta,
            }));
        }
        Ok(None)
    }

    fn parse_semi_structured_key_segment(
        &mut self,
    ) -> ParseResult<SemiStructuredPathSegment<D::Ext>> {
        let key = self.parse_as_alias_ident()?;
        let meta = self.make_meta(key.span());
        Ok(SemiStructuredPathSegment::Key { key, meta })
    }
    /// Parse the contents of an array subscript `[...]`: an index `[i]`, a two-bound slice
    /// `[lower:upper]` (either bound optional — `[lo:]`, `[:hi]`, `[:]`), or, under a dialect
    /// with [`slice_step`](crate::ast::dialect::ExpressionSyntax::slice_step), a DuckDB
    /// three-bound slice `[lower:upper:step]`.
    ///
    /// An index has its value in `lower`. A three-bound slice may omit the lower bound and
    /// the step but not the middle bound: DuckDB spells an open upper bound as a bare `-`
    /// before the second `:` (`[lower:-:step]`), read into `upper == None` and distinguished
    /// from a negative-expression bound like `-5`. An empty `[]` or an empty middle
    /// (`[lower::step]`) is rejected.
    fn parse_subscript_bounds(&mut self) -> ParseResult<SubscriptBounds<D::Ext>> {
        let lower = if self.peek_is_punct(Punctuation::Colon)?
            || self.peek_is_punct(Punctuation::RBracket)?
        {
            None
        } else {
            Some(self.parse_expr()?)
        };
        if !self.eat_punct(Punctuation::Colon)? {
            // No separator: a bare index, whose value must be present.
            return if lower.is_some() {
                Ok(SubscriptBounds {
                    lower,
                    upper: None,
                    step: None,
                    kind: SubscriptKind::Index,
                })
            } else {
                Err(self.unexpected("an array subscript index"))
            };
        }
        // One `:` seen. The `-` open-upper placeholder is a three-bound-slice-only spelling,
        // recognised only immediately before a second `:`; elsewhere `-` opens an ordinary
        // (negative) expression, so a two-bound dialect never mistakes `[lo:-5]` for it.
        let stepped = self.features().expression_syntax.slice_step;
        let dash_upper = stepped
            && self.peek_is_op(Operator::Minus)?
            && self.peek_nth_is_punct(1, Punctuation::Colon)?;
        let upper = if dash_upper {
            self.advance()?; // the `-` placeholder
            None
        } else if self.peek_is_punct(Punctuation::Colon)?
            || self.peek_is_punct(Punctuation::RBracket)?
        {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let second_colon = stepped && self.eat_punct(Punctuation::Colon)?;
        if !second_colon {
            // Two-bound slice `[lower:upper]`. The `-` placeholder needs a following `:`,
            // so `dash_upper` cannot hold here.
            return Ok(SubscriptBounds {
                lower,
                upper,
                step: None,
                kind: SubscriptKind::Slice,
            });
        }
        // Second `:` seen: a three-bound slice `[lower:upper:step]`. The middle bound is
        // mandatory (an empty `[lower::step]` is a DuckDB parse error), satisfied by either
        // an expression or the `-` placeholder.
        if upper.is_none() && !dash_upper {
            return Err(self.unexpected("a slice upper bound or `-` before the step"));
        }
        let step = if self.peek_is_punct(Punctuation::RBracket)? {
            None
        } else {
            Some(self.parse_expr()?)
        };
        Ok(SubscriptBounds {
            lower,
            upper,
            step,
            kind: SubscriptKind::SliceWithStep,
        })
    }
    /// Whether the cursor begins the one predicate `b_expr` keeps — `IS [NOT] DISTINCT
    /// FROM` (PostgreSQL's `b_expr IS [NOT] DISTINCT FROM b_expr`). Under the `b_expr`
    /// restriction this admits the distinct test while the rest of the `IS`/`IN`/`BETWEEN`/
    /// `LIKE`/`IS NULL` predicate family is left unconsumed. (`IS [NOT] DOCUMENT` is the
    /// other `b_expr` predicate, but the parser does not yet model it, so it is not
    /// admitted here — it stays a reject either way, no over-acceptance.)
    fn peek_starts_is_distinct_predicate(&mut self) -> ParseResult<bool> {
        if !self.peek_is_keyword(Keyword::Is)? {
            return Ok(false);
        }
        if self.peek_nth_is_keyword(1, Keyword::Distinct)? {
            return Ok(true);
        }
        Ok(self.peek_nth_is_keyword(1, Keyword::Not)?
            && self.peek_nth_is_keyword(2, Keyword::Distinct)?)
    }
    /// True if the cursor begins a comparison-level predicate: `IS …`,
    /// `[NOT] BETWEEN …`, `[NOT] IN …`, the dialect-gated `[NOT]
    /// LIKE`/`ILIKE`/`SIMILAR TO` pattern-match family, or the dialect-gated
    /// `OVERLAPS` period predicate (which carries its own tighter binding power —
    /// see [`predicate_binding_power`](Self::predicate_binding_power)).
    fn peek_starts_predicate(&mut self) -> ParseResult<bool> {
        if self.peek_is_keyword(Keyword::Is)?
            || self.peek_is_keyword(Keyword::In)?
            || self.peek_is_keyword(Keyword::Between)?
        {
            return Ok(true);
        }
        // PostgreSQL/SQLite's postfix `ISNULL`/`NOTNULL` synonyms bind at the same
        // comparison-level rank as `IS NULL` and are dispatched through `parse_predicate`.
        if self.features().operator_syntax.null_test_postfix
            && (self.peek_is_keyword(Keyword::Isnull)? || self.peek_is_keyword(Keyword::Notnull)?)
        {
            return Ok(true);
        }
        if self.features().predicate_syntax.overlaps_period_predicate
            && self.peek_is_keyword(Keyword::Overlaps)?
        {
            return Ok(true);
        }
        let predicate = self.features().predicate_syntax;
        if (predicate.like && self.peek_is_keyword(Keyword::Like)?)
            || (predicate.ilike && self.peek_is_keyword(Keyword::Ilike)?)
        {
            return Ok(true);
        }
        // `SIMILAR TO` is two keywords; a bare `SIMILAR` is an ordinary name, so it
        // only opens the predicate when `TO` follows.
        if predicate.similar_to
            && self.peek_is_keyword(Keyword::Similar)?
            && self.peek_nth_is_keyword(1, Keyword::To)?
        {
            return Ok(true);
        }
        if self.peek_is_keyword(Keyword::Not)? {
            return Ok(self.peek_nth_is_keyword(1, Keyword::In)?
                || self.peek_nth_is_keyword(1, Keyword::Between)?
                || (predicate.like && self.peek_nth_is_keyword(1, Keyword::Like)?)
                || (predicate.ilike && self.peek_nth_is_keyword(1, Keyword::Ilike)?)
                || (predicate.similar_to
                    && self.peek_nth_is_keyword(1, Keyword::Similar)?
                    && self.peek_nth_is_keyword(2, Keyword::To)?)
                // SQLite/DuckDB's two-word `<expr> NOT NULL` postfix null test (synonym for
                // `IS NOT NULL`). A bounded two-token lookahead: `NOT` immediately followed by
                // `NULL` in predicate position opens it; every other `NOT`-led form (`NOT IN`,
                // `NOT LIKE`, the keyword operators below) is unaffected.
                || (predicate.null_test_two_word_postfix
                    && self.peek_nth_is_keyword(1, Keyword::Null)?)
                // SQLite `a NOT GLOB b` (and `NOT MATCH`/`NOT REGEXP`): a keyword
                // operator negated by a leading `NOT`. The bare form binds through the
                // infix path; only the negated form opens the predicate here.
                || self.keyword_operator_at(1)?.is_some());
        }
        Ok(false)
    }
    /// For the `IS [NOT] [<form>] NORMALIZED` predicate: consume an optional Unicode-normal-form
    /// keyword and the mandatory `NORMALIZED` keyword, returning the form (`None` for the bare
    /// `IS NORMALIZED`). Returns `None` when `NORMALIZED` does not close the run, leaving the
    /// cursor untouched so an unreserved form word is never mis-consumed.
    fn eat_normalization_form_before_normalized(
        &mut self,
    ) -> ParseResult<Option<Option<NormalizationForm>>> {
        if self.eat_keyword(Keyword::Normalized)? {
            return Ok(Some(None));
        }
        let form = if self.peek_is_keyword(Keyword::Nfc)? {
            NormalizationForm::Nfc
        } else if self.peek_is_keyword(Keyword::Nfd)? {
            NormalizationForm::Nfd
        } else if self.peek_is_keyword(Keyword::Nfkc)? {
            NormalizationForm::Nfkc
        } else if self.peek_is_keyword(Keyword::Nfkd)? {
            NormalizationForm::Nfkd
        } else {
            return Ok(None);
        };
        // Only a form word immediately followed by `NORMALIZED` opens the predicate; otherwise
        // the word is an ordinary operand and the cursor stays put.
        if !self.peek_nth_is_keyword(1, Keyword::Normalized)? {
            return Ok(None);
        }
        self.advance()?; // the form keyword
        self.advance()?; // `NORMALIZED`
        Ok(Some(Some(form)))
    }
    /// The binding power of the predicate at the cursor. The `OVERLAPS` period predicate
    /// carries PostgreSQL's own `%nonassoc OVERLAPS` rank just above the comparison family;
    /// the range/pattern/membership predicates (`[NOT] BETWEEN`/`IN`/`LIKE`/`ILIKE`/`SIMILAR
    /// TO`) carry the dialect's [`range_predicate`](squonk_ast::precedence::BindingPowerTable::range_predicate)
    /// rank (above comparison under PostgreSQL/Lenient, level with it elsewhere); the
    /// `IS …`/`ISNULL`/`NOTNULL`/`NOT NULL` family carries the
    /// [`predicate`](squonk_ast::precedence::BindingPowerTable::predicate) rank (below
    /// comparison under PostgreSQL/DuckDB/Lenient, level with it elsewhere); and the negated
    /// `GLOB`/`MATCH`/`REGEXP`/`RLIKE` keyword operators bind at the shared comparison level,
    /// like their bare infix forms.
    fn predicate_binding_power(&mut self) -> ParseResult<BindingPower> {
        if self.features().predicate_syntax.overlaps_period_predicate
            && self.peek_is_keyword(Keyword::Overlaps)?
        {
            return Ok(OVERLAPS_PREDICATE);
        }
        if self.peek_starts_range_predicate()? {
            return Ok(self.features().binding_powers.range_predicate());
        }
        // A `NOT`-negated keyword operator (`NOT GLOB`/`MATCH`/`REGEXP`/`RLIKE`) folds to
        // `NOT (a <op> b)` with the inner operator at comparison precedence, so — like its bare
        // infix form — it binds at comparison, not the (possibly looser) `IS`-family tier.
        if self.peek_is_keyword(Keyword::Not)? && self.keyword_operator_at(1)?.is_some() {
            return Ok(self
                .features()
                .binding_power(&BinaryOperator::Eq(EqualsSpelling::Single)));
        }
        Ok(self.features().binding_powers.predicate())
    }
    /// True if the cursor begins a range/pattern/membership predicate — `[NOT] BETWEEN`,
    /// `[NOT] IN`, `[NOT] LIKE`/`ILIKE`/`SIMILAR TO` — the family PostgreSQL ranks one tier
    /// above comparison (see [`predicate_binding_power`](Self::predicate_binding_power)).
    /// The `IS …`/`ISNULL`/`NOTNULL`/`OVERLAPS` predicates and the SQLite `[NOT] GLOB`/
    /// `MATCH`/`REGEXP` keyword operators stay at the comparison level and are excluded.
    /// Only reached from a predicate dispatch, so the relevant dialect flags are known on.
    fn peek_starts_range_predicate(&mut self) -> ParseResult<bool> {
        if self.peek_is_keyword(Keyword::In)? || self.peek_is_keyword(Keyword::Between)? {
            return Ok(true);
        }
        let predicate = self.features().predicate_syntax;
        if (predicate.like && self.peek_is_keyword(Keyword::Like)?)
            || (predicate.ilike && self.peek_is_keyword(Keyword::Ilike)?)
        {
            return Ok(true);
        }
        if predicate.similar_to
            && self.peek_is_keyword(Keyword::Similar)?
            && self.peek_nth_is_keyword(1, Keyword::To)?
        {
            return Ok(true);
        }
        if self.peek_is_keyword(Keyword::Not)? {
            return Ok(self.peek_nth_is_keyword(1, Keyword::In)?
                || self.peek_nth_is_keyword(1, Keyword::Between)?
                || (predicate.like && self.peek_nth_is_keyword(1, Keyword::Like)?)
                || (predicate.ilike && self.peek_nth_is_keyword(1, Keyword::Ilike)?)
                || (predicate.similar_to
                    && self.peek_nth_is_keyword(1, Keyword::Similar)?
                    && self.peek_nth_is_keyword(2, Keyword::To)?));
        }
        Ok(false)
    }
    /// Parse a comparison-level predicate after its left-hand expression:
    /// `IS [NOT] NULL`, `[NOT] BETWEEN low AND high`, `[NOT] IN (list | <query>)`,
    /// or the `OVERLAPS` period predicate (`expr_grouped` feeds its
    /// re-parenthesized-operand reject; every other family ignores it).
    fn parse_predicate(
        &mut self,
        expr: Expr<D::Ext>,
        expr_grouped: bool,
    ) -> ParseResult<Expr<D::Ext>> {
        // The SQL-standard `(s1, e1) OVERLAPS (s2, e2)` period predicate — `row OVERLAPS
        // row`, both operands exactly-two-element rows, yielding a boolean. Dispatched
        // first since no other predicate family starts with the keyword; non-chaining is
        // enforced by the operand-shape check in `parse_overlaps_predicate`, not the
        // climb: the boolean result is not a row, so a second `OVERLAPS` finds no valid
        // left operand.
        if self.features().predicate_syntax.overlaps_period_predicate
            && self.peek_is_keyword(Keyword::Overlaps)?
        {
            return self.parse_overlaps_predicate(expr, expr_grouped);
        }
        if self.eat_keyword(Keyword::Is)? {
            let negated = self.eat_keyword(Keyword::Not)?;
            if self.features().predicate_syntax.is_distinct_from
                && self.eat_keyword(Keyword::Distinct)?
            {
                self.expect_keyword(Keyword::From)?;
                // Parse the right operand at the `IS`-family predicate's own right binding
                // power (as the postfix members are climbed), so an equal/looser-precedence
                // predicate stops rather than nesting: `a IS DISTINCT FROM b IS DISTINCT FROM c`
                // then rejects as a non-associative chain in the caller, matching PostgreSQL's
                // `%prec IS` non-associativity. Under PostgreSQL/DuckDB this rank sits below
                // comparison, so a tighter comparison right operand still folds in
                // (`a IS DISTINCT FROM b = c` is `a IS DISTINCT FROM (b = c)`); elsewhere it
                // equals the comparison right power, byte-identical to before.
                let right_bp = self.features().binding_powers.predicate().right;
                let right = self.parse_expr_bp(right_bp)?;
                let op = if negated {
                    BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Keyword)
                } else {
                    BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Keyword)
                };
                let span = expr.span().union(right.span());
                let meta = self.make_meta(span);
                return Ok(Expr::BinaryOp {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                    meta,
                });
            }
            // The truth-value tests `IS [NOT] {TRUE | FALSE | UNKNOWN}` (SQL:2016 F571).
            // A bounded one-keyword lookahead settles the shared `IS` lead: `DISTINCT` was
            // already consumed above, so `TRUE`/`FALSE`/`UNKNOWN` here are unambiguously the
            // truth-value keyword and not a `DISTINCT FROM` or `NULL` continuation. Checked
            // ahead of the SQLite general-equality arm so that under a dialect enabling both
            // (LENIENT) the standard truth reading wins; SQLite itself leaves this off and
            // folds `IS TRUE`/`IS FALSE` onto general equality against the boolean literal.
            if self.features().operator_syntax.truth_value_tests {
                let value = if self.eat_keyword(Keyword::True)? {
                    Some(TruthValue::True)
                } else if self.eat_keyword(Keyword::False)? {
                    Some(TruthValue::False)
                } else if self.eat_keyword(Keyword::Unknown)? {
                    Some(TruthValue::Unknown)
                } else {
                    None
                };
                if let Some(value) = value {
                    let span = expr.span().union(self.preceding_span());
                    let meta = self.make_meta(span);
                    return Ok(Expr::IsTruth {
                        expr: Box::new(expr),
                        value,
                        negated,
                        meta,
                    });
                }
            }
            // The SQL/JSON `IS [NOT] JSON [type] [WITH|WITHOUT UNIQUE [KEYS]]` predicate
            // (SQL:2016). Checked ahead of the SQLite general-equality arm so that under a
            // dialect enabling both (Lenient) the JSON predicate wins over reading `json` as
            // an ordinary right operand; only `JSON` as the immediate next keyword opens it.
            if self.features().call_syntax.sqljson_expression_functions
                && self.peek_is_keyword(Keyword::Json)?
            {
                return self.parse_is_json_predicate(expr, negated);
            }
            // The SQL/XML `IS [NOT] DOCUMENT` predicate (SQL:2006). Like `IS JSON`, checked
            // ahead of the SQLite general-equality arm so the predicate wins under a dialect
            // enabling both; only `DOCUMENT` as the immediate next keyword opens it.
            if self.features().call_syntax.xml_expression_functions
                && self.peek_is_keyword(Keyword::Document)?
            {
                return self.parse_is_document_predicate(expr, negated);
            }
            // The SQL-standard `IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` Unicode-normalization
            // test (T061). The optional form keyword is consumed only when `NORMALIZED`
            // follows, so a bare form word (unreserved) is never mistaken for the predicate;
            // checked ahead of the general-equality arm so the predicate wins under a dialect
            // enabling both.
            if self.features().predicate_syntax.is_normalized {
                if let Some(form) = self.eat_normalization_form_before_normalized()? {
                    let span = expr.span().union(self.preceding_span());
                    let meta = self.make_meta(span);
                    return Ok(Expr::IsNormalized {
                        expr: Box::new(expr),
                        form,
                        negated,
                        meta,
                    });
                }
            }
            // SQLite's `IS` is a general null-safe equality, not only `IS NULL` /
            // `IS [NOT] DISTINCT FROM`: `a IS b` / `a IS NOT b` over a non-`NULL` right
            // operand folds onto the null-safe operators (`a IS NOT DISTINCT FROM b` /
            // `a IS DISTINCT FROM b` — SQLite's exact semantics). `IS [NOT] NULL` keeps
            // its dedicated `Expr::IsNull` shape, so the null test is unaffected.
            if self.features().operator_syntax.is_general_equality
                && !self.peek_is_keyword(Keyword::Null)?
            {
                let right_bp = self
                    .features()
                    .binding_power(&BinaryOperator::Eq(EqualsSpelling::Single))
                    .right;
                let right = self.parse_expr_bp(right_bp)?;
                // SQLite's bare `IS`/`IS NOT` keep their own spelling tag so they render
                // back as `IS`/`IS NOT` rather than the explicit `IS [NOT] DISTINCT FROM`.
                let op = if negated {
                    BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Is)
                } else {
                    BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Is)
                };
                let span = expr.span().union(right.span());
                let meta = self.make_meta(span);
                return Ok(Expr::BinaryOp {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                    meta,
                });
            }
            self.expect_keyword(Keyword::Null)?;
            let span = expr.span().union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::IsNull {
                expr: Box::new(expr),
                negated,
                spelling: NullTestSpelling::Is,
                meta,
            });
        }

        // PostgreSQL/SQLite's one-word postfix null tests `<expr> ISNULL` / `<expr> NOTNULL`
        // (synonyms for `IS NULL` / `IS NOT NULL`), gated by `null_test_postfix`.
        // `peek_starts_predicate` already confirmed the feature is on before dispatching here.
        if self.features().operator_syntax.null_test_postfix {
            if self.eat_keyword(Keyword::Isnull)? {
                let span = expr.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(Expr::IsNull {
                    expr: Box::new(expr),
                    negated: false,
                    spelling: NullTestSpelling::Postfix,
                    meta,
                });
            }
            if self.eat_keyword(Keyword::Notnull)? {
                let span = expr.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(Expr::IsNull {
                    expr: Box::new(expr),
                    negated: true,
                    spelling: NullTestSpelling::Postfix,
                    meta,
                });
            }
        }

        let negated = self.eat_keyword(Keyword::Not)?;

        // SQLite/DuckDB's two-word `<expr> NOT NULL` postfix null test — a synonym for
        // `IS NOT NULL`, folded onto `Expr::IsNull` with the `PostfixNotNull` spelling so it
        // round-trips. Distinct from the one-word `NOTNULL` keyword-operator
        // (`OperatorSyntax::null_test_postfix`): PostgreSQL accepts that but rejects this,
        // so the two-word form rides its own `null_test_two_word_postfix` gate.
        // `peek_starts_predicate` already confirmed the flag is on and `NULL` follows.
        if negated
            && self.features().predicate_syntax.null_test_two_word_postfix
            && self.eat_keyword(Keyword::Null)?
        {
            let span = expr.span().union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::IsNull {
                expr: Box::new(expr),
                negated: true,
                spelling: NullTestSpelling::PostfixNotNull,
                meta,
            });
        }

        if self.eat_keyword(Keyword::Between)? {
            // The SQL-standard `SYMMETRIC`/`ASYMMETRIC` modifier (T461), gated by
            // `between_symmetric`. `SYMMETRIC` is load-bearing (it permits `low > high`);
            // the default `ASYMMETRIC` is a noise word consumed and dropped.
            let symmetric = if self.features().predicate_syntax.between_symmetric {
                if self.eat_keyword(Keyword::Symmetric)? {
                    true
                } else {
                    let _ = self.eat_keyword(Keyword::Asymmetric)?;
                    false
                }
            } else {
                false
            };
            // The bounds bind tighter than the `BETWEEN` separator `AND`, so the inner
            // `AND` is a delimiter rather than a boolean operator. Parsing at the range
            // predicate's own right binding power admits additive/`||`/etc. bounds while
            // stopping before `AND`/`OR`, the comparisons, and — crucially — another
            // same-tier range predicate: a second `BETWEEN` must not fold into the high
            // bound (`a BETWEEN b AND c BETWEEN d AND e`), it has to surface at the outer
            // climb where the non-associative-chain check rejects it (matching PostgreSQL).
            // Under dialects that keep range predicates at comparison level this rank equals
            // the comparison's right power, so their bounds are unchanged.
            let bound_bp = self.features().binding_powers.range_predicate().right;
            let low = self.parse_expr_bp(bound_bp)?;
            self.expect_keyword(Keyword::And)?;
            let high = self.parse_expr_bp(bound_bp)?;
            let span = expr.span().union(high.span());
            let meta = self.make_meta(span);
            return Ok(Expr::Between {
                expr: Box::new(expr),
                low: Box::new(low),
                high: Box::new(high),
                negated,
                symmetric,
                meta,
            });
        }

        // The dialect-gated pattern-match predicates `[NOT] LIKE|ILIKE|SIMILAR TO`.
        // `peek_starts_predicate` already confirmed the relevant feature is enabled
        // and (for `SIMILAR`) that `TO` follows, so these consume unconditionally.
        let predicate = self.features().predicate_syntax;
        if predicate.like && self.eat_keyword(Keyword::Like)? {
            return self.parse_like_predicate(expr, negated, LikeSpelling::Like);
        }
        if predicate.ilike && self.eat_keyword(Keyword::Ilike)? {
            return self.parse_like_predicate(expr, negated, LikeSpelling::ILike);
        }
        if predicate.similar_to && self.eat_keyword(Keyword::Similar)? {
            self.expect_keyword(Keyword::To)?;
            return self.parse_like_predicate(expr, negated, LikeSpelling::SimilarTo);
        }

        // SQLite `a NOT GLOB b` (and `NOT MATCH`/`NOT REGEXP`): a keyword operator
        // negated by the `NOT` just consumed. The operator has no negated surface of
        // its own, so it folds to the semantic identity `NOT (a <op> b)` — the right
        // operand climbs at the operator's own right binding power, as a `BETWEEN`
        // bound does. Reached only with `negated` (the bare form binds through the
        // infix path), so an un-negated keyword operator never lands here.
        if negated {
            if let Some(op) = self.keyword_operator_at(0)? {
                self.advance()?; // the keyword operator
                let right_bp = self.features().binding_power(&op).right;
                let right = self.parse_expr_bp(right_bp)?;
                let span = expr.span().union(right.span());
                let inner = Expr::BinaryOp {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                    meta: self.make_meta(span),
                };
                let meta = self.make_meta(span);
                return Ok(Expr::UnaryOp {
                    op: UnaryOperator::Not,
                    expr: Box::new(inner),
                    meta,
                });
            }
        }

        self.expect_keyword(Keyword::In)?;
        self.expect_punct(Punctuation::LParen, "`(` after `IN`")?;
        // `x IN (VALUES (…))` is a subquery, but `x IN (values)` is a one-element
        // list over the non-reserved column `values` (see
        // `peek_starts_subquery_in_parens`); otherwise it falls to the expr list.
        if self.peek_starts_subquery_in_parens()? {
            let subquery = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `IN` subquery")?;
            let span = expr.span().union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::InSubquery {
                expr: Box::new(expr),
                subquery: Box::new(subquery),
                negated,
                meta,
            });
        }
        // A nested `(` is the same ambiguity `try_parenthesized_query` resolves
        // for a grouped expression: `x IN ((SELECT …) UNION …)` is a subquery
        // operand over a parenthesized-operand set operation, while `x IN ((a),
        // b)` is an ordinary list whose first element happens to be
        // parenthesized.
        if self.peek_is_punct(Punctuation::LParen)? {
            let checkpoint = self.checkpoint();
            if let Some(subquery) = self.try_parenthesized_query()? {
                let span = expr.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(Expr::InSubquery {
                    expr: Box::new(expr),
                    subquery: Box::new(subquery),
                    negated,
                    meta,
                });
            }
            self.rewind(checkpoint);
        }
        // SQLite accepts an empty `IN ()` list (`empty_in_list`); the standard requires at
        // least one element. Checked before the list parse so the closing `)` is consumed
        // as the empty list rather than rejected as a missing first element.
        let list = if self.features().predicate_syntax.empty_in_list
            && self.peek_is_punct(Punctuation::RParen)?
        {
            ThinVec::new()
        } else {
            // DuckDB tolerates a trailing comma before the `IN` list's closing `)`.
            self.parse_comma_separated_trailing(Self::parse_expr, |p| {
                p.peek_is_punct(Punctuation::RParen)
            })?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the `IN` list")?;
        let span = expr.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::InList {
            expr: Box::new(expr),
            list,
            negated,
            meta,
        })
    }
    /// Peek DuckDB's unparenthesized `[NOT] IN <c_expr>` list-membership operator.
    ///
    /// Returns `Some(negated)` only when the feature is on and the cursor is at `IN` (or
    /// `NOT IN`) followed by a token that opens the restricted `c_expr` right operand.
    /// A leading `(` (the standard `IN (list)`/`IN (subquery)` predicate) or a
    /// constant/unary-sign/star/`EXISTS`/`COLUMNS`/`ROW`/`GROUPING` starter returns
    /// `None`, leaving the keyword for the comparison-level predicate path — which reads
    /// the parenthesized form or reports the same reject on the missing `(`. DuckDB's
    /// gram.y forbids the excluded leading tokens (`IN 4` / `IN 'a'` / `IN -5` / `IN *` /
    /// `IN EXISTS …` are parser errors, an LALR restriction), so the leading-token gate
    /// reproduces its acceptance exactly. Never advances the cursor.
    fn peek_unparenthesized_in_list(&mut self) -> ParseResult<Option<bool>> {
        if !self.features().predicate_syntax.unparenthesized_in_list {
            return Ok(None);
        }
        let (negated, rhs_offset) = if self.peek_is_keyword(Keyword::In)? {
            (false, 1)
        } else if self.peek_is_keyword(Keyword::Not)? && self.peek_nth_is_keyword(1, Keyword::In)? {
            (true, 2)
        } else {
            return Ok(None);
        };
        Ok(if self.token_opens_in_expr_rhs(rhs_offset)? {
            Some(negated)
        } else {
            None
        })
    }
    /// Whether the token `offset` ahead opens the restricted `c_expr` right operand of an
    /// unparenthesized `IN` (see [`peek_unparenthesized_in_list`](Self::peek_unparenthesized_in_list)
    /// for the excluded set and its DuckDB grounding). Never advances the cursor.
    fn token_opens_in_expr_rhs(&mut self, offset: usize) -> ParseResult<bool> {
        let Some(token) = self.peek_nth(offset)? else {
            return Ok(false);
        };
        // A following string constant makes the leading token a typed literal (`DATE
        // '2020-01-01'`, `float8 'NaN'`) — an `AexprConst` DuckDB forbids here; a following
        // `(` makes `COLUMNS`/`ROW`/`GROUPING` their constructor form (also excluded).
        let next_kind = self.peek_nth(offset + 1)?.map(|t| t.kind);
        let next_is_string = matches!(next_kind, Some(TokenKind::String));
        let next_is_lparen = matches!(next_kind, Some(TokenKind::Punctuation(Punctuation::LParen)));
        let allowed = match token.kind {
            // The parenthesized `IN (list)` / `IN (subquery)` predicate owns a `(` RHS.
            TokenKind::Punctuation(Punctuation::LParen) => false,
            // Constants: numeric / string / bit-string literals and the keyword constants.
            TokenKind::Number | TokenKind::String => false,
            TokenKind::Keyword(Keyword::Null | Keyword::True | Keyword::False) => false,
            // `EXISTS (…)` is a `c_expr` production DuckDB excludes from the RHS.
            TokenKind::Keyword(Keyword::Exists) => false,
            // `COLUMNS(…)` / `ROW(…)` / `GROUPING(…)` constructors are excluded; a bare
            // `columns`/`row`/`grouping` (no `(`) stays an ordinary column name, allowed.
            TokenKind::Keyword(Keyword::Columns | Keyword::Row | Keyword::Grouping)
                if next_is_lparen =>
            {
                false
            }
            // A type-name prefix on a string constant is a typed literal (temporal
            // `DATE '…'` or the generalized `float8 'NaN'`) — also an `AexprConst`.
            TokenKind::Word | TokenKind::QuotedIdent | TokenKind::Keyword(_) if next_is_string => {
                false
            }
            // Unary sign / bitwise complement and the star (`*` / `*COLUMNS(…)` unpack)
            // open a unary or star expression, both forbidden as the RHS leading token.
            TokenKind::Operator(
                Operator::Minus | Operator::Plus | Operator::Tilde | Operator::Star,
            ) => false,
            _ => true,
        };
        Ok(allowed)
    }
    /// Parse DuckDB's unparenthesized `[NOT] IN <c_expr>` after its left operand `expr`.
    /// [`peek_unparenthesized_in_list`](Self::peek_unparenthesized_in_list) has confirmed
    /// the feature, the keyword(s), and an allowed RHS leading token, so this consumes
    /// them unconditionally.
    fn parse_unparenthesized_in_list(
        &mut self,
        expr: Expr<D::Ext>,
        negated: bool,
    ) -> ParseResult<Expr<D::Ext>> {
        if negated {
            self.expect_keyword(Keyword::Not)?;
        }
        self.expect_keyword(Keyword::In)?;
        let rhs = self.parse_in_expr_rhs()?;
        let span = expr.span().union(rhs.span());
        let meta = self.make_meta(span);
        Ok(Expr::InExpr {
            expr: Box::new(expr),
            rhs: Box::new(rhs),
            negated,
            meta,
        })
    }
    /// Parse the restricted `c_expr` right operand of an unparenthesized `IN`: a primary
    /// plus its `c_expr`-level subscript indirection, but none of the `a_expr` postfix
    /// operators (`::`, `COLLATE`, `AT TIME ZONE`) or any infix operator. `z IN y[1]`
    /// binds the subscript into the RHS (`contains(y[1], z)`), while `z IN y::INT` leaves
    /// the cast outside (`(z IN y)::INT`) — both measured on DuckDB 1.5.4.
    fn parse_in_expr_rhs(&mut self) -> ParseResult<Expr<D::Ext>> {
        let mut rhs = self.parse_prefix()?.expr;
        while self.features().expression_syntax.subscript
            && self.peek_is_punct(Punctuation::LBracket)?
        {
            rhs = self.parse_postfix(rhs, PostfixOp::Subscript)?;
        }
        Ok(rhs)
    }
    /// Parse the tail of a pattern-match predicate after its keyword: `<pattern>
    /// [ESCAPE <c>]`, folding it into the canonical [`Expr::Like`] with `spelling`.
    ///
    /// The pattern (and the optional `ESCAPE` character) bind at the comparison's
    /// right binding power, exactly like a `BETWEEN` bound: an additive/concat
    /// pattern is admitted while `AND`/`OR` end it, so `a LIKE 'x' AND b` parses as
    /// `(a LIKE 'x') AND b`. The escape character is usually a string literal but is
    /// parsed as a general expression for round-trip fidelity.
    fn parse_like_predicate(
        &mut self,
        expr: Expr<D::Ext>,
        negated: bool,
        spelling: LikeSpelling,
    ) -> ParseResult<Expr<D::Ext>> {
        // PostgreSQL quantifies the pattern-match operators over an array operand:
        // `<expr> [NOT] LIKE|ILIKE {ANY | ALL | SOME} (<array>)`, its pattern-match
        // `ScalarArrayOpExpr`. Only `LIKE`/`ILIKE` — `SIMILAR TO ANY` is a reject
        // (engine-probed) — and only under the behaviour gate; the operand is a value
        // expression (the corpus forms are `ARRAY[…]` / `'{…}'` literals), so a bare
        // subquery operand (`LIKE ANY (SELECT …)`) is not modelled here.
        if self.features().predicate_syntax.pattern_match_quantifier
            && matches!(spelling, LikeSpelling::Like | LikeSpelling::ILike)
        {
            if let Some(quantifier) = self.parse_quantifier()? {
                self.expect_punct(Punctuation::LParen, "`(` after `ANY`, `ALL`, or `SOME`")?;
                let pattern = self.parse_expr()?;
                self.expect_punct(
                    Punctuation::RParen,
                    "`)` to close the quantified pattern operand",
                )?;
                let span = expr.span().union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(Expr::QuantifiedLike {
                    left: Box::new(expr),
                    pattern: Box::new(pattern),
                    quantifier,
                    negated,
                    spelling,
                    meta,
                });
            }
        }
        // The pattern binds at the range predicate's own right power so a second same-tier
        // predicate cannot fold into it (`a LIKE b LIKE c` must reject at the outer climb,
        // not parse as `a LIKE (b LIKE c)`); it equals the comparison's right power under
        // dialects that keep pattern-match at comparison level, leaving them unchanged.
        let pattern_bp = self.features().binding_powers.range_predicate().right;
        let pattern = self.parse_expr_bp(pattern_bp)?;
        let escape = if self.eat_keyword(Keyword::Escape)? {
            Some(Box::new(self.parse_expr_bp(pattern_bp)?))
        } else {
            None
        };
        let span = expr.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::Like {
            expr: Box::new(expr),
            pattern: Box::new(pattern),
            escape,
            negated,
            spelling,
            meta,
        })
    }
    /// Parse the operand of a quantified `<op> {ANY | ALL | SOME} (…)`, the quantifier
    /// already consumed, and build the resulting node. Split from the infix climb in
    /// [`parse_expr_bp_inner`](Self::parse_expr_bp_inner) so its operand locals (subquery,
    /// array, backtrack checkpoint) stay off that hot recursive frame — the stack canary
    /// budget the file NOTE guards.
    ///
    /// The parenthesized content splits the SQL-standard subquery form (`= ANY (SELECT …)`)
    /// from the scalar list/array form (`= ANY (b)` / `= ANY ([1, 2, 3])`) exactly as the
    /// `IN (…)` site splits `InSubquery` from `InList` — the two are distinct engine nodes
    /// (PostgreSQL `AnySublink`/`AllSublink` vs `ScalarArrayOpExpr`), hence distinct AST
    /// variants. A bare leading query keyword is unambiguously a subquery; a leading `(` is
    /// speculative — `(SELECT …)::int[]` is a *cast of a scalar subquery* (an expression
    /// operand) while `(SELECT …) UNION (SELECT …)` is a parenthesized-set-operand subquery,
    /// and the two diverge only after the inner group, so this backtracks as `IN (…)` does.
    fn parse_quantified_operator_tail(
        &mut self,
        lhs: Expr<D::Ext>,
        op: BinaryOperator,
        op_token: Token,
        quantifier: Quantifier,
    ) -> ParseResult<Expr<D::Ext>> {
        self.expect_punct(Punctuation::LParen, "`(` after `ANY`, `ALL`, or `SOME`")?;
        if self.peek_starts_query()? {
            let subquery = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the quantified subquery")?;
            let span = lhs.span().union(op_token.span).union(subquery.span());
            let meta = self.make_meta(span);
            return Ok(Expr::QuantifiedComparison {
                left: Box::new(lhs),
                op,
                quantifier,
                subquery: Box::new(subquery),
                meta,
            });
        }
        if self.features().select_syntax.parenthesized_query_operands
            && self.peek_is_punct(Punctuation::LParen)?
        {
            let checkpoint = self.checkpoint();
            if let Some(subquery) = self.try_parenthesized_query()? {
                let span = lhs.span().union(op_token.span).union(subquery.span());
                let meta = self.make_meta(span);
                return Ok(Expr::QuantifiedComparison {
                    left: Box::new(lhs),
                    op,
                    quantifier,
                    subquery: Box::new(subquery),
                    meta,
                });
            }
            self.rewind(checkpoint);
        }
        if !self.features().operator_syntax.quantified_comparison_lists {
            return Err(self.unexpected("a subquery"));
        }
        let array = self.parse_expr()?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the quantified list operand",
        )?;
        let span = lhs.span().union(op_token.span).union(array.span());
        let meta = self.make_meta(span);
        Ok(Expr::QuantifiedList {
            left: Box::new(lhs),
            op,
            quantifier,
            array: Box::new(array),
            meta,
        })
    }
    /// Whether `op` may take a trailing `{ANY | ALL | SOME} (…)` quantifier. The six
    /// comparison operators always may (the SQL-standard quantified comparison); under
    /// [`OperatorSyntax::quantified_arbitrary_operator`](crate::ast::dialect::OperatorSyntax) (PostgreSQL) any operator except
    /// the boolean keywords `AND`/`OR` may too, matching PostgreSQL's `MathOp`/`Op`
    /// grammar (engine-probed via libpg_query).
    fn is_quantifiable_operator(&self, op: &BinaryOperator) -> bool {
        is_comparison_operator(op)
            || (self
                .features()
                .operator_syntax
                .quantified_arbitrary_operator
                && !matches!(op, BinaryOperator::And | BinaryOperator::Or))
    }
    /// Parse an `ANY`/`ALL`/`SOME` quantifier if present.
    fn parse_quantifier(&mut self) -> ParseResult<Option<Quantifier>> {
        if !self.features().operator_syntax.quantified_comparisons {
            // Dialects without quantified subquery comparisons (SQLite) do not read
            // `ANY`/`ALL`/`SOME` as a quantifier here: `ALL` is reserved and rejects, and
            // `ANY (<subquery>)` surfaces as the usual parse error at the bare subquery.
            return Ok(None);
        }
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        let quantifier = match token.kind {
            TokenKind::Keyword(Keyword::Any) => Quantifier::Any,
            TokenKind::Keyword(Keyword::All) => Quantifier::All,
            TokenKind::Keyword(Keyword::Some) => Quantifier::Some,
            _ => return Ok(None),
        };
        self.advance()?;
        Ok(Some(quantifier))
    }
    /// Parse a parenthesized query after a construct that requires one.
    pub(super) fn parse_query_in_parens(
        &mut self,
        open_expected: &'static str,
        close_expected: &'static str,
    ) -> ParseResult<crate::ast::Query<D::Ext>> {
        self.expect_punct(Punctuation::LParen, open_expected)?;
        // A leading `(` opens a parenthesized set-operation operand
        // (`EXISTS( (SELECT …) UNION (SELECT …) )`) on the dialects that admit one — the
        // set-operand grammar in `parse_query` handles it, mirroring the acceptance
        // condition in `parse_parenthesized_set_operand`. Without the carve-out a leading
        // `(` here would reject as "a subquery" before that grammar is reached.
        if !(self.peek_starts_query()?
            || (self.features().select_syntax.parenthesized_query_operands
                && self.peek_is_punct(Punctuation::LParen)?))
        {
            return Err(self.unexpected("a subquery"));
        }
        let query = self.parse_query()?;
        self.expect_punct(Punctuation::RParen, close_expected)?;
        Ok(query)
    }
    /// Peek the PostgreSQL `OPERATOR(...)` explicit-operator infix form, returning
    /// its binding power when the dialect enables it and the cursor is `OPERATOR (`.
    ///
    /// The construct binds at the "any other operator" rank — the same
    /// `string_concat` (`%left Op OPERATOR`) level as `||` — so the binary
    /// binding-power table is the single source of its precedence; there is no
    /// separate field to drift. `OPERATOR` is non-reserved, but in infix position
    /// `OPERATOR(` is unambiguous (a bare word cannot follow a parsed operand). The
    /// cursor is not advanced.
    fn peek_operator_construct(&mut self) -> ParseResult<Option<BindingPower>> {
        if self.features().operator_syntax.operator_construct
            && self.peek_is_keyword(Keyword::Operator)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return Ok(Some(self.features().binding_powers.string_concat));
        }
        Ok(None)
    }
    /// Parse `OPERATOR(schema.op)` and fold it into its left operand `lhs`, climbing
    /// the right operand at the construct's right binding power `right_bp`.
    ///
    /// The operator is `(ColId '.')* <symbol>` (PostgreSQL `any_operator`): an
    /// optional schema qualification followed by the symbolic operator itself, whose
    /// exact spelling is interned so it round-trips (the operator is always symbolic,
    /// never a word, hence a bare [`Symbol`] rather than an [`Ident`]).
    fn parse_operator_construct(
        &mut self,
        lhs: Expr<D::Ext>,
        right_bp: u8,
    ) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // `OPERATOR`
        self.expect_punct(Punctuation::LParen, "`(` after `OPERATOR`")?;
        // The optional schema qualification is a `(ColId '.')*` chain; a part is
        // taken only when a name is immediately followed by `.`, so the symbolic
        // operator (never a name) ends the chain.
        let mut schema = ThinVec::new();
        while self
            .peek()?
            .is_some_and(|token| self.token_can_be_column_name(token))
            && self.peek_nth_is_punct(1, Punctuation::Dot)?
        {
            schema.push(self.parse_ident()?);
            self.expect_punct(Punctuation::Dot, "`.` in the qualified operator name")?;
        }
        let op = self.parse_operator_symbol()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `OPERATOR(...)`")?;
        let right = self.parse_expr_bp(right_bp)?;
        let span = lhs.span().union(right.span());
        let named_operator = NamedOperatorExpr {
            left: lhs,
            schema: ObjectName(schema),
            op,
            right,
            spelling: NamedOperatorSpelling::OperatorKeyword,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::NamedOperator {
            named_operator: Box::new(named_operator),
            meta,
        })
    }
    /// Whether the next token is a bare PostgreSQL symbolic operator that builds an
    /// [`Expr::NamedOperator`] — the regex `~`/`!~`/`~*`/`!~*`, a geometric/network/
    /// text-search op (`&&`, `<->`, `<<|`, `^@`, …), or a fully user-defined operator
    /// ([`Operator::Custom`]). Reports the "any other operator" binding power (the
    /// `string_concat`/`Op` rank, left-associative). Only under `custom_operators`, where the
    /// tokenizer's maximal-munch scanner produces these tokens; a built-in operator is
    /// claimed by [`peek_infix_operator`](Self::peek_infix_operator) first. Does not consume.
    fn peek_bare_infix_operator(&mut self) -> ParseResult<Option<BindingPower>> {
        if !self.features().operator_syntax.custom_operators {
            return Ok(None);
        }
        Ok(match self.peek()? {
            Some(token) => match token.kind {
                // `~`/`!` are the single-byte regex-family leads (`~` bare regex-match, `!`
                // never a bare infix operator on its own but kept for run symmetry), `&&` the
                // array-overlap operator, and `Custom` every multi-byte residue.
                TokenKind::Operator(
                    Operator::Custom | Operator::Tilde | Operator::Bang | Operator::AmpAmp,
                ) => Some(self.features().binding_powers.any_operator),
                _ => None,
            },
            None => None,
        })
    }
    /// Parse a bare PostgreSQL symbolic operator and fold it into its left operand `lhs`,
    /// climbing the right operand at the operator's right binding power `right_bp`. The
    /// exact spelling is taken from the operator token's span and interned so it
    /// round-trips (the operator is always symbolic, hence a bare [`Symbol`]).
    fn parse_bare_infix_operator(
        &mut self,
        lhs: Expr<D::Ext>,
        right_bp: u8,
    ) -> ParseResult<Expr<D::Ext>> {
        let op_token = self
            .advance()?
            .expect("peek_bare_infix_operator confirmed an operator token is present");
        let op = self.intern_text(self.span_text(op_token.span));
        let right = self.parse_expr_bp(right_bp)?;
        let span = lhs.span().union(right.span());
        let named_operator = NamedOperatorExpr {
            left: lhs,
            schema: ObjectName(ThinVec::new()),
            op,
            right,
            spelling: NamedOperatorSpelling::Bare,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::NamedOperator {
            named_operator: Box::new(named_operator),
            meta,
        })
    }
    /// Whether the cursor is a general symbolic operator eligible for DuckDB's postfix
    /// reduction AND no operand follows it — the operand-absent position that separates the
    /// postfix application (`1 !`) from the infix reading (`1 ! 2`). Does not consume. Gated on
    /// [`OperatorSyntax::postfix_operators`](crate::ast::dialect::OperatorSyntax::postfix_operators)
    /// by the caller.
    fn peek_postfix_symbolic_operator(&mut self) -> ParseResult<bool> {
        let Some(token) = self.peek()? else {
            return Ok(false);
        };
        let TokenKind::Operator(op) = token.kind else {
            return Ok(false);
        };
        if !is_postfix_symbolic_operator(op) {
            return Ok(false);
        }
        Ok(!self.peek_operand_follows_operator()?)
    }
    /// Whether an operand begins immediately after the peeked operator — the infix/postfix
    /// fork. Speculative: it advances past the operator and asks the real
    /// [`parse_prefix`](Self::parse_prefix) whether a primary starts, then rewinds the cursor
    /// and any clause marks it recorded, so the classification can never drift from the actual
    /// FIRST set of a primary. DuckDB reduces `a Op` to a postfix application exactly when the
    /// follower is not in FIRST(a_expr) (its bison FOLLOW-set), so an operand-absent follower
    /// is the postfix reading and an operand-present one stays infix.
    fn peek_operand_follows_operator(&mut self) -> ParseResult<bool> {
        // Fast path: a value / name / parameter / `(` immediately after the operator is
        // unconditionally the start of a primary (`parse_prefix` dispatches every one of these
        // to an operand), so the infix reading holds without the speculative parse — this keeps
        // the common `a || b` off the speculative path. Only an ambiguous follower (another
        // operator, a keyword, a non-`(` punctuation, or end of input) needs the real check.
        if let Some(next) = self.peek_nth(1)? {
            match next.kind {
                TokenKind::Word
                | TokenKind::QuotedIdent
                | TokenKind::Number
                | TokenKind::String
                | TokenKind::Parameter
                | TokenKind::PositionalColumn
                | TokenKind::Variable
                | TokenKind::Punctuation(Punctuation::LParen) => return Ok(true),
                _ => {}
            }
        }
        let checkpoint = self.checkpoint();
        let clause_marks = self.clause_marks_checkpoint();
        self.advance()?; // the operator token
        let follows = self.parse_prefix().is_ok();
        self.rewind(checkpoint);
        self.truncate_clause_marks(clause_marks);
        Ok(follows)
    }
    /// Fold a postfix symbolic operator (the current token, already confirmed operand-absent by
    /// [`peek_postfix_symbolic_operator`](Self::peek_postfix_symbolic_operator)) onto its left
    /// operand `lhs`. The exact spelling is interned from the operator token's span so it
    /// round-trips (always symbolic, hence a bare [`Symbol`]).
    fn parse_postfix_symbolic_operator(&mut self, lhs: Expr<D::Ext>) -> ParseResult<Expr<D::Ext>> {
        let op_token = self
            .advance()?
            .expect("peek_postfix_symbolic_operator confirmed an operator token is present");
        let op = self.intern_text(self.span_text(op_token.span));
        let span = lhs.span().union(op_token.span);
        let postfix_operator = PostfixOperatorExpr {
            operand: lhs,
            op,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::PostfixOperator {
            postfix_operator: Box::new(postfix_operator),
            meta,
        })
    }
    /// Parse the symbolic operator inside `OPERATOR(...)`: a contiguous run of
    /// operator tokens (`+`, `<=`, `->>`), interned with its exact spelling. At least
    /// one operator token is required.
    pub(crate) fn parse_operator_symbol(&mut self) -> ParseResult<Symbol> {
        let start = self.current_span()?;
        if !self.peek_is_operator_token()? {
            return Err(self.unexpected("an operator inside `OPERATOR(...)`"));
        }
        while self.peek_is_operator_token()? {
            self.advance()?;
        }
        let span = start.union(self.preceding_span());
        // An operator run is not a settled Word/Keyword token, so it takes the full
        // keyword-checking intern (symbolic operator text matches no keyword anyway).
        let text = self.span_text(span);
        Ok(self.intern_text(text))
    }
    /// Whether the cursor is any symbolic operator token (the lexemes that spell an
    /// `OPERATOR(...)` operator).
    fn peek_is_operator_token(&mut self) -> ParseResult<bool> {
        Ok(self
            .peek()?
            .is_some_and(|token| matches!(token.kind, TokenKind::Operator(_))))
    }
    /// Map the current token to its infix [`BinaryOperator`], without consuming it.
    ///
    /// Both spellings feed the one binding-power table: symbolic operator tokens
    /// and the keyword tokens `AND`/`OR`. Any other word is not an infix operator
    /// and so ends the expression.
    fn peek_infix_operator(&mut self) -> ParseResult<Option<BinaryOperator>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        Ok(match token.kind {
            TokenKind::Operator(op) => infix_operator(op, self.features()),
            TokenKind::Keyword(Keyword::And) => Some(BinaryOperator::And),
            TokenKind::Keyword(Keyword::Or) => Some(BinaryOperator::Or),
            // MySQL's keyword infix operators (`DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP`) are
            // dialect data, like the symbolic `||`/`&&` above: a dialect that does not
            // enable them returns `None`, ending the expression rather than mis-binding.
            TokenKind::Keyword(keyword) => {
                self.features().keyword_operators.binary_operator(keyword)
            }
            TokenKind::Word => None,
            TokenKind::Number => None,
            TokenKind::String => None,
            TokenKind::QuotedIdent => None,
            TokenKind::Parameter => None,
            TokenKind::PositionalColumn => None,
            TokenKind::Variable | TokenKind::StageReference => None,
            TokenKind::Punctuation(_) => None,
            TokenKind::Unknown => None,
        })
    }
    /// The infix [`BinaryOperator`] of the keyword operator `n` tokens ahead, if that
    /// token is a keyword the active dialect treats as one (`GLOB`/`MATCH`/`REGEXP`
    /// for SQLite, `DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP` for MySQL). Does not consume —
    /// used to detect a `NOT <keyword-operator>` negation before the operand is built.
    fn keyword_operator_at(&mut self, n: usize) -> ParseResult<Option<BinaryOperator>> {
        Ok(match self.peek_nth(n)? {
            Some(token) => match token.kind {
                TokenKind::Keyword(keyword) => {
                    self.features().keyword_operators.binary_operator(keyword)
                }
                _ => None,
            },
            None => None,
        })
    }
    /// Is `lhs` itself a non-associative operator at precedence `level`?
    ///
    /// True signals an illegal chain such as `a < b < c`: the left operand is
    /// already a comparison at the same precedence, so binding a second one would
    /// impose the left-associativity the dialect forbids. The caller
    /// turns this into a clean `ParseError` instead.
    ///
    /// A custom operator node ([`Expr::Other`]) reports its precedence through the
    /// dialect (the parse-time analogue of `Render::operand_binding_power`), so a
    /// non-associative *extension* operator rejects chains by the same rule; a
    /// self-delimiting extension node returns `None` and never chains.
    fn lhs_chains_nonassoc(&self, lhs: &Expr<D::Ext>, level: u8) -> bool {
        let bp = match lhs {
            Expr::BinaryOp { op, .. }
            | Expr::QuantifiedComparison { op, .. }
            | Expr::QuantifiedList { op, .. } => self.features().binding_power(op),
            // The range/pattern/membership predicates report the `range_predicate` rank
            // (above comparison under PostgreSQL/Lenient), so a second one chained onto them
            // rejects at that tier (`a BETWEEN b AND c BETWEEN d AND e`); the `IS …` family
            // reports the `predicate` rank (below comparison under PostgreSQL/DuckDB/Lenient).
            Expr::InSubquery { .. }
            | Expr::InList { .. }
            | Expr::Between { .. }
            | Expr::Like { .. }
            | Expr::QuantifiedLike { .. } => self.features().binding_powers.range_predicate(),
            Expr::IsNull { .. } | Expr::IsTruth { .. } | Expr::IsNormalized { .. } => {
                self.features().binding_powers.predicate()
            }
            Expr::Other { ext, .. } => match D::extension_operand_binding_power(ext) {
                Some(bp) => bp,
                None => return false,
            },
            _ => return false,
        };
        bp.assoc == Assoc::NonAssoc && bp.left == level
    }

    /// Parse the tail of `<left> OVERLAPS <right>`, the cursor on the `OVERLAPS` keyword
    /// and `left` already parsed as the left operand.
    ///
    /// Both operands must be exactly-two-element rows: PostgreSQL's `row OVERLAPS row`
    /// production restricts each side to a `ROW(...)` / bare parenthesized-pair `row`
    /// nonterminal and raises a grammar-level error on any other arity, which
    /// `pg_query` surfaces as a parse reject. So a scalar, a single-element grouping
    /// `(a)` (a plain grouping, not a row), a three-element row, or the already-built
    /// boolean of a preceding `OVERLAPS` (the non-chaining guard) is a parse error here.
    /// `grouped` distinguishes the direct row `(a, b)` from a re-parenthesized
    /// `((a, b))`, which PostgreSQL also rejects (the outer parens make it an ordinary
    /// grouped `a_expr` rather than the `row` nonterminal). The right operand is parsed
    /// as a bare prefix/primary so a trailing looser operator (`... = TRUE`, `... + 1`,
    /// `... AND ...`) folds onto the boolean result in the caller's climb, not into the
    /// row — matching PostgreSQL, where the right side is likewise just a `row`.
    fn parse_overlaps_predicate(
        &mut self,
        left: Expr<D::Ext>,
        left_grouped: bool,
    ) -> ParseResult<Expr<D::Ext>> {
        if left_grouped || !is_two_element_row(&left) {
            return Err(
                self.unexpected("a two-element row `(a, b)` or `ROW(a, b)` before `OVERLAPS`")
            );
        }
        self.expect_keyword(Keyword::Overlaps)?;
        let ParsedExpr {
            expr: right,
            grouped: right_grouped,
        } = self.parse_prefix()?;
        if right_grouped || !is_two_element_row(&right) {
            return Err(
                self.unexpected("a two-element row `(a, b)` or `ROW(a, b)` after `OVERLAPS`")
            );
        }
        let span = left.span().union(right.span());
        let meta = self.make_meta(span);
        Ok(Expr::BinaryOp {
            left: Box::new(left),
            op: BinaryOperator::Overlaps,
            right: Box::new(right),
            meta,
        })
    }
}

/// Whether `expr` is a row constructor of exactly two fields — the operand shape the
/// `OVERLAPS` period predicate requires on each side (`(a, b)` or `ROW(a, b)`).
fn is_two_element_row<X: Extension>(expr: &Expr<X>) -> bool {
    matches!(expr, Expr::Row { row, .. } if row.fields.len() == 2)
}

fn is_comparison_operator(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq(_)
            | BinaryOperator::NotEq(_)
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
    )
}

/// Whether `op` is a general symbolic operator eligible for DuckDB's postfix reduction
/// ([`OperatorSyntax::postfix_operators`](crate::ast::dialect::OperatorSyntax::postfix_operators)).
///
/// The `Op`-class symbolic operators DuckDB reduces in postfix position when no operand
/// follows: the [`Custom`](Operator::Custom) residue (`!!`, `<->`, `~*`, `@#@`, …), the lone
/// `~`/`!`/`&&`, and the dedicated symbolic infix operators (`& | << >> || <@ @> ^@`). The
/// comparison and arithmetic grammar tokens (`= < > <= >= <> != + - * / % ^`) and the JSON
/// arrows `->`/`->>` are excluded — DuckDB rejects a trailing one (engine-measured on 1.5.4:
/// `1 ->` / `1 <` syntax-error, `1 <@` / `1 &` reduce to `<@__postfix` / `&__postfix`).
fn is_postfix_symbolic_operator(op: Operator) -> bool {
    matches!(
        op,
        Operator::Custom
            | Operator::Tilde
            | Operator::Bang
            | Operator::AmpAmp
            | Operator::Amp
            | Operator::Pipe
            | Operator::ShiftLeft
            | Operator::ShiftRight
            | Operator::Concat
            | Operator::LtAt
            | Operator::AtGt
            | Operator::CaretAt
    )
}

/// The lambda parameters of a `->` left operand, when it has exactly the shape
/// DuckDB's binder admits: a bare unqualified name, a parenthesized name list
/// `(x, y)` (which parses as an implicit row), or the equivalent explicit
/// `ROW(x, y)` — probed on 1.5.4, whose reject message is the spec: "Parameters
/// must be unqualified comma-separated names like x or (x, y)". Anything else
/// (a qualified name, a constant, an expression, an empty `ROW()`) returns `None`
/// and keeps the JSON-arrow reading. `grouped` distinguishes the bare single name
/// from its parenthesized spelling (`x ->` vs `(x) ->`), which the AST records for
/// round-trip; the idents are cloned off the borrowed operand — at most a few
/// 20-byte values on a cold branch — so the caller keeps `lhs` for the fall-through
/// binary fold.
fn lambda_params<X: Extension>(
    lhs: &Expr<X>,
    grouped: bool,
) -> Option<(ThinVec<Ident>, LambdaParamSpelling)> {
    match lhs {
        Expr::Column { name, .. } if name.0.len() == 1 => {
            let spelling = if grouped {
                LambdaParamSpelling::Parenthesized
            } else {
                LambdaParamSpelling::Bare
            };
            Some((name.0.clone(), spelling))
        }
        Expr::Row { row, .. } if !row.fields.is_empty() => {
            let params = row
                .fields
                .iter()
                .map(|field| match field {
                    Expr::Column { name, .. } if name.0.len() == 1 => Some(name.0[0].clone()),
                    _ => None,
                })
                .collect::<Option<ThinVec<Ident>>>()?;
            let spelling = if row.explicit {
                LambdaParamSpelling::RowKeyword
            } else {
                LambdaParamSpelling::Parenthesized
            };
            Some((params, spelling))
        }
        _ => None,
    }
}

/// Map a symbolic operator token to its infix [`BinaryOperator`], if the dialect defines
/// one.
///
/// Exhaustive over [`Operator`] so a new spelling is a build error, not a silent
/// gap. The lexer-class bytes with no infix meaning under the dialect (`!`, a prefix-only
/// `~`) return `None`, ending the expression rather than mis-binding.
///
/// `||` and `&&` are the operators whose *meaning* is dialect data: `||`
/// ([`PipeOperator`](crate::ast::dialect::PipeOperator)) concatenates in ANSI/PostgreSQL
/// but ORs in MySQL (without `PIPES_AS_CONCAT`); `&&`
/// ([`DoubleAmpersand`](crate::ast::dialect::DoubleAmpersand)) is logical AND in MySQL and
/// not a scalar operator elsewhere. The bitwise binaries (`|`/`&`/`<<`/`>>`) are infix only
/// under `bitwise_operators`; the `^` byte's meaning is one dialect axis
/// ([`CaretOperator`](crate::ast::dialect::CaretOperator)) — arithmetic power
/// (PostgreSQL/DuckDB), bitwise XOR (MySQL), or no infix meaning — and `#` is bitwise XOR
/// only under `hash_bitwise_xor` (PostgreSQL); a dialect with no meaning for the byte ends
/// the expression.
/// The chosen canonical operator carries the correct binding power automatically, so there
/// is no separate precedence override.
fn infix_operator(op: Operator, features: &FeatureSet) -> Option<BinaryOperator> {
    let (pipe, amp) = (features.pipe_operator, features.double_ampersand);
    let bitwise = features.operator_syntax.bitwise_operators;
    let (caret, hash_xor) = (features.caret_operator, features.hash_bitwise_xor);
    let mapped = match op {
        Operator::Plus => BinaryOperator::Plus,
        Operator::Minus => BinaryOperator::Minus,
        Operator::Star => BinaryOperator::Multiply,
        Operator::Slash => BinaryOperator::Divide,
        // DuckDB's `//` integer division. Its lexeme is munched only under the
        // `integer_divide_slash` gate, so a token reaching here already means the dialect
        // enables it — the mapping is unconditional, like `==`/`<=>`. Folds onto the one
        // canonical integer-divide operator (ADR-0011); the spelling tag keeps `//` and the
        // MySQL `DIV` keyword round-tripping to their own surface form.
        Operator::SlashSlash => BinaryOperator::IntegerDivide(IntegerDivideSpelling::SlashSlash),
        Operator::Percent => BinaryOperator::Modulo(ModuloSpelling::Percent),
        // Both equality lexemes fold onto the one canonical operator (ADR-0011); the
        // spelling tag records which the source used so `=`/`==` round-trip exactly.
        Operator::Eq => BinaryOperator::Eq(EqualsSpelling::Single),
        Operator::EqEq => BinaryOperator::Eq(EqualsSpelling::Double),
        // Both inequality lexemes fold onto the one canonical operator (ADR-0011); the
        // spelling tag records which the source used so `<>`/`!=` round-trip exactly.
        // The one `Operator::NotEq` token covers both lexemes, so the provisional
        // `AngleBracket` here is corrected from the token's source text in `fold_infix`.
        Operator::NotEq => BinaryOperator::NotEq(NotEqSpelling::AngleBracket),
        Operator::Lt => BinaryOperator::Lt,
        Operator::LtEq => BinaryOperator::LtEq,
        Operator::Gt => BinaryOperator::Gt,
        Operator::GtEq => BinaryOperator::GtEq,
        // MySQL `<=>` null-safe equality folds onto the canonical null-safe operator
        // (ADR-0011), the same one `IS NOT DISTINCT FROM` produces; the spelling tag keeps
        // the two round-tripping to their own surface form. Its lexeme is munched only
        // under the MySQL gate, so a token reaching here already means the dialect enables
        // it — the mapping is unconditional, like the containment/JSON-arrow lexemes.
        Operator::LtEqGt => {
            BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::NullSafeEq)
        }
        Operator::Concat => pipe.binary_operator(),
        // `&&` already returns `Option`: `None` ends the expression under a dialect
        // that does not treat it as a scalar operator (ANSI/PostgreSQL).
        Operator::AmpAmp => return amp.binary_operator(),
        // The PostgreSQL `@`-family and JSON operators. Their lexemes are munched only
        // under the containment / JSON-arrow dialect flags (the tokenizer gate), so a
        // token reaching here already means the dialect enables them — the mapping is
        // unconditional, like the `Arrow` (`=>`) lexeme's feature-gated lexing.
        Operator::AtGt => BinaryOperator::Contains,
        Operator::LtAt => BinaryOperator::ContainedBy,
        // DuckDB `^@` starts-with. Lexeme munched only under `starts_with_operator`, so a
        // token here already means the dialect enables it.
        Operator::CaretAt => BinaryOperator::StartsWith,
        Operator::MinusGt => BinaryOperator::JsonGet,
        Operator::MinusGtGt => BinaryOperator::JsonGetText,
        // The PostgreSQL `jsonb` existence/path/search operators. Their lexemes are munched
        // only under the `jsonb_operators` dialect gate, so a token reaching here already
        // means the dialect enables them — the mapping is unconditional, like `@>`/`->`.
        Operator::Question => BinaryOperator::JsonExists,
        Operator::QuestionPipe => BinaryOperator::JsonExistsAny,
        Operator::QuestionAmp => BinaryOperator::JsonExistsAll,
        Operator::AtQuestion => BinaryOperator::JsonPathExists,
        Operator::AtAt => BinaryOperator::JsonPathMatch,
        Operator::HashGt => BinaryOperator::JsonExtractPath,
        Operator::HashGtGt => BinaryOperator::JsonExtractPathText,
        Operator::HashMinus => BinaryOperator::JsonDeletePath,
        // The binary bitwise operators are infix only when the dialect admits the family;
        // otherwise `None` ends the expression (a lone `|` etc. is not an operator).
        Operator::Pipe => return bitwise.then_some(BinaryOperator::BitwiseOr),
        Operator::Amp => return bitwise.then_some(BinaryOperator::BitwiseAnd),
        Operator::ShiftLeft => return bitwise.then_some(BinaryOperator::BitwiseShiftLeft),
        Operator::ShiftRight => return bitwise.then_some(BinaryOperator::BitwiseShiftRight),
        // The `^` byte's meaning is a single dialect axis ([`CaretOperator`]): arithmetic
        // power at its own precedence tier (PostgreSQL/DuckDB), bitwise XOR under a `Caret`
        // spelling (MySQL/Lenient), or no infix meaning at all (ends the expression). The
        // meaning-enum makes the "both power and XOR" state unrepresentable, so no ordering
        // between the two readings is needed here. `#` lexes only under a `Hash`-XOR dialect,
        // so it always maps there.
        Operator::Caret => return caret.binary_operator(),
        Operator::Hash => {
            return hash_xor.then_some(BinaryOperator::BitwiseXor(BitwiseXorSpelling::Hash));
        }
        // Prefix-only `~`, the named-argument arrows (recognised only inside an argument
        // list, `parse_function_arg`), and the `|>` query pipe separator (consumed by the
        // query-tail pipe parser, never an expression operator): in infix position each
        // ends the expression. `Bang`/`Tilde`/`Custom` are the general PostgreSQL bare
        // operators (regex `~`/`!~`, geometric, user-defined) — under `custom_operators`
        // the caller's `peek_bare_infix_operator` claims them ahead of this into an
        // `Expr::NamedOperator`, so reaching here means the gate is off and each ends the
        // expression.
        Operator::Bang
        | Operator::Tilde
        | Operator::Custom
        | Operator::Arrow
        | Operator::ColonEquals
        | Operator::PipeArrow => {
            return None;
        }
    };
    Some(mapped)
}
