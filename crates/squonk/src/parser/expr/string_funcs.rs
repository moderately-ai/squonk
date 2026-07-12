// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The standard-SQL string special forms (the `planner-parity-expr-substring/`
//! `position/overlay/trim` bundle).
//!
//! `SUBSTRING(x FROM a [FOR b])` (+ the `FOR`-leading orders and PostgreSQL's
//! `SIMILAR … ESCAPE` regex form), `POSITION(substr IN str)`,
//! `OVERLAY(x PLACING y FROM a [FOR b])`, and
//! `TRIM([{BOTH|LEADING|TRAILING}] [chars] FROM src)` (+ PostgreSQL's loose
//! `trim_list` tails) — the SQL-92 E021 / SQL:1999 T312 keyword-argument string
//! functions, cross-engine probed (pg_query PG 17, DuckDB 1.5.4, SQLite, MySQL
//! 8.4) and gated per form on the [`CallSyntax`](crate::ast::dialect::CallSyntax)
//! `substring_*`/`position_*`/`overlay_*`/`trim_*` flags. The comma plain-call
//! spellings (`substring(x, 1, 2)`, `trim(x, y)`, `overlay(a, b, c)`) keep their
//! ordinary [`Expr::Function`] reading everywhere — each dispatcher falls back to
//! the generic call path (or rejects, where the engine's grammar has no fallback:
//! `position(a, b)` and DuckDB's `overlay(a, b)` are parse errors).

