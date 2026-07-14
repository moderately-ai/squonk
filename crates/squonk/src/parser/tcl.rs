// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Transaction-control statement grammar (operational family).
//!
//! Owns `BEGIN` / `START TRANSACTION`, `COMMIT`, `ROLLBACK` (with the optional
//! savepoint rewind), `SAVEPOINT`, `RELEASE SAVEPOINT`, and `SET TRANSACTION`
//! characteristics, plus the leading-token recognizer the statement dispatcher in
//! [`super::query`] consults. As in the DDL/DML families, this vocabulary is matched
//! as contextual words rather than by keyword tag — the full ANSI/PostgreSQL
//! keyword inventory is a separate ticket, and none of these words are reserved.
//! Statement availability, exact aliases, block words, mode kinds, and mode repetition
//! are independent [`UtilitySyntax`](crate::ast::dialect::UtilitySyntax) decisions.
//! SQLite's `BEGIN {DEFERRED | IMMEDIATE | EXCLUSIVE}` modifier remains a distinct
//! grammar position from the standard [`TransactionMode`] list.

use crate::ast::{
    Expr, IsolationLevel, Literal, Span, Statement, TransactionAccessMode, TransactionBlockKeyword,
    TransactionCommitKeyword, TransactionMode, TransactionModeKind, TransactionRollbackKeyword,
    TransactionStart, TransactionStatement, XaAssociation, XaStartKeyword, XaStatement, XaSuspend,
    Xid, split_radix_prefix,
};
use crate::error::ParseResult;
use crate::tokenizer::{Punctuation, TokenKind};
use thin_vec::ThinVec;

use super::Dialect;
use super::engine::Parser;
use super::expr::number_literal_kind;

