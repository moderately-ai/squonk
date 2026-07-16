// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Utility-statement grammar: PostgreSQL `COPY` and `EXPLAIN`, and the
//! SQLite `PRAGMA` / `ATTACH` / `DETACH` configuration statements and
//! `VACUUM` / `REINDEX` / `ANALYZE` maintenance statements.
//!
//! Owns the utility statements plus their option grammars, and is reached from the
//! statement dispatcher in [`super::query`]. The vocabulary is matched as
//! contextual words (the full keyword inventory is a separate ticket). `COPY` is a
//! PostgreSQL-specific statement, so its leading keyword is gated on
//! [`UtilitySyntax::copy`](crate::ast::dialect::UtilitySyntax): dispatched under
//! PostgreSQL (and its generic/lenient supersets), left undispatched — an unknown
//! statement — under ANSI and MySQL, the same reject mechanism the `merge`/
//! `replace_into` leading-keyword gates use. The SQLite statements gate the same
//! way on [`UtilitySyntax::pragma`](crate::ast::dialect::UtilitySyntax),
//! [`UtilitySyntax::attach`](crate::ast::dialect::UtilitySyntax) (one flag for the
//! `ATTACH`/`DETACH` pair), and the per-statement
//! [`vacuum`](crate::ast::dialect::UtilitySyntax)/
//! [`reindex`](crate::ast::dialect::UtilitySyntax)/
//! [`analyze`](crate::ast::dialect::UtilitySyntax) maintenance flags (independent
//! statements, so independent flags — the `copy`/`comment_on` precedent, not the
//! paired-`attach` one). The Snowflake `COPY INTO` load/unload statement shares the
//! leading `COPY` keyword but is its own grammar and its own gate
//! ([`UtilitySyntax::copy_into`](crate::ast::dialect::UtilitySyntax)): the dispatcher
//! routes on the `INTO` after `COPY`, so a preset can enable either surface
//! independently (Snowflake has `copy_into` without `copy`; PostgreSQL the reverse).
//! `EXPLAIN` carries no gate yet and is accepted
//! dialect-agnostically; a *leading* `ANALYZE` is dispatched (under its gate) before
//! `EXPLAIN` ever sees the word, whose own `ANALYZE` is a non-leading option
//! position.
//!
//! `COPY` covers the table and `COPY (<query>)` row sources, both `FROM`/`TO`
//! endpoints, both the parenthesized and legacy un-parenthesized option spellings,
//! and the remaining legacy table-only surfaces: the `FORCE QUOTE`/`FORCE NULL`/
//! `FORCE NOT NULL` column-list options, the `opt_binary` prefix (`COPY BINARY t`),
//! the `[USING] DELIMITERS '<str>'` clause, and the `COPY <table> FROM ... WHERE`
//! filter (`FROM`-only, matching PostgreSQL). `EXPLAIN` covers the legacy `[ANALYZE]
//! [VERBOSE]` prefix and the parenthesized option list with the common
//! `FORMAT`/boolean options.

use crate::ast::{
    AccountName, AnalyzeHistogram, AnalyzeStatement, AssignGtidsKind, AttachStatement,
    BinlogStatement, CacheIndexKeyList, CacheIndexKeyword, CacheIndexStatement, CacheIndexTable,
    CacheIndexTargets, CallStatement, ChangeReplicationSourceOption,
    ChangeReplicationSourceOptionValue, CheckTableOption, CheckpointStatement, ChecksumTableOption,
    CloneDataDirectory, CloneSsl, CloneStatement, CopyDirection, CopyIntoSource, CopyIntoStatement,
    CopyIntoTarget, CopyOption, CopyOptionValue, CopySource, CopyStatement, CopyTarget,
    DeallocateKeyword, DeallocateStatement, DescribeColumn, DescribeStatement, DetachStatement,
    DoArg, DoExpressionsStatement, DoStatement, ExecuteStatement, ExecuteUsingStatement,
    ExplainFormat, ExplainKeyword, ExplainOption, ExplainStatement, ExportStatement, Expr,
    FlushOption, FlushStatement, FlushTablesLock, FlushTarget, ForceKind, GroupReplicationOption,
    HelpStatement, Ident, ImportStatement, ImportTableStatement, InstanceLockStatement,
    IoThreadKeyword, KeyCacheName, Keyword, KeywordSet, KillStatement, KillTarget, LanguageName,
    Literal, LiteralKind, LoadDataConcurrency, LoadDataDuplicate, LoadDataEnclosed,
    LoadDataFieldOrVar, LoadDataFields, LoadDataFormat, LoadDataIgnoreRows, LoadDataIgnoreUnit,
    LoadDataLines, LoadDataStatement, LoadFieldsSpelling, LoadIndexStatement, LoadIndexTable,
    LoadIndexTargets, LoadStatement, LoadTarget, LockTablesStatement, NoWriteToBinlog, ObjectName,
    PartitionSelection, PragmaStatement, PrepareFromStatement, PrepareSource, PrepareStatement,
    PurgeStatement, PurgeTarget, QuoteStyle, ReindexStatement, RenameStatement, RepairTableOption,
    ReplicaThreadOption, ReplicaUntilCondition, ReplicationFilterRule, ReplicationStatement,
    RequirePrimaryKeyCheck, RewriteDbPair, ShowBare, ShowColumnsSpelling, ShowCreateKind,
    ShowDiagnosticKind, ShowEngineArtifact, ShowFilter, ShowFrom, ShowFromKeyword,
    ShowFunctionsFilter, ShowFunctionsScope, ShowIndexSpelling, ShowLimit, ShowListing,
    ShowProfileType, ShowRoutineKind, ShowScope, ShowStatement, ShowTarget, SourceOption, Span,
    Statement, TableKeyword, TableLock, TableLockKind, TableMaintenanceKind,
    TableMaintenanceStatement, TableRename, UnlockTablesStatement, UpdateExtensionsStatement,
    UseStatement, UserRename, VacuumAnalyze, VacuumStatement,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;
use super::expr::{number_literal_kind, string_literal_is_name_sconst, string_literal_is_sconst};

/// Validate a possibly-continued `U&'…'` span by concatenating segment *bodies*
/// (PostgreSQL joins continued constants before escape decoding — so five `\\` in
/// one segment plus one in the next is six backslashes, a valid pair sequence).
fn unicode_escape_segments_are_valid(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 4 || !matches!(bytes[0], b'U' | b'u') || bytes[1] != b'&' || bytes[2] != b'\''
    {
        return crate::ast::unicode_escape_string_is_valid(text);
    }
    let Some(joined) = join_continued_string_bodies(bytes, 2) else {
        return false;
    };
    let mut synthetic = String::with_capacity(joined.len() + 4);
    synthetic.push_str("U&'");
    synthetic.push_str(&joined);
    synthetic.push('\'');
    crate::ast::unicode_escape_string_is_valid(&synthetic)
}

/// Extract and concatenate the bodies of quote-delimited segments in a continued
/// string span. `first_open` is the index of the first opening `'`.
fn join_continued_string_bodies(bytes: &[u8], first_open: usize) -> Option<String> {
    let mut joined = String::new();
    let mut i = first_open;
    loop {
        if i >= bytes.len() || bytes[i] != b'\'' {
            return None;
        }
        i += 1; // past open
        let body_start = i;
        let mut closed = false;
        while i < bytes.len() {
            if bytes[i] == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2; // doubled quote stays in body
                } else {
                    let body = std::str::from_utf8(&bytes[body_start..i]).ok()?;
                    joined.push_str(body);
                    i += 1;
                    closed = true;
                    break;
                }
            } else {
                i += 1;
            }
        }
        if !closed {
            return None;
        }
        let mut saw_newline = false;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' | b'\r' => {
                    saw_newline = true;
                    i += 1;
                }
                b' ' | b'\t' | 0x0b | 0x0c => i += 1,
                _ => break,
            }
        }
        if saw_newline && i < bytes.len() && bytes[i] == b'\'' {
            continue;
        }
        return (i == bytes.len()).then_some(joined);
    }
}

/// The value shape a `CHANGE REPLICATION SOURCE TO` option name dictates — the parser-internal
/// axis that [`SOURCE_OPTION_TABLE`] pairs with each [`SourceOption`] so the value grammar is
/// read once per shape rather than once per option.
#[derive(Clone, Copy)]
enum SourceOptionShape {
    /// A string literal.
    String,
    /// A numeric literal (integer or fractional; the `0`/`1` flags ride here too).
    Number,
    /// A string literal or the bare `NULL` (`SOURCE_TLS_CIPHERSUITES`).
    NullableString,
    /// An account or the bare `NULL` (`PRIVILEGE_CHECKS_USER`).
    User,
    /// A parenthesized unsigned-integer list (`IGNORE_SERVER_IDS`).
    ServerIds,
    /// `ON | OFF | STREAM | GENERATE` (`REQUIRE_TABLE_PRIMARY_KEY_CHECK`).
    PrimaryKeyCheck,
    /// `OFF | LOCAL | '<uuid>'` (`ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS`).
    AssignGtids,
}

/// The measured `CHANGE REPLICATION SOURCE TO` option set paired with each option's value
/// shape. Each keyword (via [`SourceOption::keyword`]) is a whole-identifier token, so the
/// scan order is irrelevant. The set is engine-narrowed from `sql_yacc.yy` `source_def`: no
/// `MASTER_*` name (removed in 8.4) and the *plural* `SOURCE_COMPRESSION_ALGORITHMS`.
const SOURCE_OPTION_TABLE: &[(SourceOption, SourceOptionShape)] = &[
    (SourceOption::SourceBind, SourceOptionShape::String),
    (SourceOption::SourceHost, SourceOptionShape::String),
    (SourceOption::SourceUser, SourceOptionShape::String),
    (SourceOption::SourcePassword, SourceOptionShape::String),
    (SourceOption::SourcePort, SourceOptionShape::Number),
    (SourceOption::SourceConnectRetry, SourceOptionShape::Number),
    (SourceOption::SourceRetryCount, SourceOptionShape::Number),
    (SourceOption::SourceDelay, SourceOptionShape::Number),
    (
        SourceOption::SourceHeartbeatPeriod,
        SourceOptionShape::Number,
    ),
    (SourceOption::SourceLogFile, SourceOptionShape::String),
    (SourceOption::SourceLogPos, SourceOptionShape::Number),
    (SourceOption::SourceAutoPosition, SourceOptionShape::Number),
    (SourceOption::RelayLogFile, SourceOptionShape::String),
    (SourceOption::RelayLogPos, SourceOptionShape::Number),
    (SourceOption::SourceSsl, SourceOptionShape::Number),
    (SourceOption::SourceSslCa, SourceOptionShape::String),
    (SourceOption::SourceSslCapath, SourceOptionShape::String),
    (SourceOption::SourceSslCert, SourceOptionShape::String),
    (SourceOption::SourceSslCipher, SourceOptionShape::String),
    (SourceOption::SourceSslKey, SourceOptionShape::String),
    (
        SourceOption::SourceSslVerifyServerCert,
        SourceOptionShape::Number,
    ),
    (SourceOption::SourceSslCrl, SourceOptionShape::String),
    (SourceOption::SourceSslCrlpath, SourceOptionShape::String),
    (SourceOption::SourceTlsVersion, SourceOptionShape::String),
    (
        SourceOption::SourceTlsCiphersuites,
        SourceOptionShape::NullableString,
    ),
    (SourceOption::SourcePublicKeyPath, SourceOptionShape::String),
    (SourceOption::GetSourcePublicKey, SourceOptionShape::Number),
    (SourceOption::NetworkNamespace, SourceOptionShape::String),
    (SourceOption::IgnoreServerIds, SourceOptionShape::ServerIds),
    (
        SourceOption::SourceCompressionAlgorithms,
        SourceOptionShape::String,
    ),
    (
        SourceOption::SourceZstdCompressionLevel,
        SourceOptionShape::Number,
    ),
    (SourceOption::PrivilegeChecksUser, SourceOptionShape::User),
    (SourceOption::RequireRowFormat, SourceOptionShape::Number),
    (
        SourceOption::RequireTablePrimaryKeyCheck,
        SourceOptionShape::PrimaryKeyCheck,
    ),
    (
        SourceOption::SourceConnectionAutoFailover,
        SourceOptionShape::Number,
    ),
    (
        SourceOption::AssignGtidsToAnonymousTransactions,
        SourceOptionShape::AssignGtids,
    ),
    (SourceOption::GtidOnly, SourceOptionShape::Number),
];

impl<'a, D: Dialect> Parser<'a, D> {
    // --- COPY ---------------------------------------------------------------

    /// Dispatch a leading `COPY` to the Snowflake `COPY INTO` load/unload statement or
    /// the PostgreSQL `COPY <table> {FROM | TO}` transfer.
    ///
    /// The two are disjoint: PostgreSQL's `COPY` never has `INTO` after the keyword, so a
    /// `COPY INTO` lookahead routes to [`parse_copy_into_statement`](Self::parse_copy_into_statement)
    /// when `copy_into` is on, and everything else falls to the `copy`-gated
    /// [`parse_copy_statement`](Self::parse_copy_statement). Reached only when at least one
    /// of the two gates is on (the dispatcher's guard), so a `COPY INTO` under a preset with
    /// `copy_into` off but `copy` on parses as PostgreSQL `COPY` and rejects on the `INTO`,
    /// while `COPY` under `copy_into` on but `copy` off with no `INTO` rejects here.
    pub(super) fn parse_copy_or_copy_into_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        if self.features().utility_syntax.copy_into
            && self.peek_nth_is_contextual_keyword(1, "INTO")?
        {
            return self.parse_copy_into_statement();
        }
        if self.features().utility_syntax.copy {
            return self.parse_copy_statement();
        }
        // `copy_into` is on (the dispatcher guard) but the input is not `COPY INTO` and PG
        // `COPY` is off: report the missing `INTO` rather than the misleading PG expectation.
        Err(self.unexpected("`INTO` after `COPY`"))
    }

    /// Parse a `COPY` statement into [`Statement::Copy`].
    pub(super) fn parse_copy_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let copy = self.parse_copy_statement_body(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Copy {
            copy: Box::new(copy),
            meta,
        })
    }

    fn parse_copy_statement_body(&mut self, start: Span) -> ParseResult<CopyStatement<D::Ext>> {
        self.expect_contextual_keyword("COPY")?;
        // `opt_binary` is a table-source-only prefix: PostgreSQL's grammar is
        // `COPY opt_binary qualified_name ...`, so once `BINARY` is seen the source
        // must be a table (a `(` query source is rejected by `parse_object_name`).
        let binary = self.eat_contextual_keyword("BINARY")?;
        let source = if !binary && self.peek_is_punct(Punctuation::LParen)? {
            self.parse_copy_query_source()?
        } else {
            self.parse_copy_table_source()?
        };
        let is_table = matches!(source, CopySource::Table { .. });
        let direction = self.parse_copy_direction(&source)?;
        let target = self.parse_copy_target()?;
        // `copy_delimiter` and `where_clause` are table-source-only in PostgreSQL's
        // grammar (the query production carries neither), so they are parsed only for
        // the table source; after a query source they are left for the outer loop to
        // reject as unexpected.
        let delimiters = if is_table {
            self.parse_optional_copy_delimiters()?
        } else {
            None
        };
        let (parenthesized, options) = self.parse_optional_copy_options()?;
        let filter = self.parse_optional_copy_filter(direction, is_table)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyStatement {
            binary,
            source,
            direction,
            target,
            delimiters,
            parenthesized,
            options,
            filter,
            meta,
        })
    }

    // --- COPY INTO (Snowflake) ----------------------------------------------

    /// Parse a Snowflake `COPY INTO` statement into [`Statement::CopyInto`].
    pub(super) fn parse_copy_into_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let copy = self.parse_copy_into_statement_body(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::CopyInto {
            copy: Box::new(copy),
            meta,
        })
    }

    fn parse_copy_into_statement_body(
        &mut self,
        start: Span,
    ) -> ParseResult<CopyIntoStatement<D::Ext>> {
        self.expect_contextual_keyword("COPY")?;
        self.expect_contextual_keyword("INTO")?;
        let target = self.parse_copy_into_target()?;
        self.expect_contextual_keyword("FROM")?;
        let source = self.parse_copy_into_source()?;
        let options = self.parse_copy_into_option_list()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyIntoStatement {
            target,
            source,
            options,
            meta,
        })
    }

    /// Parse the `INTO <target>`: an external location string (`'s3://…'`) or a
    /// `[<db>.<schema>.]<table> [(col, ...)]` relation.
    fn parse_copy_into_target(&mut self) -> ParseResult<CopyIntoTarget> {
        let start = self.current_span()?;
        if let Some(reference) = self.try_parse_stage_reference()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyIntoTarget::Stage { reference, meta });
        }
        if let Some(location) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyIntoTarget::External { location, meta });
        }
        let table = self.parse_object_name()?;
        let columns = self.parse_optional_copy_columns()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyIntoTarget::Table {
            table,
            columns,
            meta,
        })
    }

    /// Parse the `FROM <source>`: a parenthesized transformation query, an external
    /// location string, or a `[<db>.<schema>.]<table>` relation.
    fn parse_copy_into_source(&mut self) -> ParseResult<CopyIntoSource<D::Ext>> {
        let start = self.current_span()?;
        if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let statement = self.parse_statement()?;
            // Snowflake's transformation source is a `SELECT` (`COPY INTO t FROM (SELECT
            // ... )`); a non-query inner is rejected rather than silently accepted.
            if !matches!(statement, Statement::Query { .. }) {
                return Err(
                    self.unexpected("a SELECT query inside a `COPY INTO ... FROM ( ... )` source")
                );
            }
            self.expect_punct(Punctuation::RParen, "`)` to close the COPY INTO query")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyIntoSource::Query {
                query: Box::new(statement),
                meta,
            });
        }
        if let Some(reference) = self.try_parse_stage_reference()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyIntoSource::Stage { reference, meta });
        }
        if let Some(location) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyIntoSource::External { location, meta });
        }
        let table = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyIntoSource::Table { table, meta })
    }

    /// Snowflake `@stage` / `@~` / `@%table` stage endpoint when
    /// [`UtilitySyntax::stage_references`](crate::ast::dialect::UtilitySyntax::stage_references)
    /// is on. The full stage text (including `@` and path) is interned as a bare
    /// [`Ident`] so render can re-emit the source spelling.
    fn try_parse_stage_reference(&mut self) -> ParseResult<Option<Ident>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        if token.kind != TokenKind::StageReference {
            return Ok(None);
        }
        let span = token.span;
        let text = self.span_text(span);
        let sym = self.intern_text(text);
        self.advance()?;
        Ok(Some(Ident {
            sym,
            quote: QuoteStyle::None,
            meta: self.make_meta(span),
        }))
    }

    /// Parse the trailing space-separated `<name> = <value>` option list (`FILE_FORMAT
    /// = (...)`, `FILES = (...)`, `PATTERN = '...'`, `VALIDATION_MODE = ...`, copy
    /// options). The list runs until a token that does not start an option (`;`, end of
    /// input, or anything else the outer statement loop rejects).
    fn parse_copy_into_option_list(&mut self) -> ParseResult<ThinVec<CopyOption>> {
        let mut options = ThinVec::new();
        while self.peek_starts_copy_into_option()? {
            options.push(self.parse_copy_into_option()?);
        }
        Ok(options)
    }

    /// Whether the current token starts a `COPY INTO` option: a name word immediately
    /// followed by `=`. Anything else ends the option list.
    fn peek_starts_copy_into_option(&mut self) -> ParseResult<bool> {
        let is_word = matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Word | TokenKind::Keyword(_))
        );
        Ok(is_word && self.peek_nth_is_op(1, Operator::Eq)?)
    }

    /// Parse one `<name> = <value>` option. The name is a `ColLabel` (a keyword such as
    /// `TYPE` is admitted), matching the outer and nested-list grammars.
    fn parse_copy_into_option(&mut self) -> ParseResult<CopyOption> {
        let start = self.current_span()?;
        let name = self.parse_as_alias_ident()?;
        if !self.eat_op(Operator::Eq)? {
            return Err(self.unexpected("`=` after a COPY INTO option name"));
        }
        let value = Some(self.parse_copy_into_option_value()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyOption { name, value, meta })
    }

    /// Parse a `COPY INTO` option value: a nested `( <name> = <value> ... )` option
    /// list (`FILE_FORMAT`), or the generic value grammar shared with PostgreSQL COPY
    /// (a bare word, string, number, `*`, or comma-separated `( ... )` value list such
    /// as `FILES`).
    fn parse_copy_into_option_value(&mut self) -> ParseResult<CopyOptionValue> {
        // A `(` opening a `<name> =` pair is the nested keyed option list; a `(`
        // opening bare comma-separated values (`FILES = ('a', 'b')`) is the generic
        // `List`, so it falls through to the shared value parser below. The `=` sits
        // two tokens past the `(` (`( <name> = ...`).
        if self.peek_is_punct(Punctuation::LParen)? && self.peek_nth_is_op(2, Operator::Eq)? {
            let start = self.current_span()?;
            self.advance()?; // `(`
            let mut options = ThinVec::new();
            while !self.peek_is_punct(Punctuation::RParen)? {
                options.push(self.parse_copy_into_option()?);
            }
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the COPY INTO nested option list",
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyOptionValue::OptionList { options, meta });
        }
        self.parse_copy_generic_option_value()
    }

    /// Parse the optional `[USING] DELIMITERS '<str>'` clause (PostgreSQL
    /// `copy_delimiter`), between the endpoint and the option list. The optional
    /// `USING` is not load-bearing (PostgreSQL `opt_using`) and is dropped;
    /// `DELIMITERS` (plural) is distinct from the singular `DELIMITER` option.
    fn parse_optional_copy_delimiters(&mut self) -> ParseResult<Option<Literal>> {
        // A leading `USING` here can only introduce `DELIMITERS`.
        if self.eat_contextual_keyword("USING")? {
            self.expect_contextual_keyword("DELIMITERS")?;
            return Ok(Some(
                self.expect_string_literal("a string after `USING DELIMITERS`")?,
            ));
        }
        if self.eat_contextual_keyword("DELIMITERS")? {
            return Ok(Some(
                self.expect_string_literal("a string after `DELIMITERS`")?,
            ));
        }
        Ok(None)
    }

    /// Parse the optional `WHERE <predicate>` row filter of a `COPY FROM`.
    ///
    /// PostgreSQL admits the filter for `COPY FROM` on a table source only: it
    /// errors "WHERE clause not allowed with COPY TO", and the query source
    /// (always `TO`) has no `where_clause` production at all. Both rejections are
    /// surfaced here with one diagnostic rather than deferred to the outer loop.
    fn parse_optional_copy_filter(
        &mut self,
        direction: CopyDirection,
        is_table: bool,
    ) -> ParseResult<Option<Expr<D::Ext>>> {
        if !self.peek_is_keyword(Keyword::Where)? {
            return Ok(None);
        }
        if !is_table || direction == CopyDirection::To {
            return Err(self.unexpected(
                "the end of the COPY statement; a WHERE filter is only allowed with COPY FROM",
            ));
        }
        self.advance()?; // `WHERE`
        Ok(Some(self.parse_expr()?))
    }

    /// Parse the table row source: `<table> [(col, ...)]`.
    fn parse_copy_table_source(&mut self) -> ParseResult<CopySource<D::Ext>> {
        let start = self.current_span()?;
        let table = self.parse_object_name()?;
        let columns = self.parse_optional_copy_columns()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopySource::Table {
            table,
            columns,
            meta,
        })
    }

    /// Parse the query row source: `( <query> )`.
    ///
    /// PostgreSQL's grammar is `( PreparableStmt )` — a `SELECT`/`INSERT`/`UPDATE`/
    /// `DELETE`/`MERGE`. The inner is parsed as a full statement and then gated to
    /// the preparable kinds we model, so a non-preparable inner (`COPY (CREATE ...)
    /// TO`) is rejected like PostgreSQL rather than silently accepted.
    fn parse_copy_query_source(&mut self) -> ParseResult<CopySource<D::Ext>> {
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the COPY query")?;
        let statement = self.parse_statement()?;
        if !matches!(
            statement,
            Statement::Query { .. }
                | Statement::Insert { .. }
                | Statement::Update { .. }
                | Statement::Delete { .. }
        ) {
            return Err(self.unexpected(
                "a SELECT, VALUES, INSERT, UPDATE, or DELETE query inside `COPY ( ... )`",
            ));
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the COPY query")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopySource::Query {
            query: Box::new(statement),
            meta,
        })
    }

    /// Parse the transfer direction. The table source admits `FROM` or `TO`; the
    /// query source is `TO`-only (PostgreSQL forbids `COPY (<query>) FROM`).
    fn parse_copy_direction(&mut self, source: &CopySource<D::Ext>) -> ParseResult<CopyDirection> {
        if matches!(source, CopySource::Query { .. }) {
            self.expect_contextual_keyword("TO")?;
            return Ok(CopyDirection::To);
        }
        if self.eat_contextual_keyword("FROM")? {
            Ok(CopyDirection::From)
        } else if self.eat_contextual_keyword("TO")? {
            Ok(CopyDirection::To)
        } else {
            Err(self.unexpected("`FROM` or `TO`"))
        }
    }

    /// Parse an optional `( <column> [, ...] )` column list.
    fn parse_optional_copy_columns(&mut self) -> ParseResult<ThinVec<Ident>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(ThinVec::new());
        }
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the COPY column list")?;
        Ok(columns)
    }

    /// Parse the data endpoint: `'file'`, `PROGRAM 'cmd'`, `STDIN`, or `STDOUT`.
    ///
    /// `STDIN`/`STDOUT` are accepted for either direction: PostgreSQL's grammar does
    /// not bind them to `FROM`/`TO` (the mismatch is a later semantic check), so the
    /// exact keyword is preserved verbatim rather than gated here.
    fn parse_copy_target(&mut self) -> ParseResult<CopyTarget> {
        let start = self.current_span()?;
        if let Some(path) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyTarget::File { path, meta });
        }
        if self.eat_contextual_keyword("PROGRAM")? {
            let command = self.expect_string_literal("a command string after `PROGRAM`")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyTarget::Program { command, meta });
        }
        if self.eat_contextual_keyword("STDIN")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyTarget::Stdin { meta });
        }
        if self.eat_contextual_keyword("STDOUT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyTarget::Stdout { meta });
        }
        Err(self.unexpected("a file name, `STDIN`, `STDOUT`, or `PROGRAM`"))
    }

    /// Parse the optional option trailer in either spelling, reporting whether the
    /// parenthesized list was used (the surface tag) alongside the options.
    ///
    /// `[WITH] ( opt [, ...] )` is the modern generic list; the legacy spelling is a
    /// space-separated `[WITH] opt ...` over PostgreSQL's fixed `copy_opt_item` set.
    /// `WITH` is optional and not load-bearing in either spelling — it canonicalizes
    /// to the parenthesized form on render — so it is consumed and dropped.
    fn parse_optional_copy_options(&mut self) -> ParseResult<(bool, ThinVec<CopyOption>)> {
        self.eat_contextual_keyword("WITH")?;
        self.parse_copy_options_trailer("`)` to close the COPY option list")
    }

    /// Parse the `copy_options` production body: a parenthesized generic list `( opt [,
    /// ...] )` or the legacy space-separated `copy_opt_list`, returning the
    /// parenthesized-spelling surface tag alongside the options. Shared by `COPY` (after
    /// its optional `WITH`) and DuckDB `EXPORT DATABASE`, whose grammar reuses the same
    /// `copy_options` production but omits the `opt_with` prefix (`EXPORT DATABASE '<p>'
    /// WITH (...)` is a parser error, probed on 1.5.4), so `EXPORT` calls this directly.
    fn parse_copy_options_trailer(
        &mut self,
        close_msg: &'static str,
    ) -> ParseResult<(bool, ThinVec<CopyOption>)> {
        if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let options = self.parse_comma_separated(Self::parse_copy_option)?;
            self.expect_punct(Punctuation::RParen, close_msg)?;
            return Ok((true, options));
        }
        Ok((false, self.parse_legacy_copy_options()?))
    }

    /// Parse one parenthesized `<name> [<value>]` option. The name is a `ColLabel`,
    /// so a keyword option name such as `NULL` is admitted (PostgreSQL's generic
    /// option grammar).
    fn parse_copy_option(&mut self) -> ParseResult<CopyOption> {
        let start = self.current_span()?;
        let name = self.parse_as_alias_ident()?;
        let value = self.parse_optional_copy_option_value()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyOption { name, value, meta })
    }

    fn parse_optional_copy_option_value(&mut self) -> ParseResult<Option<CopyOptionValue>> {
        // A value is whatever follows the name up to the next list separator.
        if self.peek_is_punct(Punctuation::Comma)? || self.peek_is_punct(Punctuation::RParen)? {
            return Ok(None);
        }
        Ok(Some(self.parse_copy_generic_option_value()?))
    }

    /// Parse one generic `copy_generic_opt_arg` value: a `*`, a parenthesized list, a
    /// signed/unsigned number, a string, or a bareword. Recurses for the list case,
    /// which is the shape DuckDB's `PARTITION_BY (y, m)` / `FORCE_QUOTE (a, b)` and
    /// PostgreSQL's generic list argument share; pg_query accepts every shape here
    /// under the plain `copy` gate, so no dialect flag guards it.
    fn parse_copy_generic_option_value(&mut self) -> ParseResult<CopyOptionValue> {
        let start = self.current_span()?;
        if self.peek_is_op(Operator::Star)? {
            self.advance()?; // `*`
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyOptionValue::Star { meta });
        }
        if self.peek_is_punct(Punctuation::LParen)? {
            self.advance()?; // `(`
            let values = self.parse_comma_separated(Self::parse_copy_generic_option_value)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the COPY option argument list",
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyOptionValue::List { values, meta });
        }
        // A leading sign binds only to a numeric value (PostgreSQL `NumericOnly` folds
        // the sign into the constant), matching the `SET` generic value parser.
        if self.peek_is_op(Operator::Minus)? || self.peek_is_op(Operator::Plus)? {
            let sign_span = self.current_span()?;
            self.advance()?; // sign
            let number = self
                .peek()?
                .filter(|token| token.kind == TokenKind::Number)
                .ok_or_else(|| self.unexpected("a number after a sign in a COPY option value"))?;
            self.advance()?;
            let span = sign_span.union(number.span);
            let value = Literal {
                kind: number_literal_kind(self.span_text(span), self.float_as_decimal_enabled()),
                meta: self.make_meta(span),
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyOptionValue::Number { value, meta });
        }
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::Number {
                self.advance()?;
                let value = Literal {
                    kind: number_literal_kind(
                        self.span_text(token.span),
                        self.float_as_decimal_enabled(),
                    ),
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(CopyOptionValue::Number { value, meta });
            }
        }
        if let Some(value) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CopyOptionValue::String { value, meta });
        }
        let word = self.parse_as_alias_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyOptionValue::Word { word, meta })
    }

    /// Parse the legacy un-parenthesized option list (PostgreSQL `copy_opt_list`).
    ///
    /// Unlike the generic parenthesized grammar, the legacy list is a fixed keyword
    /// set whose options each have a known arity; that arity is what delimits one
    /// option from the next (`CSV HEADER` is two bare options, not `CSV` with the
    /// argument `HEADER`), so the list cannot be parsed with the generic grammar. It
    /// runs until a token that does not start a legacy option — `;`, end of input,
    /// or anything else, which the surrounding statement loop then rejects.
    fn parse_legacy_copy_options(&mut self) -> ParseResult<ThinVec<CopyOption>> {
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_legacy_copy_option()? {
            options.push(option);
        }
        Ok(options)
    }

    /// Parse one legacy option, or `Ok(None)` when the current token does not start
    /// one (ending the list).
    fn parse_legacy_copy_option(&mut self) -> ParseResult<Option<CopyOption>> {
        let start = self.current_span()?;
        // String-argument options: `DELIMITER|NULL|QUOTE|ESCAPE [AS] '<str>'`. The
        // `AS` is optional (PostgreSQL `opt_as`) and not load-bearing.
        if self.peek_is_contextual_keyword("DELIMITER")?
            || self.peek_is_contextual_keyword("NULL")?
            || self.peek_is_contextual_keyword("QUOTE")?
            || self.peek_is_contextual_keyword("ESCAPE")?
        {
            let name = self.parse_as_alias_ident()?;
            self.eat_contextual_keyword("AS")?;
            let value = self.parse_copy_string_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CopyOption {
                name,
                value: Some(value),
                meta,
            }));
        }
        // `ENCODING '<str>'` takes a string but, unlike the above, admits no `AS`.
        if self.peek_is_contextual_keyword("ENCODING")? {
            let name = self.parse_as_alias_ident()?;
            let value = self.parse_copy_string_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CopyOption {
                name,
                value: Some(value),
                meta,
            }));
        }
        // Bare keyword options.
        if self.peek_is_contextual_keyword("BINARY")?
            || self.peek_is_contextual_keyword("FREEZE")?
            || self.peek_is_contextual_keyword("CSV")?
            || self.peek_is_contextual_keyword("HEADER")?
        {
            let name = self.parse_as_alias_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CopyOption {
                name,
                value: None,
                meta,
            }));
        }
        // `FORCE {QUOTE | NULL | NOT NULL} {<cols> | *}` — the compound-keyword
        // column-list options. The `FORCE` word is the option name; the sub-keyword
        // and target ride the `Force` value.
        if self.peek_is_contextual_keyword("FORCE")? {
            let name = self.parse_as_alias_ident()?;
            let value = self.parse_force_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CopyOption {
                name,
                value: Some(value),
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse the `{QUOTE | NULL | NOT NULL} {<column-list> | *}` tail of a `FORCE`
    /// option (the `FORCE` keyword is already consumed).
    fn parse_force_option_value(&mut self) -> ParseResult<CopyOptionValue> {
        let start = self.current_span()?;
        let kind = if self.eat_contextual_keyword("QUOTE")? {
            ForceKind::Quote
        } else if self.eat_contextual_keyword("NOT")? {
            self.expect_contextual_keyword("NULL")?;
            ForceKind::NotNull
        } else if self.eat_contextual_keyword("NULL")? {
            ForceKind::Null
        } else {
            return Err(self.unexpected("`QUOTE`, `NULL`, or `NOT NULL` after `FORCE`"));
        };
        let columns = self.parse_force_columns()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyOptionValue::Force {
            kind,
            columns,
            meta,
        })
    }

    /// Parse a `FORCE` option's target: `*` (all columns, yielding an empty list) or
    /// an un-parenthesized comma-separated column list (PostgreSQL `columnList`,
    /// always non-empty).
    fn parse_force_columns(&mut self) -> ParseResult<ThinVec<Ident>> {
        if self.peek_is_op(Operator::Star)? {
            self.advance()?; // `*`
            return Ok(ThinVec::new());
        }
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        Ok(columns)
    }

    fn parse_copy_string_option_value(&mut self) -> ParseResult<CopyOptionValue> {
        let start = self.current_span()?;
        let value = self.expect_string_literal("a string value for the COPY option")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CopyOptionValue::String { value, meta })
    }

    // --- EXPLAIN / DESCRIBE -------------------------------------------------

    /// Parse an `EXPLAIN` statement, or — under
    /// [`ShowSyntax::describe`](crate::ast::dialect::UtilitySyntax) — one of MySQL's
    /// `DESCRIBE`/`DESC` synonyms, in either the query-plan form ([`Statement::Explain`],
    /// spelling-tagged) or the table-metadata form ([`Statement::Describe`]).
    ///
    /// MySQL treats all three keyword spellings as synonyms for both forms, so they share
    /// this one entry. With the gate off (ANSI/PostgreSQL/SQLite) only `EXPLAIN` is routed
    /// here and only the query-plan form is taken — a table after `EXPLAIN` is rejected, as
    /// PostgreSQL does. With it on, a leading table name selects the table-metadata form
    /// ([`peek_starts_describe_table`](Self::peek_starts_describe_table) disambiguates
    /// table name from explainable statement).
    pub(super) fn parse_explain_or_describe_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let keyword = self.parse_explain_keyword()?;
        if self.features().show_syntax.describe && self.peek_starts_describe_table()? {
            return self.parse_describe_table_statement(start, keyword);
        }
        let (parenthesized, options) = if self.peek_is_punct(Punctuation::LParen)? {
            (true, self.parse_explain_option_list()?)
        } else {
            (false, self.parse_legacy_explain_options()?)
        };
        let statement = self.parse_statement()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Explain {
            explain: Box::new(ExplainStatement {
                spelling: keyword,
                parenthesized,
                options,
                statement: Box::new(statement),
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Consume the leading `EXPLAIN`/`DESCRIBE`/`DESC` keyword and report which was
    /// written. The dispatcher only routes `DESCRIBE`/`DESC` here under the `describe`
    /// gate, so those spellings never reach a dialect that lacks them.
    fn parse_explain_keyword(&mut self) -> ParseResult<ExplainKeyword> {
        if self.eat_contextual_keyword("DESCRIBE")? {
            Ok(ExplainKeyword::Describe)
        } else if self.eat_contextual_keyword("DESC")? {
            Ok(ExplainKeyword::Desc)
        } else {
            self.expect_contextual_keyword("EXPLAIN")?;
            Ok(ExplainKeyword::Explain)
        }
    }

    /// True when, under the MySQL `describe` gate, what follows the EXPLAIN-family keyword
    /// is a table name (the table-metadata form) rather than an explainable statement or an
    /// EXPLAIN option (the query-plan form).
    ///
    /// The query-plan form leads with `(` (the parenthesized option list), the `ANALYZE`
    /// option, or a reserved statement keyword such as `SELECT`; the table-metadata form
    /// leads with a bare `table_ident`. `ANALYZE`/`ANALYSE` are guarded explicitly because
    /// they are the one shared option keyword that is otherwise a legal identifier; every
    /// other query leader (`SELECT`/`INSERT`/…) is reserved, so
    /// [`peek_can_start_column_name`](Self::peek_can_start_column_name) already excludes it.
    fn peek_starts_describe_table(&mut self) -> ParseResult<bool> {
        if self.peek_is_punct(Punctuation::LParen)?
            || self.peek_is_contextual_keyword("ANALYZE")?
            || self.peek_is_contextual_keyword("ANALYSE")?
        {
            return Ok(false);
        }
        self.peek_can_start_column_name()
    }

    /// Parse the MySQL `{DESCRIBE | DESC | EXPLAIN} <table> [<column> | '<pattern>']`
    /// table-metadata form into [`Statement::Describe`] (the keyword already consumed).
    fn parse_describe_table_statement(
        &mut self,
        start: Span,
        keyword: ExplainKeyword,
    ) -> ParseResult<Statement<D::Ext>> {
        let table = self.parse_object_name()?;
        let column = self.parse_optional_describe_column()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Describe {
            describe: Box::new(DescribeStatement {
                keyword,
                table,
                column,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse the optional `<column> | '<pattern>'` narrowing after the described table:
    /// a single identifier names one column, a string is a `LIKE` pattern (MySQL
    /// `opt_describe_column`). A dotted name, a `*`, or a second argument is left
    /// unconsumed and surfaces as a trailing-input error, matching MySQL.
    fn parse_optional_describe_column(&mut self) -> ParseResult<Option<DescribeColumn>> {
        let start = self.current_span()?;
        if let Some(pattern) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DescribeColumn::Wild { pattern, meta }));
        }
        if self.peek_can_start_column_name()? {
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(DescribeColumn::Name { name, meta }));
        }
        Ok(None)
    }

    // --- KILL (MySQL) -------------------------------------------------------

    /// Parse a MySQL `KILL [CONNECTION | QUERY] <id>` statement into [`Statement::Kill`],
    /// reached under [`UtilitySyntax::kill`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The optional `CONNECTION`/`QUERY` scope keyword (MySQL `kill_option`) is consumed
    /// when it leads, then the thread id is a full expression — MySQL's grammar is
    /// `KILL kill_option expr`, so `KILL 5`, `KILL '5'`, `KILL @id`, and `KILL 1 + 1` all
    /// prepare — reusing [`parse_expr`](Self::parse_expr). A bare `KILL` /
    /// `KILL CONNECTION` with no id, and trailing input after the id, are left for the
    /// statement loop to reject, matching MySQL.
    pub(super) fn parse_kill_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("KILL")?;
        let target = if self.eat_contextual_keyword("CONNECTION")? {
            KillTarget::Connection
        } else if self.eat_contextual_keyword("QUERY")? {
            KillTarget::Query
        } else {
            KillTarget::Unspecified
        };
        let id = self.parse_expr()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Kill {
            kill: Box::new(KillStatement {
                target,
                id,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- SHOW TABLES (MySQL / DuckDB) ---------------------------------------

    /// True when a top-level `SHOW` opens the typed `SHOW TABLES` statement — the current
    /// token is `SHOW` and, past the optional `EXTENDED`/`FULL`/`ALL` modifiers, the next
    /// word is `TABLES`.
    ///
    /// The lookahead insists on the `TABLES` keyword rather than dispatching on the
    /// modifiers alone: `SHOW ALL` (list every setting) and `SHOW FULL` are generic
    /// session `SHOW`s that must fall through to
    /// [`parse_session_statement`](Self::parse_session_statement), so only `SHOW ALL
    /// TABLES` is claimed here. No shipped dialect writes more than two modifiers before
    /// `TABLES` (MySQL `EXTENDED FULL`, DuckDB `ALL`), so a three-deep scan is exhaustive.
    pub(super) fn peek_starts_show_tables(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        let is_modifier = |parser: &mut Self, n: usize| -> ParseResult<bool> {
            Ok(parser.peek_nth_is_contextual_keyword(n, "EXTENDED")?
                || parser.peek_nth_is_contextual_keyword(n, "FULL")?
                || parser.peek_nth_is_contextual_keyword(n, "ALL")?)
        };
        if self.peek_nth_is_contextual_keyword(1, "TABLES")? {
            return Ok(true);
        }
        if is_modifier(self, 1)? && self.peek_nth_is_contextual_keyword(2, "TABLES")? {
            return Ok(true);
        }
        Ok(is_modifier(self, 1)?
            && is_modifier(self, 2)?
            && self.peek_nth_is_contextual_keyword(3, "TABLES")?)
    }

    /// Parse a typed `SHOW [EXTENDED] [FULL] [ALL] TABLES [{FROM | IN} <db>] [LIKE
    /// '<pat>' | WHERE <expr>]` statement into [`Statement::Show`], reached under
    /// [`ShowSyntax::show_tables`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The leading modifiers are consumed in any order (each at most once) so the MySQL
    /// `EXTENDED FULL` and DuckDB `ALL` spellings both parse; the single flag admits the
    /// modifier union permissively, the DESCRIBE/PRAGMA single-flag-utility precedent.
    pub(super) fn parse_show_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        let mut extended = false;
        let mut full = false;
        let mut all = false;
        loop {
            if !extended && self.eat_contextual_keyword("EXTENDED")? {
                extended = true;
            } else if !full && self.eat_contextual_keyword("FULL")? {
                full = true;
            } else if !all && self.eat_contextual_keyword("ALL")? {
                all = true;
            } else {
                break;
            }
        }
        self.expect_contextual_keyword("TABLES")?;
        let from = self.parse_optional_show_from()?;
        let filter = self.parse_optional_show_filter()?;
        let span = start.union(self.preceding_span());
        let target = ShowTarget::Tables {
            extended,
            full,
            all,
            from,
            filter,
            meta: self.make_meta(span),
        };
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- SHOW COLUMNS (MySQL) -----------------------------------------------

    /// True when a top-level `SHOW` opens the typed `SHOW COLUMNS` statement — the current
    /// token is `SHOW` and, past the optional `EXTENDED`/`FULL` modifiers, the next word is
    /// `COLUMNS` or its `FIELDS` synonym.
    ///
    /// Like [`peek_starts_show_tables`](Self::peek_starts_show_tables), the lookahead insists
    /// on the listing keyword so every other `SHOW <var>` (including a bare `SHOW FULL`)
    /// still falls through to the session statement. No `SHOW COLUMNS` form writes more than
    /// two modifiers (`EXTENDED FULL`) before the keyword, so a three-deep scan is exhaustive.
    pub(super) fn peek_starts_show_columns(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        let is_modifier = |parser: &mut Self, n: usize| -> ParseResult<bool> {
            Ok(parser.peek_nth_is_contextual_keyword(n, "EXTENDED")?
                || parser.peek_nth_is_contextual_keyword(n, "FULL")?)
        };
        let is_keyword = |parser: &mut Self, n: usize| -> ParseResult<bool> {
            Ok(parser.peek_nth_is_contextual_keyword(n, "COLUMNS")?
                || parser.peek_nth_is_contextual_keyword(n, "FIELDS")?)
        };
        if is_keyword(self, 1)? {
            return Ok(true);
        }
        if is_modifier(self, 1)? && is_keyword(self, 2)? {
            return Ok(true);
        }
        Ok(is_modifier(self, 1)? && is_modifier(self, 2)? && is_keyword(self, 3)?)
    }

    /// Parse a typed `SHOW [EXTENDED] [FULL] {COLUMNS | FIELDS} {FROM | IN} <tbl>
    /// [{FROM | IN} <db>] [LIKE '<pat>' | WHERE <expr>]` statement into
    /// [`Statement::Show`], reached under
    /// [`ShowSyntax::show_columns`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The first `{FROM | IN}` qualifier is mandatory (it names the table); a second,
    /// optional one names the database. `FIELDS` is an exact synonym of `COLUMNS`, recorded
    /// on the [`ShowColumnsSpelling`] tag so the written keyword round-trips.
    pub(super) fn parse_show_columns_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        let mut extended = false;
        let mut full = false;
        loop {
            if !extended && self.eat_contextual_keyword("EXTENDED")? {
                extended = true;
            } else if !full && self.eat_contextual_keyword("FULL")? {
                full = true;
            } else {
                break;
            }
        }
        let spelling = if self.eat_contextual_keyword("COLUMNS")? {
            ShowColumnsSpelling::Columns
        } else {
            self.expect_contextual_keyword("FIELDS")?;
            ShowColumnsSpelling::Fields
        };
        let table = self
            .parse_optional_show_from()?
            .ok_or_else(|| self.unexpected("`FROM` or `IN` naming the table"))?;
        let database = self.parse_optional_show_from()?;
        let filter = self.parse_optional_show_filter()?;
        let span = start.union(self.preceding_span());
        let target = ShowTarget::Columns {
            extended,
            full,
            spelling,
            table,
            database,
            filter,
            meta: self.make_meta(span),
        };
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- SHOW CREATE TABLE (MySQL) ------------------------------------------

    /// True when a top-level `SHOW` opens the typed `SHOW CREATE TABLE` statement — the
    /// current token is `SHOW` and the next two words are `CREATE TABLE`.
    ///
    /// The lookahead insists on *both* keywords so a bare `SHOW create` (a generic session
    /// `SHOW <var>` reading `create` as the variable name, as PostgreSQL does) still falls
    /// through to the session statement. This subform has no `EXTENDED`/`FULL` modifiers
    /// (MySQL docs), so a two-deep scan is exhaustive.
    pub(super) fn peek_starts_show_create_table(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        Ok(self.peek_nth_is_contextual_keyword(1, "CREATE")?
            && self.peek_nth_is_contextual_keyword(2, "TABLE")?)
    }

    /// Parse a typed `SHOW CREATE TABLE <tbl>` statement into [`Statement::Show`], reached
    /// under [`ShowSyntax::show_create_table`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The two `CREATE TABLE` keywords are fixed and the single operand is the target table
    /// (schema-qualifiable). This gate handles only the `TABLE` object kind; the other MySQL
    /// `SHOW CREATE …` kinds (including `SHOW CREATE USER`) ride the `show_admin` family
    /// dispatch (`finish_show_create`).
    pub(super) fn parse_show_create_table_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        self.expect_contextual_keyword("CREATE")?;
        self.expect_contextual_keyword("TABLE")?;
        let name = self.parse_object_name()?;
        let span = start.union(self.preceding_span());
        let target = ShowTarget::Create {
            kind: ShowCreateKind::Table,
            name,
            if_not_exists: false,
            meta: self.make_meta(span),
        };
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- SHOW FUNCTIONS (Spark / Databricks) --------------------------------

    /// True when a top-level `SHOW` opens the typed `SHOW FUNCTIONS` statement — the current
    /// token is `SHOW` and, past the optional `USER`/`SYSTEM`/`ALL` scope keyword, the next
    /// word is `FUNCTIONS`.
    ///
    /// The lookahead insists on the `FUNCTIONS` keyword so every other `SHOW <var>` — and a
    /// bare `SHOW ALL` (the `ALL` scope with no `FUNCTIONS`) — still falls through to the
    /// session statement. No form writes more than one scope keyword before `FUNCTIONS`, so
    /// a two-deep scan is exhaustive.
    pub(super) fn peek_starts_show_functions(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        if self.peek_nth_is_contextual_keyword(1, "FUNCTIONS")? {
            return Ok(true);
        }
        let is_scope = |parser: &mut Self, n: usize| -> ParseResult<bool> {
            Ok(parser.peek_nth_is_contextual_keyword(n, "USER")?
                || parser.peek_nth_is_contextual_keyword(n, "SYSTEM")?
                || parser.peek_nth_is_contextual_keyword(n, "ALL")?)
        };
        Ok(is_scope(self, 1)? && self.peek_nth_is_contextual_keyword(2, "FUNCTIONS")?)
    }

    /// Parse a typed `SHOW [{USER | SYSTEM | ALL}] FUNCTIONS [{FROM | IN} <schema>]
    /// [[LIKE] {<function_name> | '<regex>'}]` statement into [`Statement::Show`], reached
    /// under [`ShowSyntax::show_functions`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The optional scope keyword precedes `FUNCTIONS`; the optional `{FROM | IN}` schema
    /// qualifier and the optional `[LIKE] {name | regex}` narrowing follow it (Spark /
    /// Databricks).
    pub(super) fn parse_show_functions_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        let kind = if self.eat_contextual_keyword("USER")? {
            Some(ShowFunctionsScope::User)
        } else if self.eat_contextual_keyword("SYSTEM")? {
            Some(ShowFunctionsScope::System)
        } else if self.eat_contextual_keyword("ALL")? {
            Some(ShowFunctionsScope::All)
        } else {
            None
        };
        self.expect_contextual_keyword("FUNCTIONS")?;
        let from = self.parse_optional_show_from()?;
        let filter = self.parse_optional_show_functions_filter()?;
        let span = start.union(self.preceding_span());
        let target = ShowTarget::Functions {
            kind,
            from,
            filter,
            meta: self.make_meta(span),
        };
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse the optional trailing `[LIKE] {<function_name> | '<regex_pattern>'}` narrowing
    /// of `SHOW FUNCTIONS`; `None` when no pattern follows.
    ///
    /// The `LIKE` keyword is optional in the grammar and the operand is either a bare
    /// (optionally qualified) function name or a quoted regex string. A string literal is a
    /// regex pattern; otherwise a leading name token (or a consumed `LIKE`, which then
    /// *requires* a pattern) is a function name. With no `LIKE` and no leading name token
    /// there is no filter.
    fn parse_optional_show_functions_filter(&mut self) -> ParseResult<Option<ShowFunctionsFilter>> {
        let start = self.current_span()?;
        let like = self.eat_contextual_keyword("LIKE")?;
        if let Some(pattern) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ShowFunctionsFilter::Regex {
                like,
                pattern,
                meta,
            }));
        }
        let leads_name = match self.peek()? {
            Some(token) => self.token_admissible(token, self.features().reserved_column_name),
            None => false,
        };
        if like || leads_name {
            let name = self.parse_object_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ShowFunctionsFilter::Name { like, name, meta }));
        }
        Ok(None)
    }

    // --- SHOW {FUNCTION | PROCEDURE} STATUS (MySQL) -------------------------

    /// True when a top-level `SHOW` opens the typed `SHOW {FUNCTION | PROCEDURE} STATUS`
    /// stored-routine listing — the current token is `SHOW` and the next two words are
    /// `FUNCTION STATUS` or `PROCEDURE STATUS`.
    ///
    /// The lookahead insists on *both* the object keyword and the trailing `STATUS` so the
    /// seam steals only the full `FUNCTION STATUS` / `PROCEDURE STATUS` two-keyword prefix.
    /// `FUNCTION`/`PROCEDURE` are reserved keywords, so a bare `SHOW FUNCTION` (no `STATUS`)
    /// is *not* a generic session `SHOW <var>` — the reserved word cannot name a variable, so
    /// it is a parse error both with the flag on and off, exactly as `SHOW CREATE` is (the
    /// reserved `CREATE`). This subform has no scope keyword and no `{FROM | IN}` qualifier
    /// (MySQL rejects `SHOW FUNCTION STATUS FROM db`), so a two-deep scan is exhaustive.
    pub(super) fn peek_starts_show_routine_status(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        Ok((self.peek_nth_is_contextual_keyword(1, "FUNCTION")?
            || self.peek_nth_is_contextual_keyword(1, "PROCEDURE")?)
            && self.peek_nth_is_contextual_keyword(2, "STATUS")?)
    }

    /// Parse a typed `SHOW {FUNCTION | PROCEDURE} STATUS [LIKE '<pat>' | WHERE <expr>]`
    /// statement into [`Statement::Show`], reached under
    /// [`ShowSyntax::show_routine_status`](crate::ast::dialect::ShowSyntax).
    ///
    /// The singular `FUNCTION`/`PROCEDURE` object keyword rides the [`ShowRoutineKind`]
    /// surface tag; the mandatory `STATUS` keyword is fixed, and the only trailing operand is
    /// the optional `LIKE '<pat>'` / `WHERE <expr>` narrowing, reusing the shared
    /// [`parse_optional_show_filter`](Self::parse_optional_show_filter) (MySQL).
    pub(super) fn parse_show_routine_status_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        let kind = if self.eat_contextual_keyword("FUNCTION")? {
            ShowRoutineKind::Function
        } else {
            self.expect_contextual_keyword("PROCEDURE")?;
            ShowRoutineKind::Procedure
        };
        self.expect_contextual_keyword("STATUS")?;
        let filter = self.parse_optional_show_filter()?;
        let span = start.union(self.preceding_span());
        let target = ShowTarget::RoutineStatus {
            kind,
            filter,
            meta: self.make_meta(span),
        };
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- SHOW server-administration / catalogue family (MySQL) --------------

    /// True when a top-level `SHOW` opens the MySQL server-administration / catalogue
    /// family — the current token is `SHOW` and, past any leading modifier, the next word
    /// is one of the family's lead keywords. Reached under
    /// [`ShowSyntax::show_admin`](crate::ast::dialect::ShowSyntax::show_admin), after the
    /// individually-gated `TABLES`/`COLUMNS`/`CREATE TABLE`/`{FUNCTION|PROCEDURE} STATUS`
    /// lookaheads have each had their turn.
    ///
    /// The lookahead insists on a family lead keyword so every unrelated `SHOW <var>` still
    /// falls through to [`parse_session_statement`](Self::parse_session_statement), keeping
    /// the seams MECE. The ambiguous leads are disambiguated by a second keyword: `TABLE`
    /// only when `STATUS` follows (`TABLES` is the earlier `show_tables` seam), the scope /
    /// `FULL` / `STORAGE` / `EXTENDED` modifiers only when their matching keyword follows,
    /// and `PROCEDURE`/`FUNCTION` only when `CODE` follows (`… STATUS` is the earlier
    /// `show_routine_status` seam).
    pub(super) fn peek_starts_show_admin(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("SHOW")?);
        // Single-keyword leads: the token right after SHOW settles it.
        const SOLE_LEADS: &[&str] = &[
            "DATABASES",
            "SCHEMAS",
            "COLLATION",
            "CHARSET",
            "CHARACTER",
            "STATUS",
            "VARIABLES",
            "EVENTS",
            "TRIGGERS",
            "PLUGINS",
            "ENGINES",
            "ENGINE",
            "PRIVILEGES",
            "PROFILE",
            "PROFILES",
            "PROCESSLIST",
            "REPLICA",
            "REPLICAS",
            "INDEX",
            "INDEXES",
            "KEYS",
            "GRANTS",
            "WARNINGS",
            "ERRORS",
            "COUNT",
            "CREATE",
            "BINARY",
            "BINLOG",
            "RELAYLOG",
            "OPEN",
        ];
        for lead in SOLE_LEADS {
            if self.peek_nth_is_contextual_keyword(1, lead)? {
                return Ok(true);
            }
        }
        // `TABLE STATUS` — bare `TABLE`/`TABLES` belong to the earlier `show_tables` seam.
        if self.peek_nth_is_contextual_keyword(1, "TABLE")?
            && self.peek_nth_is_contextual_keyword(2, "STATUS")?
        {
            return Ok(true);
        }
        // Modifier-led forms: the modifier alone is ambiguous, so require its keyword.
        if (self.peek_nth_is_contextual_keyword(1, "GLOBAL")?
            || self.peek_nth_is_contextual_keyword(1, "SESSION")?
            || self.peek_nth_is_contextual_keyword(1, "LOCAL")?)
            && (self.peek_nth_is_contextual_keyword(2, "STATUS")?
                || self.peek_nth_is_contextual_keyword(2, "VARIABLES")?)
        {
            return Ok(true);
        }
        if self.peek_nth_is_contextual_keyword(1, "FULL")?
            && (self.peek_nth_is_contextual_keyword(2, "TRIGGERS")?
                || self.peek_nth_is_contextual_keyword(2, "PROCESSLIST")?)
        {
            return Ok(true);
        }
        if self.peek_nth_is_contextual_keyword(1, "STORAGE")?
            && self.peek_nth_is_contextual_keyword(2, "ENGINES")?
        {
            return Ok(true);
        }
        if self.peek_nth_is_contextual_keyword(1, "EXTENDED")?
            && (self.peek_nth_is_contextual_keyword(2, "INDEX")?
                || self.peek_nth_is_contextual_keyword(2, "INDEXES")?
                || self.peek_nth_is_contextual_keyword(2, "KEYS")?)
        {
            return Ok(true);
        }
        // `{PROCEDURE | FUNCTION} CODE` — bare `… STATUS` is the earlier routine-status seam.
        if (self.peek_nth_is_contextual_keyword(1, "PROCEDURE")?
            || self.peek_nth_is_contextual_keyword(1, "FUNCTION")?)
            && self.peek_nth_is_contextual_keyword(2, "CODE")?
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// Parse a MySQL server-administration / catalogue-introspection `SHOW` sub-command into
    /// [`Statement::Show`], reached under
    /// [`ShowSyntax::show_admin`](crate::ast::dialect::ShowSyntax::show_admin). The
    /// sub-command is table-driven data on the [`ShowTarget`] axis, not one arm per keyword.
    pub(super) fn parse_show_admin_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHOW")?;
        let target = self.parse_show_admin_target(start)?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Show {
            show: Box::new(ShowStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Dispatch the sub-command after `SHOW` has been consumed; `start` spans the `SHOW`
    /// keyword so each branch can close the statement span at its own tail.
    fn parse_show_admin_target(&mut self, start: Span) -> ParseResult<ShowTarget<D::Ext>> {
        // Scoped `[GLOBAL | SESSION | LOCAL] {STATUS | VARIABLES}`.
        if let Some(scope) = self.parse_optional_show_scope()? {
            return self.finish_show_status_or_variables(start, Some(scope));
        }
        // `FULL {TRIGGERS | PROCESSLIST}`.
        if self.eat_contextual_keyword("FULL")? {
            if self.eat_contextual_keyword("PROCESSLIST")? {
                return Ok(self.show_bare(start, ShowBare::Processlist { full: true }));
            }
            self.expect_contextual_keyword("TRIGGERS")?;
            let from = self.parse_optional_show_from()?;
            return self.finish_show_listing(start, ShowListing::Triggers { full: true }, from);
        }
        // `STORAGE ENGINES`.
        if self.eat_contextual_keyword("STORAGE")? {
            self.expect_contextual_keyword("ENGINES")?;
            return Ok(self.show_bare(start, ShowBare::Engines { storage: true }));
        }
        // `EXTENDED {INDEX | INDEXES | KEYS} …`.
        if self.eat_contextual_keyword("EXTENDED")? {
            return self.finish_show_index(start, true);
        }
        // `CREATE {TABLE | VIEW | DATABASE | …} <name>`.
        if self.eat_contextual_keyword("CREATE")? {
            return self.finish_show_create(start);
        }
        // `ENGINE {<name> | ALL} {STATUS | MUTEX | LOGS}` (`None` engine == `ALL`).
        if self.eat_contextual_keyword("ENGINE")? {
            let engine = if self.eat_contextual_keyword("ALL")? {
                None
            } else {
                Some(self.parse_ident()?)
            };
            let artifact = if self.eat_contextual_keyword("STATUS")? {
                ShowEngineArtifact::Status
            } else if self.eat_contextual_keyword("MUTEX")? {
                ShowEngineArtifact::Mutex
            } else {
                self.expect_contextual_keyword("LOGS")?;
                ShowEngineArtifact::Logs
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ShowTarget::Engine {
                engine,
                artifact,
                meta,
            });
        }
        // Bare `STATUS` / `VARIABLES` (no scope).
        if self.peek_is_contextual_keyword("STATUS")?
            || self.peek_is_contextual_keyword("VARIABLES")?
        {
            return self.finish_show_status_or_variables(start, None);
        }
        // `{DATABASES | SCHEMAS}`.
        if self.eat_contextual_keyword("DATABASES")? {
            return self.finish_show_listing(
                start,
                ShowListing::Databases { schemas: false },
                None,
            );
        }
        if self.eat_contextual_keyword("SCHEMAS")? {
            return self.finish_show_listing(start, ShowListing::Databases { schemas: true }, None);
        }
        // `COLLATION`.
        if self.eat_contextual_keyword("COLLATION")? {
            return self.finish_show_listing(start, ShowListing::Collation, None);
        }
        // `{CHARSET | CHARACTER SET}`.
        if self.eat_contextual_keyword("CHARSET")? {
            return self.finish_show_listing(
                start,
                ShowListing::CharacterSet { charset: true },
                None,
            );
        }
        if self.eat_contextual_keyword("CHARACTER")? {
            self.expect_contextual_keyword("SET")?;
            return self.finish_show_listing(
                start,
                ShowListing::CharacterSet { charset: false },
                None,
            );
        }
        // `EVENTS [{FROM | IN} db]`.
        if self.eat_contextual_keyword("EVENTS")? {
            let from = self.parse_optional_show_from()?;
            return self.finish_show_listing(start, ShowListing::Events, from);
        }
        // `OPEN TABLES [{FROM | IN} db]`.
        if self.eat_contextual_keyword("OPEN")? {
            self.expect_contextual_keyword("TABLES")?;
            let from = self.parse_optional_show_from()?;
            return self.finish_show_listing(start, ShowListing::OpenTables, from);
        }
        // `TABLE STATUS [{FROM | IN} db]`.
        if self.eat_contextual_keyword("TABLE")? {
            self.expect_contextual_keyword("STATUS")?;
            let from = self.parse_optional_show_from()?;
            return self.finish_show_listing(start, ShowListing::TableStatus, from);
        }
        // `TRIGGERS [{FROM | IN} db]` (no `FULL`).
        if self.eat_contextual_keyword("TRIGGERS")? {
            let from = self.parse_optional_show_from()?;
            return self.finish_show_listing(start, ShowListing::Triggers { full: false }, from);
        }
        // No-operand listings.
        if self.eat_contextual_keyword("PLUGINS")? {
            return Ok(self.show_bare(start, ShowBare::Plugins));
        }
        if self.eat_contextual_keyword("ENGINES")? {
            return Ok(self.show_bare(start, ShowBare::Engines { storage: false }));
        }
        if self.eat_contextual_keyword("PRIVILEGES")? {
            return Ok(self.show_bare(start, ShowBare::Privileges));
        }
        if self.eat_contextual_keyword("PROFILES")? {
            return Ok(self.show_bare(start, ShowBare::Profiles));
        }
        if self.eat_contextual_keyword("PROCESSLIST")? {
            return Ok(self.show_bare(start, ShowBare::Processlist { full: false }));
        }
        if self.eat_contextual_keyword("REPLICAS")? {
            return Ok(self.show_bare(start, ShowBare::Replicas));
        }
        // `GRANTS [FOR <user> [USING <role list>]]`.
        if self.eat_contextual_keyword("GRANTS")? {
            return self.finish_show_grants(start);
        }
        // `PROFILE [<type list>] [FOR QUERY <n>] [LIMIT …]` — singular; plural `PROFILES` is
        // the earlier bare-listing seam.
        if self.eat_contextual_keyword("PROFILE")? {
            return self.finish_show_profile(start);
        }
        // `BINLOG EVENTS [IN '<log>'] [FROM <pos>] [LIMIT …]` (no channel).
        if self.eat_contextual_keyword("BINLOG")? {
            self.expect_contextual_keyword("EVENTS")?;
            return self.finish_show_log_events(start, false);
        }
        // `RELAYLOG EVENTS [IN '<log>'] [FROM <pos>] [LIMIT …] [FOR CHANNEL '<c>']`.
        if self.eat_contextual_keyword("RELAYLOG")? {
            self.expect_contextual_keyword("EVENTS")?;
            return self.finish_show_log_events(start, true);
        }
        // `BINARY {LOGS | LOG STATUS}`.
        if self.eat_contextual_keyword("BINARY")? {
            if self.eat_contextual_keyword("LOGS")? {
                return Ok(self.show_bare(start, ShowBare::BinaryLogs));
            }
            self.expect_contextual_keyword("LOG")?;
            self.expect_contextual_keyword("STATUS")?;
            return Ok(self.show_bare(start, ShowBare::BinaryLogStatus));
        }
        // `REPLICA STATUS [FOR CHANNEL '<c>']`.
        if self.eat_contextual_keyword("REPLICA")? {
            self.expect_contextual_keyword("STATUS")?;
            let channel = self.parse_optional_for_channel()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ShowTarget::ReplicaStatus { channel, meta });
        }
        // `[EXTENDED] {INDEX | INDEXES | KEYS} …` (the un-`EXTENDED` spelling).
        if self.peek_is_contextual_keyword("INDEX")?
            || self.peek_is_contextual_keyword("INDEXES")?
            || self.peek_is_contextual_keyword("KEYS")?
        {
            return self.finish_show_index(start, false);
        }
        // `{WARNINGS | ERRORS} [LIMIT …]`.
        if self.eat_contextual_keyword("WARNINGS")? {
            return self.finish_show_diagnostics(start, ShowDiagnosticKind::Warnings);
        }
        if self.eat_contextual_keyword("ERRORS")? {
            return self.finish_show_diagnostics(start, ShowDiagnosticKind::Errors);
        }
        // `COUNT(*) {WARNINGS | ERRORS}`.
        if self.eat_contextual_keyword("COUNT")? {
            self.expect_punct(Punctuation::LParen, "`(` after `COUNT`")?;
            self.expect_op(Operator::Star, "`*` in `COUNT(*)`")?;
            self.expect_punct(Punctuation::RParen, "`)` closing `COUNT(*)`")?;
            let kind = if self.eat_contextual_keyword("WARNINGS")? {
                ShowDiagnosticKind::Warnings
            } else {
                self.expect_contextual_keyword("ERRORS")?;
                ShowDiagnosticKind::Errors
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ShowTarget::Diagnostics {
                kind,
                count: true,
                limit: None,
                meta,
            });
        }
        // `{PROCEDURE | FUNCTION} CODE <name>` (`… STATUS` is the earlier routine-status seam).
        if self.eat_contextual_keyword("PROCEDURE")? {
            self.expect_contextual_keyword("CODE")?;
            return self.finish_show_routine_code(start, ShowRoutineKind::Procedure);
        }
        if self.eat_contextual_keyword("FUNCTION")? {
            self.expect_contextual_keyword("CODE")?;
            return self.finish_show_routine_code(start, ShowRoutineKind::Function);
        }
        Err(self.unexpected("a recognized `SHOW` sub-command"))
    }

    /// Build a no-operand [`ShowTarget::Bare`], closing the statement span at the current
    /// position.
    fn show_bare(&mut self, start: Span, kind: ShowBare) -> ShowTarget<D::Ext> {
        let meta = self.make_meta(start.union(self.preceding_span()));
        ShowTarget::Bare { kind, meta }
    }

    /// Attach the optional `{FROM | IN} <db>` qualifier and shared `[LIKE | WHERE]` tail to a
    /// [`ShowListing`] and close the statement. `from` is `None` for the members that take no
    /// qualifier (`DATABASES`, `COLLATION`, `{CHARACTER SET | CHARSET}`, `STATUS`,
    /// `VARIABLES`).
    fn finish_show_listing(
        &mut self,
        start: Span,
        kind: ShowListing,
        from: Option<ShowFrom>,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let filter = self.parse_optional_show_filter()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Listing {
            kind,
            from,
            filter,
            meta,
        })
    }

    /// Consume `{STATUS | VARIABLES}` (the keyword is not yet eaten) and finish the listing.
    fn finish_show_status_or_variables(
        &mut self,
        start: Span,
        scope: Option<ShowScope>,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let kind = if self.eat_contextual_keyword("STATUS")? {
            ShowListing::Status { scope }
        } else {
            self.expect_contextual_keyword("VARIABLES")?;
            ShowListing::Variables { scope }
        };
        self.finish_show_listing(start, kind, None)
    }

    /// Parse `{INDEX | INDEXES | KEYS} {FROM | IN} <tbl> [{FROM | IN} <db>] [WHERE <expr>]`;
    /// the `EXTENDED` modifier is already consumed and reported via `extended`.
    fn finish_show_index(
        &mut self,
        start: Span,
        extended: bool,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let spelling = if self.eat_contextual_keyword("INDEXES")? {
            ShowIndexSpelling::Indexes
        } else if self.eat_contextual_keyword("INDEX")? {
            ShowIndexSpelling::Index
        } else {
            self.expect_contextual_keyword("KEYS")?;
            ShowIndexSpelling::Keys
        };
        let table = self
            .parse_optional_show_from()?
            .ok_or_else(|| self.unexpected("`FROM` or `IN` naming the table"))?;
        let database = self.parse_optional_show_from()?;
        // The index grammar admits only a `WHERE` narrowing (no `LIKE`).
        let filter = self.parse_optional_show_where()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Index {
            spelling,
            extended,
            table,
            database,
            filter,
            meta,
        })
    }

    /// Parse `{TABLE | VIEW | DATABASE [IF NOT EXISTS] | USER | …} <name>`; `CREATE` is
    /// already consumed.
    fn finish_show_create(&mut self, start: Span) -> ParseResult<ShowTarget<D::Ext>> {
        // `USER <user>` — its operand is the shared account grammar, not an `ObjectName`, so
        // it rides its own variant rather than a `ShowCreateKind`.
        if self.eat_contextual_keyword("USER")? {
            let user = self.parse_account_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ShowTarget::CreateUser { user, meta });
        }
        let (kind, if_not_exists) = if self.eat_contextual_keyword("TABLE")? {
            (ShowCreateKind::Table, false)
        } else if self.eat_contextual_keyword("VIEW")? {
            (ShowCreateKind::View, false)
        } else if self.eat_contextual_keyword("EVENT")? {
            (ShowCreateKind::Event, false)
        } else if self.eat_contextual_keyword("PROCEDURE")? {
            (ShowCreateKind::Procedure, false)
        } else if self.eat_contextual_keyword("FUNCTION")? {
            (ShowCreateKind::Function, false)
        } else if self.eat_contextual_keyword("TRIGGER")? {
            (ShowCreateKind::Trigger, false)
        } else {
            let schema = if self.eat_contextual_keyword("SCHEMA")? {
                true
            } else {
                self.expect_contextual_keyword("DATABASE")?;
                false
            };
            // `SHOW CREATE {DATABASE | SCHEMA} [IF NOT EXISTS] <db>`.
            let if_not_exists = if self.eat_contextual_keyword("IF")? {
                self.expect_contextual_keyword("NOT")?;
                self.expect_contextual_keyword("EXISTS")?;
                true
            } else {
                false
            };
            (ShowCreateKind::Database { schema }, if_not_exists)
        };
        let name = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Create {
            kind,
            name,
            if_not_exists,
            meta,
        })
    }

    /// Parse the optional `LIMIT [<offset>,] <row_count>` tail and finish a diagnostics
    /// readout; the `{WARNINGS | ERRORS}` keyword is already consumed.
    fn finish_show_diagnostics(
        &mut self,
        start: Span,
        kind: ShowDiagnosticKind,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let limit = self.parse_optional_show_limit()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Diagnostics {
            kind,
            count: false,
            limit,
            meta,
        })
    }

    /// Parse the routine name and finish a `SHOW {PROCEDURE | FUNCTION} CODE` dump; the
    /// object keyword and `CODE` are already consumed.
    fn finish_show_routine_code(
        &mut self,
        start: Span,
        kind: ShowRoutineKind,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let name = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::RoutineCode { kind, name, meta })
    }

    /// Parse the optional `FOR <user> [USING <role list>]` tail of `SHOW GRANTS`; `GRANTS` is
    /// already consumed. `USING` is grammar-valid only after `FOR <user>`, so it is read only
    /// inside the `FOR` branch.
    fn finish_show_grants(&mut self, start: Span) -> ParseResult<ShowTarget<D::Ext>> {
        let (user, using_roles) = if self.eat_contextual_keyword("FOR")? {
            let user = self.parse_account_name()?;
            let using_roles = if self.eat_contextual_keyword("USING")? {
                self.parse_comma_separated(Self::parse_account_name)?
            } else {
                ThinVec::new()
            };
            (Some(user), using_roles)
        } else {
            (None, ThinVec::new())
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Grants {
            user,
            using_roles,
            meta,
        })
    }

    /// Parse `[<type list>] [FOR QUERY <n>] [LIMIT …]` after `SHOW PROFILE`; `PROFILE` is
    /// already consumed. The three clauses are order-fixed (types, then `FOR QUERY`, then
    /// `LIMIT`).
    fn finish_show_profile(&mut self, start: Span) -> ParseResult<ShowTarget<D::Ext>> {
        let mut types = ThinVec::new();
        if let Some(first) = self.try_parse_profile_type()? {
            types.push(first);
            while self.eat_punct(Punctuation::Comma)? {
                let next = self
                    .try_parse_profile_type()?
                    .ok_or_else(|| self.unexpected("a profile type after `,`"))?;
                types.push(next);
            }
        }
        let query = if self.eat_contextual_keyword("FOR")? {
            self.expect_contextual_keyword("QUERY")?;
            Some(self.parse_show_unsigned_integer("a query id after `FOR QUERY`")?)
        } else {
            None
        };
        let limit = self.parse_optional_show_limit()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::Profile {
            types,
            query,
            limit,
            meta,
        })
    }

    /// Consume one `profile_def` resource-type keyword (or two-word pair); `None` when the
    /// next token starts no profile type.
    fn try_parse_profile_type(&mut self) -> ParseResult<Option<ShowProfileType>> {
        if self.eat_contextual_keyword("ALL")? {
            return Ok(Some(ShowProfileType::All));
        }
        if self.eat_contextual_keyword("CPU")? {
            return Ok(Some(ShowProfileType::Cpu));
        }
        if self.eat_contextual_keyword("MEMORY")? {
            return Ok(Some(ShowProfileType::Memory));
        }
        if self.eat_contextual_keyword("IPC")? {
            return Ok(Some(ShowProfileType::Ipc));
        }
        if self.eat_contextual_keyword("SWAPS")? {
            return Ok(Some(ShowProfileType::Swaps));
        }
        if self.eat_contextual_keyword("SOURCE")? {
            return Ok(Some(ShowProfileType::Source));
        }
        if self.eat_contextual_keyword("BLOCK")? {
            self.expect_contextual_keyword("IO")?;
            return Ok(Some(ShowProfileType::BlockIo));
        }
        if self.eat_contextual_keyword("CONTEXT")? {
            self.expect_contextual_keyword("SWITCHES")?;
            return Ok(Some(ShowProfileType::ContextSwitches));
        }
        if self.eat_contextual_keyword("PAGE")? {
            self.expect_contextual_keyword("FAULTS")?;
            return Ok(Some(ShowProfileType::PageFaults));
        }
        Ok(None)
    }

    /// Parse `[IN '<log>'] [FROM <pos>] [LIMIT …] [FOR CHANNEL '<c>']` after `SHOW {BINLOG |
    /// RELAYLOG} EVENTS`; the keywords through `EVENTS` are already consumed. `FOR CHANNEL` is
    /// `RELAYLOG`-only, so it is read only when `relay` is set (`SHOW BINLOG EVENTS FOR
    /// CHANNEL …` is a syntax error on the server). The `IN` must precede the `FROM`.
    fn finish_show_log_events(
        &mut self,
        start: Span,
        relay: bool,
    ) -> ParseResult<ShowTarget<D::Ext>> {
        let log_name = if self.eat_contextual_keyword("IN")? {
            Some(
                self.try_parse_string_literal()?
                    .ok_or_else(|| self.unexpected("a log-file name string after `IN`"))?,
            )
        } else {
            None
        };
        let position = if self.eat_contextual_keyword("FROM")? {
            Some(self.parse_show_unsigned_integer("a log position after `FROM`")?)
        } else {
            None
        };
        let limit = self.parse_optional_show_limit()?;
        let channel = if relay {
            self.parse_optional_for_channel()?
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ShowTarget::LogEvents {
            relay,
            log_name,
            position,
            limit,
            channel,
            meta,
        })
    }

    /// Parse an optional `GLOBAL`/`SESSION`/`LOCAL` scope keyword; `None` when none leads.
    fn parse_optional_show_scope(&mut self) -> ParseResult<Option<ShowScope>> {
        if self.eat_contextual_keyword("GLOBAL")? {
            Ok(Some(ShowScope::Global))
        } else if self.eat_contextual_keyword("SESSION")? {
            Ok(Some(ShowScope::Session))
        } else if self.eat_contextual_keyword("LOCAL")? {
            Ok(Some(ShowScope::Local))
        } else {
            Ok(None)
        }
    }

    /// Parse an optional `WHERE <expr>` narrowing (the `SHOW INDEX` grammar admits no
    /// `LIKE`); `None` when no `WHERE` leads.
    fn parse_optional_show_where(&mut self) -> ParseResult<Option<ShowFilter<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("WHERE")? {
            let predicate = self.parse_expr()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ShowFilter::Where { predicate, meta }));
        }
        Ok(None)
    }

    /// Parse an optional `LIMIT` tail — MySQL's `opt_limit_clause` in all three forms:
    /// `LIMIT <row_count>`, `LIMIT <offset>, <row_count>`, and `LIMIT <row_count> OFFSET
    /// <offset>`. `None` when no `LIMIT` leads.
    fn parse_optional_show_limit(&mut self) -> ParseResult<Option<ShowLimit>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("LIMIT")? {
            return Ok(None);
        }
        let first = self.parse_show_unsigned_integer("an integer after `LIMIT`")?;
        let (offset, offset_keyword, row_count) = if self.eat_punct(Punctuation::Comma)? {
            let second = self.parse_show_unsigned_integer("a row count after `LIMIT <offset>,`")?;
            (Some(first), false, second)
        } else if self.eat_contextual_keyword("OFFSET")? {
            let second = self.parse_show_unsigned_integer("an offset after `LIMIT <n> OFFSET`")?;
            (Some(second), true, first)
        } else {
            (None, false, first)
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(ShowLimit {
            offset,
            offset_keyword,
            row_count,
            meta,
        }))
    }

    /// Parse an optional `FOR CHANNEL '<name>'` qualifier; `None` when no `FOR` leads.
    fn parse_optional_for_channel(&mut self) -> ParseResult<Option<Literal>> {
        if !self.eat_contextual_keyword("FOR")? {
            return Ok(None);
        }
        self.expect_contextual_keyword("CHANNEL")?;
        let name = self
            .try_parse_string_literal()?
            .ok_or_else(|| self.unexpected("a channel-name string after `FOR CHANNEL`"))?;
        Ok(Some(name))
    }

    /// Consume one unsigned-integer [`Literal`] (a `Number` token); error with `context`
    /// otherwise. A local helper so the `SHOW … LIMIT` operands need not reach into another
    /// module's integer parser.
    fn parse_show_unsigned_integer(&mut self, context: &'static str) -> ParseResult<Literal> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                self.advance()?;
                Ok(Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(token.span),
                })
            }
            _ => Err(self.unexpected(context)),
        }
    }

    /// Parse an optional `{FROM | IN} <name>` object qualifier; `None` when neither
    /// keyword leads. Shared by `SHOW TABLES` (the database qualifier), `SHOW COLUMNS`
    /// (both the table and database qualifiers), and `SHOW FUNCTIONS` (the schema qualifier).
    fn parse_optional_show_from(&mut self) -> ParseResult<Option<ShowFrom>> {
        let start = self.current_span()?;
        let keyword = if self.eat_contextual_keyword("FROM")? {
            ShowFromKeyword::From
        } else if self.eat_contextual_keyword("IN")? {
            ShowFromKeyword::In
        } else {
            return Ok(None);
        };
        let name = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(ShowFrom {
            keyword,
            name,
            meta,
        }))
    }

    /// Parse the optional trailing `LIKE '<pat>'` / `WHERE <expr>` narrowing; `None` when
    /// neither keyword leads. The two are mutually exclusive in the grammar.
    fn parse_optional_show_filter(&mut self) -> ParseResult<Option<ShowFilter<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("LIKE")? {
            let pattern = self
                .try_parse_string_literal()?
                .ok_or_else(|| self.unexpected("a string pattern after `LIKE`"))?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ShowFilter::Like { pattern, meta }));
        }
        if self.eat_contextual_keyword("WHERE")? {
            let predicate = self.parse_expr()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ShowFilter::Where { predicate, meta }));
        }
        Ok(None)
    }

    /// Parse the legacy `[ANALYZE | ANALYSE] [VERBOSE]` keyword prefix; both options
    /// are bare (no argument) in this spelling, and `ANALYZE` precedes `VERBOSE`.
    fn parse_legacy_explain_options(&mut self) -> ParseResult<ThinVec<ExplainOption>> {
        let mut options = ThinVec::new();
        if self.eat_contextual_keyword("ANALYZE")? || self.eat_contextual_keyword("ANALYSE")? {
            let meta = self.make_meta(self.preceding_span());
            options.push(ExplainOption::Analyze { value: None, meta });
        }
        if self.eat_contextual_keyword("VERBOSE")? {
            let meta = self.make_meta(self.preceding_span());
            options.push(ExplainOption::Verbose { value: None, meta });
        }
        Ok(options)
    }

    fn parse_explain_option_list(&mut self) -> ParseResult<ThinVec<ExplainOption>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the EXPLAIN option list")?;
        let options = self.parse_comma_separated(Self::parse_explain_option)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the EXPLAIN option list")?;
        Ok(options)
    }

    fn parse_explain_option(&mut self) -> ParseResult<ExplainOption> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ANALYZE")? || self.eat_contextual_keyword("ANALYSE")? {
            let value = self.parse_optional_explain_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ExplainOption::Analyze { value, meta })
        } else if self.eat_contextual_keyword("VERBOSE")? {
            let value = self.parse_optional_explain_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ExplainOption::Verbose { value, meta })
        } else if self.eat_contextual_keyword("FORMAT")? {
            let format = self.parse_explain_format()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ExplainOption::Format { format, meta })
        } else {
            let name = self.parse_ident()?;
            let value = self.parse_optional_explain_option_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ExplainOption::Other { name, value, meta })
        }
    }

    /// Parse an option's optional boolean/word argument. The value is a `ColLabel`,
    /// so the boolean spellings `TRUE`/`FALSE`/`ON`/`OFF` (some reserved) are
    /// admitted verbatim; numeric and string arguments are not modelled.
    fn parse_optional_explain_option_value(&mut self) -> ParseResult<Option<Ident>> {
        if self.peek_is_punct(Punctuation::Comma)? || self.peek_is_punct(Punctuation::RParen)? {
            return Ok(None);
        }
        Ok(Some(self.parse_as_alias_ident()?))
    }

    fn parse_explain_format(&mut self) -> ParseResult<ExplainFormat> {
        if self.eat_contextual_keyword("TEXT")? {
            Ok(ExplainFormat::Text)
        } else if self.eat_contextual_keyword("XML")? {
            Ok(ExplainFormat::Xml)
        } else if self.eat_contextual_keyword("JSON")? {
            Ok(ExplainFormat::Json)
        } else if self.eat_contextual_keyword("YAML")? {
            Ok(ExplainFormat::Yaml)
        } else {
            Err(self.unexpected("a format: `TEXT`, `XML`, `JSON`, or `YAML`"))
        }
    }

    // --- PRAGMA / ATTACH / DETACH (SQLite) -----------------------------------

    /// Parse a `PRAGMA [<schema> .] <name> [= <value> | (<value>)]` statement into
    /// [`Statement::Pragma`], reached under
    /// [`UtilitySyntax::pragma`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The assignment and call spellings share one value slot with the
    /// `parenthesized` tag recording which was written. SQLite's
    /// `pragma-value` is `signed-number | name | string-literal` — not a general
    /// expression (`PRAGMA cache_size = 1 + 2` is a SQLite syntax error) — which is
    /// exactly the `SET` parameter-value grammar, so the value reuses
    /// [`parse_set_parameter_value`](Self::parse_set_parameter_value) (folded sign
    /// included). The assignment form matches the `=` operator token only: SQLite's
    /// tokenizer folds `==` onto its `TK_EQ` and so accepts `PRAGMA x == 1`, a
    /// spelling we deliberately leave unaccepted (no corpus evidence, and accepting
    /// it would canonicalize away the `==`).
    pub(super) fn parse_pragma_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("PRAGMA")?;
        let name = self.parse_object_name()?;
        let (value, parenthesized) = if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
            (Some(self.parse_set_parameter_value()?), false)
        } else if self.eat_punct(Punctuation::LParen)? {
            let value = self.parse_set_parameter_value()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the PRAGMA value")?;
            (Some(value), true)
        } else {
            (None, false)
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Pragma {
            pragma: Box::new(PragmaStatement {
                name,
                value,
                parenthesized,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse an `ATTACH [DATABASE] <expr> AS <schema>` statement into
    /// [`Statement::Attach`], reached under
    /// [`UtilitySyntax::attach`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The database source is a full expression (SQLite `parse.y`: `ATTACH
    /// database_kw_opt expr AS expr`); the schema alias is read as a label
    /// identifier — the deliberate narrowing recorded on
    /// [`AttachStatement`]. The trailing SEE `KEY
    /// <expr>` clause is not modelled.
    pub(super) fn parse_attach_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("ATTACH")?;
        let database_keyword = self.eat_contextual_keyword("DATABASE")?;
        let target = self.parse_expr()?;
        self.expect_keyword(Keyword::As)?;
        let schema = self.parse_as_alias_ident()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Attach {
            attach: Box::new(AttachStatement {
                database_keyword,
                target,
                schema,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse a `DETACH [DATABASE] [IF EXISTS] <schema>` statement into
    /// [`Statement::Detach`] — the `ATTACH` inverse, sharing its
    /// [`UtilitySyntax::attach`](crate::ast::dialect::UtilitySyntax) gate.
    ///
    /// The DuckDB `IF EXISTS` guard
    /// ([`UtilitySyntax::detach_if_exists`](crate::ast::dialect::UtilitySyntax)) is
    /// admitted only after the `DATABASE` keyword — `DETACH DATABASE IF EXISTS x` parses
    /// but `DETACH IF EXISTS x` is a parser error (DuckDB 1.5.4) — so the guard is read
    /// only when `DATABASE` was written.
    pub(super) fn parse_detach_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DETACH")?;
        let database_keyword = self.eat_contextual_keyword("DATABASE")?;
        let if_exists = if database_keyword
            && self.features().utility_syntax.detach_if_exists
            && self.eat_contextual_keyword("IF")?
        {
            self.expect_keyword(Keyword::Exists)?;
            true
        } else {
            false
        };
        let schema = self.parse_as_alias_ident()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Detach {
            detach: Box::new(DetachStatement {
                database_keyword,
                if_exists,
                schema,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse a DuckDB `EXPORT DATABASE ['<db>' TO] '<path>' [<copy-options>]` statement
    /// into [`Statement::Export`], reached under
    /// [`UtilitySyntax::export_import_database`](crate::ast::dialect::UtilitySyntax).
    ///
    /// Two forms (DuckDB `ExportStmt`): the bare `EXPORT DATABASE '<path>'` and the named
    /// `EXPORT DATABASE <db> TO '<path>'`. A leading string is the bare form; a leading
    /// identifier is the catalogue name, which a *required* `TO` separates from the path
    /// (`EXPORT DATABASE db '<path>'` without `TO` is a parser error, probed on 1.5.4).
    /// The options are the shared `copy_options` production (a legacy `copy_opt_list` or a
    /// parenthesized generic list) with no leading `WITH`.
    pub(super) fn parse_export_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("EXPORT")?;
        self.expect_contextual_keyword("DATABASE")?;
        let (database, path) = if let Some(path) = self.try_parse_string_literal()? {
            (None, path)
        } else {
            let database = self.parse_ident()?;
            self.expect_contextual_keyword("TO")?;
            let path = self.expect_string_literal(
                "a destination path string after `EXPORT DATABASE <db> TO`",
            )?;
            (Some(database), path)
        };
        let (parenthesized, options) =
            self.parse_copy_options_trailer("`)` to close the EXPORT option list")?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Export {
            export: Box::new(ExportStatement {
                database,
                path,
                parenthesized,
                options,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse a DuckDB `IMPORT DATABASE '<path>'` statement into [`Statement::Import`] — the
    /// [`parse_export_statement`](Self::parse_export_statement) inverse, sharing its
    /// [`UtilitySyntax::export_import_database`](crate::ast::dialect::UtilitySyntax) gate.
    ///
    /// The grammar (DuckDB `ImportStmt`) is exactly `IMPORT DATABASE '<path>'`: one string
    /// path and no trailing options (`IMPORT DATABASE '<p>' (...)` is a parser error,
    /// probed on 1.5.4), left for the outer statement loop to reject.
    pub(super) fn parse_import_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("IMPORT")?;
        self.expect_contextual_keyword("DATABASE")?;
        let path = self.expect_string_literal("a source path string after `IMPORT DATABASE`")?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Import {
            import: Box::new(ImportStatement { path, meta }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `SHUTDOWN` statement into [`Statement::Shutdown`], reached under
    /// [`UtilitySyntax::shutdown`](crate::ast::dialect::UtilitySyntax). A nullary leading keyword
    /// — any trailing operand (`SHUTDOWN 1`) is left for the outer statement loop to reject, as
    /// mysql:8 does.
    pub(super) fn parse_shutdown_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("SHUTDOWN")?;
        Ok(Statement::Shutdown {
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Parse a MySQL `RESTART` statement into [`Statement::Restart`], reached under
    /// [`UtilitySyntax::restart`](crate::ast::dialect::UtilitySyntax). A nullary leading keyword.
    pub(super) fn parse_restart_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("RESTART")?;
        Ok(Statement::Restart {
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Parse a MySQL `CLONE` statement into [`Statement::Clone`], reached under
    /// [`UtilitySyntax::clone`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The `LOCAL` form is `CLONE LOCAL DATA DIRECTORY [=] '<dir>'`; the `INSTANCE` form is
    /// `CLONE INSTANCE FROM <user>[@<host>]:<port> IDENTIFIED BY '<pw>' [DATA DIRECTORY [=]
    /// '<dir>'] [REQUIRE [NO] SSL]` (`sql_yacc.yy` `clone_stmt`). The donor `:<port>` is
    /// mandatory and must abut the account — mysql:8 rejects whitespace on either side of the
    /// `:` with a raw-offset adjacency check, enforced here on the token spans.
    pub(super) fn parse_clone_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("CLONE")?;
        let clone = if self.eat_contextual_keyword("LOCAL")? {
            let data_directory = self.parse_clone_data_directory()?;
            CloneStatement::Local {
                data_directory,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("INSTANCE")? {
            self.expect_contextual_keyword("FROM")?;
            let source = self.parse_account_name()?;
            // The `:<port>` abuts the account with no surrounding whitespace: mysql:8's grammar
            // checks the raw byte offsets, so a space on either side of the `:` is
            // `ER_PARSE_ERROR`. Compare the token spans to enforce the same boundary.
            let account_end = self.preceding_span().end();
            let colon_span = self.current_span()?;
            self.expect_punct(
                Punctuation::Colon,
                "`:` before the CLONE INSTANCE donor port",
            )?;
            let port_span = self.current_span()?;
            let adjacent =
                colon_span.start() == account_end && port_span.start() == colon_span.end();
            let port = self.expect_unsigned_integer_literal("the CLONE INSTANCE donor port")?;
            if !adjacent {
                let joined = colon_span.union(self.preceding_span());
                let found = self.span_text(joined).to_owned();
                return Err(self.error_at(
                    joined,
                    "a CLONE INSTANCE `<user>:<port>` with the port abutting the colon (no \
                     surrounding whitespace)",
                    found,
                ));
            }
            self.expect_contextual_keyword("IDENTIFIED")?;
            self.expect_contextual_keyword("BY")?;
            let password = self
                .expect_string_literal("a password string after CLONE INSTANCE `IDENTIFIED BY`")?;
            let data_directory = if self.peek_is_contextual_keyword("DATA")? {
                Some(self.parse_clone_data_directory()?)
            } else {
                None
            };
            let ssl = self.parse_clone_ssl()?;
            CloneStatement::Instance {
                source,
                port,
                password,
                data_directory,
                ssl,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else {
            return Err(self.unexpected("`LOCAL` or `INSTANCE` after `CLONE`"));
        };
        let span = start.union(self.preceding_span());
        Ok(Statement::Clone {
            clone: Box::new(clone),
            meta: self.make_meta(span),
        })
    }

    /// Parse the `DATA DIRECTORY [=] '<dir>'` target shared by both [`CloneStatement`] forms.
    fn parse_clone_data_directory(&mut self) -> ParseResult<CloneDataDirectory> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DATA")?;
        self.expect_contextual_keyword("DIRECTORY")?;
        let equals = self.eat_op(Operator::Eq)?;
        let path = self.expect_string_literal("a directory path string after `DATA DIRECTORY`")?;
        Ok(CloneDataDirectory {
            equals,
            path,
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Parse the optional `REQUIRE [NO] SSL` transport clause of a `CLONE INSTANCE` statement
    /// (`sql_yacc.yy` `opt_ssl`).
    fn parse_clone_ssl(&mut self) -> ParseResult<CloneSsl> {
        if !self.eat_contextual_keyword("REQUIRE")? {
            return Ok(CloneSsl::Unspecified);
        }
        let no = self.eat_contextual_keyword("NO")?;
        self.expect_contextual_keyword("SSL")?;
        Ok(if no {
            CloneSsl::RequireNo
        } else {
            CloneSsl::Require
        })
    }

    /// Parse a MySQL `IMPORT TABLE FROM '<file>' [, …]` statement into
    /// [`Statement::ImportTable`], reached under
    /// [`UtilitySyntax::import_table`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The operand is a non-empty comma-separated list of **string** literals
    /// (`TEXT_STRING_sys_list`); a bare identifier is `ER_PARSE_ERROR` on mysql:8, so each item
    /// is read as a string literal, not a name.
    pub(super) fn parse_import_table_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("IMPORT")?;
        self.expect_contextual_keyword("TABLE")?;
        self.expect_contextual_keyword("FROM")?;
        let files = self.parse_comma_separated(|parser| {
            parser.expect_string_literal("a `.sdi` file path string after `IMPORT TABLE FROM`")
        })?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::ImportTable {
            import_table: Box::new(ImportTableStatement {
                files,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `HELP '<topic>'` statement into [`Statement::Help`], reached under
    /// [`UtilitySyntax::help_statement`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The operand is a single `ident_or_text` (`sql_yacc.yy` `help`): a bare identifier (`HELP
    /// contents`) and a quoted string (`HELP 'contents'`) are both accepted and fold to one
    /// [`Ident`], as [`parse_account_name`](Self::parse_account_name)'s name parts do.
    pub(super) fn parse_help_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("HELP")?;
        let topic = self.parse_ident_or_text()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Help {
            help: Box::new(HelpStatement {
                topic,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `BINLOG '<base64-event>'` statement into [`Statement::Binlog`], reached
    /// under [`UtilitySyntax::binlog`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The operand is a single string literal (`TEXT_STRING_sys`); a bare identifier is
    /// `ER_PARSE_ERROR` on mysql:8. The base64 payload is preserved verbatim from its span — it
    /// is never decoded here (decode and event application are execution-time concerns).
    pub(super) fn parse_binlog_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("BINLOG")?;
        let event = self.expect_string_literal("a base64 event string after `BINLOG`")?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Binlog {
            binlog: Box::new(BinlogStatement {
                event,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a `[FORCE] CHECKPOINT [<database>]` statement into
    /// [`Statement::Checkpoint`], reached under
    /// [`MaintenanceSyntax::checkpoint`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The bare `CHECKPOINT` is PostgreSQL/DuckDB; the `FORCE` modifier and the single
    /// database operand are DuckDB extensions gated by
    /// [`MaintenanceSyntax::checkpoint_database`](crate::ast::dialect::UtilitySyntax). The
    /// database is one bare [`Ident`] — DuckDB rejects a dotted `CHECKPOINT a.b` and a
    /// quoted-string operand — so it reads a single identifier, present only when a name
    /// can start there (a trailing name under PostgreSQL, where the gate is off, is left
    /// unconsumed and rejected as a stray token).
    pub(super) fn parse_checkpoint_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let force = self.eat_contextual_keyword("FORCE")?;
        self.expect_contextual_keyword("CHECKPOINT")?;
        let database = if self.features().maintenance_syntax.checkpoint_database
            && self.peek_can_start_column_name()?
        {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Checkpoint {
            checkpoint: Box::new(CheckpointStatement {
                force,
                database,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse a `LOAD <extension>` statement into [`Statement::Load`], reached under
    /// [`UtilitySyntax::load_extension`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The argument is a string path (`LOAD 'plpgsql'`, PostgreSQL/DuckDB) or — under
    /// [`UtilitySyntax::load_bare_name`](crate::ast::dialect::UtilitySyntax) — a bare
    /// identifier extension name (`LOAD tpch`, DuckDB). A string is tried first so a
    /// quoted argument never falls through to the name path.
    pub(super) fn parse_load_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LOAD")?;
        let target = self.parse_load_target()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Load {
            load: Box::new(LoadStatement { target, meta }),
            meta: statement_meta,
        })
    }

    /// Parse the argument of a [`LoadStatement`]: a string path or a bare name.
    fn parse_load_target(&mut self) -> ParseResult<LoadTarget> {
        let start = self.current_span()?;
        if let Some(path) = self.try_parse_string_literal()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(LoadTarget::Path { path, meta })
        } else if self.features().utility_syntax.load_bare_name {
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(LoadTarget::Name { name, meta })
        } else {
            Err(self.unexpected("a string extension path after `LOAD`"))
        }
    }

    // --- UPDATE EXTENSIONS (DuckDB) -----------------------------------------

    /// True when a leading `UPDATE` opens the DuckDB `UPDATE EXTENSIONS` refresh statement
    /// rather than the DML `UPDATE <target> SET …`.
    ///
    /// A second-token peek that returns without consuming: the hot DML `UPDATE` path pays
    /// one keyword comparison and — since `EXTENSIONS` is not the DML relation's first word
    /// in practice — falls straight through to [`Self::parse_update_statement_with`], never
    /// entering the extension-statement frame. `EXTENSIONS` is DuckDB-*unreserved*, so it
    /// is also a legal table name; the catalogue statement therefore wins only when the
    /// word is followed by the parenthesized name list (`(`) or the statement end (`;` /
    /// EOF). An `UPDATE extensions SET …` / `UPDATE EXTENSIONS AS e SET …` keeps targeting a
    /// table literally named `extensions`, exactly as DuckDB's own grammar resolves the
    /// shared `UPDATE EXTENSIONS` prefix (engine-probed on 1.5.4).
    pub(super) fn peek_starts_update_extensions(&mut self) -> ParseResult<bool> {
        debug_assert!(self.peek_is_contextual_keyword("UPDATE")?);
        if !self.peek_nth_is_contextual_keyword(1, "EXTENSIONS")? {
            return Ok(false);
        }
        Ok(self.peek_nth_is_punct(2, Punctuation::LParen)?
            || self.peek_nth(2)?.is_none()
            || self.peek_nth_is_punct(2, Punctuation::Semicolon)?)
    }

    /// Parse the DuckDB `UPDATE EXTENSIONS [( <name>, ... )]` statement into
    /// [`Statement::UpdateExtensions`], reached under
    /// [`UtilitySyntax::update_extensions`](crate::ast::dialect::UtilitySyntax) once
    /// [`peek_starts_update_extensions`](Self::peek_starts_update_extensions) has confirmed
    /// the `EXTENSIONS` keyword.
    ///
    /// The optional parenthesized operand is DuckDB's `opt_column_list` — a non-empty
    /// comma-separated `ColId` list (bare or quoted identifiers; a string, dotted name, or
    /// empty `()` are all DuckDB parser errors), so it reuses
    /// [`parse_ident`](Self::parse_ident). Its absence is the bare `UPDATE EXTENSIONS`
    /// (refresh all installed), recorded as an empty [`extensions`] list.
    ///
    /// [`extensions`]: crate::ast::UpdateExtensionsStatement::extensions
    pub(super) fn parse_update_extensions_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("UPDATE")?;
        self.expect_contextual_keyword("EXTENSIONS")?;
        let extensions = if self.eat_punct(Punctuation::LParen)? {
            let names = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the UPDATE EXTENSIONS list",
            )?;
            names
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::UpdateExtensions {
            update_extensions: Box::new(UpdateExtensionsStatement {
                extensions,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- VACUUM / REINDEX / ANALYZE (SQLite) --------------------------------

    /// Parse a `VACUUM [<schema>] [INTO <expr>]` statement into
    /// [`Statement::Vacuum`], reached under
    /// [`MaintenanceSyntax::vacuum`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The optional schema is a single database name (SQLite rejects the dotted
    /// `VACUUM main.t`), so it is read as one [`Ident`] rather than an object name; it
    /// is present only when the next token can start a name and is not the `INTO`
    /// keyword (`VACUUM INTO 'f'` has no schema). The `INTO <filename>` target is a
    /// full expression (`VACUUM INTO 'a' || '.db'` is legal SQLite), so it reuses
    /// [`parse_expr`](Self::parse_expr).
    pub(super) fn parse_vacuum_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("VACUUM")?;
        // The leading `VACUUM` dispatches under either gate; the tail read depends on
        // which is active. With both on (Lenient) the accepted language is the exact
        // UNION of the two grammars, not their cross product: engine-measured (SQLite
        // 3.x + DuckDB 1.5.4), every hybrid tail — `VACUUM ANALYZE … INTO`, a column
        // list before `INTO`, a dotted name before `INTO` — is rejected by BOTH
        // engines, so the `INTO` tail below is admitted only on a SQLite-shaped prefix.
        let duck = self.features().maintenance_syntax.vacuum_analyze;
        let sqlite = self.features().maintenance_syntax.vacuum;
        // DuckDB `ANALYZE` option — the only vacuum option 1.5.4 admits, spelled bare
        // (`VACUUM ANALYZE`) or as a parenthesized option list (`VACUUM (ANALYZE)`), the two
        // tracked distinctly on `VacuumAnalyze` so each round-trips verbatim. Every other
        // option rejects at one of the two engine layers, so neither spelling parses it here
        // (engine-measured on libduckdb 1.5.4): the parser rejects `NOWAIT`/`SKIP_TOAST`/any
        // unknown option and the boolean-argument form `(ANALYZE true)`, and the transform
        // throws `NotImplementedException` on `FULL`/`FREEZE`/`VERBOSE`/`disable_page_skipping`.
        let analyze = if duck && self.peek_is_punct(Punctuation::LParen)? {
            // The parenthesized option list is a non-empty comma-separated list whose only
            // legal element is the `ANALYZE` keyword; repeats (`(ANALYZE, ANALYZE)`) prepare
            // on the oracle and canonicalize to the single flag. A leading `(` after `VACUUM`
            // can only open this list (the column list rides a trailing table), so it commits
            // to the DuckDB grammar even under the Lenient union — SQLite syntax-errors here.
            self.expect_punct(Punctuation::LParen, "`(` to open the VACUUM option list")?;
            loop {
                self.expect_contextual_keyword("ANALYZE")?;
                if !self.eat_punct(Punctuation::Comma)? {
                    break;
                }
            }
            self.expect_punct(Punctuation::RParen, "`)` to close the VACUUM option list")?;
            Some(VacuumAnalyze::Parenthesized)
        } else if duck
            // With the SQLite gate also on, `VACUUM ANALYZE INTO 'f'` belongs to SQLite's
            // grammar alone (`nm` = `ANALYZE`, a fallback keyword there — engine-measured, the
            // SQLite reject is a prepare-time "unknown database", i.e. a grammar accept, while
            // DuckDB rejects every `INTO` tail), so a directly following `INTO` leaves the
            // `ANALYZE` for the name read below instead of eating it as the DuckDB option.
            && !(sqlite
                && self.peek_is_contextual_keyword("ANALYZE")?
                && self.peek_nth_is_contextual_keyword(1, "INTO")?)
            && self.eat_contextual_keyword("ANALYZE")?
        {
            Some(VacuumAnalyze::Keyword)
        } else {
            None
        };
        // The optional name operand: DuckDB's qualified table (or Sconst table name
        // — plain `'…'`, `E'…'`, `$$…$$`) or SQLite's single-ident schema.
        // `VACUUM main.t` is a SQLite syntax error, so the SQLite branch reads a bare
        // [`parse_ident`](Self::parse_ident); DuckDB reads either a dotted
        // [`parse_object_name`](Self::parse_object_name) or one string-spelled identifier.
        let string_table = duck && self.peek_is_name_sconst()?;
        let (schema, table) = if !self.peek_is_contextual_keyword("INTO")?
            && (self.peek_can_start_column_name()? || string_table)
        {
            if string_table {
                let ident = self.parse_name_sconst_ident("a table name after VACUUM")?;
                (None, Some(ObjectName(thin_vec![ident])))
            } else if duck {
                (None, Some(self.parse_object_name()?))
            } else {
                (Some(self.parse_ident()?), None)
            }
        } else {
            (None, None)
        };
        // DuckDB column list, only alongside a table (`VACUUM t (a, b)`).
        let columns = if duck && table.is_some() {
            self.parse_optional_maintenance_column_list()?
        } else {
            None
        };
        // SQLite `INTO <expr>` tail (`VACUUM INTO 'a' || '.db'` is legal) — only on a
        // SQLite-shaped prefix: no `ANALYZE` keyword, no column list, and a name operand
        // of at most one part. On a DuckDB-only prefix the `INTO` is left unconsumed and
        // the statement loop rejects it, matching both engines' verdicts on the hybrids.
        let sqlite_prefix = analyze.is_none()
            && columns.is_none()
            && table.as_ref().is_none_or(|name| name.0.len() == 1);
        let into = if sqlite && sqlite_prefix && self.eat_contextual_keyword("INTO")? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        // A taken `INTO` selects the SQLite grammar, whose name operand is the single
        // database name: demote the DuckDB-read single-part table into the schema slot,
        // preserving the node invariant that only one dialect's fields are populated.
        let (schema, table) = match (&into, table) {
            (Some(_), Some(name)) => (name.0.into_iter().next(), None),
            (_, table) => (schema, table),
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Vacuum {
            vacuum: Box::new(VacuumStatement {
                schema,
                into,
                analyze,
                table,
                columns,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse an optional DuckDB maintenance `( <column> [, …] )` column list into
    /// `Some(non-empty)`, or `None` when no `(` follows. Shared by the `VACUUM`/`ANALYZE`
    /// DuckDB tails; the list is always non-empty (`ANALYZE t ()` is a parser error, per
    /// libpg_query's `name_list`).
    fn parse_optional_maintenance_column_list(&mut self) -> ParseResult<Option<ThinVec<Ident>>> {
        if self.eat_punct(Punctuation::LParen)? {
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the column list")?;
            Ok(Some(columns))
        } else {
            Ok(None)
        }
    }

    /// Parse a `REINDEX [<collation> | [<schema> .] <table-or-index>]` statement into
    /// [`Statement::Reindex`], reached under
    /// [`MaintenanceSyntax::reindex`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The optional target is a possibly schema-qualified name; SQLite disambiguates a
    /// collation, table, or index name by catalogue lookup rather than syntax, so one
    /// [`parse_object_name`](Self::parse_object_name) slot covers all three. Absent
    /// when the statement ends after the keyword (bare `REINDEX`).
    pub(super) fn parse_reindex_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("REINDEX")?;
        let target = if self.peek_can_start_column_name()? {
            Some(self.parse_object_name()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Reindex {
            reindex: Box::new(ReindexStatement { target, meta }),
            meta: statement_meta,
        })
    }

    /// Parse an `ANALYZE [<schema> | [<schema> .] <table-or-index>]` statement into
    /// [`Statement::Analyze`], reached under
    /// [`MaintenanceSyntax::analyze`](crate::ast::dialect::UtilitySyntax). The optional
    /// target mirrors [`parse_reindex_statement`](Self::parse_reindex_statement), with
    /// DuckDB's single-quoted table-name spelling alongside its column-list form.
    pub(super) fn parse_analyze_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("ANALYZE")?;
        let string_target =
            self.features().maintenance_syntax.analyze_columns && self.peek_is_name_sconst()?;
        let target = if string_target {
            let ident = self.parse_name_sconst_ident("a table name after ANALYZE")?;
            Some(ObjectName(thin_vec![ident]))
        } else if self.peek_can_start_column_name()? {
            Some(self.parse_object_name()?)
        } else {
            None
        };
        // DuckDB column list (`ANALYZE t (a, b)`), only alongside a target and gated by
        // `analyze_columns`; SQLite's `ANALYZE` takes no column list.
        let columns = if self.features().maintenance_syntax.analyze_columns && target.is_some() {
            self.parse_optional_maintenance_column_list()?
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Analyze {
            analyze: Box::new(AnalyzeStatement {
                target,
                columns,
                meta,
            }),
            meta: statement_meta,
        })
    }

    // --- MySQL admin-table maintenance & RENAME -----------------------------

    /// Whether the cursor is at the start of a MySQL admin-table maintenance verb —
    /// `{ANALYZE | CHECK | CHECKSUM | OPTIMIZE | REPAIR}` followed by the tokens that
    /// commit it to the `<verb> [prefix] {TABLE | TABLES} …` form.
    ///
    /// Keeps the family MECE with the SQLite/DuckDB leading-`ANALYZE`
    /// [`analyze`](crate::ast::dialect::MaintenanceSyntax::analyze) gate: MySQL's `ANALYZE`
    /// always takes `{TABLE | TABLES}` (optionally after `NO_WRITE_TO_BINLOG | LOCAL`), so a
    /// bare `ANALYZE` never satisfies this lookahead and still falls through to the sibling.
    /// `OPTIMIZE`/`REPAIR` share that binlog-prefix follow-set; `CHECK`/`CHECKSUM` take the
    /// `TABLE`/`TABLES` keyword directly.
    pub(super) fn peek_starts_table_maintenance(&mut self) -> ParseResult<bool> {
        if self.peek_is_contextual_keyword("ANALYZE")?
            || self.peek_is_contextual_keyword("OPTIMIZE")?
            || self.peek_is_contextual_keyword("REPAIR")?
        {
            return Ok(self.peek_nth_starts_table_or_tables(1)?
                || self.peek_nth_is_contextual_keyword(1, "NO_WRITE_TO_BINLOG")?
                || self.peek_nth_is_contextual_keyword(1, "LOCAL")?);
        }
        if self.peek_is_contextual_keyword("CHECK")?
            || self.peek_is_contextual_keyword("CHECKSUM")?
        {
            return self.peek_nth_starts_table_or_tables(1);
        }
        Ok(false)
    }

    /// Whether the token `n` positions ahead is the `TABLE` keyword or its `TABLES` synonym.
    pub(super) fn peek_nth_starts_table_or_tables(&mut self, n: usize) -> ParseResult<bool> {
        Ok(self.peek_nth_is_contextual_keyword(n, "TABLE")?
            || self.peek_nth_is_contextual_keyword(n, "TABLES")?)
    }

    /// Parse a MySQL admin-table maintenance statement `{ANALYZE | CHECK | CHECKSUM |
    /// OPTIMIZE | REPAIR} [NO_WRITE_TO_BINLOG | LOCAL] {TABLE | TABLES} <table-list>
    /// [options]` into [`Statement::TableMaintenance`], reached under
    /// [`MaintenanceSyntax::table_maintenance`](crate::ast::dialect::MaintenanceSyntax).
    ///
    /// The five verbs share the `<verb> [prefix] TABLE(S) <list>` spine — read once here —
    /// and diverge only in their prefix eligibility and trailing options, which ride the
    /// [`TableMaintenanceKind`] axis.
    pub(super) fn parse_table_maintenance_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        // The verb identity while the shared spine is read; the per-verb payload is built
        // into [`TableMaintenanceKind`] once the option tail is known.
        enum MaintenanceVerb {
            Analyze,
            Check,
            Checksum,
            Optimize,
            Repair,
        }
        let start = self.current_span()?;
        // The verb, plus the `NO_WRITE_TO_BINLOG | LOCAL` prefix for the verbs that admit
        // it (ANALYZE/OPTIMIZE/REPAIR; CHECK/CHECKSUM have none).
        let (verb, no_write_to_binlog) = if self.eat_contextual_keyword("ANALYZE")? {
            (
                MaintenanceVerb::Analyze,
                self.parse_optional_no_write_to_binlog()?,
            )
        } else if self.eat_contextual_keyword("OPTIMIZE")? {
            (
                MaintenanceVerb::Optimize,
                self.parse_optional_no_write_to_binlog()?,
            )
        } else if self.eat_contextual_keyword("REPAIR")? {
            (
                MaintenanceVerb::Repair,
                self.parse_optional_no_write_to_binlog()?,
            )
        } else if self.eat_contextual_keyword("CHECKSUM")? {
            (MaintenanceVerb::Checksum, None)
        } else {
            self.expect_contextual_keyword("CHECK")?;
            (MaintenanceVerb::Check, None)
        };
        let table_keyword = self.parse_table_or_tables_keyword()?;
        let tables = self.parse_comma_separated(Self::parse_object_name)?;
        // Read the per-verb option tail, then stamp the verb payload with the statement
        // span (the hoisted spine has no contiguous sub-span, so — like `VACUUM` — the
        // inner `meta` mirrors the statement span).
        let kind = match verb {
            MaintenanceVerb::Analyze => {
                let histogram = self.parse_optional_analyze_histogram()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                TableMaintenanceKind::Analyze {
                    no_write_to_binlog,
                    histogram,
                    meta,
                }
            }
            MaintenanceVerb::Check => {
                let options = self.parse_check_table_options()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                TableMaintenanceKind::Check { options, meta }
            }
            MaintenanceVerb::Checksum => {
                let option = self.parse_optional_checksum_table_option()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                TableMaintenanceKind::Checksum { option, meta }
            }
            MaintenanceVerb::Optimize => {
                let meta = self.make_meta(start.union(self.preceding_span()));
                TableMaintenanceKind::Optimize {
                    no_write_to_binlog,
                    meta,
                }
            }
            MaintenanceVerb::Repair => {
                let options = self.parse_repair_table_options()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                TableMaintenanceKind::Repair {
                    no_write_to_binlog,
                    options,
                    meta,
                }
            }
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::TableMaintenance {
            table_maintenance: Box::new(TableMaintenanceStatement {
                kind,
                table_keyword,
                tables,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Consume the optional `NO_WRITE_TO_BINLOG | LOCAL` binlog-suppression prefix; `LOCAL`
    /// is an exact synonym, and the written spelling is preserved for round-trip.
    fn parse_optional_no_write_to_binlog(&mut self) -> ParseResult<Option<NoWriteToBinlog>> {
        if self.eat_contextual_keyword("NO_WRITE_TO_BINLOG")? {
            Ok(Some(NoWriteToBinlog::NoWriteToBinlog))
        } else if self.eat_contextual_keyword("LOCAL")? {
            Ok(Some(NoWriteToBinlog::Local))
        } else {
            Ok(None)
        }
    }

    /// Consume the shared `TABLE`/`TABLES` keyword (MySQL's `table_or_tables`), preserving
    /// the written spelling.
    fn parse_table_or_tables_keyword(&mut self) -> ParseResult<TableKeyword> {
        if self.eat_contextual_keyword("TABLES")? {
            Ok(TableKeyword::Tables)
        } else {
            self.expect_contextual_keyword("TABLE")?;
            Ok(TableKeyword::Table)
        }
    }

    /// Parse the optional `ANALYZE TABLE` histogram tail — `UPDATE HISTOGRAM ON <cols>
    /// [WITH <n> BUCKETS]` or `DROP HISTOGRAM ON <cols>`; `None` when neither leads.
    ///
    /// The 8.4 `UPDATE` extensions (`{AUTO | MANUAL} UPDATE`, `USING DATA '<json>'`) are
    /// not modelled — this is the measured `WITH <n> BUCKETS` surface.
    fn parse_optional_analyze_histogram(&mut self) -> ParseResult<Option<AnalyzeHistogram>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("UPDATE")? {
            self.expect_contextual_keyword("HISTOGRAM")?;
            self.expect_contextual_keyword("ON")?;
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            let buckets = if self.eat_contextual_keyword("WITH")? {
                let count = self.parse_show_unsigned_integer("a bucket count after `WITH`")?;
                self.expect_contextual_keyword("BUCKETS")?;
                Some(count)
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(AnalyzeHistogram::Update {
                columns,
                buckets,
                meta,
            }))
        } else if self.eat_contextual_keyword("DROP")? {
            self.expect_contextual_keyword("HISTOGRAM")?;
            self.expect_contextual_keyword("ON")?;
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(AnalyzeHistogram::Drop { columns, meta }))
        } else {
            Ok(None)
        }
    }

    /// Parse the repeatable `CHECK TABLE` check-type option list (`opt_mi_check_types`);
    /// order and repeats are preserved so the statement round-trips.
    fn parse_check_table_options(&mut self) -> ParseResult<ThinVec<CheckTableOption>> {
        let mut options = ThinVec::new();
        loop {
            let option = if self.eat_contextual_keyword("FOR")? {
                self.expect_contextual_keyword("UPGRADE")?;
                CheckTableOption::ForUpgrade
            } else if self.eat_contextual_keyword("QUICK")? {
                CheckTableOption::Quick
            } else if self.eat_contextual_keyword("FAST")? {
                CheckTableOption::Fast
            } else if self.eat_contextual_keyword("MEDIUM")? {
                CheckTableOption::Medium
            } else if self.eat_contextual_keyword("EXTENDED")? {
                CheckTableOption::Extended
            } else if self.eat_contextual_keyword("CHANGED")? {
                CheckTableOption::Changed
            } else {
                break;
            };
            options.push(option);
        }
        Ok(options)
    }

    /// Parse the single optional `CHECKSUM TABLE` mode (`opt_checksum_type`): `QUICK` or
    /// `EXTENDED`, mutually exclusive.
    fn parse_optional_checksum_table_option(&mut self) -> ParseResult<Option<ChecksumTableOption>> {
        if self.eat_contextual_keyword("QUICK")? {
            Ok(Some(ChecksumTableOption::Quick))
        } else if self.eat_contextual_keyword("EXTENDED")? {
            Ok(Some(ChecksumTableOption::Extended))
        } else {
            Ok(None)
        }
    }

    /// Parse the repeatable `REPAIR TABLE` repair-type option list (`opt_mi_repair_types`);
    /// order and repeats are preserved so the statement round-trips.
    fn parse_repair_table_options(&mut self) -> ParseResult<ThinVec<RepairTableOption>> {
        let mut options = ThinVec::new();
        loop {
            let option = if self.eat_contextual_keyword("QUICK")? {
                RepairTableOption::Quick
            } else if self.eat_contextual_keyword("EXTENDED")? {
                RepairTableOption::Extended
            } else if self.eat_contextual_keyword("USE_FRM")? {
                RepairTableOption::UseFrm
            } else {
                break;
            };
            options.push(option);
        }
        Ok(options)
    }

    /// Parse a MySQL `CACHE INDEX <t> [<keys>][, ...] [PARTITION (...)] IN <cache>` key-cache
    /// assignment into [`Statement::CacheIndex`], reached under
    /// [`UtilitySyntax::key_cache_statements`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The list arm and the single-table `PARTITION` arm are mutually exclusive (`sql_yacc.yy`
    /// `keycache_stmt`): they diverge on the token after the first table, so both share the
    /// leading `parse_object_name` before [`parse_cache_index_targets`](Self::parse_cache_index_targets)
    /// branches on a following `PARTITION`.
    pub(super) fn parse_cache_index_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Cache)?;
        self.expect_keyword(Keyword::Index)?;
        let targets = self.parse_cache_index_targets()?;
        self.expect_keyword(Keyword::In)?;
        let cache = self.parse_key_cache_name()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::CacheIndex {
            cache_index: Box::new(CacheIndexStatement {
                targets,
                cache,
                meta,
            }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `LOAD INDEX INTO CACHE <t> [PARTITION (...)] [<keys>] [IGNORE LEAVES][, ...]`
    /// index-preload into [`Statement::LoadIndex`], reached under
    /// [`UtilitySyntax::key_cache_statements`](crate::ast::dialect::UtilitySyntax). Same
    /// list-vs-partition split as `CACHE INDEX`, with a per-table `IGNORE LEAVES` tail and no
    /// `IN <cache>` clause (`sql_yacc.yy` `preload_stmt`).
    pub(super) fn parse_load_index_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Load)?;
        self.expect_keyword(Keyword::Index)?;
        self.expect_keyword(Keyword::Into)?;
        self.expect_keyword(Keyword::Cache)?;
        let targets = self.parse_load_index_targets()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::LoadIndex {
            load_index: Box::new(LoadIndexStatement { targets, meta }),
            meta: statement_meta,
        })
    }

    /// Read the `CACHE INDEX` target(s): a `PARTITION` after the first table selects the
    /// single-table [`CacheIndexTargets::Partition`] arm (no list); otherwise the comma-joined
    /// [`CacheIndexTargets::Tables`] arm, each entry an optional key list.
    fn parse_cache_index_targets(&mut self) -> ParseResult<CacheIndexTargets> {
        let start = self.current_span()?;
        let table = self.parse_object_name()?;
        if self.peek_is_keyword(Keyword::Partition)? {
            let partition = self.parse_partition_selection()?;
            let keys = self.parse_optional_cache_index_key_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CacheIndexTargets::Partition {
                table,
                partition,
                keys,
                meta,
            });
        }
        let keys = self.parse_optional_cache_index_key_list()?;
        let first_meta = self.make_meta(start.union(self.preceding_span()));
        let mut tables = ThinVec::new();
        tables.push(CacheIndexTable {
            table,
            keys,
            meta: first_meta,
        });
        while self.eat_punct(Punctuation::Comma)? {
            let entry_start = self.current_span()?;
            let table = self.parse_object_name()?;
            let keys = self.parse_optional_cache_index_key_list()?;
            let meta = self.make_meta(entry_start.union(self.preceding_span()));
            tables.push(CacheIndexTable { table, keys, meta });
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CacheIndexTargets::Tables { tables, meta })
    }

    /// Read the `LOAD INDEX INTO CACHE` target(s): the [`LoadIndexTargets`] mirror of
    /// [`parse_cache_index_targets`](Self::parse_cache_index_targets), with a per-table
    /// `IGNORE LEAVES` flag after each key list.
    fn parse_load_index_targets(&mut self) -> ParseResult<LoadIndexTargets> {
        let start = self.current_span()?;
        let table = self.parse_object_name()?;
        if self.peek_is_keyword(Keyword::Partition)? {
            let partition = self.parse_partition_selection()?;
            let keys = self.parse_optional_cache_index_key_list()?;
            let ignore_leaves = self.parse_optional_ignore_leaves()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(LoadIndexTargets::Partition {
                table,
                partition,
                keys,
                ignore_leaves,
                meta,
            });
        }
        let keys = self.parse_optional_cache_index_key_list()?;
        let ignore_leaves = self.parse_optional_ignore_leaves()?;
        let first_meta = self.make_meta(start.union(self.preceding_span()));
        let mut tables = ThinVec::new();
        tables.push(LoadIndexTable {
            table,
            keys,
            ignore_leaves,
            meta: first_meta,
        });
        while self.eat_punct(Punctuation::Comma)? {
            let entry_start = self.current_span()?;
            let table = self.parse_object_name()?;
            let keys = self.parse_optional_cache_index_key_list()?;
            let ignore_leaves = self.parse_optional_ignore_leaves()?;
            let meta = self.make_meta(entry_start.union(self.preceding_span()));
            tables.push(LoadIndexTable {
                table,
                keys,
                ignore_leaves,
                meta,
            });
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LoadIndexTargets::Tables { tables, meta })
    }

    /// Parse the optional `{INDEX | KEY} (<key>[, ...])` list shared by the key-cache
    /// statements (`opt_cache_key_list`). The parenthesized name list may be empty
    /// (`INDEX ()`); `PRIMARY` is admitted unquoted as a key name (`key_usage_element`).
    fn parse_optional_cache_index_key_list(&mut self) -> ParseResult<Option<CacheIndexKeyList>> {
        let start = self.current_span()?;
        let keyword = if self.eat_keyword(Keyword::Index)? {
            CacheIndexKeyword::Index
        } else if self.eat_keyword(Keyword::Key)? {
            CacheIndexKeyword::Key
        } else {
            return Ok(None);
        };
        self.expect_punct(Punctuation::LParen, "`(` to open the key-cache index list")?;
        let keys = if self.peek_is_punct(Punctuation::RParen)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated(Self::parse_cache_index_key)?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the key-cache index list")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(CacheIndexKeyList {
            keyword,
            keys,
            meta,
        }))
    }

    /// Parse one key-cache index name: an ordinary identifier, or the reserved `PRIMARY`
    /// keyword (the primary key), admitted here unquoted (`key_usage_element: ident |
    /// PRIMARY_SYM`) and preserved as an unquoted [`Ident`].
    fn parse_cache_index_key(&mut self) -> ParseResult<Ident> {
        if self.peek_is_keyword(Keyword::Primary)? {
            return self.parse_ident_admitting(KeywordSet::EMPTY, "a key-cache index name");
        }
        self.parse_ident()
    }

    /// Consume the optional trailing `IGNORE LEAVES` flag of a `LOAD INDEX INTO CACHE` entry
    /// (`opt_ignore_leaves`; `LEAVES` is a plain contextual word).
    fn parse_optional_ignore_leaves(&mut self) -> ParseResult<bool> {
        if self.peek_is_keyword(Keyword::Ignore)?
            && self.peek_nth_is_contextual_keyword(1, "LEAVES")?
        {
            self.expect_keyword(Keyword::Ignore)?;
            self.expect_contextual_keyword("LEAVES")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse the `PARTITION (ALL | <name>[, ...])` selection of a partitioned key-cache
    /// statement (`adm_partition` / `all_or_alt_part_name_list`).
    fn parse_partition_selection(&mut self) -> ParseResult<PartitionSelection> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Partition)?;
        self.expect_punct(Punctuation::LParen, "`(` to open the partition list")?;
        let all = self.eat_keyword(Keyword::All)?;
        let names = if all {
            ThinVec::new()
        } else {
            self.parse_comma_separated(Self::parse_ident)?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the partition list")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(if all {
            PartitionSelection::All { meta }
        } else {
            PartitionSelection::Names { names, meta }
        })
    }

    /// Parse the `IN {<cache_name> | DEFAULT}` destination of a `CACHE INDEX` statement
    /// (`key_cache_name`).
    fn parse_key_cache_name(&mut self) -> ParseResult<KeyCacheName> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Default)? {
            Ok(KeyCacheName::Default {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else {
            let name = self.parse_ident()?;
            Ok(KeyCacheName::Named {
                name,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        }
    }

    /// Parse a MySQL standalone `RENAME {TABLE | TABLES} <a> TO <b>[, …]` /
    /// `RENAME USER <u> TO <v>[, …]` object-rename statement into [`Statement::Rename`],
    /// reached under [`UtilitySyntax::rename_statement`](crate::ast::dialect::UtilitySyntax).
    pub(super) fn parse_rename_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("RENAME")?;
        let rename = if self.eat_contextual_keyword("USER")? {
            let renames = self.parse_comma_separated(Self::parse_user_rename)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            RenameStatement::User { renames, meta }
        } else {
            let table_keyword = self.parse_table_or_tables_keyword()?;
            let renames = self.parse_comma_separated(Self::parse_table_rename)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            RenameStatement::Table {
                table_keyword,
                renames,
                meta,
            }
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Rename {
            rename: Box::new(rename),
            meta: statement_meta,
        })
    }

    /// Parse one `<from> TO <to>` table-rename mapping.
    fn parse_table_rename(&mut self) -> ParseResult<TableRename> {
        let start = self.current_span()?;
        let from = self.parse_object_name()?;
        self.expect_contextual_keyword("TO")?;
        let to = self.parse_object_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableRename { from, to, meta })
    }

    /// Parse one `<from> TO <to>` account-rename mapping.
    fn parse_user_rename(&mut self) -> ParseResult<UserRename> {
        let start = self.current_span()?;
        let from = self.parse_account_name()?;
        self.expect_contextual_keyword("TO")?;
        let to = self.parse_account_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(UserRename { from, to, meta })
    }

    /// Parse a MySQL account name — the full `user` grammar axis: a named `<user>[@<host>]`
    /// account or the `CURRENT_USER [()]` self-reference.
    ///
    /// Each part of a named account is an `ident_or_text` (bare/backtick identifier or a quoted
    /// `'…'`/`"…"` string), folded to an [`Ident`] whose quote style round-trips. The `@host`
    /// split rides the tokenizer boundary the maintenance landing established and this axis
    /// completes: MySQL's context-free lexer folds an *unquoted* `@host` into one user-variable
    /// token (host recovered by stripping the `@` sigil), while a *quoted* host cannot fold
    /// there and the lexer emits a standalone `@` (`Punctuation::At`) followed by the quoted
    /// `ident_or_text` — `parse_optional_account_host` reconciles both. A bare user with no
    /// host leaves `host` `None`.
    pub(super) fn parse_account_name(&mut self) -> ParseResult<AccountName> {
        let start = self.current_span()?;
        // `CURRENT_USER [()]` — the current-account self-reference (`CURRENT_USER` lexes as a
        // keyword; `eat_contextual_keyword` matches it, as the `DEFINER` prefix does).
        if self.eat_contextual_keyword("CURRENT_USER")? {
            let parens = if self.eat_punct(Punctuation::LParen)? {
                self.expect_punct(Punctuation::RParen, "`)` to close `CURRENT_USER()`")?;
                true
            } else {
                false
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AccountName::CurrentUser { parens, meta });
        }
        let user = self.parse_ident_or_text()?;
        let host = self.parse_optional_account_host()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccountName::Account { user, host, meta })
    }

    /// Consume the optional `@<host>` part of a named account, from either tokenizer spelling
    /// of the `@` boundary.
    ///
    /// * An unquoted host folds into one user-variable token (`@localhost`); the host is the
    ///   token text with its `@` sigil stripped (a `@@…` system-variable token is not an
    ///   account host and is left alone).
    /// * A quoted/backtick host cannot fold — the lexer emits a standalone `@`
    ///   (`Punctuation::At`) and the host is the following `ident_or_text` (`@'h'`, `@"h"`,
    ///   `` @`h` ``), whose quote style round-trips.
    fn parse_optional_account_host(&mut self) -> ParseResult<Option<Ident>> {
        self.parse_optional_user_variable_ref()
    }

    /// Read an optional `'@' ident_or_text` user-variable *reference* — the shared read-only
    /// `@`-name surface behind MySQL's account-host `@host`, the `PREPARE ... FROM @var`
    /// source, and the `EXECUTE ... USING @var` list — from either tokenizer spelling of the
    /// `@` boundary, returning the name with its `@` sigil stripped and quote style preserved:
    ///
    /// * An unquoted name folds into one user-variable token (`@v`); the name is the token
    ///   text with its `@` sigil stripped. A `@@…` system-variable token is *not* a
    ///   single-`@` reference and is left untouched (returns `None`), so the caller reports
    ///   its own "expected `@variable`" error rather than mis-reading a system variable.
    /// * A quoted/backtick name cannot fold — the lexer emits a standalone `@`
    ///   (`Punctuation::At`) and the name is the following `ident_or_text` (`@'v'`, `@"v"`,
    ///   `` @`v` ``), whose quote style round-trips.
    ///
    /// This reads a reference only; it never consumes the `SET @v := …` assignment tail, so it
    /// is safe to share with the assignment grammar without colliding on it.
    fn parse_optional_user_variable_ref(&mut self) -> ParseResult<Option<Ident>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        match token.kind {
            TokenKind::Variable => {
                let text = self.span_text(token.span);
                let Some(name_text) = text.strip_prefix('@').filter(|rest| !rest.starts_with('@'))
                else {
                    return Ok(None);
                };
                let sym = self.intern_text(name_text);
                self.advance()?; // consume the `@name` token
                let meta = self.make_meta(token.span);
                Ok(Some(Ident {
                    sym,
                    quote: QuoteStyle::None,
                    meta,
                }))
            }
            TokenKind::Punctuation(Punctuation::At) => {
                self.advance()?; // consume the standalone `@`
                let name = self.parse_ident_or_text()?;
                Ok(Some(name))
            }
            _ => Ok(None),
        }
    }

    /// Parse an `ident_or_text` — a bare/backtick identifier, or a quoted string folded to
    /// an [`Ident`] whose quote style round-trips.
    pub(super) fn parse_ident_or_text(&mut self) -> ParseResult<Ident> {
        if let Some(ident) = self.parse_string_alias_ident()? {
            return Ok(ident);
        }
        self.parse_ident()
    }

    // --- FLUSH / PURGE (MySQL) ----------------------------------------------

    /// Parse a MySQL `FLUSH [NO_WRITE_TO_BINLOG | LOCAL] <target>` server-administration
    /// statement into [`Statement::Flush`], reached under
    /// [`UtilitySyntax::flush`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The optional binlog-suppression prefix reuses the admin-table
    /// [`parse_optional_no_write_to_binlog`](Self::parse_optional_no_write_to_binlog); the
    /// target splits on the leading `{TABLE | TABLES}` keyword (the table form) against every
    /// other leading keyword (the comma-separated keyword-target list). The two are the
    /// grammar's mutually-exclusive `flush_options` alternatives — `TABLES` never joins the
    /// list.
    pub(super) fn parse_flush_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("FLUSH")?;
        let no_write_to_binlog = self.parse_optional_no_write_to_binlog()?;
        let target = self.parse_flush_target(start)?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Flush {
            flush: Box::new(FlushStatement {
                no_write_to_binlog,
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse the `flush_options` body — the `{TABLE | TABLES} [<list>] [WITH READ LOCK | FOR
    /// EXPORT]` form when `TABLE`/`TABLES` leads, else the comma-separated keyword-target
    /// list. `start` is the whole-statement start (the target `meta` mirrors the statement
    /// span, the hoisted-spine convention). `FOR EXPORT` requires a non-empty table list —
    /// enforced here, matching the mysql:8.4.10 `ER_PARSE_ERROR` on `FLUSH TABLES FOR EXPORT`.
    fn parse_flush_target(&mut self, start: Span) -> ParseResult<FlushTarget> {
        if self.peek_is_contextual_keyword("TABLE")? || self.peek_is_contextual_keyword("TABLES")? {
            let table_keyword = self.parse_table_or_tables_keyword()?;
            // `opt_table_list`: a list follows only when the next token is neither a lock
            // keyword (`WITH`/`FOR`) nor the statement end.
            let has_table_list = self.peek()?.is_some()
                && !self.peek_is_punct(Punctuation::Semicolon)?
                && !self.peek_is_contextual_keyword("WITH")?
                && !self.peek_is_contextual_keyword("FOR")?;
            let tables = if has_table_list {
                self.parse_comma_separated(Self::parse_object_name)?
            } else {
                ThinVec::new()
            };
            let lock = self.parse_optional_flush_tables_lock()?;
            if matches!(lock, Some(FlushTablesLock::ForExport)) && tables.is_empty() {
                return Err(self.unexpected("a table list before `FOR EXPORT`"));
            }
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(FlushTarget::Tables {
                table_keyword,
                tables,
                lock,
                meta,
            })
        } else {
            let options = self.parse_comma_separated(Self::parse_flush_option)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(FlushTarget::Options { options, meta })
        }
    }

    /// Consume the optional trailing `WITH READ LOCK | FOR EXPORT` lock clause on the `FLUSH
    /// TABLES` form (MySQL's `opt_flush_lock`); `None` when neither leads.
    fn parse_optional_flush_tables_lock(&mut self) -> ParseResult<Option<FlushTablesLock>> {
        if self.eat_contextual_keyword("WITH")? {
            self.expect_contextual_keyword("READ")?;
            self.expect_contextual_keyword("LOCK")?;
            Ok(Some(FlushTablesLock::WithReadLock))
        } else if self.eat_contextual_keyword("FOR")? {
            self.expect_contextual_keyword("EXPORT")?;
            Ok(Some(FlushTablesLock::ForExport))
        } else {
            Ok(None)
        }
    }

    /// Parse one `flush_option` keyword target. The `<x> LOGS` compound forms
    /// (`BINARY`/`ENGINE`/`ERROR`/`GENERAL`/`SLOW`/`RELAY`) lead on their first word, so the
    /// bare `LOGS` (`FLUSH LOGS`) is tried last; `RELAY LOGS` additionally reads an optional
    /// `FOR CHANNEL '<name>'`.
    fn parse_flush_option(&mut self) -> ParseResult<FlushOption> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("PRIVILEGES")? {
            Ok(FlushOption::Privileges {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("STATUS")? {
            Ok(FlushOption::Status {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("USER_RESOURCES")? {
            Ok(FlushOption::UserResources {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("OPTIMIZER_COSTS")? {
            Ok(FlushOption::OptimizerCosts {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("BINARY")? {
            self.expect_contextual_keyword("LOGS")?;
            Ok(FlushOption::BinaryLogs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("ENGINE")? {
            self.expect_contextual_keyword("LOGS")?;
            Ok(FlushOption::EngineLogs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("ERROR")? {
            self.expect_contextual_keyword("LOGS")?;
            Ok(FlushOption::ErrorLogs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("GENERAL")? {
            self.expect_contextual_keyword("LOGS")?;
            Ok(FlushOption::GeneralLogs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("SLOW")? {
            self.expect_contextual_keyword("LOGS")?;
            Ok(FlushOption::SlowLogs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("RELAY")? {
            self.expect_contextual_keyword("LOGS")?;
            let channel = self.parse_optional_for_channel()?;
            Ok(FlushOption::RelayLogs {
                channel,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("LOGS")? {
            Ok(FlushOption::Logs {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else {
            Err(self.unexpected("a FLUSH target keyword"))
        }
    }

    /// Parse a MySQL `PURGE BINARY LOGS {TO '<log>' | BEFORE <datetime>}` binary-log purge
    /// statement into [`Statement::Purge`], reached under
    /// [`UtilitySyntax::purge_binary_logs`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The `BINARY LOGS` keywords are fixed — MySQL 8.4 dropped the `MASTER` synonym — and
    /// exactly one target clause is required (`TO '<log>'` or `BEFORE <expr>`), a bare `PURGE
    /// BINARY LOGS` erroring as mysql:8.4.10 does. `BEFORE` takes a full expression via
    /// [`parse_expr`](Self::parse_expr).
    pub(super) fn parse_purge_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("PURGE")?;
        self.expect_contextual_keyword("BINARY")?;
        self.expect_contextual_keyword("LOGS")?;
        let target_start = self.current_span()?;
        let target = if self.eat_contextual_keyword("TO")? {
            let log =
                self.expect_string_literal("a binary-log file name after `PURGE BINARY LOGS TO`")?;
            let meta = self.make_meta(target_start.union(self.preceding_span()));
            PurgeTarget::To { log, meta }
        } else {
            self.expect_contextual_keyword("BEFORE")?;
            let datetime = self.parse_expr()?;
            let meta = self.make_meta(target_start.union(self.preceding_span()));
            PurgeTarget::Before { datetime, meta }
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Purge {
            purge: Box::new(PurgeStatement {
                target,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- Replication administration (MySQL) ---------------------------------

    /// Whether the leading tokens start a replication-administration statement — `CHANGE
    /// REPLICATION …`, `START`/`STOP REPLICA`, or `START`/`STOP GROUP_REPLICATION`. A
    /// two-word lookahead so it refines the shared `CHANGE`/`START`/`STOP` keywords without
    /// stealing `START TRANSACTION` (or any other use of those words) from the transaction
    /// dispatcher.
    pub(super) fn peek_starts_replication_statement(&mut self) -> ParseResult<bool> {
        if self.peek_is_contextual_keyword("CHANGE")? {
            return self.peek_nth_is_contextual_keyword(1, "REPLICATION");
        }
        if self.peek_is_contextual_keyword("START")? || self.peek_is_contextual_keyword("STOP")? {
            return Ok(self.peek_nth_is_contextual_keyword(1, "REPLICA")?
                || self.peek_nth_is_contextual_keyword(1, "GROUP_REPLICATION")?);
        }
        Ok(false)
    }

    /// Parse a MySQL replication-administration statement into
    /// [`Statement::Replication`], reached under
    /// [`UtilitySyntax::replication_statements`](crate::ast::dialect::UtilitySyntax) once
    /// [`peek_starts_replication_statement`](Self::peek_starts_replication_statement) has
    /// claimed the leading keywords.
    pub(super) fn parse_replication_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let replication = self.parse_replication_statement_kind(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Replication {
            replication: Box::new(replication),
            meta,
        })
    }

    fn parse_replication_statement_kind(
        &mut self,
        start: Span,
    ) -> ParseResult<ReplicationStatement> {
        if self.eat_contextual_keyword("CHANGE")? {
            self.expect_contextual_keyword("REPLICATION")?;
            if self.eat_contextual_keyword("SOURCE")? {
                self.expect_contextual_keyword("TO")?;
                self.parse_change_replication_source(start)
            } else {
                self.expect_contextual_keyword("FILTER")?;
                self.parse_change_replication_filter(start)
            }
        } else if self.eat_contextual_keyword("START")? {
            if self.eat_contextual_keyword("GROUP_REPLICATION")? {
                self.parse_start_group_replication(start)
            } else {
                self.expect_contextual_keyword("REPLICA")?;
                self.parse_start_replica(start)
            }
        } else {
            self.expect_contextual_keyword("STOP")?;
            if self.eat_contextual_keyword("GROUP_REPLICATION")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(ReplicationStatement::StopGroupReplication { meta })
            } else {
                self.expect_contextual_keyword("REPLICA")?;
                self.parse_stop_replica(start)
            }
        }
    }

    /// `CHANGE REPLICATION SOURCE TO <option-list> [FOR CHANNEL '<ch>']`. The option list is
    /// non-empty (`parse_comma_separated` requires one) and the channel is a trailing suffix,
    /// so an option after the channel never reduces (matching the engine's `ER_PARSE_ERROR`).
    fn parse_change_replication_source(
        &mut self,
        start: Span,
    ) -> ParseResult<ReplicationStatement> {
        let options = self.parse_comma_separated(Self::parse_change_replication_source_option)?;
        let channel = self.parse_optional_for_channel()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ReplicationStatement::ChangeSource {
            options,
            channel,
            meta,
        })
    }

    /// Parse one `<name> = <value>` `CHANGE REPLICATION SOURCE TO` option. Table-driven over
    /// the measured [`SourceOption`] set (each spelling is a whole-identifier token, so the
    /// scan order is irrelevant); the value shape the name dictates is read by
    /// [`parse_source_option_value`](Self::parse_source_option_value).
    fn parse_change_replication_source_option(
        &mut self,
    ) -> ParseResult<ChangeReplicationSourceOption> {
        let start = self.current_span()?;
        for &(option, shape) in SOURCE_OPTION_TABLE {
            if self.eat_contextual_keyword(option.keyword())? {
                self.expect_op(
                    Operator::Eq,
                    "`=` after a CHANGE REPLICATION SOURCE option name",
                )?;
                let value = self.parse_source_option_value(shape)?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(ChangeReplicationSourceOption {
                    name: option,
                    value,
                    meta,
                });
            }
        }
        Err(self.unexpected("a CHANGE REPLICATION SOURCE TO option name"))
    }

    fn parse_source_option_value(
        &mut self,
        shape: SourceOptionShape,
    ) -> ParseResult<ChangeReplicationSourceOptionValue> {
        let start = self.current_span()?;
        let value = match shape {
            SourceOptionShape::String => {
                let value = self.expect_string_literal("a string value after `=`")?;
                ChangeReplicationSourceOptionValue::String {
                    value,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::Number => {
                let value = self.parse_numeric_literal("a numeric value after `=`")?;
                ChangeReplicationSourceOptionValue::Number {
                    value,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::NullableString => {
                let value = if self.eat_contextual_keyword("NULL")? {
                    None
                } else {
                    Some(self.expect_string_literal("a string value or `NULL` after `=`")?)
                };
                ChangeReplicationSourceOptionValue::NullableString {
                    value,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::User => {
                let account = if self.eat_contextual_keyword("NULL")? {
                    None
                } else {
                    Some(self.parse_privilege_checks_user()?)
                };
                ChangeReplicationSourceOptionValue::User {
                    account,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::ServerIds => {
                let ids =
                    self.parse_parenthesized_list(Self::parse_replication_unsigned_literal)?;
                ChangeReplicationSourceOptionValue::ServerIds {
                    ids,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::PrimaryKeyCheck => {
                let check = if self.eat_contextual_keyword("ON")? {
                    RequirePrimaryKeyCheck::On
                } else if self.eat_contextual_keyword("OFF")? {
                    RequirePrimaryKeyCheck::Off
                } else if self.eat_contextual_keyword("STREAM")? {
                    RequirePrimaryKeyCheck::Stream
                } else if self.eat_contextual_keyword("GENERATE")? {
                    RequirePrimaryKeyCheck::Generate
                } else {
                    return Err(self.unexpected(
                        "ON, OFF, STREAM, or GENERATE after REQUIRE_TABLE_PRIMARY_KEY_CHECK =",
                    ));
                };
                ChangeReplicationSourceOptionValue::PrimaryKeyCheck {
                    check,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
            SourceOptionShape::AssignGtids => {
                let (kind, uuid) = if self.eat_contextual_keyword("OFF")? {
                    (AssignGtidsKind::Off, None)
                } else if self.eat_contextual_keyword("LOCAL")? {
                    (AssignGtidsKind::Local, None)
                } else {
                    let uuid = self.expect_string_literal(
                        "OFF, LOCAL, or a UUID string after \
                         ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS =",
                    )?;
                    (AssignGtidsKind::Uuid, Some(uuid))
                };
                ChangeReplicationSourceOptionValue::AssignGtids {
                    kind,
                    uuid,
                    meta: self.make_meta(start.union(self.preceding_span())),
                }
            }
        };
        Ok(value)
    }

    /// Parse the `user_ident_or_text [@ host]` account of a `PRIVILEGE_CHECKS_USER` option.
    /// Narrower than [`parse_account_name`](Self::parse_account_name): the grammar admits an
    /// ident/text user with an optional host, but not the `CURRENT_USER` self-reference.
    fn parse_privilege_checks_user(&mut self) -> ParseResult<AccountName> {
        let start = self.current_span()?;
        let user = self.parse_ident_or_text()?;
        let host = self.parse_optional_account_host()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccountName::Account { user, host, meta })
    }

    /// `CHANGE REPLICATION FILTER <rule-list> [FOR CHANNEL '<ch>']`.
    fn parse_change_replication_filter(
        &mut self,
        start: Span,
    ) -> ParseResult<ReplicationStatement> {
        let rules = self.parse_comma_separated(Self::parse_replication_filter_rule)?;
        let channel = self.parse_optional_for_channel()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ReplicationStatement::ChangeFilter {
            rules,
            channel,
            meta,
        })
    }

    fn parse_replication_filter_rule(&mut self) -> ParseResult<ReplicationFilterRule> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("REPLICATE_DO_DB")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_DO_DB")?;
            let databases = self.parse_parenthesized_list(Self::parse_ident)?;
            Ok(ReplicationFilterRule::DoDb {
                databases,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_IGNORE_DB")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_IGNORE_DB")?;
            let databases = self.parse_parenthesized_list(Self::parse_ident)?;
            Ok(ReplicationFilterRule::IgnoreDb {
                databases,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_DO_TABLE")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_DO_TABLE")?;
            let tables = self.parse_parenthesized_list(Self::parse_filter_table_ident)?;
            Ok(ReplicationFilterRule::DoTable {
                tables,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_IGNORE_TABLE")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_IGNORE_TABLE")?;
            let tables = self.parse_parenthesized_list(Self::parse_filter_table_ident)?;
            Ok(ReplicationFilterRule::IgnoreTable {
                tables,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_WILD_DO_TABLE")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_WILD_DO_TABLE")?;
            let patterns = self.parse_parenthesized_list(Self::parse_filter_string)?;
            Ok(ReplicationFilterRule::WildDoTable {
                patterns,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_WILD_IGNORE_TABLE")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_WILD_IGNORE_TABLE")?;
            let patterns = self.parse_parenthesized_list(Self::parse_filter_string)?;
            Ok(ReplicationFilterRule::WildIgnoreTable {
                patterns,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("REPLICATE_REWRITE_DB")? {
            self.expect_op(Operator::Eq, "`=` after REPLICATE_REWRITE_DB")?;
            let pairs = self.parse_parenthesized_list(Self::parse_rewrite_db_pair)?;
            Ok(ReplicationFilterRule::RewriteDb {
                pairs,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else {
            Err(self.unexpected("a CHANGE REPLICATION FILTER rule name"))
        }
    }

    /// A schema-qualified `db.t` table name for a `CHANGE REPLICATION FILTER` table rule
    /// (`filter_table_ident` is `schema '.' ident`, so a bare unqualified name is
    /// `ER_PARSE_ERROR` — enforced here so parse acceptance tracks the engine).
    fn parse_filter_table_ident(&mut self) -> ParseResult<ObjectName> {
        let name = self.parse_object_name()?;
        if name.0.len() != 2 {
            return Err(self.unexpected(
                "a schema-qualified `db.table` name in a CHANGE REPLICATION FILTER table rule",
            ));
        }
        Ok(name)
    }

    fn parse_filter_string(&mut self) -> ParseResult<Literal> {
        self.expect_string_literal("a wildcard pattern string")
    }

    fn parse_rewrite_db_pair(&mut self) -> ParseResult<RewriteDbPair> {
        let start = self.current_span()?;
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open a REPLICATE_REWRITE_DB pair",
        )?;
        let from = self.parse_ident()?;
        self.expect_punct(
            Punctuation::Comma,
            "`,` between the two databases of a rewrite pair",
        )?;
        let to = self.parse_ident()?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close a REPLICATE_REWRITE_DB pair",
        )?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(RewriteDbPair { from, to, meta })
    }

    /// `START REPLICA [<threads>] [UNTIL <cond-list>] [<connection>] [FOR CHANNEL '<ch>']`.
    fn parse_start_replica(&mut self, start: Span) -> ParseResult<ReplicationStatement> {
        let threads = self.parse_replica_thread_options()?;
        let until = if self.eat_contextual_keyword("UNTIL")? {
            self.parse_replica_until()?
        } else {
            ThinVec::new()
        };
        let user = self.parse_replica_connection_option("USER")?;
        let password = self.parse_replica_connection_option("PASSWORD")?;
        let default_auth = self.parse_replica_connection_option("DEFAULT_AUTH")?;
        let plugin_dir = self.parse_replica_connection_option("PLUGIN_DIR")?;
        let channel = self.parse_optional_for_channel()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ReplicationStatement::StartReplica {
            threads,
            until,
            user,
            password,
            default_auth,
            plugin_dir,
            channel,
            meta,
        })
    }

    /// `STOP REPLICA [<threads>] [FOR CHANNEL '<ch>']` — no `UNTIL`/connection tail.
    fn parse_stop_replica(&mut self, start: Span) -> ParseResult<ReplicationStatement> {
        let threads = self.parse_replica_thread_options()?;
        let channel = self.parse_optional_for_channel()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ReplicationStatement::StopReplica {
            threads,
            channel,
            meta,
        })
    }

    /// One `USER`/`PASSWORD`/`DEFAULT_AUTH`/`PLUGIN_DIR` fixed-position optional of `START
    /// REPLICA` (`opt_user_option` …); `None` when the keyword does not lead.
    fn parse_replica_connection_option(
        &mut self,
        keyword: &'static str,
    ) -> ParseResult<Option<Literal>> {
        if self.eat_contextual_keyword(keyword)? {
            self.expect_op(Operator::Eq, "`=` after a START REPLICA connection option")?;
            Ok(Some(
                self.expect_string_literal("a string value after `=`")?,
            ))
        } else {
            Ok(None)
        }
    }

    /// The `opt_replica_thread_option_list` — a comma-separated `SQL_THREAD`/`IO_THREAD` list,
    /// possibly empty (no threads named).
    fn parse_replica_thread_options(&mut self) -> ParseResult<ThinVec<ReplicaThreadOption>> {
        let mut threads = ThinVec::new();
        if let Some(first) = self.try_parse_replica_thread_option()? {
            threads.push(first);
            while self.eat_punct(Punctuation::Comma)? {
                let next = self.try_parse_replica_thread_option()?.ok_or_else(|| {
                    self.unexpected("SQL_THREAD, IO_THREAD, or RELAY_THREAD after `,`")
                })?;
                threads.push(next);
            }
        }
        Ok(threads)
    }

    fn try_parse_replica_thread_option(&mut self) -> ParseResult<Option<ReplicaThreadOption>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SQL_THREAD")? {
            Ok(Some(ReplicaThreadOption::Sql {
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else if self.eat_contextual_keyword("IO_THREAD")? {
            Ok(Some(ReplicaThreadOption::Io {
                keyword: IoThreadKeyword::Io,
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else if self.eat_contextual_keyword("RELAY_THREAD")? {
            Ok(Some(ReplicaThreadOption::Io {
                keyword: IoThreadKeyword::Relay,
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else {
            Ok(None)
        }
    }

    /// The `replica_until` condition list. Any single condition may lead, but only the
    /// file/position coordinates may follow a comma — a GTID/gaps condition is admitted only
    /// as the first element (so `UNTIL SQL_AFTER_GTIDS = 'x', SQL_BEFORE_GTIDS = 'y'` rejects,
    /// as on the engine), while the coherence of the combination is a later semantic check.
    fn parse_replica_until(&mut self) -> ParseResult<ThinVec<ReplicaUntilCondition>> {
        let mut conditions = ThinVec::new();
        conditions.push(self.parse_replica_until_condition(true)?);
        while self.eat_punct(Punctuation::Comma)? {
            conditions.push(self.parse_replica_until_condition(false)?);
        }
        Ok(conditions)
    }

    fn parse_replica_until_condition(
        &mut self,
        allow_gtid: bool,
    ) -> ParseResult<ReplicaUntilCondition> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SOURCE_LOG_FILE")? {
            self.expect_op(Operator::Eq, "`=` after SOURCE_LOG_FILE")?;
            let value = self.expect_string_literal("a log-file name after `=`")?;
            Ok(ReplicaUntilCondition::SourceLogFile {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("SOURCE_LOG_POS")? {
            self.expect_op(Operator::Eq, "`=` after SOURCE_LOG_POS")?;
            let value = self.parse_numeric_literal("a log position after `=`")?;
            Ok(ReplicaUntilCondition::SourceLogPos {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("RELAY_LOG_FILE")? {
            self.expect_op(Operator::Eq, "`=` after RELAY_LOG_FILE")?;
            let value = self.expect_string_literal("a relay-log file name after `=`")?;
            Ok(ReplicaUntilCondition::RelayLogFile {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if self.eat_contextual_keyword("RELAY_LOG_POS")? {
            self.expect_op(Operator::Eq, "`=` after RELAY_LOG_POS")?;
            let value = self.parse_numeric_literal("a relay-log position after `=`")?;
            Ok(ReplicaUntilCondition::RelayLogPos {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if allow_gtid && self.eat_contextual_keyword("SQL_BEFORE_GTIDS")? {
            self.expect_op(Operator::Eq, "`=` after SQL_BEFORE_GTIDS")?;
            let value = self.expect_string_literal("a GTID set after `=`")?;
            Ok(ReplicaUntilCondition::SqlBeforeGtids {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if allow_gtid && self.eat_contextual_keyword("SQL_AFTER_GTIDS")? {
            self.expect_op(Operator::Eq, "`=` after SQL_AFTER_GTIDS")?;
            let value = self.expect_string_literal("a GTID set after `=`")?;
            Ok(ReplicaUntilCondition::SqlAfterGtids {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if allow_gtid && self.eat_contextual_keyword("SQL_AFTER_MTS_GAPS")? {
            Ok(ReplicaUntilCondition::SqlAfterMtsGaps {
                meta: self.make_meta(start.union(self.preceding_span())),
            })
        } else if allow_gtid {
            Err(self.unexpected("a START REPLICA UNTIL condition"))
        } else {
            Err(self.unexpected(
                "a log-file/position UNTIL condition after `,` (a GTID/gaps condition may only \
                 lead the list)",
            ))
        }
    }

    /// `START GROUP_REPLICATION [<option-list>]` — the `USER`/`PASSWORD`/`DEFAULT_AUTH`
    /// options are comma-separated (unlike the space-separated `START REPLICA` connection
    /// tail) and possibly empty.
    fn parse_start_group_replication(&mut self, start: Span) -> ParseResult<ReplicationStatement> {
        let mut options = ThinVec::new();
        if let Some(first) = self.try_parse_group_replication_option()? {
            options.push(first);
            while self.eat_punct(Punctuation::Comma)? {
                let next = self
                    .try_parse_group_replication_option()?
                    .ok_or_else(|| self.unexpected("USER, PASSWORD, or DEFAULT_AUTH after `,`"))?;
                options.push(next);
            }
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ReplicationStatement::StartGroupReplication { options, meta })
    }

    fn try_parse_group_replication_option(
        &mut self,
    ) -> ParseResult<Option<GroupReplicationOption>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("USER")? {
            self.expect_op(Operator::Eq, "`=` after USER")?;
            let value = self.expect_string_literal("a user string after `=`")?;
            Ok(Some(GroupReplicationOption::User {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else if self.eat_contextual_keyword("PASSWORD")? {
            self.expect_op(Operator::Eq, "`=` after PASSWORD")?;
            let value = self.expect_string_literal("a password string after `=`")?;
            Ok(Some(GroupReplicationOption::Password {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else if self.eat_contextual_keyword("DEFAULT_AUTH")? {
            self.expect_op(Operator::Eq, "`=` after DEFAULT_AUTH")?;
            let value = self.expect_string_literal("an auth-plugin string after `=`")?;
            Ok(Some(GroupReplicationOption::DefaultAuth {
                value,
                meta: self.make_meta(start.union(self.preceding_span())),
            }))
        } else {
            Ok(None)
        }
    }

    /// A parenthesized, comma-separated list of `item`s, possibly empty (`()`), for the
    /// `CHANGE REPLICATION FILTER` argument lists and `IGNORE_SERVER_IDS`.
    fn parse_parenthesized_list<T>(
        &mut self,
        item: fn(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<ThinVec<T>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the list")?;
        let mut items = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            items.push(item(self)?);
            while self.eat_punct(Punctuation::Comma)? {
                items.push(item(self)?);
            }
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the list")?;
        Ok(items)
    }

    /// One unsigned-integer [`Literal`] (a `Number` token) — a `IGNORE_SERVER_IDS` element.
    fn parse_replication_unsigned_literal(&mut self) -> ParseResult<Literal> {
        self.parse_numeric_literal("a server id")
    }

    /// Consume one numeric-literal token (integer or fractional), classifying its
    /// [`LiteralKind`] from the source text; error with `context` otherwise.
    fn parse_numeric_literal(&mut self, context: &'static str) -> ParseResult<Literal> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                self.advance()?;
                Ok(Literal {
                    kind: number_literal_kind(
                        self.span_text(token.span),
                        self.float_as_decimal_enabled(),
                    ),
                    meta: self.make_meta(token.span),
                })
            }
            _ => Err(self.unexpected(context)),
        }
    }

    // --- USE (DuckDB / MySQL) -----------------------------------------------

    /// Parse a `USE <catalog> [. <schema>]` (DuckDB) / `USE <schema>` (MySQL) statement into
    /// [`Statement::Use`], reached under
    /// [`UtilitySyntax::use_statement`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The name arity is dialect data on
    /// [`UtilitySyntax::use_qualified_name`](crate::ast::dialect::UtilitySyntax): DuckDB's
    /// grammar admits a one- or two-part name (`USE db` / `USE db.schema`) and rejects a
    /// three-part `USE a.b.c` at parse (`Expected "USE database" or "USE database.schema"`),
    /// while MySQL's `USE ident` takes a single unqualified schema and `ER_PARSE_ERROR`s any
    /// dotted name (engine-measured on mysql:8). The shared
    /// [`parse_object_name`](Self::parse_object_name) consumes any number of dotted parts, so
    /// the dialect's arity bound is enforced here to keep parse acceptance aligned with the
    /// engine rather than over-accepting the deeper name.
    ///
    /// Under [`UtilitySyntax::use_string_literal_name`](crate::ast::dialect::UtilitySyntax)
    /// DuckDB also admits a single-part Sconst target (`USE 'n'`, `USE E'n'`, `USE $$n$$`;
    /// engine-measured on libduckdb 1.5.4). A dotted string name (`USE 'a'.'b'`) is left to
    /// the identifier path, which rejects the string token — matching DuckDB's parser
    /// error at the `.`.
    pub(super) fn parse_use_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("USE")?;
        let name = if self.features().utility_syntax.use_string_literal_name
            && self.peek_is_name_sconst()?
        {
            ObjectName(thin_vec![
                self.parse_name_sconst_ident("a schema name after USE")?
            ])
        } else {
            let name = self.parse_object_name()?;
            let max_parts = if self.features().utility_syntax.use_qualified_name {
                2
            } else {
                1
            };
            if name.0.len() > max_parts {
                return Err(self.unexpected(if max_parts == 2 {
                    "at most a two-part `catalog.schema` name after USE (DuckDB rejects a deeper name)"
                } else {
                    "a single unqualified schema name after USE (MySQL rejects a dotted name)"
                }));
            }
            name
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(Statement::Use {
            use_statement: Box::new(UseStatement { name, meta }),
            meta: statement_meta,
        })
    }

    /// Whether the current token is an Sconst spelling DuckDB admits as a name
    /// (`'…'`, `E'…'`, `$$…$$`); see [`string_literal_is_name_sconst`].
    pub(super) fn peek_is_name_sconst(&mut self) -> ParseResult<bool> {
        let Some(token) = self.peek()? else {
            return Ok(false);
        };
        Ok(token.kind == TokenKind::String
            && string_literal_is_name_sconst(self.span_text(token.span)))
    }

    /// Fold a name-position Sconst into an [`Ident`] with [`QuoteStyle::Single`] so the
    /// quotes round-trip on render. Plain `'…'` reuses
    /// [`parse_string_alias_ident`](Self::parse_string_alias_ident); escape `E'…'` and
    /// dollar-quoted forms materialize their value via [`Literal::as_str`] and still
    /// record `QuoteStyle::Single` (the semantic name is the string value; exact source
    /// fidelity of the `E`/`$$` spelling is not required for accept/reject parity).
    pub(super) fn parse_name_sconst_ident(&mut self, expected: &'static str) -> ParseResult<Ident> {
        if let Some(ident) = self.parse_string_alias_ident()? {
            return Ok(ident);
        }
        let literal = self.expect_string_literal(expected)?;
        let value = literal
            .as_str(self.source())
            .map_err(|_| self.unexpected(expected))?;
        let sym = self.intern_text(&value);
        Ok(Ident {
            sym,
            quote: QuoteStyle::Single,
            meta: literal.meta,
        })
    }

    // --- PREPARE / EXECUTE / DEALLOCATE / CALL (DuckDB) ----------------------

    /// Parse a `PREPARE <name> [ ( <type> [, ...] ) ] AS <statement>` statement into
    /// [`Statement::Prepare`], reached under
    /// [`UtilitySyntax::prepared_statements`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The body is parsed by the shared statement dispatcher
    /// ([`parse_statement`](Self::parse_statement)), which recursion-guards the nesting;
    /// any statement parses (the [`ExplainStatement`] "grammar accepts any statement"
    /// contract), and DuckDB restricts the preparable kinds at bind, not parse.
    ///
    /// The parenthesized parameter-type list is a widening of the name position, gated
    /// by its own
    /// [`prepare_typed_parameters`](crate::ast::dialect::UtilitySyntax::prepare_typed_parameters)
    /// flag: only when that flag is on and a `(` follows the name is the list consumed
    /// (a non-empty comma-separated [`parse_data_type`](Self::parse_data_type) list —
    /// PostgreSQL rejects an empty `()`). When the flag is off, a `(` after the name is
    /// left untouched and falls through to the `AS` expectation below, preserving
    /// today's error shape for dialects (DuckDB) that structurally reject the typed-list
    /// form ("Prepared statement argument types are not supported, use CAST").
    pub(super) fn parse_prepare_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("PREPARE")?;
        let name = self.parse_ident()?;
        let parameter_types = if self.features().utility_syntax.prepare_typed_parameters
            && self.peek_is_punct(Punctuation::LParen)?
        {
            self.expect_punct(
                Punctuation::LParen,
                "`(` to open the PREPARE parameter-type list",
            )?;
            let types = self.parse_comma_separated(Self::parse_data_type)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the PREPARE parameter-type list",
            )?;
            types
        } else {
            ThinVec::new()
        };
        self.expect_keyword(Keyword::As)?;
        let statement = self.parse_statement()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Prepare {
            prepare: Box::new(PrepareStatement {
                name,
                parameter_types,
                statement: Box::new(statement),
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse an `EXECUTE <name> [ ( <arg> [, …] ) ]` statement into
    /// [`Statement::Execute`], sharing the `prepared_statements` gate.
    ///
    /// The parenthesized argument list is optional; when present it must be non-empty —
    /// `parse_comma_separated` requires at least one item, so an empty `EXECUTE v1()` is
    /// rejected exactly as DuckDB does. A bare `EXECUTE v1` leaves `args` empty.
    pub(super) fn parse_execute_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("EXECUTE")?;
        let execute = self.parse_execute_statement_body(start)?;
        let statement_meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::Execute {
            execute: Box::new(execute),
            meta: statement_meta,
        })
    }

    /// Parse the `<name> [ ( <arg> [, …] ) ]` body of an `EXECUTE`, after the `EXECUTE` keyword
    /// has been consumed (`start` spans from that keyword). Shared by the standalone
    /// [`Statement::Execute`] path and the `CREATE TABLE … AS EXECUTE` CTAS source.
    pub(super) fn parse_execute_statement_body(
        &mut self,
        start: Span,
    ) -> ParseResult<ExecuteStatement<D::Ext>> {
        let name = self.parse_ident()?;
        let args = if self.peek_is_punct(Punctuation::LParen)? {
            self.expect_punct(Punctuation::LParen, "`(` to open the EXECUTE argument list")?;
            let args = self.parse_comma_separated_exprs()?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the EXECUTE argument list",
            )?;
            args
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        Ok(ExecuteStatement {
            name,
            args,
            meta: self.make_meta(span),
        })
    }

    /// Parse a `DEALLOCATE [PREPARE] <name>` statement into [`Statement::Deallocate`] from the
    /// `DEALLOCATE` keyword. DuckDB (`prepared_statements`) makes the `PREPARE` keyword
    /// optional; MySQL (`prepared_statements_from`) requires it — see
    /// [`finish_deallocate_statement`](Self::finish_deallocate_statement). Neither dialect has
    /// a `DEALLOCATE ALL`, so a name is always required (a missing one is left for the
    /// statement loop to reject).
    pub(super) fn parse_deallocate_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DEALLOCATE")?;
        self.finish_deallocate_statement(start, DeallocateKeyword::Deallocate)
    }

    /// Parse the `PREPARE <name>` tail of MySQL's `DROP PREPARE <name>` prepared-statement
    /// release into [`Statement::Deallocate`] with the [`DeallocateKeyword::Drop`] spelling,
    /// after the leading `DROP` keyword has been consumed by the DROP dispatcher (`start`
    /// spans from that keyword). Reached only under `prepared_statements_from`, where MySQL's
    /// `deallocate_or_drop` grammar makes `DROP` a synonym for `DEALLOCATE`.
    pub(super) fn parse_drop_prepare_statement(
        &mut self,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        self.finish_deallocate_statement(start, DeallocateKeyword::Drop)
    }

    /// Finish a `{DEALLOCATE | DROP} [PREPARE] <name>` release after the leading verb is read.
    /// The `PREPARE` keyword is mandatory under MySQL's `prepared_statements_from` (a bare
    /// `DEALLOCATE name` is `ER_PARSE_ERROR` on mysql:8) and optional under DuckDB's
    /// `prepared_statements`; no shipped preset arms both, so exactly one rule applies. The
    /// `prepared_statements_from`-first order here means the both-on combination — registry-rejected
    /// as [`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`](crate::ast::dialect::GrammarConflict)
    /// — resolves this tail MySQL-first, disagreeing with the DuckDB-first dispatch of the
    /// `PREPARE`/`EXECUTE` heads. That is the incoherence the registry variant declares undefined:
    /// the both-on semantics are deliberately not reconciled here.
    fn finish_deallocate_statement(
        &mut self,
        start: Span,
        keyword: DeallocateKeyword,
    ) -> ParseResult<Statement<D::Ext>> {
        let prepare_keyword = if self.features().utility_syntax.prepared_statements_from {
            self.expect_contextual_keyword("PREPARE")?;
            true
        } else {
            self.eat_contextual_keyword("PREPARE")?
        };
        let name = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Deallocate {
            deallocate: Box::new(DeallocateStatement {
                keyword,
                prepare_keyword,
                name,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `PREPARE <name> FROM {'<text>' | @<var>}` statement into
    /// [`Statement::PrepareFrom`], reached under
    /// [`UtilitySyntax::prepared_statements_from`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The source (`sql_yacc.yy` `prepare_src`) is a string literal *or* a user-variable
    /// reference, never an arbitrary expression — `PREPARE s FROM 1+1` is `ER_PARSE_ERROR` on
    /// mysql:8, which the string-or-`@var` grammar below enforces. The source string is kept
    /// opaque (not re-parsed).
    pub(super) fn parse_prepare_from_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("PREPARE")?;
        let name = self.parse_ident()?;
        self.expect_keyword(Keyword::From)?;
        let source = self.parse_prepare_source()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::PrepareFrom {
            prepare_from: Box::new(PrepareFromStatement {
                name,
                source,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse the `prepare_src` of a MySQL `PREPARE ... FROM`: a `@variable` reference
    /// ([`PrepareSource::Variable`]) or, failing that, a `TEXT_STRING_sys` string literal
    /// ([`PrepareSource::Text`]). Anything else (a bare number, an expression, a `@@`
    /// system variable) reaches the string-literal expectation and is rejected there,
    /// matching the engine's `ER_PARSE_ERROR`.
    fn parse_prepare_source(&mut self) -> ParseResult<PrepareSource> {
        let start = self.current_span()?;
        if let Some(name) = self.parse_optional_user_variable_ref()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PrepareSource::Variable { name, meta });
        }
        let source = self.expect_string_literal(
            "a statement-source string or `@variable` after `PREPARE ... FROM`",
        )?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PrepareSource::Text { source, meta })
    }

    /// Parse a MySQL `EXECUTE <name> [USING @<var> [, ...]]` statement into
    /// [`Statement::ExecuteUsing`], reached under
    /// [`UtilitySyntax::prepared_statements_from`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The `USING` list (`sql_yacc.yy` `execute_var_list`) is a non-empty comma-separated list
    /// of user-variable references only (`execute_var_ident: '@' ident_or_text`) — never
    /// arbitrary expressions and never a parenthesized argument list, both of which MySQL
    /// `ER_PARSE_ERROR`s. A bare `EXECUTE s` (no `USING`) leaves the list empty.
    pub(super) fn parse_execute_using_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("EXECUTE")?;
        let name = self.parse_ident()?;
        let using = if self.eat_keyword(Keyword::Using)? {
            self.parse_comma_separated(Self::parse_execute_using_var)?
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::ExecuteUsing {
            execute_using: Box::new(ExecuteUsingStatement {
                name,
                using,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse one `execute_var_ident` (`'@' ident_or_text`) of an `EXECUTE ... USING` list: a
    /// required user-variable reference. A non-`@` token (or a `@@` system variable) is the
    /// syntax error MySQL reports for the list member.
    fn parse_execute_using_var(&mut self) -> ParseResult<Ident> {
        match self.parse_optional_user_variable_ref()? {
            Some(name) => Ok(name),
            None => Err(self.unexpected("a user variable reference (`@name`) in the `USING` list")),
        }
    }

    /// Parse a `CALL <name> [ ( [ <arg> [, …] ] ) ]` statement into [`Statement::Call`],
    /// reached under [`UtilitySyntax::call`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The name is a routine name capped like a relation target — MySQL/SQLite (no catalog
    /// qualifier) reject a three-part `a.b.c` name, matching MySQL's `sp_name` grammar
    /// (`ident '.' ident | ident`). The parenthesized argument list is mandatory for DuckDB
    /// (a bare `CALL pragma_version` is a syntax error) and, when present, may be empty
    /// (`CALL pragma_version()`), so an empty `)` after `(` yields empty `args`. Under
    /// [`UtilitySyntax::call_bare_name`](crate::ast::dialect::UtilitySyntax) the whole
    /// argument list is optional: a `CALL name` with no following `(` is MySQL's bare form
    /// (`opt_paren_expr_list` %empty), recorded with [`CallStatement::parenthesized`] `false`.
    pub(super) fn parse_call_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("CALL")?;
        let name = self.parse_target_relation_name()?;
        let (parenthesized, args) = if self.features().utility_syntax.call_bare_name
            && !self.peek_is_punct(Punctuation::LParen)?
        {
            // MySQL's bare `CALL name` — the optional argument list is absent entirely.
            (false, ThinVec::new())
        } else {
            self.expect_punct(Punctuation::LParen, "`(` to open the CALL argument list")?;
            let args = if self.peek_is_punct(Punctuation::RParen)? {
                ThinVec::new()
            } else {
                self.parse_comma_separated_exprs()?
            };
            self.expect_punct(Punctuation::RParen, "`)` to close the CALL argument list")?;
            (true, args)
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Call {
            call: Box::new(CallStatement {
                name,
                args,
                parenthesized,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- DO (PostgreSQL) ----------------------------------------------------

    /// Parse a `DO [LANGUAGE <lang>] '<body>'` anonymous code block into
    /// [`Statement::Do`], reached under
    /// [`UtilitySyntax::do_statement`](crate::ast::dialect::UtilitySyntax).
    ///
    /// PostgreSQL's grammar is `DO dostmt_opt_list`, a *non-empty* sequence of items each
    /// either an `Sconst` body or `LANGUAGE <name>`, in any order and with no arity limit
    /// (`makeDefElem("as"/"language", …)`). The loop collects that list verbatim: a repeated
    /// or missing body and a repeated language all parse, because — like the
    /// [`parse_prepare_statement`](Self::parse_prepare_statement) "any statement" contract —
    /// PostgreSQL defers the "exactly one body, at most one language" check to execution, so
    /// enforcing it here would over-reject inputs libpg_query accepts. The list must be
    /// non-empty (a bare `DO` is a syntax error); a trailing token that is neither a string
    /// nor `LANGUAGE` ends the list and is left for the statement terminator to reject
    /// (`DO 'x' FOO`).
    ///
    /// The code-block body is an `Sconst`, not any string constant: a bit-string
    /// (`b'…'`/`x'…'`) or national (`N'…'`) constant is a different literal type, so it ends
    /// the list rather than opening a body, matching libpg_query's reject of `DO b'0'`,
    /// `DO x'ab'`, and `DO N'x'` (see [`string_literal_is_sconst`]). The `LANGUAGE` argument
    /// is a `NonReservedWord_or_Sconst` ([`parse_do_language_name`](Self::parse_do_language_name)):
    /// a bare non-reserved word or an `Sconst` string, matching the `CREATE FUNCTION`
    /// `LANGUAGE` clause — a reserved word there is the syntax error PostgreSQL reports
    /// (`DO LANGUAGE select 'x'`).
    pub(super) fn parse_do_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DO")?;
        let mut args = ThinVec::new();
        // Peek first so end-of-input ends the list rather than erroring; the span is needed
        // only once an item is confirmed present. A token that is neither an `Sconst` body
        // nor `LANGUAGE` also ends the list (left for the statement terminator to reject).
        while let Some(token) = self.peek()? {
            let arg_start = token.span;
            if token.kind == TokenKind::String {
                // Only an `Sconst` opens a code block; a bit/hex/national constant is a
                // different literal type that PostgreSQL rejects here, so end the list.
                if !string_literal_is_sconst(self.span_text(token.span)) {
                    break;
                }
                let body = self
                    .try_parse_string_literal()?
                    .expect("peeked an `Sconst` string body");
                let meta = self.make_meta(arg_start.union(self.preceding_span()));
                args.push(DoArg::Body { body, meta });
            } else if self.peek_is_contextual_keyword("LANGUAGE")? {
                self.expect_contextual_keyword("LANGUAGE")?;
                let name = self.parse_do_language_name()?;
                let meta = self.make_meta(arg_start.union(self.preceding_span()));
                args.push(DoArg::Language { name, meta });
            } else {
                break;
            }
        }
        if args.is_empty() {
            return Err(self.unexpected("a string body or `LANGUAGE <name>` after `DO`"));
        }
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Do {
            do_block: Box::new(DoStatement {
                args,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a `DO ... LANGUAGE <name>` operand: PostgreSQL's `NonReservedWord_or_Sconst`,
    /// a bare non-reserved word or an `Sconst` string constant.
    ///
    /// The string arm admits only an `Sconst`; a bit/hex/national constant falls through to
    /// [`parse_ident`](Self::parse_ident), which rejects it (as PostgreSQL does), so the
    /// word/string choice never over-accepts a non-`Sconst` string. A reserved word is
    /// likewise the `parse_ident` reject PostgreSQL reports.
    fn parse_do_language_name(&mut self) -> ParseResult<LanguageName> {
        let start = self.current_span()?;
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::String
                && string_literal_is_sconst(self.span_text(token.span))
            {
                let value = self.expect_string_literal("a language name")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(LanguageName::String { value, meta });
            }
        }
        let word = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LanguageName::Word { word, meta })
    }

    // --- DO (MySQL) ---------------------------------------------------------

    /// Parse a MySQL `DO <expr> [, <expr> ...]` evaluate-and-discard statement into
    /// [`Statement::DoExpressions`], reached under
    /// [`UtilitySyntax::do_expression_list`](crate::ast::dialect::UtilitySyntax).
    ///
    /// A *different behaviour on the same `DO` keyword* from PostgreSQL's anonymous code block
    /// ([`parse_do_statement`](Self::parse_do_statement)): MySQL's grammar is literally
    /// `DO select_item_list` (`sql_yacc.yy` `do_stmt`), so the items are
    /// [`parse_select_item`](Self::parse_select_item)s — a select alias (`DO 1 AS x`) and a
    /// wildcard (`DO *`, `DO t.*`) grammar-parse here exactly as in a projection, keeping
    /// raw-parse acceptance aligned with the engine (the wildcard forms bind-reject, but that
    /// is a resolver verdict, not a syntax reject). The list is non-empty:
    /// [`parse_comma_separated`](Self::parse_comma_separated) requires at least one item, so a
    /// bare `DO` is the `ER_PARSE_ERROR` MySQL reports.
    pub(super) fn parse_do_expressions_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DO")?;
        let items = self.parse_comma_separated(Self::parse_select_item)?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::DoExpressions {
            do_expressions: Box::new(DoExpressionsStatement {
                items,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- LOCK / UNLOCK TABLES and LOCK / UNLOCK INSTANCE (MySQL) --------------

    /// Parse a MySQL `LOCK {TABLES | TABLE} <tbl> [[AS] <alias>] <lock-kind> [, ...]`
    /// statement into [`Statement::LockTables`], reached under
    /// [`UtilitySyntax::lock_tables`](crate::ast::dialect::UtilitySyntax) when the word after
    /// `LOCK` is `TABLES`/`TABLE` (the dispatch already verified both words).
    ///
    /// The grammar is `LOCK table_or_tables table_lock_list` (mysql `sql_yacc.yy`): the
    /// keyword spelling is preserved on the node's `plural` and the non-empty per-table list
    /// is [`parse_table_lock`](Self::parse_table_lock) items.
    pub(super) fn parse_lock_tables_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LOCK")?;
        let plural = self.parse_table_or_tables()?;
        let tables = self.parse_comma_separated(Self::parse_table_lock)?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::LockTables {
            lock_tables: Box::new(LockTablesStatement {
                plural,
                tables,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse one `table_lock` list item: `<name> [[AS] <alias>] {READ [LOCAL] | WRITE}`.
    ///
    /// The lock kind is mandatory — a bare `LOCK TABLES t1` is `ER_PARSE_ERROR` on
    /// mysql:8.4.10 — and the pre-8.0 `LOW_PRIORITY WRITE` modifier needs no special
    /// handling: `LOW_PRIORITY` is a MySQL *reserved* word, so the bare-alias position
    /// (which consults the dialect's `reserved_bare_alias` set, exactly like a FROM-clause
    /// alias) rejects it and the mandatory-kind expectation reports the error, matching the
    /// engine's 1064. `READ`/`WRITE` are likewise reserved, so a following lock kind can
    /// never be mistaken for a bare alias.
    fn parse_table_lock(&mut self) -> ParseResult<TableLock> {
        let start = self.current_span()?;
        let name = self.parse_object_name()?;
        // `opt_table_alias` is `[AS] ident`: MySQL's single reserved-word class governs the
        // alias whether or not `AS` is written, so both arms use the bare-alias ident set.
        let alias = if self.eat_keyword(Keyword::As)? || self.peek_can_start_bare_alias()? {
            Some(self.parse_bare_alias_ident()?)
        } else {
            None
        };
        let kind = if self.eat_contextual_keyword("READ")? {
            if self.eat_contextual_keyword("LOCAL")? {
                TableLockKind::ReadLocal
            } else {
                TableLockKind::Read
            }
        } else if self.eat_contextual_keyword("WRITE")? {
            TableLockKind::Write
        } else {
            return Err(self.unexpected("a table lock kind (`READ [LOCAL]` or `WRITE`)"));
        };
        let span = start.union(self.preceding_span());
        Ok(TableLock {
            name,
            alias,
            kind,
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `UNLOCK {TABLES | TABLE}` statement into [`Statement::UnlockTables`],
    /// reached under the same
    /// [`UtilitySyntax::lock_tables`](crate::ast::dialect::UtilitySyntax) gate as the
    /// acquire side. No table list — `UNLOCK` releases everything the session holds.
    pub(super) fn parse_unlock_tables_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("UNLOCK")?;
        let plural = self.parse_table_or_tables()?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::UnlockTables {
            unlock_tables: Box::new(UnlockTablesStatement {
                plural,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Consume the `table_or_tables` keyword alternative, reporting `true` for the plural
    /// `TABLES` spelling and `false` for `TABLE` (mysql `sql_yacc.yy`: the two are
    /// grammar-equal). The dispatch lookahead already saw one of them, so a miss here is an
    /// internal expectation error, not a user-reachable path.
    fn parse_table_or_tables(&mut self) -> ParseResult<bool> {
        if self.eat_contextual_keyword("TABLES")? {
            Ok(true)
        } else {
            self.expect_contextual_keyword("TABLE")?;
            Ok(false)
        }
    }

    /// Parse a MySQL `LOCK INSTANCE FOR BACKUP` statement into
    /// [`Statement::InstanceLock`] (acquire side), reached under
    /// [`UtilitySyntax::lock_instance`](crate::ast::dialect::UtilitySyntax) when the word
    /// after `LOCK` is `INSTANCE`. The `FOR BACKUP` tail is mandatory and fixed.
    pub(super) fn parse_lock_instance_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LOCK")?;
        self.expect_contextual_keyword("INSTANCE")?;
        self.expect_keyword(Keyword::For)?;
        self.expect_contextual_keyword("BACKUP")?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::InstanceLock {
            instance_lock: Box::new(InstanceLockStatement {
                acquire: true,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `UNLOCK INSTANCE` statement into [`Statement::InstanceLock`] (release
    /// side), reached under the same
    /// [`UtilitySyntax::lock_instance`](crate::ast::dialect::UtilitySyntax) gate.
    pub(super) fn parse_unlock_instance_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("UNLOCK")?;
        self.expect_contextual_keyword("INSTANCE")?;
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::InstanceLock {
            instance_lock: Box::new(InstanceLockStatement {
                acquire: false,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    // --- LOAD DATA / LOAD XML (MySQL) ----------------------------------------

    /// Parse a MySQL `LOAD {DATA | XML} … INFILE … INTO TABLE …` bulk-import statement into
    /// [`Statement::LoadData`], reached under
    /// [`UtilitySyntax::load_data`](crate::ast::dialect::UtilitySyntax) when the word after
    /// `LOAD` is `DATA`/`XML` (the dispatch already verified both words, keeping this seam MECE
    /// with the PostgreSQL/DuckDB `load_extension` `LOAD '<lib>'` reading of the same keyword).
    ///
    /// The grammar is the mysql `sql_yacc.yy` `load_stmt` rule. The whole clause train is
    /// *order-sensitive* (engine-measured on mysql:8.4.10: any out-of-order clause is
    /// `ER_PARSE_ERROR` 1064), so each optional clause is tried in the grammar's fixed order and
    /// an out-of-order clause is left unconsumed to reject at the statement boundary. Every
    /// clause parses under either format — the `FIELDS`/`LINES`-under-`XML` and
    /// `ROWS IDENTIFIED BY`-under-`DATA` restrictions are semantic, enforced by the server only
    /// after the whole statement parses and the table resolves, so the parse layer accepts them
    /// under both (the "grammar accepts, bind restricts" contract). Only the classic documented
    /// surface is covered; the MySQL 8.4 secondary-engine bulk-load extension clauses (see
    /// [`LoadDataStatement`]) are a separate feature.
    pub(super) fn parse_load_data_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LOAD")?;
        // `data_or_xml`: the single distinguishing keyword between the two readings.
        let format = if self.eat_contextual_keyword("DATA")? {
            LoadDataFormat::Data
        } else {
            self.expect_contextual_keyword("XML")?;
            LoadDataFormat::Xml
        };
        // `load_data_lock`: the default (no keyword) is a plain write lock, modelled as `None`.
        let concurrency = if self.eat_contextual_keyword("LOW_PRIORITY")? {
            Some(LoadDataConcurrency::LowPriority)
        } else if self.eat_contextual_keyword("CONCURRENT")? {
            Some(LoadDataConcurrency::Concurrent)
        } else {
            None
        };
        // `opt_local`.
        let local = self.eat_contextual_keyword("LOCAL")?;
        // `load_source_type` + `TEXT_STRING_filesystem`: only the classic `INFILE '<path>'`
        // source is modelled (the `URL`/`S3` source types are the deferred cloud-load feature).
        self.expect_contextual_keyword("INFILE")?;
        let file = self.expect_string_literal("a file-path string after `INFILE`")?;
        // `opt_duplicate`: `REPLACE`/`IGNORE` are mutually exclusive (writing both is 1064 — the
        // second keyword is left unconsumed and rejects at the `INTO` expectation below).
        let on_duplicate = if self.eat_contextual_keyword("REPLACE")? {
            Some(LoadDataDuplicate::Replace)
        } else if self.eat_contextual_keyword("IGNORE")? {
            Some(LoadDataDuplicate::Ignore)
        } else {
            None
        };
        self.expect_contextual_keyword("INTO")?;
        self.expect_contextual_keyword("TABLE")?;
        let table = self.parse_object_name()?;
        // `opt_use_partition`: `PARTITION (name [, ...])`, a non-empty name list.
        let partitions = if self.eat_contextual_keyword("PARTITION")? {
            self.expect_punct(Punctuation::LParen, "`(` to open the partition list")?;
            let names = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the partition list")?;
            names
        } else {
            ThinVec::new()
        };
        // `opt_load_data_charset`: `{CHARACTER SET | CHARSET} <charset_name>`.
        let charset = if (self.peek_is_contextual_keyword("CHARACTER")?
            && self.peek_nth_is_contextual_keyword(1, "SET")?)
            || self.peek_is_contextual_keyword("CHARSET")?
        {
            if self.eat_contextual_keyword("CHARACTER")? {
                self.expect_contextual_keyword("SET")?;
            } else {
                self.expect_contextual_keyword("CHARSET")?;
            }
            Some(self.parse_charset_name()?)
        } else {
            None
        };
        // `opt_xml_rows_identified_by`: `ROWS IDENTIFIED BY '<tag>'`. Grammar-shared by both
        // formats though only meaningful under `XML`.
        let rows_identified_by = if self.peek_is_contextual_keyword("ROWS")?
            && self.peek_nth_is_contextual_keyword(1, "IDENTIFIED")?
        {
            self.expect_contextual_keyword("ROWS")?;
            self.expect_contextual_keyword("IDENTIFIED")?;
            self.expect_contextual_keyword("BY")?;
            Some(self.expect_string_literal("a row-tag string after `ROWS IDENTIFIED BY`")?)
        } else {
            None
        };
        // `opt_field_term`: `{FIELDS | COLUMNS} <field_term>+`.
        let fields = if self.peek_is_contextual_keyword("FIELDS")?
            || self.peek_is_contextual_keyword("COLUMNS")?
        {
            Some(self.parse_load_data_fields()?)
        } else {
            None
        };
        // `opt_line_term`: `LINES <line_term>+`.
        let lines = if self.peek_is_contextual_keyword("LINES")? {
            Some(self.parse_load_data_lines()?)
        } else {
            None
        };
        // `opt_ignore_lines`: `IGNORE <n> {LINES | ROWS}`. This `IGNORE` is positionally distinct
        // from the `opt_duplicate` `IGNORE` above (which precedes `INTO`).
        let ignore_rows = if self.peek_is_contextual_keyword("IGNORE")? {
            Some(self.parse_load_data_ignore_rows()?)
        } else {
            None
        };
        // `opt_field_or_var_spec`: `(col_or_var [, ...])`, or an empty `()` (folded to absent).
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_load_data_column_list()?
        } else {
            ThinVec::new()
        };
        // `opt_load_data_set_spec`: `SET col = {expr | DEFAULT} [, ...]`, reusing the
        // single-column-assignment parser (a tuple assignment is not grammar-valid here, and the
        // fitted MySql preset's `multi_column_assignment` gate is off, so `parse_update_assignment`
        // only ever yields the `Single` form).
        let set = if self.eat_contextual_keyword("SET")? {
            self.parse_update_assignments()?
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::LoadData {
            load_data: Box::new(LoadDataStatement {
                format,
                concurrency,
                local,
                file,
                on_duplicate,
                table,
                partitions,
                charset,
                rows_identified_by,
                fields,
                lines,
                ignore_rows,
                columns,
                set,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Parse the `{FIELDS | COLUMNS} <field_term>+` clause (mysql `sql_yacc.yy` `field_term_list`).
    /// The two keywords are interchangeable synonyms; the spelling rides the node. At least one
    /// sub-clause is required (a bare `FIELDS` is 1064). Each sub-clause may appear in any order
    /// and a repeat is last-wins (mirroring `merge_field_separators`).
    fn parse_load_data_fields(&mut self) -> ParseResult<LoadDataFields> {
        let start = self.current_span()?;
        let spelling = if self.eat_contextual_keyword("FIELDS")? {
            LoadFieldsSpelling::Fields
        } else {
            self.expect_contextual_keyword("COLUMNS")?;
            LoadFieldsSpelling::Columns
        };
        let mut terminated_by = None;
        let mut enclosed_by = None;
        let mut escaped_by = None;
        let mut saw_any = false;
        loop {
            if self.eat_contextual_keyword("TERMINATED")? {
                self.expect_contextual_keyword("BY")?;
                terminated_by = Some(self.expect_string_literal("a string after `TERMINATED BY`")?);
            } else if self.peek_is_contextual_keyword("OPTIONALLY")?
                || self.peek_is_contextual_keyword("ENCLOSED")?
            {
                let enclosed_start = self.current_span()?;
                let optionally = self.eat_contextual_keyword("OPTIONALLY")?;
                self.expect_contextual_keyword("ENCLOSED")?;
                self.expect_contextual_keyword("BY")?;
                let value = self.expect_string_literal("a string after `ENCLOSED BY`")?;
                let meta = self.make_meta(enclosed_start.union(self.preceding_span()));
                enclosed_by = Some(LoadDataEnclosed {
                    optionally,
                    value,
                    meta,
                });
            } else if self.eat_contextual_keyword("ESCAPED")? {
                self.expect_contextual_keyword("BY")?;
                escaped_by = Some(self.expect_string_literal("a string after `ESCAPED BY`")?);
            } else {
                break;
            }
            saw_any = true;
        }
        if !saw_any {
            return Err(self.unexpected(
                "a `FIELDS`/`COLUMNS` sub-clause (`TERMINATED BY`, `[OPTIONALLY] ENCLOSED BY`, or `ESCAPED BY`)",
            ));
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LoadDataFields {
            spelling,
            terminated_by,
            enclosed_by,
            escaped_by,
            meta,
        })
    }

    /// Parse the `LINES <line_term>+` clause (mysql `sql_yacc.yy` `line_term_list`). At least one
    /// sub-clause is required (a bare `LINES` is 1064); each may appear in any order, a repeat is
    /// last-wins (mirroring `merge_line_separators`).
    fn parse_load_data_lines(&mut self) -> ParseResult<LoadDataLines> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LINES")?;
        let mut starting_by = None;
        let mut terminated_by = None;
        let mut saw_any = false;
        loop {
            if self.eat_contextual_keyword("STARTING")? {
                self.expect_contextual_keyword("BY")?;
                starting_by = Some(self.expect_string_literal("a string after `STARTING BY`")?);
            } else if self.eat_contextual_keyword("TERMINATED")? {
                self.expect_contextual_keyword("BY")?;
                terminated_by = Some(self.expect_string_literal("a string after `TERMINATED BY`")?);
            } else {
                break;
            }
            saw_any = true;
        }
        if !saw_any {
            return Err(self.unexpected("a `LINES` sub-clause (`STARTING BY` or `TERMINATED BY`)"));
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LoadDataLines {
            starting_by,
            terminated_by,
            meta,
        })
    }

    /// Parse the `IGNORE <n> {LINES | ROWS}` header-skip clause (mysql `sql_yacc.yy`
    /// `opt_ignore_lines`). The unit keywords are interchangeable; the spelling rides the node.
    fn parse_load_data_ignore_rows(&mut self) -> ParseResult<LoadDataIgnoreRows> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("IGNORE")?;
        let count = match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                self.advance()?;
                Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(token.span),
                }
            }
            _ => return Err(self.unexpected("a row count after `IGNORE`")),
        };
        let unit = if self.eat_contextual_keyword("LINES")? {
            LoadDataIgnoreUnit::Lines
        } else if self.eat_contextual_keyword("ROWS")? {
            LoadDataIgnoreUnit::Rows
        } else {
            return Err(self.unexpected("`LINES` or `ROWS` after the `IGNORE <n>` count"));
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LoadDataIgnoreRows { count, unit, meta })
    }

    /// Parse the `(col_or_var [, ...])` target list (mysql `sql_yacc.yy` `opt_field_or_var_spec`).
    /// A written empty `()` is grammar-valid and folds to the absent form (both yield MySQL's
    /// nullptr), so it returns an empty list rather than a distinct surface.
    fn parse_load_data_column_list(&mut self) -> ParseResult<ThinVec<LoadDataFieldOrVar>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the column list")?;
        if self.peek_is_punct(Punctuation::RParen)? {
            self.advance()?;
            return Ok(ThinVec::new());
        }
        let items = self.parse_comma_separated(Self::parse_load_data_field_or_var)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the column list")?;
        Ok(items)
    }

    /// Parse one `field_or_var`: a destination column (`simple_ident_nospvar`) or a user variable
    /// (`@name`). A `@@`-prefixed system variable is not valid here (`ER_PARSE_ERROR` on
    /// mysql:8), matching the grammar's `'@' ident_or_text`.
    fn parse_load_data_field_or_var(&mut self) -> ParseResult<LoadDataFieldOrVar> {
        let start = self.current_span()?;
        if let Some(token) = self.peek()? {
            match token.kind {
                // `@'name'` / `@"name"` / `` @`name` `` cannot fold at the lexer, so the sigil is a
                // standalone `@` followed by the quoted `ident_or_text`.
                TokenKind::Punctuation(Punctuation::At) => {
                    self.advance()?;
                    let name = self.parse_ident_or_text()?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    return Ok(LoadDataFieldOrVar::Variable { name, meta });
                }
                // `@name` folds into one `Variable` token; `@@sys` is a system variable and not
                // valid in this position.
                TokenKind::Variable => {
                    let text = self.span_text(token.span);
                    if text.starts_with("@@") {
                        return Err(self.unexpected("a column name or `@user_variable`"));
                    }
                    self.advance()?;
                    let sym = self.intern_text(&text[1..]);
                    let name = Ident {
                        sym,
                        quote: QuoteStyle::None,
                        meta: self.make_meta(token.span),
                    };
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    return Ok(LoadDataFieldOrVar::Variable { name, meta });
                }
                _ => {}
            }
        }
        let name = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LoadDataFieldOrVar::Column { name, meta })
    }

    // --- shared -------------------------------------------------------------

    /// Consume the current token as a string literal when it is one, leaving the
    /// cursor unchanged otherwise. The value rides `meta.span`, so no
    /// owned string is interned.
    fn try_parse_string_literal(&mut self) -> ParseResult<Option<Literal>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        if token.kind != TokenKind::String {
            return Ok(None);
        }
        // One string token only — no adjacent-string continuation. DO (and other
        // multi-`Sconst` positions) admit several string args separated by any
        // whitespace (`DO 'a'\t'b'`); folding them into one constant would reject
        // libpg_query-valid multi-arg forms with "expected a newline between
        // adjacent string literals". `U&'…'` still folds optional `UESCAPE` and
        // validates the escape body so invalid `U&'\d'` is rejected.
        self.advance()?;
        let text = self.span_text(token.span);
        // Newline continuation only (same-line second strings stay for multi-arg DO).
        let mut span = self.consume_string_continuations_with(
            token.span, text, /* same_line_is_error */ false,
        )?;
        let full = self.span_text(span);
        let is_unicode = matches!(full.as_bytes(), [b'U' | b'u', b'&', b'\'', ..]);
        if is_unicode {
            span = self.consume_optional_uescape(span)?;
            let u_full = self.span_text(span);
            // Continued U& spans join plain `'…'` segments; validate each segment's
            // escape body (libpg_query rejects `U&''\r'\$'` as invalid Unicode escape).
            let valid = if u_full.bytes().any(|b| matches!(b, b'\n' | b'\r')) {
                unicode_escape_segments_are_valid(u_full)
            } else {
                crate::ast::unicode_escape_string_is_valid(u_full)
            };
            if !valid {
                return Err(crate::error::ParseError::lexical(
                    span,
                    crate::tokenizer::LexErrorKind::InvalidEscapeSequence,
                ));
            }
        } else if matches!(full.as_bytes(), [b'E' | b'e', b'\'', ..]) {
            let valid = if full.bytes().any(|b| matches!(b, b'\n' | b'\r')) {
                crate::tokenizer::escape_string_segments_are_valid(full)
            } else {
                crate::ast::postgres_escape_string_is_valid(full)
            };
            if !valid {
                return Err(crate::error::ParseError::lexical(
                    span,
                    crate::tokenizer::LexErrorKind::InvalidEscapeSequence,
                ));
            }
        }
        Ok(Some(Literal {
            kind: LiteralKind::String,
            meta: self.make_meta(span),
        }))
    }

    pub(super) fn expect_string_literal(&mut self, expected: &'static str) -> ParseResult<Literal> {
        match self.try_parse_string_literal()? {
            Some(literal) => Ok(literal),
            None => Err(self.unexpected(expected)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{
        FeatureDelta, FeatureDependencyViolation, FeatureSet, GrammarConflict, MaintenanceSyntax,
        ShowSyntax, UtilitySyntax,
    };
    use crate::ast::{
        AccountName, AnalyzeHistogram, CheckTableOption, ChecksumTableOption, CopyDirection,
        CopyIntoSource, CopyIntoTarget, CopyOptionValue, CopySource, CopyTarget, DescribeColumn,
        DoArg, ExplainFormat, ExplainKeyword, ExplainOption, Expr, FlushOption, FlushStatement,
        FlushTablesLock, FlushTarget, ForceKind, KillTarget, LanguageName, LiteralKind,
        NoWriteToBinlog, PurgeStatement, PurgeTarget, RenameStatement, RepairTableOption,
        Resolver as _, SetParameterValue, ShowBare, ShowColumnsSpelling, ShowCreateKind,
        ShowDiagnosticKind, ShowEngineArtifact, ShowFilter, ShowFromKeyword, ShowFunctionsFilter,
        ShowFunctionsScope, ShowIndexSpelling, ShowLimit, ShowListing, ShowRoutineKind, ShowScope,
        ShowTarget, Statement, TableKeyword, TableMaintenanceKind, VacuumAnalyze,
    };
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};
    use crate::render::Renderer;

    /// ANSI with the `COPY` utility statement enabled, to exercise the COPY grammar:
    /// the ANSI baseline gates `COPY` off (it is PostgreSQL-specific), so the parse
    /// tests opt it on here while keeping the rest of the ANSI surface. The gate's
    /// reject path is covered by `copy_is_gated_off_under_ansi`.
    const COPY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                copy: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn parse_one(sql: &str) -> Parsed {
        parse_with(sql, crate::ParseConfig::new(COPY_DIALECT))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"))
    }

    fn copy_of(parsed: &Parsed) -> &crate::ast::CopyStatement {
        let [Statement::Copy { copy, .. }] = parsed.statements() else {
            panic!("expected one COPY statement, got {:?}", parsed.statements());
        };
        copy
    }

    fn explain_of(parsed: &Parsed) -> &crate::ast::ExplainStatement {
        let [Statement::Explain { explain, .. }] = parsed.statements() else {
            panic!(
                "expected one EXPLAIN statement, got {:?}",
                parsed.statements(),
            );
        };
        explain
    }

    /// The dispatch contract: the `COPY` and `EXPLAIN` utility keywords are
    /// routed by the central `parse_statement` to this module's two entries and yield
    /// `Statement::Copy` / `Statement::Explain` (the `*_of` helpers panic otherwise).
    #[test]
    fn dispatch_routes_utility_keywords_to_this_family() {
        let _ = copy_of(&parse_one("COPY t TO STDOUT"));
        let _ = explain_of(&parse_one("EXPLAIN SELECT 1"));
    }

    #[test]
    fn copy_to_file_captures_table_direction_and_path() {
        let parsed = parse_one("COPY t TO '/tmp/out.csv'");
        let copy = copy_of(&parsed);
        let CopySource::Table { table, columns, .. } = &copy.source else {
            panic!("expected a table COPY source, got {:?}", copy.source);
        };
        assert_eq!(parsed.resolver().resolve(table.0[0].sym), "t");
        assert!(columns.is_empty());
        assert_eq!(copy.direction, CopyDirection::To);
        assert!(matches!(copy.target, CopyTarget::File { .. }));
        assert!(copy.options.is_empty());
    }

    #[test]
    fn copy_from_stdin_with_columns_and_options() {
        let parsed = parse_one("COPY t (a, b) FROM STDIN WITH (FORMAT csv, HEADER, DELIMITER ',')");
        let copy = copy_of(&parsed);
        let CopySource::Table { columns, .. } = &copy.source else {
            panic!("expected a table COPY source, got {:?}", copy.source);
        };
        assert_eq!(columns.len(), 2);
        assert_eq!(copy.direction, CopyDirection::From);
        assert!(matches!(copy.target, CopyTarget::Stdin { .. }));
        // The parenthesized list sets the surface tag.
        assert!(copy.parenthesized);
        assert_eq!(copy.options.len(), 3);
        // `FORMAT csv` keeps its bareword value; `HEADER` is bare; `DELIMITER ','`
        // carries a string value.
        assert!(matches!(
            copy.options[0].value,
            Some(CopyOptionValue::Word { .. })
        ));
        assert!(copy.options[1].value.is_none());
        assert!(matches!(
            copy.options[2].value,
            Some(CopyOptionValue::String { .. })
        ));
    }

    #[test]
    fn copy_generic_option_value_shapes_parse_and_round_trip() {
        // The generic parenthesized option grammar (`copy_generic_opt_arg`) carries
        // value shapes beyond bareword/string: a numeric argument, the bare `*`, and a
        // parenthesized argument list — the DuckDB/Snowflake-parity file-format/option
        // surfaces. Each parses into typed data and renders back verbatim.
        let parsed = parse_one(
            "COPY t TO 'f' (FORMAT csv, HEADER 1, ROW_GROUP_SIZE -2, FORCE_QUOTE (a, b), FORCE_NULL *)",
        );
        let copy = copy_of(&parsed);
        assert!(copy.parenthesized);
        assert_eq!(copy.options.len(), 5);
        assert!(matches!(
            copy.options[0].value,
            Some(CopyOptionValue::Word { .. }) // FORMAT csv
        ));
        assert!(matches!(
            copy.options[1].value,
            Some(CopyOptionValue::Number { .. }) // HEADER 1
        ));
        // A sign-folded negative number is one numeric literal, not a unary expression.
        assert!(matches!(
            copy.options[2].value,
            Some(CopyOptionValue::Number { .. }) // ROW_GROUP_SIZE -2
        ));
        let Some(CopyOptionValue::List { values, .. }) = &copy.options[3].value else {
            panic!("expected a list value, got {:?}", copy.options[3].value);
        };
        assert_eq!(values.len(), 2); // FORCE_QUOTE (a, b)
        assert!(
            values
                .iter()
                .all(|v| matches!(v, CopyOptionValue::Word { .. }))
        );
        assert!(matches!(
            copy.options[4].value,
            Some(CopyOptionValue::Star { .. }) // FORCE_NULL *
        ));

        let rendered = Renderer::new(COPY_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("renders: {err:?}"));
        assert_eq!(
            rendered,
            "COPY t TO 'f' WITH (FORMAT csv, HEADER 1, ROW_GROUP_SIZE -2, FORCE_QUOTE (a, b), FORCE_NULL *)",
        );
    }

    #[test]
    fn copy_to_program_and_stdout_parse() {
        assert!(matches!(
            copy_of(&parse_one("COPY t TO PROGRAM 'gzip > out.gz'")).target,
            CopyTarget::Program { .. },
        ));
        assert!(matches!(
            copy_of(&parse_one("COPY t TO STDOUT")).target,
            CopyTarget::Stdout { .. },
        ));
    }

    #[test]
    fn copy_query_form_is_to_only_and_embeds_the_inner_statement() {
        let parsed = parse_one("COPY (SELECT a FROM t) TO STDOUT");
        let copy = copy_of(&parsed);
        let CopySource::Query { query, .. } = &copy.source else {
            panic!("expected a query COPY source, got {:?}", copy.source);
        };
        assert!(matches!(**query, Statement::Query { .. }));
        assert_eq!(copy.direction, CopyDirection::To);
        assert!(matches!(copy.target, CopyTarget::Stdout { .. }));
    }

    #[test]
    fn copy_query_form_admits_dml_source() {
        // PostgreSQL's query source is a `PreparableStmt`, so a data-modifying
        // statement is a valid inner source, not only a `SELECT`. (The PostgreSQL
        // `RETURNING` spelling rides the dialect preset and is exercised in the
        // conformance corpus.)
        let parsed = parse_one("COPY (INSERT INTO t VALUES (1)) TO STDOUT");
        let copy = copy_of(&parsed);
        assert!(matches!(
            &copy.source,
            CopySource::Query { query, .. } if matches!(**query, Statement::Insert { .. }),
        ));
    }

    #[test]
    fn copy_legacy_un_parenthesized_options() {
        // The legacy spelling is space-separated with neither `WITH` nor parens;
        // each fixed keyword keeps its arity, so `CSV HEADER` are two bare options
        // and `DELIMITER ','` carries the string. All ride the canonical option
        // shape, tagged as not parenthesized (ADR-0011).
        let parsed = parse_one("COPY t FROM 'f' CSV HEADER DELIMITER ','");
        let copy = copy_of(&parsed);
        assert!(!copy.parenthesized);
        assert_eq!(copy.options.len(), 3);
        assert!(copy.options[0].value.is_none()); // CSV
        assert!(copy.options[1].value.is_none()); // HEADER
        assert!(matches!(
            copy.options[2].value,
            Some(CopyOptionValue::String { .. }) // DELIMITER ','
        ));
    }

    #[test]
    fn copy_legacy_with_keyword_canonicalizes_away() {
        // `WITH` may precede the legacy list but is not load-bearing: the captured
        // option set is identical with or without it.
        let with_parsed = parse_one("COPY t TO 'f' WITH CSV");
        let without_parsed = parse_one("COPY t TO 'f' CSV");
        let with = copy_of(&with_parsed);
        let without = copy_of(&without_parsed);
        assert!(!with.parenthesized && !without.parenthesized);
        assert_eq!(with.options.len(), 1);
        assert_eq!(without.options.len(), 1);
    }

    #[test]
    fn copy_legacy_force_column_list_options() {
        // The compound-keyword `FORCE` options carry a column list or `*`. The
        // `FORCE` word is the option name; the sub-keyword and target ride the
        // `Force` value, with an empty column list standing for `*`.
        let parsed = parse_one("COPY t TO 'f' FORCE QUOTE a, b FORCE NOT NULL c FORCE NULL *");
        let copy = copy_of(&parsed);
        assert!(!copy.parenthesized);
        let kinds: Vec<_> = copy
            .options
            .iter()
            .map(|option| match &option.value {
                Some(CopyOptionValue::Force { kind, columns, .. }) => (*kind, columns.len()),
                other => panic!("expected a FORCE value, got {other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (ForceKind::Quote, 2),   // FORCE QUOTE a, b
                (ForceKind::NotNull, 1), // FORCE NOT NULL c
                (ForceKind::Null, 0),    // FORCE NULL * (empty list == star)
            ],
        );
        // Each FORCE option's name is the leading `FORCE` keyword.
        assert_eq!(parsed.resolver().resolve(copy.options[0].name.sym), "FORCE");
    }

    #[test]
    fn copy_binary_prefix_is_table_only() {
        // `COPY BINARY <table>` is the `opt_binary` prefix (distinct from the
        // `BINARY` option), and it only precedes the table source.
        let parsed = parse_one("COPY BINARY t TO STDOUT");
        let copy = copy_of(&parsed);
        assert!(copy.binary);
        assert!(matches!(copy.source, CopySource::Table { .. }));
        // A plain COPY leaves the prefix off; the `BINARY` *option* is not the prefix.
        let plain = parse_one("COPY t TO 'f' BINARY");
        assert!(!copy_of(&plain).binary);
        // The prefix cannot introduce the query source (PG has no `COPY BINARY (...)`).
        assert!(
            parse_with(
                "COPY BINARY (SELECT 1) TO STDOUT",
                crate::ParseConfig::new(COPY_DIALECT)
            )
            .is_err()
        );
    }

    #[test]
    fn copy_using_delimiters_clause() {
        // `[USING] DELIMITERS '<str>'` rides a dedicated field; the optional `USING`
        // canonicalizes away, so both spellings capture the same delimiter string.
        let using = parse_one("COPY t FROM 'f' USING DELIMITERS ','");
        let bare = parse_one("COPY t FROM 'f' DELIMITERS ','");
        assert!(copy_of(&using).delimiters.is_some());
        assert!(copy_of(&bare).delimiters.is_some());
        // The clause is distinct from the singular `DELIMITER` option and coexists
        // with the option list.
        let both = parse_one("COPY t FROM 'f' DELIMITERS ',' CSV");
        let copy = copy_of(&both);
        assert!(copy.delimiters.is_some());
        assert_eq!(copy.options.len(), 1);
    }

    #[test]
    fn copy_from_where_filter_is_from_only() {
        // The `WHERE <predicate>` filter is captured for COPY FROM.
        let parsed = parse_one("COPY t FROM STDIN WHERE a > 1");
        let copy = copy_of(&parsed);
        assert_eq!(copy.direction, CopyDirection::From);
        assert!(copy.filter.is_some());
        // A FROM without WHERE leaves it empty.
        assert!(copy_of(&parse_one("COPY t FROM STDIN")).filter.is_none());
        // COPY TO rejects the filter (PG: "WHERE clause not allowed with COPY TO"),
        // and the query source (always TO) has no WHERE either.
        assert!(
            parse_with(
                "COPY t TO STDOUT WHERE a > 1",
                crate::ParseConfig::new(COPY_DIALECT)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "COPY (SELECT 1) TO STDOUT WHERE a > 1",
                crate::ParseConfig::new(COPY_DIALECT)
            )
            .is_err()
        );
    }

    #[test]
    fn explain_bare_and_legacy_prefix() {
        // A bare EXPLAIN records no options and is not the parenthesized spelling.
        let bare = parse_one("EXPLAIN SELECT 1");
        let explain = explain_of(&bare);
        assert!(!explain.parenthesized);
        assert!(explain.options.is_empty());
        assert!(matches!(*explain.statement, Statement::Query { .. }));

        // The legacy `ANALYZE VERBOSE` prefix yields two bare options in order.
        let legacy = parse_one("EXPLAIN ANALYZE VERBOSE SELECT 1");
        let explain = explain_of(&legacy);
        assert!(!explain.parenthesized);
        assert!(matches!(
            explain.options.as_slice(),
            [
                ExplainOption::Analyze { value: None, .. },
                ExplainOption::Verbose { value: None, .. },
            ],
        ));
    }

    #[test]
    fn explain_parenthesized_options_parse() {
        let parsed = parse_one("EXPLAIN (ANALYZE, VERBOSE, FORMAT JSON) SELECT 1");
        let explain = explain_of(&parsed);
        assert!(explain.parenthesized);
        assert!(matches!(
            explain.options.as_slice(),
            [
                ExplainOption::Analyze { .. },
                ExplainOption::Verbose { .. },
                ExplainOption::Format {
                    format: ExplainFormat::Json,
                    ..
                },
            ],
        ));
    }

    #[test]
    fn explain_other_option_and_boolean_value() {
        // A non-built-in option rides `Other`; an explicit boolean argument is kept.
        let parsed = parse_one("EXPLAIN (BUFFERS, ANALYZE true) INSERT INTO t VALUES (1)");
        let explain = explain_of(&parsed);
        let [
            ExplainOption::Other {
                name, value: None, ..
            },
            ExplainOption::Analyze { value: Some(_), .. },
        ] = explain.options.as_slice()
        else {
            panic!(
                "expected BUFFERS (Other) then ANALYZE true, got {:?}",
                explain.options,
            );
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "BUFFERS");
        assert!(matches!(*explain.statement, Statement::Insert { .. }));
    }

    #[test]
    fn malformed_copy_and_explain_are_rejected() {
        for sql in [
            "COPY",                                    // missing table
            "COPY t",                                  // missing FROM/TO
            "COPY t FROM",                             // missing source
            "COPY (SELECT 1) FROM STDIN",              // the query form is TO-only
            "COPY (CREATE TABLE x (a int)) TO STDOUT", // inner must be a preparable query
            "COPY t TO 'f' FOO",                       // unknown legacy option keyword
            "COPY t TO 'f' CSV FOO",                   // trailing unknown legacy option
            "COPY t TO 'f' ENCODING AS 'UTF8'",        // ENCODING admits no `AS`
            "COPY t TO 'f' FORCE BOGUS",               // FORCE needs QUOTE/NULL/NOT NULL
            "COPY t FROM 'f' USING FOO",               // USING only introduces DELIMITERS
            "COPY t TO STDOUT WHERE a > 1",            // WHERE is COPY FROM-only
            "EXPLAIN",                                 // missing inner statement
            "EXPLAIN () SELECT 1",                     // empty option list
        ] {
            // Under `COPY_DIALECT` (COPY gate on), these reject on the grammar, not the
            // gate — the intent of this test.
            assert!(
                parse_with(sql, crate::ParseConfig::new(COPY_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn copy_is_gated_off_under_ansi() {
        // `utility_syntax.copy` gates the leading `COPY` keyword: the ANSI baseline
        // leaves it off, so a well-formed `COPY` is not dispatched and surfaces as an
        // unknown statement (an unexpected-keyword error) — the deliberate dialect
        // divergence. `EXPLAIN` is ungated, so it still parses under the same baseline;
        // and the identical `COPY` parses once the gate is on (`COPY_DIALECT`).
        assert!(
            parse_with("COPY t TO STDOUT", crate::ParseConfig::new(TestDialect)).is_err(),
            "ANSI gates COPY off, so a leading COPY is an unknown statement",
        );
        assert!(parse_with("EXPLAIN SELECT 1", crate::ParseConfig::new(TestDialect)).is_ok());
        assert!(parse_with("COPY t TO STDOUT", crate::ParseConfig::new(COPY_DIALECT)).is_ok());
    }

    // --- COPY INTO (Snowflake) ----------------------------------------------

    /// ANSI with only the `copy_into` gate on, so the Snowflake `COPY INTO` grammar is
    /// exercised in isolation (the PostgreSQL `copy` gate stays off, proving the two
    /// leading-`COPY` surfaces are independent).
    const COPY_INTO_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                copy_into: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn parse_copy_into(sql: &str) -> Parsed {
        parse_with(sql, crate::ParseConfig::new(COPY_INTO_DIALECT))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"))
    }

    fn copy_into_of(parsed: &Parsed) -> &crate::ast::CopyIntoStatement {
        let [Statement::CopyInto { copy, .. }] = parsed.statements() else {
            panic!(
                "expected one COPY INTO statement, got {:?}",
                parsed.statements(),
            );
        };
        copy
    }

    #[test]
    fn copy_into_load_with_file_format_and_options_round_trips() {
        // The load direction: a table target, an external-location source, the nested
        // `FILE_FORMAT = (...)` keyed list, and word/string/number copy options. The
        // nested list rides `CopyOptionValue::OptionList` (each element a full
        // `CopyOption`), the flat options ride the shared value shapes.
        let sql = "COPY INTO t FROM 's3://bucket/data/' FILE_FORMAT = (TYPE = CSV FIELD_DELIMITER = ',' SKIP_HEADER = 1) PATTERN = '.*[.]csv' ON_ERROR = CONTINUE SIZE_LIMIT = 100";
        let parsed = parse_copy_into(sql);
        let copy = copy_into_of(&parsed);
        let CopyIntoTarget::Table { table, columns, .. } = &copy.target else {
            panic!("expected a table COPY INTO target, got {:?}", copy.target);
        };
        assert_eq!(parsed.resolver().resolve(table.0[0].sym), "t");
        assert!(columns.is_empty());
        assert!(matches!(copy.source, CopyIntoSource::External { .. }));
        assert_eq!(copy.options.len(), 4);
        let Some(CopyOptionValue::OptionList { options, .. }) = &copy.options[0].value else {
            panic!(
                "expected a nested FILE_FORMAT option list, got {:?}",
                copy.options[0].value,
            );
        };
        assert_eq!(options.len(), 3);
        assert!(matches!(
            options[0].value,
            Some(CopyOptionValue::Word { .. }) // TYPE = CSV
        ));
        assert!(matches!(
            options[1].value,
            Some(CopyOptionValue::String { .. }) // FIELD_DELIMITER = ','
        ));
        assert!(matches!(
            options[2].value,
            Some(CopyOptionValue::Number { .. }) // SKIP_HEADER = 1
        ));
        assert!(matches!(
            copy.options[1].value,
            Some(CopyOptionValue::String { .. }) // PATTERN = '.*[.]csv'
        ));
        assert!(matches!(
            copy.options[2].value,
            Some(CopyOptionValue::Word { .. }) // ON_ERROR = CONTINUE
        ));
        assert!(matches!(
            copy.options[3].value,
            Some(CopyOptionValue::Number { .. }) // SIZE_LIMIT = 100
        ));

        let rendered = Renderer::new(COPY_INTO_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("renders: {err:?}"));
        assert_eq!(rendered, sql);
    }

    #[test]
    fn copy_into_files_list_and_validation_mode() {
        // `FILES = ('a.csv', 'b.csv')` is the generic comma-separated `List` value
        // (bare values, no `=` inside), distinct from the keyed `OptionList`.
        let parsed = parse_copy_into(
            "COPY INTO t (a, b) FROM 's3://b/' FILES = ('a.csv', 'b.csv') VALIDATION_MODE = RETURN_ERRORS",
        );
        let copy = copy_into_of(&parsed);
        let CopyIntoTarget::Table { columns, .. } = &copy.target else {
            panic!("expected a table COPY INTO target, got {:?}", copy.target);
        };
        assert_eq!(columns.len(), 2);
        let Some(CopyOptionValue::List { values, .. }) = &copy.options[0].value else {
            panic!(
                "expected a FILES list value, got {:?}",
                copy.options[0].value
            );
        };
        assert_eq!(values.len(), 2);
        assert!(
            values
                .iter()
                .all(|v| matches!(v, CopyOptionValue::String { .. }))
        );
        assert!(matches!(
            copy.options[1].value,
            Some(CopyOptionValue::Word { .. }) // VALIDATION_MODE = RETURN_ERRORS
        ));
    }

    #[test]
    fn copy_into_unload_to_external_location() {
        // The unload direction: an external-location target with a table source, and
        // the `FORMAT_NAME` spelling of `FILE_FORMAT` (a string, not a nested list).
        let sql = "COPY INTO 's3://bucket/unload/' FROM t FILE_FORMAT = (FORMAT_NAME = 'my_csv_format') OVERWRITE = TRUE";
        let parsed = parse_copy_into(sql);
        let copy = copy_into_of(&parsed);
        assert!(matches!(copy.target, CopyIntoTarget::External { .. }));
        assert!(matches!(copy.source, CopyIntoSource::Table { .. }));
        let rendered = Renderer::new(COPY_INTO_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("renders: {err:?}"));
        assert_eq!(rendered, sql);
    }

    #[test]
    fn copy_into_transformation_query_source() {
        // The load-with-transformation form: `FROM ( <query> )`; the inner is gated to
        // a query, mirroring the PostgreSQL `COPY (<query>)` source treatment.
        let parsed = parse_copy_into("COPY INTO t FROM (SELECT a FROM src) ON_ERROR = SKIP_FILE");
        let copy = copy_into_of(&parsed);
        let CopyIntoSource::Query { query, .. } = &copy.source else {
            panic!("expected a query COPY INTO source, got {:?}", copy.source);
        };
        assert!(matches!(**query, Statement::Query { .. }));
        assert!(
            parse_with(
                "COPY INTO t FROM (CREATE TABLE x (a INT))",
                crate::ParseConfig::new(COPY_INTO_DIALECT)
            )
            .is_err(),
            "the transformation source admits only a query",
        );
    }

    #[test]
    fn malformed_copy_into_is_rejected() {
        for sql in [
            "COPY INTO",                                           // missing target
            "COPY INTO t",                                         // missing FROM
            "COPY INTO t FROM",                                    // missing source
            "COPY INTO t FROM 's3://b/' FILE_FORMAT", // an option name requires `= <value>`
            "COPY INTO t FROM 's3://b/' FILE_FORMAT = (TYPE CSV)", // nested pairs require `=`
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(COPY_INTO_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn copy_into_and_copy_gates_are_independent() {
        // `copy_into` off: the `INTO` after `COPY` is not a Snowflake dispatch. Under
        // bare ANSI (both gates off) the leading `COPY` is an unknown statement; under
        // the PostgreSQL-style `copy` gate alone, `COPY INTO ...` parses as PG `COPY`
        // with `INTO` as the table name and rejects on the malformed remainder —
        // either way a reject, so the labelled coverage flip holds.
        assert!(
            parse_with(
                "COPY INTO t FROM 's3://b/'",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "COPY INTO t FROM 's3://b/'",
                crate::ParseConfig::new(COPY_DIALECT)
            )
            .is_err()
        );
        // `copy` off, `copy_into` on: the PostgreSQL transfer shape is not admitted.
        assert!(
            parse_with(
                "COPY t TO STDOUT",
                crate::ParseConfig::new(COPY_INTO_DIALECT)
            )
            .is_err()
        );
        // Both surfaces coexist when both gates are on (the Lenient union).
        const BOTH: FeatureDialect = {
            const FEATURES: FeatureSet =
                FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                    copy: true,
                    copy_into: true,
                    ..UtilitySyntax::ANSI
                }));
            FeatureDialect {
                features: &FEATURES,
            }
        };
        assert!(parse_with("COPY t TO STDOUT", crate::ParseConfig::new(BOTH)).is_ok());
        assert!(parse_with("COPY INTO t FROM 's3://b/'", crate::ParseConfig::new(BOTH)).is_ok());
    }

    // --- PRAGMA / ATTACH / DETACH -------------------------------------------

    /// ANSI with only the `pragma` gate on, so the PRAGMA grammar is exercised in
    /// isolation and the flip test proves the knob moves only its own family.
    const PRAGMA_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                pragma: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI with only the `attach` gate on — the `ATTACH`/`DETACH` pair, no PRAGMA.
    const ATTACH_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                attach: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn pragma_of(parsed: &Parsed) -> &crate::ast::PragmaStatement {
        let [Statement::Pragma { pragma, .. }] = parsed.statements() else {
            panic!(
                "expected one PRAGMA statement, got {:?}",
                parsed.statements(),
            );
        };
        pragma
    }

    #[test]
    fn pragma_parses_bare_assignment_and_call_forms_onto_one_shape() {
        // The three surface forms land on the single canonical shape: the bare read
        // has no value, and `= v` / `(v)` share the value slot under the
        // `parenthesized` tag (ADR-0011).
        let bare = parse_with(
            "PRAGMA user_version",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        let pragma = pragma_of(&bare);
        assert_eq!(
            bare.resolver().resolve(pragma.name.0[0].sym),
            "user_version"
        );
        assert!(pragma.value.is_none());

        let assign = parse_with(
            "PRAGMA foreign_keys = ON",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        let pragma = pragma_of(&assign);
        assert!(!pragma.parenthesized);
        let Some(SetParameterValue::Name { name, .. }) = &pragma.value else {
            panic!("`ON` should be a keyword-as-name value, got {pragma:?}");
        };
        assert_eq!(assign.resolver().resolve(name.sym), "ON");

        let call = parse_with(
            "PRAGMA table_info(sqlite_master)",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        let pragma = pragma_of(&call);
        assert!(pragma.parenthesized);
        assert!(matches!(pragma.value, Some(SetParameterValue::Name { .. })));

        // A signed number folds the sign into the literal (PG `NumericOnly` /
        // SQLite `signed-number`), and a string value is a plain literal.
        let signed = parse_with(
            "PRAGMA cache_size = -2000",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        let Some(SetParameterValue::Literal { literal, .. }) = &pragma_of(&signed).value else {
            panic!("`-2000` should be one literal value");
        };
        assert_eq!(literal.kind, LiteralKind::Integer);
        let string = parse_with(
            "PRAGMA QUICK_CHECK('sqlite_master')",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        let pragma = pragma_of(&string);
        assert!(pragma.parenthesized);
        assert!(matches!(
            &pragma.value,
            Some(SetParameterValue::Literal { literal, .. })
                if literal.kind == LiteralKind::String
        ));

        // The name is a qualified-name position: `PRAGMA main.user_version`.
        let qualified = parse_with(
            "PRAGMA main.user_version",
            crate::ParseConfig::new(PRAGMA_DIALECT),
        )
        .unwrap();
        assert_eq!(pragma_of(&qualified).name.0.len(), 2);
    }

    #[test]
    fn pragma_value_is_not_a_general_expression() {
        // SQLite's `pragma-value` is `signed-number | name | string-literal` only;
        // these all syntax-error in SQLite and must reject here too (with the gate
        // ON, so the reject is the grammar's, not the gate's).
        for sql in [
            "PRAGMA cache_size = 1 + 2",
            "PRAGMA quick_check(1, 2)",
            "PRAGMA foreign_keys = ON OFF",
            "PRAGMA",
            "PRAGMA user_version = ",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(PRAGMA_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn attach_and_detach_parse_with_the_database_keyword_tag() {
        // The optional `DATABASE` noise keyword is round-trip-significant, so both
        // spellings parse onto one shape with the surface tag recording it.
        let with_kw = parse_with(
            "ATTACH DATABASE ':memory:' AS aux",
            crate::ParseConfig::new(ATTACH_DIALECT),
        )
        .unwrap();
        let [Statement::Attach { attach, .. }] = with_kw.statements() else {
            panic!("expected an ATTACH statement");
        };
        assert!(attach.database_keyword);
        assert_eq!(with_kw.resolver().resolve(attach.schema.sym), "aux");

        let bare = parse_with(
            "ATTACH ':memory:' AS aux2",
            crate::ParseConfig::new(ATTACH_DIALECT),
        )
        .unwrap();
        let [Statement::Attach { attach, .. }] = bare.statements() else {
            panic!("expected an ATTACH statement");
        };
        assert!(!attach.database_keyword);

        // The database source is a full expression, not just a string literal.
        let expr = parse_with(
            "ATTACH 'a' || '.db' AS x",
            crate::ParseConfig::new(ATTACH_DIALECT),
        )
        .unwrap();
        let [Statement::Attach { attach, .. }] = expr.statements() else {
            panic!("expected an ATTACH statement");
        };
        assert!(matches!(attach.target, Expr::BinaryOp { .. }));

        let detach = parse_with(
            "DETACH DATABASE aux",
            crate::ParseConfig::new(ATTACH_DIALECT),
        )
        .unwrap();
        let [Statement::Detach { detach, .. }] = detach.statements() else {
            panic!("expected a DETACH statement");
        };
        assert!(detach.database_keyword);

        let detach_bare =
            parse_with("DETACH aux", crate::ParseConfig::new(ATTACH_DIALECT)).unwrap();
        let [Statement::Detach { detach, .. }] = detach_bare.statements() else {
            panic!("expected a DETACH statement");
        };
        assert!(!detach.database_keyword);

        // The `AS <schema>` tail is required, matching SQLite.
        assert!(parse_with("ATTACH ':memory:'", crate::ParseConfig::new(ATTACH_DIALECT)).is_err());
        assert!(parse_with("DETACH", crate::ParseConfig::new(ATTACH_DIALECT)).is_err());
    }

    #[test]
    fn pragma_and_attach_gates_flip_independently() {
        // Each knob moves only its own family: `pragma` never admits
        // `ATTACH`/`DETACH`, `attach` never admits `PRAGMA`, and the ANSI baseline
        // (both off) rejects all three as unknown statements.
        assert!(
            parse_with(
                "PRAGMA user_version",
                crate::ParseConfig::new(PRAGMA_DIALECT)
            )
            .is_ok()
        );
        assert!(
            parse_with(
                "ATTACH ':memory:' AS aux",
                crate::ParseConfig::new(PRAGMA_DIALECT)
            )
            .is_err()
        );
        assert!(parse_with("DETACH aux", crate::ParseConfig::new(PRAGMA_DIALECT)).is_err());

        assert!(
            parse_with(
                "ATTACH ':memory:' AS aux",
                crate::ParseConfig::new(ATTACH_DIALECT)
            )
            .is_ok()
        );
        assert!(parse_with("DETACH aux", crate::ParseConfig::new(ATTACH_DIALECT)).is_ok());
        assert!(
            parse_with(
                "PRAGMA user_version",
                crate::ParseConfig::new(ATTACH_DIALECT)
            )
            .is_err()
        );

        for sql in [
            "PRAGMA user_version",
            "ATTACH ':memory:' AS aux",
            "DETACH aux",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off as an unknown statement",
            );
        }
    }

    // --- EXPORT / IMPORT DATABASE (DuckDB) ----------------------------------

    /// ANSI with only the `export_import_database` gate on — the `EXPORT DATABASE` /
    /// `IMPORT DATABASE` pair in isolation, no other DuckDB surface.
    const EXPORT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                export_import_database: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn export_database_bare_and_named_forms_round_trip() {
        // Bare form: no catalogue name, path only.
        let bare = parse_with(
            "EXPORT DATABASE 'dir'",
            crate::ParseConfig::new(EXPORT_DIALECT),
        )
        .unwrap();
        let [Statement::Export { export, .. }] = bare.statements() else {
            panic!("expected an EXPORT statement, got {:?}", bare.statements());
        };
        assert!(export.database.is_none());
        assert!(export.options.is_empty());

        // Named form threads the catalogue through a required `TO` before the path.
        let named = parse_with(
            "EXPORT DATABASE mydb TO 'dir'",
            crate::ParseConfig::new(EXPORT_DIALECT),
        )
        .unwrap();
        let [Statement::Export { export, .. }] = named.statements() else {
            panic!("expected an EXPORT statement");
        };
        let database = export
            .database
            .as_ref()
            .expect("named form keeps the catalogue");
        assert_eq!(named.resolver().resolve(database.sym), "mydb");

        // The named form's `TO` is required: `EXPORT DATABASE db 'dir'` without it rejects.
        assert!(
            parse_with(
                "EXPORT DATABASE mydb 'dir'",
                crate::ParseConfig::new(EXPORT_DIALECT)
            )
            .is_err()
        );

        for sql in ["EXPORT DATABASE 'dir'", "EXPORT DATABASE mydb TO 'dir'"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(EXPORT_DIALECT)).unwrap();
            let rendered = Renderer::new(EXPORT_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn export_database_reuses_the_copy_option_axis() {
        // Parenthesized generic list (the `(FORMAT ..., opt v, ...)` spelling): reuses
        // `CopyOption`/`CopyOptionValue` verbatim, with the parenthesized surface tag.
        let parsed = parse_with(
            "EXPORT DATABASE 'dir' (FORMAT parquet, COMPRESSION 'zstd', ROW_GROUP_SIZE 100000)",
            crate::ParseConfig::new(EXPORT_DIALECT),
        )
        .unwrap();
        let [Statement::Export { export, .. }] = parsed.statements() else {
            panic!("expected an EXPORT statement");
        };
        assert!(export.parenthesized);
        assert_eq!(export.options.len(), 3);
        assert!(matches!(
            export.options[0].value,
            Some(CopyOptionValue::Word { .. }) // FORMAT parquet
        ));
        assert!(matches!(
            export.options[1].value,
            Some(CopyOptionValue::String { .. }) // COMPRESSION 'zstd'
        ));
        assert!(matches!(
            export.options[2].value,
            Some(CopyOptionValue::Number { .. }) // ROW_GROUP_SIZE 100000
        ));
        let rendered = Renderer::new(EXPORT_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("renders: {err:?}"));
        assert_eq!(
            rendered,
            "EXPORT DATABASE 'dir' (FORMAT parquet, COMPRESSION 'zstd', ROW_GROUP_SIZE 100000)",
        );

        // Unlike COPY there is no leading `WITH`: the parenthesized form is bare `(...)`.
        assert!(
            parse_with(
                "EXPORT DATABASE 'dir' WITH (FORMAT parquet)",
                crate::ParseConfig::new(EXPORT_DIALECT)
            )
            .is_err()
        );

        // The legacy un-parenthesized `copy_opt_list` branch is reachable too (DuckDB
        // accepts `HEADER`), tagged non-parenthesized.
        let legacy = parse_with(
            "EXPORT DATABASE 'dir' HEADER",
            crate::ParseConfig::new(EXPORT_DIALECT),
        )
        .unwrap();
        let [Statement::Export { export, .. }] = legacy.statements() else {
            panic!("expected an EXPORT statement");
        };
        assert!(!export.parenthesized);
        assert_eq!(export.options.len(), 1);
    }

    #[test]
    fn import_database_parses_a_bare_path_and_round_trips() {
        let parsed = parse_with(
            "IMPORT DATABASE 'dir'",
            crate::ParseConfig::new(EXPORT_DIALECT),
        )
        .unwrap();
        let [Statement::Import { .. }] = parsed.statements() else {
            panic!(
                "expected an IMPORT statement, got {:?}",
                parsed.statements()
            );
        };
        let rendered = Renderer::new(EXPORT_DIALECT)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("renders: {err:?}"));
        assert_eq!(rendered, "IMPORT DATABASE 'dir'");

        // IMPORT takes no options and no path is a parse error.
        assert!(
            parse_with(
                "IMPORT DATABASE 'dir' (FORMAT parquet)",
                crate::ParseConfig::new(EXPORT_DIALECT)
            )
            .is_err()
        );
        assert!(parse_with("IMPORT DATABASE", crate::ParseConfig::new(EXPORT_DIALECT)).is_err());
    }

    #[test]
    fn export_import_gate_is_off_under_ansi() {
        // One gate covers both halves; the ANSI baseline (gate off) rejects both leading
        // keywords as unknown statements.
        for sql in ["EXPORT DATABASE 'dir'", "IMPORT DATABASE 'dir'"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off as an unknown statement",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(EXPORT_DIALECT)).is_ok(),
                "{sql:?} parses with the gate on",
            );
        }
    }

    // --- VACUUM / REINDEX / ANALYZE -----------------------------------------

    /// ANSI with the three maintenance gates on, so the grammar is exercised and
    /// round-tripped in isolation from the rest of the SQLite surface.
    const MAINTENANCE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.maintenance_syntax(MaintenanceSyntax {
                vacuum: true,
                reindex: true,
                analyze: true,
                ..MaintenanceSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// Single-gate ANSI dialects, so the independence test proves each utility
    /// knob admits only its own statement (the `copy`/`comment_on` separate-flag
    /// precedent). One `FeatureDialect` const per flag — a value, not a type, so
    /// the gates share one engine monomorphization.
    macro_rules! one_gate_dialect {
        ($name:ident, $setter:ident, $ty:ident, $field:ident) => {
            const $name: FeatureDialect = {
                const FEATURES: FeatureSet =
                    FeatureSet::ANSI.with(FeatureDelta::EMPTY.$setter($ty {
                        $field: true,
                        ..$ty::ANSI
                    }));
                FeatureDialect {
                    features: &FEATURES,
                }
            };
        };
    }

    one_gate_dialect!(VACUUM_ONLY, maintenance_syntax, MaintenanceSyntax, vacuum);
    one_gate_dialect!(REINDEX_ONLY, maintenance_syntax, MaintenanceSyntax, reindex);
    one_gate_dialect!(ANALYZE_ONLY, maintenance_syntax, MaintenanceSyntax, analyze);

    fn vacuum_of(parsed: &Parsed) -> &crate::ast::VacuumStatement {
        let [Statement::Vacuum { vacuum, .. }] = parsed.statements() else {
            panic!(
                "expected one VACUUM statement, got {:?}",
                parsed.statements()
            );
        };
        vacuum
    }

    #[test]
    fn maintenance_statements_parse_and_round_trip() {
        // Every form is a bundled-SQLite accept (verified against `sqlite3`): bare,
        // the optional single schema name, the `VACUUM INTO <expr>` file target (a full
        // expression, not just a string), and the qualified `REINDEX`/`ANALYZE` targets.
        for sql in [
            "VACUUM",
            "VACUUM main",
            "VACUUM INTO 'file.db'",
            "VACUUM main INTO 'file.db'",
            "VACUUM INTO 'a' || '.db'",
            "REINDEX",
            "REINDEX t",
            "REINDEX main.t",
            "ANALYZE",
            "ANALYZE main",
            "ANALYZE sqlite_master",
            "ANALYZE main.t",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MAINTENANCE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MAINTENANCE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn vacuum_captures_schema_and_into_independently() {
        assert!(
            vacuum_of(&parse_with("VACUUM", crate::ParseConfig::new(MAINTENANCE_DIALECT)).unwrap())
                .schema
                .is_none()
        );
        let with_schema =
            parse_with("VACUUM main", crate::ParseConfig::new(MAINTENANCE_DIALECT)).unwrap();
        let vacuum = vacuum_of(&with_schema);
        assert_eq!(
            with_schema
                .resolver()
                .resolve(vacuum.schema.as_ref().unwrap().sym),
            "main",
        );
        assert!(vacuum.into.is_none());

        // `INTO` is a full expression target, and it is independent of the schema.
        let into_only = parse_with(
            "VACUUM INTO 'f'",
            crate::ParseConfig::new(MAINTENANCE_DIALECT),
        )
        .unwrap();
        let vacuum = vacuum_of(&into_only);
        assert!(vacuum.schema.is_none());
        assert!(matches!(vacuum.into, Some(Expr::Literal { .. })));
        let both = parse_with(
            "VACUUM main INTO 'a' || '.db'",
            crate::ParseConfig::new(MAINTENANCE_DIALECT),
        )
        .unwrap();
        let vacuum = vacuum_of(&both);
        assert!(vacuum.schema.is_some());
        assert!(matches!(vacuum.into, Some(Expr::BinaryOp { .. })));
    }

    #[test]
    fn reindex_and_analyze_capture_optional_target() {
        let bare = parse_with("REINDEX", crate::ParseConfig::new(MAINTENANCE_DIALECT)).unwrap();
        let [Statement::Reindex { reindex, .. }] = bare.statements() else {
            panic!("expected a REINDEX statement");
        };
        assert!(reindex.target.is_none(), "bare REINDEX has no target");

        let qualified = parse_with(
            "REINDEX main.t",
            crate::ParseConfig::new(MAINTENANCE_DIALECT),
        )
        .unwrap();
        let [Statement::Reindex { reindex, .. }] = qualified.statements() else {
            panic!("expected a REINDEX statement");
        };
        assert_eq!(reindex.target.as_ref().unwrap().0.len(), 2, "schema.object");

        let analyze = parse_with(
            "ANALYZE sqlite_master",
            crate::ParseConfig::new(MAINTENANCE_DIALECT),
        )
        .unwrap();
        let [Statement::Analyze { analyze, .. }] = analyze.statements() else {
            panic!("expected an ANALYZE statement");
        };
        assert_eq!(analyze.target.as_ref().unwrap().0.len(), 1);
    }

    #[test]
    fn maintenance_statements_reject_malformed() {
        // Bundled-SQLite syntax rejects; with the gates ON these reject on the grammar.
        // `VACUUM main.t` (dotted schema), `VACUUM 1 + 2` (schema is a name, not an
        // expression), `VACUUM INTO` (missing target), and a numeric REINDEX/ANALYZE
        // target all leave unparsed input the statement loop rejects.
        for sql in [
            "VACUUM main.t",
            "VACUUM 1 + 2",
            "VACUUM INTO",
            "REINDEX 1",
            "ANALYZE 1",
        ] {
            parse_with(sql, crate::ParseConfig::new(MAINTENANCE_DIALECT))
                .expect_err(&format!("{sql:?} should be rejected"));
        }
    }

    #[test]
    fn duckdb_vacuum_analyze_parse_and_round_trip() {
        // The forms DuckDB 1.5.4 admits (measured on the live `duckdb` v1.5.4 oracle): the
        // bare statements, the `VACUUM ANALYZE` option in both its bare-keyword and
        // parenthesized-list spellings, a qualified or single-quoted table name, and the
        // parenthesized column list. Each round-trips through the fitted `DuckDb` renderer.
        for sql in [
            "VACUUM",
            "VACUUM ANALYZE",
            "VACUUM (ANALYZE)",
            "VACUUM db1.integers",
            "VACUUM ANALYZE t",
            "VACUUM (ANALYZE) t",
            "VACUUM ANALYZE t (a, b)",
            "VACUUM (ANALYZE) t (a, b)",
            "VACUUM t (a)",
            "VACUUM ''",
            "VACUUM 'table name'",
            "VACUUM ANALYZE 'table name'",
            "VACUUM (ANALYZE) 'table name'",
            "ANALYZE",
            "ANALYZE t4",
            "ANALYZE main.t",
            "ANALYZE t (a, b)",
            "ANALYZE 'table name'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(DUCKDB_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn duckdb_vacuum_analyze_capture_shapes() {
        // `VACUUM ANALYZE` sets the analyze flag (keyword spelling) with no operand.
        let va = parse_with("VACUUM ANALYZE", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        let vacuum = vacuum_of(&va);
        assert_eq!(vacuum.analyze, Some(VacuumAnalyze::Keyword));
        assert!(vacuum.table.is_none() && vacuum.columns.is_none());
        // The SQLite-only slots stay empty under DuckDB.
        assert!(vacuum.schema.is_none() && vacuum.into.is_none());

        // The parenthesized option list is the distinct `Parenthesized` spelling; a
        // repeated-ANALYZE list canonicalizes to the same single flag.
        let paren = parse_with("VACUUM (ANALYZE)", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert_eq!(
            vacuum_of(&paren).analyze,
            Some(VacuumAnalyze::Parenthesized)
        );
        let repeated = parse_with(
            "VACUUM (ANALYZE, ANALYZE)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        assert_eq!(
            vacuum_of(&repeated).analyze,
            Some(VacuumAnalyze::Parenthesized)
        );
        // The paren form also carries the table + column operands.
        let paren_full = parse_with(
            "VACUUM (ANALYZE) db1.integers (a, b)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        let vacuum = vacuum_of(&paren_full);
        assert_eq!(vacuum.analyze, Some(VacuumAnalyze::Parenthesized));
        assert_eq!(vacuum.table.as_ref().unwrap().0.len(), 2, "schema.table");
        assert_eq!(vacuum.columns.as_ref().unwrap().len(), 2);

        // A qualified table plus a column list (keyword spelling).
        let full = parse_with(
            "VACUUM ANALYZE db1.integers (a, b)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        let vacuum = vacuum_of(&full);
        assert_eq!(vacuum.analyze, Some(VacuumAnalyze::Keyword));
        assert_eq!(vacuum.table.as_ref().unwrap().0.len(), 2, "schema.table");
        assert_eq!(vacuum.columns.as_ref().unwrap().len(), 2);

        // Bare `VACUUM` (no analyze) — distinct from either analyze spelling.
        let bare = parse_with("VACUUM", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert_eq!(vacuum_of(&bare).analyze, None);

        // DuckDB `ANALYZE <table> (<cols>)` — the column list rides `analyze_columns`.
        let ana = parse_with("ANALYZE t (a, b)", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        let [Statement::Analyze { analyze, .. }] = ana.statements() else {
            panic!("expected an ANALYZE statement");
        };
        assert_eq!(analyze.target.as_ref().unwrap().0.len(), 1);
        assert_eq!(analyze.columns.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn duckdb_vacuum_analyze_reject_matches_oracle() {
        // The reject direction — measured against the live `duckdb` v1.5.4 oracle, so
        // this is parser/engine parity, not just a grammar choice:
        // - `FULL`/`FREEZE`/`VERBOSE` are the vacuum options 1.5.4's transform throws
        //   `NotImplementedException` on; they are reserved keywords here, so a bare
        //   `VACUUM FULL` / `ANALYZE VERBOSE` rejects (the engine `prepare` rejects too).
        // - The parenthesized option list admits only `ANALYZE`: every other option
        //   (`FULL`/`VERBOSE`/`NOWAIT`/`SKIP_TOAST`/unknown), a boolean-argument form
        //   (`(ANALYZE true)`), a mixed list, an empty list, and an `INTO` tail after a paren
        //   prefix all reject at one of the two engine layers, so none parse here.
        // - DuckDB's `VACUUM` has no SQLite `INTO <expr>` tail (a `duckdb` *parser* error).
        // - An empty column list `()` is a parser error (libpg_query's `name_list` is
        //   non-empty).
        for sql in [
            "VACUUM FULL",
            "VACUUM FREEZE",
            "VACUUM VERBOSE",
            "ANALYZE VERBOSE",
            "VACUUM INTO 'f'",
            "VACUUM main INTO 'f'",
            "VACUUM t ()",
            "ANALYZE t ()",
            "VACUUM (FULL)",
            "VACUUM (VERBOSE)",
            "VACUUM (NOWAIT)",
            "VACUUM (SKIP_TOAST)",
            "VACUUM (disable_page_skipping)",
            "VACUUM (a)",
            "VACUUM (ANALYZE true)",
            "VACUUM (ANALYZE, VERBOSE)",
            "VACUUM ()",
            "VACUUM (ANALYZE) INTO 'f'",
            "VACUUM (ANALYZE) t (a) INTO 'f'",
        ] {
            parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .expect_err(&format!("{sql:?} should be rejected"));
        }
    }

    #[test]
    fn maintenance_gates_flip_independently() {
        // Each of the three flags admits only its own leading keyword — the proof they
        // are separate flags (the `copy`/`comment_on` precedent), not one shared gate.
        assert!(parse_with("VACUUM", crate::ParseConfig::new(VACUUM_ONLY)).is_ok());
        assert!(parse_with("REINDEX", crate::ParseConfig::new(VACUUM_ONLY)).is_err());
        assert!(parse_with("ANALYZE", crate::ParseConfig::new(VACUUM_ONLY)).is_err());
        assert!(parse_with("REINDEX", crate::ParseConfig::new(REINDEX_ONLY)).is_ok());
        assert!(parse_with("VACUUM", crate::ParseConfig::new(REINDEX_ONLY)).is_err());
        assert!(parse_with("ANALYZE", crate::ParseConfig::new(ANALYZE_ONLY)).is_ok());
        assert!(parse_with("VACUUM", crate::ParseConfig::new(ANALYZE_ONLY)).is_err());

        // The ANSI baseline (all three off) rejects all three as unknown statements.
        for sql in ["VACUUM", "REINDEX", "ANALYZE"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off as an unknown statement",
            );
        }
    }

    #[test]
    fn lenient_vacuum_union_is_exact_not_cross_product() {
        use crate::dialect::Lenient;

        // Regression proof for the union: every form a fitted preset accepts still
        // parses and round-trips with both gates on.
        for sql in [
            // SQLite forms (the `vacuum` gate).
            "VACUUM",
            "VACUUM main",
            "VACUUM INTO 'file.db'",
            "VACUUM main INTO 'file.db'",
            "VACUUM INTO 'a' || '.db'",
            // DuckDB forms (the `vacuum_analyze` gate).
            "VACUUM ANALYZE",
            "VACUUM (ANALYZE)",
            "VACUUM db1.integers",
            "VACUUM ANALYZE t",
            "VACUUM (ANALYZE) t",
            "VACUUM ANALYZE t (a, b)",
            "VACUUM (ANALYZE) t (a, b)",
            "VACUUM t (a)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }

        // The cross-dialect hybrids are accepted by NEITHER engine (measured: SQLite
        // 3.x rejects the ANALYZE-then-operand / column-list / dotted prefixes at the
        // grammar; DuckDB 1.5.4 rejects every `INTO` tail with a parser error), so the
        // union rejects them too — the `INTO` tail is admitted only on a SQLite-shaped
        // prefix.
        for sql in [
            "VACUUM ANALYZE t INTO 'f'",
            "VACUUM t (a) INTO 'f'",
            "VACUUM ANALYZE t (a) INTO 'f'",
            "VACUUM (ANALYZE) t (a) INTO 'f'",
            "VACUUM db.t INTO 'f'",
        ] {
            parse_with(sql, crate::ParseConfig::new(Lenient))
                .expect_err(&format!("{sql:?} should be rejected"));
        }

        // `VACUUM ANALYZE INTO 'f'` is different: SQLite's grammar accepts it (`nm` =
        // `ANALYZE`, a fallback keyword there; the engine reject is a prepare-time
        // "unknown database", and the fitted `Sqlite` preset accepts it), but `ANALYZE`
        // is reserved in Lenient's ANSI reserved model (conflict-resolution rule 5), so
        // it cannot be read as the schema name here — a lex-class exclusion, recovered
        // by quoting, not a grammar hybrid.
        parse_with("VACUUM ANALYZE INTO 'f'", crate::ParseConfig::new(Lenient))
            .expect_err("ANALYZE is reserved under Lenient");
        let quoted = parse_with(
            "VACUUM \"ANALYZE\" INTO 'f'",
            crate::ParseConfig::new(Lenient),
        )
        .unwrap();
        let vacuum = vacuum_of(&quoted);
        assert!(vacuum.schema.is_some() && vacuum.into.is_some() && vacuum.analyze.is_none());

        // A taken `INTO` selects the SQLite grammar: its single-part name populates the
        // SQLite `schema` slot, never the DuckDB `table` slot, so at most one dialect's
        // fields are populated (the `VacuumStatement` node invariant) under the union too.
        let parsed = parse_with("VACUUM main INTO 'f'", crate::ParseConfig::new(Lenient)).unwrap();
        let vacuum = vacuum_of(&parsed);
        assert_eq!(
            parsed
                .resolver()
                .resolve(vacuum.schema.as_ref().unwrap().sym),
            "main",
        );
        assert!(vacuum.table.is_none() && vacuum.columns.is_none() && vacuum.analyze.is_none());
        assert!(vacuum.into.is_some());
        // Without `INTO` the name stays the DuckDB table operand (the more general,
        // qualified reading).
        let bare_name = parse_with("VACUUM main", crate::ParseConfig::new(Lenient)).unwrap();
        let vacuum = vacuum_of(&bare_name);
        assert!(vacuum.schema.is_none());
        assert_eq!(vacuum.table.as_ref().unwrap().0.len(), 1);
    }

    #[test]
    fn vacuum_union_with_unreserved_analyze_reads_sqlite_grammar() {
        // Both `VACUUM` gates on over the SQLite lex model, where `ANALYZE` is NOT a
        // reserved column name — the composition that exercises the parser's
        // ANALYZE-then-INTO lookahead: `VACUUM ANALYZE INTO 'f'` belongs to SQLite's
        // grammar alone (`nm` = `ANALYZE`; DuckDB rejects every `INTO` tail), so the
        // `ANALYZE` must be read as the schema name, not the DuckDB option.
        const BOTH_SQLITE_LEX: FeatureDialect = {
            const FEATURES: FeatureSet = FeatureSet::SQLITE.with(
                FeatureDelta::EMPTY.maintenance_syntax(MaintenanceSyntax {
                    vacuum_analyze: true,
                    ..MaintenanceSyntax::SQLITE
                }),
            );
            FeatureDialect {
                features: &FEATURES,
            }
        };

        let parsed = parse_with(
            "VACUUM ANALYZE INTO 'f'",
            crate::ParseConfig::new(BOTH_SQLITE_LEX),
        )
        .unwrap();
        let vacuum = vacuum_of(&parsed);
        assert_eq!(
            parsed
                .resolver()
                .resolve(vacuum.schema.as_ref().unwrap().sym),
            "ANALYZE",
        );
        assert!(vacuum.into.is_some() && vacuum.analyze.is_none() && vacuum.table.is_none());
        // Without a following `INTO`, the same word is the DuckDB option.
        let option =
            parse_with("VACUUM ANALYZE", crate::ParseConfig::new(BOTH_SQLITE_LEX)).unwrap();
        let vacuum = vacuum_of(&option);
        assert!(
            vacuum.analyze == Some(VacuumAnalyze::Keyword)
                && vacuum.schema.is_none()
                && vacuum.table.is_none()
        );
    }

    // --- MySQL admin-table maintenance & RENAME -----------------------------

    /// The MySQL preset, which carries `table_maintenance` (the five admin-table verbs),
    /// `rename_statement` (standalone RENAME), and the `user_variables` lexer gate that
    /// folds `@host` into one token — the full surface the maintenance/rename grammar needs.
    const MYSQL_MAINT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet = FeatureSet::MYSQL;
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn table_maintenance_of(parsed: &Parsed) -> &crate::ast::TableMaintenanceStatement {
        let [
            Statement::TableMaintenance {
                table_maintenance, ..
            },
        ] = parsed.statements()
        else {
            panic!(
                "expected one table-maintenance statement, got {:?}",
                parsed.statements()
            );
        };
        table_maintenance
    }

    fn rename_of(parsed: &Parsed) -> &RenameStatement {
        let [Statement::Rename { rename, .. }] = parsed.statements() else {
            panic!(
                "expected one RENAME statement, got {:?}",
                parsed.statements()
            );
        };
        rename
    }

    #[test]
    fn table_maintenance_parses_and_round_trips() {
        // Every form is engine-recognized on MySQL 8.4.10 (PREPARE-only probe). Covers each
        // verb, the `NO_WRITE_TO_BINLOG | LOCAL` prefix on the verbs that admit it, the
        // `TABLE`/`TABLES` synonym, multi-table lists, the CHECK/REPAIR repeatable option
        // lists, the single CHECKSUM mode, and the ANALYZE histogram tails.
        for sql in [
            "ANALYZE TABLE t1",
            "ANALYZE TABLE t1, s.t2",
            "ANALYZE NO_WRITE_TO_BINLOG TABLE t1",
            "ANALYZE LOCAL TABLE t1",
            "ANALYZE TABLES t1",
            "ANALYZE TABLE t1 UPDATE HISTOGRAM ON c1, c2",
            "ANALYZE TABLE t1 UPDATE HISTOGRAM ON c1 WITH 16 BUCKETS",
            "ANALYZE TABLE t1 DROP HISTOGRAM ON c1, c2",
            "CHECK TABLE t1",
            "CHECK TABLE t1, t2 FOR UPGRADE QUICK",
            "CHECK TABLE t1 FAST MEDIUM EXTENDED CHANGED",
            "CHECKSUM TABLE t1",
            "CHECKSUM TABLE t1 QUICK",
            "CHECKSUM TABLE t1, t2 EXTENDED",
            "OPTIMIZE TABLE t1",
            "OPTIMIZE LOCAL TABLE t1, t2",
            "REPAIR TABLE t1",
            "REPAIR NO_WRITE_TO_BINLOG TABLE t1 QUICK EXTENDED USE_FRM",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_MAINT_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_MAINT_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn table_maintenance_captures_verb_payloads() {
        // The `NO_WRITE_TO_BINLOG`/`LOCAL` prefix spelling round-trips distinctly.
        let parsed = parse_with(
            "ANALYZE LOCAL TABLE t1",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        assert!(matches!(
            stmt.kind,
            TableMaintenanceKind::Analyze {
                no_write_to_binlog: Some(NoWriteToBinlog::Local),
                histogram: None,
                ..
            }
        ));
        assert_eq!(stmt.tables.len(), 1);
        assert_eq!(stmt.table_keyword, TableKeyword::Table);

        // The `TABLES` synonym is captured, and a multi-table list is preserved.
        let parsed = parse_with(
            "OPTIMIZE TABLES a, b, c",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        assert_eq!(stmt.table_keyword, TableKeyword::Tables);
        assert_eq!(stmt.tables.len(), 3);

        // The CHECK option list preserves order and repeats.
        let parsed = parse_with(
            "CHECK TABLE t QUICK QUICK FAST",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        let TableMaintenanceKind::Check { options, .. } = &stmt.kind else {
            panic!("expected CHECK");
        };
        assert_eq!(
            options.as_slice(),
            [
                CheckTableOption::Quick,
                CheckTableOption::Quick,
                CheckTableOption::Fast
            ]
        );

        // The CHECKSUM mode is a single optional token.
        let parsed = parse_with(
            "CHECKSUM TABLE t EXTENDED",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        assert!(matches!(
            stmt.kind,
            TableMaintenanceKind::Checksum {
                option: Some(ChecksumTableOption::Extended),
                ..
            }
        ));

        // The REPAIR option list combines all three flags in order.
        let parsed = parse_with(
            "REPAIR TABLE t QUICK EXTENDED USE_FRM",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        let TableMaintenanceKind::Repair { options, .. } = &stmt.kind else {
            panic!("expected REPAIR");
        };
        assert_eq!(
            options.as_slice(),
            [
                RepairTableOption::Quick,
                RepairTableOption::Extended,
                RepairTableOption::UseFrm
            ]
        );

        // The ANALYZE histogram bucket count is captured.
        let parsed = parse_with(
            "ANALYZE TABLE t UPDATE HISTOGRAM ON c WITH 32 BUCKETS",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let stmt = table_maintenance_of(&parsed);
        let TableMaintenanceKind::Analyze {
            histogram:
                Some(AnalyzeHistogram::Update {
                    columns, buckets, ..
                }),
            ..
        } = &stmt.kind
        else {
            panic!("expected ANALYZE … UPDATE HISTOGRAM");
        };
        assert_eq!(columns.len(), 1);
        assert!(buckets.is_some());
    }

    #[test]
    fn analyze_table_seam_leaves_bare_analyze_to_the_sibling_gate() {
        // MySQL's `ANALYZE` always takes `{TABLE | TABLES}` (optionally after the prefix), so
        // the maintenance dispatch claims only those forms. A bare `ANALYZE` under the MySQL
        // preset (which has no SQLite/DuckDB `analyze` gate) is *not* claimed — it rejects as
        // an unknown statement, keeping the keyword free for the bare-ANALYZE sibling.
        assert!(
            parse_with(
                "ANALYZE TABLE t",
                crate::ParseConfig::new(MYSQL_MAINT_DIALECT)
            )
            .is_ok()
        );
        assert!(
            parse_with("ANALYZE t", crate::ParseConfig::new(MYSQL_MAINT_DIALECT)).is_err(),
            "a bare `ANALYZE <name>` is not the MySQL maintenance form",
        );

        // The SQLite/DuckDB bare-`analyze` gate keeps admitting a bare `ANALYZE` on its own,
        // untouched by the MySQL maintenance lookahead (which insists on `{TABLE | TABLES}`).
        assert!(parse_with("ANALYZE", crate::ParseConfig::new(ANALYZE_ONLY)).is_ok());
    }

    #[test]
    fn rename_parses_and_round_trips() {
        for sql in [
            "RENAME TABLE a TO b",
            "RENAME TABLE a TO b, c TO d",
            "RENAME TABLE s.a TO s.b",
            "RENAME USER u TO v",
            "RENAME USER u@localhost TO v@localhost",
            "RENAME USER u@localhost TO v@localhost, a TO b",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_MAINT_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_MAINT_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn rename_captures_table_and_user_forms() {
        let parsed = parse_with(
            "RENAME TABLE a TO b, c TO d",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let RenameStatement::Table { renames, .. } = rename_of(&parsed) else {
            panic!("expected RENAME TABLE");
        };
        assert_eq!(renames.len(), 2);

        let parsed = parse_with(
            "RENAME USER u@localhost TO v@localhost",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let RenameStatement::User { renames, .. } = rename_of(&parsed) else {
            panic!("expected RENAME USER");
        };
        assert_eq!(renames.len(), 1);
        let AccountName::Account { host, .. } = &renames[0].from else {
            panic!("expected a named account");
        };
        assert!(host.is_some(), "the account host is captured");
    }

    #[test]
    fn table_maintenance_and_rename_reject_malformed() {
        for sql in [
            "ANALYZE TABLE",                      // no table list
            "CHECK TABLE t BOGUS",                // unknown check option leaves trailing input
            "CHECKSUM TABLE t QUICK EXTENDED",    // CHECKSUM mode is single, not a list
            "OPTIMIZE t1",                        // missing the TABLE keyword
            "REPAIR TABLE t USE_FRM QUICK EXTRA", // trailing junk
            "RENAME TABLE a b",                   // missing TO
            "RENAME USER u v",                    // missing TO
            "RENAME a TO b",                      // missing TABLE/USER
        ] {
            parse_with(sql, crate::ParseConfig::new(MYSQL_MAINT_DIALECT))
                .expect_err(&format!("{sql:?} should be rejected"));
        }
    }

    #[test]
    fn table_maintenance_and_rename_gated_off_under_ansi() {
        for sql in [
            "ANALYZE TABLE t",
            "CHECK TABLE t",
            "CHECKSUM TABLE t",
            "OPTIMIZE TABLE t",
            "REPAIR TABLE t",
            "RENAME TABLE a TO b",
            "RENAME USER u TO v",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off as an unknown statement",
            );
        }
    }

    // --- FLUSH / PURGE (MySQL) ----------------------------------------------

    fn flush_of(parsed: &Parsed) -> &FlushStatement {
        let [Statement::Flush { flush, .. }] = parsed.statements() else {
            panic!(
                "expected one FLUSH statement, got {:?}",
                parsed.statements()
            );
        };
        flush
    }

    fn purge_of(parsed: &Parsed) -> &PurgeStatement {
        let [Statement::Purge { purge, .. }] = parsed.statements() else {
            panic!(
                "expected one PURGE statement, got {:?}",
                parsed.statements()
            );
        };
        purge
    }

    #[test]
    fn flush_and_purge_parse_and_round_trip() {
        // Every form is engine-recognized on MySQL 8.4.10: FLUSH prepares (accept), PURGE
        // grammar-accepts (ER_UNSUPPORTED_PS 1295), a `FLUSH TABLES <list>` form binding-errors
        // (ER_NO_DB_ERROR 1046) but parses. Covers the `NO_WRITE_TO_BINLOG | LOCAL` prefix, the
        // `TABLE`/`TABLES` synonym + list + `WITH READ LOCK`/`FOR EXPORT` lock, every keyword
        // target, comma lists, the `RELAY LOGS FOR CHANNEL` qualifier, and both PURGE targets.
        for sql in [
            "FLUSH PRIVILEGES",
            "FLUSH NO_WRITE_TO_BINLOG PRIVILEGES",
            "FLUSH LOCAL STATUS",
            "FLUSH LOGS",
            "FLUSH BINARY LOGS",
            "FLUSH ENGINE LOGS",
            "FLUSH ERROR LOGS",
            "FLUSH GENERAL LOGS",
            "FLUSH SLOW LOGS",
            "FLUSH RELAY LOGS",
            "FLUSH RELAY LOGS FOR CHANNEL 'c1'",
            "FLUSH USER_RESOURCES",
            "FLUSH OPTIMIZER_COSTS",
            "FLUSH LOGS, STATUS",
            "FLUSH PRIVILEGES, LOGS, STATUS",
            "FLUSH BINARY LOGS, ENGINE LOGS",
            "FLUSH TABLE",
            "FLUSH TABLES",
            "FLUSH TABLES t1",
            "FLUSH TABLES t1, t2",
            "FLUSH TABLES WITH READ LOCK",
            "FLUSH TABLES t1 WITH READ LOCK",
            "FLUSH TABLES t1 FOR EXPORT",
            "FLUSH TABLE t1, s.t2 WITH READ LOCK",
            "PURGE BINARY LOGS TO 'log.000001'",
            "PURGE BINARY LOGS BEFORE '2000-01-01 00:00:00'",
            "PURGE BINARY LOGS BEFORE NOW()",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_MAINT_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_MAINT_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn flush_captures_prefix_target_and_lock() {
        // The `LOCAL` prefix spelling is captured, and the keyword-target list is preserved.
        let parsed = parse_with(
            "FLUSH LOCAL PRIVILEGES, STATUS",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let flush = flush_of(&parsed);
        assert_eq!(flush.no_write_to_binlog, Some(NoWriteToBinlog::Local));
        let FlushTarget::Options { options, .. } = &flush.target else {
            panic!("expected the keyword-target list form");
        };
        assert!(matches!(
            options.as_slice(),
            [FlushOption::Privileges { .. }, FlushOption::Status { .. }]
        ));

        // The `RELAY LOGS FOR CHANNEL` qualifier is captured.
        let parsed = parse_with(
            "FLUSH RELAY LOGS FOR CHANNEL 'c1'",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let FlushTarget::Options { options, .. } = &flush_of(&parsed).target else {
            panic!("expected the keyword-target list form");
        };
        assert!(matches!(
            options.as_slice(),
            [FlushOption::RelayLogs {
                channel: Some(_),
                ..
            }]
        ));

        // The `TABLES` synonym, a multi-table list, and the `WITH READ LOCK` lock are all held.
        let parsed = parse_with(
            "FLUSH TABLES t1, t2 WITH READ LOCK",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        let FlushTarget::Tables {
            table_keyword,
            tables,
            lock,
            ..
        } = &flush_of(&parsed).target
        else {
            panic!("expected the TABLES form");
        };
        assert_eq!(*table_keyword, TableKeyword::Tables);
        assert_eq!(tables.len(), 2);
        assert_eq!(*lock, Some(FlushTablesLock::WithReadLock));
    }

    #[test]
    fn purge_captures_target_forms() {
        let parsed = parse_with(
            "PURGE BINARY LOGS TO 'log.1'",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        assert!(matches!(purge_of(&parsed).target, PurgeTarget::To { .. }));

        let parsed = parse_with(
            "PURGE BINARY LOGS BEFORE NOW()",
            crate::ParseConfig::new(MYSQL_MAINT_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            purge_of(&parsed).target,
            PurgeTarget::Before { .. }
        ));
    }

    #[test]
    fn flush_and_purge_reject_malformed() {
        for sql in [
            "FLUSH HOSTS",              // removed 5.x target
            "FLUSH RESOURCES",          // only USER_RESOURCES is accepted
            "FLUSH QUERY CACHE",        // removed 5.x target
            "FLUSH TABLES FOR EXPORT",  // FOR EXPORT requires a table list
            "FLUSH TABLES, LOGS",       // TABLES never joins the keyword list
            "FLUSH LOGS, TABLES",       // TABLES never joins the keyword list
            "FLUSH",                    // no target
            "PURGE MASTER LOGS TO 'x'", // 8.4 dropped the MASTER synonym
            "PURGE BINARY LOGS",        // a TO/BEFORE target is required
            "PURGE LOGS BEFORE NOW()",  // the BINARY keyword is mandatory
        ] {
            parse_with(sql, crate::ParseConfig::new(MYSQL_MAINT_DIALECT))
                .expect_err(&format!("{sql:?} should be rejected"));
        }
    }

    #[test]
    fn flush_and_purge_gated_off_under_ansi() {
        for sql in [
            "FLUSH PRIVILEGES",
            "FLUSH TABLES",
            "PURGE BINARY LOGS TO 'x'",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off as an unknown statement",
            );
        }
    }

    // --- KILL / DESCRIBE (MySQL) --------------------------------------------

    /// ANSI with the `kill` gate on, to exercise the KILL grammar and round-trip it in
    /// isolation from the rest of the MySQL surface. The gate's reject path is covered by
    /// `kill_is_gated_off_under_ansi`.
    const KILL_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                kill: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI with the `describe` gate on: the `DESCRIBE`/`DESC` EXPLAIN synonyms and the
    /// `{DESCRIBE | DESC | EXPLAIN} <table>` table-metadata overload, exercised in
    /// isolation.
    const DESCRIBE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.show_syntax(ShowSyntax {
                describe: true,
                ..ShowSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    one_gate_dialect!(KILL_ONLY, utility_syntax, UtilitySyntax, kill);
    one_gate_dialect!(DESCRIBE_ONLY, show_syntax, ShowSyntax, describe);

    fn kill_of(parsed: &Parsed) -> &crate::ast::KillStatement {
        let [Statement::Kill { kill, .. }] = parsed.statements() else {
            panic!("expected one KILL statement, got {:?}", parsed.statements());
        };
        kill
    }

    fn describe_table_of(parsed: &Parsed) -> &crate::ast::DescribeStatement {
        let [Statement::Describe { describe, .. }] = parsed.statements() else {
            panic!(
                "expected one DESCRIBE (table) statement, got {:?}",
                parsed.statements(),
            );
        };
        describe
    }

    #[test]
    fn kill_captures_scope_keyword_and_expression_id() {
        // Engine-verified against live mysql:8: bare `KILL <id>` writes no scope keyword
        // (MySQL defaults it to CONNECTION), and `CONNECTION`/`QUERY` ride the target tag.
        assert_eq!(
            kill_of(&parse_with("KILL 5", crate::ParseConfig::new(KILL_DIALECT)).unwrap()).target,
            KillTarget::Unspecified,
        );
        assert_eq!(
            kill_of(
                &parse_with("KILL CONNECTION 5", crate::ParseConfig::new(KILL_DIALECT)).unwrap()
            )
            .target,
            KillTarget::Connection,
        );
        assert_eq!(
            kill_of(&parse_with("KILL QUERY 5", crate::ParseConfig::new(KILL_DIALECT)).unwrap())
                .target,
            KillTarget::Query,
        );
        // The id is a full expression: a string, and an arithmetic expression, both prepare
        // on mysql:8 (`KILL '123'` / `KILL 1 + 1`).
        assert!(matches!(
            kill_of(&parse_with("KILL '123'", crate::ParseConfig::new(KILL_DIALECT)).unwrap()).id,
            Expr::Literal { .. },
        ));
        assert!(matches!(
            kill_of(&parse_with("KILL 1 + 1", crate::ParseConfig::new(KILL_DIALECT)).unwrap()).id,
            Expr::BinaryOp { .. },
        ));
    }

    #[test]
    fn kill_round_trips_each_spelling() {
        for sql in ["KILL 5", "KILL CONNECTION 5", "KILL QUERY 5", "KILL '123'"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(KILL_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(KILL_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn kill_parses_the_mysql_variable_and_corpus_forms() {
        // The exact sqlglot-corpus KILL gaps plus the `@id` user-variable id, all
        // engine-verified to prepare on mysql:8, parse under the fitted `MySql` preset.
        use crate::dialect::MySql;
        for sql in [
            "KILL '123'",
            "KILL CONNECTION 123",
            "KILL QUERY '123'",
            "KILL @id",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySql should parse {sql:?}",
            );
        }
    }

    #[test]
    fn kill_rejects_malformed() {
        // Engine-verified rejects on mysql:8: a missing id (bare or after the scope
        // keyword), trailing input, and stacking both scope keywords.
        for sql in [
            "KILL",
            "KILL CONNECTION",
            "KILL QUERY",
            "KILL 5 6",
            "KILL CONNECTION QUERY 5",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(KILL_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn describe_and_desc_are_explain_synonyms_with_a_spelling_tag() {
        // MySQL accepts `DESCRIBE`/`DESC` as EXPLAIN synonyms for a query (engine-verified);
        // the spelling tag records which keyword so it round-trips.
        let describe = parse_with(
            "DESCRIBE SELECT 1",
            crate::ParseConfig::new(DESCRIBE_DIALECT),
        )
        .unwrap();
        assert_eq!(explain_of(&describe).spelling, ExplainKeyword::Describe);
        assert!(matches!(
            *explain_of(&describe).statement,
            Statement::Query { .. },
        ));
        assert_eq!(
            explain_of(
                &parse_with("DESC SELECT 1", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .spelling,
            ExplainKeyword::Desc,
        );
        // Plain `EXPLAIN` keeps the `Explain` spelling.
        assert_eq!(
            explain_of(
                &parse_with(
                    "EXPLAIN SELECT 1",
                    crate::ParseConfig::new(DESCRIBE_DIALECT)
                )
                .unwrap()
            )
            .spelling,
            ExplainKeyword::Explain,
        );
        // The legacy `ANALYZE` option is still an option, not a table (`DESCRIBE ANALYZE
        // SELECT 1` is the query form, engine-verified).
        assert_eq!(
            explain_of(
                &parse_with(
                    "DESCRIBE ANALYZE SELECT 1",
                    crate::ParseConfig::new(DESCRIBE_DIALECT)
                )
                .unwrap()
            )
            .spelling,
            ExplainKeyword::Describe,
        );
    }

    #[test]
    fn describe_table_form_captures_table_and_optional_column() {
        // A table name (not an explainable statement) after the keyword is the
        // table-metadata form; all three keyword spellings reach it (engine-verified).
        let bare = parse_with("DESCRIBE t", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap();
        let describe = describe_table_of(&bare);
        assert_eq!(describe.keyword, ExplainKeyword::Describe);
        assert_eq!(bare.resolver().resolve(describe.table.0[0].sym), "t");
        assert!(describe.column.is_none());
        assert_eq!(
            describe_table_of(
                &parse_with("EXPLAIN t", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .keyword,
            ExplainKeyword::Explain,
        );
        assert_eq!(
            describe_table_of(
                &parse_with("DESC t", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .keyword,
            ExplainKeyword::Desc,
        );
        // The optional trailing argument is a single column name or a wildcard pattern.
        assert!(matches!(
            describe_table_of(
                &parse_with("DESCRIBE t col", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .column,
            Some(DescribeColumn::Name { .. }),
        ));
        assert!(matches!(
            describe_table_of(
                &parse_with("DESCRIBE t 'a%'", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .column,
            Some(DescribeColumn::Wild { .. }),
        ));
        // The table is a possibly-qualified name (`DESCRIBE db.t`).
        assert_eq!(
            describe_table_of(
                &parse_with("DESCRIBE db.t", crate::ParseConfig::new(DESCRIBE_DIALECT)).unwrap()
            )
            .table
            .0
            .len(),
            2,
        );
    }

    #[test]
    fn describe_query_and_table_forms_round_trip() {
        for sql in [
            "EXPLAIN SELECT 1",
            "DESCRIBE SELECT 1",
            "DESC SELECT 1",
            "DESCRIBE t",
            "DESC t",
            "EXPLAIN t",
            "DESCRIBE t col",
            "DESCRIBE t 'a%'",
            "DESCRIBE db.t",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DESCRIBE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(DESCRIBE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn describe_table_form_rejects_malformed() {
        // Engine-verified rejects on mysql:8: a dotted column, `*`, and a second trailing
        // argument (the trailing tokens are left for the statement loop to reject).
        for sql in ["DESCRIBE t a.b", "DESCRIBE t *", "DESCRIBE t a b"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DESCRIBE_DIALECT)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn describe_gate_does_not_disturb_order_by_desc() {
        // The ticket's load-bearing check: a leading `DESC` dispatched as a statement must
        // not perturb the `ORDER BY … DESC` sort direction (a non-leading `DESC` consumed by
        // the order-by grammar). Green with the `describe` gate on.
        assert!(
            parse_with(
                "SELECT 1 ORDER BY 1 DESC",
                crate::ParseConfig::new(DESCRIBE_DIALECT)
            )
            .is_ok()
        );
        assert!(
            parse_with(
                "SELECT a FROM t ORDER BY a DESC, b ASC",
                crate::ParseConfig::new(DESCRIBE_DIALECT)
            )
            .is_ok()
        );
    }

    #[test]
    fn kill_and_describe_gates_flip_independently_and_ansi_rejects_both() {
        // Each knob moves only its own family, and the ANSI baseline (both off) rejects all
        // of them as unknown statements; `EXPLAIN` stays ungated (its query-plan form
        // parses everywhere), but a table after `EXPLAIN` needs the `describe` gate — with
        // it off, `EXPLAIN t` rejects, as PostgreSQL does.
        assert!(parse_with("KILL 5", crate::ParseConfig::new(KILL_ONLY)).is_ok());
        assert!(parse_with("DESCRIBE t", crate::ParseConfig::new(KILL_ONLY)).is_err());
        assert!(parse_with("DESCRIBE t", crate::ParseConfig::new(DESCRIBE_ONLY)).is_ok());
        assert!(parse_with("KILL 5", crate::ParseConfig::new(DESCRIBE_ONLY)).is_err());

        for sql in [
            "KILL 5",
            "DESCRIBE t",
            "DESC t",
            "DESCRIBE SELECT 1",
            "EXPLAIN t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "ANSI gates {sql:?} off",
            );
        }
        assert!(parse_with("EXPLAIN SELECT 1", crate::ParseConfig::new(TestDialect)).is_ok());
    }

    // --- SHOW TABLES (MySQL / DuckDB) ---------------------------------------

    one_gate_dialect!(SHOW_TABLES_DIALECT, show_syntax, ShowSyntax, show_tables);

    fn show_tables_of(parsed: &Parsed) -> (bool, bool, bool) {
        let [Statement::Show { show, .. }] = parsed.statements() else {
            panic!("expected one SHOW statement, got {:?}", parsed.statements());
        };
        let ShowTarget::Tables {
            extended,
            full,
            all,
            ..
        } = &show.target
        else {
            panic!("expected a SHOW TABLES target, got {:?}", show.target);
        };
        (*extended, *full, *all)
    }

    #[test]
    fn show_tables_parses_modifiers_from_and_filter() {
        // Bare form (DuckDB + MySQL): no modifiers, no qualifier, no filter.
        assert_eq!(
            show_tables_of(
                &parse_with("SHOW TABLES", crate::ParseConfig::new(SHOW_TABLES_DIALECT)).unwrap()
            ),
            (false, false, false),
        );
        // DuckDB `SHOW ALL TABLES` (engine-verified); MySQL `EXTENDED`/`FULL` (doc-cited).
        assert_eq!(
            show_tables_of(
                &parse_with(
                    "SHOW ALL TABLES",
                    crate::ParseConfig::new(SHOW_TABLES_DIALECT)
                )
                .unwrap()
            ),
            (false, false, true),
        );
        assert_eq!(
            show_tables_of(
                &parse_with(
                    "SHOW FULL TABLES",
                    crate::ParseConfig::new(SHOW_TABLES_DIALECT)
                )
                .unwrap()
            ),
            (false, true, false),
        );
        assert_eq!(
            show_tables_of(
                &parse_with(
                    "SHOW EXTENDED FULL TABLES",
                    crate::ParseConfig::new(SHOW_TABLES_DIALECT)
                )
                .unwrap()
            ),
            (true, true, false),
        );
        // `{FROM | IN} <db>` qualifier (DuckDB accepts only `FROM`; MySQL both).
        let parsed = parse_with(
            "SHOW TABLES FROM main",
            crate::ParseConfig::new(SHOW_TABLES_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Tables { from, filter, .. } = &show.target else {
            unreachable!()
        };
        let from = from.as_ref().expect("FROM qualifier");
        assert_eq!(from.keyword, ShowFromKeyword::From);
        assert_eq!(parsed.resolver().resolve(from.name.0[0].sym), "main");
        assert!(filter.is_none());
        // `IN` synonym.
        let parsed = parse_with(
            "SHOW TABLES IN db",
            crate::ParseConfig::new(SHOW_TABLES_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Tables { from, .. } = &show.target else {
            unreachable!()
        };
        assert_eq!(from.as_ref().unwrap().keyword, ShowFromKeyword::In);
        // `LIKE '<pat>'` / `WHERE <expr>` filters (MySQL; mutually exclusive).
        let parsed = parse_with(
            "SHOW TABLES LIKE 'a%'",
            crate::ParseConfig::new(SHOW_TABLES_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Tables { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(filter, Some(ShowFilter::Like { .. })));
        let parsed = parse_with(
            "SHOW TABLES WHERE x = 1",
            crate::ParseConfig::new(SHOW_TABLES_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Tables { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(filter, Some(ShowFilter::Where { .. })));
    }

    #[test]
    fn show_tables_round_trips() {
        for sql in [
            "SHOW TABLES",
            "SHOW ALL TABLES",
            "SHOW FULL TABLES",
            "SHOW EXTENDED FULL TABLES",
            "SHOW TABLES FROM main",
            "SHOW TABLES IN db",
            "SHOW TABLES LIKE 'a%'",
            "SHOW TABLES WHERE x = 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_TABLES_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_TABLES_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_tables_is_mece_with_generic_session_show() {
        // The typed statement claims only `SHOW [mods] TABLES`; every other `SHOW <var>` —
        // including a bare `SHOW ALL` (list all settings) with no trailing `TABLES` — stays a
        // generic session `SHOW`, so the two seams do not overlap.
        for sql in ["SHOW search_path", "SHOW ALL", "SHOW tables_something"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(SHOW_TABLES_DIALECT))
                        .unwrap()
                        .statements(),
                    [Statement::Session { .. }],
                ),
                "{sql:?} must stay a generic session SHOW",
            );
        }
        // With `show_tables` off (plain ANSI, session_statements still on), `SHOW TABLES`
        // falls back to the generic session `SHOW` — the parse PostgreSQL already accepts.
        assert!(matches!(
            parse_with("SHOW TABLES", crate::ParseConfig::new(TestDialect))
                .unwrap()
                .statements(),
            [Statement::Session { .. }],
        ));
    }

    #[test]
    fn show_tables_parses_under_real_duckdb_and_mysql_presets_and_sqlite_rejects() {
        use crate::dialect::{DuckDb, MySql, Sqlite};
        // DuckDB (engine-verified 1.5.4): bare / ALL / FROM.
        for sql in ["SHOW TABLES", "SHOW ALL TABLES", "SHOW TABLES FROM main"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(DuckDb))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "DuckDb should parse {sql:?} as a typed SHOW",
            );
        }
        // MySQL (doc-cited): the FULL/FROM/LIKE/WHERE surface.
        for sql in [
            "SHOW TABLES",
            "SHOW FULL TABLES",
            "SHOW TABLES FROM db",
            "SHOW TABLES LIKE 'a%'",
            "SHOW TABLES WHERE 1 = 1",
        ] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(MySql))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "MySql should parse {sql:?} as a typed SHOW",
            );
        }
        // SQLite has neither session statements nor typed SHOW: `SHOW TABLES` is rejected.
        assert!(parse_with("SHOW TABLES", crate::ParseConfig::new(Sqlite)).is_err());
    }

    // --- SHOW COLUMNS (MySQL) -----------------------------------------------

    one_gate_dialect!(SHOW_COLUMNS_DIALECT, show_syntax, ShowSyntax, show_columns);

    #[test]
    fn show_columns_parses_modifiers_spelling_from_from_and_filter() {
        // Bare form: `COLUMNS` spelling, mandatory table qualifier, no modifiers/db/filter.
        let parsed = parse_with(
            "SHOW COLUMNS FROM t",
            crate::ParseConfig::new(SHOW_COLUMNS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            panic!("expected one SHOW statement, got {:?}", parsed.statements());
        };
        let ShowTarget::Columns {
            extended,
            full,
            spelling,
            table,
            database,
            filter,
            ..
        } = &show.target
        else {
            panic!("expected a SHOW COLUMNS target, got {:?}", show.target);
        };
        assert_eq!((*extended, *full), (false, false));
        assert_eq!(*spelling, ShowColumnsSpelling::Columns);
        assert_eq!(table.keyword, ShowFromKeyword::From);
        assert_eq!(parsed.resolver().resolve(table.name.0[0].sym), "t");
        assert!(database.is_none());
        assert!(filter.is_none());

        // `FIELDS` synonym is captured on the spelling tag.
        let parsed = parse_with(
            "SHOW FIELDS FROM t",
            crate::ParseConfig::new(SHOW_COLUMNS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Columns { spelling, .. } = &show.target else {
            unreachable!()
        };
        assert_eq!(*spelling, ShowColumnsSpelling::Fields);

        // Modifiers `EXTENDED FULL` plus the doubled `{FROM|IN}` (table then db) and the
        // `IN` synonym on the table qualifier.
        let parsed = parse_with(
            "SHOW EXTENDED FULL COLUMNS IN t IN db",
            crate::ParseConfig::new(SHOW_COLUMNS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Columns {
            extended,
            full,
            table,
            database,
            ..
        } = &show.target
        else {
            unreachable!()
        };
        assert_eq!((*extended, *full), (true, true));
        assert_eq!(table.keyword, ShowFromKeyword::In);
        assert_eq!(parsed.resolver().resolve(table.name.0[0].sym), "t");
        let database = database.as_ref().expect("second FROM/IN db qualifier");
        assert_eq!(database.keyword, ShowFromKeyword::In);
        assert_eq!(parsed.resolver().resolve(database.name.0[0].sym), "db");

        // `LIKE '<pat>'` / `WHERE <expr>` filters (mutually exclusive).
        let parsed = parse_with(
            "SHOW COLUMNS FROM t LIKE 'a%'",
            crate::ParseConfig::new(SHOW_COLUMNS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Columns { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(filter, Some(ShowFilter::Like { .. })));
        let parsed = parse_with(
            "SHOW COLUMNS FROM t WHERE x = 1",
            crate::ParseConfig::new(SHOW_COLUMNS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Columns { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(filter, Some(ShowFilter::Where { .. })));
    }

    #[test]
    fn show_columns_round_trips() {
        for sql in [
            "SHOW COLUMNS FROM t",
            "SHOW FIELDS FROM t",
            "SHOW FULL COLUMNS FROM t",
            "SHOW EXTENDED FULL COLUMNS FROM t",
            "SHOW COLUMNS IN t",
            "SHOW COLUMNS FROM t FROM db",
            "SHOW COLUMNS IN t IN db",
            "SHOW COLUMNS FROM t LIKE 'a%'",
            "SHOW FIELDS FROM t WHERE x = 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_COLUMNS_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_COLUMNS_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_columns_is_mece_with_generic_session_show() {
        // The typed statement claims only `SHOW [mods] {COLUMNS|FIELDS}`; every other
        // `SHOW <var>` stays a generic session `SHOW`, so the two seams do not overlap.
        for sql in ["SHOW search_path", "SHOW ALL", "SHOW columns_something"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(SHOW_COLUMNS_DIALECT))
                        .unwrap()
                        .statements(),
                    [Statement::Session { .. }],
                ),
                "{sql:?} must stay a generic session SHOW",
            );
        }
        // With `show_columns` off (plain ANSI, session_statements on), the mandatory table
        // qualifier means `SHOW COLUMNS FROM t` cannot parse as a generic session `SHOW` —
        // the trailing `FROM t` is leftover — so it is a genuine parse error (accept flip).
        assert!(parse_with("SHOW COLUMNS FROM t", crate::ParseConfig::new(TestDialect)).is_err());
    }

    #[test]
    fn show_columns_parses_under_mysql_preset_and_duckdb_sqlite_reject() {
        use crate::dialect::{DuckDb, MySql, Sqlite};
        // MySQL (doc-cited): the EXTENDED/FULL/FIELDS/doubled-FROM/LIKE/WHERE surface.
        for sql in [
            "SHOW COLUMNS FROM t",
            "SHOW FIELDS FROM t",
            "SHOW FULL COLUMNS FROM t",
            "SHOW EXTENDED FULL COLUMNS FROM t FROM db",
            "SHOW COLUMNS FROM t LIKE 'a%'",
            "SHOW COLUMNS FROM t WHERE 1 = 1",
        ] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(MySql))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "MySql should parse {sql:?} as a typed SHOW",
            );
        }
        // DuckDB has no `SHOW COLUMNS` grammar (engine-probed reject on 1.5.4), so
        // `show_columns` is off: even the `COLUMNS` keyword is just a generic session `SHOW`
        // variable, never the typed node. (A bare `SHOW COLUMNS` avoids DuckDB's `from_first`,
        // which would otherwise read a trailing `FROM t` as a separate standalone query — an
        // orthogonal leniency, not the typed SHOW COLUMNS.)
        assert!(matches!(
            parse_with("SHOW COLUMNS", crate::ParseConfig::new(DuckDb))
                .unwrap()
                .statements(),
            [Statement::Session { .. }],
        ));
        // SQLite has neither session statements nor typed SHOW: `SHOW COLUMNS …` is rejected.
        assert!(parse_with("SHOW COLUMNS FROM t", crate::ParseConfig::new(Sqlite)).is_err());
    }

    // --- SHOW CREATE TABLE (MySQL) ------------------------------------------

    one_gate_dialect!(
        SHOW_CREATE_TABLE_DIALECT,
        show_syntax,
        ShowSyntax,
        show_create_table
    );

    #[test]
    fn show_create_table_parses_name() {
        // Bare table name.
        let parsed = parse_with(
            "SHOW CREATE TABLE t",
            crate::ParseConfig::new(SHOW_CREATE_TABLE_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            panic!("expected one SHOW statement, got {:?}", parsed.statements());
        };
        let ShowTarget::Create {
            kind: ShowCreateKind::Table,
            name,
            ..
        } = &show.target
        else {
            panic!("expected a SHOW CREATE TABLE target, got {:?}", show.target);
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "t");

        // Schema-qualified table name round-trips through both idents.
        let parsed = parse_with(
            "SHOW CREATE TABLE db.t",
            crate::ParseConfig::new(SHOW_CREATE_TABLE_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Create {
            kind: ShowCreateKind::Table,
            name,
            ..
        } = &show.target
        else {
            unreachable!()
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "db");
        assert_eq!(parsed.resolver().resolve(name.0[1].sym), "t");
    }

    #[test]
    fn show_create_table_round_trips() {
        for sql in ["SHOW CREATE TABLE t", "SHOW CREATE TABLE db.t"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_CREATE_TABLE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_CREATE_TABLE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_create_table_is_mece_with_generic_session_show() {
        // Ordinary generic session `SHOW <var>` forms are untouched by the typed dispatch.
        for sql in ["SHOW search_path", "SHOW ALL"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(SHOW_CREATE_TABLE_DIALECT))
                        .unwrap()
                        .statements(),
                    [Statement::Session { .. }],
                ),
                "{sql:?} must stay a generic session SHOW",
            );
        }
        // The typed statement claims only `SHOW CREATE TABLE`: the two-keyword lookahead
        // requires `TABLE` to follow `CREATE`, so a bare `SHOW CREATE` (no `TABLE`) is *not*
        // claimed and falls through unchanged. `CREATE` is a reserved keyword, so a generic
        // session `SHOW CREATE` cannot name it as a variable — it is a parse error both with
        // the flag on and with it off (matching PostgreSQL, where `SHOW create` likewise
        // cannot read the reserved word as a session variable). The flag does not disturb
        // that behaviour: the seam only steals the full `CREATE TABLE` two-keyword prefix.
        assert!(
            parse_with(
                "SHOW CREATE",
                crate::ParseConfig::new(SHOW_CREATE_TABLE_DIALECT)
            )
            .is_err()
        );
        assert!(parse_with("SHOW CREATE", crate::ParseConfig::new(TestDialect)).is_err());
        // With `show_create_table` off (plain ANSI, session_statements on), the fixed
        // `CREATE TABLE` keywords likewise cannot parse as a generic session `SHOW`, so
        // `SHOW CREATE TABLE t` is a genuine parse error (accept flip against the flag).
        assert!(parse_with("SHOW CREATE TABLE t", crate::ParseConfig::new(TestDialect)).is_err());
    }

    #[test]
    fn show_create_table_parses_under_mysql_preset_and_duckdb_sqlite_reject() {
        use crate::dialect::{DuckDb, MySql, Sqlite};
        // MySQL (doc-cited): bare and schema-qualified target tables.
        for sql in ["SHOW CREATE TABLE t", "SHOW CREATE TABLE db.t"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(MySql))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "MySql should parse {sql:?} as a typed SHOW",
            );
        }
        // DuckDB has no `SHOW CREATE TABLE` grammar, so `show_create_table` is off: the typed
        // dispatch never fires, and the generic session `SHOW` cannot read the reserved
        // `CREATE` keyword as a variable, so `SHOW CREATE TABLE t` is rejected — never the
        // typed node.
        assert!(parse_with("SHOW CREATE TABLE t", crate::ParseConfig::new(DuckDb)).is_err());
        // SQLite has neither session statements nor typed SHOW: `SHOW CREATE TABLE t` is
        // rejected.
        assert!(parse_with("SHOW CREATE TABLE t", crate::ParseConfig::new(Sqlite)).is_err());
    }

    // --- SHOW FUNCTIONS (Spark / Databricks) --------------------------------

    one_gate_dialect!(
        SHOW_FUNCTIONS_DIALECT,
        show_syntax,
        ShowSyntax,
        show_functions
    );

    #[test]
    fn show_functions_parses_forms() {
        // Bare listing: no scope, no schema, no filter.
        let parsed = parse_with(
            "SHOW FUNCTIONS",
            crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            panic!("expected one SHOW statement, got {:?}", parsed.statements());
        };
        let ShowTarget::Functions {
            kind, from, filter, ..
        } = &show.target
        else {
            panic!("expected a SHOW FUNCTIONS target, got {:?}", show.target);
        };
        assert!(kind.is_none() && from.is_none() && filter.is_none());

        // The optional scope keyword before `FUNCTIONS`.
        for (sql, want) in [
            ("SHOW USER FUNCTIONS", ShowFunctionsScope::User),
            ("SHOW SYSTEM FUNCTIONS", ShowFunctionsScope::System),
            ("SHOW ALL FUNCTIONS", ShowFunctionsScope::All),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT)).unwrap();
            let [Statement::Show { show, .. }] = parsed.statements() else {
                unreachable!()
            };
            let ShowTarget::Functions { kind, .. } = &show.target else {
                unreachable!()
            };
            assert_eq!(*kind, Some(want), "{sql:?}");
        }

        // The `{FROM | IN} <schema>` qualifier.
        let parsed = parse_with(
            "SHOW FUNCTIONS IN db",
            crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Functions { from, .. } = &show.target else {
            unreachable!()
        };
        let from = from.as_ref().expect("a FROM/IN schema qualifier");
        assert_eq!(from.keyword, ShowFromKeyword::In);
        assert_eq!(parsed.resolver().resolve(from.name.0[0].sym), "db");

        // `LIKE '<regex>'` — a quoted regex pattern.
        let parsed = parse_with(
            "SHOW FUNCTIONS LIKE 't*'",
            crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Functions { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(
            filter,
            Some(ShowFunctionsFilter::Regex { like: true, .. })
        ));

        // A bare (optionally qualified) function name without `LIKE`.
        let parsed = parse_with(
            "SHOW FUNCTIONS myfunc",
            crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Functions { filter, .. } = &show.target else {
            unreachable!()
        };
        let Some(ShowFunctionsFilter::Name { like, name, .. }) = filter else {
            panic!("expected a bare-name filter, got {filter:?}");
        };
        assert!(!like);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "myfunc");

        // The fully-decorated form: scope + schema + `LIKE` name.
        let parsed = parse_with(
            "SHOW SYSTEM FUNCTIONS FROM db LIKE myfunc",
            crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::Functions {
            kind, from, filter, ..
        } = &show.target
        else {
            unreachable!()
        };
        assert_eq!(*kind, Some(ShowFunctionsScope::System));
        assert!(from.is_some());
        assert!(matches!(
            filter,
            Some(ShowFunctionsFilter::Name { like: true, .. })
        ));
    }

    #[test]
    fn show_functions_round_trips() {
        for sql in [
            "SHOW FUNCTIONS",
            "SHOW USER FUNCTIONS",
            "SHOW SYSTEM FUNCTIONS",
            "SHOW ALL FUNCTIONS",
            "SHOW FUNCTIONS IN db",
            "SHOW FUNCTIONS FROM db",
            "SHOW FUNCTIONS myfunc",
            "SHOW FUNCTIONS LIKE 't*'",
            "SHOW FUNCTIONS LIKE myfunc",
            "SHOW SYSTEM FUNCTIONS FROM db LIKE 'max'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_FUNCTIONS_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_functions_is_mece_with_generic_session_show() {
        // Generic session `SHOW <var>` — including `SHOW ALL` with no `FUNCTIONS` — is
        // untouched by the typed dispatch: the lookahead insists on the `FUNCTIONS` keyword,
        // so a bare `SHOW ALL` (the `ALL` scope with no `FUNCTIONS`) falls through unclaimed.
        for sql in ["SHOW search_path", "SHOW ALL"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(SHOW_FUNCTIONS_DIALECT))
                        .unwrap()
                        .statements(),
                    [Statement::Session { .. }],
                ),
                "{sql:?} must stay a generic session SHOW",
            );
        }
        // With `show_functions` off (plain ANSI, session_statements on), `SHOW FUNCTIONS`
        // reads `FUNCTIONS` as the session variable name, so the trailing `LIKE 't*'` is
        // leftover and the statement is a genuine parse error (accept flip against the flag).
        assert!(
            parse_with(
                "SHOW FUNCTIONS LIKE 't*'",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err()
        );
    }

    #[test]
    fn show_functions_parses_under_databricks_preset_and_mysql_duckdb_reject() {
        use crate::dialect::{Databricks, DuckDb, MySql};
        // Databricks (doc-cited): the first typed-`SHOW` gate on under this preset.
        for sql in [
            "SHOW FUNCTIONS",
            "SHOW SYSTEM FUNCTIONS FROM db LIKE 'max'",
            "SHOW FUNCTIONS myfunc",
        ] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(Databricks))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "Databricks should parse {sql:?} as a typed SHOW",
            );
        }
        // MySQL has no bare `SHOW FUNCTIONS` listing — its `SHOW FUNCTION STATUS` is a
        // *different* routine-catalogue statement — so `show_functions` is off: the decorated
        // form cannot parse as a generic session `SHOW` (the trailing `LIKE 'x'` is leftover).
        assert!(parse_with("SHOW FUNCTIONS LIKE 'x'", crate::ParseConfig::new(MySql)).is_err());
        // DuckDB has no `SHOW FUNCTIONS` grammar either (`SHOW <name>` is a DESCRIBE alias):
        // the decorated form is rejected, never the typed node.
        assert!(parse_with("SHOW FUNCTIONS LIKE 'x'", crate::ParseConfig::new(DuckDb)).is_err());
    }

    // --- SHOW {FUNCTION | PROCEDURE} STATUS (MySQL) -------------------------

    one_gate_dialect!(
        SHOW_ROUTINE_STATUS_DIALECT,
        show_syntax,
        ShowSyntax,
        show_routine_status
    );

    #[test]
    fn show_routine_status_parses_kind_and_filter() {
        // Both object keywords, bare (no filter).
        for (sql, want) in [
            ("SHOW FUNCTION STATUS", ShowRoutineKind::Function),
            ("SHOW PROCEDURE STATUS", ShowRoutineKind::Procedure),
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT)).unwrap();
            let [Statement::Show { show, .. }] = parsed.statements() else {
                panic!("expected one SHOW statement, got {:?}", parsed.statements());
            };
            let ShowTarget::RoutineStatus { kind, filter, .. } = &show.target else {
                panic!(
                    "expected a SHOW ROUTINE STATUS target, got {:?}",
                    show.target
                );
            };
            assert_eq!(*kind, want, "{sql:?}");
            assert!(filter.is_none(), "{sql:?} has no filter");
        }

        // The shared `LIKE '<pat>'` filter (reused from `SHOW TABLES`/`SHOW COLUMNS`).
        let parsed = parse_with(
            "SHOW FUNCTION STATUS LIKE 'a%'",
            crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::RoutineStatus { filter, .. } = &show.target else {
            unreachable!()
        };
        assert!(matches!(filter, Some(ShowFilter::Like { .. })));

        // The shared `WHERE <expr>` predicate filter.
        let parsed = parse_with(
            "SHOW PROCEDURE STATUS WHERE Db = 'x'",
            crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT),
        )
        .unwrap();
        let [Statement::Show { show, .. }] = parsed.statements() else {
            unreachable!()
        };
        let ShowTarget::RoutineStatus { kind, filter, .. } = &show.target else {
            unreachable!()
        };
        assert_eq!(*kind, ShowRoutineKind::Procedure);
        assert!(matches!(filter, Some(ShowFilter::Where { .. })));
    }

    #[test]
    fn show_routine_status_rejects_from_qualifier() {
        // Engine-probed on mysql:8: `SHOW FUNCTION STATUS FROM db` is `ER_PARSE_ERROR` — the
        // subform has no `{FROM | IN}` qualifier, unlike `SHOW TABLES`/`SHOW COLUMNS`. The
        // trailing `FROM db` is left unconsumed, so the statement is a parse error.
        assert!(
            parse_with(
                "SHOW FUNCTION STATUS FROM db",
                crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT)
            )
            .is_err()
        );
    }

    #[test]
    fn show_routine_status_round_trips() {
        for sql in [
            "SHOW FUNCTION STATUS",
            "SHOW PROCEDURE STATUS",
            "SHOW FUNCTION STATUS LIKE 'a%'",
            "SHOW PROCEDURE STATUS LIKE 'a%'",
            "SHOW FUNCTION STATUS WHERE Db = 'x'",
            "SHOW PROCEDURE STATUS WHERE Db = 'x'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_ROUTINE_STATUS_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_routine_status_is_mece_with_generic_session_show() {
        // Generic session `SHOW <var>` is untouched: the two-keyword lookahead insists on the
        // object keyword *and* the trailing `STATUS`, so an ordinary `SHOW <var>` (including
        // `SHOW status`, whose `status` is a non-reserved word) falls through unclaimed to the
        // session statement.
        for sql in ["SHOW search_path", "SHOW status", "SHOW ALL"] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(SHOW_ROUTINE_STATUS_DIALECT))
                        .unwrap()
                        .statements(),
                    [Statement::Session { .. }],
                ),
                "{sql:?} must stay a generic session SHOW",
            );
        }
        // With `show_routine_status` off (plain ANSI, session_statements on), `FUNCTION` is a
        // reserved keyword, so `SHOW FUNCTION STATUS` cannot parse as a generic session `SHOW
        // <var>` (like `SHOW CREATE`) — a genuine parse error, so the flag is genuinely
        // required (accept flip against the flag).
        assert!(parse_with("SHOW FUNCTION STATUS", crate::ParseConfig::new(TestDialect)).is_err());
    }

    #[test]
    fn show_routine_status_parses_under_mysql_preset_and_databricks_rejects() {
        use crate::dialect::{Databricks, MySql};
        // MySQL (engine-probed on mysql:8): both object kinds with every filter form.
        for sql in [
            "SHOW FUNCTION STATUS",
            "SHOW PROCEDURE STATUS",
            "SHOW FUNCTION STATUS LIKE 'a%'",
            "SHOW PROCEDURE STATUS WHERE Db = 'x'",
        ] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(MySql))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "MySql should parse {sql:?} as a typed SHOW",
            );
        }
        // Databricks has the sibling `SHOW FUNCTIONS` gate on but *not* `show_routine_status`
        // (engine-probed: MySQL rejects `SHOW FUNCTIONS`, and the singular `FUNCTION STATUS`
        // is MySQL-only), so `SHOW FUNCTION STATUS` cannot reach the typed node there — the
        // reserved `FUNCTION` keyword cannot be read as a session variable, so it is rejected.
        assert!(parse_with("SHOW FUNCTION STATUS", crate::ParseConfig::new(Databricks)).is_err());
    }

    // --- SHOW server-administration / catalogue family (MySQL) --------------

    one_gate_dialect!(SHOW_ADMIN_DIALECT, show_syntax, ShowSyntax, show_admin);

    fn show_admin_target(parsed: &Parsed) -> &ShowTarget {
        let [Statement::Show { show, .. }] = parsed.statements() else {
            panic!("expected one SHOW statement, got {:?}", parsed.statements());
        };
        &show.target
    }

    #[test]
    fn show_admin_parses_each_target_shape() {
        // Listing family: the DATA sub-command axis + shared FROM / [LIKE|WHERE] tail.
        let parsed = parse_with(
            "SHOW DATABASES",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Listing {
                kind: ShowListing::Databases { schemas: false },
                from: None,
                filter: None,
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW SCHEMAS LIKE 'a%'",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Listing {
                kind: ShowListing::Databases { schemas: true },
                filter: Some(ShowFilter::Like { .. }),
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW GLOBAL STATUS",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Listing {
                kind: ShowListing::Status {
                    scope: Some(ShowScope::Global),
                },
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW FULL TRIGGERS FROM db",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Listing {
                kind: ShowListing::Triggers { full: true },
                from: Some(_),
                ..
            },
        ));

        // Bare family (no operand).
        let parsed = parse_with(
            "SHOW STORAGE ENGINES",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Bare {
                kind: ShowBare::Engines { storage: true },
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW FULL PROCESSLIST",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Bare {
                kind: ShowBare::Processlist { full: true },
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW BINARY LOG STATUS",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Bare {
                kind: ShowBare::BinaryLogStatus,
                ..
            },
        ));

        // Generalized SHOW CREATE <kind>.
        let parsed = parse_with(
            "SHOW CREATE VIEW v",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Create {
                kind: ShowCreateKind::View,
                if_not_exists: false,
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW CREATE DATABASE IF NOT EXISTS db",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Create {
                kind: ShowCreateKind::Database { schema: false },
                if_not_exists: true,
                ..
            },
        ));

        // Index listing (WHERE-only tail).
        let parsed = parse_with(
            "SHOW EXTENDED KEYS FROM t FROM db",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Index {
                spelling: ShowIndexSpelling::Keys,
                extended: true,
                database: Some(_),
                ..
            },
        ));

        // Engine dump (None engine == ALL).
        let parsed = parse_with(
            "SHOW ENGINE ALL MUTEX",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Engine {
                engine: None,
                artifact: ShowEngineArtifact::Mutex,
                ..
            },
        ));

        // Diagnostics: LIMIT offset,count and the COUNT(*) cardinality form.
        let parsed = parse_with(
            "SHOW WARNINGS LIMIT 1, 5",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Diagnostics {
                kind: ShowDiagnosticKind::Warnings,
                count: false,
                limit: Some(ShowLimit {
                    offset: Some(_),
                    ..
                }),
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW COUNT(*) ERRORS",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::Diagnostics {
                kind: ShowDiagnosticKind::Errors,
                count: true,
                limit: None,
                ..
            },
        ));

        // Replica status and routine code.
        let parsed = parse_with(
            "SHOW REPLICA STATUS FOR CHANNEL 'c'",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::ReplicaStatus {
                channel: Some(_),
                ..
            },
        ));
        let parsed = parse_with(
            "SHOW PROCEDURE CODE p",
            crate::ParseConfig::new(SHOW_ADMIN_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            show_admin_target(&parsed),
            ShowTarget::RoutineCode {
                kind: ShowRoutineKind::Procedure,
                ..
            },
        ));
    }

    /// The `SHOW INDEX` grammar admits only a `WHERE` narrowing, never `LIKE`.
    #[test]
    fn show_index_rejects_like_filter() {
        assert!(
            parse_with(
                "SHOW INDEX FROM t WHERE x = 1",
                crate::ParseConfig::new(SHOW_ADMIN_DIALECT)
            )
            .is_ok()
        );
        assert!(
            parse_with(
                "SHOW INDEX FROM t LIKE 'a%'",
                crate::ParseConfig::new(SHOW_ADMIN_DIALECT)
            )
            .is_err()
        );
    }

    #[test]
    fn show_admin_round_trips() {
        // Canonical spellings authored to render byte-identically; every probe is grammar
        // valid on MySQL 8.4.10 (PREPARE-only oracle: ER_UNSUPPORTED_PS 1295 counts as
        // grammar-recognized, only ER_PARSE_ERROR 1064 would be a reject).
        for sql in [
            "SHOW DATABASES",
            "SHOW DATABASES LIKE 'a%'",
            "SHOW SCHEMAS",
            "SHOW CHARSET",
            "SHOW CHARACTER SET LIKE 'utf8%'",
            "SHOW COLLATION WHERE Charset = 'utf8mb4'",
            "SHOW GLOBAL STATUS",
            "SHOW SESSION STATUS LIKE 'Threads%'",
            "SHOW LOCAL VARIABLES",
            "SHOW VARIABLES LIKE 'max%'",
            "SHOW EVENTS",
            "SHOW EVENTS FROM db",
            "SHOW TABLE STATUS FROM db LIKE 'a%'",
            "SHOW OPEN TABLES IN db",
            "SHOW TRIGGERS",
            "SHOW FULL TRIGGERS FROM db",
            "SHOW PLUGINS",
            "SHOW ENGINES",
            "SHOW STORAGE ENGINES",
            "SHOW PRIVILEGES",
            "SHOW PROFILES",
            "SHOW PROCESSLIST",
            "SHOW FULL PROCESSLIST",
            "SHOW BINARY LOGS",
            "SHOW REPLICAS",
            "SHOW BINARY LOG STATUS",
            "SHOW GRANTS",
            "SHOW GRANTS FOR u",
            "SHOW GRANTS FOR CURRENT_USER()",
            "SHOW GRANTS FOR u USING r1, r2",
            "SHOW CREATE USER u",
            "SHOW CREATE USER CURRENT_USER",
            "SHOW PROFILE",
            "SHOW PROFILE ALL",
            "SHOW PROFILE CPU, MEMORY",
            "SHOW PROFILE BLOCK IO FOR QUERY 2",
            "SHOW PROFILE CONTEXT SWITCHES, PAGE FAULTS LIMIT 5",
            "SHOW PROFILE ALL FOR QUERY 1 LIMIT 10 OFFSET 3",
            "SHOW PROFILE IPC, SWAPS, SOURCE LIMIT 2, 5",
            "SHOW BINLOG EVENTS",
            "SHOW BINLOG EVENTS IN 'log'",
            "SHOW BINLOG EVENTS FROM 4",
            "SHOW BINLOG EVENTS IN 'log' FROM 4 LIMIT 10",
            "SHOW BINLOG EVENTS LIMIT 2, 10",
            "SHOW BINLOG EVENTS LIMIT 10 OFFSET 2",
            "SHOW RELAYLOG EVENTS",
            "SHOW RELAYLOG EVENTS FOR CHANNEL 'c'",
            "SHOW RELAYLOG EVENTS IN 'log' FROM 4 LIMIT 10 FOR CHANNEL 'c'",
            "SHOW CREATE VIEW v",
            "SHOW CREATE DATABASE db",
            "SHOW CREATE SCHEMA db",
            "SHOW CREATE DATABASE IF NOT EXISTS db",
            "SHOW CREATE EVENT e",
            "SHOW CREATE PROCEDURE p",
            "SHOW CREATE FUNCTION f",
            "SHOW CREATE TRIGGER t",
            "SHOW INDEX FROM t",
            "SHOW INDEXES FROM t FROM db",
            "SHOW EXTENDED KEYS FROM t WHERE x = 1",
            "SHOW WARNINGS",
            "SHOW WARNINGS LIMIT 5",
            "SHOW WARNINGS LIMIT 5 OFFSET 2",
            "SHOW ERRORS LIMIT 1, 5",
            "SHOW COUNT(*) WARNINGS",
            "SHOW COUNT(*) ERRORS",
            "SHOW ENGINE INNODB STATUS",
            "SHOW ENGINE ALL MUTEX",
            "SHOW ENGINE INNODB LOGS",
            "SHOW REPLICA STATUS",
            "SHOW REPLICA STATUS FOR CHANNEL 'c'",
            "SHOW PROCEDURE CODE p",
            "SHOW FUNCTION CODE f",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_ADMIN_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Show { .. }]),
                "{sql:?} should be a typed SHOW, got {:?}",
                parsed.statements(),
            );
            let rendered = Renderer::new(SHOW_ADMIN_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    /// The account-bearing `SHOW` forms whose operand is a quoted `'<user>'@'<host>'` — the
    /// `@` lexes only under the MySQL preset's user-variable tokenizer, so these round-trip
    /// against `MySql` rather than the flag-isolated `SHOW_ADMIN_DIALECT` (the bare-name and
    /// `CURRENT_USER` account forms ride the isolated dialect in `show_admin_round_trips`).
    #[test]
    fn show_admin_account_host_forms_round_trip_under_mysql() {
        for sql in [
            "SHOW GRANTS FOR 'u'@'localhost'",
            "SHOW GRANTS FOR 'u'@'localhost' USING 'r'@'%'",
            "SHOW CREATE USER 'u'@'localhost'",
            "SHOW GRANTS FOR u@localhost",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Show { .. }]),
                "{sql:?} should be a typed SHOW, got {:?}",
                parsed.statements(),
            );
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_admin_parses_under_mysql_preset_and_sqlite_rejects() {
        use crate::dialect::{MySql, Sqlite};
        // A representative spread reaches the typed node under the fitted `MySql` preset;
        // SQLite has no session/typed SHOW at all, so the leading `SHOW` is never dispatched.
        for sql in [
            "SHOW DATABASES",
            "SHOW GLOBAL STATUS",
            "SHOW ENGINES",
            "SHOW CREATE VIEW v",
            "SHOW INDEX FROM t",
            "SHOW GRANTS",
            "SHOW WARNINGS LIMIT 5",
            "SHOW ENGINE INNODB STATUS",
            "SHOW REPLICA STATUS",
        ] {
            assert!(
                matches!(
                    parse_with(sql, crate::ParseConfig::new(MySql))
                        .unwrap()
                        .statements(),
                    [Statement::Show { .. }],
                ),
                "MySql should parse {sql:?} as a typed SHOW",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite should reject {sql:?} (no SHOW dispatch)",
            );
        }
    }

    // --- USE (DuckDB) -------------------------------------------------------

    /// ANSI with only the `use_statement` gate on, isolating the DuckDB `USE` dispatch
    /// from the rest of the preset; implements `RenderDialect` for the round-trip checks.
    const USE_DIALECT: FeatureDialect = {
        // The DuckDB-style `USE` grammar: the statement on and the dotted `catalog.schema`
        // name admitted (`use_qualified_name`). MySQL's single-ident form is exercised
        // against the `MySql` preset directly in `mysql_use_statement_*`.
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                use_statement: true,
                use_qualified_name: true,
                use_string_literal_name: true,
                ..UtilitySyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn use_of(parsed: &Parsed) -> &crate::ast::UseStatement {
        let [Statement::Use { use_statement, .. }] = parsed.statements() else {
            panic!("expected one USE statement, got {:?}", parsed.statements());
        };
        use_statement
    }

    #[test]
    fn use_statement_parses_one_and_two_part_names() {
        let parsed = parse_with("USE s1", crate::ParseConfig::new(USE_DIALECT))
            .expect("USE <schema> parses");
        assert_eq!(use_of(&parsed).name.0.len(), 1);
        assert_eq!(
            parsed.resolver().resolve(use_of(&parsed).name.0[0].sym),
            "s1"
        );

        let parsed = parse_with("USE memory.main", crate::ParseConfig::new(USE_DIALECT))
            .expect("USE <catalog>.<schema> parses");
        assert_eq!(use_of(&parsed).name.0.len(), 2);
    }

    #[test]
    fn use_statement_rejects_a_three_part_name() {
        // DuckDB parse-rejects `USE a.b.c` (`Expected "USE database" or "USE
        // database.schema"`); the arity bound is enforced rather than over-accepted.
        assert!(parse_with("USE a.b.c", crate::ParseConfig::new(USE_DIALECT)).is_err());
    }

    #[test]
    fn use_statement_is_gated_off_without_the_flag() {
        // `use_statement` off (the ANSI baseline) leaves a leading `USE` an unknown
        // statement — the reject path — while the identical text parses with the gate on.
        assert!(parse_with("USE s1", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("USE s1", crate::ParseConfig::new(USE_DIALECT)).is_ok());
    }

    #[test]
    fn use_statement_round_trips() {
        for sql in ["USE s1", "USE memory.main", "USE 'n'"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(USE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(USE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?}: {err}"));
            assert_eq!(rendered, sql, "exact round-trip for {sql:?}");
        }
        // Compact `use'n'` (no trivia) parses and renders the canonical form.
        let compact = parse_with("use'n'", crate::ParseConfig::new(USE_DIALECT))
            .expect("compact use'n' parses");
        assert_eq!(
            Renderer::new(USE_DIALECT)
                .render_parsed(&compact)
                .expect("renders"),
            "USE 'n'",
        );
    }

    #[test]
    fn use_statement_admits_sconst_name_spellings() {
        use crate::ast::QuoteStyle;
        use crate::dialect::{DuckDb, MySql};

        // DuckDB's USE name production admits plain / escape / dollar-quoted Sconsts
        // as a single-part name (engine-measured on libduckdb 1.5.4). Folded to a
        // single-quoted Ident so plain forms round-trip and E/$$ share the value.
        for (sql, value) in [
            ("USE 'n'", "n"),
            ("use'n'", "n"),
            ("USE E'n'", "n"),
            ("USE $$n$$", "n"),
            ("USE ''", ""),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let name = &use_of(&parsed).name;
            assert_eq!(name.0.len(), 1, "{sql:?}");
            assert_eq!(name.0[0].quote, QuoteStyle::Single, "{sql:?}");
            assert_eq!(parsed.resolver().resolve(name.0[0].sym), value, "{sql:?}");
        }

        // Dotted string names reject (DuckDB parser error at the `.`).
        for sql in ["USE 'a'.'b'", "USE 'a'.b", "USE a.'b'"] {
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("{sql:?} must reject"));
        }

        // MySQL rejects a string USE name (ER_PARSE_ERROR on mysql:8).
        parse_with("USE 'n'", crate::ParseConfig::new(MySql))
            .expect_err("MySQL USE rejects a string name");
    }

    /// The fitted `MySql` `FeatureSet` wired as a `RenderDialect` (the bare `MySql` preset
    /// struct is parse-only on this crate's test surface, like [`DUCKDB_RENDER`]).
    const MYSQL_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };

    #[test]
    fn mysql_use_statement_takes_a_single_unqualified_schema() {
        use crate::dialect::{DuckDb, MySql};

        // MySQL's `USE ident` switches the default schema by a single unqualified name and
        // round-trips; a dotted name is `ER_PARSE_ERROR`.
        let parsed = parse_with("USE s1", crate::ParseConfig::new(MySql))
            .expect("MySQL USE <schema> parses");
        assert_eq!(use_of(&parsed).name.0.len(), 1);
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("USE renders"),
            "USE s1",
        );
        parse_with("USE a.b", crate::ParseConfig::new(MySql))
            .expect_err("MySQL USE rejects a dotted name");

        // DuckDB, by contrast, admits the two-part `catalog.schema` name (`use_qualified_name`).
        assert!(parse_with("USE memory.main", crate::ParseConfig::new(DuckDb)).is_ok());
    }

    // --- UPDATE EXTENSIONS (DuckDB) -----------------------------------------

    fn update_extensions_of(parsed: &Parsed) -> &crate::ast::UpdateExtensionsStatement {
        let [
            Statement::UpdateExtensions {
                update_extensions, ..
            },
        ] = parsed.statements()
        else {
            panic!(
                "expected one UPDATE EXTENSIONS statement, got {:?}",
                parsed.statements(),
            );
        };
        update_extensions
    }

    #[test]
    fn update_extensions_captures_optional_name_list() {
        // Bare form: empty name list (refresh all installed).
        let parsed =
            parse_with("UPDATE EXTENSIONS", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert!(update_extensions_of(&parsed).extensions.is_empty());

        // Parenthesized form: the written `ColId` list, in order.
        let parsed = parse_with(
            "UPDATE EXTENSIONS (httpfs, json)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap_or_else(|err| panic!("{err:?}"));
        let names: Vec<_> = update_extensions_of(&parsed)
            .extensions
            .iter()
            .map(|ident| parsed.resolver().resolve(ident.sym))
            .collect();
        assert_eq!(names, ["httpfs", "json"]);
    }

    #[test]
    fn update_extensions_rejects_engine_rejects() {
        // DuckDB parser errors (engine-probed on 1.5.4): empty parens, a string operand, a
        // dotted name, and a `WITH` prefix are all rejected here too.
        for sql in [
            "UPDATE EXTENSIONS ()",
            "UPDATE EXTENSIONS ('httpfs')",
            "UPDATE EXTENSIONS (a.b)",
            "WITH x AS (SELECT 1) UPDATE EXTENSIONS",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER)).is_err(),
                "expected reject for {sql:?}",
            );
        }
    }

    #[test]
    fn update_extensions_does_not_disturb_the_dml_update_path() {
        // The `EXTENSIONS` seam claims a name only before `(` or the statement end, so a
        // table literally named `extensions` — and every ordinary `UPDATE … SET` — still
        // routes to the DML parser under the same gate-on dialect (engine-probed on 1.5.4:
        // `UPDATE extensions SET …` / `UPDATE EXTENSIONS AS e SET …` are DML `UPDATE`s).
        for sql in [
            "UPDATE t SET a = 1",
            "UPDATE extensions SET a = 1",
            "UPDATE EXTENSIONS SET a = 1",
            "UPDATE EXTENSIONS AS e SET a = 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Update { .. }]),
                "{sql:?} must parse as a DML UPDATE",
            );
        }
    }

    #[test]
    fn update_extensions_is_gated_off_without_the_flag() {
        // With `update_extensions` off (the ANSI baseline) the `EXTENSIONS` lookahead is
        // never taken: `UPDATE EXTENSIONS` reaches the DML parser as `UPDATE <rel=EXTENSIONS>`
        // and rejects for the missing `SET`, while the gate-on dialect accepts it.
        assert!(parse_with("UPDATE EXTENSIONS", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("UPDATE EXTENSIONS", crate::ParseConfig::new(DUCKDB_RENDER)).is_ok());
    }

    #[test]
    fn update_extensions_round_trips() {
        for sql in ["UPDATE EXTENSIONS", "UPDATE EXTENSIONS (httpfs, json)"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(DUCKDB_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?}: {err}"));
            assert_eq!(rendered, sql, "exact round-trip for {sql:?}");
        }
    }

    // --- PREPARE / EXECUTE / DEALLOCATE / CALL (DuckDB) ----------------------

    /// The fitted `DuckDb` `FeatureSet` wired as a `RenderDialect`, so the round-trip test
    /// can render the DuckDB-only statements (the `DuckDb` preset struct is parse-only on
    /// this crate's test surface, like [`KILL_DIALECT`]).
    const DUCKDB_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::DUCKDB,
    };

    fn prepare_of(parsed: &Parsed) -> &crate::ast::PrepareStatement {
        let [Statement::Prepare { prepare, .. }] = parsed.statements() else {
            panic!(
                "expected one PREPARE statement, got {:?}",
                parsed.statements()
            );
        };
        prepare
    }

    fn execute_of(parsed: &Parsed) -> &crate::ast::ExecuteStatement {
        let [Statement::Execute { execute, .. }] = parsed.statements() else {
            panic!(
                "expected one EXECUTE statement, got {:?}",
                parsed.statements()
            );
        };
        execute
    }

    fn call_of(parsed: &Parsed) -> &crate::ast::CallStatement {
        let [Statement::Call { call, .. }] = parsed.statements() else {
            panic!("expected one CALL statement, got {:?}", parsed.statements());
        };
        call
    }

    fn deallocate_of(parsed: &Parsed) -> &crate::ast::DeallocateStatement {
        let [Statement::Deallocate { deallocate, .. }] = parsed.statements() else {
            panic!(
                "expected one DEALLOCATE statement, got {:?}",
                parsed.statements(),
            );
        };
        deallocate
    }

    #[test]
    fn prepare_execute_call_capture_shapes() {
        // PREPARE wraps an arbitrary statement body (here a SELECT carrying the `?`
        // placeholder the fitted DuckDb preset now lexes).
        let parsed = parse_with(
            "PREPARE v1 AS SELECT 'Test' LIMIT ?",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        assert!(
            prepare_of(&parsed).statement.as_query().is_some(),
            "body is the SELECT",
        );
        // EXECUTE: a bare invocation leaves `args` empty; a parenthesized list fills it.
        let bare = parse_with("EXECUTE v1", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert!(execute_of(&bare).args.is_empty());
        let two = parse_with("EXECUTE v1(1, 2)", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert_eq!(execute_of(&two).args.len(), 2);
        // CALL: the parentheses are mandatory but may hold an empty list.
        let empty = parse_with(
            "CALL pragma_version()",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        assert!(call_of(&empty).args.is_empty());
        let one = parse_with(
            "CALL pragma_table_info('t')",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        assert_eq!(call_of(&one).args.len(), 1);
        // DEALLOCATE records whether the optional `PREPARE` keyword was written.
        let dealloc = parse_with("DEALLOCATE v1", crate::ParseConfig::new(DUCKDB_RENDER)).unwrap();
        assert!(!deallocate_of(&dealloc).prepare_keyword);
        let dealloc_prep = parse_with(
            "DEALLOCATE PREPARE v1",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .unwrap();
        assert!(deallocate_of(&dealloc_prep).prepare_keyword);
    }

    /// The `prepared_statements` + `prepared_statements_from` both-on combination is
    /// registry-rejected (`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`), so
    /// its parse semantics are deliberately left undefined: the `PREPARE`/`EXECUTE` heads dispatch
    /// DuckDB-first while `finish_deallocate_statement` resolves the tail MySQL-first (mandatory
    /// `PREPARE`). This test pins that decision — rely on the registry variant to declare the
    /// combination conflicted rather than reconciling the two winners in the parser — by asserting
    /// the registry flags the delta.
    ///
    /// The parse-entry `debug_assert!` enforces the same verdict, so in a debug
    /// build the seam trips before any token is read and a parser over this delta is unbuildable —
    /// the incoherent tail is unreachable there. That debug-time enforcement is pinned by the
    /// companion [`both_prepared_flags_trip_the_grammar_conflict_assert`] below.
    #[test]
    fn deallocate_under_both_prepared_flags_is_registry_rejected() {
        const BOTH_ON: FeatureSet =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                prepared_statements_from: true,
                ..FeatureSet::DUCKDB.utility_syntax
            }));

        // The registry declares the combination conflicted — this is the pinned semantics.
        assert_eq!(
            BOTH_ON.grammar_conflict(),
            Some(GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom),
        );
    }

    /// Debug-build enforcement of the registry rejection above: the parse-entry
    /// `debug_assert!` refuses to build a parser over a grammar-conflicting feature set, so
    /// constructing one over the both-prepared-flags delta trips the seam. Gated on
    /// `debug_assertions` because the assert compiles out in release, where the panic would not
    /// fire.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "grammar-position conflict")]
    fn both_prepared_flags_trip_the_grammar_conflict_assert() {
        const BOTH_ON: FeatureSet =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                prepared_statements_from: true,
                ..FeatureSet::DUCKDB.utility_syntax
            }));
        const BOTH_ON_RENDER: FeatureDialect = FeatureDialect { features: &BOTH_ON };
        let _ = parse_with(
            "DEALLOCATE PREPARE p",
            crate::ParseConfig::new(BOTH_ON_RENDER),
        );
    }

    /// Debug-build enforcement of the `feature_dependencies` registry at the parse-entry seam:
    /// a refinement flag enabled without the base it rides on (here
    /// `call_bare_name` without the base `call` statement,
    /// [`FeatureDependencyViolation::CallBareNameWithoutCall`]) makes the flag inert, and the seam
    /// `debug_assert!` refuses to build a parser over the dependency-violating set. Gated on
    /// `debug_assertions` because the assert compiles out in release.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "unsatisfied grammar-flag dependency")]
    fn dependency_violating_delta_trips_the_feature_dependency_assert() {
        const DANGLING: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
                call_bare_name: true,
                call: false,
                ..FeatureSet::ANSI.utility_syntax
            }));
        // The registry names the violation — the seam asserts this same verdict.
        const _: () = assert!(matches!(
            DANGLING.feature_dependencies(),
            Some(FeatureDependencyViolation::CallBareNameWithoutCall)
        ));
        const DANGLING_RENDER: FeatureDialect = FeatureDialect {
            features: &DANGLING,
        };
        let _ = parse_with("CALL p", crate::ParseConfig::new(DANGLING_RENDER));
    }

    #[test]
    fn prepare_execute_call_round_trip() {
        for sql in [
            "PREPARE v1 AS SELECT 'Test' LIMIT ?",
            "EXECUTE v1",
            "EXECUTE v1(1, 2)",
            "DEALLOCATE v1",
            "DEALLOCATE PREPARE v1",
            "CALL pragma_version()",
            "CALL pragma_table_info('t')",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(DUCKDB_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    /// The PostgreSQL `PREPARE name ( <type> [, ...] ) AS <statement>` typed
    /// parameter-type list (`planner-parity-prepare-typed-parameters`; pg_query
    /// 6.1.1-verified): accepted (with parameterized and array element types) under
    /// PostgreSQL, empty parens rejected, the gate-off DuckDB error shape preserved,
    /// and MySQL's unrelated `PREPARE ... FROM` grammar unaffected.
    #[test]
    fn prepare_typed_parameter_list() {
        use crate::dialect::{MySql, Postgres};

        // Two full type names, round-tripping with the AST carrying both.
        let parsed = parse_with(
            "PREPARE p(int, text) AS SELECT $1, $2",
            crate::ParseConfig::new(Postgres),
        )
        .expect("typed parameter list parses under PostgreSQL");
        assert_eq!(prepare_of(&parsed).parameter_types.len(), 2);
        let rendered = Renderer::new(Postgres)
            .render_parsed(&parsed)
            .expect("typed PREPARE renders");
        assert_eq!(rendered, "PREPARE p(INTEGER, TEXT) AS SELECT $1, $2");

        // A parameterized type and an array element type, both accepted.
        for (sql, expected) in [
            (
                "PREPARE p(numeric(10, 2)) AS SELECT $1",
                "PREPARE p(NUMERIC(10, 2)) AS SELECT $1",
            ),
            (
                "PREPARE p(int[]) AS SELECT $1",
                "PREPARE p(INTEGER[]) AS SELECT $1",
            ),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(prepare_of(&parsed).parameter_types.len(), 1);
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, expected, "round-trip");
        }

        // PostgreSQL rejects an empty written `()` (pg_query-verified).
        assert!(
            parse_with("PREPARE p() AS SELECT 1", crate::ParseConfig::new(Postgres)).is_err(),
            "empty parameter-type list should reject",
        );

        // Bare `PREPARE` (no parens) still parses and carries an empty list.
        let bare = parse_with("PREPARE p AS SELECT 1", crate::ParseConfig::new(Postgres))
            .expect("bare PREPARE still parses");
        assert!(prepare_of(&bare).parameter_types.is_empty());

        // DuckDB structurally rejects the typed list (`prepare_typed_parameters` off): the
        // `(` after the name is left untouched, so the statement falls through to the `AS`
        // expectation and errors there — today's error shape, unchanged by this widening.
        assert!(
            parse_with(
                "PREPARE p(int) AS SELECT $1",
                crate::ParseConfig::new(DUCKDB_RENDER)
            )
            .is_err(),
            "DuckDB should reject the typed parameter list",
        );

        // MySQL's `PREPARE stmt_name FROM <string>` grammar (its own
        // `prepared_statements_from` gate) has no type-list slot: the `(` after the name
        // falls through to the `FROM` expectation and errors there, engine-matching
        // (`PREPARE p(int) FROM 'x'` is `ER_PARSE_ERROR` — the typed list is
        // PostgreSQL-only).
        assert!(
            parse_with(
                "PREPARE p(int) FROM 'SELECT 1'",
                crate::ParseConfig::new(MySql)
            )
            .is_err(),
            "MySQL's PREPARE ... FROM grammar admits no type list",
        );
    }

    #[test]
    fn pragma_is_dispatched_under_the_duckdb_preset() {
        use crate::dialect::DuckDb;

        // The DuckDb preset flips `utility_syntax.pragma` on, reusing the SQLite
        // `PragmaStatement` grammar for the bare and assignment forms the corpus carries
        // (`duckdb-settings-and-session-statements`); under PostgreSQL the same text is a
        // gated-off unknown statement.
        for sql in ["PRAGMA verify_parallelism", "PRAGMA default_order = 'ASC'"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Pragma { .. }]),
                "{sql:?} should parse to a PRAGMA under DuckDb",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres)).is_err(),
                "{sql:?} is gated off under PostgreSQL",
            );
        }
    }

    #[test]
    fn prepare_execute_call_reject_edge_cases() {
        // Engine-verified rejects on DuckDB 1.5.4 (`Connection::prepare`): an empty
        // `EXECUTE` argument list, a paren-less `CALL`, and `DEALLOCATE ALL` (DuckDB has no
        // all-form). The remaining two are structurally incomplete (no AS body / no name),
        // which every parser rejects.
        for sql in [
            "EXECUTE v1()",
            "CALL pragma_version",
            "DEALLOCATE ALL",
            "PREPARE v1 AS",
            "DEALLOCATE",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER)).is_err(),
                "DuckDb should reject {sql:?}",
            );
        }
    }

    #[test]
    fn prepared_statements_and_call_gated_off_by_default() {
        use crate::dialect::{Ansi, Postgres};
        // `prepared_statements`/`call` are off outside Ansi (no fitted preset ships
        // them there), so the leading keywords fall through to the unknown-statement
        // error; `?` likewise stays a stray byte where `anonymous_question` is off
        // (PostgreSQL uses `$1`).
        for sql in [
            "PREPARE v1 AS SELECT 1",
            "EXECUTE v1(1)",
            "DEALLOCATE v1",
            "CALL f(1)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "Ansi should reject {sql:?}"
            );
        }
        // PostgreSQL has its own `PREPARE`/`EXECUTE`/`DEALLOCATE` prepared-statement
        // lifecycle (`planner-parity-prepare-typed-parameters`), so only `CALL` stays
        // rejected there — its own flag, off under PostgreSQL until a routine-call
        // grammar ticket fits it.
        assert!(
            parse_with("PREPARE v1 AS SELECT 1", crate::ParseConfig::new(Postgres)).is_ok(),
            "Postgres should accept the bare PREPARE form",
        );
        assert!(
            parse_with("EXECUTE v1(1)", crate::ParseConfig::new(Postgres)).is_ok(),
            "Postgres should accept EXECUTE",
        );
        assert!(
            parse_with("DEALLOCATE v1", crate::ParseConfig::new(Postgres)).is_ok(),
            "Postgres should accept DEALLOCATE",
        );
        assert!(
            parse_with("CALL f(1)", crate::ParseConfig::new(Postgres)).is_err(),
            "Postgres should reject CALL (its own, still-off flag)",
        );
        assert!(
            parse_with("SELECT ?", crate::ParseConfig::new(Postgres)).is_err(),
            "`?` stays a stray byte under PostgreSQL",
        );
    }

    /// MySQL's `CALL sp_name opt_paren_expr_list` stored-procedure invocation (`parse-mysql-call`).
    /// The parenthesized argument list is *optional* — a bare `CALL p`, an empty `CALL p()`, and a
    /// filled `CALL p(1, 2)` all parse and round-trip; the `parenthesized` surface flag distinguishes
    /// the bare form from the empty-parens form. The name is capped like a relation target
    /// (`sp_name` is `ident '.' ident | ident`), so a three-part `a.b.c` name rejects. All boundaries
    /// measured on mysql:8.4.10 (`corpus_mysql_verdicts::mysql_call_bare_and_parenthesized_forms_evidence`).
    #[test]
    fn mysql_call_bare_and_parenthesized_forms() {
        use crate::dialect::MySql;
        const MYSQL_RENDER: FeatureDialect = FeatureDialect {
            features: &FeatureSet::MYSQL,
        };

        // The bare form: no argument list at all, so `parenthesized` is false and `args` empty.
        let bare = parse_with("CALL p", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts the bare CALL form");
        assert!(
            !call_of(&bare).parenthesized,
            "bare CALL has no argument list"
        );
        assert!(call_of(&bare).args.is_empty());

        // The empty-parens form: `parenthesized` true, `args` empty — a distinct written shape.
        let empty = parse_with("CALL p()", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts empty parens");
        assert!(call_of(&empty).parenthesized);
        assert!(call_of(&empty).args.is_empty());

        // A filled and a qualified `db.proc` form.
        let two = parse_with("CALL p(1, 2)", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts a filled list");
        assert!(call_of(&two).parenthesized);
        assert_eq!(call_of(&two).args.len(), 2);
        let qualified = parse_with("CALL db.p(1)", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts a db.proc name");
        assert_eq!(call_of(&qualified).name.0.len(), 2);

        // Every grammar-valid form round-trips byte-identically (the bare form renders no `()`).
        for sql in ["CALL p", "CALL p()", "CALL p(1, 2)", "CALL db.p(1)"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }

        // A three-part name is a syntax error (MySQL's `sp_name` is at most `db.proc`).
        assert!(
            parse_with("CALL a.b.c(1)", crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects a three-part routine name",
        );
        // A bare name with no parens is a syntax error where `call_bare_name` is off: DuckDB's
        // parentheses are mandatory, so `CALL pragma_version` still rejects there.
        assert!(
            parse_with(
                "CALL pragma_version",
                crate::ParseConfig::new(DUCKDB_RENDER)
            )
            .is_err(),
            "DuckDB requires the parenthesized argument list",
        );
    }

    // --- PREPARE ... FROM / EXECUTE ... USING / {DEALLOCATE | DROP} PREPARE (MySQL) ---

    fn prepare_from_of(parsed: &Parsed) -> &crate::ast::PrepareFromStatement {
        let [Statement::PrepareFrom { prepare_from, .. }] = parsed.statements() else {
            panic!(
                "expected one PREPARE ... FROM statement, got {:?}",
                parsed.statements(),
            );
        };
        prepare_from
    }

    fn execute_using_of(parsed: &Parsed) -> &crate::ast::ExecuteUsingStatement {
        let [Statement::ExecuteUsing { execute_using, .. }] = parsed.statements() else {
            panic!(
                "expected one EXECUTE [USING] statement, got {:?}",
                parsed.statements(),
            );
        };
        execute_using
    }

    #[test]
    fn prepare_from_family_captures_shapes() {
        use crate::ast::{DeallocateKeyword, PrepareSource, QuoteStyle};
        use crate::dialect::MySql;

        // The string source is the opaque `Text` arm; the `@var` source is `Variable`, its
        // name held sigil-less with the quote style preserved (each `ident_or_text`
        // spelling: folded `@name`, and the standalone-`@` quoted forms).
        let text = parse_with("PREPARE s FROM 'SELECT 1'", crate::ParseConfig::new(MySql)).unwrap();
        assert!(matches!(
            prepare_from_of(&text).source,
            PrepareSource::Text { .. },
        ));
        let bare_var = parse_with("PREPARE s FROM @code", crate::ParseConfig::new(MySql)).unwrap();
        let PrepareSource::Variable { name, .. } = &prepare_from_of(&bare_var).source else {
            panic!("expected a Variable source");
        };
        assert_eq!(name.quote, QuoteStyle::None);
        let quoted_var =
            parse_with("PREPARE s FROM @'code'", crate::ParseConfig::new(MySql)).unwrap();
        let PrepareSource::Variable { name, .. } = &prepare_from_of(&quoted_var).source else {
            panic!("expected a Variable source");
        };
        assert_eq!(name.quote, QuoteStyle::Single);

        // A bare EXECUTE leaves the USING list empty; the list members are sigil-less names.
        let bare = parse_with("EXECUTE s", crate::ParseConfig::new(MySql)).unwrap();
        assert!(execute_using_of(&bare).using.is_empty());
        let two = parse_with("EXECUTE s USING @a, @'b'", crate::ParseConfig::new(MySql)).unwrap();
        assert_eq!(execute_using_of(&two).using.len(), 2);
        assert_eq!(execute_using_of(&two).using[1].quote, QuoteStyle::Single);

        // The release verb records its `deallocate_or_drop` spelling; the mandatory
        // `PREPARE` keyword is always recorded written.
        let dealloc = parse_with("DEALLOCATE PREPARE s", crate::ParseConfig::new(MySql)).unwrap();
        assert_eq!(
            deallocate_of(&dealloc).keyword,
            DeallocateKeyword::Deallocate,
        );
        assert!(deallocate_of(&dealloc).prepare_keyword);
        let drop = parse_with("DROP PREPARE s", crate::ParseConfig::new(MySql)).unwrap();
        assert_eq!(deallocate_of(&drop).keyword, DeallocateKeyword::Drop);
        assert!(deallocate_of(&drop).prepare_keyword);
    }

    /// Every grammar-valid form of the MySQL lifecycle round-trips byte-identically —
    /// including the `DEALLOCATE`-vs-`DROP` verb spelling and each `ident_or_text` quote
    /// spelling of a `@`-variable. The same forms are live-oracle-verified (all
    /// `ER_UNSUPPORTED_PS` 1295, grammar-valid) in
    /// `corpus_mysql_verdicts::mysql_prepared_statement_live_oracle_parity`.
    #[test]
    fn prepare_from_family_round_trips() {
        use crate::dialect::MySql;
        for sql in [
            "PREPARE s FROM 'SELECT 1'",
            "PREPARE `s` FROM 'SELECT 1'",
            "PREPARE s FROM @code",
            "PREPARE s FROM @'code'",
            "PREPARE s FROM @\"code\"",
            "PREPARE s FROM @`code`",
            "PREPARE s FROM 'PREPARE x FROM \\'SELECT 1\\''",
            "EXECUTE s",
            "EXECUTE s USING @a",
            "EXECUTE s USING @a, @b",
            "EXECUTE s USING @a, @'b'",
            "EXECUTE s USING @`a`",
            "DEALLOCATE PREPARE s",
            "DEALLOCATE PREPARE `s`",
            "DROP PREPARE s",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn prepare_from_family_reject_edge_cases() {
        use crate::dialect::MySql;
        // Engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10: a `prepare_src` is a
        // string or `@`-variable only, the `EXECUTE` arguments ride `USING @var` only (no
        // values, no parenthesized list, no `@@` system variables), the release verb's
        // `PREPARE` keyword is mandatory, single-name, with no `IF EXISTS` guard. The same
        // boundaries are both-reject-pinned in `m3::SCHEMA_INDEPENDENT_REJECT`.
        for sql in [
            "PREPARE s FROM 1+1",
            "PREPARE s FROM SELECT 1",
            "PREPARE s FROM @@global.x",
            "PREPARE s",
            "EXECUTE s USING 1",
            "EXECUTE s USING",
            "EXECUTE s (1)",
            "EXECUTE s ()",
            "EXECUTE s USING @@x",
            "DEALLOCATE s",
            "DEALLOCATE PREPARE s, t",
            "DROP PREPARE IF EXISTS s",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySql should reject {sql:?}",
            );
        }
    }

    #[test]
    fn prepare_from_family_gated_off_elsewhere() {
        use crate::dialect::{Ansi, Postgres};
        // `prepared_statements_from` is MySQL-only: under the typed-`AS` dialects the
        // `FROM`/`USING` spellings reject (the shared keywords dispatch to the other
        // grammar), and under ANSI the keywords are not dispatched at all. `DROP PREPARE`
        // falls through to the generic drop path everywhere else -> unknown object kind.
        for sql in [
            "PREPARE s FROM 'SELECT 1'",
            "EXECUTE s USING @a",
            "DROP PREPARE s",
        ] {
            for (dialect_name, result) in [
                ("Ansi", parse_with(sql, crate::ParseConfig::new(Ansi))),
                (
                    "Postgres",
                    parse_with(sql, crate::ParseConfig::new(Postgres)),
                ),
                (
                    "DuckDb",
                    parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER)),
                ),
            ] {
                assert!(result.is_err(), "{dialect_name} should reject {sql:?}");
            }
        }
    }

    fn do_of(parsed: &Parsed) -> &crate::ast::DoStatement {
        let [Statement::Do { do_block, .. }] = parsed.statements() else {
            panic!("expected one DO statement, got {:?}", parsed.statements());
        };
        do_block
    }

    #[test]
    fn do_captures_the_free_arg_list_in_source_order() {
        use crate::dialect::Postgres;

        // Body only — one `Body` arg.
        let body = parse_with("DO $$BEGIN NULL; END$$", crate::ParseConfig::new(Postgres)).unwrap();
        assert!(matches!(do_of(&body).args.as_slice(), [DoArg::Body { .. }]));

        // Both `LANGUAGE`/body orders round-trip as distinct arg sequences.
        let lang_first = parse_with(
            "DO LANGUAGE plpgsql $$x$$",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&lang_first).args.as_slice(),
            [DoArg::Language { .. }, DoArg::Body { .. }],
        ));
        let body_first = parse_with(
            "DO $$x$$ LANGUAGE plpgsql",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&body_first).args.as_slice(),
            [DoArg::Body { .. }, DoArg::Language { .. }],
        ));

        // The raw-parse forms libpg_query accepts but only rejects at execution: a
        // language-only block, a repeated body, and a repeated language all parse here, since
        // the "exactly one body, at most one language" check is deferred (like `PREPARE`).
        let lang_only =
            parse_with("DO LANGUAGE plpgsql", crate::ParseConfig::new(Postgres)).unwrap();
        assert!(matches!(
            do_of(&lang_only).args.as_slice(),
            [DoArg::Language { .. }]
        ));
        let two_bodies = parse_with("DO $$a$$ $$b$$", crate::ParseConfig::new(Postgres)).unwrap();
        assert_eq!(do_of(&two_bodies).args.len(), 2);
        let two_langs = parse_with(
            "DO 'x' LANGUAGE a LANGUAGE b",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&two_langs).args.as_slice(),
            [
                DoArg::Body { .. },
                DoArg::Language { .. },
                DoArg::Language { .. }
            ],
        ));
    }

    #[test]
    fn do_round_trips_every_form() {
        use crate::dialect::Postgres;
        for sql in [
            "DO $$BEGIN NULL; END$$",
            "DO 'BEGIN NULL; END'",
            "DO LANGUAGE plpgsql $$x$$",
            "DO $$x$$ LANGUAGE plpgsql",
            "DO LANGUAGE plpgsql",
            "DO $$a$$ $$b$$",
            "DO 'x' LANGUAGE a LANGUAGE b",
            "DO 'x' LANGUAGE 'plpgsql'",
            "DO LANGUAGE 'plpgsql' $$x$$",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn do_rejects_parse_boundary_forms() {
        use crate::dialect::Postgres;
        // The reject boundary PostgreSQL enforces at *parse* (not execution): an empty list
        // (bare `DO`), a non-string / non-`LANGUAGE` item, a dangling `LANGUAGE`, a trailing
        // token past a complete block, and a reserved-word language name.
        for sql in [
            "DO",
            "DO 42",
            "DO $$x$$ LANGUAGE",
            "DO 'x' FOO",
            "DO LANGUAGE select 'x'",
            // A code block is an `Sconst`, not a bit/hex/national constant: libpg_query
            // rejects each of these (a `bit`-typed `BCONST`/`XCONST` or the bare word `N`
            // that a national spelling lexes to), so the list never opens a body.
            "DO b'0'",
            "DO x'ab'",
            "DO N'x'",
            // The `LANGUAGE` operand is a `NonReservedWord_or_Sconst` — a bit/hex constant is
            // neither, so it rejects too (`DO 'x' LANGUAGE b'0'` is a syntax error in pg).
            "DO 'x' LANGUAGE b'0'",
            "DO 'x' LANGUAGE x'ab'",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "Postgres should reject {sql:?}",
            );
        }
    }

    #[test]
    fn do_language_accepts_word_or_sconst_string() {
        use crate::dialect::Postgres;
        // `NonReservedWord_or_Sconst`: the operand is a bare word or an `Sconst` string, in
        // either `LANGUAGE`/body order. libpg_query accepts each of these.
        let word =
            parse_with("DO 'x' LANGUAGE plpgsql", crate::ParseConfig::new(Postgres)).unwrap();
        assert!(matches!(
            do_of(&word).args.as_slice(),
            [
                DoArg::Body { .. },
                DoArg::Language {
                    name: LanguageName::Word { .. },
                    ..
                }
            ],
        ));
        let string = parse_with(
            "DO 'x' LANGUAGE 'plpgsql'",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&string).args.as_slice(),
            [
                DoArg::Body { .. },
                DoArg::Language {
                    name: LanguageName::String { .. },
                    ..
                }
            ],
        ));
        let string_first = parse_with(
            "DO LANGUAGE 'plpgsql' $$x$$",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&string_first).args.as_slice(),
            [
                DoArg::Language {
                    name: LanguageName::String { .. },
                    ..
                },
                DoArg::Body { .. }
            ],
        ));
        // An `E'…'` escape string and a dollar-quoted string are `Sconst`s too.
        let escape = parse_with(
            "DO 'x' LANGUAGE E'plpgsql'",
            crate::ParseConfig::new(Postgres),
        )
        .unwrap();
        assert!(matches!(
            do_of(&escape).args.as_slice(),
            [
                DoArg::Body { .. },
                DoArg::Language {
                    name: LanguageName::String { .. },
                    ..
                }
            ],
        ));
    }

    #[test]
    fn do_is_gated_off_outside_postgres() {
        use crate::dialect::{Ansi, DuckDb, Sqlite};
        // `do_statement` is a leading-keyword gate on for PostgreSQL/Lenient only; elsewhere the
        // `DO` code block is not dispatched and surfaces as an unknown statement. (MySQL is
        // excluded: it dispatches the *different* `do_expression_list` behaviour on `DO`, so
        // `DO 'x'` parses there as the expression form — see `mysql_do_expressions_*`.)
        for dialect_rejects in [
            parse_with("DO 'x'", crate::ParseConfig::new(Ansi)).is_err(),
            parse_with("DO 'x'", crate::ParseConfig::new(DuckDb)).is_err(),
            parse_with("DO 'x'", crate::ParseConfig::new(Sqlite)).is_err(),
        ] {
            assert!(
                dialect_rejects,
                "DO is gated off outside PostgreSQL/Lenient"
            );
        }
    }

    fn do_expressions_of(parsed: &Parsed) -> &crate::ast::DoExpressionsStatement {
        let [Statement::DoExpressions { do_expressions, .. }] = parsed.statements() else {
            panic!(
                "expected one MySQL DO-expressions statement, got {:?}",
                parsed.statements()
            );
        };
        do_expressions
    }

    #[test]
    fn mysql_do_expressions_parses_and_round_trips() {
        use crate::dialect::MySql;

        // MySQL's `DO <expr-list>` evaluates a select-item list for side effects — a distinct
        // behaviour on the `DO` keyword from PostgreSQL's code block. The list round-trips and
        // an alias is grammar-legal (`DO select_item_list`), so it parses like a projection.
        let parsed = parse_with("DO 1 + 1, SLEEP(0)", crate::ParseConfig::new(MySql))
            .expect("MySQL DO parses");
        assert_eq!(do_expressions_of(&parsed).items.len(), 2);
        for sql in ["DO 1 + 1", "DO 1, 2, 3", "DO SLEEP(0)", "DO 1 AS x"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }

        // A bare `DO` has no expression list — `ER_PARSE_ERROR` on mysql:8.
        parse_with("DO", crate::ParseConfig::new(MySql))
            .expect_err("MySQL DO requires an expression list");
    }

    // --- LOCK / UNLOCK TABLES and LOCK / UNLOCK INSTANCE (MySQL) --------------

    fn lock_tables_of(parsed: &Parsed) -> &crate::ast::LockTablesStatement {
        let [Statement::LockTables { lock_tables, .. }] = parsed.statements() else {
            panic!(
                "expected one LOCK TABLES statement, got {:?}",
                parsed.statements()
            );
        };
        lock_tables
    }

    #[test]
    fn mysql_lock_tables_parses_shapes_and_round_trips() {
        use crate::ast::TableLockKind;
        use crate::dialect::MySql;

        // The full per-table shape: qualified name, bare and `AS` aliases, all three lock
        // kinds, and a multi-entry list (mysql `sql_yacc.yy` `table_lock_list`; every shape
        // engine-verified grammar-positive on 8.4.10 — 1046/1295, never 1064).
        let parsed = parse_with(
            "LOCK TABLES t1 AS a READ LOCAL, db.t2 WRITE, t3 b READ",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL LOCK TABLES parses");
        let lock = lock_tables_of(&parsed);
        assert!(lock.plural);
        assert_eq!(lock.tables.len(), 3);
        assert_eq!(lock.tables[0].kind, TableLockKind::ReadLocal);
        assert!(lock.tables[0].alias.is_some());
        assert_eq!(lock.tables[1].kind, TableLockKind::Write);
        assert_eq!(lock.tables[1].name.0.len(), 2);
        assert_eq!(lock.tables[2].kind, TableLockKind::Read);

        // The `TABLES`/`TABLE` spelling and each lock kind round-trip exactly; a bare alias
        // renders as written.
        for sql in [
            "LOCK TABLES t1 READ",
            "LOCK TABLE t1 READ",
            "LOCK TABLES t1 READ LOCAL",
            "LOCK TABLES t1 WRITE",
            "LOCK TABLES t1 a READ, db.t2 WRITE",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }

        // The `AS` alias spelling is not recorded (MySQL's `opt_as` noise keyword), so it
        // renders in the canonical bare form.
        let parsed = parse_with("LOCK TABLES t1 AS a READ", crate::ParseConfig::new(MySql))
            .expect("AS alias parses");
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("renders"),
            "LOCK TABLES t1 a READ",
        );
    }

    #[test]
    fn mysql_lock_tables_rejects_measured_boundaries() {
        use crate::dialect::MySql;

        // Each engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10: the lock kind is
        // mandatory, the list is non-empty, a bare `LOCK` is nothing, and the pre-8.0
        // `LOW_PRIORITY WRITE` modifier is gone — `LOW_PRIORITY` is a MySQL reserved word,
        // so the bare-alias position rejects it without a spelling special case.
        for sql in [
            "LOCK TABLES t1",
            "LOCK TABLES",
            "LOCK",
            "LOCK TABLES t1 LOW_PRIORITY WRITE",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql)).expect_err(sql);
        }
    }

    fn unlock_tables_of(parsed: &Parsed) -> &crate::ast::UnlockTablesStatement {
        let [Statement::UnlockTables { unlock_tables, .. }] = parsed.statements() else {
            panic!(
                "expected one UNLOCK TABLES statement, got {:?}",
                parsed.statements()
            );
        };
        unlock_tables
    }

    #[test]
    fn mysql_unlock_tables_parses_and_round_trips() {
        use crate::dialect::MySql;

        for (sql, plural) in [("UNLOCK TABLES", true), ("UNLOCK TABLE", false)] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(unlock_tables_of(&parsed).plural, plural);
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }
        // A bare `UNLOCK` is `ER_PARSE_ERROR` on mysql:8.4.10.
        parse_with("UNLOCK", crate::ParseConfig::new(MySql))
            .expect_err("UNLOCK needs TABLES/TABLE or INSTANCE");
    }

    fn instance_lock_of(parsed: &Parsed) -> &crate::ast::InstanceLockStatement {
        let [Statement::InstanceLock { instance_lock, .. }] = parsed.statements() else {
            panic!(
                "expected one instance-lock statement, got {:?}",
                parsed.statements()
            );
        };
        instance_lock
    }

    #[test]
    fn mysql_instance_lock_parses_and_round_trips() {
        use crate::dialect::MySql;

        for (sql, acquire) in [
            ("LOCK INSTANCE FOR BACKUP", true),
            ("UNLOCK INSTANCE", false),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(instance_lock_of(&parsed).acquire, acquire);
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }
        // The `FOR BACKUP` tail is mandatory on the acquire side.
        parse_with("LOCK INSTANCE", crate::ParseConfig::new(MySql))
            .expect_err("LOCK INSTANCE requires FOR BACKUP");
    }

    #[test]
    fn lock_statements_are_gated_off_outside_mysql() {
        use crate::dialect::{Ansi, DuckDb, Postgres, Sqlite};

        // `lock_tables`/`lock_instance` are leading-keyword gates on for MySQL/Lenient only;
        // elsewhere `LOCK`/`UNLOCK` are not dispatched and surface as unknown statements —
        // including PostgreSQL, whose own statement-level mode-list `LOCK TABLE` is a
        // different, not-yet-modelled behaviour with its own future gate.
        for sql in [
            "LOCK TABLES t1 READ",
            "UNLOCK TABLES",
            "LOCK INSTANCE FOR BACKUP",
            "UNLOCK INSTANCE",
        ] {
            for rejects in [
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            ] {
                assert!(rejects, "{sql:?} is gated off outside MySQL/Lenient");
            }
        }
    }

    // --- LOAD DATA / LOAD XML (MySQL) ---------------------------------------

    fn load_data_of(parsed: &Parsed) -> &crate::ast::LoadDataStatement {
        let [Statement::LoadData { load_data, .. }] = parsed.statements() else {
            panic!(
                "expected one LOAD DATA statement, got {:?}",
                parsed.statements()
            );
        };
        load_data
    }

    #[test]
    fn load_data_full_clause_train_parses_and_round_trips() {
        use crate::ast::{
            LoadDataConcurrency, LoadDataDuplicate, LoadDataFieldOrVar, LoadDataFormat,
            LoadDataIgnoreUnit,
        };
        use crate::dialect::MySql;

        let sql = "LOAD DATA LOW_PRIORITY LOCAL INFILE 'f.tsv' REPLACE INTO TABLE t \
                   PARTITION (p0, p1) CHARACTER SET utf8mb4 \
                   FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' ESCAPED BY '\\\\' \
                   LINES STARTING BY '>' TERMINATED BY '\\n' \
                   IGNORE 1 LINES (a, @v) SET b = @v";
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let load = load_data_of(&parsed);
        assert_eq!(load.format, LoadDataFormat::Data);
        assert_eq!(load.concurrency, Some(LoadDataConcurrency::LowPriority));
        assert!(load.local);
        assert_eq!(load.on_duplicate, Some(LoadDataDuplicate::Replace));
        assert_eq!(load.partitions.len(), 2);
        assert!(load.charset.is_some());
        let fields = load.fields.as_ref().expect("a FIELDS clause");
        assert!(fields.terminated_by.is_some());
        let enclosed = fields
            .enclosed_by
            .as_ref()
            .expect("an ENCLOSED BY sub-clause");
        assert!(enclosed.optionally);
        assert!(fields.escaped_by.is_some());
        let lines = load.lines.as_ref().expect("a LINES clause");
        assert!(lines.starting_by.is_some() && lines.terminated_by.is_some());
        let ignore = load.ignore_rows.as_ref().expect("an IGNORE clause");
        assert_eq!(ignore.unit, LoadDataIgnoreUnit::Lines);
        // The `@v` target rides the `Variable` arm; the column `a` the `Column` arm.
        assert_eq!(load.columns.len(), 2);
        assert!(matches!(load.columns[0], LoadDataFieldOrVar::Column { .. }));
        assert!(matches!(
            load.columns[1],
            LoadDataFieldOrVar::Variable { .. }
        ));
        assert_eq!(load.set.len(), 1);

        let rendered = Renderer::new(MYSQL_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
        assert_eq!(rendered, sql, "round-trip");
    }

    #[test]
    fn load_xml_with_rows_identified_by_parses_and_round_trips() {
        use crate::ast::LoadDataFormat;
        use crate::dialect::MySql;

        // Every clause is grammar-shared with `LOAD DATA` (engine-measured: the XML-vs-DATA
        // clause restrictions are semantic, not syntactic), so `ROWS IDENTIFIED BY` parses here.
        let sql = "LOAD XML LOCAL INFILE 'f.xml' INTO TABLE t CHARACTER SET utf8mb4 \
                   ROWS IDENTIFIED BY '<row>' IGNORE 2 ROWS (a, @v) SET b = @v";
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let load = load_data_of(&parsed);
        assert_eq!(load.format, LoadDataFormat::Xml);
        assert!(load.rows_identified_by.is_some());
        let rendered = Renderer::new(MYSQL_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
        assert_eq!(rendered, sql, "round-trip");
    }

    #[test]
    fn load_data_columns_synonym_and_minimal_forms_round_trip() {
        use crate::ast::{LoadDataConcurrency, LoadFieldsSpelling};
        use crate::dialect::MySql;

        // The `COLUMNS` synonym, the `CONCURRENT` lock, `IGNORE` duplicate handling, and a bare
        // minimal form all round-trip; `COLUMNS` preserves its spelling rather than folding to
        // `FIELDS`.
        for sql in [
            "LOAD DATA INFILE 'f' INTO TABLE t",
            "LOAD DATA CONCURRENT INFILE 'f' INTO TABLE db.t",
            "LOAD DATA INFILE 'f' IGNORE INTO TABLE t",
            "LOAD DATA INFILE 'f' INTO TABLE t COLUMNS TERMINATED BY '\\t' ENCLOSED BY '\"'",
            "LOAD DATA INFILE 'f' INTO TABLE t LINES TERMINATED BY '\\n'",
            "LOAD DATA INFILE 'f' INTO TABLE t IGNORE 3 LINES",
            "LOAD DATA INFILE 'f' INTO TABLE t (a, b, @c) SET d = @c, e = DEFAULT",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }
        let concurrent = parse_with(
            "LOAD DATA CONCURRENT INFILE 'f' INTO TABLE db.t",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        assert_eq!(
            load_data_of(&concurrent).concurrency,
            Some(LoadDataConcurrency::Concurrent),
        );
        let columns = parse_with(
            "LOAD DATA INFILE 'f' INTO TABLE t COLUMNS TERMINATED BY '\\t' ENCLOSED BY '\"'",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        assert_eq!(
            load_data_of(&columns).fields.as_ref().unwrap().spelling,
            LoadFieldsSpelling::Columns,
        );
        // A written empty `()` column list folds to absent (both are MySQL's nullptr).
        let empty = parse_with(
            "LOAD DATA INFILE 'f' INTO TABLE t ()",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        assert!(load_data_of(&empty).columns.is_empty());
    }

    #[test]
    fn load_data_dispatch_is_mece_with_load_extension() {
        use crate::dialect::Lenient;

        // Under Lenient both `load_data` and `load_extension` gate the leading `LOAD`; the
        // two-word `LOAD DATA`/`LOAD XML` lookahead routes the bulk-import reading here while a
        // bare `LOAD '<lib>'` stays the extension load.
        assert!(matches!(
            parse_with(
                "LOAD DATA INFILE 'f' INTO TABLE t",
                crate::ParseConfig::new(Lenient)
            )
            .unwrap()
            .statements(),
            [Statement::LoadData { .. }],
        ));
        assert!(matches!(
            parse_with(
                "LOAD XML INFILE 'f' INTO TABLE t",
                crate::ParseConfig::new(Lenient)
            )
            .unwrap()
            .statements(),
            [Statement::LoadData { .. }],
        ));
        assert!(matches!(
            parse_with("LOAD 'plpgsql'", crate::ParseConfig::new(Lenient))
                .unwrap()
                .statements(),
            [Statement::Load { .. }],
        ));
    }

    #[test]
    fn load_data_rejects_malformed_and_out_of_order_clauses() {
        use crate::dialect::MySql;

        // Engine-measured `ER_PARSE_ERROR` (1064) boundaries: the clause train is order-sensitive
        // and the sub-clause-less `FIELDS`/`LINES` and double duplicate keyword are all rejects.
        for sql in [
            // A clause after its grammar slot (charset must precede FIELDS).
            "LOAD DATA INFILE 'f' INTO TABLE t FIELDS TERMINATED BY ',' CHARACTER SET utf8mb4",
            // LINES must follow FIELDS, never precede it.
            "LOAD DATA INFILE 'f' INTO TABLE t LINES TERMINATED BY '\\n' FIELDS TERMINATED BY ','",
            // A bare `FIELDS`/`LINES` with no sub-clause.
            "LOAD DATA INFILE 'f' INTO TABLE t FIELDS",
            "LOAD DATA INFILE 'f' INTO TABLE t LINES",
            // `REPLACE`/`IGNORE` are mutually exclusive.
            "LOAD DATA INFILE 'f' REPLACE IGNORE INTO TABLE t",
            // The lock modifier precedes `LOCAL`, never follows it.
            "LOAD DATA LOCAL LOW_PRIORITY INFILE 'f' INTO TABLE t",
            // `INTO TABLE` is mandatory.
            "LOAD DATA INFILE 'f'",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("{sql:?} must reject"));
        }
    }

    #[test]
    fn load_data_is_gated_off_outside_mysql() {
        use crate::dialect::{Ansi, DuckDb, Postgres, Sqlite};

        // `load_data` is a MySQL/Lenient-only leading-keyword gate; elsewhere `LOAD DATA` is not
        // dispatched to the bulk loader. Under PostgreSQL/DuckDB the leading `LOAD` reaches the
        // extension-load grammar instead, which cannot parse the `DATA … INTO TABLE` tail, so the
        // statement still rejects.
        let sql = "LOAD DATA INFILE 'f' INTO TABLE t";
        for rejects in [
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
        ] {
            assert!(rejects, "{sql:?} is gated off outside MySQL/Lenient");
        }
    }
    /// Every grammar-valid form of the six MySQL server-administration families
    /// (`parse-mysql-server-admin`) round-trips byte-identically: the nullary `SHUTDOWN`/
    /// `RESTART`, both `CLONE` forms with the optional `=`/`DATA DIRECTORY`/`REQUIRE [NO] SSL`
    /// tails, the `IMPORT TABLE` string list, `HELP`'s bare-or-quoted operand, and `BINLOG`'s
    /// base64 string. The same forms are live-oracle-verified grammar-valid in
    /// `corpus_mysql_verdicts::mysql_server_admin_live_oracle_parity`.
    #[test]
    fn server_admin_family_round_trips() {
        use crate::ast::{CloneStatement, HelpStatement};

        // Structural spot-checks: the CLONE INSTANCE donor account, port, and SSL axis.
        let instance = parse_with(
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' REQUIRE NO SSL",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::Clone { clone, .. } = &instance.statements()[0] else {
            panic!("expected a CLONE statement");
        };
        assert!(matches!(
            **clone,
            CloneStatement::Instance {
                ssl: crate::ast::CloneSsl::RequireNo,
                ..
            },
        ));

        // HELP accepts a bare identifier and a quoted string alike (`ident_or_text`).
        let help_bare = parse_with("HELP contents", crate::ParseConfig::new(MYSQL_RENDER)).unwrap();
        let Statement::Help { help, .. } = &help_bare.statements()[0] else {
            panic!("expected a HELP statement");
        };
        let HelpStatement { topic, .. } = &**help;
        assert_eq!(topic.quote, crate::ast::QuoteStyle::None);

        for sql in [
            "SHUTDOWN",
            "RESTART",
            "CLONE LOCAL DATA DIRECTORY 'd'",
            "CLONE LOCAL DATA DIRECTORY = 'd'",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM 'u'@'h':3306 IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM u:3306 IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM CURRENT_USER:3306 IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY 'd'",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY = 'd'",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' REQUIRE SSL",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' REQUIRE NO SSL",
            "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY 'd' REQUIRE SSL",
            "IMPORT TABLE FROM 'f'",
            "IMPORT TABLE FROM 'f', 'g'",
            "HELP 'contents'",
            "HELP contents",
            "BINLOG 'YWJj'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip for {sql:?}");
        }
    }

    #[test]
    fn server_admin_family_reject_edge_cases() {
        // Engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10, both-reject-pinned in
        // `m3::SCHEMA_INDEPENDENT_REJECT`: the nullary keywords take no operand, CLONE LOCAL
        // requires `DATA DIRECTORY`, CLONE INSTANCE requires an abutting `:<port>`, IMPORT
        // TABLE / BINLOG take strings not bare idents, and HELP takes exactly one operand.
        for sql in [
            "SHUTDOWN 1",
            "RESTART 1",
            "CLONE LOCAL 'd'",
            "CLONE LOCAL DATA DIRECTORY",
            "CLONE INSTANCE FROM u@h IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM u@h :3306 IDENTIFIED BY 'p'",
            "CLONE INSTANCE FROM u@h: 3306 IDENTIFIED BY 'p'",
            "IMPORT TABLE FROM f",
            "IMPORT TABLE FROM",
            "HELP",
            "HELP 'a' 'b'",
            "BINLOG garbage",
            "BINLOG",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER)).is_err(),
                "{sql:?} must reject",
            );
        }
    }

    #[test]
    fn server_admin_family_is_gated_off_outside_mysql() {
        use crate::dialect::{Ansi, DuckDb, Lenient, MySql, Postgres, Sqlite};

        // Each leading keyword is gated on for MySQL and the Lenient superset only; elsewhere
        // it is not dispatched and surfaces as an unknown statement. IMPORT TABLE is the one
        // that shares its leading `IMPORT` with DuckDB's `IMPORT DATABASE`, so DuckDB rejecting
        // it confirms the second-keyword split.
        for sql in [
            "SHUTDOWN",
            "RESTART",
            "CLONE LOCAL DATA DIRECTORY 'd'",
            "IMPORT TABLE FROM 'f'",
            "HELP 'x'",
            "BINLOG 'YWJj'",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "{sql:?} parses under MySQL"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
                "{sql:?} parses under Lenient",
            );
            for rejects in [
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            ] {
                assert!(rejects, "{sql:?} is gated off outside MySQL/Lenient");
            }
        }
    }
    /// Every measured-accept replication form parses under the `MySql` preset and renders
    /// byte-identically to its canonical spelling — the full grammar surface the
    /// `replication_statements` gate now covers (`parse-mysql-replication`). The forms are the
    /// live-oracle-verified accept probes; the canonical spelling matches the source for each
    /// (option order, list punctuation, and the `FOR CHANNEL` suffix all preserved).
    #[test]
    fn replication_family_round_trips() {
        // A render-capable MySQL dialect (`crate::dialect::MySql` parses but does not render);
        // the replication gate is already on in `FeatureSet::MYSQL`.
        const REPLICATION_DIALECT: FeatureDialect = {
            const FEATURES: FeatureSet = FeatureSet::MYSQL;
            FeatureDialect {
                features: &FEATURES,
            }
        };

        const FORMS: &[&str] = &[
            // CHANGE REPLICATION SOURCE — value shapes across the option set.
            "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h'",
            "CHANGE REPLICATION SOURCE TO SOURCE_PORT = 3306",
            "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h', SOURCE_PORT = 3306",
            "CHANGE REPLICATION SOURCE TO SOURCE_LOG_FILE = 'f', SOURCE_LOG_POS = 4",
            "CHANGE REPLICATION SOURCE TO RELAY_LOG_FILE = 'r', RELAY_LOG_POS = 8",
            "CHANGE REPLICATION SOURCE TO SOURCE_AUTO_POSITION = 1",
            "CHANGE REPLICATION SOURCE TO SOURCE_HEARTBEAT_PERIOD = 1.5",
            "CHANGE REPLICATION SOURCE TO SOURCE_COMPRESSION_ALGORITHMS = 'zstd'",
            "CHANGE REPLICATION SOURCE TO SOURCE_TLS_CIPHERSUITES = 'x'",
            "CHANGE REPLICATION SOURCE TO SOURCE_TLS_CIPHERSUITES = NULL",
            "CHANGE REPLICATION SOURCE TO IGNORE_SERVER_IDS = (1, 2, 3)",
            "CHANGE REPLICATION SOURCE TO IGNORE_SERVER_IDS = ()",
            "CHANGE REPLICATION SOURCE TO PRIVILEGE_CHECKS_USER = 'u'@'h'",
            "CHANGE REPLICATION SOURCE TO PRIVILEGE_CHECKS_USER = 'u'",
            "CHANGE REPLICATION SOURCE TO PRIVILEGE_CHECKS_USER = NULL",
            "CHANGE REPLICATION SOURCE TO REQUIRE_TABLE_PRIMARY_KEY_CHECK = ON",
            "CHANGE REPLICATION SOURCE TO REQUIRE_TABLE_PRIMARY_KEY_CHECK = GENERATE",
            "CHANGE REPLICATION SOURCE TO ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS = OFF",
            "CHANGE REPLICATION SOURCE TO ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS = LOCAL",
            "CHANGE REPLICATION SOURCE TO ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS = 'uuid'",
            "CHANGE REPLICATION SOURCE TO GTID_ONLY = 1",
            "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h' FOR CHANNEL 'ch'",
            // CHANGE REPLICATION FILTER — every rule shape, empty reset, channel.
            "CHANGE REPLICATION FILTER REPLICATE_DO_DB = (a, b)",
            "CHANGE REPLICATION FILTER REPLICATE_DO_DB = ()",
            "CHANGE REPLICATION FILTER REPLICATE_IGNORE_DB = (a)",
            "CHANGE REPLICATION FILTER REPLICATE_DO_TABLE = (db.t1, db.t2)",
            "CHANGE REPLICATION FILTER REPLICATE_IGNORE_TABLE = (db.t1)",
            "CHANGE REPLICATION FILTER REPLICATE_WILD_DO_TABLE = ('db.%')",
            "CHANGE REPLICATION FILTER REPLICATE_WILD_IGNORE_TABLE = ('db.%')",
            "CHANGE REPLICATION FILTER REPLICATE_REWRITE_DB = ((a, b))",
            "CHANGE REPLICATION FILTER REPLICATE_REWRITE_DB = ((a, b), (c, d))",
            "CHANGE REPLICATION FILTER REPLICATE_REWRITE_DB = ()",
            "CHANGE REPLICATION FILTER REPLICATE_DO_DB = (a), REPLICATE_IGNORE_DB = (b)",
            "CHANGE REPLICATION FILTER REPLICATE_DO_DB = (a) FOR CHANNEL 'ch'",
            // START / STOP REPLICA — threads, UNTIL, connection, channel.
            "START REPLICA",
            "STOP REPLICA",
            "START REPLICA SQL_THREAD",
            "START REPLICA IO_THREAD",
            "START REPLICA RELAY_THREAD",
            "START REPLICA SQL_THREAD, IO_THREAD",
            "STOP REPLICA IO_THREAD, SQL_THREAD",
            "START REPLICA FOR CHANNEL 'ch'",
            "STOP REPLICA SQL_THREAD FOR CHANNEL 'ch'",
            "START REPLICA UNTIL SOURCE_LOG_FILE = 'f', SOURCE_LOG_POS = 4",
            "START REPLICA UNTIL RELAY_LOG_FILE = 'r', RELAY_LOG_POS = 8",
            "START REPLICA UNTIL SQL_BEFORE_GTIDS = 'g'",
            "START REPLICA UNTIL SQL_AFTER_GTIDS = 'g'",
            "START REPLICA UNTIL SQL_AFTER_MTS_GAPS",
            "START REPLICA USER = 'u' PASSWORD = 'p'",
            "START REPLICA PASSWORD = 'p'",
            "START REPLICA USER = 'u' PASSWORD = 'p' DEFAULT_AUTH = 'a' PLUGIN_DIR = 'd'",
            "START REPLICA IO_THREAD UNTIL SOURCE_LOG_FILE = 'f', SOURCE_LOG_POS = 4 \
             USER = 'u' PASSWORD = 'p' FOR CHANNEL 'ch'",
            // GROUP REPLICATION — comma-separated options, both verbs.
            "START GROUP_REPLICATION",
            "STOP GROUP_REPLICATION",
            "START GROUP_REPLICATION USER = 'u'",
            "START GROUP_REPLICATION USER = 'u', PASSWORD = 'p'",
            "START GROUP_REPLICATION USER = 'u', PASSWORD = 'p', DEFAULT_AUTH = 'a'",
        ];

        for sql in FORMS {
            let parsed = parse_with(sql, crate::ParseConfig::new(REPLICATION_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                matches!(parsed.statements(), [Statement::Replication { .. }]),
                "{sql:?} did not parse as a replication statement: {:?}",
                parsed.statements(),
            );
            let rendered = Renderer::new(REPLICATION_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(&rendered, sql, "round-trip mismatch for {sql:?}");
        }
    }

    /// The measured reject boundaries: forms the engine `ER_PARSE_ERROR`s, which the parser
    /// must reject too. Covers the 8.4-removed legacy spellings, the non-empty option/rule
    /// requirement, the doubly-parenthesized rewrite pairs, the schema-qualified filter table,
    /// the `STOP REPLICA` UNTIL/connection ban, the space-separated GROUP option separator,
    /// and the UNTIL GTID-tail restriction.
    #[test]
    fn replication_reject_boundaries() {
        use crate::dialect::MySql;

        const REJECTS: &[&str] = &[
            "CHANGE MASTER TO MASTER_HOST = 'h'",
            "CHANGE REPLICATION SOURCE TO MASTER_HOST = 'h'",
            "CHANGE REPLICATION SOURCE TO SOURCE_COMPRESSION_ALGORITHM = 'zstd'",
            "CHANGE REPLICATION SOURCE TO",
            "CHANGE REPLICATION FILTER",
            "CHANGE REPLICATION FILTER REPLICATE_DO_TABLE = (t1)",
            "CHANGE REPLICATION FILTER REPLICATE_REWRITE_DB = (a, b)",
            "START SLAVE",
            "STOP SLAVE",
            "STOP REPLICA UNTIL SQL_AFTER_MTS_GAPS",
            "STOP REPLICA USER = 'u'",
            "START GROUP_REPLICATION USER = 'u' PASSWORD = 'p'",
            "STOP GROUP_REPLICATION USER = 'u'",
            "START REPLICA UNTIL SQL_AFTER_GTIDS = 'x', SQL_BEFORE_GTIDS = 'y'",
        ];

        for sql in REJECTS {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "{sql:?} should be rejected by the MySql preset",
            );
        }
    }

    /// The replication gate is off outside MySQL/Lenient: the leading sequences surface as
    /// unknown statements (or, for `START`, fall to the transaction dispatcher's mandatory
    /// `TRANSACTION`).
    #[test]
    fn replication_is_gated_off_outside_mysql() {
        use crate::dialect::{DuckDb, Sqlite};

        for sql in [
            "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h'",
            "CHANGE REPLICATION FILTER REPLICATE_DO_DB = (a)",
            "START REPLICA",
            "STOP REPLICA",
            "START GROUP_REPLICATION",
            "STOP GROUP_REPLICATION",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} must be gated off under ANSI",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "{sql:?} must be gated off under SQLite",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "{sql:?} must be gated off under DuckDB",
            );
        }
    }
}
