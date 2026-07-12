// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! MySQL diagnostics-area statements: `SIGNAL`, `RESIGNAL`, and `GET DIAGNOSTICS`.
//!
//! These three attach to MySQL's top-level `simple_statement` production and are dispatched
//! from [`parse_statement`](super::engine::Parser::parse_statement) behind the
//! [`UtilitySyntax::signal_diagnostics`](crate::ast::dialect::UtilitySyntax) gate. They also
//! appear in stored-program bodies, which the body dispatcher reaches through the same
//! top-level dispatcher (its fall-through for non-compound keywords), so this one grammar
//! serves both surfaces.
//!
//! # The top-level vs body admission boundary
//!
//! MySQL recognizes all three at top level (measured `1295`/`ER_UNSUPPORTED_PS` over the
//! PREPARE oracle — grammar-valid, merely not preparable). The only difference between top
//! level and a stored-program body is *semantic*, not grammatical: a `SIGNAL <name>`
//! condition-name and a bare-identifier `GET DIAGNOSTICS` target both require a stored-program
//! parsing context to resolve (`1319`/`1327` outside one), whereas a `SQLSTATE '…'` condition
//! and a `@user` target are unrestricted. Those are name-resolution rejects (not `1064`
//! syntax rejects), so — consistent with this parser deferring stored-program name resolution
//! — the grammar is accepted uniformly and the sp-context restriction is left to a resolver.
//! A bare error code *is* a syntax error for `SIGNAL` (`SIGNAL 1051` → `1064`), so the
//! condition parser admits only `SQLSTATE` and a condition name, never an error code.

