// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQL:2016 `MATCH_RECOGNIZE (…)` row-pattern-recognition table factor
//! (Snowflake / Oracle) — the clause grammar and its recursive row-pattern
//! sublanguage.
//!
//! Reached as a table-factor suffix from
//! [`parse_pivot_suffixes`](Parser::parse_pivot_suffixes) (the PIVOT/UNPIVOT sibling),
//! gated on
//! [`TableFactorSyntax::match_recognize`](crate::ast::dialect::TableExpressionSyntax)
//! (Snowflake / Lenient). Snowflake position-reserves `MATCH_RECOGNIZE` on the `ColId`
//! axis (`SNOWFLAKE_TABLE_OPERATOR_RESERVATION`, alongside `PIVOT`/`UNPIVOT` — the
//! `bigquery-snowflake-pivot-keyword-reservation` follow-up), so a bare
//! `FROM t MATCH_RECOGNIZE (…)` reaches the operator directly. Lenient, the permissive
//! render target, deliberately keeps `MATCH_RECOGNIZE` *un*reserved — it must keep
//! accepting `match_recognize` as an ordinary identifier — so under Lenient the keyword is
//! swallowed as the source's alias first and the suffix is reachable only after an explicit
//! alias (`FROM t AS m MATCH_RECOGNIZE (…)`), the standard-PIVOT precedent.
//!
//! # The row-pattern sublanguage
//!
//! The `PATTERN ( … )` body is a small regular-expression grammar over pattern
//! variables — its own recursive-descent parser ([`parse_row_pattern`](Parser::parse_row_pattern)
//! and friends), deliberately kept out of any hot expression frame (the expr-split
//! precedent): the entry is `#[inline(never)]` and guarded by the shared recursion
//! budget ([`enter_recursion`](Parser::enter_recursion)), so a pathological deeply
//! nested pattern (`((((…))))`, `PERMUTE(PERMUTE(…))`) fails with a clean
//! `RecursionLimitExceeded` rather than overflowing the stack. Precedence, tightest
//! first: quantifier (postfix `*`/`+`/`{n,m}`) > concatenation (juxtaposition) >
//! alternation (`|`).
//!
//! ## Lexer reachability of `$` and `?`
//!
//! Two pattern tokens are *not* reachable through the eager, context-free tokenizer
//! (ADR-0005), which lexes the whole statement before the parser runs and has no
//! notion of "inside `PATTERN(…)`":
//!
//! - a bare `$` (the end anchor) is a stray byte under both Snowflake and Lenient — no
//!   dialect lexes it as a token;
//! - `?` (the `AtMostOne` quantifier, and the reluctant marker) is the anonymous
//!   parameter placeholder under Lenient and a stray byte under Snowflake.
//!
//! Recognizing them only inside a pattern would need a context-sensitive lexer *mode*,
//! which the tokenizer architecture excludes. The AST models
//! [`End`](crate::ast::MatchRecognizePattern::End) and
//! [`AtMostOne`](crate::ast::RepetitionQuantifier::AtMostOne) for sqlparser-rs parity
//! and lossless rendering (a programmatic builder can still produce them), but the
//! parser reaches only the tokens the lexer can emit: the `^` start anchor
//! ([`Operator::Caret`]), `|`, `( )`, `{- -}`, `PERMUTE`, and the greedy quantifiers
//! `*`, `+`, `{n}`, `{n,}`, `{,m}`, `{n,m}`.

