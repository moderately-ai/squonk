// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQL:2016/2023 SQL/JSON expression functions (`pg-sqljson-expression-functions`).
//!
//! `JSON_VALUE`/`JSON_QUERY`/`JSON_EXISTS` query functions, the `JSON_OBJECT`/`JSON_ARRAY`
//! constructors and their `JSON_OBJECTAGG`/`JSON_ARRAYAGG` aggregates, the bare
//! `JSON`/`JSON_SCALAR`/`JSON_SERIALIZE` constructors, and the `IS [NOT] JSON` predicate —
//! PostgreSQL's `func_expr_common_subexpr` / `json_aggregate_func` / `JsonIsPredicate`
//! grammar, engine-verified against `pg_query` (PG 17). Gated on
//! [`CallSyntax::sqljson_expression_functions`](crate::ast::dialect::CallSyntax); the whole
//! clause surface is PostgreSQL's raw-parse layer (per-function legality that PostgreSQL only
//! enforces during parse *analysis* — e.g. `JSON_VALUE` cannot yield an `EMPTY ARRAY` — is
//! admitted here, matching the `ParseOnly` oracle).

use crate::ast::{
    Expr, IsJsonExpr, JsonAggregateBody, JsonAggregateExpr, JsonArrayBody, JsonArrayExpr,
    JsonBehavior, JsonBehaviorKind, JsonConstructorExpr, JsonConstructorKind, JsonEncoding,
    JsonFormat, JsonFuncExpr, JsonFuncKind, JsonItemType, JsonKeyValue, JsonKeyValueSpelling,
    JsonNullClause, JsonObjectExpr, JsonPassingArg, JsonQuotesBehavior, JsonReturning,
    JsonValueExpr, JsonWrapperBehavior, Keyword, Span, Spanned,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Punctuation, Token};