fn same_transaction_mode_kind(left: &TransactionMode, right: &TransactionMode) -> bool {
    matches!(
        (left, right),
        (
            TransactionMode::IsolationLevel { .. },
            TransactionMode::IsolationLevel { .. }
        ) | (
            TransactionMode::AccessMode { .. },
            TransactionMode::AccessMode { .. }
        ) | (
            TransactionMode::Deferrable { .. },
            TransactionMode::Deferrable { .. }
        ) | (
            TransactionMode::ConsistentSnapshot { .. },
            TransactionMode::ConsistentSnapshot { .. }
        )
    )
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// True if the current token begins a transaction-control statement.
    ///
    /// `SET` is shared with the session [`SET`](super::Parser::parse_session_statement);
    /// only `SET TRANSACTION` is transaction control, so it is claimed here only
    /// when `TRANSACTION` follows. The dispatcher must therefore test this before
    /// the session recognizer.
    pub(super) fn peek_starts_transaction_statement(&mut self) -> ParseResult<bool> {
        let syntax = self.features().utility_syntax;
        Ok(self.peek_is_contextual_keyword("BEGIN")?
            || (syntax.start_transaction && self.peek_is_contextual_keyword("START")?)
            || self.peek_is_contextual_keyword("COMMIT")?
            || (syntax.end_transaction_alias && self.peek_is_contextual_keyword("END")?)
            || self.peek_is_contextual_keyword("ROLLBACK")?
            || (syntax.abort_transaction_alias && self.peek_is_contextual_keyword("ABORT")?)
            || (syntax.transaction_savepoints
                && (self.peek_is_contextual_keyword("SAVEPOINT")?
                    || self.peek_is_contextual_keyword("RELEASE")?))
            || (syntax.set_transaction
                && self.peek_is_contextual_keyword("SET")?
                && self.peek_nth_is_contextual_keyword(1, "TRANSACTION")?))
    }

    /// Parse a transaction-control statement into [`Statement::Transaction`].
    pub(super) fn parse_transaction_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let transaction = self.parse_transaction_statement_kind(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Transaction {
            transaction: Box::new(transaction),
            meta,
        })
    }

    fn parse_transaction_statement_kind(
        &mut self,
        start: Span,
    ) -> ParseResult<TransactionStatement> {
        if self.eat_contextual_keyword("BEGIN")? {
            let mode = self.parse_transaction_mode_kind()?;
            let block = self.eat_transaction_block_keyword(
                self.features().utility_syntax.begin_transaction_keyword,
            )?;
            let modes = if self.features().utility_syntax.begin_transaction_modes {
                self.parse_transaction_modes_with(
                    self.features().utility_syntax.transaction_isolation_mode,
                    self.features().utility_syntax.transaction_access_mode,
                    self.features().utility_syntax.transaction_deferrable_mode,
                    false,
                )?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Begin {
                syntax: TransactionStart::Begin,
                mode,
                block,
                modes,
                meta,
            })
        } else if self.features().utility_syntax.start_transaction
            && self.eat_contextual_keyword("START")?
        {
            let block = if self
                .features()
                .utility_syntax
                .start_transaction_block_optional
            {
                self.eat_transaction_block_keyword(true)?
            } else {
                self.expect_contextual_keyword("TRANSACTION")?;
                Some(TransactionBlockKeyword::Transaction)
            };
            let modes = self.parse_transaction_modes_with(
                self.features()
                    .utility_syntax
                    .start_transaction_isolation_mode,
                self.features().utility_syntax.transaction_access_mode,
                self.features()
                    .utility_syntax
                    .start_transaction_deferrable_mode,
                self.features()
                    .utility_syntax
                    .start_transaction_consistent_snapshot,
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Begin {
                syntax: TransactionStart::Start,
                mode: None,
                block,
                modes,
                meta,
            })
        } else if let Some(syntax) = self.eat_transaction_commit_keyword()? {
            let block = self.eat_transaction_block_keyword(
                self.features().utility_syntax.commit_transaction_keyword,
            )?;
            let (chain, release) = self.parse_transaction_completion()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Commit {
                syntax,
                block,
                chain,
                release,
                meta,
            })
        } else if let Some(syntax) = self.eat_transaction_rollback_keyword()? {
            let block = self.eat_transaction_block_keyword(
                self.features().utility_syntax.rollback_transaction_keyword,
            )?;
            let (savepoint_keyword, to_savepoint) =
                if self.features().utility_syntax.transaction_savepoints
                    && self.eat_contextual_keyword("TO")?
                {
                    // The `SAVEPOINT` keyword is optional in `ROLLBACK TO [SAVEPOINT] <name>`.
                    let savepoint_keyword = self.eat_contextual_keyword("SAVEPOINT")?;
                    (savepoint_keyword, Some(self.parse_ident()?))
                } else {
                    (false, None)
                };
            let (chain, release) = if to_savepoint.is_none() {
                self.parse_transaction_completion()?
            } else {
                (None, None)
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Rollback {
                syntax,
                block,
                savepoint_keyword,
                to_savepoint,
                chain,
                release,
                meta,
            })
        } else if self.features().utility_syntax.transaction_savepoints
            && self.eat_contextual_keyword("SAVEPOINT")?
        {
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Savepoint { name, meta })
        } else if self.features().utility_syntax.transaction_savepoints
            && self.eat_contextual_keyword("RELEASE")?
        {
            // The `SAVEPOINT` keyword is optional in `RELEASE [SAVEPOINT] <name>`.
            let savepoint_keyword = if self
                .features()
                .utility_syntax
                .release_savepoint_keyword_optional
            {
                self.eat_contextual_keyword("SAVEPOINT")?
            } else {
                self.expect_contextual_keyword("SAVEPOINT")?;
                true
            };
            let savepoint = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::Release {
                savepoint_keyword,
                savepoint,
                meta,
            })
        } else if self.features().utility_syntax.set_transaction
            && self.eat_contextual_keyword("SET")?
        {
            self.expect_contextual_keyword("TRANSACTION")?;
            let modes = self.parse_transaction_modes()?;
            if modes.is_empty() {
                return Err(self.unexpected("a transaction mode after `SET TRANSACTION`"));
            }
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TransactionStatement::SetCharacteristics { modes, meta })
        } else {
            Err(self.unexpected("a transaction-control statement"))
        }
    }

    fn parse_transaction_chain(&mut self) -> ParseResult<Option<bool>> {
        if !self.features().utility_syntax.transaction_chain
            || !self.eat_contextual_keyword("AND")?
        {
            return Ok(None);
        }
        let chain = !self.eat_contextual_keyword("NO")?;
        self.expect_contextual_keyword("CHAIN")?;
        Ok(Some(chain))
    }

    fn parse_transaction_completion(&mut self) -> ParseResult<(Option<bool>, Option<bool>)> {
        let chain = self.parse_transaction_chain()?;
        if chain.is_some() || !self.features().utility_syntax.transaction_release {
            return Ok((chain, None));
        }
        let release = if self.eat_contextual_keyword("RELEASE")? {
            Some(true)
        } else if self.peek_is_contextual_keyword("NO")?
            && self.peek_nth_is_contextual_keyword(1, "RELEASE")?
        {
            self.expect_contextual_keyword("NO")?;
            self.expect_contextual_keyword("RELEASE")?;
            Some(false)
        } else {
            None
        };
        Ok((None, release))
    }

    fn eat_transaction_commit_keyword(&mut self) -> ParseResult<Option<TransactionCommitKeyword>> {
        if self.eat_contextual_keyword("COMMIT")? {
            Ok(Some(TransactionCommitKeyword::Commit))
        } else if self.features().utility_syntax.end_transaction_alias
            && self.eat_contextual_keyword("END")?
        {
            Ok(Some(TransactionCommitKeyword::End))
        } else {
            Ok(None)
        }
    }

    fn eat_transaction_rollback_keyword(
        &mut self,
    ) -> ParseResult<Option<TransactionRollbackKeyword>> {
        if self.eat_contextual_keyword("ROLLBACK")? {
            Ok(Some(TransactionRollbackKeyword::Rollback))
        } else if self.features().utility_syntax.abort_transaction_alias
            && self.eat_contextual_keyword("ABORT")?
        {
            Ok(Some(TransactionRollbackKeyword::Abort))
        } else {
            Ok(None)
        }
    }

    /// Consume the interchangeable `WORK` / `TRANSACTION` block noise word that may
    /// follow `BEGIN`/`COMMIT`/`ROLLBACK`, returning which was written (or `None`).
    /// The two spellings carry no meaning; the tag lets a source-fidelity render
    /// replay the exact word.
    fn eat_transaction_block_keyword(
        &mut self,
        transaction_keyword: bool,
    ) -> ParseResult<Option<TransactionBlockKeyword>> {
        if self.features().utility_syntax.transaction_work_keyword
            && self.eat_contextual_keyword("WORK")?
        {
            Ok(Some(TransactionBlockKeyword::Work))
        } else if transaction_keyword && self.eat_contextual_keyword("TRANSACTION")? {
            Ok(Some(TransactionBlockKeyword::Transaction))
        } else {
            Ok(None)
        }
    }

    /// Parse SQLite's optional `{DEFERRED | IMMEDIATE | EXCLUSIVE}` transaction-mode
    /// modifier immediately after `BEGIN`, gated by `utility_syntax.begin_transaction_mode`.
    /// `None` when the dialect does not admit the modifier or the statement omits it; in
    /// either case the word (if any) is left unconsumed for the noise-word/mode-list parse
    /// that follows, so an unrecognized modifier surfaces as the existing trailing-token
    /// error rather than a bespoke one.
    fn parse_transaction_mode_kind(&mut self) -> ParseResult<Option<TransactionModeKind>> {
        if !self.features().utility_syntax.begin_transaction_mode {
            return Ok(None);
        }
        if self.eat_contextual_keyword("DEFERRED")? {
            Ok(Some(TransactionModeKind::Deferred))
        } else if self.eat_contextual_keyword("IMMEDIATE")? {
            Ok(Some(TransactionModeKind::Immediate))
        } else if self.eat_contextual_keyword("EXCLUSIVE")? {
            Ok(Some(TransactionModeKind::Exclusive))
        } else {
            Ok(None)
        }
    }

    /// Parse a possibly-empty transaction mode list (`START`/`BEGIN`,
    /// `SET TRANSACTION`, and the session `SET SESSION CHARACTERISTICS`).
    pub(super) fn parse_transaction_modes(&mut self) -> ParseResult<ThinVec<TransactionMode>> {
        self.parse_transaction_modes_with(
            self.features().utility_syntax.transaction_isolation_mode,
            self.features().utility_syntax.transaction_access_mode,
            self.features().utility_syntax.transaction_deferrable_mode,
            false,
        )
    }

    fn parse_transaction_modes_with(
        &mut self,
        isolation: bool,
        access: bool,
        deferrable: bool,
        consistent_snapshot: bool,
    ) -> ParseResult<ThinVec<TransactionMode>> {
        let mut modes = ThinVec::new();
        let mut mode_required = false;
        loop {
            let Some(mode) = self.parse_optional_transaction_mode(
                isolation,
                access,
                deferrable,
                consistent_snapshot,
            )?
            else {
                if mode_required {
                    return Err(self.unexpected("a transaction mode after `,`"));
                }
                break;
            };
            if self.features().utility_syntax.transaction_modes_unique
                && modes
                    .iter()
                    .any(|existing| same_transaction_mode_kind(existing, &mode))
            {
                return Err(self.unexpected("a transaction mode that has not already appeared"));
            }
            modes.push(mode);
            if !self.features().utility_syntax.transaction_multiple_modes {
                break;
            }
            // The mode separator is an optional comma: ANSI writes commas between
            // modes while PostgreSQL also allows bare juxtaposition. Consume one if
            // present; the loop ends when no further mode follows.
            let comma = self.eat_punct(Punctuation::Comma)?;
            if self
                .features()
                .utility_syntax
                .transaction_mode_comma_required
                && !comma
            {
                break;
            }
            mode_required = comma;
        }
        Ok(modes)
    }

    fn parse_optional_transaction_mode(
        &mut self,
        isolation: bool,
        access: bool,
        deferrable: bool,
        consistent_snapshot: bool,
    ) -> ParseResult<Option<TransactionMode>> {
        let start = self.current_span()?;
        if isolation && self.eat_contextual_keyword("ISOLATION")? {
            self.expect_contextual_keyword("LEVEL")?;
            let level = self.parse_isolation_level()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TransactionMode::IsolationLevel { level, meta }))
        } else if access && self.eat_contextual_keyword("READ")? {
            let access = if self.eat_contextual_keyword("ONLY")? {
                TransactionAccessMode::ReadOnly
            } else if self.eat_contextual_keyword("WRITE")? {
                TransactionAccessMode::ReadWrite
            } else {
                return Err(self.unexpected("`ONLY` or `WRITE` after `READ`"));
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TransactionMode::AccessMode { access, meta }))
        } else if deferrable && self.eat_contextual_keyword("DEFERRABLE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TransactionMode::Deferrable {
                deferrable: true,
                meta,
            }))
        } else if deferrable && self.eat_contextual_keyword("NOT")? {
            // `NOT` opens only `NOT DEFERRABLE` in a transaction mode list.
            self.expect_contextual_keyword("DEFERRABLE")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TransactionMode::Deferrable {
                deferrable: false,
                meta,
            }))
        } else if consistent_snapshot && self.eat_contextual_keyword("WITH")? {
            self.expect_contextual_keyword("CONSISTENT")?;
            self.expect_contextual_keyword("SNAPSHOT")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TransactionMode::ConsistentSnapshot { meta }))
        } else {
            Ok(None)
        }
    }

    fn parse_isolation_level(&mut self) -> ParseResult<IsolationLevel> {
        if self.eat_contextual_keyword("READ")? {
            if self.eat_contextual_keyword("UNCOMMITTED")? {
                Ok(IsolationLevel::ReadUncommitted)
            } else if self.eat_contextual_keyword("COMMITTED")? {
                Ok(IsolationLevel::ReadCommitted)
            } else {
                Err(self.unexpected("`UNCOMMITTED` or `COMMITTED` after `READ`"))
            }
        } else if self.eat_contextual_keyword("REPEATABLE")? {
            self.expect_contextual_keyword("READ")?;
            Ok(IsolationLevel::RepeatableRead)
        } else if self.eat_contextual_keyword("SERIALIZABLE")? {
            Ok(IsolationLevel::Serializable)
        } else {
            Err(self.unexpected("an isolation level"))
        }
    }

    /// Parse a MySQL `XA` distributed-transaction statement into [`Statement::Xa`],
    /// reached under [`UtilitySyntax::xa_transactions`](crate::ast::dialect::UtilitySyntax).
    pub(super) fn parse_xa_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("XA")?;
        let xa = self.parse_xa_statement_kind(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Xa {
            xa: Box::new(xa),
            meta,
        })
    }

    fn parse_xa_statement_kind(&mut self, start: Span) -> ParseResult<XaStatement> {
        // `begin_or_start`: `START` and `BEGIN` are exact synonyms for the branch-start verb.
        let keyword = if self.eat_contextual_keyword("START")? {
            Some(XaStartKeyword::Start)
        } else if self.eat_contextual_keyword("BEGIN")? {
            Some(XaStartKeyword::Begin)
        } else {
            None
        };
        if let Some(keyword) = keyword {
            let xid = self.parse_xid()?;
            // `opt_join_or_resume`: valid only on the branch-start verb.
            let association = if self.eat_contextual_keyword("JOIN")? {
                Some(XaAssociation::Join)
            } else if self.eat_contextual_keyword("RESUME")? {
                Some(XaAssociation::Resume)
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(XaStatement::Start {
                keyword,
                xid,
                association,
                meta,
            });
        }
        if self.eat_contextual_keyword("END")? {
            let xid = self.parse_xid()?;
            // `opt_suspend`: `SUSPEND`, optionally `SUSPEND FOR MIGRATE`.
            let suspend = if self.eat_contextual_keyword("SUSPEND")? {
                if self.eat_contextual_keyword("FOR")? {
                    self.expect_contextual_keyword("MIGRATE")?;
                    Some(XaSuspend::SuspendForMigrate)
                } else {
                    Some(XaSuspend::Suspend)
                }
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(XaStatement::End { xid, suspend, meta })
        } else if self.eat_contextual_keyword("PREPARE")? {
            let xid = self.parse_xid()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(XaStatement::Prepare { xid, meta })
        } else if self.eat_contextual_keyword("COMMIT")? {
            let xid = self.parse_xid()?;
            // `opt_one_phase`: the `ONE PHASE` single-phase-commit optimisation.
            let one_phase = if self.eat_contextual_keyword("ONE")? {
                self.expect_contextual_keyword("PHASE")?;
                true
            } else {
                false
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(XaStatement::Commit {
                xid,
                one_phase,
                meta,
            })
        } else if self.eat_contextual_keyword("ROLLBACK")? {
            let xid = self.parse_xid()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(XaStatement::Rollback { xid, meta })
        } else if self.eat_contextual_keyword("RECOVER")? {
            // `opt_convert_xid`: both words are mandatory together.
            let convert_xid = if self.eat_contextual_keyword("CONVERT")? {
                self.expect_contextual_keyword("XID")?;
                true
            } else {
                false
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(XaStatement::Recover { convert_xid, meta })
        } else {
            Err(self.unexpected(
                "an XA verb (`START`/`BEGIN`/`END`/`PREPARE`/`COMMIT`/`ROLLBACK`/`RECOVER`)",
            ))
        }
    }

    /// Parse an XA transaction-branch identifier `gtrid [, bqual [, formatID]]`
    /// (`sql_yacc.yy` `xid`). `formatID` is admitted only after a `bqual`.
    fn parse_xid(&mut self) -> ParseResult<Xid> {
        let gtrid = self.parse_xid_text("an XID `gtrid` string or hex/binary literal")?;
        let start = gtrid.meta.span;
        let (bqual, format_id) = if self.eat_punct(Punctuation::Comma)? {
            let bqual =
                self.parse_xid_text("an XID `bqual` string or hex/binary literal after `,`")?;
            let format_id = if self.eat_punct(Punctuation::Comma)? {
                Some(self.parse_xid_format_id()?)
            } else {
                None
            };
            (Some(bqual), format_id)
        } else {
            (None, None)
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Xid {
            gtrid,
            bqual,
            format_id,
            meta,
        })
    }

    /// Parse an xid `gtrid`/`bqual` byte-string constant (`text_string`): a character-string
    /// literal, or a hexadecimal / binary literal (`0x…` / `X'…'` / `0b…` / `B'…'`). A bare
    /// decimal number is *not* accepted here (only `HEX_NUM` / `BIN_NUM`), matching the engine.
    fn parse_xid_text(&mut self, expected: &'static str) -> ParseResult<Literal> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected(expected));
        };
        match token.kind {
            // The string forms (`'…'`, `X'…'`, `B'…'`) reuse the expression string-literal
            // reader, so the bit-string kind and any adjacent-literal continuation resolve
            // exactly as elsewhere; it always yields an `Expr::Literal`.
            TokenKind::String => match self.parse_string_literal(token)? {
                Expr::Literal { literal, .. } => Ok(literal),
                other => unreachable!("parse_string_literal yields Expr::Literal, got {other:?}"),
            },
            // A radix-prefixed number (`0x…` / `0b…`) is the `HEX_NUM` / `BIN_NUM` spelling of a
            // byte string; a base-10 number is a plain integer, which `text_string` rejects.
            TokenKind::Number if split_radix_prefix(self.span_text(token.span)).0 != 10 => {
                self.advance()?;
                Ok(Literal {
                    kind: number_literal_kind(
                        self.span_text(token.span),
                        self.float_as_decimal_enabled(),
                    ),
                    meta: self.make_meta(token.span),
                })
            }
            _ => Err(self.unexpected(expected)),
        }
    }

    /// Parse an xid `formatID` (`ulong_num`): any non-negative numeric literal. A leading
    /// sign is a separate token, so `-1` is left for the trailing-token check and rejects, as
    /// the engine does.
    fn parse_xid_format_id(&mut self) -> ParseResult<Literal> {
        let expected = "a numeric `formatID` after the branch qualifier";
        let Some(token) = self.peek()? else {
            return Err(self.unexpected(expected));
        };
        if token.kind != TokenKind::Number {
            return Err(self.unexpected(expected));
        }
        self.advance()?;
        Ok(Literal {
            kind: number_literal_kind(self.span_text(token.span), self.float_as_decimal_enabled()),
            meta: self.make_meta(token.span),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        IsolationLevel, Resolver as _, Span, Spanned, Statement, TransactionAccessMode,
        TransactionMode, TransactionStart, TransactionStatement,
    };
    use crate::parser::{TestDialect, parse_with};

    fn parse_transaction(sql: &str) -> TransactionStatement {
        let parsed = parse_with(sql, crate::ParseConfig::new(TestDialect))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let [Statement::Transaction { transaction, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one transaction statement: {:?}",
                parsed.statements(),
            );
        };
        (**transaction).clone()
    }

    /// The dispatch contract: each leading keyword the transaction family claims
    /// is routed by the central `parse_statement` to this module's entry and yields a
    /// `Statement::Transaction`. This pins the dispatch boundary — the full claimed
    /// keyword set — independently of the per-construct grammar the other tests cover;
    /// `parse_transaction` panics if a keyword fails to route to this family.
    #[test]
    fn dispatch_routes_transaction_keywords_to_this_family() {
        for sql in [
            "BEGIN",
            "START TRANSACTION",
            "COMMIT",
            "ROLLBACK",
            "SAVEPOINT sp",
            "RELEASE sp",
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
        ] {
            let _ = parse_transaction(sql);
        }
    }

    #[test]
    fn begin_and_start_transaction_share_a_shape_with_a_surface_tag() {
        // BEGIN and START TRANSACTION are synonyms recorded by a surface tag, and
        // the WORK/TRANSACTION noise words are accepted but not represented.
        for (sql, expected) in [
            ("BEGIN", TransactionStart::Begin),
            ("BEGIN WORK", TransactionStart::Begin),
            ("BEGIN TRANSACTION", TransactionStart::Begin),
            ("START TRANSACTION", TransactionStart::Start),
        ] {
            let TransactionStatement::Begin { syntax, modes, .. } = parse_transaction(sql) else {
                panic!("{sql:?} should be a Begin statement");
            };
            assert_eq!(syntax, expected, "{sql:?}");
            assert!(modes.is_empty(), "{sql:?} has no modes");
        }
    }

    #[test]
    fn begin_statement_span_covers_the_whole_construct() {
        let parsed =
            parse_with("START TRANSACTION", crate::ParseConfig::new(TestDialect)).expect("parses");
        let [stmt @ Statement::Transaction { .. }] = parsed.statements() else {
            panic!("expected one transaction statement");
        };
        assert_eq!(stmt.span(), Span::new(0, "START TRANSACTION".len() as u32));
    }

    #[test]
    fn transaction_modes_parse_with_or_without_commas() {
        // ANSI comma-separated and PostgreSQL space-separated mode lists are both
        // accepted and yield the same shape.
        for sql in [
            "START TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY",
            "START TRANSACTION ISOLATION LEVEL SERIALIZABLE READ ONLY",
        ] {
            let TransactionStatement::Begin { modes, .. } = parse_transaction(sql) else {
                panic!("{sql:?} should be a Begin statement");
            };
            assert!(
                matches!(
                    modes.as_slice(),
                    [
                        TransactionMode::IsolationLevel {
                            level: IsolationLevel::Serializable,
                            ..
                        },
                        TransactionMode::AccessMode {
                            access: TransactionAccessMode::ReadOnly,
                            ..
                        },
                    ],
                ),
                "{sql:?} modes: {modes:?}",
            );
        }
    }

    #[test]
    fn all_isolation_levels_parse() {
        for (sql, expected) in [
            (
                "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED",
                IsolationLevel::ReadUncommitted,
            ),
            (
                "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
                IsolationLevel::ReadCommitted,
            ),
            (
                "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ",
                IsolationLevel::RepeatableRead,
            ),
            (
                "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
                IsolationLevel::Serializable,
            ),
        ] {
            let TransactionStatement::SetCharacteristics { modes, .. } = parse_transaction(sql)
            else {
                panic!("{sql:?} should be SET TRANSACTION");
            };
            assert!(
                matches!(
                    modes.as_slice(),
                    [TransactionMode::IsolationLevel { level, .. }] if *level == expected,
                ),
                "{sql:?} modes: {modes:?}",
            );
        }
    }

    #[test]
    fn deferrable_mode_parses_on_start_and_set_transaction() {
        for (sql, expected) in [
            ("START TRANSACTION DEFERRABLE", true),
            ("START TRANSACTION NOT DEFERRABLE", false),
        ] {
            let TransactionStatement::Begin { modes, .. } = parse_transaction(sql) else {
                panic!("{sql:?} should be a Begin statement");
            };
            assert!(
                matches!(
                    modes.as_slice(),
                    [TransactionMode::Deferrable { deferrable, .. }] if *deferrable == expected,
                ),
                "{sql:?} modes: {modes:?}",
            );
        }
        // The mode rides `SET TRANSACTION` and mixes with the others.
        let TransactionStatement::SetCharacteristics { modes, .. } =
            parse_transaction("SET TRANSACTION READ ONLY, NOT DEFERRABLE")
        else {
            panic!("expected SET TRANSACTION");
        };
        assert!(matches!(
            modes.as_slice(),
            [
                TransactionMode::AccessMode { .. },
                TransactionMode::Deferrable {
                    deferrable: false,
                    ..
                },
            ],
        ));
    }

    #[test]
    fn commit_and_rollback_parse_with_optional_savepoint_rewind() {
        assert!(matches!(
            parse_transaction("COMMIT"),
            TransactionStatement::Commit { .. }
        ));
        assert!(matches!(
            parse_transaction("COMMIT WORK"),
            TransactionStatement::Commit { .. }
        ));
        assert!(matches!(
            parse_transaction("ROLLBACK"),
            TransactionStatement::Rollback {
                to_savepoint: None,
                ..
            }
        ));
        // `TO SAVEPOINT name` and the SAVEPOINT-less `TO name` are the same shape.
        for sql in ["ROLLBACK TO SAVEPOINT sp1", "ROLLBACK TO sp1"] {
            assert!(
                matches!(
                    parse_transaction(sql),
                    TransactionStatement::Rollback {
                        to_savepoint: Some(_),
                        ..
                    }
                ),
                "{sql:?}",
            );
        }
    }

    #[test]
    fn savepoint_and_release_capture_the_name() {
        let parsed =
            parse_with("SAVEPOINT sp1", crate::ParseConfig::new(TestDialect)).expect("parses");
        let [Statement::Transaction { transaction, .. }] = parsed.statements() else {
            panic!("expected a transaction statement");
        };
        let TransactionStatement::Savepoint { name, .. } = &**transaction else {
            panic!("expected SAVEPOINT");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "sp1");

        for sql in ["RELEASE SAVEPOINT sp1", "RELEASE sp1"] {
            assert!(
                matches!(parse_transaction(sql), TransactionStatement::Release { .. }),
                "{sql:?}",
            );
        }
    }

    #[test]
    fn malformed_transaction_statements_are_rejected() {
        for sql in [
            "SAVEPOINT",                 // missing name
            "RELEASE",                   // missing name
            "START",                     // missing TRANSACTION
            "SET TRANSACTION",           // missing mode
            "START TRANSACTION READ",    // READ without ONLY/WRITE
            "SET TRANSACTION ISOLATION", // ISOLATION without LEVEL
            "START TRANSACTION NOT",     // NOT without DEFERRABLE
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    // --- XA distributed-transaction family -----------------------------------

    use crate::ast::{XaAssociation, XaStartKeyword, XaStatement, XaSuspend};
    use crate::parser::FeatureDialect;
    use crate::render::Renderer;

    /// A MySQL-featured dialect so the `xa_transactions`-gated `XA` family parses and
    /// renders back to a MySQL target through one round-trip value.
    const XA_DIALECT: FeatureDialect = FeatureDialect {
        features: &crate::ast::dialect::FeatureSet::MYSQL,
    };

    fn parse_xa(sql: &str) -> XaStatement {
        let parsed = parse_with(sql, crate::ParseConfig::new(XA_DIALECT))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let [Statement::Xa { xa, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one XA statement: {:?}",
                parsed.statements(),
            );
        };
        (**xa).clone()
    }

    #[test]
    fn xa_dispatch_routes_only_under_the_gate() {
        // The leading `XA` keyword routes to this family only when `xa_transactions` is on;
        // it is not dispatched under ANSI, where it surfaces as an unknown statement.
        let _ = parse_xa("XA PREPARE 'x'");
        assert!(
            parse_with("XA PREPARE 'x'", crate::ParseConfig::new(TestDialect)).is_err(),
            "`XA` must not be dispatched without the gate",
        );
    }

    #[test]
    fn xa_every_verb_parses_and_round_trips() {
        for (sql, check) in [
            (
                "XA START 'gtrid'",
                &(|xa: &XaStatement| {
                    matches!(
                        xa,
                        XaStatement::Start {
                            keyword: XaStartKeyword::Start,
                            association: None,
                            ..
                        }
                    )
                }) as &dyn Fn(&XaStatement) -> bool,
            ),
            ("XA BEGIN 'gtrid'", &|xa| {
                matches!(
                    xa,
                    XaStatement::Start {
                        keyword: XaStartKeyword::Begin,
                        ..
                    }
                )
            }),
            ("XA START 'gtrid' JOIN", &|xa| {
                matches!(
                    xa,
                    XaStatement::Start {
                        association: Some(XaAssociation::Join),
                        ..
                    }
                )
            }),
            ("XA START 'gtrid' RESUME", &|xa| {
                matches!(
                    xa,
                    XaStatement::Start {
                        association: Some(XaAssociation::Resume),
                        ..
                    }
                )
            }),
            ("XA END 'gtrid'", &|xa| {
                matches!(xa, XaStatement::End { suspend: None, .. })
            }),
            ("XA END 'gtrid' SUSPEND", &|xa| {
                matches!(
                    xa,
                    XaStatement::End {
                        suspend: Some(XaSuspend::Suspend),
                        ..
                    }
                )
            }),
            ("XA END 'gtrid' SUSPEND FOR MIGRATE", &|xa| {
                matches!(
                    xa,
                    XaStatement::End {
                        suspend: Some(XaSuspend::SuspendForMigrate),
                        ..
                    }
                )
            }),
            ("XA PREPARE 'gtrid'", &|xa| {
                matches!(xa, XaStatement::Prepare { .. })
            }),
            ("XA COMMIT 'gtrid'", &|xa| {
                matches!(
                    xa,
                    XaStatement::Commit {
                        one_phase: false,
                        ..
                    }
                )
            }),
            ("XA COMMIT 'gtrid' ONE PHASE", &|xa| {
                matches!(
                    xa,
                    XaStatement::Commit {
                        one_phase: true,
                        ..
                    }
                )
            }),
            ("XA ROLLBACK 'gtrid'", &|xa| {
                matches!(xa, XaStatement::Rollback { .. })
            }),
            ("XA RECOVER", &|xa| {
                matches!(
                    xa,
                    XaStatement::Recover {
                        convert_xid: false,
                        ..
                    }
                )
            }),
            ("XA RECOVER CONVERT XID", &|xa| {
                matches!(
                    xa,
                    XaStatement::Recover {
                        convert_xid: true,
                        ..
                    }
                )
            }),
        ] {
            let xa = parse_xa(sql);
            assert!(check(&xa), "{sql:?} shape: {xa:?}");
            let parsed = parse_with(sql, crate::ParseConfig::new(XA_DIALECT)).expect("parses");
            let rendered = Renderer::new(XA_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn xid_admits_string_hex_and_binary_forms_and_round_trips() {
        // `gtrid`/`bqual` are `text_string`: a character string, a `0x`/`X'…'` hex literal,
        // or a `0b`/`B'…'` binary literal; `formatID` is any non-negative numeric literal.
        // Each spelling round-trips byte-identically.
        for sql in [
            "XA START 'gtrid', 'bqual'",
            "XA START 'gtrid', 'bqual', 42",
            "XA START 0x1234",
            "XA START 0x1234, 0xABCD, 7",
            "XA START X'1234'",
            "XA START 0b1010",
            "XA START B'1010'",
            "XA START 'g', 'b', 0x10",
            "XA START 'g', 'b', 3.5",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(XA_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Xa { .. }]),
                "{sql:?} should parse to an XA statement",
            );
            let rendered = Renderer::new(XA_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn xa_reject_boundaries_match_the_engine() {
        // Every arm here is a live-8.4.10 `ER_PARSE_ERROR` (1064): xid mandatory where the
        // grammar requires it, the suffix keywords bound to their own verbs, `formatID`
        // numeric and only after a `bqual`, and a bare decimal `gtrid` rejected.
        for sql in [
            "XA START",                      // missing xid
            "XA PREPARE",                    // missing xid
            "XA START 42",                   // decimal gtrid is not text_string
            "XA START 'g', 'b', 'c'",        // formatID must be numeric
            "XA START 'g' JOIN RESUME",      // at most one association keyword
            "XA START 'gtrid' SUSPEND",      // SUSPEND is an END-only suffix
            "XA END 'gtrid' JOIN",           // JOIN is a START-only suffix
            "XA END 'g' FOR MIGRATE",        // FOR MIGRATE requires SUSPEND
            "XA END 'g' SUSPEND MIGRATE",    // MIGRATE requires the FOR keyword
            "XA COMMIT 'gtrid' JOIN",        // COMMIT takes only ONE PHASE
            "XA COMMIT 'gtrid' TWO PHASE",   // only ONE PHASE
            "XA PREPARE 'gtrid' ONE PHASE",  // PREPARE takes no suffix
            "XA ROLLBACK 'gtrid' ONE PHASE", // ROLLBACK takes no suffix
            "XA RECOVER 'gtrid'",            // RECOVER takes no xid
            "XA RECOVER CONVERT",            // CONVERT requires XID
            "XA WOBBLE 'gtrid'",             // unknown verb
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(XA_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }
}