use crate::ast::{
    ConditionInfoItem, ConditionInfoItemName, ConditionValue, DiagnosticsArea, DiagnosticsInfo,
    Expr, GetDiagnosticsStatement, Meta, ObjectName, SignalItem, SignalItemName, SignalStatement,
    Span, Statement, StatementInfoItem, StatementInfoItemName,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;

impl<'a, D: Dialect> Parser<'a, D> {
    /// True if the cursor sits at a `GET [CURRENT | STACKED] DIAGNOSTICS` statement: the
    /// current token is `GET` and the next opens the `which_area`/`DIAGNOSTICS` prefix. The
    /// two-word lookahead keeps the leading `GET` from stealing an unrelated `GET <ident>`.
    pub(super) fn peek_starts_get_diagnostics(&mut self) -> ParseResult<bool> {
        Ok(self.peek_nth_is_contextual_keyword(1, "DIAGNOSTICS")?
            || self.peek_nth_is_contextual_keyword(1, "CURRENT")?
            || self.peek_nth_is_contextual_keyword(1, "STACKED")?)
    }

    /// Parse `SIGNAL {SQLSTATE [VALUE] '…' | <condition-name>} [SET <item> = <expr> [, …]]`.
    /// The condition is mandatory (grammar `signal_value`); a bare `SIGNAL` rejects.
    pub(super) fn parse_signal_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SIGNAL")?;
        let condition = Some(self.parse_signal_condition()?);
        let set_items = self.parse_signal_set_items()?;
        self.finish_signal(start, condition, set_items, false)
    }

    /// Parse `RESIGNAL [{SQLSTATE [VALUE] '…' | <condition-name>}] [SET <item> = <expr> [, …]]`.
    /// Every part is optional (grammar `opt_signal_value opt_set_signal_information`): a bare
    /// `RESIGNAL` re-raises the current condition, and `RESIGNAL SET …` amends it in place.
    pub(super) fn parse_resignal_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("RESIGNAL")?;
        // The condition is absent when the next token opens the `SET` clause or ends the
        // statement; otherwise it is a `SQLSTATE`/condition-name value.
        let condition = if self.at_statement_end()? || self.peek_is_contextual_keyword("SET")? {
            None
        } else {
            Some(self.parse_signal_condition()?)
        };
        let set_items = self.parse_signal_set_items()?;
        self.finish_signal(start, condition, set_items, true)
    }

    /// Assemble the shared `SIGNAL`/`RESIGNAL` payload into its [`Statement`] variant.
    fn finish_signal(
        &mut self,
        start: Span,
        condition: Option<ConditionValue>,
        set_items: ThinVec<SignalItem<D::Ext>>,
        resignal: bool,
    ) -> ParseResult<Statement<D::Ext>> {
        let (inner_meta, meta) = self.signal_meta_pair(start);
        let payload = Box::new(SignalStatement {
            condition,
            set_items,
            meta: inner_meta,
        });
        Ok(if resignal {
            Statement::Resignal {
                resignal: payload,
                meta,
            }
        } else {
            Statement::Signal {
                signal: payload,
                meta,
            }
        })
    }

    /// Parse a `SIGNAL`/`RESIGNAL` condition value: `SQLSTATE [VALUE] '…'` or a declared
    /// condition name. A bare error code is *not* accepted (`SIGNAL 1051` → `1064`), so a
    /// number in this position rejects through [`parse_ident`](Self::parse_ident).
    fn parse_signal_condition(&mut self) -> ParseResult<ConditionValue> {
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
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConditionValue::ConditionName { name, meta })
        }
    }

    /// Parse the optional `SET <item> = <expr> [, …]` amendment list; empty when absent.
    fn parse_signal_set_items(&mut self) -> ParseResult<ThinVec<SignalItem<D::Ext>>> {
        if self.eat_contextual_keyword("SET")? {
            self.parse_comma_separated(Self::parse_signal_item)
        } else {
            Ok(ThinVec::new())
        }
    }

    /// Parse one `<name> = <expr>` `SIGNAL` `SET` item.
    fn parse_signal_item(&mut self) -> ParseResult<SignalItem<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_signal_item_name()?;
        self.expect_op(Operator::Eq, "`=` in a SIGNAL SET item")?;
        let value = self.parse_expr()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SignalItem { name, value, meta })
    }

    /// Parse a `SIGNAL`-settable condition-information item name (the 12-name subset that
    /// excludes the read-only `RETURNED_SQLSTATE`).
    fn parse_signal_item_name(&mut self) -> ParseResult<SignalItemName> {
        let name = if self.eat_contextual_keyword("CLASS_ORIGIN")? {
            SignalItemName::ClassOrigin
        } else if self.eat_contextual_keyword("SUBCLASS_ORIGIN")? {
            SignalItemName::SubclassOrigin
        } else if self.eat_contextual_keyword("CONSTRAINT_CATALOG")? {
            SignalItemName::ConstraintCatalog
        } else if self.eat_contextual_keyword("CONSTRAINT_SCHEMA")? {
            SignalItemName::ConstraintSchema
        } else if self.eat_contextual_keyword("CONSTRAINT_NAME")? {
            SignalItemName::ConstraintName
        } else if self.eat_contextual_keyword("CATALOG_NAME")? {
            SignalItemName::CatalogName
        } else if self.eat_contextual_keyword("SCHEMA_NAME")? {
            SignalItemName::SchemaName
        } else if self.eat_contextual_keyword("TABLE_NAME")? {
            SignalItemName::TableName
        } else if self.eat_contextual_keyword("COLUMN_NAME")? {
            SignalItemName::ColumnName
        } else if self.eat_contextual_keyword("CURSOR_NAME")? {
            SignalItemName::CursorName
        } else if self.eat_contextual_keyword("MESSAGE_TEXT")? {
            SignalItemName::MessageText
        } else if self.eat_contextual_keyword("MYSQL_ERRNO")? {
            SignalItemName::MysqlErrno
        } else {
            return Err(self.unexpected("a SIGNAL condition-information item name"));
        };
        Ok(name)
    }

    /// Parse `GET [CURRENT | STACKED] DIAGNOSTICS <diagnostics-information>`.
    pub(super) fn parse_get_diagnostics_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("GET")?;
        let area = if self.eat_contextual_keyword("CURRENT")? {
            DiagnosticsArea::Current
        } else if self.eat_contextual_keyword("STACKED")? {
            DiagnosticsArea::Stacked
        } else {
            DiagnosticsArea::Implicit
        };
        self.expect_contextual_keyword("DIAGNOSTICS")?;
        let info = self.parse_diagnostics_info()?;
        let (inner_meta, meta) = self.signal_meta_pair(start);
        Ok(Statement::GetDiagnostics {
            get_diagnostics: Box::new(GetDiagnosticsStatement {
                area,
                info,
                meta: inner_meta,
            }),
            meta,
        })
    }

    /// Parse the `diagnostics_information`: statement-level items, or a `CONDITION <number>`
    /// selector plus condition-level items.
    fn parse_diagnostics_info(&mut self) -> ParseResult<DiagnosticsInfo<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("CONDITION")? {
            let number = self.parse_expr()?;
            let items = self.parse_comma_separated(Self::parse_condition_info_item)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(DiagnosticsInfo::Condition {
                number: Box::new(number),
                items,
                meta,
            })
        } else {
            let items = self.parse_comma_separated(Self::parse_statement_info_item)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(DiagnosticsInfo::Statement { items, meta })
        }
    }

    /// Parse one `<target> = {NUMBER | ROW_COUNT}` statement-information item.
    fn parse_statement_info_item(&mut self) -> ParseResult<StatementInfoItem<D::Ext>> {
        let start = self.current_span()?;
        let target = self.parse_diagnostics_target()?;
        self.expect_op(Operator::Eq, "`=` in a GET DIAGNOSTICS item")?;
        let name = if self.eat_contextual_keyword("NUMBER")? {
            StatementInfoItemName::Number
        } else if self.eat_contextual_keyword("ROW_COUNT")? {
            StatementInfoItemName::RowCount
        } else {
            return Err(self.unexpected("`NUMBER` or `ROW_COUNT`"));
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(StatementInfoItem { target, name, meta })
    }

    /// Parse one `<target> = <condition-item-name>` condition-information item.
    fn parse_condition_info_item(&mut self) -> ParseResult<ConditionInfoItem<D::Ext>> {
        let start = self.current_span()?;
        let target = self.parse_diagnostics_target()?;
        self.expect_op(Operator::Eq, "`=` in a GET DIAGNOSTICS CONDITION item")?;
        let name = self.parse_condition_info_item_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ConditionInfoItem { target, name, meta })
    }

    /// Parse a readable condition-information item name (the 12 signal-settable names plus the
    /// read-only `RETURNED_SQLSTATE`).
    fn parse_condition_info_item_name(&mut self) -> ParseResult<ConditionInfoItemName> {
        let name = if self.eat_contextual_keyword("CLASS_ORIGIN")? {
            ConditionInfoItemName::ClassOrigin
        } else if self.eat_contextual_keyword("SUBCLASS_ORIGIN")? {
            ConditionInfoItemName::SubclassOrigin
        } else if self.eat_contextual_keyword("CONSTRAINT_CATALOG")? {
            ConditionInfoItemName::ConstraintCatalog
        } else if self.eat_contextual_keyword("CONSTRAINT_SCHEMA")? {
            ConditionInfoItemName::ConstraintSchema
        } else if self.eat_contextual_keyword("CONSTRAINT_NAME")? {
            ConditionInfoItemName::ConstraintName
        } else if self.eat_contextual_keyword("CATALOG_NAME")? {
            ConditionInfoItemName::CatalogName
        } else if self.eat_contextual_keyword("SCHEMA_NAME")? {
            ConditionInfoItemName::SchemaName
        } else if self.eat_contextual_keyword("TABLE_NAME")? {
            ConditionInfoItemName::TableName
        } else if self.eat_contextual_keyword("COLUMN_NAME")? {
            ConditionInfoItemName::ColumnName
        } else if self.eat_contextual_keyword("CURSOR_NAME")? {
            ConditionInfoItemName::CursorName
        } else if self.eat_contextual_keyword("MESSAGE_TEXT")? {
            ConditionInfoItemName::MessageText
        } else if self.eat_contextual_keyword("MYSQL_ERRNO")? {
            ConditionInfoItemName::MysqlErrno
        } else if self.eat_contextual_keyword("RETURNED_SQLSTATE")? {
            ConditionInfoItemName::ReturnedSqlstate
        } else {
            return Err(self.unexpected("a GET DIAGNOSTICS condition-information item name"));
        };
        Ok(name)
    }

    /// Parse a `GET DIAGNOSTICS` assignment target: a `@user` variable or a bare local-variable
    /// name (grammar `simple_target_specification`). Stops before the `=`, so the value side is
    /// parsed separately.
    fn parse_diagnostics_target(&mut self) -> ParseResult<Expr<D::Ext>> {
        let token = match self.peek()? {
            Some(token) => token,
            None => return Err(self.unexpected("a GET DIAGNOSTICS target variable")),
        };
        if token.kind == TokenKind::Variable {
            self.parse_session_variable(token)
        } else {
            let start = self.current_span()?;
            let ident = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Expr::Column {
                name: ObjectName(thin_vec![ident]),
                meta,
            })
        }
    }

    /// True when the cursor is at the end of a statement — end of input or a `;` terminator —
    /// so an optional `RESIGNAL` clause can tell "absent" from "present".
    fn at_statement_end(&mut self) -> ParseResult<bool> {
        Ok(self.peek()?.is_none() || self.peek_is_punct(Punctuation::Semicolon)?)
    }

    /// A pair of fresh [`Meta`] over `start..preceding` — one for the boxed payload and one for
    /// its wrapping [`Statement`] variant, each a distinct node id (the boxed-statement idiom).
    fn signal_meta_pair(&mut self, start: Span) -> (Meta, Meta) {
        let span = start.union(self.preceding_span());
        (self.make_meta(span), self.make_meta(span))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::FeatureSet;
    use crate::ast::{
        ConditionInfoItemName, ConditionValue, DiagnosticsArea, DiagnosticsInfo,
        GetDiagnosticsStatement, SignalItemName, SignalStatement, Statement, StatementInfoItemName,
    };
    use crate::error::ParseErrorKind;
    use crate::parser::{FeatureDialect, Parser};
    use crate::render::Renderer;
    use crate::tokenizer::tokenize_with;

    /// The MySQL feature preset (`signal_diagnostics` on) as a data-only test dialect.
    const MYSQL: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };
    /// ANSI (`signal_diagnostics` off) — the gate-off reject dialect.
    const ANSI: FeatureDialect = FeatureDialect {
        features: &FeatureSet::ANSI,
    };

    /// Parse one top-level statement under the MySQL preset.
    fn parse(src: &str) -> Statement {
        let tokens = tokenize_with(src, MYSQL.features).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, MYSQL);
        parser
            .parse_statement()
            .unwrap_or_else(|err| panic!("{src}: {err:?}"))
    }

    /// Parse a fragment under `dialect`, returning the reject for the negative pins.
    fn reject(src: &str, dialect: FeatureDialect) -> ParseErrorKind {
        let tokens = tokenize_with(src, dialect.features).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, dialect);
        parser
            .parse_statement()
            .expect_err(&format!("{src} must reject"))
            .kind
    }

    /// Parse, render, and re-parse; the AST must be structurally stable across the round-trip
    /// (`Meta` is excluded from equality).
    fn assert_round_trips(src: &str) {
        let tokens = tokenize_with(src, MYSQL.features).expect("the fragment lexes");
        let mut parser = Parser::new(src, &tokens, MYSQL);
        let first = parser
            .parse_statement()
            .unwrap_or_else(|err| panic!("{src}: {err:?}"));
        let resolver = parser.finish();
        let rendered = Renderer::new(MYSQL)
            .render_statement(&first, &resolver, src)
            .unwrap_or_else(|err| panic!("{src}: render {err}"));
        let second = parse(&rendered);
        assert_eq!(
            first, second,
            "round-trip changed the AST\n  in:  {src}\n  out: {rendered}"
        );
    }

    #[test]
    fn signal_sqlstate_forms_parse() {
        let Statement::Signal { signal, .. } = parse("SIGNAL SQLSTATE '45000'") else {
            panic!("expected a SIGNAL");
        };
        assert!(matches!(
            signal.condition,
            Some(ConditionValue::SqlState {
                value_keyword: false,
                ..
            })
        ));
        assert!(signal.set_items.is_empty());

        let Statement::Signal { signal, .. } = parse("SIGNAL SQLSTATE VALUE '45000'") else {
            panic!("expected a SIGNAL");
        };
        assert!(matches!(
            signal.condition,
            Some(ConditionValue::SqlState {
                value_keyword: true,
                ..
            })
        ));
    }

    #[test]
    fn signal_condition_name_parses() {
        // The condition-name form is grammar-valid; the sp-context restriction is semantic.
        let Statement::Signal { signal, .. } = parse("SIGNAL myc") else {
            panic!("expected a SIGNAL");
        };
        assert!(matches!(
            signal.condition,
            Some(ConditionValue::ConditionName { .. })
        ));
    }

    #[test]
    fn signal_set_items_parse() {
        let Statement::Signal { signal, .. } =
            parse("SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'oops', MYSQL_ERRNO = 5")
        else {
            panic!("expected a SIGNAL");
        };
        assert_eq!(signal.set_items.len(), 2);
        assert_eq!(signal.set_items[0].name, SignalItemName::MessageText);
        assert_eq!(signal.set_items[1].name, SignalItemName::MysqlErrno);
    }

    #[test]
    fn resignal_all_parts_optional() {
        // Bare RESIGNAL: no condition, no SET.
        let Statement::Resignal { resignal, .. } = parse("RESIGNAL") else {
            panic!("expected a RESIGNAL");
        };
        let SignalStatement {
            condition,
            set_items,
            ..
        } = *resignal;
        assert!(condition.is_none() && set_items.is_empty());

        // RESIGNAL SET (no condition).
        let Statement::Resignal { resignal, .. } = parse("RESIGNAL SET MESSAGE_TEXT = 'x'") else {
            panic!("expected a RESIGNAL");
        };
        assert!(resignal.condition.is_none());
        assert_eq!(resignal.set_items.len(), 1);

        // RESIGNAL with a condition.
        let Statement::Resignal { resignal, .. } = parse("RESIGNAL SQLSTATE '45000'") else {
            panic!("expected a RESIGNAL");
        };
        assert!(resignal.condition.is_some());
    }

    #[test]
    fn get_diagnostics_statement_forms_parse() {
        let Statement::GetDiagnostics {
            get_diagnostics, ..
        } = parse("GET DIAGNOSTICS @n = NUMBER")
        else {
            panic!("expected a GET DIAGNOSTICS");
        };
        let GetDiagnosticsStatement { area, info, .. } = *get_diagnostics;
        assert_eq!(area, DiagnosticsArea::Implicit);
        let DiagnosticsInfo::Statement { items, .. } = info else {
            panic!("expected statement-info form");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, StatementInfoItemName::Number);

        assert!(matches!(
            parse("GET CURRENT DIAGNOSTICS @n = NUMBER"),
            Statement::GetDiagnostics { get_diagnostics, .. }
                if get_diagnostics.area == DiagnosticsArea::Current
        ));
        assert!(matches!(
            parse("GET STACKED DIAGNOSTICS @n = NUMBER"),
            Statement::GetDiagnostics { get_diagnostics, .. }
                if get_diagnostics.area == DiagnosticsArea::Stacked
        ));
    }

    #[test]
    fn get_diagnostics_condition_form_parses() {
        let Statement::GetDiagnostics {
            get_diagnostics, ..
        } = parse("GET DIAGNOSTICS CONDITION 1 @s = RETURNED_SQLSTATE, @m = MESSAGE_TEXT")
        else {
            panic!("expected a GET DIAGNOSTICS");
        };
        let DiagnosticsInfo::Condition { items, .. } = get_diagnostics.info else {
            panic!("expected condition-info form");
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, ConditionInfoItemName::ReturnedSqlstate);
        assert_eq!(items[1].name, ConditionInfoItemName::MessageText);
    }

    #[test]
    fn get_diagnostics_bare_ident_target_parses() {
        // A local-variable (bare ident) target — valid grammar (the sp-context restriction is
        // semantic).
        assert!(matches!(
            parse("GET DIAGNOSTICS lv = NUMBER"),
            Statement::GetDiagnostics { .. }
        ));
    }

    #[test]
    fn signal_family_appears_in_bodies() {
        // The body dispatcher reaches the family through its fall-through to `parse_statement`.
        let src = "BEGIN SIGNAL SQLSTATE '45000'; END";
        let tokens = tokenize_with(src, MYSQL.features).expect("lexes");
        let mut parser = Parser::new(src, &tokens, MYSQL);
        let Statement::Compound { compound, .. } = parser
            .parse_body_statement()
            .unwrap_or_else(|err| panic!("body: {err:?}"))
        else {
            panic!("expected a compound block");
        };
        assert!(matches!(compound.body[0], Statement::Signal { .. }));
    }

    // --- Reject pins (spike-probed server behaviour) ---------------------------

    #[test]
    fn signal_bare_error_code_is_a_syntax_error() {
        // `SIGNAL 1051` — a bare error code — is a syntax error on the server (measured 1064),
        // so the condition parser rejects a number in that position.
        assert_eq!(reject("SIGNAL 1051", MYSQL), ParseErrorKind::Syntax);
    }

    #[test]
    fn signal_requires_a_condition() {
        // `SIGNAL` alone is incomplete (the condition is mandatory).
        assert_eq!(reject("SIGNAL", MYSQL), ParseErrorKind::Syntax);
    }

    #[test]
    fn signal_family_is_gated_off_under_ansi() {
        // Without `signal_diagnostics` (ANSI) the leading keyword is not dispatched and the
        // statement rejects.
        assert_eq!(
            reject("SIGNAL SQLSTATE '45000'", ANSI),
            ParseErrorKind::Syntax
        );
        assert_eq!(reject("RESIGNAL", ANSI), ParseErrorKind::Syntax);
        // A bare-ident target so the fragment lexes under ANSI (no `@` stray byte); the reject
        // is then the gate-off parse error, not a lex error.
        assert_eq!(
            reject("GET DIAGNOSTICS lv = NUMBER", ANSI),
            ParseErrorKind::Syntax
        );
    }

    // --- Render round-trips ----------------------------------------------------

    #[test]
    fn signal_family_round_trips() {
        for src in [
            "SIGNAL SQLSTATE '45000'",
            "SIGNAL SQLSTATE VALUE '45000'",
            "SIGNAL myc",
            "SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'oops', MYSQL_ERRNO = 5",
            "RESIGNAL",
            "RESIGNAL SQLSTATE '45000'",
            "RESIGNAL SET CLASS_ORIGIN = 'x'",
            "GET DIAGNOSTICS @n = NUMBER",
            "GET CURRENT DIAGNOSTICS @n = NUMBER, @r = ROW_COUNT",
            "GET STACKED DIAGNOSTICS @n = NUMBER",
            "GET DIAGNOSTICS CONDITION 1 @s = RETURNED_SQLSTATE, @m = MESSAGE_TEXT",
            "GET DIAGNOSTICS lv = NUMBER",
        ] {
            assert_round_trips(src);
        }
    }
}
