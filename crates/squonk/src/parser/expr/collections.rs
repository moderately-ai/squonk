// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Collection-constructor primaries: `ARRAY[...]`/`ARRAY(<query>)`, the DuckDB
//! bare-bracket list `[…]` (and its comprehension form), the struct `{…}` and
//! `MAP {…}` literals, the `ROW(...)` constructor, and the keyword lambda —
//! the collection-opener class of [`parse_prefix`](crate::parser::Parser::parse_prefix)'s
//! dispatch.

use crate::ast::{
    ArrayExpr, ArraySpelling, ComprehensionSource, Expr, Keyword, LambdaExpr, LambdaParamSpelling,
    ListComprehension, MapEntry, MapExpr, QuoteStyle, RowExpr, Span, Spanned, StructConstructorArg,
    StructConstructorExpr, StructConstructorField, StructExpr, StructField, StructKeySpelling,
    Symbol,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::parser::from::materialize_quoted_ident;
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use std::borrow::Cow;
use thin_vec::{ThinVec, thin_vec};

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse an `ARRAY[...]` / `ARRAY(<query>)` constructor, or fall back to reading
    /// `array` as an ordinary name when neither bracket form follows.
    pub(super) fn parse_array_or_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        if self.features().expression_syntax.array_constructor
            && (self.peek_nth_is_punct(1, Punctuation::LBracket)?
                || self.peek_nth_is_punct(1, Punctuation::LParen)?)
        {
            self.parse_array()
        } else {
            self.parse_word_prefix(token)
        }
    }
    /// Parse an `ARRAY[...]` element list or an `ARRAY(<query>)` subquery.
    fn parse_array(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_array is reached only with a current ARRAY token")
            .span;
        let array = if self.eat_punct(Punctuation::LBracket)? {
            let elements = if self.features().expression_syntax.multidim_array_literals {
                // PostgreSQL multidimensional literal: the element list may be a level of
                // bare-bracket sub-rows (`ARRAY[[1,2],[3,4]]`), uniform per level.
                self.parse_array_multidim_elements()?
            } else if self.peek_is_punct(Punctuation::RBracket)? {
                ThinVec::new()
            } else {
                // DuckDB tolerates a single trailing comma before the `]` here too.
                self.parse_comma_separated_trailing(Self::parse_expr, |p| {
                    p.peek_is_punct(Punctuation::RBracket)
                })?
            };
            self.expect_punct(Punctuation::RBracket, "`]` to close the array constructor")?;
            let span = start.union(self.preceding_span());
            ArrayExpr::Elements {
                elements,
                spelling: ArraySpelling::Keyword,
                meta: self.make_meta(span),
            }
        } else {
            let query = self.parse_query_in_parens(
                "`[` or `(` after `ARRAY`",
                "`)` to close the array subquery",
            )?;
            let span = start.union(self.preceding_span());
            ArrayExpr::Subquery {
                query: Box::new(query),
                meta: self.make_meta(span),
            }
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::Array {
            array: Box::new(array),
            meta,
        })
    }
    /// Parse a DuckDB bare-bracket list literal `[a, b, …]` (possibly empty `[]`), or the
    /// list comprehension `[element for var in source (if filter)?]` that the same bracket
    /// opens — the `for` after the first element selects it.
    pub(super) fn parse_list_literal(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_list_literal is reached only with a current `[` token")
            .span;
        // An empty list has no first element, so it can never be a comprehension.
        if self.peek_is_punct(Punctuation::RBracket)? {
            self.advance()?; // `]`
            return Ok(self.build_list_elements(start, ThinVec::new()));
        }
        let first = self.parse_expr()?;
        // The DuckDB list comprehension: a `for` keyword after the first element replaces
        // the element list. `for` is a reserved keyword, so a plain list element can never
        // collide (a bare `for` is not an identifier; a quoted `"for"` stays a list). The
        // subscript `a[…]` is postfix — it needs a base and never enters this
        // primary-position bracket — so there is no `[… for …]` / slice ambiguity to
        // disambiguate.
        if self.peek_is_keyword(Keyword::For)? {
            return self.parse_list_comprehension_tail(start, first);
        }
        let mut elements = thin_vec![first];
        while self.eat_punct(Punctuation::Comma)? {
            // DuckDB tolerates a single trailing comma before the list's `]`.
            if self.trailing_comma_at(Punctuation::RBracket)? {
                break;
            }
            elements.push(self.parse_expr()?);
        }
        self.expect_punct(Punctuation::RBracket, "`]` to close the list literal")?;
        Ok(self.build_list_elements(start, elements))
    }
    /// Assemble a bare-bracket [`ArrayExpr::Elements`] from the collected elements, the
    /// span running from the opening `[` (`start`) to the just-consumed `]`.
    fn build_list_elements(
        &mut self,
        start: Span,
        elements: ThinVec<Expr<D::Ext>>,
    ) -> Expr<D::Ext> {
        let span = start.union(self.preceding_span());
        let array = ArrayExpr::Elements {
            elements,
            spelling: ArraySpelling::Bracket,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Expr::Array {
            array: Box::new(array),
            meta,
        }
    }
    /// Parse the contents of one PostgreSQL multidimensional array-literal bracket level —
    /// cursor just past the opening `[`, stopping at (not consuming) the closing `]`.
    ///
    /// A level is uniform (matching PG's `array_expr` grammar): the first element's leading
    /// token fixes the whole level. A leading `[` makes every element a bare-bracket sub-row
    /// (recursing through [`parse_array_row`](Self::parse_array_row)); otherwise every element
    /// is a scalar `a_expr`. A mixed level is a parse error — a stray sub-row surfaces when
    /// [`parse_array_row`] fails its `[`, and a stray scalar in a row level fails
    /// `parse_array_row`'s `[` too; a bare `[` in the scalar branch is rejected by
    /// [`parse_expr`](Self::parse_expr) under a preset without `collection_literals`. Ragged
    /// nestings parse-accept: PostgreSQL rejects unequal sub-row lengths at bind time.
    ///
    /// [`parse_array_row`]: Self::parse_array_row
    fn parse_array_multidim_elements(&mut self) -> ParseResult<ThinVec<Expr<D::Ext>>> {
        if self.peek_is_punct(Punctuation::RBracket)? {
            return Ok(ThinVec::new());
        }
        let rows = self.peek_is_punct(Punctuation::LBracket)?;
        let mut elements = ThinVec::new();
        loop {
            elements.push(if rows {
                self.parse_array_row()?
            } else {
                self.parse_expr()?
            });
            if !self.eat_punct(Punctuation::Comma)? {
                break;
            }
        }
        Ok(elements)
    }
    /// Parse a bare-bracket sub-row `[...]` of a PostgreSQL multidimensional array literal,
    /// the cursor at the opening `[`. Produces a `Bracket`-spelled [`ArrayExpr::Elements`] so
    /// a nested row renders and shapes identically to a DuckDB list level; recurses through
    /// [`parse_array_multidim_elements`](Self::parse_array_multidim_elements) for deeper
    /// nesting. `expect_punct` on the `[` is the uniformity guard: entered for every element
    /// of a sub-row level, a non-`[` element (`ARRAY[[1,2],3]`, `ARRAY[[1,2],ARRAY[3,4]]`) is
    /// the mixed-level reject rather than a silent accept.
    fn parse_array_row(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LBracket, "`[` to open a nested array row")?;
        let elements = self.parse_array_multidim_elements()?;
        self.expect_punct(Punctuation::RBracket, "`]` to close the array row")?;
        Ok(self.build_list_elements(start, elements))
    }
    /// Parse the `for var in source (if filter)?]` tail of a list comprehension, the
    /// leading `[` and its `element` expression already consumed.
    fn parse_list_comprehension_tail(
        &mut self,
        start: Span,
        element: Expr<D::Ext>,
    ) -> ParseResult<Expr<D::Ext>> {
        self.expect_keyword(Keyword::For)?;
        // One or more loop variables: `for x in …` or `for x, i in …` (DuckDB value+index;
        // probed on 1.5.4). Three+ names parse-accept; the engine binder rejects them.
        let mut vars = thin_vec![self.parse_ident()?];
        while self.eat_punct(Punctuation::Comma)? {
            vars.push(self.parse_ident()?);
        }
        self.expect_keyword(Keyword::In)?;
        let source = self.parse_comprehension_source()?;
        let filter = if self.eat_keyword(Keyword::If)? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect_punct(Punctuation::RBracket, "`]` to close the list comprehension")?;
        let span = start.union(self.preceding_span());
        let comprehension = ListComprehension {
            element: Box::new(element),
            vars,
            source,
            filter,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        let array = ArrayExpr::Comprehension {
            comprehension: Box::new(comprehension),
            meta,
        };
        let meta = self.make_meta(span);
        Ok(Expr::Array {
            array: Box::new(array),
            meta,
        })
    }
    /// Parse a list-comprehension source: a general list-valued expression, or the DuckDB
    /// column-star `*` / `(* EXCLUDE (i))` form valid only inside `COLUMNS(…)` (a bind-time
    /// rule; the parser admits it anywhere, as DuckDB does). The bare `*` is not a value
    /// expression in the ordinary grammar, so it is recognised here directly rather than
    /// through [`parse_expr`](Self::parse_expr).
    fn parse_comprehension_source(&mut self) -> ParseResult<ComprehensionSource<D::Ext>> {
        // A bare column star, optionally with wildcard modifiers.
        if self.peek_is_op(Operator::Star)? {
            let star = self
                .advance()?
                .expect("peek_is_op confirmed the `*` is present");
            let options = self.parse_wildcard_modifier_tail(star.span)?;
            let span = star.span.union(self.preceding_span());
            return Ok(ComprehensionSource::Star {
                parenthesized: false,
                options,
                meta: self.make_meta(span),
            });
        }
        // A parenthesized column star `(*)` / `(* EXCLUDE (i))`: the modifiers are legal
        // only inside these parens (DuckDB rejects a bare `* EXCLUDE` here).
        if self.peek_is_punct(Punctuation::LParen)? && self.peek_nth_is_op(1, Operator::Star)? {
            let open = self.advance()?.expect("peek confirmed the `(` is present");
            let star = self.advance()?.expect("peek_nth confirmed the `*`");
            let options = self.parse_wildcard_modifier_tail(star.span)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the star source")?;
            let span = open.span.union(self.preceding_span());
            return Ok(ComprehensionSource::Star {
                parenthesized: true,
                options,
                meta: self.make_meta(span),
            });
        }
        let expr = self.parse_expr()?;
        let meta = self.make_meta(expr.span());
        Ok(ComprehensionSource::Expr {
            expr: Box::new(expr),
            meta,
        })
    }
    /// Parse DuckDB's python-style keyword lambda `lambda <params>: <body>`. The caller
    /// confirmed the current word is `lambda` under
    /// [`ExpressionSyntax::lambda_keyword`](crate::ast::dialect::ExpressionSyntax::lambda_keyword).
    /// Parameters are comma-separated bare names (DuckDB rejects the parenthesized
    /// `lambda (x): …`), and the body is the full expression after the `:`. Folds onto the
    /// shared [`Expr::Lambda`] node with the [`LambdaParamSpelling::Keyword`] spelling tag.
    /// Position-independent like the arrow form — it parses anywhere an
    /// expression sits, and only DuckDB's binder rejects a lambda no function consumed.
    pub(super) fn parse_keyword_lambda(
        &mut self,
        lambda_token: Token,
    ) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // `lambda`
        let mut params = ThinVec::new();
        loop {
            params.push(self.parse_ident()?);
            if !self.eat_punct(Punctuation::Comma)? {
                break;
            }
        }
        self.expect_punct(Punctuation::Colon, "`:` after the lambda parameters")?;
        let body = self.parse_expr()?;
        let span = lambda_token.span.union(self.preceding_span());
        let lambda = LambdaExpr {
            params,
            spelling: LambdaParamSpelling::Keyword,
            body,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::Lambda {
            lambda: Box::new(lambda),
            meta,
        })
    }
    /// Parse a DuckDB struct literal `{'key': value, …}`.
    ///
    /// At least one field is required — DuckDB rejects an empty `{}` — so an
    /// immediate `}` is a parse error here too rather than an accepted struct the
    /// engine refuses.
    pub(super) fn parse_struct_literal(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_struct_literal is reached only with a current `{` token")
            .span;
        let mut fields = ThinVec::new();
        loop {
            let field_start = self.current_span()?;
            let (key, key_spelling) = self.parse_struct_key()?;
            self.expect_punct(Punctuation::Colon, "`:` after the struct field key")?;
            let value = self.parse_expr()?;
            let field_span = field_start.union(self.preceding_span());
            fields.push(StructField {
                key,
                key_spelling,
                value,
                meta: self.make_meta(field_span),
            });
            if !self.eat_punct(Punctuation::Comma)? {
                break;
            }
            // DuckDB tolerates a single trailing comma before the struct's `}`.
            if self.trailing_comma_at(Punctuation::RBrace)? {
                break;
            }
        }
        self.expect_punct(Punctuation::RBrace, "`}` to close the struct literal")?;
        let span = start.union(self.preceding_span());
        let struct_expr = StructExpr {
            fields,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::Struct {
            r#struct: Box::new(struct_expr),
            meta,
        })
    }
    /// Parse one struct-field key: a single-quoted string (`{'a': …}`), a bare
    /// identifier (`{a: …}`), or a double-quoted identifier (`{"a": …}`).
    ///
    /// The key is a field name, never a value expression (DuckDB rejects `{1: 'x'}`),
    /// and a bare key is a `ColId` — DuckDB likewise rejects a reserved word there
    /// (`{select: 1}` is a syntax error). Only the plain `'…'` string form names a
    /// key; the extended string constants (`E'…'`, `$tag$…$tag$`, …) are not key
    /// grammar.
    fn parse_struct_key(&mut self) -> ParseResult<(Symbol, StructKeySpelling)> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a struct field key"));
        };
        match token.kind {
            TokenKind::String if self.span_text(token.span).starts_with('\'') => {
                self.advance()?;
                let raw = self.span_text(token.span);
                let inner = &raw[1..raw.len() - 1];
                let text = if inner.contains("''") {
                    Cow::Owned(inner.replace("''", "'"))
                } else {
                    Cow::Borrowed(inner)
                };
                let sym = self.intern_text(&text);
                Ok((sym, StructKeySpelling::SingleQuoted))
            }
            TokenKind::QuotedIdent => {
                self.advance()?;
                let (quote, text) = materialize_quoted_ident(self.span_text(token.span));
                if quote != QuoteStyle::Double {
                    return Err(self.unexpected("a struct field key"));
                }
                let sym = self.intern_text(&text);
                Ok((sym, StructKeySpelling::DoubleQuoted))
            }
            TokenKind::Word | TokenKind::Keyword(_) if self.token_can_be_column_name(token) => {
                self.advance()?;
                let sym = self.intern_identifier(token);
                Ok((sym, StructKeySpelling::Bare))
            }
            _ => Err(self.unexpected("a struct field key")),
        }
    }
    /// Parse a `MAP {…}` map literal, or fall back to reading `map` as an ordinary
    /// name when no `{` follows — the `MAP(<keys>, <values>)` spelling is a plain
    /// call to the `map` function, not dedicated grammar.
    pub(super) fn parse_map_or_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        if self.features().expression_syntax.collection_literals
            && self.peek_nth_is_punct(1, Punctuation::LBrace)?
        {
            self.parse_map_literal()
        } else {
            self.parse_word_prefix(token)
        }
    }
    /// Parse a DuckDB map literal `MAP {k: v, …}` (possibly empty `MAP {}`).
    fn parse_map_literal(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_map_literal is reached only with a current MAP token")
            .span;
        self.expect_punct(Punctuation::LBrace, "`{` after `MAP`")?;
        let mut entries = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RBrace)? {
            loop {
                let entry_start = self.current_span()?;
                let key = self.parse_expr()?;
                self.expect_punct(Punctuation::Colon, "`:` after the map entry key")?;
                let value = self.parse_expr()?;
                let entry_span = entry_start.union(self.preceding_span());
                entries.push(MapEntry {
                    key,
                    value,
                    meta: self.make_meta(entry_span),
                });
                if !self.eat_punct(Punctuation::Comma)? {
                    break;
                }
                // DuckDB tolerates a single trailing comma before the map's `}`.
                if self.trailing_comma_at(Punctuation::RBrace)? {
                    break;
                }
            }
        }
        self.expect_punct(Punctuation::RBrace, "`}` to close the map literal")?;
        let span = start.union(self.preceding_span());
        let map = MapExpr {
            entries,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::Map {
            map: Box::new(map),
            meta,
        })
    }
    /// Parse an explicit `ROW(...)` constructor, or fall back to reading `row` as an
    /// ordinary name when no `(` follows.
    pub(super) fn parse_row_or_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        if self.features().expression_syntax.row_constructor
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            self.parse_row()
        } else {
            self.parse_word_prefix(token)
        }
    }
    /// Parse an explicit `ROW(a, b, …)` row constructor (possibly empty `ROW()`).
    fn parse_row(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_row is reached only with a current ROW token")
            .span;
        self.expect_punct(Punctuation::LParen, "`(` after `ROW`")?;
        let fields = if self.peek_is_punct(Punctuation::RParen)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated_exprs()?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the row constructor")?;
        let span = start.union(self.preceding_span());
        let row = RowExpr {
            fields,
            explicit: true,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::Row {
            row: Box::new(row),
            meta,
        })
    }

    /// Parse a BigQuery `STRUCT(...)` value constructor, the leading (contextual)
    /// `STRUCT` word still current: the typeless `STRUCT(1, 2)` / `STRUCT(x AS a)` forms
    /// or the typed `STRUCT<a INT64, b STRING>(1, 'x')` form. The caller has confirmed
    /// `STRUCT` is immediately followed by `(` or `<` under
    /// [`ExpressionSyntax::struct_constructor`](crate::ast::dialect::ExpressionSyntax);
    /// committing on `<` needs no rewind because a preset admitting this form does not
    /// read `STRUCT` as a bare column, so `struct < x` is not a competing comparison.
    ///
    /// `#[inline(never)]`: keeps this constructor's scratch out of its caller's frame on
    /// the recursive expression path (the `high_but_safe_nesting` stack canary budget),
    /// like the per-class prefix dispatchers.
    #[inline(never)]
    pub(super) fn parse_struct_constructor(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start = token.span;
        self.advance()?; // STRUCT
        let fields = if self.eat_op(Operator::Lt)? {
            // BigQuery requires at least one field inside `<...>` (`STRUCT<>` is a syntax
            // error), which the one-or-more comma list enforces; the non-empty list is
            // also what marks the constructor as typed (see `StructConstructorExpr`).
            let fields = self.parse_comma_separated(Self::parse_struct_constructor_field)?;
            self.expect_op(Operator::Gt, "`>` to close the STRUCT type parameters")?;
            fields
        } else {
            ThinVec::new()
        };
        self.expect_punct(Punctuation::LParen, "`(` to open the STRUCT constructor")?;
        let args = if self.peek_is_punct(Punctuation::RParen)? {
            // `STRUCT()` / `STRUCT<a INT64>()` parse-accept; BigQuery's at-least-one-value
            // and field/value arity rules are analysis rejects, not grammar ones — the
            // parse-vs-bind split, and the shape sqlparser-rs accepts.
            ThinVec::new()
        } else {
            self.parse_comma_separated(Self::parse_struct_constructor_arg)?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the STRUCT constructor")?;
        let span = start.union(self.preceding_span());
        let constructor = StructConstructorExpr {
            fields,
            args,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::StructConstructor {
            constructor: Box::new(constructor),
            meta,
        })
    }

    /// Parse one `[name] TYPE` field of a typed `STRUCT<...>` parameter list.
    ///
    /// The optional name is decided by a bounded two-token lookahead, mirroring
    /// sqlparser-rs: two abutting word-like tokens read as `name TYPE`; a single
    /// word-like token (its successor is `,`, `>`, `(`, or any non-word) is an anonymous
    /// `TYPE`. The accepted trade (shared with sqlparser-rs): an anonymous *multi-word*
    /// type name would read its first word as the field name — no BigQuery type is
    /// multi-word, so the heuristic is exact for the grammar this gate admits.
    fn parse_struct_constructor_field(&mut self) -> ParseResult<StructConstructorField<D::Ext>> {
        let start = self.current_span()?;
        let named = matches!(
            self.peek()?.map(|t| t.kind),
            Some(TokenKind::Word | TokenKind::Keyword(_) | TokenKind::QuotedIdent)
        ) && matches!(
            self.peek_nth(1)?.map(|t| t.kind),
            Some(TokenKind::Word | TokenKind::Keyword(_))
        );
        let name = if named {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let ty = self.parse_data_type()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(StructConstructorField { name, ty, meta })
    }

    /// Parse one `expr [AS name]` value argument of a `STRUCT(...)` constructor. The
    /// `AS` alias is BigQuery's typeless-form field naming; the bare-alias spelling is
    /// not grammatical here (matching sqlparser-rs's `AS`-only reading), so a trailing
    /// word without `AS` is left to the enclosing list's usual reject.
    fn parse_struct_constructor_arg(&mut self) -> ParseResult<StructConstructorArg<D::Ext>> {
        let start = self.current_span()?;
        let value = self.parse_expr()?;
        let alias = if self.eat_keyword(Keyword::As)? {
            Some(self.parse_as_alias_ident()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(StructConstructorArg { value, alias, meta })
    }
}