use crate::ast::{CeilSpelling, Expr, Keyword, MatchSearchModifier, StringFunc, TrimSide};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Punctuation, Token, TokenKind};
use thin_vec::ThinVec;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse `SUBSTRING(…)` / `SUBSTR(…)` — the keyword forms when a
    /// `FROM`/`FOR`/`SIMILAR` tail follows the first operand, else the ordinary
    /// call fallback. The caller confirmed the gate is on and that `(` follows.
    pub(super) fn parse_substring_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start_cp = self.checkpoint();
        self.advance()?; // SUBSTRING / substr
        // MySQL's `IGNORE_SPACE`-off tokenizer demotes a spaced head to the generic
        // stored-function path, where the keyword-argument grammar is illegal but any
        // comma arity parses — engine-measured (`SUBSTRING ('a' FROM 2)` is 1064,
        // `SUBSTRING ('a', 2)` parse-accepts), the same adjacency rule as `EXTRACT`.
        if self
            .features()
            .aggregate_call_syntax
            .aggregate_args_require_adjacent_paren
            && token.span.end() != self.current_span()?.start()
        {
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        self.expect_punct(Punctuation::LParen, "`(` after `SUBSTRING`")?;
        if self.peek_is_punct(Punctuation::RParen)? {
            // Empty argument list: PostgreSQL parse-accepts `substring()` (arity is a
            // catalog concern), so it stays the plain-call path — where the MySQL
            // arity floor then rejects it.
            self.rewind(start_cp);
            return self.parse_substring_plain_call(token);
        }
        let expr = self.parse_expr()?;
        if self.eat_keyword(Keyword::From)? {
            let start = self.parse_expr()?;
            let count = if self.eat_keyword(Keyword::For)? {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect_punct(Punctuation::RParen, "`)` to close `SUBSTRING`")?;
            return self.finish_substring(token, expr, Some(Box::new(start)), count);
        }
        if self.features().string_func_forms.substring_leading_for
            && self.peek_is_keyword(Keyword::For)?
        {
            self.advance()?; // FOR
            let count = self.parse_expr()?;
            // The reversed `FOR … FROM …` source order (PostgreSQL/DuckDB accept it,
            // probed) folds onto the same fields; rendering is canonically FROM-first.
            let start = if self.eat_keyword(Keyword::From)? {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect_punct(Punctuation::RParen, "`)` to close `SUBSTRING`")?;
            return self.finish_substring(token, expr, start, Some(Box::new(count)));
        }
        if self.features().string_func_forms.substring_similar
            && self.peek_is_keyword(Keyword::Similar)?
        {
            self.advance()?; // SIMILAR
            let pattern = self.parse_expr()?;
            // The ESCAPE operand is mandatory (pg_query rejects `SIMILAR p` alone).
            self.expect_keyword(Keyword::Escape)?;
            let escape = self.parse_expr()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `SUBSTRING`")?;
            let span = token.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::StringFunc {
                string_func: Box::new(StringFunc::SubstringSimilar {
                    expr: Box::new(expr),
                    pattern: Box::new(pattern),
                    escape: Box::new(escape),
                    meta,
                }),
                meta,
            });
        }
        // No keyword tail: the comma/plain-call spelling — rewind to the head and
        // take the ordinary call path (kept parsing on every probed engine).
        self.rewind(start_cp);
        self.parse_substring_plain_call(token)
    }

    fn finish_substring(
        &mut self,
        token: Token,
        expr: Expr<D::Ext>,
        start: Option<Box<Expr<D::Ext>>>,
        count: Option<Box<Expr<D::Ext>>>,
    ) -> ParseResult<Expr<D::Ext>> {
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::Substring {
                expr: Box::new(expr),
                start,
                count,
                meta,
            }),
            meta,
        })
    }

    /// The `substring`/`substr` plain-call fallback, plus MySQL's grammar-level
    /// 2-3 argument floor (`SUBSTRING('a')` / `SUBSTRING()` / a 4-argument call are
    /// `ER_PARSE_ERROR` on mysql:8.4 while PostgreSQL parse-accepts any arity).
    fn parse_substring_plain_call(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let expr = self.parse_word_prefix(token)?;
        if self
            .features()
            .string_func_forms
            .substring_plain_call_requires_2_or_3_args
        {
            if let Expr::Function { call, .. } = &expr {
                if call.wildcard || !(2..=3).contains(&call.args.len()) {
                    return Err(self.error_at(
                        call.meta.span,
                        "a 2- or 3-argument `substring(str, pos [, len])` call",
                        self.span_text(call.meta.span).to_owned(),
                    ));
                }
            }
        }
        Ok(expr)
    }

    /// Parse `POSITION(<substr> IN <string>)`. There is no plain-call fallback:
    /// every keyword-form engine parse-rejects `position(a, b)`, so a comma (or
    /// anything else) after the first operand is the clean `IN` expectation error.
    ///
    /// The operands are PostgreSQL's `b_expr` (`POSITION('a' = 'b' IN 'c')`
    /// accepts, `POSITION(1 IN 2 OR 3)` rejects — both probed on pg_query and
    /// DuckDB). Under `position_asymmetric_operands` (MySQL) the needle tightens
    /// to `bit_expr` — the `b_expr` restriction plus a binding-power floor above
    /// the comparisons — and the haystack widens to a full `a_expr` (probed:
    /// `POSITION('a' = 'b' IN 'c')` is 1064 there, `POSITION(1 IN 2 OR 3)` accepts).
    pub(super) fn parse_position_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // POSITION
        self.expect_punct(Punctuation::LParen, "`(` after `POSITION`")?;
        let asymmetric = self
            .features()
            .string_func_forms
            .position_asymmetric_operands;
        let substr = if asymmetric {
            self.parse_bit_expr()?
        } else {
            self.parse_b_expr()?
        };
        self.expect_keyword(Keyword::In)?;
        let string = if asymmetric {
            self.parse_expr()?
        } else {
            self.parse_b_expr()?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `POSITION`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::Position {
                substr: Box::new(substr),
                string: Box::new(string),
                meta,
            }),
            meta,
        })
    }

    /// Parse PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr (the leading
    /// `COLLATION FOR (` confirmed by the caller). The parentheses wrap a single
    /// `a_expr`; PostgreSQL lowers the form to `pg_catalog.pg_collation_for(<expr>)`,
    /// but the surface keyword shape is preserved as [`StringFunc::CollationFor`] so it
    /// round-trips as written.
    pub(super) fn parse_collation_for(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // COLLATION
        self.expect_keyword(Keyword::For)?;
        self.expect_punct(Punctuation::LParen, "`(` after `COLLATION FOR`")?;
        let expr = self.parse_expr()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `COLLATION FOR`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::CollationFor {
                expr: Box::new(expr),
                meta,
            }),
            meta,
        })
    }

    /// Parse `CEIL(<expr> TO <field>)` / `CEILING(<expr> TO <field>)` — the
    /// rounding-field keyword form when a `TO` tail follows the first operand, else the
    /// ordinary call fallback (including the comma scale spelling
    /// `CEIL(<expr>, <scale>)`, and any other arity — no probed oracle grammar admits
    /// the `TO` tail, so this is sqlparser-rs-parity surface only). The caller confirmed
    /// the gate is on and that `(` follows.
    pub(super) fn parse_ceil_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start_cp = self.checkpoint();
        let spelling = match token.kind {
            TokenKind::Keyword(Keyword::Ceiling) => CeilSpelling::Ceiling,
            _ => CeilSpelling::Ceil,
        };
        self.advance()?; // CEIL / CEILING
        self.expect_punct(Punctuation::LParen, "`(` after `CEIL`")?;
        if self.peek_is_punct(Punctuation::RParen)? {
            // Empty argument list stays the ordinary call path (arity is a catalog concern).
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        let expr = self.parse_expr()?;
        if self.eat_keyword(Keyword::To)? {
            let field = self.parse_ident()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `CEIL`")?;
            let span = token.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::StringFunc {
                string_func: Box::new(StringFunc::CeilTo {
                    expr: Box::new(expr),
                    field,
                    spelling,
                    meta,
                }),
                meta,
            });
        }
        // No `TO` tail: the comma/plain-call spelling — rewind and take the ordinary
        // call path (kept parsing on every probed engine).
        self.rewind(start_cp);
        self.parse_word_prefix(token)
    }

    /// Parse `FLOOR(<expr> TO <field>)` — the rounding-field keyword form when a `TO`
    /// tail follows the first operand, else the ordinary call fallback (including the
    /// comma scale spelling `FLOOR(<expr>, <scale>)`, and any other arity — no probed
    /// oracle grammar admits the `TO` tail, so this is sqlparser-rs-parity surface only).
    /// Unlike [`Self::parse_ceil_expr`], `FLOOR` has no synonym spelling to track. The
    /// caller confirmed the gate is on and that `(` follows.
    pub(super) fn parse_floor_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start_cp = self.checkpoint();
        self.advance()?; // FLOOR
        self.expect_punct(Punctuation::LParen, "`(` after `FLOOR`")?;
        if self.peek_is_punct(Punctuation::RParen)? {
            // Empty argument list stays the ordinary call path (arity is a catalog concern).
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        let expr = self.parse_expr()?;
        if self.eat_keyword(Keyword::To)? {
            let field = self.parse_ident()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `FLOOR`")?;
            let span = token.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Expr::StringFunc {
                string_func: Box::new(StringFunc::FloorTo {
                    expr: Box::new(expr),
                    field,
                    meta,
                }),
                meta,
            });
        }
        // No `TO` tail: the comma/plain-call spelling — rewind and take the ordinary
        // call path (kept parsing on every probed engine).
        self.rewind(start_cp);
        self.parse_word_prefix(token)
    }

    /// Parse MySQL's full-text `MATCH (<col>, …) AGAINST (<expr> [<modifier>])` special
    /// form (the caller confirmed `MATCH` is followed by `(` under the gate). The column
    /// list is comma-separated column references — a general expression, literal,
    /// function call, or empty list all parse-reject on mysql:8.4.10. The `AGAINST`
    /// operand is a `bit_expr` (below the comparison level) so a trailing `IN`/`WITH`
    /// opens the modifier rather than being swallowed as an `IN` predicate. `AGAINST`,
    /// `QUERY`, and `EXPANSION` are non-reserved MySQL keywords matched contextually.
    pub(super) fn parse_match_against(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // MATCH
        self.expect_punct(Punctuation::LParen, "`(` after `MATCH`")?;
        let columns = self.parse_comma_separated(Self::parse_match_column)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the `MATCH` column list")?;
        self.expect_contextual_keyword("AGAINST")?;
        self.expect_punct(Punctuation::LParen, "`(` after `AGAINST`")?;
        let against = self.parse_bit_expr()?;
        let modifier = self.parse_match_modifier()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `AGAINST`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::MatchAgainst {
                columns,
                against: Box::new(against),
                modifier,
                meta,
            }),
            meta,
        })
    }

    /// Parse one `MATCH` column-list item: a bare or 1–3-part dotted column reference
    /// ([`Expr::Column`]). A non-identifier head or a trailing operator leaves the
    /// enclosing `,`/`)` expectation to reject, matching MySQL's `simple_ident`-only
    /// column list.
    fn parse_match_column(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Expr::Column { name, meta })
    }

    /// Parse the optional full-text search modifier after the `AGAINST` operand,
    /// returning `None` for the default (no modifier words). Exactly the four documented
    /// combinations parse; every other `IN …`/`WITH …` tail (e.g. `IN BOOLEAN MODE WITH
    /// QUERY EXPANSION`, `IN QUERY EXPANSION`, `WITH EXPANSION`) leaves an unconsumed token
    /// for the closing `)` to reject, as on mysql:8.4.10.
    fn parse_match_modifier(&mut self) -> ParseResult<Option<MatchSearchModifier>> {
        if self.eat_keyword(Keyword::In)? {
            if self.eat_keyword(Keyword::Boolean)? {
                self.expect_keyword(Keyword::Mode)?;
                return Ok(Some(MatchSearchModifier::Boolean));
            }
            self.expect_keyword(Keyword::Natural)?;
            self.expect_keyword(Keyword::Language)?;
            self.expect_keyword(Keyword::Mode)?;
            if self.eat_keyword(Keyword::With)? {
                self.expect_contextual_keyword("QUERY")?;
                self.expect_contextual_keyword("EXPANSION")?;
                return Ok(Some(MatchSearchModifier::NaturalLanguageQueryExpansion));
            }
            return Ok(Some(MatchSearchModifier::NaturalLanguage));
        }
        if self.eat_keyword(Keyword::With)? {
            self.expect_contextual_keyword("QUERY")?;
            self.expect_contextual_keyword("EXPANSION")?;
            return Ok(Some(MatchSearchModifier::QueryExpansion));
        }
        Ok(None)
    }

    /// Parse MySQL's `bit_expr` operand level: the `b_expr` restriction (no `NOT`
    /// head, no `IN`/`BETWEEN`/`LIKE` predicates) with a binding-power floor above
    /// the comparisons, so only the arithmetic/bit/concat operators fold.
    fn parse_bit_expr(&mut self) -> ParseResult<Expr<D::Ext>> {
        let floor = self.features().binding_powers.comparison.left + 1;
        let saved = self.restrict_b_expr;
        self.restrict_b_expr = true;
        let result = self.parse_expr_bp(floor);
        self.restrict_b_expr = saved;
        result
    }

    /// Parse `OVERLAY(<target> PLACING <replacement> FROM <start> [FOR <count>])`,
    /// falling back to the ordinary call when no `PLACING` follows the first operand
    /// — unless `overlay_requires_placing` (DuckDB's grammar keeps only the
    /// `PLACING` production; PostgreSQL parse-accepts `overlay('a')`/`overlay()`).
    pub(super) fn parse_overlay_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start_cp = self.checkpoint();
        let requires_placing = self.features().string_func_forms.overlay_requires_placing;
        self.advance()?; // OVERLAY
        self.expect_punct(Punctuation::LParen, "`(` after `OVERLAY`")?;
        if self.peek_is_punct(Punctuation::RParen)? {
            if requires_placing {
                return Err(self.unexpected("an expression"));
            }
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        let target = self.parse_expr()?;
        if !self.eat_keyword(Keyword::Placing)? {
            if requires_placing {
                return Err(self.unexpected("`PLACING` after the `OVERLAY` target"));
            }
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        let replacement = self.parse_expr()?;
        // `FROM <start>` is mandatory: `OVERLAY(x PLACING y)` and the bare
        // `FOR`-without-`FROM` form reject on every keyword-form engine (probed).
        self.expect_keyword(Keyword::From)?;
        let start = self.parse_expr()?;
        let count = if self.eat_keyword(Keyword::For)? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `OVERLAY`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::Overlay {
                target: Box::new(target),
                replacement: Box::new(replacement),
                start: Box::new(start),
                count,
                meta,
            }),
            meta,
        })
    }

    /// Parse `TRIM([{BOTH | LEADING | TRAILING}] <trim_list>)`. The restricted
    /// shape (`trim_from`) is the standard/MySQL `[side] [chars] FROM <source>`
    /// with at least one of side/chars; `trim_list_syntax` adds PostgreSQL's loose
    /// tails (bare `FROM <list>`, a side without `FROM`, multi-expression lists,
    /// and the comma plain-call fallback). A bare `TRIM(x)` is the ordinary call
    /// everywhere; `TRIM()` rejects (all keyword-form engines probed rejecting).
    pub(super) fn parse_trim_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let start_cp = self.checkpoint();
        self.advance()?; // TRIM
        // The same MySQL spaced-head demotion as `SUBSTRING` (probed: spaced
        // `TRIM (LEADING …)` is 1064, spaced `TRIM ('abc')` parse-accepts).
        if self
            .features()
            .aggregate_call_syntax
            .aggregate_args_require_adjacent_paren
            && token.span.end() != self.current_span()?.start()
        {
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        }
        self.expect_punct(Punctuation::LParen, "`(` after `TRIM`")?;
        let side = if self.eat_keyword(Keyword::Both)? {
            Some(TrimSide::Both)
        } else if self.eat_keyword(Keyword::Leading)? {
            Some(TrimSide::Leading)
        } else if self.eat_keyword(Keyword::Trailing)? {
            Some(TrimSide::Trailing)
        } else {
            None
        };
        let loose = self.features().string_func_forms.trim_list_syntax;
        // Each grammar branch yields the `(trim_chars, from, sources)` shape; the
        // fallback/error branches return out of the function instead.
        let (trim_chars, from, sources) = 'shape: {
            // The bare `FROM <list>` head (no chars) is loose-only: MySQL rejects
            // `TRIM(FROM 'x')` (1064) while PostgreSQL/DuckDB accept it (all probed).
            if loose && self.eat_keyword(Keyword::From)? {
                let sources = self.parse_comma_separated(Self::parse_expr)?;
                break 'shape (None, true, sources);
            }
            if side.is_some() && !loose {
                // Restricted side form: `[chars] FROM <source>`, the FROM mandatory
                // (`TRIM(BOTH 'x')` is 1064 on MySQL, probed).
                let trim_chars = if self.eat_keyword(Keyword::From)? {
                    None
                } else {
                    let chars = self.parse_expr()?;
                    self.expect_keyword(Keyword::From)?;
                    Some(Box::new(chars))
                };
                let source = self.parse_expr()?;
                let mut sources = ThinVec::new();
                sources.push(source);
                break 'shape (trim_chars, true, sources);
            }
            let first = self.parse_expr()?;
            if self.eat_keyword(Keyword::From)? {
                // `<chars> FROM <sources>`: the source is one expression in the
                // restricted shape and PostgreSQL's `expr_list` under the loose one
                // (`TRIM('a' FROM 'b', 'c')` accepts there, probed).
                let sources = if loose {
                    self.parse_comma_separated(Self::parse_expr)?
                } else {
                    let mut sources = ThinVec::new();
                    sources.push(self.parse_expr()?);
                    sources
                };
                break 'shape (Some(Box::new(first)), true, sources);
            }
            if side.is_some() {
                // A side without `FROM` is PostgreSQL's loose `expr_list` tail
                // (`TRIM(TRAILING ' foo ')`, `TRIM(LEADING 'x', 'y')`); only the loose
                // grammar reaches here (the restricted arm above consumed the side).
                let mut sources = ThinVec::new();
                sources.push(first);
                while self.eat_punct(Punctuation::Comma)? {
                    sources.push(self.parse_expr()?);
                }
                break 'shape (None, false, sources);
            }
            if self.peek_is_punct(Punctuation::Comma)? && !loose {
                // No side, no FROM, a comma tail: MySQL's grammar has no comma `TRIM`
                // (`trim('a', 'b')` is 1064, probed), so there is no generic fallback.
                return Err(self.unexpected("`FROM` after the `TRIM` operand"));
            }
            // A bare single expression (or, loose, a comma list): the ordinary call
            // spelling — rewind to the head and take the generic path.
            self.rewind(start_cp);
            return self.parse_word_prefix(token);
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `TRIM`")?;
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::StringFunc {
            string_func: Box::new(StringFunc::Trim {
                side,
                trim_chars,
                from,
                sources,
                meta,
            }),
            meta,
        })
    }
}
