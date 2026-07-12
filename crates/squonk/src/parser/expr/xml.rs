// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQL/XML expression functions (`pg-xml-expression-functions`).
//!
//! `xmlelement`/`xmlforest`/`xmlconcat`/`xmlparse`/`xmlpi`/`xmlroot`/`xmlserialize`/
//! `xmlexists` — PostgreSQL's `func_expr_common_subexpr` XML productions (lowered to
//! `XmlExpr`/`XmlSerialize`) — plus the `IS [NOT] DOCUMENT` predicate. Engine-verified
//! against `pg_query` (PG 17) and gated on
//! [`CallSyntax::xml_expression_functions`](crate::ast::dialect::CallSyntax). Each form
//! opens with contextual keywords (`NAME`, `DOCUMENT`/`CONTENT`, `VERSION`,
//! `STANDALONE`, `PASSING`) that stay unreserved outside these parens.
//!
//! `xmlagg` is deliberately absent: it is an ordinary PostgreSQL aggregate (not a
//! keyword special form), so it already parses through the ordinary aggregate call
//! path with its `ORDER BY`/`FILTER`/`OVER` tails.

use crate::ast::{
    Expr, Keyword, Spanned, XmlAttribute, XmlDocumentOrContent, XmlFunc, XmlIndentOption,
    XmlPassingMechanism, XmlStandalone, XmlWhitespaceOption,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::Parser;
use crate::tokenizer::{Punctuation, Token};
use thin_vec::ThinVec;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Dispatch a SQL/XML expression function on its keyword head. The caller confirmed
    /// the gate is on and that `(` follows, so the special form is unambiguous (a bare
    /// head with no `(` never reaches here — it falls through to the ordinary name path).
    pub(super) fn parse_xml_expr(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let crate::tokenizer::TokenKind::Keyword(keyword) = token.kind else {
            unreachable!("parse_xml_expr is reached only with a keyword token");
        };
        let xml_func = match keyword {
            Keyword::Xmlelement => self.parse_xml_element(token)?,
            Keyword::Xmlforest => self.parse_xml_forest(token)?,
            Keyword::Xmlconcat => self.parse_xml_concat(token)?,
            Keyword::Xmlparse => self.parse_xml_parse(token)?,
            Keyword::Xmlpi => self.parse_xml_pi(token)?,
            Keyword::Xmlroot => self.parse_xml_root(token)?,
            Keyword::Xmlserialize => self.parse_xml_serialize(token)?,
            Keyword::Xmlexists => self.parse_xml_exists(token)?,
            _ => unreachable!("parse_xml_expr dispatched a non-XML keyword"),
        };
        let span = token.span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::XmlFunc {
            xml_func: Box::new(xml_func),
            meta,
        })
    }

    /// `xmlelement(NAME <name> [, xmlattributes(<attr>, …)] [, <content>, …])`.
    fn parse_xml_element(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLELEMENT
        self.expect_punct(Punctuation::LParen, "`(` after `xmlelement`")?;
        self.expect_keyword(Keyword::Name)?;
        let name = self.parse_as_alias_ident()?;
        let mut attributes = ThinVec::new();
        let mut content = ThinVec::new();
        if self.eat_punct(Punctuation::Comma)? {
            // After the name's comma, the `xmlattributes(…)` clause is optional and, when
            // present, must come first (PostgreSQL rejects content before it). Its head is
            // the `XMLATTRIBUTES` keyword immediately followed by `(`; anything else is the
            // start of the content expression list.
            if self.peek_is_keyword(Keyword::Xmlattributes)?
                && self.peek_nth_is_punct(1, Punctuation::LParen)?
            {
                self.advance()?; // XMLATTRIBUTES
                self.expect_punct(Punctuation::LParen, "`(` after `xmlattributes`")?;
                attributes = self.parse_comma_separated(Self::parse_xml_attribute)?;
                self.expect_punct(Punctuation::RParen, "`)` to close `xmlattributes`")?;
                if self.eat_punct(Punctuation::Comma)? {
                    content = self.parse_comma_separated(Self::parse_expr)?;
                }
            } else {
                content = self.parse_comma_separated(Self::parse_expr)?;
            }
        }
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlelement`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Element {
            name,
            attributes,
            content,
            meta,
        })
    }

    /// `xmlforest(<value> [AS <name>], …)` — a non-empty element list.
    fn parse_xml_forest(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLFOREST
        self.expect_punct(Punctuation::LParen, "`(` after `xmlforest`")?;
        let elements = self.parse_comma_separated(Self::parse_xml_attribute)?;
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlforest`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Forest { elements, meta })
    }

    /// `xmlconcat(<value>, …)` — a non-empty ordinary expression list.
    fn parse_xml_concat(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLCONCAT
        self.expect_punct(Punctuation::LParen, "`(` after `xmlconcat`")?;
        let args = self.parse_comma_separated(Self::parse_expr)?;
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlconcat`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Concat { args, meta })
    }

    /// `xmlparse({DOCUMENT | CONTENT} <value> [{PRESERVE | STRIP} WHITESPACE])`.
    fn parse_xml_parse(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLPARSE
        self.expect_punct(Punctuation::LParen, "`(` after `xmlparse`")?;
        let option = self.parse_xml_document_or_content()?;
        let arg = self.parse_expr()?;
        let whitespace = if self.eat_keyword(Keyword::Preserve)? {
            self.expect_keyword(Keyword::Whitespace)?;
            XmlWhitespaceOption::Preserve
        } else if self.eat_keyword(Keyword::Strip)? {
            self.expect_keyword(Keyword::Whitespace)?;
            XmlWhitespaceOption::Strip
        } else {
            XmlWhitespaceOption::Unspecified
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlparse`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Parse {
            option,
            arg: Box::new(arg),
            whitespace,
            meta,
        })
    }

    /// `xmlpi(NAME <name> [, <content>])` — a single optional content expression.
    fn parse_xml_pi(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLPI
        self.expect_punct(Punctuation::LParen, "`(` after `xmlpi`")?;
        self.expect_keyword(Keyword::Name)?;
        let name = self.parse_as_alias_ident()?;
        let content = if self.eat_punct(Punctuation::Comma)? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlpi`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Pi {
            name,
            content,
            meta,
        })
    }

    /// `xmlroot(<value>, VERSION {<expr> | NO VALUE} [, STANDALONE {YES | NO | NO VALUE}])`.
    fn parse_xml_root(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLROOT
        self.expect_punct(Punctuation::LParen, "`(` after `xmlroot`")?;
        let arg = self.parse_expr()?;
        self.expect_punct(
            Punctuation::Comma,
            "`,` before the `xmlroot` VERSION clause",
        )?;
        self.expect_keyword(Keyword::Version)?;
        // `VERSION NO VALUE` is the null version; any other operand is an expression.
        let version =
            if self.peek_is_keyword(Keyword::No)? && self.peek_nth_is_keyword(1, Keyword::Value)? {
                self.advance()?; // NO
                self.advance()?; // VALUE
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            };
        let standalone = if self.eat_punct(Punctuation::Comma)? {
            self.expect_keyword(Keyword::Standalone)?;
            if self.eat_keyword(Keyword::Yes)? {
                XmlStandalone::Yes
            } else if self.eat_keyword(Keyword::No)? {
                if self.eat_keyword(Keyword::Value)? {
                    XmlStandalone::NoValue
                } else {
                    XmlStandalone::No
                }
            } else {
                return Err(self.unexpected("`YES`, `NO`, or `NO VALUE` after `STANDALONE`"));
            }
        } else {
            XmlStandalone::Unspecified
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlroot`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Root {
            arg: Box::new(arg),
            version,
            standalone,
            meta,
        })
    }

    /// `xmlserialize({DOCUMENT | CONTENT} <value> AS <type> [[NO] INDENT])`.
    fn parse_xml_serialize(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLSERIALIZE
        self.expect_punct(Punctuation::LParen, "`(` after `xmlserialize`")?;
        let option = self.parse_xml_document_or_content()?;
        let arg = self.parse_expr()?;
        self.expect_keyword(Keyword::As)?;
        let data_type = self.parse_data_type()?;
        let indent = if self.eat_keyword(Keyword::Indent)? {
            XmlIndentOption::Indent
        } else if self.eat_keyword(Keyword::No)? {
            self.expect_keyword(Keyword::Indent)?;
            XmlIndentOption::NoIndent
        } else {
            XmlIndentOption::Unspecified
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlserialize`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Serialize {
            option,
            arg: Box::new(arg),
            data_type: Box::new(data_type),
            indent,
            meta,
        })
    }

    /// `xmlexists(<path> PASSING [BY {REF | VALUE}] <doc> [BY {REF | VALUE}])`.
    ///
    /// The path and document are `c_expr` (a primary) in PostgreSQL, not a full
    /// `a_expr`, so `xmlexists('a' || 'b' PASSING x)` rejects; both are parsed at the
    /// prefix (primary) level to match — a parenthesized operand re-admits an `a_expr`.
    fn parse_xml_exists(&mut self, token: Token) -> ParseResult<XmlFunc<D::Ext>> {
        self.advance()?; // XMLEXISTS
        self.expect_punct(Punctuation::LParen, "`(` after `xmlexists`")?;
        let path = self.parse_prefix()?.expr;
        self.expect_keyword(Keyword::Passing)?;
        let mechanism_before = self.parse_xml_passing_mechanism()?;
        let arg = self.parse_prefix()?.expr;
        let mechanism_after = self.parse_xml_passing_mechanism()?;
        self.expect_punct(Punctuation::RParen, "`)` to close `xmlexists`")?;
        let meta = self.make_meta(token.span.union(self.preceding_span()));
        Ok(XmlFunc::Exists {
            path: Box::new(path),
            mechanism_before,
            arg: Box::new(arg),
            mechanism_after,
            meta,
        })
    }

    /// Parse the `IS [NOT] DOCUMENT` predicate after the left operand and the
    /// already-consumed `IS [NOT]`.
    pub(super) fn parse_is_document_predicate(
        &mut self,
        expr: Expr<D::Ext>,
        negated: bool,
    ) -> ParseResult<Expr<D::Ext>> {
        self.expect_keyword(Keyword::Document)?;
        let span = expr.span().union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Expr::IsDocument {
            expr: Box::new(expr),
            negated,
            meta,
        })
    }

    // --- shared clause parsers -------------------------------------------------------

    /// Parse one `<value> [AS <name>]` attribute/forest element.
    fn parse_xml_attribute(&mut self) -> ParseResult<XmlAttribute<D::Ext>> {
        let start = self.current_span()?;
        let value = self.parse_expr()?;
        let name = if self.eat_keyword(Keyword::As)? {
            Some(self.parse_as_alias_ident()?)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(XmlAttribute {
            value: Box::new(value),
            name,
            meta,
        })
    }

    /// Parse the mandatory `DOCUMENT` / `CONTENT` mode word of `xmlparse`/`xmlserialize`.
    fn parse_xml_document_or_content(&mut self) -> ParseResult<XmlDocumentOrContent> {
        if self.eat_keyword(Keyword::Document)? {
            Ok(XmlDocumentOrContent::Document)
        } else if self.eat_keyword(Keyword::Content)? {
            Ok(XmlDocumentOrContent::Content)
        } else {
            Err(self.unexpected("`DOCUMENT` or `CONTENT`"))
        }
    }

    /// Parse an optional `BY {REF | VALUE}` passing mechanism.
    ///
    /// `pub(crate)` so the `XMLTABLE` table factor ([`super::super::from`]) reuses the same
    /// `PASSING [BY REF|VALUE] doc [BY REF|VALUE]` mechanism grammar.
    pub(crate) fn parse_xml_passing_mechanism(
        &mut self,
    ) -> ParseResult<Option<XmlPassingMechanism>> {
        if !self.eat_keyword(Keyword::By)? {
            return Ok(None);
        }
        if self.eat_keyword(Keyword::Ref)? {
            Ok(Some(XmlPassingMechanism::ByRef))
        } else if self.eat_keyword(Keyword::Value)? {
            Ok(Some(XmlPassingMechanism::ByValue))
        } else {
            Err(self.unexpected("`REF` or `VALUE` after `BY`"))
        }
    }
}