use crate::ast::{
    AfterMatchSkip, EmptyMatchesMode, Keyword, MatchRecognize, MatchRecognizePattern, Measure,
    RepetitionQuantifier, RowsPerMatch, Span, SubsetDefinition, SymbolDefinition, TableFactor,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse one `<source> MATCH_RECOGNIZE (…)` suffix onto `source`, cursor on the
    /// `MATCH_RECOGNIZE` keyword. Every subclause but `PATTERN` is optional; their order
    /// is fixed (SQL:2016). This position owns the trailing `AS <alias>`.
    pub(super) fn parse_match_recognize_suffix(
        &mut self,
        start: Span,
        source: TableFactor<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::MatchRecognize)?;
        self.expect_punct(Punctuation::LParen, "`(` after MATCH_RECOGNIZE")?;

        let partition_by = if self.eat_keyword(Keyword::Partition)? {
            self.expect_keyword(Keyword::By)?;
            self.parse_comma_separated_exprs()?
        } else {
            ThinVec::new()
        };

        // `parse_order_by` consumes the `ORDER BY` keywords and yields an empty list
        // when the clause is absent.
        let order_by = self.parse_order_by()?;

        let measures = if self.eat_keyword(Keyword::Measures)? {
            self.parse_comma_separated(Self::parse_match_recognize_measure)?
        } else {
            ThinVec::new()
        };

        let rows_per_match = self.parse_rows_per_match()?;
        let after_match_skip = self.parse_after_match_skip()?;

        self.expect_keyword(Keyword::Pattern)?;
        self.expect_punct(Punctuation::LParen, "`(` after PATTERN")?;
        let pattern = self.parse_row_pattern()?;
        self.expect_punct(Punctuation::RParen, "`)` to close PATTERN")?;

        let subsets = if self.eat_keyword(Keyword::Subset)? {
            self.parse_comma_separated(Self::parse_subset_definition)?
        } else {
            ThinVec::new()
        };

        let define = if self.eat_keyword(Keyword::Define)? {
            self.parse_comma_separated(Self::parse_symbol_definition)?
        } else {
            ThinVec::new()
        };

        self.expect_punct(Punctuation::RParen, "`)` to close MATCH_RECOGNIZE")?;

        let core_meta = self.make_meta(start.union(self.preceding_span()));
        let match_recognize = Box::new(MatchRecognize {
            source: Box::new(source),
            partition_by,
            order_by,
            measures,
            rows_per_match,
            after_match_skip,
            pattern,
            subsets,
            define,
            meta: core_meta,
        });
        let alias = self.parse_optional_table_alias()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableFactor::MatchRecognize {
            match_recognize,
            alias,
            meta,
        })
    }

    /// One `MEASURES` item: `<expr> [AS] <name>`. The output-name alias is mandatory
    /// (the `AS` keyword itself optional, the Lenient-friendly relaxation).
    fn parse_match_recognize_measure(&mut self) -> ParseResult<Measure<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        self.eat_keyword(Keyword::As)?;
        let alias = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Measure { expr, alias, meta })
    }

    /// `ONE ROW PER MATCH` / `ALL ROWS PER MATCH [ SHOW EMPTY MATCHES | OMIT EMPTY
    /// MATCHES | WITH UNMATCHED ROWS ]`; `None` when neither is written.
    fn parse_rows_per_match(&mut self) -> ParseResult<Option<RowsPerMatch>> {
        if self.eat_keyword(Keyword::One)? {
            self.expect_keyword(Keyword::Row)?;
            self.expect_keyword(Keyword::Per)?;
            self.expect_keyword(Keyword::Match)?;
            return Ok(Some(RowsPerMatch::OneRow));
        }
        if self.eat_keyword(Keyword::All)? {
            self.expect_keyword(Keyword::Rows)?;
            self.expect_keyword(Keyword::Per)?;
            self.expect_keyword(Keyword::Match)?;
            return Ok(Some(RowsPerMatch::AllRows(
                self.parse_empty_matches_mode()?,
            )));
        }
        Ok(None)
    }

    /// The optional `ALL ROWS PER MATCH` empty/unmatched-row treatment.
    fn parse_empty_matches_mode(&mut self) -> ParseResult<Option<EmptyMatchesMode>> {
        if self.eat_keyword(Keyword::Show)? {
            self.expect_keyword(Keyword::Empty)?;
            self.expect_keyword(Keyword::Matches)?;
            return Ok(Some(EmptyMatchesMode::Show));
        }
        if self.eat_keyword(Keyword::Omit)? {
            self.expect_keyword(Keyword::Empty)?;
            self.expect_keyword(Keyword::Matches)?;
            return Ok(Some(EmptyMatchesMode::Omit));
        }
        if self.eat_keyword(Keyword::With)? {
            self.expect_keyword(Keyword::Unmatched)?;
            self.expect_keyword(Keyword::Rows)?;
            return Ok(Some(EmptyMatchesMode::WithUnmatched));
        }
        Ok(None)
    }

    /// `AFTER MATCH SKIP { PAST LAST ROW | TO NEXT ROW | TO FIRST <sym> | TO LAST <sym> }`;
    /// `None` when the clause is absent.
    fn parse_after_match_skip(&mut self) -> ParseResult<Option<AfterMatchSkip>> {
        if !self.eat_keyword(Keyword::After)? {
            return Ok(None);
        }
        let start = self.preceding_span();
        self.expect_keyword(Keyword::Match)?;
        self.expect_keyword(Keyword::Skip)?;
        if self.eat_keyword(Keyword::Past)? {
            self.expect_keyword(Keyword::Last)?;
            self.expect_keyword(Keyword::Row)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AfterMatchSkip::PastLastRow { meta }));
        }
        self.expect_keyword(Keyword::To)?;
        if self.eat_keyword(Keyword::Next)? {
            self.expect_keyword(Keyword::Row)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AfterMatchSkip::ToNextRow { meta }));
        }
        if self.eat_keyword(Keyword::First)? {
            let symbol = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AfterMatchSkip::ToFirst { symbol, meta }));
        }
        self.expect_keyword(Keyword::Last)?;
        let symbol = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(AfterMatchSkip::ToLast { symbol, meta }))
    }

    /// One `SUBSET` union-variable definition: `<name> = ( <member>, … )`.
    fn parse_subset_definition(&mut self) -> ParseResult<SubsetDefinition> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        self.expect_op(Operator::Eq, "`=` in a SUBSET definition")?;
        self.expect_punct(Punctuation::LParen, "`(` after `=` in a SUBSET definition")?;
        let members = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, "`)` to close a SUBSET member list")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SubsetDefinition {
            name,
            members,
            meta,
        })
    }

    /// One `DEFINE` clause: `<sym> AS <condition>`.
    fn parse_symbol_definition(&mut self) -> ParseResult<SymbolDefinition<D::Ext>> {
        let start = self.current_span()?;
        let symbol = self.parse_ident()?;
        self.expect_keyword(Keyword::As)?;
        let definition = self.parse_expr()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SymbolDefinition {
            symbol,
            definition,
            meta,
        })
    }

    // ---- the row-pattern sublanguage ---------------------------------------------

    /// Parse one row pattern (the `PATTERN ( … )` body, a group `( … )`, a `{- … -}`
    /// exclusion body, or a `PERMUTE(…)` argument).
    ///
    /// The single recursion entry: `#[inline(never)]` to keep the pattern parser's
    /// scratch off any hot expression frame, and guarded by the shared recursion budget
    /// so a deeply nested pattern rejects cleanly instead of overflowing the stack. Each
    /// nesting level (a group, exclusion, or permute element re-entering here) bumps the
    /// budget once.
    #[inline(never)]
    pub(super) fn parse_row_pattern(&mut self) -> ParseResult<MatchRecognizePattern> {
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard.parser().parse_row_pattern_alternation()
    }

    /// Alternation `a | b | c` — the loosest-binding pattern operator.
    fn parse_row_pattern_alternation(&mut self) -> ParseResult<MatchRecognizePattern> {
        let start = self.current_span()?;
        let first = self.parse_row_pattern_concat()?;
        if !self.peek_is_op(Operator::Pipe)? {
            return Ok(first);
        }
        let mut patterns = thin_vec![first];
        while self.eat_op(Operator::Pipe)? {
            patterns.push(self.parse_row_pattern_concat()?);
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(MatchRecognizePattern::Alternation { patterns, meta })
    }

    /// Concatenation `a b c` — juxtaposed factors, matched in order. Runs until a
    /// sequence terminator (`|`, `)`, `,`, `-}`, or end of input).
    fn parse_row_pattern_concat(&mut self) -> ParseResult<MatchRecognizePattern> {
        let start = self.current_span()?;
        let first = self.parse_row_pattern_factor()?;
        if self.peek_ends_row_pattern_sequence()? {
            return Ok(first);
        }
        let mut patterns = thin_vec![first];
        while !self.peek_ends_row_pattern_sequence()? {
            patterns.push(self.parse_row_pattern_factor()?);
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(MatchRecognizePattern::Concat { patterns, meta })
    }

    /// A pattern primary with its optional postfix quantifier (`A`, `A*`, `A{2,3}`).
    fn parse_row_pattern_factor(&mut self) -> ParseResult<MatchRecognizePattern> {
        let start = self.current_span()?;
        let primary = self.parse_row_pattern_primary()?;
        match self.parse_row_pattern_quantifier()? {
            Some(quantifier) => {
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(MatchRecognizePattern::Repetition {
                    pattern: Box::new(primary),
                    quantifier,
                    meta,
                })
            }
            None => Ok(primary),
        }
    }

    /// A pattern primary: a `^` anchor, a `( … )` group, a `{- … -}` exclusion, a
    /// `PERMUTE(…)`, or a pattern-variable symbol. (The `$` end anchor is not
    /// lexer-reachable — see the module docs.)
    fn parse_row_pattern_primary(&mut self) -> ParseResult<MatchRecognizePattern> {
        let start = self.current_span()?;
        if self.eat_op(Operator::Caret)? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MatchRecognizePattern::Start { meta });
        }
        if self.eat_punct(Punctuation::LParen)? {
            let inner = self.parse_row_pattern()?;
            self.expect_punct(Punctuation::RParen, "`)` to close a pattern group")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MatchRecognizePattern::Group {
                pattern: Box::new(inner),
                meta,
            });
        }
        // `{- … -}` exclusion. A `{` opening a factor is only ever an exclusion (a
        // `{n,m}` bound is consumed postfix, never in factor-start position), and the
        // `-` lookahead distinguishes it from a stray `{`.
        if self.peek_is_punct(Punctuation::LBrace)? && self.peek_nth_is_op(1, Operator::Minus)? {
            self.advance()?; // `{`
            self.advance()?; // `-`
            let inner = self.parse_row_pattern()?;
            self.expect_op(Operator::Minus, "`-}` to close a pattern exclusion")?;
            self.expect_punct(Punctuation::RBrace, "`-}` to close a pattern exclusion")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MatchRecognizePattern::Exclude {
                pattern: Box::new(inner),
                meta,
            });
        }
        if self.eat_keyword(Keyword::Permute)? {
            self.expect_punct(Punctuation::LParen, "`(` after PERMUTE")?;
            let patterns = self.parse_comma_separated(Self::parse_row_pattern)?;
            self.expect_punct(Punctuation::RParen, "`)` to close PERMUTE")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(MatchRecognizePattern::Permute { patterns, meta });
        }
        let symbol = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(MatchRecognizePattern::Symbol { symbol, meta })
    }

    /// The optional postfix quantifier on a pattern factor: `*`, `+`, or a `{…}` bound.
    /// A `{-` in this position is a following exclusion factor, not a bound, so it
    /// leaves the quantifier absent.
    fn parse_row_pattern_quantifier(&mut self) -> ParseResult<Option<RepetitionQuantifier>> {
        if self.eat_op(Operator::Star)? {
            return Ok(Some(RepetitionQuantifier::ZeroOrMore));
        }
        if self.eat_op(Operator::Plus)? {
            return Ok(Some(RepetitionQuantifier::OneOrMore));
        }
        if self.peek_is_punct(Punctuation::LBrace)? && !self.peek_nth_is_op(1, Operator::Minus)? {
            return Ok(Some(self.parse_row_pattern_bound()?));
        }
        Ok(None)
    }

    /// A `{…}` repetition bound: `{n}`, `{n,}`, `{,m}`, or `{n,m}`.
    fn parse_row_pattern_bound(&mut self) -> ParseResult<RepetitionQuantifier> {
        self.expect_punct(Punctuation::LBrace, "`{` to open a pattern quantifier")?;
        let low = self.try_parse_pattern_count()?;
        let quantifier = if self.eat_punct(Punctuation::Comma)? {
            let high = self.try_parse_pattern_count()?;
            match (low, high) {
                (Some(n), Some(m)) => RepetitionQuantifier::Range(n, m),
                (Some(n), None) => RepetitionQuantifier::AtLeast(n),
                (None, Some(m)) => RepetitionQuantifier::AtMost(m),
                (None, None) => return Err(self.unexpected("a repetition bound")),
            }
        } else {
            match low {
                Some(n) => RepetitionQuantifier::Exactly(n),
                None => return Err(self.unexpected("a repetition count")),
            }
        };
        self.expect_punct(Punctuation::RBrace, "`}` to close a pattern quantifier")?;
        Ok(quantifier)
    }

    /// Read an optional integer bound at the cursor — a `Number` token parsed as `u32`,
    /// or `None` (cursor unmoved) when the next token is not a number.
    fn try_parse_pattern_count(&mut self) -> ParseResult<Option<u32>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        if token.kind != TokenKind::Number {
            return Ok(None);
        }
        let span = self.advance_span()?;
        match self.span_text(span).parse::<u32>() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err(self.unexpected("an integer repetition bound")),
        }
    }

    /// Whether the cursor sits at a row-pattern sequence terminator — the closers that
    /// end a concatenation or an alternation branch: `|`, `)`, `,` (a PERMUTE
    /// separator), `-}` (an exclusion close), or end of input.
    fn peek_ends_row_pattern_sequence(&mut self) -> ParseResult<bool> {
        if self.peek()?.is_none() {
            return Ok(true);
        }
        if self.peek_is_op(Operator::Pipe)?
            || self.peek_is_punct(Punctuation::RParen)?
            || self.peek_is_punct(Punctuation::Comma)?
        {
            return Ok(true);
        }
        // `-}` exclusion close.
        Ok(self.peek_is_op(Operator::Minus)? && self.peek_nth_is_punct(1, Punctuation::RBrace)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AfterMatchSkip, EmptyMatchesMode, MatchRecognize, MatchRecognizePattern,
        RepetitionQuantifier, Resolver as _, RowsPerMatch, SetExpr, Statement, TableFactor,
    };
    use crate::dialect::{Lenient, Postgres, Snowflake};
    use crate::error::ParseErrorKind;
    use crate::parser::{Parsed, parse_with, parse_with_options};

    use super::super::ParseOptions;

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

    /// The MATCH_RECOGNIZE core of a single-query statement's first FROM factor.
    fn factor_match_recognize(parsed: &Parsed) -> &MatchRecognize<crate::ast::NoExt> {
        let TableFactor::MatchRecognize {
            match_recognize, ..
        } = relation_of(parsed)
        else {
            panic!("expected a MATCH_RECOGNIZE table factor");
        };
        match_recognize
    }

    /// Round-trip `sql` under Lenient (the render target that shares the gate): parse and
    /// render must reproduce it byte-for-byte.
    fn round_trips_under_lenient(sql: &str) {
        let parsed = parse_with(sql, Lenient).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let rendered = crate::render::Renderer::new(Lenient)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
        assert_eq!(rendered, sql, "round-trip");
    }

    #[test]
    fn snowflake_full_match_recognize_captures_every_subclause() {
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (\
             PARTITION BY a, b ORDER BY ts \
             MEASURES x AS mx, sum(y) AS sy \
             ALL ROWS PER MATCH AFTER MATCH SKIP TO NEXT ROW \
             PATTERN (^ A B+ C* D{2,3}) \
             SUBSET U = (A, B) \
             DEFINE A AS a > 0, B AS b < 0)",
            Snowflake,
        )
        .expect("the full MATCH_RECOGNIZE parses under Snowflake");
        let mr = factor_match_recognize(&parsed);
        assert!(matches!(*mr.source, TableFactor::Table { .. }));
        assert_eq!(mr.partition_by.len(), 2, "PARTITION BY keys");
        assert_eq!(mr.order_by.len(), 1, "ORDER BY keys");
        assert_eq!(mr.measures.len(), 2, "MEASURES items");
        assert_eq!(
            parsed.resolver().resolve(mr.measures[0].alias.sym),
            "mx",
            "measure alias",
        );
        assert_eq!(
            mr.rows_per_match,
            Some(RowsPerMatch::AllRows(None)),
            "ALL ROWS PER MATCH with no empty-match mode",
        );
        assert!(matches!(
            mr.after_match_skip,
            Some(AfterMatchSkip::ToNextRow { .. }),
        ));
        assert_eq!(mr.subsets.len(), 1, "SUBSET definitions");
        assert_eq!(mr.subsets[0].members.len(), 2, "SUBSET members");
        assert_eq!(mr.define.len(), 2, "DEFINE clauses");
        // `^ A B+ C* D{2,3}` is a five-element concatenation.
        let MatchRecognizePattern::Concat { patterns, .. } = &mr.pattern else {
            panic!("expected a concatenation pattern");
        };
        assert_eq!(patterns.len(), 5);
        assert!(matches!(patterns[0], MatchRecognizePattern::Start { .. }));
        assert!(matches!(patterns[1], MatchRecognizePattern::Symbol { .. }));
        assert!(matches!(
            patterns[2],
            MatchRecognizePattern::Repetition {
                quantifier: RepetitionQuantifier::OneOrMore,
                ..
            },
        ));
        assert!(matches!(
            patterns[4],
            MatchRecognizePattern::Repetition {
                quantifier: RepetitionQuantifier::Range(2, 3),
                ..
            },
        ));
    }

    #[test]
    fn snowflake_match_recognize_reachable_on_a_bare_factor() {
        // `bigquery-snowflake-pivot-keyword-reservation`: Snowflake position-reserves
        // `MATCH_RECOGNIZE` on the `ColId` axis, so the documented bare-factor spelling (no
        // alias between the table and the operator) reaches it directly.
        let parsed = parse_with(
            "SELECT * FROM t MATCH_RECOGNIZE (PATTERN (A) DEFINE A AS a > 0)",
            Snowflake,
        )
        .expect("Snowflake reaches the bare-factor MATCH_RECOGNIZE via the ColId reservation");
        assert_eq!(factor_match_recognize(&parsed).define.len(), 1);
        // The cost mirrors PIVOT's: under Snowflake an unquoted `match_recognize` is not a
        // table alias (bare or explicit-`AS`), while Lenient keeps it a plain identifier.
        assert!(
            parse_with("SELECT * FROM t AS match_recognize", Snowflake).is_err(),
            "Snowflake rejects a table alias named match_recognize",
        );
        parse_with("SELECT * FROM t AS match_recognize", Lenient)
            .expect("Lenient keeps match_recognize a plain identifier alias");
    }

    #[test]
    fn minimal_match_recognize_needs_only_a_pattern() {
        // Every clause but PATTERN is optional; a bare `PATTERN (A)` is a single symbol.
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A))",
            Snowflake,
        )
        .expect("the minimal form parses");
        let mr = factor_match_recognize(&parsed);
        assert!(mr.partition_by.is_empty());
        assert!(mr.order_by.is_empty());
        assert!(mr.measures.is_empty());
        assert!(mr.rows_per_match.is_none());
        assert!(mr.after_match_skip.is_none());
        assert!(mr.subsets.is_empty());
        assert!(mr.define.is_empty());
        assert!(matches!(mr.pattern, MatchRecognizePattern::Symbol { .. }));
    }

    #[test]
    fn match_recognize_takes_a_trailing_alias() {
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A)) AS mr",
            Snowflake,
        )
        .expect("the trailing alias parses");
        let TableFactor::MatchRecognize { alias, .. } = relation_of(&parsed) else {
            panic!("expected a MATCH_RECOGNIZE factor");
        };
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias").name.sym),
            "mr",
        );
    }

    #[test]
    fn rows_per_match_and_empty_modes_parse() {
        for (sql, expected) in [
            ("ONE ROW PER MATCH", RowsPerMatch::OneRow),
            ("ALL ROWS PER MATCH", RowsPerMatch::AllRows(None)),
            (
                "ALL ROWS PER MATCH SHOW EMPTY MATCHES",
                RowsPerMatch::AllRows(Some(EmptyMatchesMode::Show)),
            ),
            (
                "ALL ROWS PER MATCH OMIT EMPTY MATCHES",
                RowsPerMatch::AllRows(Some(EmptyMatchesMode::Omit)),
            ),
            (
                "ALL ROWS PER MATCH WITH UNMATCHED ROWS",
                RowsPerMatch::AllRows(Some(EmptyMatchesMode::WithUnmatched)),
            ),
        ] {
            let sql = format!("SELECT * FROM t AS m MATCH_RECOGNIZE ({sql} PATTERN (A))");
            let parsed =
                parse_with(&sql, Snowflake).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(
                factor_match_recognize(&parsed).rows_per_match,
                Some(expected)
            );
        }
    }

    #[test]
    fn after_match_skip_forms_parse() {
        let cases = [
            "AFTER MATCH SKIP PAST LAST ROW",
            "AFTER MATCH SKIP TO NEXT ROW",
            "AFTER MATCH SKIP TO FIRST A",
            "AFTER MATCH SKIP TO LAST A",
        ];
        for skip in cases {
            let sql = format!("SELECT * FROM t AS m MATCH_RECOGNIZE ({skip} PATTERN (A))");
            let parsed =
                parse_with(&sql, Snowflake).unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                factor_match_recognize(&parsed).after_match_skip.is_some(),
                "{sql:?}",
            );
        }
    }

    #[test]
    fn pattern_operators_shape_the_tree() {
        // Alternation is looser than concatenation: `A B | C` is `(A B) | C`.
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A B | C))",
            Snowflake,
        )
        .expect("alternation parses");
        let MatchRecognizePattern::Alternation { patterns, .. } =
            &factor_match_recognize(&parsed).pattern
        else {
            panic!("expected an alternation at the root");
        };
        assert_eq!(patterns.len(), 2);
        assert!(matches!(patterns[0], MatchRecognizePattern::Concat { .. }));
        assert!(matches!(patterns[1], MatchRecognizePattern::Symbol { .. }));

        // Grouping, exclusion, and permutation.
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN ((A | B) {- C -} PERMUTE(D, E)))",
            Snowflake,
        )
        .expect("group / exclusion / permute parse");
        let MatchRecognizePattern::Concat { patterns, .. } =
            &factor_match_recognize(&parsed).pattern
        else {
            panic!("expected a concatenation");
        };
        assert!(matches!(patterns[0], MatchRecognizePattern::Group { .. }));
        assert!(matches!(patterns[1], MatchRecognizePattern::Exclude { .. }));
        let MatchRecognizePattern::Permute {
            patterns: permuted, ..
        } = &patterns[2]
        else {
            panic!("expected a PERMUTE");
        };
        assert_eq!(permuted.len(), 2);
    }

    #[test]
    fn quantifier_bounds_parse_every_form() {
        let parsed = parse_with(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A* B+ C{2} D{3,} E{,4} F{2,5}))",
            Snowflake,
        )
        .expect("every quantifier form parses");
        let MatchRecognizePattern::Concat { patterns, .. } =
            &factor_match_recognize(&parsed).pattern
        else {
            panic!("expected a concatenation");
        };
        let quantifiers: Vec<_> = patterns
            .iter()
            .map(|pattern| match pattern {
                MatchRecognizePattern::Repetition { quantifier, .. } => *quantifier,
                other => panic!("expected a quantified pattern, got {other:?}"),
            })
            .collect();
        assert_eq!(
            quantifiers,
            vec![
                RepetitionQuantifier::ZeroOrMore,
                RepetitionQuantifier::OneOrMore,
                RepetitionQuantifier::Exactly(2),
                RepetitionQuantifier::AtLeast(3),
                RepetitionQuantifier::AtMost(4),
                RepetitionQuantifier::Range(2, 5),
            ],
        );
    }

    #[test]
    fn match_recognize_forms_round_trip_under_lenient() {
        for sql in [
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A)) AS mr",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PARTITION BY a ORDER BY ts PATTERN (A B+))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (MEASURES x AS mx ONE ROW PER MATCH PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (ALL ROWS PER MATCH SHOW EMPTY MATCHES PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (ALL ROWS PER MATCH WITH UNMATCHED ROWS PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (AFTER MATCH SKIP PAST LAST ROW PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (AFTER MATCH SKIP TO FIRST A PATTERN (A))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (^ A B+ C* D{2,3}))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN ((A | B) {- C -} PERMUTE(D, E)))",
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A) SUBSET U = (A, B) DEFINE A AS a > 0)",
        ] {
            round_trips_under_lenient(sql);
        }
    }

    #[test]
    fn match_recognize_is_rejected_off_the_gate() {
        // PostgreSQL has no MATCH_RECOGNIZE table factor, so the reserved keyword after a
        // relation is a clean parse divergence rather than lost fields.
        assert!(
            parse_with(
                "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A))",
                Postgres,
            )
            .is_err(),
            "PostgreSQL rejects the MATCH_RECOGNIZE table factor",
        );
    }

    #[test]
    fn deeply_nested_pattern_groups_reject_cleanly() {
        // The row-pattern parser is recursion-guarded: a pathologically deep group nest
        // fails with the clean recursion error rather than overflowing the stack. A
        // shallow nest parses under the default limit.
        let deep = format!(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN ({}A{}))",
            "(".repeat(64),
            ")".repeat(64),
        );
        let options = ParseOptions::default().with_recursion_limit(16);
        let err = parse_with_options(&deep, Snowflake, options)
            .expect_err("a 64-deep pattern nest must reject past a limit of 16");
        assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);

        let shallow = format!(
            "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN ({}A{}))",
            "(".repeat(4),
            ")".repeat(4),
        );
        parse_with(&shallow, Snowflake).expect("a shallow pattern nest parses under the default");
    }
}
