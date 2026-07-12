// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Keyword-opened special-form primaries: the keyword token class of
//! [`parse_prefix`](crate::parser::Parser::parse_prefix)'s dispatch — the literal
//! keywords, `CAST`/`CONVERT`, `CASE`, `EXTRACT`, `EXISTS`, `NULLIF`, the special
//! value functions, the constructor keywords (`ARRAY`/`ROW`/`MAP`), DuckDB's
//! `COLUMNS(...)`, and the temporal typed-literal heads. The SQL/JSON, SQL/XML, and
//! string keyword-argument families live in their own siblings (`sqljson.rs`,
//! `xml.rs`, `string_funcs.rs`); this dispatcher routes to them.

use super::special_function_keyword;
use crate::ast::{
    CaseExpr, CastSyntax, ColumnsSpelling, DataType, Expr, ExtractExpr, Ident, Keyword, KeywordSet,
    Literal, LiteralKind, QuoteStyle, Span, Spanned, StringFunc, UnaryOperator, WhenClause,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use thin_vec::ThinVec;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Per-class prefix dispatcher for a keyword-opened primary: the literal and
    /// special-form keyword arms of the expression grammar, falling back to the
    /// typed-literal / column / function-name reading for any keyword no arm claims
    /// (or whose gate/lookahead declines).
    ///
    /// `#[inline(never)]`: called-and-returned from the [`parse_prefix`](Self::parse_prefix)
    /// router so this class's ~30 arms of guard/arm scratch stay off the hot recursive
    /// frame (the `high_but_safe_nesting` stack canary budget).
    #[inline(never)]
    pub(super) fn parse_keyword_prefix(
        &mut self,
        token: Token,
        keyword: Keyword,
    ) -> ParseResult<Expr<D::Ext>> {
        match keyword {
            Keyword::Null => self.parse_literal_keyword(LiteralKind::Null),
            Keyword::True => self.parse_literal_keyword(LiteralKind::Boolean(true)),
            Keyword::False => self.parse_literal_keyword(LiteralKind::Boolean(false)),
            // Oracle/Snowflake `PRIOR <expr>` inside a `CONNECT BY` condition: a unary
            // prefix operator marking the parent-row operand. Recognized *only* while
            // parsing a hierarchical condition ([`Parser::in_connect_by`]); everywhere
            // else `PRIOR` falls through to its ordinary column / typed-literal reading, so
            // the global expression grammar is unchanged and a bare `prior` stays a name.
            Keyword::Prior if self.in_connect_by => self.parse_unary(UnaryOperator::Prior),
            Keyword::Cast => self.parse_cast(),
            // MySQL's `CONVERT` special form — the comma-form cast `CONVERT(x, type)` and
            // the transcoding `CONVERT(x USING cs)`. A special form only when `CONVERT` is
            // immediately followed by `(` under the gate; a bare head (no `(`, or the gate
            // off) falls through to the ordinary name/call path, matching PostgreSQL, whose
            // `CONVERT` is a plain function (`pg_query`-verified).
            Keyword::Convert
                if self.features().call_syntax.convert_function
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_convert()
            }
            // MySQL's full-text `MATCH (<col>, …) AGAINST (<expr> [<modifier>])` special form
            // — a `simple_expr` production, so `MATCH` immediately followed by `(` opens it
            // under the gate. MySQL reserves `MATCH`, so a bare `MATCH` with no `(` stays a
            // reserved-word reject; SQLite's infix `<expr> MATCH <expr>` operator is a separate
            // binding-power entry that never reaches this prefix arm.
            Keyword::Match
                if self.features().string_func_forms.match_against
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_match_against(token)
            }
            Keyword::Case => self.parse_case(),
            Keyword::Extract if self.features().call_syntax.extract_from_syntax => {
                self.parse_extract_or_column()
            }
            // The SQL:2016 SQL/JSON expression functions (JSON_VALUE/JSON_QUERY/JSON_EXISTS,
            // the JSON_OBJECT/JSON_ARRAY constructors + aggregates, and the bare
            // JSON/JSON_SCALAR/JSON_SERIALIZE constructors). Each is a special form only when
            // its `(` follows under the gate; a bare head (no `(`, or the gate off) falls
            // through to the ordinary name/typed-literal path (`json '{}'` stays a typed
            // literal, `JSON`/`json_scalar` a column/function name).
            Keyword::JsonValue
            | Keyword::JsonQuery
            | Keyword::JsonExists
            | Keyword::JsonObject
            | Keyword::JsonArray
            | Keyword::JsonObjectagg
            | Keyword::JsonArrayagg
            | Keyword::Json
            | Keyword::JsonScalar
            | Keyword::JsonSerialize
                if self.features().call_syntax.sqljson_expression_functions
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_sqljson_expr(token)
            }
            // The SQL/XML expression functions (xmlelement/xmlforest/xmlconcat/xmlparse/
            // xmlpi/xmlroot/xmlserialize/xmlexists). Each is a special form only when its
            // `(` follows under the gate; a bare head (no `(`, or the gate off) falls
            // through to the ordinary name/function path (`xmlparse` a column name).
            Keyword::Xmlelement
            | Keyword::Xmlforest
            | Keyword::Xmlconcat
            | Keyword::Xmlparse
            | Keyword::Xmlpi
            | Keyword::Xmlroot
            | Keyword::Xmlserialize
            | Keyword::Xmlexists
                if self.features().call_syntax.xml_expression_functions
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_xml_expr(token)
            }
            // The standard-SQL string special forms (SUBSTRING/POSITION/OVERLAY/TRIM
            // keyword-argument grammar). Each is a special form only when its `(`
            // follows under its gate; a bare head (no `(`, or the gate off) falls
            // through to the ordinary name/call path, and the comma plain-call
            // spellings fall back to it from inside each dispatcher.
            Keyword::Substring
                if self.features().string_func_forms.substring_from_for
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_substring_expr(token)
            }
            Keyword::Position
                if self.features().string_func_forms.position_in
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_position_expr(token)
            }
            Keyword::Overlay
                if self.features().string_func_forms.overlay_placing
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_overlay_expr(token)
            }
            Keyword::Trim
                if self.features().string_func_forms.trim_from
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_trim_expr(token)
            }
            // PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr — a dedicated
            // `COLLATION FOR '(' a_expr ')'` production, so only `COLLATION` immediately
            // followed by `FOR (` opens it (a bare `COLLATION`, or `COLLATION` not trailed
            // by `FOR (`, falls through to its ordinary type_func_name reading — e.g.
            // `collation(x)` stays a plain call). Gated PG/Lenient.
            Keyword::Collation
                if self.features().string_func_forms.collation_for_expression
                    && self.peek_nth_is_keyword(1, Keyword::For)?
                    && self.peek_nth_is_punct(2, Punctuation::LParen)? =>
            {
                self.parse_collation_for(token)
            }
            // The `CEIL`/`CEILING` rounding-field keyword form (`CEIL(x TO DAY)`) — no
            // probed oracle grammar admits it, so this is sqlparser-rs-parity surface
            // only, gated Lenient-only. The first operand parses as an ordinary
            // expression and only a following `TO` commits to the special form (mirroring
            // `substring_from_for`'s shape); a first operand with no `TO` tail (including
            // the comma scale spelling `CEIL(x, 2)`) rewinds to the ordinary call path.
            Keyword::Ceil | Keyword::Ceiling
                if self.features().string_func_forms.ceil_to_field
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_ceil_expr(token)
            }
            // The `FLOOR` rounding-field keyword form (`FLOOR(x TO DAY)`) — no probed
            // oracle grammar admits it, so this is sqlparser-rs-parity surface only,
            // gated Lenient-only. The first operand parses as an ordinary expression and
            // only a following `TO` commits to the special form (mirroring
            // `parse_ceil_expr`'s shape); a first operand with no `TO` tail (including the
            // comma scale spelling `FLOOR(x, 2)`) rewinds to the ordinary call path.
            Keyword::Floor
                if self.features().string_func_forms.floor_to_field
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_floor_expr(token)
            }
            Keyword::Exists => self.parse_exists_or_column(token),
            Keyword::Nullif => self.parse_nullif_or_column(token),
            // SQL special value functions (PostgreSQL `SQLValueFunction`): a nullary
            // keyword or, for the four temporal forms, an optional precision.
            Keyword::CurrentCatalog
            | Keyword::CurrentDate
            | Keyword::CurrentRole
            | Keyword::CurrentSchema
            | Keyword::CurrentTime
            | Keyword::CurrentTimestamp
            | Keyword::CurrentUser
            | Keyword::Localtime
            | Keyword::Localtimestamp
            | Keyword::SessionUser
            | Keyword::SystemUser
            | Keyword::User => self.parse_special_function(token),
            // MySQL `UTC_DATE` / `UTC_TIME` / `UTC_TIMESTAMP` — the UTC-clock analogues of
            // the `CURRENT_*` temporal forms, sharing the same nullary special-function
            // production. Gated MySQL-only: the keywords are non-reserved elsewhere, so
            // under other dialects they stay ordinary column/function names. Expression
            // position only — MySQL has no `func_table` promotion, so `is_special_function_keyword`
            // (the FROM-position gate) deliberately omits them.
            Keyword::UtcDate | Keyword::UtcTime | Keyword::UtcTimestamp
                if self.features().call_syntax.utc_special_functions =>
            {
                self.parse_special_function(token)
            }
            // PostgreSQL's `merge_action()` MERGE-RETURNING support function — a dedicated
            // zero-argument grammar production, so only `merge_action` immediately followed
            // by `(` opens it (a bare `merge_action` falls through to its reserved-word
            // reject unchanged). Gated PG/Lenient.
            Keyword::MergeAction
                if self.features().call_syntax.merge_action_function
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_merge_action_function(token)
            }
            // `ARRAY[...]` / `ARRAY(<query>)`, `ROW(...)`, and `MAP {...}` are
            // constructors only when their bracket follows; otherwise the word falls
            // back to a name.
            Keyword::Array => self.parse_array_or_column(token),
            Keyword::Row => self.parse_row_or_column(token),
            Keyword::Map => self.parse_map_or_column(token),
            // DuckDB's `COLUMNS(<selector>)` star expression: the (non-reserved)
            // `COLUMNS` keyword immediately followed by `(`, under the gate. A bare
            // `columns` (no `(`) falls through to the reserved-word/name path, and
            // with the gate off `COLUMNS(x)` is read as an ordinary call there too —
            // the disambiguation is this lookahead, never a tokenizer change.
            Keyword::Columns
                if self.features().call_syntax.columns_expression
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_columns_selector(token.span, ColumnsSpelling::Columns)
            }
            // MySQL `VALUES(<col>)`: the legacy reference to a column's proposed
            // insert value, grammatical only in (and gated with) `ON DUPLICATE KEY
            // UPDATE`. MySQL parses it as an ordinary `simple_expr` call, so it reuses
            // the canonical `Expr::Function` shape. Only a following `(` is the
            // reference; a bare `VALUES` keeps its table-value-constructor meaning and
            // falls through to the reserved-word path.
            Keyword::Values
                if self.features().mutation_syntax.on_duplicate_key_update
                    && self.peek_nth_is_punct(1, Punctuation::LParen)? =>
            {
                self.parse_values_reference(token)
            }
            // The temporal type keywords open a typed literal (`DATE '...'`,
            // `INTERVAL '90' DAY`, …) only when a string constant follows the type
            // prefix; otherwise they fall back to an ordinary column/function name.
            Keyword::Date | Keyword::Time | Keyword::Timestamp | Keyword::Interval => {
                // MySQL's `INTERVAL <expr> <unit>` operator interval (`d - INTERVAL 3 DAY`) is
                // read before the typed-string literal path — MySQL has no first-class interval
                // literal. A form that is not a valid operator interval (unit-less, ANSI `TO` /
                // precision, the `INTERVAL(a, b)` index function) declines and falls through.
                if keyword == Keyword::Interval
                    && self.features().expression_syntax.mysql_interval_operator
                {
                    if let Some(expr) = self.try_parse_mysql_interval_operator()? {
                        return Ok(expr);
                    }
                }
                match self.try_parse_temporal_literal(keyword)? {
                    Some(kind) => {
                        let span = token.span.union(self.preceding_span());
                        let literal = Literal {
                            kind,
                            meta: self.make_meta(span),
                        };
                        let meta = self.make_meta(span);
                        Ok(Expr::Literal { literal, meta })
                    }
                    None => self.parse_word_prefix(token),
                }
            }
            // Any other keyword (or a special-form keyword whose gate/lookahead declined
            // above): a non-reserved type-name prefix followed by a string constant opens
            // a generalized typed literal (PostgreSQL's `ConstTypename Sconst`, ADR-0011,
            // detected speculatively per ADR-0005); on a non-match the cursor rewinds and
            // the keyword falls back to its column/function reading.
            _ => match self.try_parse_typed_literal()? {
                Some(expr) => Ok(expr),
                None => self.parse_word_prefix(token),
            },
        }
    }
    /// Parse `CAST(<expr> AS <type>)`.
    fn parse_cast(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_cast is reached only with a current CAST token")
            .span;
        self.parse_cast_call(start, false)
    }

    /// Parse DuckDB's `TRY_CAST(<expr> AS <type>)` null-on-failure cast. The caller has
    /// confirmed the current (contextual) `TRY_CAST` word is immediately followed by `(`
    /// under [`CallSyntax::try_cast`](crate::ast::dialect::CallSyntax); a bare `TRY_CAST`
    /// with no `(` (or with the gate off) falls through to the ordinary name path.
    pub(super) fn parse_try_cast(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_try_cast is reached only on the TRY_CAST word")
            .span;
        self.parse_cast_call(start, true)
    }

    /// Parse the `(<expr> AS <type>)` tail shared by `CAST` and DuckDB's `TRY_CAST`, the
    /// leading keyword already consumed. `try_cast` records DuckDB's null-on-failure
    /// semantics — a distinct meaning (not a spelling) folded onto the one
    /// [`Expr::Cast`] shape.
    fn parse_cast_call(&mut self, start: Span, try_cast: bool) -> ParseResult<Expr<D::Ext>> {
        self.expect_punct(Punctuation::LParen, "`(` after the cast keyword")?;
        let expr = self.parse_expr()?;
        self.expect_keyword(Keyword::As)?;
        let type_start = self.current_span()?;
        let data_type = self.parse_data_type()?;
        self.check_restricted_cast_target(type_start, &data_type)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the cast")?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::Cast {
            expr: Box::new(expr),
            data_type: Box::new(data_type),
            syntax: CastSyntax::Call,
            try_cast,
            meta,
        })
    }

    /// Reject a parsed cast/`CONVERT` target that is not one of MySQL's narrow `cast_type`
    /// set when [`CallSyntax::restricted_cast_targets`](crate::ast::dialect::CallSyntax) is
    /// on. MySQL's `CAST`/`CONVERT` target is that closed set, not the full column-type
    /// vocabulary: `CAST(x AS INT)` / `AS VARCHAR` / `AS TIMESTAMP` (and the identical
    /// `CONVERT(x, …)` forms) are the syntax error MySQL reports, while the same names stay
    /// valid as column types (engine-measured on mysql:8). `type_start` anchors the error
    /// span at the target's first token.
    fn check_restricted_cast_target(
        &mut self,
        type_start: Span,
        data_type: &DataType<D::Ext>,
    ) -> ParseResult<()> {
        if self.features().call_syntax.restricted_cast_targets
            && !self.is_mysql_cast_target(data_type)
        {
            let span = type_start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(
                span,
                "a MySQL cast target (SIGNED/UNSIGNED, CHAR, BINARY, DATE, DATETIME, TIME, \
                 DECIMAL, DOUBLE, FLOAT, REAL, JSON, YEAR, or a spatial type)",
                found,
            ));
        }
        Ok(())
    }

    /// Parse MySQL's `CONVERT` special-form function, the leading `CONVERT` word still
    /// current. One production, two shapes: the comma-form cast `CONVERT(<expr>, <type>)`
    /// folds onto [`Expr::Cast`] as [`CastSyntax::Convert`] and shares `CAST`'s restricted
    /// `cast_type` target gate ([`check_restricted_cast_target`](Self::check_restricted_cast_target));
    /// the transcoding `CONVERT(<expr> USING <charset>)` is a
    /// [`StringFunc::ConvertUsing`]. The caller confirmed `CONVERT` is immediately followed
    /// by `(` under [`CallSyntax::convert_function`](crate::ast::dialect::CallSyntax).
    fn parse_convert(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_convert is reached only on the CONVERT word")
            .span;
        self.expect_punct(Punctuation::LParen, "`(` after CONVERT")?;
        let expr = self.parse_expr()?;
        if self.eat_keyword(Keyword::Using)? {
            let charset = self.parse_convert_charset_name()?;
            self.expect_punct(Punctuation::RParen, "`)` to close CONVERT")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Expr::StringFunc {
                string_func: Box::new(StringFunc::ConvertUsing {
                    expr: Box::new(expr),
                    charset,
                    meta,
                }),
                meta,
            });
        }
        self.expect_punct(Punctuation::Comma, "`,` or `USING` in CONVERT")?;
        let type_start = self.current_span()?;
        let data_type = self.parse_data_type()?;
        self.check_restricted_cast_target(type_start, &data_type)?;
        self.expect_punct(Punctuation::RParen, "`)` to close CONVERT")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Expr::Cast {
            expr: Box::new(expr),
            data_type: Box::new(data_type),
            syntax: CastSyntax::Convert,
            try_cast: false,
            meta,
        })
    }

    /// Parse the `charset_name` operand of `CONVERT(<expr> USING <charset>)`: a MySQL
    /// `ident_or_text` (bare/backtick identifier or quoted string, via
    /// [`parse_charset_name`](Self::parse_charset_name)) or the reserved `BINARY`
    /// transcoding name (`CONVERT(x USING binary)`, engine-verified), which the
    /// identifier path rejects as a keyword and so is captured here explicitly.
    fn parse_convert_charset_name(&mut self) -> ParseResult<Ident> {
        if self.peek_is_contextual_keyword("BINARY")? {
            let token = self.peek()?.expect("peek matched a contextual keyword");
            self.advance()?;
            return Ok(Ident {
                sym: self.intern_identifier(token),
                quote: QuoteStyle::None,
                meta: self.make_meta(token.span),
            });
        }
        self.parse_charset_name()
    }
    /// Parse DuckDB's `COLUMNS(<selector>)` star expression. The caller has confirmed
    /// the `COLUMNS` keyword is immediately followed by `(` under
    /// [`CallSyntax::columns_expression`](crate::ast::dialect::CallSyntax::columns_expression). The one argument is either a star —
    /// bare `*` or the qualified `t.*` (one name part only; the engine rejects
    /// `s.t.*`) — which may carry the `EXCLUDE`/`REPLACE`/`RENAME` modifiers
    /// (`COLUMNS(* EXCLUDE (i))`, `COLUMNS(t.* EXCLUDE (k))`; probed on 1.5.4) — or a
    /// single expression: the regex string `COLUMNS('re')`, a lambda
    /// `COLUMNS(c -> …)`, a name list `COLUMNS([…])`, or a bare column. DuckDB takes
    /// exactly one argument, so no comma list is read; `COLUMNS(a, b)` leaves the
    /// comma unconsumed and surfaces as a parse error, as it does in the engine.
    ///
    /// The cursor is on the `COLUMNS` keyword; `start` anchors the node span at the
    /// construct's first token (the `COLUMNS` for the wrapped form, the `*` for the
    /// [`ColumnsSpelling::Unpack`] prefix), and `spelling` records which surface form the
    /// caller committed to.
    pub(super) fn parse_columns_selector(
        &mut self,
        start: Span,
        spelling: ColumnsSpelling,
    ) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // `COLUMNS`
        self.expect_punct(Punctuation::LParen, "`(` after COLUMNS")?;
        let (qualifier, pattern, options) = if self.peek_is_op(Operator::Star)? {
            let star = self
                .advance()?
                .expect("peek_is_op confirmed the `*` is present");
            let options = self.parse_wildcard_modifier_tail(star.span)?;
            (None, None, options)
        } else {
            let expr = self.parse_expr()?;
            // `<name>.*`: a column name immediately followed by `.*` is the qualified
            // star form (the projection-item detection pattern) — one part only,
            // matching the engine's single `relation_name` slot.
            let qualified_star =
                self.peek_is_punct(Punctuation::Dot)? && self.peek_nth_is_op(1, Operator::Star)?;
            match expr {
                Expr::Column { name, .. } if qualified_star && name.0.len() == 1 => {
                    self.advance()?; // `.`
                    let star = self.advance()?.expect("peek confirmed the `*`");
                    let options = self.parse_wildcard_modifier_tail(star.span)?;
                    (Some(name), None, options)
                }
                expr => (None, Some(Box::new(expr)), None),
            }
        };
        self.expect_punct(Punctuation::RParen, "`)` to close COLUMNS(")?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::Columns {
            qualifier,
            pattern,
            options,
            spelling,
            meta,
        })
    }

    /// Whether the current `*` token opens a DuckDB `*COLUMNS(...)` unpack prefix — a
    /// `*` immediately followed by `COLUMNS(` under [`CallSyntax::columns_expression`](crate::ast::dialect::CallSyntax::columns_expression).
    /// The caller has a `*` current; this distinguishes the unpack prefix from the
    /// `count(*)` wildcard argument (which the function-call parser would otherwise
    /// swallow) and from infix multiplication.
    pub(in crate::parser) fn peek_is_columns_unpack_prefix(&mut self) -> ParseResult<bool> {
        Ok(self.features().call_syntax.columns_expression
            && self.peek_nth_is_keyword(1, Keyword::Columns)?
            && self.peek_nth_is_punct(2, Punctuation::LParen)?)
    }

    /// Parse an expression that may be a DuckDB bare-star columns selector — a plain
    /// `*` or the qualified `t.*` (one name part), each optionally carrying the
    /// `EXCLUDE`/`REPLACE`/`RENAME` modifiers — returning the pattern-free
    /// [`ColumnsSpelling::Star`] node for the star forms and an ordinary expression
    /// otherwise. DuckDB's `columns:false` STAR node, admitted only in the `ORDER BY`
    /// sort key and the `UNPIVOT` `ON`/`IN` column positions (`ORDER BY *`,
    /// `UNPIVOT t ON * EXCLUDE (id)`, `IN (*)`; probed on 1.5.4). Gated on
    /// [`CallSyntax::columns_expression`](crate::ast::dialect::CallSyntax::columns_expression): with the star surface off this is exactly
    /// [`parse_expr`](Self::parse_expr), so a bare `*` keeps its ordinary reject in every
    /// other dialect.
    pub(in crate::parser) fn parse_star_or_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        if !self.features().call_syntax.columns_expression {
            return self.parse_expr();
        }
        if self.peek_is_op(Operator::Star)? {
            let star = self
                .advance()?
                .expect("peek_is_op confirmed the bare `*` is present");
            let options = self.parse_wildcard_modifier_tail(star.span)?;
            return Ok(self.build_bare_star_columns(None, options, star.span));
        }
        let expr = self.parse_expr()?;
        let qualified_star =
            self.peek_is_punct(Punctuation::Dot)? && self.peek_nth_is_op(1, Operator::Star)?;
        match expr {
            Expr::Column { name, .. } if qualified_star && name.0.len() == 1 => {
                let name_span = name.span();
                self.advance()?; // `.`
                let star = self.advance()?.expect("peek confirmed the `*`");
                let options = self.parse_wildcard_modifier_tail(star.span)?;
                Ok(self.build_bare_star_columns(Some(name), options, name_span))
            }
            expr => Ok(expr),
        }
    }

    /// Assemble a [`ColumnsSpelling::Star`] node (the bare `*` / `t.*` star expansion)
    /// spanning from `start` to the just-consumed token.
    fn build_bare_star_columns(
        &mut self,
        qualifier: Option<crate::ast::ObjectName>,
        options: Option<Box<crate::ast::WildcardOptions<D::Ext>>>,
        start: Span,
    ) -> Expr<D::Ext> {
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Expr::Columns {
            qualifier,
            pattern: None,
            options,
            spelling: ColumnsSpelling::Star,
            meta,
        }
    }
    /// Parse a searched or simple `CASE … WHEN … THEN … [ELSE …] END`.
    ///
    /// A `WHEN` immediately after `CASE` is the searched form (no operand);
    /// otherwise the expression between `CASE` and the first `WHEN` is the simple
    /// form's compared operand. At least one `WHEN` branch is required.
    fn parse_case(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_case is reached only with a current CASE token")
            .span;

        let operand = if self.peek_is_keyword(Keyword::When)? {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };

        let mut when_clauses = ThinVec::new();
        while self.peek_is_keyword(Keyword::When)? {
            let when_token = self
                .advance()?
                .expect("peek_is_keyword confirmed a WHEN token is present");
            let condition = self.parse_expr()?;
            self.expect_keyword(Keyword::Then)?;
            let result = self.parse_expr()?;
            let clause_span = when_token.span.union(self.preceding_span());
            when_clauses.push(WhenClause {
                condition,
                result,
                meta: self.make_meta(clause_span),
            });
        }
        if when_clauses.is_empty() {
            return Err(self.unexpected("`WHEN` after `CASE`"));
        }

        let else_result = if self.eat_keyword(Keyword::Else)? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect_keyword(Keyword::End)?;

        let span = start.union(self.preceding_span());
        let case_meta = self.make_meta(span);
        let case = CaseExpr {
            operand,
            when_clauses,
            else_result,
            meta: case_meta,
        };
        let meta = self.make_meta(span);
        Ok(Expr::Case {
            case: Box::new(case),
            meta,
        })
    }
    /// Parse `EXTRACT(<field> FROM <source>)`, or a bare `extract` identifier.
    ///
    /// `EXTRACT` is non-reserved, so it only begins the special form when an open
    /// paren follows; otherwise it is an ordinary (possibly qualified) column.
    fn parse_extract_or_column(&mut self) -> ParseResult<Expr<D::Ext>> {
        if self.peek_nth_is_punct(1, Punctuation::LParen)? {
            self.parse_extract()
        } else {
            let name = self.parse_object_name()?;
            let meta = self.make_meta(name.span());
            Ok(Expr::Column { name, meta })
        }
    }
    /// Parse `EXTRACT(<field> FROM <source>)`.
    ///
    /// The field is the datetime field name (`YEAR`, `MONTH`, …) parsed as an
    /// identifier; the source is the value it is pulled from.
    fn parse_extract(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_extract is reached only with a current EXTRACT token")
            .span;
        // MySQL's `IGNORE_SPACE`-off tokenizer demotes a spaced `EXTRACT (…)` to a general
        // call, where the `<field> FROM <source>` special-argument grammar is illegal
        // (`EXTRACT (YEAR FROM x)` → engine-measured 1064), the same adjacency rule the
        // aggregate/window forms follow in `parse_function_call`. Adjacency is a source-offset
        // test: any trivia between `EXTRACT` and `(` leaves a gap.
        if self
            .features()
            .aggregate_call_syntax
            .aggregate_args_require_adjacent_paren
            && start.end() != self.current_span()?.start()
        {
            return Err(self.unexpected(
                "`EXTRACT` adjacent to `(`: the `EXTRACT(field FROM source)` special form \
                 requires no space before the parentheses",
            ));
        }
        self.expect_punct(Punctuation::LParen, "`(` after `EXTRACT`")?;
        // DuckDB (and PostgreSQL) admit a single-quoted string as the field
        // (`extract('year' FROM x)`); the value interns as an identifier with
        // `QuoteStyle::Single`, reusing the string-alias machinery so it round-trips.
        // With the flag off, only a bare identifier is a field.
        let field = if self.features().call_syntax.extract_string_field {
            match self.parse_string_alias_ident()? {
                Some(ident) => ident,
                None => self.parse_ident()?,
            }
        } else {
            self.parse_ident()?
        };
        self.expect_keyword(Keyword::From)?;
        let source = self.parse_expr()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the `EXTRACT` field")?;
        let span = start.union(self.preceding_span());
        let extract_meta = self.make_meta(span);
        let extract = ExtractExpr {
            field,
            source: Box::new(source),
            meta: extract_meta,
        };
        let meta = self.make_meta(span);
        Ok(Expr::Extract {
            extract: Box::new(extract),
            meta,
        })
    }
    /// Parse `EXISTS (<query>)`, or a bare `exists` column reference.
    ///
    /// `EXISTS` is the subquery operator only when `(` follows; `exists` is a
    /// `col_name` keyword, so a bare occurrence (`SELECT exists`) is an ordinary
    /// column reference, matching PostgreSQL.
    fn parse_exists_or_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        if self.peek_nth_is_punct(1, Punctuation::LParen)? {
            self.parse_exists()
        } else {
            self.parse_word_prefix(token)
        }
    }
    /// Parse `EXISTS (<query>)`.
    fn parse_exists(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self
            .advance()?
            .expect("parse_exists is reached only with a current EXISTS token")
            .span;
        let query =
            self.parse_query_in_parens("`(` after `EXISTS`", "`)` to close the `EXISTS` subquery")?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::Exists {
            query: Box::new(query),
            meta,
        })
    }
    /// Parse `NULLIF(a, b)`, or a bare `nullif` column reference.
    ///
    /// PostgreSQL's `NULLIF` is a dedicated two-argument production, not a generic
    /// call: it admits exactly two plain arguments and nothing else (`NULLIF(a)`,
    /// `NULLIF(a, b, c)`, `NULLIF(*)`, `NULLIF(DISTINCT …)` are all rejected). The
    /// canonical [`Expr::Function`] shape is kept so the structural-mapping oracle
    /// (which still treats `nullif` as a generic `FuncCall` gap) is unaffected. A
    /// bare `nullif` is a `col_name` column reference.
    fn parse_nullif_or_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        if !self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return self.parse_word_prefix(token);
        }
        let head_reserved = self.name_or_call_head_reserved()?;
        let name = self.parse_object_name_with(head_reserved)?;
        let call = self.parse_function_call(name, token.span)?;
        if call.args.len() != 2
            || call.wildcard
            || call.quantifier.is_some()
            || !call.order_by.is_empty()
            || call.within_group.is_some()
            || call.filter.is_some()
            || call.over.is_some()
        {
            return Err(self.error_at(
                call.meta.span,
                "a `NULLIF(a, b)` call with exactly two arguments",
                self.span_text(call.meta.span).to_owned(),
            ));
        }
        let meta = self.make_meta(call.meta.span);
        Ok(Expr::Function {
            call: Box::new(call),
            meta,
        })
    }
    /// Parse a MySQL `VALUES(<col>)` reference (the leading `VALUES (` confirmed by
    /// the caller).
    ///
    /// `VALUES` is a reserved function name in the shared inventory, so the call name
    /// is admitted explicitly (an empty reject set) rather than through the generic
    /// call-head gate, then folded into the canonical [`Expr::Function`] shape — the
    /// same node any other call yields, so consumers need no `VALUES`-special case.
    fn parse_values_reference(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let name = self.parse_object_name_with(KeywordSet::EMPTY)?;
        let call = self.parse_function_call(name, token.span)?;
        let meta = self.make_meta(call.meta.span);
        Ok(Expr::Function {
            call: Box::new(call),
            meta,
        })
    }
    /// Parse PostgreSQL's `merge_action()` MERGE-RETURNING support function (the leading
    /// `merge_action (` confirmed by the caller).
    ///
    /// PostgreSQL's dedicated `MERGE_ACTION '(' ')'` production takes *strictly* empty
    /// parens: `merge_action(1)` and `merge_action() OVER ()` are both syntax errors
    /// (engine-probed). The keyword is reserved against ordinary calls, so it is admitted
    /// as a call name explicitly (an empty reject set), then the parsed call is required to
    /// be niladic and modifier-free before it folds into the canonical [`Expr::Function`]
    /// shape — the same node any other call yields, so consumers need no special case.
    /// (The MERGE-RETURNING-only restriction is a parse-*analysis* concern PostgreSQL
    /// enforces after the raw parse, which accepts the form anywhere, so it is not modelled
    /// here.)
    fn parse_merge_action_function(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let name = self.parse_object_name_with(KeywordSet::EMPTY)?;
        let call = self.parse_function_call(name, token.span)?;
        if !call.args.is_empty()
            || call.wildcard
            || call.quantifier.is_some()
            || !call.order_by.is_empty()
            || call.within_group.is_some()
            || call.filter.is_some()
            || call.over.is_some()
        {
            return Err(self.error_at(
                call.meta.span,
                "`merge_action()` with empty parentheses and no other arguments",
                self.span_text(call.meta.span).to_owned(),
            ));
        }
        let meta = self.make_meta(call.meta.span);
        Ok(Expr::Function {
            call: Box::new(call),
            meta,
        })
    }
    /// Parse a SQL special value function: a nullary keyword (`CURRENT_DATE`,
    /// `CURRENT_USER`, `USER`, …) or, for the four temporal forms, an optional
    /// `(precision)`.
    ///
    /// `CURRENT_SCHEMA` is also an ordinary (`type_func_name`) function name, so a
    /// call form `current_schema(...)` defers to the generic call path; only the
    /// bare keyword is the special value function. The other keywords are reserved,
    /// so a trailing `(` (other than the temporal precision) is left to fail
    /// downstream, exactly as PostgreSQL rejects `CURRENT_DATE(1)`.
    fn parse_special_function(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let TokenKind::Keyword(keyword) = token.kind else {
            unreachable!("parse_special_function is reached only with a keyword token");
        };
        if keyword == Keyword::CurrentSchema && self.peek_nth_is_punct(1, Punctuation::LParen)? {
            return self.parse_word_prefix(token);
        }
        let (sf_keyword, takes_precision) = special_function_keyword(keyword);
        let start = self
            .advance()?
            .expect("parse_special_function is reached only with a current keyword token")
            .span;
        let precision = if takes_precision && self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let precision = self.parse_u32_type_modifier()?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the special-function precision",
            )?;
            Some(precision)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::SpecialFunction {
            keyword: sf_keyword,
            precision,
            meta,
        })
    }
}