use thin_vec::ThinVec;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Dispatch a SQL/JSON expression function on its keyword head. The caller confirmed the
    /// gate is on and that `(` follows, so the special form is unambiguous (a bare head with
    /// no `(` never reaches here — it falls through to the ordinary name/column path).
    pub(super) fn parse_sqljson_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let crate::tokenizer::TokenKind::Keyword(keyword) = token.kind else {
            unreachable!("parse_sqljson_expr is reached only with a keyword token");
        };
        match keyword {
            Keyword::JsonValue => self.parse_json_query_func(token, JsonFuncKind::Value),
            Keyword::JsonQuery => self.parse_json_query_func(token, JsonFuncKind::Query),
            Keyword::JsonExists => self.parse_json_query_func(token, JsonFuncKind::Exists),
            Keyword::JsonObject => self.parse_json_object(token),
            Keyword::JsonArray => self.parse_json_array(token),
            Keyword::JsonObjectagg => self.parse_json_object_agg(token),
            Keyword::JsonArrayagg => self.parse_json_array_agg(token),
            Keyword::Json => self.parse_json_constructor(token, JsonConstructorKind::Json),
            Keyword::JsonScalar => self.parse_json_constructor(token, JsonConstructorKind::Scalar),
            Keyword::JsonSerialize => {
                self.parse_json_constructor(token, JsonConstructorKind::Serialize)
            }
            _ => unreachable!("parse_sqljson_expr dispatched a non-SQL/JSON keyword"),
        }
    }

    /// Parse `JSON_VALUE`/`JSON_QUERY`/`JSON_EXISTS(context, path [clauses])`.
    fn parse_json_query_func(
        &mut self,
        token: Token,
        kind: JsonFuncKind,
    ) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // the JSON_VALUE / JSON_QUERY / JSON_EXISTS keyword
        self.expect_punct(Punctuation::LParen, "`(` after the SQL/JSON function name")?;
        let context = self.parse_json_value_expr()?;
        self.expect_punct(
            Punctuation::Comma,
            "`,` before the SQL/JSON path expression",
        )?;
        let path = self.parse_expr()?;
        let passing = self.parse_json_passing_opt()?;
        // Per-function clause admission: only the clauses each grammar allows are consumed;
        // a disallowed clause keyword is left for the `)` expectation to reject (matching
        // PostgreSQL — `JSON_EXISTS(… RETURNING …)` and `JSON_VALUE(… WITH WRAPPER)` reject).
        let returning = if matches!(kind, JsonFuncKind::Value | JsonFuncKind::Query) {
            self.parse_json_returning_opt()?
        } else {
            None
        };
        let (wrapper, quotes) = if matches!(kind, JsonFuncKind::Query) {
            (self.parse_json_wrapper()?, self.parse_json_quotes()?)
        } else {
            (
                JsonWrapperBehavior::Unspecified,
                JsonQuotesBehavior::Unspecified,
            )
        };
        let on_empty = if matches!(kind, JsonFuncKind::Value | JsonFuncKind::Query) {
            self.parse_json_on_behavior(Keyword::Empty)?
        } else {
            None
        };
        let on_error = self.parse_json_on_behavior(Keyword::Error)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the SQL/JSON function")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::JsonFunc {
            json_func: Box::new(JsonFuncExpr {
                kind,
                context,
                path: Box::new(path),
                passing,
                returning,
                wrapper,
                quotes,
                on_empty,
                on_error,
                meta,
            }),
            meta,
        })
    }

    /// Parse the standard `JSON_OBJECT([members] [null] [unique] [RETURNING])` constructor,
    /// falling back to the legacy `json_object(text[])` function call when the argument list
    /// is not a member list (`JSON_OBJECT('{a,1}')`, `JSON_OBJECT('{a}', '{1}')`).
    fn parse_json_object(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start = self.checkpoint();
        self.advance()?; // JSON_OBJECT
        self.expect_punct(Punctuation::LParen, "`(` after `JSON_OBJECT`")?;
        // The four object fields, filled by whichever branch matches; one construction below.
        let mut entries = ThinVec::new();
        let mut null_clause = None;
        let mut unique_keys = None;
        let mut returning = None;
        if self.peek_is_punct(Punctuation::RParen)? {
            // Empty object: `JSON_OBJECT()`.
            self.advance()?; // )
        } else if self.peek_is_keyword(Keyword::Returning)? {
            // Returning-only object: `JSON_OBJECT(RETURNING …)`.
            returning = self.parse_json_returning_opt()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `JSON_OBJECT`")?;
        } else {
            // Try the standard member list; on a non-member first item, rewind to the head
            // and take the legacy `json_object` ordinary-call path (args are not members).
            let Some(first) = self.try_parse_json_member()? else {
                self.rewind(start);
                return self.parse_word_prefix(token);
            };
            entries.push(first);
            while self.eat_punct(Punctuation::Comma)? {
                entries.push(self.parse_json_member_required()?);
            }
            null_clause = self.parse_json_null_clause()?;
            unique_keys = self.parse_json_unique()?;
            returning = self.parse_json_returning_opt()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `JSON_OBJECT`")?;
        }
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::JsonObject {
            json_object: Box::new(JsonObjectExpr {
                entries,
                null_clause,
                unique_keys,
                returning,
                meta,
            }),
            meta,
        })
    }

    /// Parse `JSON_ARRAY((values [null] | <query> [FORMAT]) [RETURNING])`.
    fn parse_json_array(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // JSON_ARRAY
        self.expect_punct(Punctuation::LParen, "`(` after `JSON_ARRAY`")?;
        let body_start = self.current_span()?;
        let body = if self.peek_is_punct(Punctuation::RParen)?
            || self.peek_is_keyword(Keyword::Returning)?
        {
            // Empty array / returning-only: `JSON_ARRAY()` / `JSON_ARRAY(RETURNING …)`.
            let meta = self.make_meta(body_start);
            JsonArrayBody::Values {
                items: ThinVec::new(),
                null_clause: None,
                meta,
            }
        } else if self.peek_starts_query()? {
            let query = self.parse_query()?;
            let format = self.parse_json_format()?;
            let meta = self.make_meta(body_start.union(self.preceding_span()));
            JsonArrayBody::Query {
                query: Box::new(query),
                format,
                meta,
            }
        } else {
            let items = self.parse_comma_separated(Self::parse_json_value_expr)?;
            let null_clause = self.parse_json_null_clause()?;
            let meta = self.make_meta(body_start.union(self.preceding_span()));
            JsonArrayBody::Values {
                items,
                null_clause,
                meta,
            }
        };
        let returning = self.parse_json_returning_opt()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `JSON_ARRAY`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::JsonArray {
            json_array: Box::new(JsonArrayExpr {
                body,
                returning,
                meta,
            }),
            meta,
        })
    }

    /// Parse `JSON_OBJECTAGG(member [null] [unique] [RETURNING]) [FILTER] [OVER]`.
    fn parse_json_object_agg(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // JSON_OBJECTAGG
        self.expect_punct(Punctuation::LParen, "`(` after `JSON_OBJECTAGG`")?;
        let body_start = self.current_span()?;
        let entry = self.parse_json_member_required()?;
        let null_clause = self.parse_json_null_clause()?;
        let unique_keys = self.parse_json_unique()?;
        let returning = self.parse_json_returning_opt()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `JSON_OBJECTAGG`")?;
        let meta = self.make_meta(body_start.union(self.preceding_span()));
        let body = JsonAggregateBody::Object {
            entry,
            unique_keys,
            meta,
        };
        self.finish_json_aggregate(token, body, null_clause, returning)
    }

    /// Parse `JSON_ARRAYAGG(value [ORDER BY] [null] [RETURNING]) [FILTER] [OVER]`.
    fn parse_json_array_agg(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // JSON_ARRAYAGG
        self.expect_punct(Punctuation::LParen, "`(` after `JSON_ARRAYAGG`")?;
        let body_start = self.current_span()?;
        let value = self.parse_json_value_expr()?;
        let order_by = self.parse_aggregate_order_by()?;
        let null_clause = self.parse_json_null_clause()?;
        let returning = self.parse_json_returning_opt()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `JSON_ARRAYAGG`")?;
        let meta = self.make_meta(body_start.union(self.preceding_span()));
        let body = JsonAggregateBody::Array {
            value,
            order_by,
            meta,
        };
        self.finish_json_aggregate(token, body, null_clause, returning)
    }

    fn finish_json_aggregate(
        &mut self,
        token: Token,
        body: JsonAggregateBody<D::Ext>,
        null_clause: Option<JsonNullClause>,
        returning: Option<JsonReturning<D::Ext>>,
    ) -> ParseResult<Expr<D::Ext>> {
        // The aggregate FILTER / OVER tails ride after `)`, shared with an ordinary call.
        // The `WHERE`-spelling tag is dropped: SQL/JSON aggregates carry no keyword-less
        // filter surface (under the standard dialects that own this node the `WHERE` is
        // still required), so the shape stays the canonical `FILTER (WHERE …)`.
        let (filter, _filter_where) = self.parse_aggregate_filter()?;
        let over = self.parse_over_clause()?.map(Box::new);
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::JsonAggregate {
            json_aggregate: Box::new(JsonAggregateExpr {
                body,
                null_clause,
                returning,
                filter,
                over,
                meta,
            }),
            meta,
        })
    }

    /// Parse `JSON(x [unique])` / `JSON_SCALAR(x)` / `JSON_SERIALIZE(x [RETURNING])`.
    ///
    /// The empty form composes with
    /// [`CallSyntax::sqljson_constructors_require_argument`](crate::ast::dialect::CallSyntax):
    /// where the arity floor is off (Lenient) the empty `JSON()`/`JSON_SCALAR()`/
    /// `JSON_SERIALIZE()` falls back to an ordinary niladic call; where it is on
    /// (PostgreSQL) the missing argument is a clean parse error.
    fn parse_json_constructor(
        &mut self,
        token: Token,
        kind: JsonConstructorKind,
    ) -> ParseResult<Expr<D::Ext>> {
        let start = self.checkpoint();
        self.advance()?; // JSON / JSON_SCALAR / JSON_SERIALIZE
        self.expect_punct(Punctuation::LParen, "`(` after the SQL/JSON constructor")?;
        if self.peek_is_punct(Punctuation::RParen)?
            && !self
                .features()
                .call_syntax
                .sqljson_constructors_require_argument
        {
            // Arity floor off: the empty constructor is an ordinary niladic call.
            self.rewind(start);
            return self.parse_word_prefix(token);
        }
        // `JSON_SCALAR` takes a plain `a_expr` (no FORMAT); the others take a value with an
        // optional FORMAT.
        let value = if matches!(kind, JsonConstructorKind::Scalar) {
            let expr = self.parse_expr()?;
            let span = expr.span();
            let meta = self.make_meta(span);
            JsonValueExpr {
                expr: Box::new(expr),
                format: None,
                meta,
            }
        } else {
            self.parse_json_value_expr()?
        };
        let unique_keys = if matches!(kind, JsonConstructorKind::Json) {
            self.parse_json_unique()?
        } else {
            None
        };
        let returning = if matches!(kind, JsonConstructorKind::Serialize) {
            self.parse_json_returning_opt()?
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the SQL/JSON constructor")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::JsonConstructor {
            json_constructor: Box::new(JsonConstructorExpr {
                kind,
                value,
                unique_keys,
                returning,
                meta,
            }),
            meta,
        })
    }

    /// Parse the `IS [NOT] JSON [type] [WITH|WITHOUT UNIQUE [KEYS]]` predicate after the
    /// left operand and the already-consumed `IS [NOT]`.
    pub(super) fn parse_is_json_predicate(
        &mut self,
        expr: Expr<D::Ext>,
        negated: bool,
    ) -> ParseResult<Expr<D::Ext>> {
        self.expect_keyword(Keyword::Json)?;
        let item_type = if self.eat_keyword(Keyword::Value)? {
            JsonItemType::Value
        } else if self.eat_keyword(Keyword::Array)? {
            JsonItemType::Array
        } else if self.eat_keyword(Keyword::Object)? {
            JsonItemType::Object
        } else if self.eat_keyword(Keyword::Scalar)? {
            JsonItemType::Scalar
        } else {
            JsonItemType::Any
        };
        // `WITH UNIQUE [KEYS]` (check uniqueness) / `WITHOUT UNIQUE [KEYS]` (the default);
        // only consumed when `UNIQUE` follows, so a trailing `WITH`/`WITHOUT` that belongs to
        // an outer clause (e.g. an output alias) stays unconsumed. `WITHOUT UNIQUE` is the
        // inert default: it is consumed for fidelity but folds onto `unique_keys = false`
        // (the same as no clause / an unwritten clause), matching PostgreSQL's normalization.
        let unique_keys = self.parse_json_unique()?.unwrap_or_default();
        let span = expr.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::IsJson {
            is_json: Box::new(IsJsonExpr {
                expr: Box::new(expr),
                negated,
                item_type,
                unique_keys,
                meta,
            }),
            meta,
        })
    }

    // --- shared clause parsers -------------------------------------------------------

    /// Parse a SQL/JSON value expression: `<a_expr> [FORMAT JSON [ENCODING …]]`.
    ///
    /// `pub(crate)` so the `JSON_TABLE` table factor
    /// ([`super::super::from`]) reuses the same context-item / `PASSING`-value grammar.
    pub(crate) fn parse_json_value_expr(&mut self) -> ParseResult<JsonValueExpr<D::Ext>> {
        let expr = self.parse_expr()?;
        let format = self.parse_json_format()?;
        let span = expr.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(JsonValueExpr {
            expr: Box::new(expr),
            format,
            meta,
        })
    }

    /// Parse an optional `FORMAT JSON [ENCODING <enc>]`. Only `FORMAT JSON` is accepted
    /// (`FORMAT JSONB` is a PostgreSQL raw-parse error), and the encoding name is validated
    /// against the closed `UTF8`/`UTF16`/`UTF32` set exactly as PostgreSQL does at parse.
    pub(crate) fn parse_json_format(&mut self) -> ParseResult<Option<JsonFormat>> {
        if !self.eat_keyword(Keyword::Format)? {
            return Ok(None);
        }
        self.expect_keyword(Keyword::Json)?;
        let encoding = if self.eat_keyword(Keyword::Encoding)? {
            Some(self.parse_json_encoding()?)
        } else {
            None
        };
        Ok(Some(JsonFormat { encoding }))
    }

    fn parse_json_encoding(&mut self) -> ParseResult<JsonEncoding> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a JSON encoding (UTF8, UTF16, or UTF32)"));
        };
        let text = self.span_text(token.span).to_owned();
        let encoding = if text.eq_ignore_ascii_case("UTF8") {
            JsonEncoding::Utf8
        } else if text.eq_ignore_ascii_case("UTF16") {
            JsonEncoding::Utf16
        } else if text.eq_ignore_ascii_case("UTF32") {
            JsonEncoding::Utf32
        } else {
            return Err(self.error_at(token.span, "a JSON encoding (UTF8, UTF16, or UTF32)", text));
        };
        self.advance()?;
        Ok(encoding)
    }

    /// Parse an optional `RETURNING <type> [FORMAT JSON [ENCODING …]]` output clause.
    fn parse_json_returning_opt(&mut self) -> ParseResult<Option<JsonReturning<D::Ext>>> {
        let start = self.current_span()?;
        if !self.eat_keyword(Keyword::Returning)? {
            return Ok(None);
        }
        let data_type = self.parse_data_type()?;
        let format = self.parse_json_format()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(JsonReturning {
            data_type: Box::new(data_type),
            format,
            meta,
        }))
    }

    /// Parse an optional `PASSING <value> [FORMAT] AS <name>, …` binding list.
    pub(crate) fn parse_json_passing_opt(
        &mut self,
    ) -> ParseResult<ThinVec<JsonPassingArg<D::Ext>>> {
        if !self.eat_keyword(Keyword::Passing)? {
            return Ok(ThinVec::new());
        }
        self.parse_comma_separated(Self::parse_json_passing_arg)
    }

    fn parse_json_passing_arg(&mut self) -> ParseResult<JsonPassingArg<D::Ext>> {
        let start = self.current_span()?;
        let value = self.parse_json_value_expr()?;
        self.expect_keyword(Keyword::As)?;
        // The bound name is a `ColLabel` (PostgreSQL admits any keyword here).
        let name = self.parse_as_alias_ident()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(JsonPassingArg { value, name, meta })
    }

    /// Parse an optional `<behavior> ON {EMPTY | ERROR}` handler. `slot` is `EMPTY` or
    /// `ERROR`; only fires when a behaviour keyword is followed by `ON <slot>`.
    pub(crate) fn parse_json_on_behavior(
        &mut self,
        slot: Keyword,
    ) -> ParseResult<Option<JsonBehavior<D::Ext>>> {
        let start = self.checkpoint();
        let Some(behavior) = self.try_parse_json_behavior()? else {
            return Ok(None);
        };
        if self.eat_keyword(Keyword::On)? && self.eat_keyword(slot)? {
            Ok(Some(behavior))
        } else {
            // The behaviour was not followed by the expected `ON <slot>` — it belongs to
            // the other slot (or is a stray keyword the `)` will reject); rewind.
            self.rewind(start);
            Ok(None)
        }
    }

    /// Parse a SQL/JSON behaviour keyword: `ERROR`/`NULL`/`TRUE`/`FALSE`/`UNKNOWN`/`EMPTY`/
    /// `EMPTY ARRAY`/`EMPTY OBJECT`/`DEFAULT <expr>`. `None` when the cursor is not on one.
    fn try_parse_json_behavior(&mut self) -> ParseResult<Option<JsonBehavior<D::Ext>>> {
        let start = self.current_span()?;
        let (kind, default_expr) = if self.eat_keyword(Keyword::Error)? {
            (JsonBehaviorKind::Error, None)
        } else if self.eat_keyword(Keyword::Null)? {
            (JsonBehaviorKind::Null, None)
        } else if self.eat_keyword(Keyword::True)? {
            (JsonBehaviorKind::True, None)
        } else if self.eat_keyword(Keyword::False)? {
            (JsonBehaviorKind::False, None)
        } else if self.eat_keyword(Keyword::Unknown)? {
            (JsonBehaviorKind::Unknown, None)
        } else if self.eat_keyword(Keyword::Empty)? {
            // `EMPTY ARRAY` / `EMPTY OBJECT` / bare `EMPTY` (PostgreSQL's shorthand).
            if self.eat_keyword(Keyword::Array)? {
                (JsonBehaviorKind::EmptyArray, None)
            } else if self.eat_keyword(Keyword::Object)? {
                (JsonBehaviorKind::EmptyObject, None)
            } else {
                (JsonBehaviorKind::Empty, None)
            }
        } else if self.eat_keyword(Keyword::Default)? {
            let expr = self.parse_expr()?;
            (JsonBehaviorKind::Default, Some(Box::new(expr)))
        } else {
            return Ok(None);
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(JsonBehavior {
            kind,
            default_expr,
            meta,
        }))
    }

    /// Parse `JSON_QUERY`'s optional wrapper: `WITH [CONDITIONAL|UNCONDITIONAL] [ARRAY]
    /// WRAPPER` / `WITHOUT [ARRAY] WRAPPER`. The `WRAPPER` keyword is mandatory (a bare
    /// `WITH`/`WITHOUT` or `WITH ARRAY` without it is rejected).
    pub(crate) fn parse_json_wrapper(&mut self) -> ParseResult<JsonWrapperBehavior> {
        if self.eat_keyword(Keyword::Without)? {
            let _ = self.eat_keyword(Keyword::Array)?;
            self.expect_keyword(Keyword::Wrapper)?;
            Ok(JsonWrapperBehavior::Without)
        } else if self.eat_keyword(Keyword::With)? {
            let conditional = if self.eat_keyword(Keyword::Conditional)? {
                true
            } else {
                let _ = self.eat_keyword(Keyword::Unconditional)?;
                false
            };
            let _ = self.eat_keyword(Keyword::Array)?;
            self.expect_keyword(Keyword::Wrapper)?;
            Ok(if conditional {
                JsonWrapperBehavior::Conditional
            } else {
                JsonWrapperBehavior::Unconditional
            })
        } else {
            Ok(JsonWrapperBehavior::Unspecified)
        }
    }

    /// Parse `JSON_QUERY`'s optional quotes clause: `{KEEP | OMIT} QUOTES [ON SCALAR STRING]`.
    pub(crate) fn parse_json_quotes(&mut self) -> ParseResult<JsonQuotesBehavior> {
        let keep = if self.eat_keyword(Keyword::Keep)? {
            true
        } else if self.eat_keyword(Keyword::Omit)? {
            false
        } else {
            return Ok(JsonQuotesBehavior::Unspecified);
        };
        self.expect_keyword(Keyword::Quotes)?;
        // The inert `ON SCALAR STRING` tail is consumed and discarded.
        if self.eat_keyword(Keyword::On)? {
            self.expect_keyword(Keyword::Scalar)?;
            self.expect_keyword(Keyword::String)?;
        }
        Ok(if keep {
            JsonQuotesBehavior::Keep
        } else {
            JsonQuotesBehavior::Omit
        })
    }

    /// Parse an optional constructor null-handling clause: `ABSENT ON NULL` / `NULL ON NULL`.
    fn parse_json_null_clause(&mut self) -> ParseResult<Option<JsonNullClause>> {
        if self.peek_is_keyword(Keyword::Absent)? {
            self.advance()?; // ABSENT
            self.expect_keyword(Keyword::On)?;
            self.expect_keyword(Keyword::Null)?;
            Ok(Some(JsonNullClause::AbsentOnNull))
        } else if self.peek_is_keyword(Keyword::Null)?
            && self.peek_nth_is_keyword(1, Keyword::On)?
        {
            self.advance()?; // NULL
            self.advance()?; // ON
            self.expect_keyword(Keyword::Null)?;
            Ok(Some(JsonNullClause::NullOnNull))
        } else {
            Ok(None)
        }
    }

    /// Parse an optional key-uniqueness clause: `WITH UNIQUE [KEYS]` (`Some(true)`) /
    /// `WITHOUT UNIQUE [KEYS]` (`Some(false)`). Only consumed when `UNIQUE` follows, so a
    /// `WITH`/`WITHOUT` belonging to an outer clause stays unconsumed.
    fn parse_json_unique(&mut self) -> ParseResult<Option<bool>> {
        let with = if self.peek_is_keyword(Keyword::With)?
            && self.peek_nth_is_keyword(1, Keyword::Unique)?
        {
            true
        } else if self.peek_is_keyword(Keyword::Without)?
            && self.peek_nth_is_keyword(1, Keyword::Unique)?
        {
            false
        } else {
            return Ok(None);
        };
        self.advance()?; // WITH / WITHOUT
        self.advance()?; // UNIQUE
        let _ = self.eat_keyword(Keyword::Keys)?;
        Ok(Some(with))
    }

    /// Parse one object member, requiring it to form (an error when it does not) — used for
    /// members after the first and for `JSON_OBJECTAGG`'s single member.
    fn parse_json_member_required(&mut self) -> ParseResult<JsonKeyValue<D::Ext>> {
        match self.try_parse_json_member()? {
            Some(member) => Ok(member),
            None => {
                Err(self
                    .unexpected("a SQL/JSON object member (`key : value` or `key VALUE value`)"))
            }
        }
    }

    /// Try to parse one object member: `[KEY] <key> {: | VALUE} <value>`. `None` when the
    /// cursor is not a member (its first item is an ordinary expression), signalling the
    /// legacy `json_object` fallback to the caller.
    ///
    /// The `VALUE` form's key is a `c_expr` (a primary — no bare binary operators) while the
    /// `:` form's key is a full `a_expr` (PostgreSQL: `1 + 2 VALUE 3` rejects, `1 + 2 : 3`
    /// accepts). So the key is first read at the primary level; if `VALUE`/`:` follows it is
    /// that member, otherwise the key is re-read as a full `a_expr` and only a following `:`
    /// makes it a member.
    fn try_parse_json_member(&mut self) -> ParseResult<Option<JsonKeyValue<D::Ext>>> {
        let start = self.current_span()?;
        // An explicit `KEY` prefix marks the standard form; consume it only when a key
        // expression (not `,`/`)`) follows, so a bare column literally named `key` still
        // takes the legacy path.
        let has_key_prefix = self.peek_is_keyword(Keyword::Key)?
            && !self.peek_nth_is_punct(1, Punctuation::Comma)?
            && !self.peek_nth_is_punct(1, Punctuation::RParen)?;
        if has_key_prefix {
            self.advance()?; // KEY
        }
        let checkpoint = self.checkpoint();
        // The `c_expr` (primary) reading — enough for `VALUE` and for a simple `:` key.
        let key_primary = self.parse_prefix()?.expr;
        if self.peek_is_keyword(Keyword::Value)? {
            self.advance()?; // VALUE
            return self.finish_json_member(start, key_primary, JsonKeyValueSpelling::Value);
        }
        if self.peek_is_punct(Punctuation::Colon)? {
            self.advance()?; // :
            return self.finish_json_member(start, key_primary, JsonKeyValueSpelling::Colon);
        }
        // A primary not terminated by `VALUE`/`:` is either a full-`a_expr` `:` key or not a
        // member at all; re-read the key as an `a_expr` and require the `:`.
        self.rewind(checkpoint);
        let key_expr = self.parse_expr()?;
        if self.peek_is_punct(Punctuation::Colon)? {
            self.advance()?; // :
            return self.finish_json_member(start, key_expr, JsonKeyValueSpelling::Colon);
        }
        // Not a member. When a `KEY` prefix was consumed this is a hard error (the legacy
        // fallback never starts with `KEY`), but the caller resets the cursor to the head
        // before taking that path, so returning `None` is correct either way.
        Ok(None)
    }

    fn finish_json_member(
        &mut self,
        start: Span,
        key: Expr<D::Ext>,
        spelling: JsonKeyValueSpelling,
    ) -> ParseResult<Option<JsonKeyValue<D::Ext>>> {
        let value = self.parse_json_value_expr()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(JsonKeyValue {
            key: Box::new(key),
            value,
            spelling,
            meta,
        }))
    }
}
