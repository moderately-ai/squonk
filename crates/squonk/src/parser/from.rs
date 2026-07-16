// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `FROM` relation grammar: qualified names, table factors, and joins.
//!
//! A `FROM` clause is a comma-separated list of table references, each a
//! [`TableFactor`] followed by zero or more joins. The qualified-name helper
//! [`parse_object_name`](Parser::parse_object_name) is shared with the expression
//! grammar in [`super::expr`] (a dotted column reference is the same `a.b.c`
//! production as a dotted table name), keeping name parsing in one place.

use crate::ast::{
    AliasSpelling, ApplyKind, AsOfJoinKind, DerivedSpelling, Expr, Extension, ForceSeekTarget,
    FunctionCall, Ident, IndexHint, IndexHintAction, IndexHintKeyword, IndexHintScope, IndexedBy,
    Join, JoinConstraint, JoinOperator, JsonTable, JsonTableColumn, Keyword, KeywordSet, Meta,
    ObjectName, OnlySyntax, OpenJson, OpenJsonColumn, Query, QuoteStyle, RelationInheritance,
    RowsFromItem, SemiAntiSide, SemiStructuredPathSegment, SetExpr, ShowRef, ShowRefKind,
    ShowRefTarget, Span, Spanned, Statement, TableAlias, TableFactor, TableFunctionColumn,
    TableHint, TableHintKeyword, TableSample, TableVersion, TableWithJoins, XmlNamespace, XmlTable,
    XmlTableColumn, is_unicode_ident, materialize_unicode_ident,
};
use crate::error::ParseResult;
use crate::tokenizer::{LexError, LexErrorKind, Operator, Punctuation, TokenKind};
use std::borrow::Cow;
use thin_vec::{ThinVec, thin_vec};

use super::engine::Parser;
use super::expr::{is_special_function_keyword, special_function_keyword};
use super::{Dialect, HookResult};

/// What a function-alias clause yields: an optional correlation [`TableAlias`] and the
/// typed column-definition list (empty for a bare or untyped alias). Named to keep the
/// two mutually-recursive alias parsers under clippy's type-complexity bar.
type FunctionAliasClause<X> = (Option<Box<TableAlias>>, ThinVec<TableFunctionColumn<X>>);

/// Which side an explicit join names, before its constraint is known.
///
/// `CROSS` is not here: it never carries a [`JoinConstraint`], so it builds its
/// operator directly. `NATURAL` reuses these sides with a `Natural` constraint.
#[derive(Clone, Copy)]
enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinSide {
    /// Build the operator for this side around its parsed constraint.
    ///
    /// `explicit` records whether the redundant side-noise keyword was written — the
    /// `INNER` of `INNER JOIN` for the inner side, the `OUTER` of `LEFT OUTER JOIN` for
    /// an outer side — so a source-fidelity render replays it (fidelity only; the two
    /// spellings are exact synonyms).
    fn into_operator<X: Extension>(
        self,
        explicit: bool,
        constraint: JoinConstraint<X>,
        meta: Meta,
    ) -> JoinOperator<X> {
        match self {
            // A keyword-led `[INNER] JOIN` is never the MySQL `STRAIGHT_JOIN` hint;
            // that spelling builds its `Inner` operator directly in `parse_join`.
            JoinSide::Inner => JoinOperator::Inner {
                straight: false,
                inner: explicit,
                constraint,
                meta,
            },
            JoinSide::Left => JoinOperator::LeftOuter {
                outer: explicit,
                constraint,
                meta,
            },
            JoinSide::Right => JoinOperator::RightOuter {
                outer: explicit,
                constraint,
                meta,
            },
            JoinSide::Full => JoinOperator::FullOuter {
                outer: explicit,
                constraint,
                meta,
            },
        }
    }
}

/// Which of DuckDB's two semi-/anti-join operators a `SEMI`/`ANTI` keyword names.
///
/// These are a `join_type` in DuckDB (mutually exclusive with the [`JoinSide`]s), so
/// the side-less DuckDB spelling never combines with `LEFT`/`RIGHT`/`FULL`; it does
/// compose with the `NATURAL` and `ASOF` ref-types, carried by `constraint` / `asof` at
/// the build site. The Spark/Hive *sided* spelling ([`SemiAntiSide::Left`]/`Right`) is a
/// separate parser arm that builds the operator directly, not through here.
#[derive(Clone, Copy)]
enum SemiAntiKind {
    Semi,
    Anti,
}

impl SemiAntiKind {
    /// Build the side-less (DuckDB) operator around its parsed constraint and `ASOF`
    /// composition flag. The sided (Spark) spelling has its own build site.
    fn into_operator<X: Extension>(
        self,
        asof: bool,
        constraint: JoinConstraint<X>,
        meta: Meta,
    ) -> JoinOperator<X> {
        match self {
            SemiAntiKind::Semi => JoinOperator::Semi {
                asof,
                side: SemiAntiSide::Sideless,
                constraint,
                meta,
            },
            SemiAntiKind::Anti => JoinOperator::Anti {
                asof,
                side: SemiAntiSide::Sideless,
                constraint,
                meta,
            },
        }
    }
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse the `FROM` clause: `<table_ref> [, <table_ref>]*`.
    ///
    /// The caller has confirmed the `FROM` keyword via `peek_is_keyword`; this
    /// consumes it. Comma-separated references are implicit cross products; each
    /// reference is a table factor plus its joins.
    pub(super) fn parse_from(&mut self) -> ParseResult<ThinVec<TableWithJoins<D::Ext>>> {
        self.advance()?; // FROM
        self.parse_table_references()
    }

    /// Parse `<table_ref> [, <table_ref>]*` after a clause keyword has been consumed.
    pub(super) fn parse_table_references(
        &mut self,
    ) -> ParseResult<ThinVec<TableWithJoins<D::Ext>>> {
        let tables = self.parse_comma_separated(Self::parse_table_with_joins)?;
        Ok(tables)
    }

    /// Parse one table reference: a table factor and every join chained onto it.
    ///
    /// `pub(super)` so the `MERGE` grammar in [`super::dml`] can read its `USING`
    /// source (SQL:2016's `<table reference>` — a table, a derived subquery, or a
    /// joined table, but never a comma-separated list) through the one
    /// table-reference entry.
    pub(super) fn parse_table_with_joins(&mut self) -> ParseResult<TableWithJoins<D::Ext>> {
        let start = self.current_span()?;
        let relation = self.parse_table_factor()?;
        let mut joins = ThinVec::new();
        while let Some(join) = self.parse_join()? {
            joins.push(join);
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableWithJoins {
            relation,
            joins,
            meta,
        })
    }

    /// Parse a table factor.
    ///
    /// Parenthesized factors disambiguate by their first inner token: query starts
    /// (`SELECT`, `VALUES`, `WITH`) build derived tables, while other table starts
    /// build nested joined-table factors when the dialect enables them.
    ///
    /// Every base factor — hook-handled or grammar-parsed — then takes the DuckDB
    /// `PIVOT`/`UNPIVOT` suffix chain (`t PIVOT (…) UNPIVOT (…)`), which binds
    /// tighter than joins (a trailing `JOIN` takes the pivoted factor as its left
    /// side; engine-verified on 1.5.4), so the suffix wraps here rather than in
    /// [`parse_table_with_joins`](Self::parse_table_with_joins).
    ///
    /// `pub(super)` so the `MERGE` grammar in [`super::dml`] can read its `USING`
    /// source (a single table or derived subquery) through the one table-factor entry.
    pub(super) fn parse_table_factor(&mut self) -> ParseResult<TableFactor<D::Ext>> {
        if self.peek_starts_prefix_colon_alias()? {
            return self.parse_prefix_colon_aliased_factor();
        }
        let start = self.current_span()?;
        self.parse_table_factor_inner(start)
    }

    /// One table factor — the hook / base grammar / pivot-suffix chain — without the
    /// DuckDB prefix `<alias> :` head, which [`parse_table_factor`](Self::parse_table_factor)
    /// strips first. Called directly (not through `parse_table_factor`) for the factor that
    /// *follows* a prefix alias, so `FROM b : c : a` cannot chain a second prefix.
    fn parse_table_factor_inner(&mut self, start: Span) -> ParseResult<TableFactor<D::Ext>> {
        let factor = match D::parse_table_factor_hook(self) {
            HookResult::Handled(factor) => factor,
            HookResult::NotHandled => self.parse_table_factor_base(start)?,
            HookResult::Err(error) => return Err(error),
        };
        self.parse_pivot_suffixes(start, factor)
    }

    /// Parse DuckDB's `FROM <alias> : <factor>` prefix colon alias: the correlation alias
    /// precedes the relation (`FROM b : a` aliases `a` as `b`; probed on 1.5.4). It folds
    /// onto the factor's ordinary alias slot (DuckDB canonicalizes it to a trailing `AS`),
    /// so the factor must not also carry its own trailing alias — `FROM b : a AS c` /
    /// `FROM b : a c` reject, matching the engine's mutual exclusion.
    fn parse_prefix_colon_aliased_factor(&mut self) -> ParseResult<TableFactor<D::Ext>> {
        // The prefix alias is a bare ColLabel, or under `string_literal_table_names` a
        // single-part Sconst (`FROM '' : t`).
        let name = if self.features().identifier_syntax.string_literal_table_names
            && self.peek_is_name_sconst()?
        {
            self.parse_name_sconst_ident("a prefix table alias")?
        } else {
            self.parse_bare_alias_ident()?
        };
        let alias_meta = self.make_meta(name.span());
        self.expect_punct(Punctuation::Colon, "`:` after a prefix table alias")?;
        let alias = Box::new(TableAlias {
            name,
            columns: ThinVec::new(),
            // The prefix `b :` correlation name folds onto the factor's trailing
            // alias slot, where it renders after the relation — a position the colon
            // form cannot occupy — so it canonicalizes to `AS`, matching DuckDB.
            spelling: AliasSpelling::As,
            meta: alias_meta,
        });
        let factor_start = self.current_span()?;
        let mut factor = self.parse_table_factor_inner(factor_start)?;
        match factor.alias_slot_mut() {
            Some(slot) if slot.is_some() => {
                return Err(
                    self.unexpected("a relation without a trailing alias after a prefix `:` alias")
                );
            }
            Some(slot) => *slot = Some(alias),
            None => {
                return Err(self.unexpected("a relation that can carry a prefix `:` alias"));
            }
        }
        Ok(factor)
    }

    /// One base table factor, without the pivot suffix chain.
    fn parse_table_factor_base(&mut self, start: Span) -> ParseResult<TableFactor<D::Ext>> {
        let lateral = if self.peek_is_keyword(Keyword::Lateral)? {
            if !self.features().table_factor_syntax.lateral {
                return Err(self.unexpected("a table expression supported by this dialect"));
            }
            self.advance()?;
            true
        } else {
            false
        };

        if self.peek_is_keyword(Keyword::Rows)? && self.peek_nth_is_keyword(1, Keyword::From)? {
            return self.parse_rows_from_factor(start, lateral);
        }
        // `UNNEST(...)` — the first-class array-expansion table factor. Reached only when
        // `UNNEST` is immediately followed by `(` and the dialect enables it; a bare
        // `UNNEST` (no parens) is left to the named-table path as an ordinary relation
        // name. Checked before the named-factor path, which would otherwise read the same
        // `UNNEST(...)` as a generic `TableFactor::Function`.
        if self.features().table_factor_syntax.unnest
            && self.peek_is_keyword(Keyword::Unnest)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return self.parse_unnest_table_factor(start, lateral);
        }
        // `JSON_TABLE(...)` / `XMLTABLE(...)` — the SQL/JSON and SQL/XML column-defining
        // table factors. Reached only when the keyword is immediately followed by `(` and the
        // dialect enables it; a bare `JSON_TABLE`/`XMLTABLE` (no parens) is left to the
        // named-table path as an ordinary relation name. Checked before that path, which would
        // otherwise read the same `(` as a generic `TableFactor::Function` call and reject the
        // `COLUMNS`/`PASSING` clause tail differently.
        if self.features().table_factor_syntax.json_table
            && self.peek_is_keyword(Keyword::JsonTable)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return self.parse_json_table_factor(start, lateral);
        }
        if self.features().table_factor_syntax.xml_table
            && self.peek_is_keyword(Keyword::Xmltable)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return self.parse_xml_table_factor(start, lateral);
        }
        // SQL Server's `OPENJSON(<json> [, <path>]) [WITH (…)]` rowset-function table factor.
        // Reached only when `OPENJSON` is immediately followed by `(` and the dialect enables
        // it; a bare `OPENJSON` (no parens) is left to the named-table path as an ordinary
        // relation name (the keyword is unreserved). Checked before that path, which would
        // otherwise read the `(` as a generic `TableFactor::Function` call and reject the
        // `WITH (…)` clause tail differently.
        if self.features().table_factor_syntax.open_json
            && self.peek_is_keyword(Keyword::Openjson)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return self.parse_open_json_table_factor(start, lateral);
        }
        // Snowflake/Oracle's `TABLE(<expr>)` first-class table-expression factor.
        // Reached only when `TABLE` is immediately followed by `(` and the dialect
        // enables it; a bare `TABLE` (no parens) falls through to the named-table path,
        // where the reserved keyword is not an admissible relation name and the
        // construct is a clean parse error — the standalone `TABLE t` query-body form
        // (`parse_table_command`) is a different, statement-position grammar entry
        // entirely and is never reached from here.
        if self.features().table_factor_syntax.table_expr_factor
            && self.peek_is_keyword(Keyword::Table)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return self.parse_table_expr_factor(start, lateral);
        }
        // DuckDB's bare `FROM VALUES (…) AS t` row list: `VALUES` opens a table factor
        // directly, without the parentheses the standard derived table requires. Checked
        // before the named-factor path (which rejects `VALUES` as a table name) and only
        // under the gate, so `VALUES` stays a clean reject elsewhere. The parenthesized
        // `FROM (VALUES …)` derived table is a different, always-on path below.
        if self.features().table_factor_syntax.from_values
            && self.peek_is_keyword(Keyword::Values)?
        {
            return self.parse_from_values_factor(start, lateral);
        }
        if self.peek_is_punct(Punctuation::LParen)? {
            return self.parse_parenthesized_table_factor(start, lateral);
        }
        if let Some(TokenKind::Keyword(keyword)) = self.peek()?.map(|token| token.kind) {
            // `CURRENT_SCHEMA` is also an ordinary (`type_func_name`) function name
            // (mirrors `Parser::parse_special_function`'s expression-position
            // carve-out): a call form `current_schema(...)` defers to the generic
            // function-call path below, since only the bare keyword is the special
            // value function.
            let is_current_schema_call = keyword == Keyword::CurrentSchema
                && self.peek_nth_is_punct(1, Punctuation::LParen)?;
            // MySQL has no PostgreSQL `func_table` promotion: a bare `current_date`/
            // `current_timestamp` in table position is a reserved-word syntax error, not a
            // special value function, so under the gate it falls through to the named-table
            // path where the reserved-word check rejects it (as the alias position does).
            if self
                .features()
                .table_factor_syntax
                .special_function_table_source
                && is_special_function_keyword(keyword)
                && !is_current_schema_call
            {
                return self.parse_special_function_table_factor(start, lateral, keyword);
            }
        }

        self.parse_named_table_factor(start, lateral)
    }

    /// Parse a bare SQL special value function as a table reference (PostgreSQL
    /// `func_table: func_expr_windowless`, e.g. `SELECT * FROM current_date`):
    /// `pg_query` lowers this to a `RangeFunction` wrapping a `SQLValueFunction`,
    /// so it is not an ordinary [`TableFactor::Function`] call (which wraps a
    /// [`FunctionCall`] — a name plus a parenthesized argument list this
    /// construct has neither of) but the dedicated `TableFactor::SpecialFunction`
    /// mirroring `Expr::SpecialFunction`, the same grammar production in
    /// expression position.
    ///
    /// `keyword` is the already-peeked current token; `LATERAL` is rejected here
    /// as it is for a plain table name — the construct takes no arguments, so
    /// there is nothing for `LATERAL` to correlate against.
    fn parse_special_function_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
        keyword: Keyword,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if lateral {
            return Err(self.unexpected("a table function call after `LATERAL`"));
        }
        self.advance()?; // the special-function keyword
        let (sf_keyword, takes_precision) = special_function_keyword(keyword);
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
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::SpecialFunction {
            keyword: sf_keyword,
            precision,
            alias,
            meta,
        })
    }

    fn parse_parenthesized_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        // Recursion-guarded (ADR-0012): a parenthesized factor is the one query/table
        // nesting that is *not* a query — a nested join tree `((t JOIN u))` recurses
        // through `parse_table_with_joins` without ever touching `parse_query`, so it
        // needs its own guard here. A derived subquery `(SELECT …)` re-enters
        // `parse_query` too and so counts at both points. The blamed location is the
        // `(` at the cursor (not `start`, which may sit on a preceding `LATERAL`).
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard
            .parser()
            .parse_parenthesized_table_factor_inner(start, lateral)
    }

    /// Parse a parenthesized table factor, one level deep under the recursion guard.
    ///
    /// A `(` in `FROM` position opens one of three things (PostgreSQL `table_ref`):
    ///
    /// - a parenthesized **query expression** — a derived table `(SELECT …)`, or a set
    ///   operation whose operands may themselves be parenthesized, `((SELECT …) UNION
    ///   …)` (PostgreSQL `select_with_parens`; the canonical set-op AST);
    /// - a parenthesized **joined table** `(t JOIN u …)` (PostgreSQL `'('
    ///   joined_table ')'`).
    ///
    /// A leading `SELECT`/`VALUES`/`WITH` is unambiguously the query reading. A leading
    /// `(` is the genuinely ambiguous case: `((SELECT …) UNION …)` is a query, while
    /// `((t JOIN u))` and `((SELECT …) x JOIN …)` are joined tables — the two diverge
    /// only *after* the inner parenthesized group, so no bounded prefix tells them
    /// apart. Decide it by speculatively reading the set-op-aware query grammar
    /// (backtracking, reusing the `query` set-op climb rather than re-deriving
    /// precedence): the group is the query reading exactly when that parse consumes it
    /// whole — the closing `)` immediately follows — otherwise the cursor rewinds and
    /// the joined-table path re-reads it as a table reference.
    fn parse_parenthesized_table_factor_inner(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.advance()?; // `(`
        // Try the parenthesized PIVOT/UNPIVOT statement first — a bare `(PIVOT t ON …)`
        // or its WITH-prefixed form `(WITH c AS (…) PIVOT c ON …)`. This must precede
        // the query path: the query grammar now admits PIVOT/UNPIVOT as a query body
        // (`SetExpr::Pivot`, for the CTE/VIEW/CTAS positions), so a with-prefixed pivot
        // here would otherwise be swallowed as a derived table rather than the
        // statement-spelled `TableFactor::Pivot` the FROM position keeps. DuckDB
        // desugars both to the same `SUBQUERY` (`SELECT * FROM <pivot>`), so the factor
        // spelling is our canonical FROM shape either way (ADR-0011); keeping it stable
        // preserves the pivot landing's structural mapping and allowlist. On a non-match
        // the helper leaves the cursor unspecified, so rewind before falling through.
        if self.features().table_factor_syntax.pivot || self.features().table_factor_syntax.unpivot
        {
            let checkpoint = self.checkpoint();
            if let Some(factor) = self.try_parenthesized_pivot_factor(start, lateral)? {
                return Ok(factor);
            }
            self.rewind(checkpoint);
        }
        // A leading `DESCRIBE`/`SHOW`/`SUMMARIZE` keyword opens DuckDB's `SHOW_REF`
        // table source (`FROM (DESCRIBE SELECT …)`, `FROM (SHOW databases)`); these are
        // never query starts, so they are read here rather than through the query path.
        if self.features().table_factor_syntax.show_ref {
            if let Some(factor) = self.try_parenthesized_show_ref(start, lateral)? {
                return Ok(factor);
            }
        }
        if self.peek_starts_query()? {
            // A derived subquery `(SELECT …)` or set operation — pivot/unpivot were
            // already claimed above, so a failure here is a genuine query error.
            let subquery = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the subquery")?;
            return self.finish_derived_table_factor(start, lateral, subquery);
        }
        if self.peek_is_punct(Punctuation::LParen)? {
            let checkpoint = self.checkpoint();
            if let Some(factor) = self.try_parenthesized_query_factor(start, lateral)? {
                return Ok(factor);
            }
            self.rewind(checkpoint);
        }

        if lateral {
            return Err(self.unexpected("a query after `LATERAL (`"));
        }
        if !self.features().table_expressions.parenthesized_joins {
            return Err(self.unexpected("a table expression supported by this dialect"));
        }
        let table = self.parse_table_with_joins()?;
        let collapse_redundant_join_parens = table.joins.is_empty()
            && matches!(&table.relation, TableFactor::NestedJoin { alias: None, .. });
        if table.joins.is_empty() && !collapse_redundant_join_parens {
            return Err(self.unexpected("a joined table inside parentheses"));
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the joined table")?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        // MySQL admits a parenthesized join but rejects an alias on it (`(a CROSS JOIN b) AS
        // x` is a syntax error there, while a derived-table `(SELECT …) AS x` — a different,
        // always-aliasable path — parses). A derived subquery never reaches this branch.
        if alias.is_some() && !self.features().table_expressions.aliased_parenthesized_join {
            return Err(self.error_at(
                span,
                "no correlation alias on a parenthesized joined table",
                self.span_text(span).to_owned(),
            ));
        }
        let meta = self.make_meta(span);
        if collapse_redundant_join_parens {
            let TableFactor::NestedJoin {
                table: inner_table,
                alias: None,
                ..
            } = table.relation
            else {
                unreachable!("collapse_redundant_join_parens matched a nested join without alias");
            };
            return Ok(TableFactor::NestedJoin {
                table: inner_table,
                alias,
                meta,
            });
        }
        Ok(TableFactor::NestedJoin {
            table: Box::new(table),
            alias,
            meta,
        })
    }

    /// Speculatively read a `(`-opening parenthesized `FROM` group — one whose first
    /// inner token is itself `(` — as a query expression (PostgreSQL
    /// `select_with_parens` with a parenthesized leading operand, e.g. `((SELECT …)
    /// UNION …)`).
    ///
    /// Returns `Some` derived table when the set-op-aware query grammar consumes the
    /// whole group (a closing `)` immediately follows the parsed query), and `None` —
    /// leaving the cursor for the caller to rewind — when the content is instead a
    /// joined table whose first factor merely begins with `(` (`((t JOIN u))`,
    /// `((SELECT …) x JOIN …)`). A query parse *error* is likewise a non-match: under
    /// fail-fast the reported error is inert (only the returned `Result` drives control
    /// flow), so the rewound joined-table path surfaces the real diagnostic.
    /// Once the `)` confirms the query reading, a trailing-alias error is committed.
    fn try_parenthesized_query_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<Option<TableFactor<D::Ext>>> {
        // Arm the grouping context: the inner leading `(` is a FROM table-or-subquery
        // grouping (`FROM ((SELECT 1))`), a complete standalone primary SQLite accepts
        // with `parenthesized_query_operands` off — distinct from a bare compound operand.
        self.set_paren_query_grouping(true);
        let Ok(subquery) = self.parse_query() else {
            return Ok(None);
        };
        if !self.peek_is_punct(Punctuation::RParen)? {
            return Ok(None);
        }
        self.advance()?; // `)`
        Ok(Some(
            self.finish_derived_table_factor(start, lateral, subquery)?,
        ))
    }

    /// Build a derived-table factor from an already-parsed parenthesized `subquery`
    /// (its closing `)` consumed), reading the optional correlation alias that trails.
    fn finish_derived_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
        subquery: Query<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Derived {
            lateral,
            subquery: Box::new(subquery),
            alias,
            spelling: DerivedSpelling::Parenthesized,
            meta,
        })
    }

    /// Parse DuckDB's bare `FROM VALUES (<row>, …) [AS] <alias>` row-list table factor —
    /// a `VALUES` constructor standing directly as a table factor without the wrapping
    /// parentheses the standard derived table requires. DuckDB parse-requires a table
    /// alias here (a bare `FROM VALUES (1)` is a syntax error; `FROM VALUES (1) t`
    /// accepts — probed on 1.5.4), so a missing alias is rejected rather than left an
    /// unaliased relation; that reject also keeps us from over-accepting relative to the
    /// engine. The body is wrapped in the shared [`TableFactor::Derived`] node tagged
    /// [`DerivedSpelling::BareValues`] (one derived-table shape, the paren
    /// spelling kept as data), reusing [`parse_values`](Self::parse_values) — so the row
    /// grammar and the ragged-row reject come for free — and only the query-tail clauses
    /// (`ORDER BY`/`LIMIT`) stay outside, belonging to the enclosing query, not the
    /// constant row list (`FROM VALUES (1) t ORDER BY 1` sorts the outer query; probed).
    /// Only reached under [`TableFactorSyntax::from_values`](crate::ast::dialect::TableExpressionSyntax).
    fn parse_from_values_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if lateral {
            // A constant `VALUES` row list has nothing for `LATERAL` to correlate
            // against (mirrors the special-function / `ROWS FROM` LATERAL rejects).
            return Err(self.unexpected("a table function call after `LATERAL`"));
        }
        let values = self.parse_values()?;
        let body_span = values.span();
        let body = SetExpr::Values {
            values: Box::new(values),
            meta: self.make_meta(body_span),
        };
        let subquery = Query {
            with: None,
            body,
            order_by: ThinVec::new(),
            order_by_all: None,
            limit_by: None,
            limit: None,
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: self.make_meta(body_span),
        };
        let Some(alias) = self.parse_optional_table_alias()? else {
            return Err(self.unexpected("a table alias after a bare `FROM VALUES` row list"));
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Derived {
            lateral: false,
            subquery: Box::new(subquery),
            alias: Some(alias),
            spelling: DerivedSpelling::BareValues,
            meta,
        })
    }

    /// Read a parenthesized DuckDB `DESCRIBE`/`SHOW`/`SUMMARIZE` utility as a
    /// [`TableFactor::ShowRef`] — DuckDB's `SHOW_REF` table source
    /// (`FROM (DESCRIBE SELECT …)`, `FROM (SHOW databases)`) — with the cursor just
    /// after the opening `(`. Returns `None` (cursor unmoved) when the content is not
    /// one of these keywords, so the caller falls through to the joined-table path.
    /// Only reached under [`TableFactorSyntax::show_ref`](crate::ast::dialect::TableExpressionSyntax).
    pub(super) fn try_parenthesized_show_ref(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<Option<TableFactor<D::Ext>>> {
        let inner_start = self.current_span()?;
        // `SHOW` names its target; `DESCRIBE`/`DESC`/`SUMMARIZE` take a query *or* a table
        // name. This statement-leading position is disjoint from `DESC` sort direction.
        let kind = if self.peek_is_contextual_keyword("DESCRIBE")? {
            ShowRefKind::Describe
        } else if self.peek_is_contextual_keyword("DESC")? {
            ShowRefKind::Desc
        } else if self.peek_is_contextual_keyword("SUMMARIZE")? {
            ShowRefKind::Summarize
        } else if self.peek_is_contextual_keyword("SHOW")? {
            ShowRefKind::Show
        } else {
            return Ok(None);
        };
        if lateral {
            // The utility source has nothing for LATERAL to correlate against
            // (mirrors the parenthesized-pivot reject).
            return Err(self.unexpected("a query after `LATERAL (`"));
        }
        self.advance()?; // the DESCRIBE / DESC / SHOW / SUMMARIZE keyword
        let target = self.parse_show_ref_target(kind)?;
        let show_meta = self.make_meta(inner_start.union(self.preceding_span()));
        self.expect_punct(Punctuation::RParen, "`)` to close the show statement")?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(TableFactor::ShowRef {
            show: Box::new(ShowRef {
                kind,
                target,
                meta: show_meta,
            }),
            alias,
            meta,
        }))
    }

    /// Parse DuckDB's leading-keyword `{DESCRIBE | SUMMARIZE} <query> | <table>`
    /// introspection statement into [`Statement::ShowRef`] — the `SHOW_REF` utility at
    /// statement position, reusing the same [`ShowRef`] core (kind + target) as the
    /// parenthesized-`FROM` table factor [`try_parenthesized_show_ref`](Self::try_parenthesized_show_ref).
    /// DuckDB desugars the statement to `SELECT * FROM (SHOW_REF …)`. Only reached under
    /// [`ShowSyntax::describe_summarize`](crate::ast::dialect::UtilitySyntax); the caller
    /// has confirmed the leading keyword is `DESCRIBE`, `DESC`, or `SUMMARIZE`.
    pub(super) fn parse_describe_summarize_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let kind = if self.peek_is_contextual_keyword("DESCRIBE")? {
            ShowRefKind::Describe
        } else if self.peek_is_contextual_keyword("DESC")? {
            ShowRefKind::Desc
        } else {
            ShowRefKind::Summarize
        };
        self.advance()?; // the DESCRIBE / DESC / SUMMARIZE keyword
        let target = if matches!(kind, ShowRefKind::Describe | ShowRefKind::Desc)
            && (self.is_eof()? || self.peek_is_punct(Punctuation::Semicolon)?)
        {
            ShowRefTarget::Empty {
                meta: self.make_meta(self.preceding_span()),
            }
        } else {
            self.parse_show_ref_target(kind)?
        };
        let span = start.union(self.preceding_span());
        Ok(Statement::ShowRef {
            show: Box::new(ShowRef {
                kind,
                target,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a [`ShowRef`]'s target: `DESCRIBE`/`SUMMARIZE` describe a nested query
    /// when a query head follows (`DESCRIBE SELECT …`, `DESCRIBE PIVOT …`), otherwise a
    /// table name (`DESCRIBE t`); `SHOW` always names its target (`SHOW databases`).
    pub(super) fn parse_show_ref_target(
        &mut self,
        kind: ShowRefKind,
    ) -> ParseResult<ShowRefTarget<D::Ext>> {
        let start = self.current_span()?;
        let is_query = !matches!(kind, ShowRefKind::Show) && self.peek_starts_show_ref_query()?;
        if is_query {
            let query = self.parse_query()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ShowRefTarget::Query {
                query: Box::new(query),
                meta,
            })
        } else {
            // DuckDB admits a single-part Sconst table name after DESCRIBE/SUMMARIZE
            // (`DESCRIBE e'0e'`, `SUMMARIZE 't'`; engine-measured on libduckdb 1.5.4).
            let name = if self.features().identifier_syntax.string_literal_table_names
                && self.peek_is_name_sconst()?
            {
                ObjectName(thin_vec![self.parse_name_sconst_ident("a table name")?])
            } else {
                self.parse_object_name()?
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ShowRefTarget::Name { name, meta })
        }
    }

    /// True when the token after `DESCRIBE`/`SUMMARIZE` opens a describable query — a
    /// query primary ([`peek_starts_query`](Self::peek_starts_query)) or a leading
    /// `PIVOT`/`UNPIVOT` operator (which `parse_query` now admits as a query body).
    fn peek_starts_show_ref_query(&mut self) -> ParseResult<bool> {
        Ok(self.peek_starts_query()?
            || (self.features().table_factor_syntax.pivot
                && self.peek_is_keyword(Keyword::Pivot)?)
            || (self.features().table_factor_syntax.unpivot
                && self.peek_is_keyword(Keyword::Unpivot)?))
    }

    fn parse_rows_from_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if !self.features().table_factor_syntax.rows_from {
            return Err(self.unexpected("a table expression supported by this dialect"));
        }
        self.expect_keyword(Keyword::Rows)?;
        self.expect_keyword(Keyword::From)?;
        self.expect_punct(Punctuation::LParen, "`(` after `ROWS FROM`")?;
        let functions = self.parse_comma_separated(Self::parse_rows_from_item)?;
        self.expect_punct(Punctuation::RParen, "`)` to close `ROWS FROM`")?;
        let with_ordinality = self.parse_with_ordinality()?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::RowsFrom {
            lateral,
            functions,
            with_ordinality,
            alias,
            meta,
        })
    }

    /// Parse the first-class `UNNEST(<expr>[, <expr>…])` table factor, cursor on the
    /// `UNNEST` keyword (its following `(` already confirmed by the caller). Reads, in
    /// order: the parenthesized array-expression list; a PostgreSQL/DuckDB `WITH
    /// ORDINALITY` (before the alias); the correlation-alias clause (untyped column list
    /// or, for the rare PostgreSQL typed spelling, a column-definition list); and finally
    /// a BigQuery `WITH OFFSET [AS <alias>]` tail (after the alias, gated by
    /// [`TableFactorSyntax::unnest_with_offset`](crate::ast::dialect::TableFactorSyntax)). The `WITH ORDINALITY`/`WITH OFFSET`
    /// split around the alias is what lets both the PostgreSQL (`… WITH ORDINALITY AS
    /// u(…)`) and BigQuery (`… AS u WITH OFFSET`) orderings round-trip through one node.
    fn parse_unnest_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::Unnest)?;
        self.expect_punct(Punctuation::LParen, "`(` after `UNNEST`")?;
        // PostgreSQL admits a multi-array `unnest(a, b)` and even a degenerate `unnest()`;
        // the empty and multi-arg forms both flow through the same expression list.
        // Arity is deliberately NOT gated here: DuckDB/BigQuery accept `unnest(a, b)`
        // grammatically and reject it only at BIND (function-catalog resolution — DuckDB's
        // signature is `unnest(ANY)`, a single positional array, so it raises a *Binder
        // Error*, not a parse error; PostgreSQL binds and zips). A parse-only validator does
        // not own function-arity resolution, so the multi-array over-acceptance under the
        // DuckDb preset is a tolerated bind-layer residual, not a grammar boundary — same
        // class as the window/aggregate function-arity residuals
        // (`duckdb-unnest-multiarg-over-accept`, `duckdb-window-aggregate-validation-over-accepts`).
        let array_exprs = if self.peek_is_punct(Punctuation::RParen)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated_exprs()?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `UNNEST`")?;
        // `WITH ORDINALITY` binds before the alias. Guard entry so a BigQuery `WITH OFFSET`
        // (which follows the alias, or stands here when there is no alias) is not misread as
        // an `ORDINALITY` and errored on — it is left for the offset tail below.
        let with_ordinality = if self.peek_is_keyword(Keyword::With)?
            && self.peek_nth_is_contextual_keyword(1, "ORDINALITY")?
        {
            self.parse_with_ordinality()?
        } else {
            false
        };
        let (alias, column_defs) = self.parse_function_alias_clause()?;
        let (with_offset, with_offset_alias) = self.parse_unnest_offset_tail()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Unnest {
            lateral,
            array_exprs,
            with_ordinality,
            alias,
            column_defs,
            with_offset,
            with_offset_alias,
            meta,
        })
    }

    /// Parse the BigQuery/ZetaSQL `WITH OFFSET [[AS] <alias>]` tail of an `UNNEST` factor.
    /// Returns `(false, None)` — cursor unmoved — when no `WITH OFFSET` follows, or when
    /// [`TableFactorSyntax::unnest_with_offset`](crate::ast::dialect::TableFactorSyntax) is off: leaving the tail unconsumed
    /// makes it surface as a parse error, matching PostgreSQL/DuckDB, which both
    /// parse-reject `WITH OFFSET` (engine-probed).
    fn parse_unnest_offset_tail(&mut self) -> ParseResult<(bool, Option<Ident>)> {
        if !(self.peek_is_keyword(Keyword::With)?
            && self.peek_nth_is_contextual_keyword(1, "OFFSET")?)
        {
            return Ok((false, None));
        }
        if !self.features().table_factor_syntax.unnest_with_offset {
            return Ok((false, None));
        }
        self.advance()?; // WITH
        self.advance()?; // OFFSET
        // `[AS] <alias>`: the `AS` is optional (BigQuery). After an explicit `AS` an
        // identifier is required; without it, a bare identifier that can start a column
        // name is the offset alias, while a reserved continuation keyword (`JOIN`,
        // `WHERE`, …) declines and leaves the tail bare.
        let explicit_as = self.eat_keyword(Keyword::As)?;
        let with_offset_alias = if explicit_as
            || self
                .peek()?
                .is_some_and(|token| self.token_can_be_column_name(token))
        {
            Some(self.parse_ident()?)
        } else {
            None
        };
        Ok((true, with_offset_alias))
    }

    /// Parse the SQL/JSON `JSON_TABLE(…)` table factor, cursor on `JSON_TABLE` (its `(`
    /// confirmed by the caller). The context/`PASSING`/behaviour clauses reuse [`super::expr`]'s
    /// SQL/JSON parsers; the row and column paths are string literals (see
    /// [`parse_json_table_string_path`](Self::parse_json_table_string_path)).
    fn parse_json_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::JsonTable)?;
        self.expect_punct(Punctuation::LParen, "`(` after `JSON_TABLE`")?;
        let context = self.parse_json_value_expr()?;
        self.expect_punct(Punctuation::Comma, "`,` before the JSON_TABLE path")?;
        let path = self.parse_json_table_string_path()?;
        // The optional `AS <name>` on the row path is a `ColId` (PostgreSQL's `name`): `value`
        // accepts (a `col_name` keyword), the reserved `select` rejects — engine-verified.
        // Distinct from a `PASSING` argument name, which is the wider `ColLabel`.
        let path_name = if self.eat_keyword(Keyword::As)? {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let passing = self.parse_json_passing_opt()?;
        self.expect_keyword(Keyword::Columns)?;
        self.expect_punct(Punctuation::LParen, "`(` after `COLUMNS`")?;
        // Non-empty: `COLUMNS ()` rejects, so `parse_comma_separated` (one-or-more) is exact.
        let columns = self.parse_comma_separated(Self::parse_json_table_column)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the JSON_TABLE columns")?;
        // The top-level handler is `ON ERROR` only (a top-level `ON EMPTY` rejects); the whole
        // behaviour set is admitted (engine-verified).
        let on_error = self.parse_json_on_behavior(Keyword::Error)?;
        self.expect_punct(Punctuation::RParen, "`)` to close `JSON_TABLE`")?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::JsonTable {
            json_table: Box::new(JsonTable {
                lateral,
                context,
                path,
                path_name,
                passing,
                columns,
                on_error,
                meta,
            }),
            alias,
            meta,
        })
    }

    /// Parse one `JSON_TABLE` column definition, recursing for `NESTED`. Each column kind
    /// admits only its own clause tail (PostgreSQL's per-`coltype` legality).
    fn parse_json_table_column(&mut self) -> ParseResult<JsonTableColumn<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Nested)? {
            // `NESTED [PATH] <string> [AS <name>] COLUMNS ( … )` — the `PATH` keyword is
            // optional, the nested `COLUMNS` list is mandatory and non-empty.
            let _ = self.eat_keyword(Keyword::Path)?;
            let path = self.parse_json_table_string_path()?;
            // The `AS <name>` is a `ColId`, like the top-level row-path name.
            let path_name = if self.eat_keyword(Keyword::As)? {
                Some(self.parse_ident()?)
            } else {
                None
            };
            self.expect_keyword(Keyword::Columns)?;
            self.expect_punct(Punctuation::LParen, "`(` after nested `COLUMNS`")?;
            let columns = self.parse_comma_separated(Self::parse_json_table_column)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the nested columns")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(JsonTableColumn::Nested {
                path,
                path_name,
                columns,
                meta,
            });
        }
        let name = self.parse_ident()?;
        if self.eat_keyword(Keyword::For)? {
            self.expect_keyword(Keyword::Ordinality)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(JsonTableColumn::ForOrdinality { name, meta });
        }
        let data_type = Box::new(self.parse_data_type()?);
        if self.eat_keyword(Keyword::Exists)? {
            // `EXISTS [PATH <string>] [<behaviour> ON ERROR]` — no FORMAT/wrapper/quotes/ON
            // EMPTY (each rejects, engine-verified).
            let path = if self.eat_keyword(Keyword::Path)? {
                Some(self.parse_json_table_string_path()?)
            } else {
                None
            };
            let on_error = self.parse_json_on_behavior(Keyword::Error)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(JsonTableColumn::Exists {
                name,
                data_type,
                path,
                on_error,
                meta,
            });
        }
        // Regular value column: PostgreSQL's fixed clause order is `[FORMAT] [PATH] [wrapper]
        // [quotes] [ON EMPTY] [ON ERROR]`; an out-of-order clause is left unconsumed and
        // surfaces as a clean parse error (e.g. `WITH WRAPPER PATH '$'` rejects at `PATH`).
        let format = self.parse_json_format()?;
        let path = if self.eat_keyword(Keyword::Path)? {
            Some(self.parse_json_table_string_path()?)
        } else {
            None
        };
        let wrapper = self.parse_json_wrapper()?;
        let quotes = self.parse_json_quotes()?;
        let on_empty = self.parse_json_on_behavior(Keyword::Empty)?;
        let on_error = self.parse_json_on_behavior(Keyword::Error)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(JsonTableColumn::Regular {
            name,
            data_type,
            format,
            path,
            wrapper,
            quotes,
            on_empty,
            on_error,
            meta,
        })
    }

    /// Parse a JSON_TABLE path — a string literal only. PostgreSQL restricts the row path,
    /// column `PATH`, and `NESTED PATH` to string constants ("only string constants are
    /// supported in JSON_TABLE path specification"), so a non-string token is a parse error.
    fn parse_json_table_string_path(&mut self) -> ParseResult<Box<Expr<D::Ext>>> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a string-literal JSON_TABLE path"));
        };
        if token.kind != TokenKind::String {
            return Err(self.unexpected("a string-literal JSON_TABLE path"));
        }
        Ok(Box::new(self.parse_string_literal(token)?))
    }

    /// Parse the SQL/XML `XMLTABLE(…)` table factor, cursor on `XMLTABLE` (its `(` confirmed by
    /// the caller). The row and document expressions are `c_expr`; the `PASSING` mechanism
    /// reuses [`super::expr`]'s `parse_xml_passing_mechanism`.
    fn parse_xml_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::Xmltable)?;
        self.expect_punct(Punctuation::LParen, "`(` after `XMLTABLE`")?;
        // The optional `XMLNAMESPACES( … ),` prefix — its head is the keyword immediately
        // followed by `(`; anything else is the start of the row expression.
        let namespaces = if self.peek_is_keyword(Keyword::Xmlnamespaces)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            self.advance()?; // XMLNAMESPACES
            self.expect_punct(Punctuation::LParen, "`(` after `XMLNAMESPACES`")?;
            let namespaces = self.parse_comma_separated(Self::parse_xml_namespace)?;
            self.expect_punct(Punctuation::RParen, "`)` to close `XMLNAMESPACES`")?;
            self.expect_punct(Punctuation::Comma, "`,` after the `XMLNAMESPACES` clause")?;
            namespaces
        } else {
            ThinVec::new()
        };
        let row_expr = Box::new(self.parse_c_expr()?);
        self.expect_keyword(Keyword::Passing)?;
        let passing_mechanism_before = self.parse_xml_passing_mechanism()?;
        let document = Box::new(self.parse_c_expr()?);
        let passing_mechanism_after = self.parse_xml_passing_mechanism()?;
        self.expect_keyword(Keyword::Columns)?;
        // XMLTABLE's `COLUMNS` list is *unparenthesized* (unlike JSON_TABLE's), non-empty.
        let columns = self.parse_comma_separated(Self::parse_xml_table_column)?;
        self.expect_punct(Punctuation::RParen, "`)` to close `XMLTABLE`")?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::XmlTable {
            xml_table: Box::new(XmlTable {
                lateral,
                namespaces,
                row_expr,
                document,
                passing_mechanism_before,
                passing_mechanism_after,
                columns,
                meta,
            }),
            alias,
            meta,
        })
    }

    /// Parse one `XMLNAMESPACES` entry: `<uri> AS <name>` or `DEFAULT <uri>` (the unnamed
    /// default namespace). The URI is a full `a_expr`.
    fn parse_xml_namespace(&mut self) -> ParseResult<XmlNamespace<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Default)? {
            let uri = Box::new(self.parse_expr()?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(XmlNamespace {
                uri,
                name: None,
                meta,
            });
        }
        let uri = Box::new(self.parse_expr()?);
        self.expect_keyword(Keyword::As)?;
        let name = Some(self.parse_as_alias_ident()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(XmlNamespace { uri, name, meta })
    }

    /// Parse one `XMLTABLE` column definition (PostgreSQL's `RangeTableFuncCol`): a bare `FOR
    /// ORDINALITY` column, or a regular column whose `PATH`/`DEFAULT`/`NULL`/`NOT NULL`
    /// options are order-free. PostgreSQL rejects a repeated `PATH`/`DEFAULT` and a
    /// conflicting/redundant nullability declaration at parse, which the loop reproduces.
    fn parse_xml_table_column(&mut self) -> ParseResult<XmlTableColumn<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        if self.eat_keyword(Keyword::For)? {
            self.expect_keyword(Keyword::Ordinality)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(XmlTableColumn::ForOrdinality { name, meta });
        }
        let data_type = Box::new(self.parse_data_type()?);
        let mut path = None;
        let mut default = None;
        let mut not_null = None;
        loop {
            if self.peek_is_keyword(Keyword::Path)? {
                if path.is_some() {
                    return Err(self.unexpected("at most one PATH per XMLTABLE column"));
                }
                self.advance()?; // PATH
                path = Some(Box::new(self.parse_b_expr()?));
            } else if self.peek_is_keyword(Keyword::Default)? {
                if default.is_some() {
                    return Err(self.unexpected("at most one DEFAULT per XMLTABLE column"));
                }
                self.advance()?; // DEFAULT
                default = Some(Box::new(self.parse_b_expr()?));
            } else if self.peek_is_keyword(Keyword::Not)?
                && self.peek_nth_is_keyword(1, Keyword::Null)?
            {
                if not_null.is_some() {
                    return Err(self.unexpected("conflicting or redundant NULL / NOT NULL"));
                }
                self.advance()?; // NOT
                self.advance()?; // NULL
                not_null = Some(true);
            } else if self.peek_is_keyword(Keyword::Null)? {
                if not_null.is_some() {
                    return Err(self.unexpected("conflicting or redundant NULL / NOT NULL"));
                }
                self.advance()?; // NULL
                not_null = Some(false);
            } else {
                break;
            }
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(XmlTableColumn::Regular {
            name,
            data_type,
            path,
            default,
            not_null,
            meta,
        })
    }

    /// Parse SQL Server's `OPENJSON(<json> [, <path>]) [WITH (<col> <type> [<path>] [AS JSON],
    /// …)]` rowset-function table factor, cursor on `OPENJSON` (its `(` confirmed by the
    /// caller). The JSON source is any expression; the optional second argument and every
    /// column path are string literals (see
    /// [`parse_open_json_string_path`](Self::parse_open_json_string_path)). `OPENJSON` takes no
    /// `LATERAL` correlation, so a leading `LATERAL` is a clean reject (the `TABLE(<expr>)`
    /// precedent).
    fn parse_open_json_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if lateral {
            return Err(self.unexpected("a table function call after `LATERAL`"));
        }
        self.expect_keyword(Keyword::Openjson)?;
        self.expect_punct(Punctuation::LParen, "`(` after `OPENJSON`")?;
        let json_expr = Box::new(self.parse_expr()?);
        // The optional `, <path>` — a string-literal JSON path selecting the array/object to
        // iterate (MSSQL restricts it to a string constant, like the column paths).
        let path = if self.eat_punct(Punctuation::Comma)? {
            Some(self.parse_open_json_string_path()?)
        } else {
            None
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `OPENJSON`")?;
        // The `WITH (…)` explicit schema is optional; when absent the default key/value/type
        // schema applies and `columns` stays empty. MSSQL rejects an empty `WITH ()`, so a
        // present clause is one-or-more columns (`parse_comma_separated` is exact).
        let columns = if self.eat_keyword(Keyword::With)? {
            self.expect_punct(Punctuation::LParen, "`(` after `WITH`")?;
            let columns = self.parse_comma_separated(Self::parse_open_json_column)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the OPENJSON `WITH` columns",
            )?;
            columns
        } else {
            ThinVec::new()
        };
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::OpenJson {
            open_json: Box::new(OpenJson {
                json_expr,
                path,
                columns,
                meta,
            }),
            alias,
            meta,
        })
    }

    /// Parse one `OPENJSON` `WITH` column: `<name> <type> [<column_path>] [AS JSON]`. The
    /// optional column path is a string literal; `AS JSON` marks a nested-JSON column.
    fn parse_open_json_column(&mut self) -> ParseResult<OpenJsonColumn<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        let data_type = Box::new(self.parse_data_type()?);
        let path = if self.peek_is_string()? {
            Some(self.parse_open_json_string_path()?)
        } else {
            None
        };
        let as_json = self.eat_keyword(Keyword::As)? && {
            self.expect_keyword(Keyword::Json)?;
            true
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(OpenJsonColumn {
            name,
            data_type,
            path,
            as_json,
            meta,
        })
    }

    /// Parse an OPENJSON path — a string literal only. MSSQL's JSON path arguments (the
    /// `OPENJSON` second argument and each column path) are string constants, so a non-string
    /// token is a parse error (the `parse_json_table_string_path` precedent).
    fn parse_open_json_string_path(&mut self) -> ParseResult<Box<Expr<D::Ext>>> {
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a string-literal OPENJSON path"));
        };
        if token.kind != TokenKind::String {
            return Err(self.unexpected("a string-literal OPENJSON path"));
        }
        Ok(Box::new(self.parse_string_literal(token)?))
    }

    /// Parse Snowflake/Oracle's `TABLE(<expr>)` first-class table-expression factor
    /// (sqlparser-rs's `TableFactor::TableFunction`): an arbitrary expression evaluated
    /// as a set-returning table source. Distinct from a *named* table function
    /// ([`parse_named_table_factor`](Self::parse_named_table_factor), `FROM f(1)`),
    /// whose head is a call, not a parenthesized expression. The factor carries no
    /// `LATERAL` correlation (unlike [`Function`](TableFactor::Function)), so a leading
    /// `LATERAL` is a clean reject here, mirroring the special-value-function factor.
    fn parse_table_expr_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if lateral {
            return Err(self.unexpected("a table function call after `LATERAL`"));
        }
        self.advance()?; // TABLE
        self.advance()?; // `(`
        let expr = Box::new(self.parse_expr()?);
        self.expect_punct(Punctuation::RParen, "`)` to close `TABLE(...)`")?;
        let alias = self.parse_optional_table_alias()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::TableExpr { expr, alias, meta })
    }

    fn parse_named_table_factor(
        &mut self,
        start: Span,
        lateral: bool,
    ) -> ParseResult<TableFactor<D::Ext>> {
        if self.peek_is_keyword(Keyword::Only)?
            && (self.peek_nth_is_punct(1, Punctuation::LParen)?
                || self
                    .peek_nth(1)?
                    .is_some_and(|token| self.token_can_be_column_name(token)))
        {
            if lateral {
                return Err(self.unexpected("a table function call after `LATERAL`"));
            }
            if !self.features().table_expressions.only {
                return Err(self.unexpected("a table expression supported by this dialect"));
            }
            return self.parse_only_table_factor(start);
        }

        // DuckDB admits a single-part Sconst relation name (`FROM ''`, `FROM 't'`,
        // `FROM E't'`, `FROM $$t$$`; engine-measured on libduckdb 1.5.4). Dotted string
        // names (`FROM 'a'.'b'`) remain rejects. Gated by
        // [`IdentifierSyntax::string_literal_table_names`]; MySQL/PostgreSQL syntax-reject
        // a string here. A string name cannot open a table-function call (`'f'(…)`),
        // so the Sconst arm falls straight through to the named-table tail.
        let (name, name_start) = if self.features().identifier_syntax.string_literal_table_names
            && self.peek_is_name_sconst()?
        {
            let ident = self.parse_name_sconst_ident("a table name")?;
            let span = ident.meta.span;
            (ObjectName(thin_vec![ident]), span)
        } else {
            let Some(token) = self.peek()? else {
                return Err(self.unexpected("a table name, function call, or `(`"));
            };
            let head_reserved = self.name_or_call_head_reserved()?;
            if !self.token_admissible(token, head_reserved) {
                return Err(self.unexpected("a table name, function call, or `(`"));
            }
            let name = self.parse_object_name_with(head_reserved)?;
            (name, token.span)
        };
        if self.peek_is_punct(Punctuation::LParen)? {
            if !self.features().table_factor_syntax.table_functions {
                return Err(self.unexpected("a table expression supported by this dialect"));
            }
            let function = self.parse_function_call(name, name_start)?;
            // PostgreSQL's `func_table` is `func_expr_windowless`: a function in FROM admits
            // the plain `func_application` (arguments, a `DISTINCT` quantifier, an
            // in-parenthesis `ORDER BY`) but never the windowed/aggregate wrapper clauses —
            // `OVER`, `FILTER`, `WITHIN GROUP` — so `SELECT * FROM rank() OVER (…)` is a
            // syntax error. Reject the same set; a windowed function is not a valid FROM
            // source in any SQL dialect (universal, ungated).
            if function.over.is_some()
                || function.filter.is_some()
                || function.within_group.is_some()
            {
                let span = function.meta.span;
                return Err(self.error_at(
                    span,
                    "a plain table function: a function in FROM cannot carry an `OVER`, \
                     `FILTER`, or `WITHIN GROUP` clause",
                    self.span_text(span).to_owned(),
                ));
            }
            let with_ordinality = self.parse_with_ordinality()?;
            let (alias, column_defs) = self.parse_function_alias_clause()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(TableFactor::Function {
                lateral,
                function: Box::new(function),
                with_ordinality,
                alias,
                column_defs,
                meta,
            });
        }
        if lateral {
            return Err(self.unexpected("a table function call after `LATERAL`"));
        }

        // PostgreSQL caps a relation (`qualified_name`) at catalog.schema.table — three
        // name parts; a fourth is "improper qualified name (too many dotted names)".
        // SQLite has no catalog qualifier, so its relation names cap at schema.table — two
        // parts ([`FeatureSet::catalog_qualified_names`]). A column reference reaches four
        // parts via composite-field selection, but that is a different grammar position
        // (see `expr`), so only the relation path is capped.
        if name.0.len() > self.max_relation_name_parts() {
            let span = name_start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(span, self.relation_name_depth_expected(), found));
        }

        let inheritance = self.parse_descendant_star()?;
        // A PartiQL / SUPER JSON path (`FROM src[0].a`), attached directly to the name —
        // entered only by a `[` immediately after it, so it binds tighter than the version
        // and partition tails that follow.
        let json_path = self.parse_optional_table_json_path()?;
        // A version / time-travel modifier (`FOR SYSTEM_TIME AS OF …`, `VERSION AS OF …`)
        // binds immediately after the table name, before the alias. Read before the alias
        // parse so a bare `VERSION`/`TIMESTAMP` leading the clause is never swallowed as an
        // alias.
        let version = self.parse_optional_table_version()?;
        // MySQL `PARTITION (p0, p1)` binds between the name and the alias.
        let partition = self.parse_optional_partition_selection()?;
        // A base table: a column-list alias is gated separately from the derived positions
        // (MySQL admits `(SELECT …) AS c(x)` but rejects `t AS y(a, b)`).
        let alias = self.parse_optional_base_table_alias()?;
        // SQLite `INDEXED BY <index>` / `NOT INDEXED` binds after the alias.
        let indexed_by = self.parse_optional_indexed_by()?;
        // MySQL index hints bind after the alias.
        let index_hints = self.parse_index_hints()?;
        let sample = self.parse_optional_table_sample()?;
        // MSSQL `WITH (...)` table hints bind after the tablesample clause.
        let table_hints = self.parse_table_hints()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Table {
            name,
            inheritance,
            json_path,
            version,
            partition,
            alias,
            indexed_by,
            index_hints,
            sample,
            table_hints,
            meta,
        })
    }

    /// Parse an optional PartiQL / SUPER JSON path on a base table, navigating into a
    /// semi-structured column at the table-source position (`FROM src[0].a`,
    /// `FROM src[0].a[1].b`) — Redshift's SUPER navigation and Snowflake's PartiQL access.
    /// Empty when absent.
    ///
    /// Gated by
    /// [`TableExpressionSyntax::table_json_path`](crate::ast::dialect::TableExpressionSyntax).
    /// The entry trigger is a `[` immediately after the table name — a bracket index root —
    /// after which each `.key` / `[index]` suffix is read with the shared
    /// [`parse_semi_structured_path_suffix`](Self::parse_semi_structured_path_suffix). A name
    /// not followed by `[` has no path, so a dotted `FROM src.a.b` stays a compound relation
    /// name (its dots were already consumed by [`parse_object_name_with`](Self::parse_object_name_with)).
    fn parse_optional_table_json_path(
        &mut self,
    ) -> ParseResult<ThinVec<SemiStructuredPathSegment<D::Ext>>> {
        if !self.features().table_expressions.table_json_path {
            return Ok(ThinVec::new());
        }
        if !self.peek_is_punct(Punctuation::LBracket)? {
            return Ok(ThinVec::new());
        }
        let mut path = ThinVec::new();
        while let Some(segment) = self.parse_semi_structured_path_suffix()? {
            path.push(segment);
        }
        Ok(path)
    }

    /// Parse an optional table version / time-travel modifier written immediately after
    /// the table name (`FOR SYSTEM_TIME …`, `VERSION AS OF …`, `TIMESTAMP AS OF …`), or
    /// `None` when none leads. `Box`ed to keep [`TableFactor::Table`] within its size
    /// budget (ADR-0007).
    ///
    /// Gated by
    /// [`TableExpressionSyntax::table_version`](crate::ast::dialect::TableExpressionSyntax);
    /// when off, the clause keyword is left unconsumed.
    ///
    /// The `FOR SYSTEM_TIME` form sits at the *table-factor* position — read here, before
    /// the alias and before any join — so it never contends with the query-level `FOR`
    /// clauses (row locking, MSSQL `FOR XML`/`FOR JSON`), which are parsed only after the
    /// whole `FROM`/`WHERE`: the positions are partitioned, not the token. The trigger is
    /// `FOR` immediately followed by `SYSTEM_TIME`, so a query-level `FOR UPDATE`/`FOR XML`
    /// (a different word after `FOR`) is left for the query tail even under a preset that
    /// enables both (MSSQL). The `VERSION`/`TIMESTAMP` forms require the full `AS OF`
    /// lookahead before committing, so a bare `FROM t TIMESTAMP` still reads `TIMESTAMP` as
    /// the correlation alias rather than a truncated time-travel clause.
    fn parse_optional_table_version(&mut self) -> ParseResult<Option<Box<TableVersion<D::Ext>>>> {
        if !self.features().table_expressions.table_version {
            return Ok(None);
        }
        let start = self.current_span()?;
        if self.peek_is_keyword(Keyword::For)?
            && self.peek_nth_is_keyword(1, Keyword::SystemTime)?
        {
            self.advance()?; // FOR
            self.advance()?; // SYSTEM_TIME
            return Ok(Some(Box::new(self.parse_for_system_time_tail(start)?)));
        }
        // Delta/Databricks `VERSION AS OF …` / `TIMESTAMP AS OF …`. Committing needs the
        // full `AS OF` lookahead: a bare `FROM t VERSION` / `FROM t TIMESTAMP` must still
        // alias the table rather than begin a truncated clause.
        let is_timestamp = if self.peek_is_keyword(Keyword::Version)? {
            false
        } else if self.peek_is_keyword(Keyword::Timestamp)? {
            true
        } else {
            return Ok(None);
        };
        if !(self.peek_nth_is_keyword(1, Keyword::As)?
            && self.peek_nth_is_keyword(2, Keyword::Of)?)
        {
            return Ok(None);
        }
        self.advance()?; // VERSION | TIMESTAMP
        self.advance()?; // AS
        self.advance()?; // OF
        let expr = Box::new(self.parse_expr()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        let version = if is_timestamp {
            TableVersion::TimestampAsOf { point: expr, meta }
        } else {
            TableVersion::VersionAsOf {
                version: expr,
                meta,
            }
        };
        Ok(Some(Box::new(version)))
    }

    /// Parse the `FOR SYSTEM_TIME` tail after the `SYSTEM_TIME` keyword: one of the five
    /// MSSQL temporal forms (`AS OF`, `FROM … TO`, `BETWEEN … AND`, `CONTAINED IN (…, …)`,
    /// `ALL`), of which BigQuery accepts only the `AS OF` subset. `start` is the span of
    /// the leading `FOR`.
    ///
    /// The `BETWEEN`/`FROM` endpoints are parsed at the range-predicate's right binding
    /// power (the same rank the real `BETWEEN` operator parses its bounds at), so the
    /// separating `AND`/`TO` is read as a delimiter rather than folded into the endpoint
    /// expression.
    fn parse_for_system_time_tail(&mut self, start: Span) -> ParseResult<TableVersion<D::Ext>> {
        let bound_bp = self.features().binding_powers.range_predicate().right;
        let version = if self.eat_keyword(Keyword::As)? {
            self.expect_keyword(Keyword::Of)?;
            let point = Box::new(self.parse_expr()?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            TableVersion::ForSystemTimeAsOf { point, meta }
        } else if self.eat_keyword(Keyword::From)? {
            let from = Box::new(self.parse_expr_bp(bound_bp)?);
            self.expect_keyword(Keyword::To)?;
            let to = Box::new(self.parse_expr()?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            TableVersion::ForSystemTimeFromTo {
                start: from,
                end: to,
                meta,
            }
        } else if self.eat_keyword(Keyword::Between)? {
            let low = Box::new(self.parse_expr_bp(bound_bp)?);
            self.expect_keyword(Keyword::And)?;
            let high = Box::new(self.parse_expr_bp(bound_bp)?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            TableVersion::ForSystemTimeBetween {
                start: low,
                end: high,
                meta,
            }
        } else if self.eat_keyword(Keyword::Contained)? {
            self.expect_keyword(Keyword::In)?;
            self.expect_punct(Punctuation::LParen, "`(` after `CONTAINED IN`")?;
            let from = Box::new(self.parse_expr()?);
            self.expect_punct(
                Punctuation::Comma,
                "`,` between the `CONTAINED IN` endpoints",
            )?;
            let to = Box::new(self.parse_expr()?);
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the `CONTAINED IN` endpoints",
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            TableVersion::ForSystemTimeContainedIn {
                start: from,
                end: to,
                meta,
            }
        } else if self.eat_keyword(Keyword::All)? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            TableVersion::ForSystemTimeAll { meta }
        } else {
            return Err(self.unexpected(
                "`AS OF`, `FROM`, `BETWEEN`, `CONTAINED IN`, or `ALL` after `FOR SYSTEM_TIME`",
            ));
        };
        Ok(version)
    }

    /// Parse the optional MySQL explicit partition selection `PARTITION (p0, p1)`,
    /// written between the table name and the alias. Empty when absent.
    ///
    /// Gated by
    /// [`TableExpressionSyntax::partition_selection`](crate::ast::dialect::TableExpressionSyntax);
    /// a dialect without it leaves `PARTITION` unconsumed, so the construct surfaces as
    /// a parse error. The partition names are a non-empty comma list of bare
    /// identifiers.
    fn parse_optional_partition_selection(&mut self) -> ParseResult<ThinVec<Ident>> {
        if !(self.features().table_expressions.partition_selection
            && self.peek_is_keyword(Keyword::Partition)?)
        {
            return Ok(ThinVec::new());
        }
        self.advance()?; // PARTITION
        self.expect_punct(Punctuation::LParen, "`(` after `PARTITION`")?;
        let names = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the partition list")?;
        Ok(names)
    }

    /// Parse the MySQL index-hint list on a table factor, written after the alias.
    /// Empty when absent. Multiple hints are juxtaposed (space-separated, no comma).
    ///
    /// Gated by
    /// [`TableExpressionSyntax::index_hints`](crate::ast::dialect::TableExpressionSyntax);
    /// a dialect without it leaves the hint keyword (`USE`/`FORCE`/`IGNORE`) to the
    /// identifier grammar, so the construct surfaces as a parse error.
    fn parse_index_hints(&mut self) -> ParseResult<ThinVec<IndexHint>> {
        let mut hints = ThinVec::new();
        if !self.features().table_expressions.index_hints {
            return Ok(hints);
        }
        while let Some(hint) = self.parse_optional_index_hint()? {
            hints.push(hint);
        }
        Ok(hints)
    }

    /// Parse an optional SQLite `INDEXED BY <index>` / `NOT INDEXED` index directive after a
    /// base table's alias, or `None` when none leads. `Box`ed to keep [`TableFactor::Table`]
    /// within its size budget (ADR-0007).
    ///
    /// Gated by
    /// [`TableExpressionSyntax::indexed_by`](crate::ast::dialect::TableExpressionSyntax);
    /// when off the `INDEXED`/`NOT` keywords are left unconsumed. A bare `INDEXED` at the
    /// alias position is already declined as a correlation alias by
    /// [`peek_opens_indexed_by_clause`](Self::peek_opens_indexed_by_clause), so it reaches
    /// here; a bare `INDEXED` with no trailing `BY` is then a parse error, matching SQLite
    /// (which commits to the directive on the keyword). The `NOT INDEXED` form needs no alias
    /// decline — `NOT` is already reserved as a bare SQLite alias — so its head is read
    /// directly here, and only when `NOT` is immediately followed by `INDEXED` (so a bare
    /// `NOT` opening some other construct is left untouched).
    fn parse_optional_indexed_by(&mut self) -> ParseResult<Option<Box<IndexedBy>>> {
        if !self.features().table_expressions.indexed_by {
            return Ok(None);
        }
        let start = self.current_span()?;
        if self.peek_is_keyword(Keyword::Not)? && self.peek_nth_is_keyword(1, Keyword::Indexed)? {
            self.eat_keyword(Keyword::Not)?;
            self.eat_keyword(Keyword::Indexed)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(Box::new(IndexedBy::NotIndexed { meta })));
        }
        if self.eat_keyword(Keyword::Indexed)? {
            self.expect_keyword(Keyword::By)?;
            let index = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(Box::new(IndexedBy::Named { index, meta })));
        }
        Ok(None)
    }

    /// Parse one MySQL index hint, or `None` when no `USE`/`FORCE`/`IGNORE` leads:
    /// `{USE|FORCE|IGNORE} {INDEX|KEY} [FOR {JOIN|ORDER BY|GROUP BY}] (<index>, …)`.
    fn parse_optional_index_hint(&mut self) -> ParseResult<Option<IndexHint>> {
        let start = self.current_span()?;
        let action = if self.eat_keyword(Keyword::Use)? {
            IndexHintAction::Use
        } else if self.eat_keyword(Keyword::Force)? {
            IndexHintAction::Force
        } else if self.eat_keyword(Keyword::Ignore)? {
            IndexHintAction::Ignore
        } else {
            return Ok(None);
        };
        let keyword = if self.eat_keyword(Keyword::Index)? {
            IndexHintKeyword::Index
        } else if self.eat_keyword(Keyword::Key)? {
            IndexHintKeyword::Key
        } else {
            return Err(self.unexpected("`INDEX` or `KEY` in an index hint"));
        };
        // The optional `FOR {JOIN|ORDER BY|GROUP BY}` scope precedes the index list.
        let scope = if self.eat_keyword(Keyword::For)? {
            Some(self.parse_index_hint_scope()?)
        } else {
            None
        };
        self.expect_punct(Punctuation::LParen, "`(` to open the index hint list")?;
        let indexes = if self.peek_is_punct(Punctuation::RParen)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated(Self::parse_ident)?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the index hint list")?;
        // MySQL admits the empty list only for `USE INDEX ()` (the "use no index" form);
        // `FORCE`/`IGNORE` require at least one index name.
        if indexes.is_empty() && action != IndexHintAction::Use {
            return Err(self.error_at(
                start.union(self.preceding_span()),
                "at least one index name for a `FORCE`/`IGNORE` index hint",
                self.span_text(start.union(self.preceding_span()))
                    .to_owned(),
            ));
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(IndexHint {
            action,
            keyword,
            scope,
            indexes,
            meta,
        }))
    }

    /// Parse the `JOIN` / `ORDER BY` / `GROUP BY` scope keyword(s) after an index
    /// hint's `FOR`.
    fn parse_index_hint_scope(&mut self) -> ParseResult<IndexHintScope> {
        if self.eat_keyword(Keyword::Join)? {
            Ok(IndexHintScope::Join)
        } else if self.eat_keyword(Keyword::Order)? {
            self.expect_keyword(Keyword::By)?;
            Ok(IndexHintScope::OrderBy)
        } else if self.eat_keyword(Keyword::Group)? {
            self.expect_keyword(Keyword::By)?;
            Ok(IndexHintScope::GroupBy)
        } else {
            Err(self.unexpected("`JOIN`, `ORDER BY`, or `GROUP BY` after `FOR` in an index hint"))
        }
    }

    /// Parse the optional MSSQL / T-SQL `WITH ( <hint>, … )` table-hint list, written
    /// after the tablesample clause. Empty when absent.
    ///
    /// Gated by
    /// [`TableExpressionSyntax::table_hints`](crate::ast::dialect::TableExpressionSyntax);
    /// when off, the trailing `WITH` is left unconsumed and the construct surfaces as a
    /// clean parse error (under ANSI/PostgreSQL `WITH` stays CTE-only). At this
    /// table-factor tail position `WITH` is never otherwise consumed on a base table, so
    /// `WITH (` is unambiguously a hint list — distinct from the leading-`WITH` CTE
    /// clause, which sits at statement start.
    ///
    /// Kept in its own helper (not inlined into `parse_table_factor`) so the hot
    /// recursive table-factor frame stays lean — the stack-canary discipline.
    ///
    /// Only the modern comma-separated `WITH (...)` surface is modelled. T-SQL's legacy
    /// bare parenthesized hint (`FROM t (NOLOCK)`, no `WITH`) collides with the
    /// function-call / column-list-alias readings and is a deliberate deferral.
    fn parse_table_hints(&mut self) -> ParseResult<ThinVec<TableHint>> {
        if !self.features().table_expressions.table_hints {
            return Ok(ThinVec::new());
        }
        if !self.peek_is_keyword(Keyword::With)? {
            return Ok(ThinVec::new());
        }
        self.advance()?; // WITH
        self.expect_punct(
            Punctuation::LParen,
            "`(` after `WITH` to open the table hint list",
        )?;
        let hints = self.parse_comma_separated(Self::parse_table_hint)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the table hint list")?;
        Ok(hints)
    }

    /// Parse one MSSQL table hint: the argument-bearing `INDEX` / `FORCESEEK` forms, a
    /// modelled single-keyword hint, or an unrecognized word preserved verbatim.
    ///
    /// The leading word names the hint; its source text is read for classification
    /// whether the tokenizer produced a keyword (`INDEX`) or a bare word (`NOLOCK`,
    /// `FORCESEEK`, …), so the modelled set is not tied to the reserved-keyword table.
    fn parse_table_hint(&mut self) -> ParseResult<TableHint> {
        let start = self.current_span()?;
        let word = self.span_text(start).to_ascii_uppercase();
        match word.as_str() {
            "INDEX" => {
                self.advance()?; // INDEX
                let (equals, indexes) = self.parse_index_table_hint_body()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(TableHint::Index {
                    equals,
                    indexes,
                    meta,
                })
            }
            "FORCESEEK" => {
                self.advance()?; // FORCESEEK
                let target = self.parse_optional_forceseek_target()?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(TableHint::ForceSeek { target, meta })
            }
            other => match TableHintKeyword::from_upper(other) {
                Some(keyword) => {
                    self.advance()?; // the modelled hint word
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    Ok(TableHint::Keyword { keyword, meta })
                }
                // An unrecognized word: read it as an identifier (preserving spelling)
                // so it round-trips rather than over-rejecting.
                None => {
                    let ident = self.parse_ident()?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    Ok(TableHint::Other { ident, meta })
                }
            },
        }
    }

    /// Parse the tail of a T-SQL `INDEX` table hint (`INDEX` already consumed):
    /// `( <index>, … )`, `= <index>`, or `= ( <index>, … )`, returning
    /// `(equals, indexes)`. Only named indexes are modelled; a numeric index id
    /// (`INDEX(0)`) is a conservative deferral.
    fn parse_index_table_hint_body(&mut self) -> ParseResult<(bool, ThinVec<Ident>)> {
        let equals = self.eat_op(Operator::Eq)?;
        let indexes = if self.eat_punct(Punctuation::LParen)? {
            let ids = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `INDEX` hint list")?;
            ids
        } else if equals {
            // `INDEX = <index>`: a single unparenthesized index name.
            thin_vec::thin_vec![self.parse_ident()?]
        } else {
            return Err(self.unexpected("`(` or `=` after `INDEX` in a table hint"));
        };
        Ok((equals, indexes))
    }

    /// Parse the optional `( <index> ( <column>, … ) )` argument of a `FORCESEEK` table
    /// hint (`FORCESEEK` already consumed): `None` for the bare `FORCESEEK`.
    fn parse_optional_forceseek_target(&mut self) -> ParseResult<Option<ForceSeekTarget>> {
        let start = self.current_span()?;
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(None);
        }
        let index = self.parse_ident()?;
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the `FORCESEEK` column list",
        )?;
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the `FORCESEEK` column list",
        )?;
        self.expect_punct(Punctuation::RParen, "`)` to close the `FORCESEEK` hint")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(ForceSeekTarget {
            index,
            columns,
            meta,
        }))
    }

    fn parse_only_table_factor(&mut self, start: Span) -> ParseResult<TableFactor<D::Ext>> {
        self.expect_keyword(Keyword::Only)?;
        let (only, name) = if self.eat_punct(Punctuation::LParen)? {
            let name = self.parse_object_name()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `ONLY` table name")?;
            (OnlySyntax::Parenthesized, name)
        } else {
            (OnlySyntax::Bare, self.parse_object_name()?)
        };
        let alias = self.parse_optional_table_alias()?;
        let sample = self.parse_optional_table_sample()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableFactor::Table {
            name,
            inheritance: RelationInheritance::Only(only),
            // `ONLY` is PostgreSQL inheritance suppression; the Redshift/Snowflake PartiQL
            // path, the BigQuery/MSSQL version, and the MySQL-only partition / index-hint
            // tails never attach to it.
            json_path: ThinVec::new(),
            version: None,
            partition: ThinVec::new(),
            alias,
            // `ONLY` is PostgreSQL inheritance suppression; the SQLite `INDEXED BY` directive
            // never attaches to it.
            indexed_by: None,
            index_hints: ThinVec::new(),
            sample,
            // `ONLY` is PostgreSQL inheritance suppression; the MSSQL `WITH (...)` hint
            // tail never attaches to it.
            table_hints: ThinVec::new(),
            meta,
        })
    }

    /// Parse the optional trailing `*` descendant-table marker on a `relation_expr`
    /// (PostgreSQL `qualified_name '*'`): [`RelationInheritance::Descendants`] when
    /// the star is present, else [`RelationInheritance::Plain`].
    ///
    /// The `*` shares the `ONLY` inheritance gate (`table_expressions.only`):
    /// PostgreSQL's `relation_expr` enables `ONLY` suppression and the explicit
    /// descendant `*` together — no dialect has one without the other. Shared by
    /// `FROM` items and `UPDATE`/`DELETE` targets, where `*` follows the relation
    /// name (and never an `ONLY` form, which already selects all descendants).
    pub(super) fn parse_descendant_star(&mut self) -> ParseResult<RelationInheritance> {
        if !self.peek_is_op(Operator::Star)? {
            return Ok(RelationInheritance::Plain);
        }
        if !self.features().table_expressions.only {
            return Err(self.unexpected("a table relation supported by this dialect"));
        }
        self.advance()?;
        Ok(RelationInheritance::Descendants)
    }

    fn parse_table_function_call(&mut self) -> ParseResult<FunctionCall<D::Ext>> {
        let start = self.current_span()?;
        let Some(token) = self.peek()? else {
            return Err(self.unexpected("a table function call"));
        };
        // A table function name is gated like any call: the head widens to the
        // function-name set for an unqualified `func(`, and is a `ColId` qualifier
        // otherwise (`schema.func(`).
        let head_reserved = self.name_or_call_head_reserved()?;
        if !self.token_admissible(token, head_reserved) {
            return Err(self.unexpected("a table function call"));
        }
        let name = self.parse_object_name_with(head_reserved)?;
        if !self.peek_is_punct(Punctuation::LParen)? {
            return Err(self.unexpected("`(` after a table function name"));
        }
        self.parse_function_call(name, start)
    }

    /// Parse one `ROWS FROM (...)` item: a table function call and its optional
    /// per-function column definition list (PostgreSQL `rowsfrom_item`). A ROWS
    /// FROM item carries no correlation name of its own — that belongs to the
    /// `ROWS FROM` factor's alias — so the only tail it accepts is `AS ( ... )`.
    fn parse_rows_from_item(&mut self) -> ParseResult<RowsFromItem<D::Ext>> {
        let start = self.current_span()?;
        let function = self.parse_table_function_call()?;
        let column_defs = if self.eat_keyword(Keyword::As)? {
            self.parse_table_func_element_list()?
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(RowsFromItem {
            function,
            column_defs,
            meta,
        })
    }

    /// Parse a table function's `func_alias_clause`: an optional correlation alias
    /// and an optional column definition list.
    ///
    /// PostgreSQL lets a record-returning function type its output columns here —
    /// `func(...) AS (a int, b text)` or `func(...) AS x(a int, b text)` — which is
    /// a distinct production from an untyped alias column list (`func(...) AS x(a,
    /// b)`). The two parenthesized forms are told apart by whether the first element
    /// carries a type, so a typed definition is never recorded as an alias column.
    fn parse_function_alias_clause(&mut self) -> ParseResult<FunctionAliasClause<D::Ext>> {
        let explicit_as = self.eat_keyword(Keyword::As)?;

        // `AS ( a int, ... )`: a column definition list with no correlation name.
        if explicit_as && self.peek_is_punct(Punctuation::LParen)? {
            let column_defs = self.parse_table_func_element_list()?;
            return Ok((None, column_defs));
        }

        // No correlation name and no leading column definition list.
        if !explicit_as && !self.peek_can_start_column_name()? {
            return Ok((None, ThinVec::new()));
        }

        let start = self.current_span()?;
        let name = self.parse_ident()?;
        if self.peek_is_punct(Punctuation::LParen)? {
            return self.parse_named_function_alias_paren(start, name, explicit_as);
        }

        // A bare correlation name (`func(...) AS x` / `func(...) x`).
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok((
            Some(Box::new(TableAlias {
                name,
                columns: ThinVec::new(),
                spelling: alias_spelling(explicit_as),
                meta,
            })),
            ThinVec::new(),
        ))
    }

    /// Parse the parenthesized tail of a named function alias, once the correlation
    /// `name` (spanning from `start`) and a following `(` have been seen. The first
    /// element decides the form: a bare name then `,`/`)` is an alias column-name
    /// list, anything else is the first typed entry of a definition list.
    fn parse_named_function_alias_paren(
        &mut self,
        start: Span,
        name: Ident,
        explicit_as: bool,
    ) -> ParseResult<FunctionAliasClause<D::Ext>> {
        self.advance()?; // `(`
        let element_start = self.current_span()?;
        let first = self.parse_ident()?;

        if self.peek_is_punct(Punctuation::Comma)? || self.peek_is_punct(Punctuation::RParen)? {
            if !self.features().table_expressions.table_alias_column_lists {
                return Err(self.unexpected("a table alias supported by this dialect"));
            }
            let mut columns = thin_vec![first];
            while self.eat_punct(Punctuation::Comma)? {
                columns.push(self.parse_ident()?);
            }
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the table alias column list",
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok((
                Some(Box::new(TableAlias {
                    name,
                    columns,
                    spelling: alias_spelling(explicit_as),
                    meta,
                })),
                ThinVec::new(),
            ));
        }

        let mut column_defs = thin_vec![self.finish_table_func_element(element_start, first)?];
        while self.eat_punct(Punctuation::Comma)? {
            column_defs.push(self.parse_table_func_element()?);
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the column definition list",
        )?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok((
            Some(Box::new(TableAlias {
                name,
                columns: ThinVec::new(),
                spelling: alias_spelling(explicit_as),
                meta,
            })),
            column_defs,
        ))
    }

    /// Parse a parenthesized `TableFuncElementList`: `( name type [, name type]* )`.
    /// The opening `(` is expected here (not yet consumed).
    fn parse_table_func_element_list(
        &mut self,
    ) -> ParseResult<ThinVec<TableFunctionColumn<D::Ext>>> {
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the column definition list",
        )?;
        let column_defs = self.parse_comma_separated(Self::parse_table_func_element)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the column definition list",
        )?;
        Ok(column_defs)
    }

    /// Parse one `name type` column definition (PostgreSQL `TableFuncElement`).
    fn parse_table_func_element(&mut self) -> ParseResult<TableFunctionColumn<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        self.finish_table_func_element(start, name)
    }

    /// Finish a column definition whose `name` (spanning from `start`) is parsed,
    /// reading the type that makes it a typed definition rather than an alias name.
    fn finish_table_func_element(
        &mut self,
        start: Span,
        name: Ident,
    ) -> ParseResult<TableFunctionColumn<D::Ext>> {
        let data_type = self.parse_data_type()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableFunctionColumn {
            name,
            data_type,
            meta,
        })
    }

    /// Parse an optional `WITH ORDINALITY` tail on a table-valued `FROM` source. Gated by
    /// [`TableFactorSyntax::table_function_ordinality`](crate::ast::dialect::TableExpressionSyntax):
    /// a dialect without it (SQLite admits generic `table_functions` but not this tail)
    /// leaves `WITH` unconsumed, so the trailing clause surfaces as a parse error.
    fn parse_with_ordinality(&mut self) -> ParseResult<bool> {
        if !self
            .features()
            .table_factor_syntax
            .table_function_ordinality
        {
            return Ok(false);
        }
        if !self.peek_is_keyword(Keyword::With)? {
            return Ok(false);
        }
        self.advance()?;
        self.expect_contextual_keyword("ORDINALITY")?;
        Ok(true)
    }

    fn parse_optional_table_sample(&mut self) -> ParseResult<Option<TableSample<D::Ext>>> {
        if !self.peek_is_keyword(Keyword::Tablesample)? {
            return Ok(None);
        }
        if !self.features().table_expressions.table_sample {
            return Err(self.unexpected("a table expression supported by this dialect"));
        }
        let start = self.current_span()?;
        self.advance()?;
        Ok(Some(self.parse_table_sample_tail(start)?))
    }

    /// Parse a `TABLESAMPLE` clause body — everything after the `TABLESAMPLE` keyword:
    /// `<method> (<args>) [REPEATABLE (<seed>)]`. `start` is the span of the
    /// `TABLESAMPLE` keyword, so the returned node's span covers the whole clause. Split
    /// from [`parse_optional_table_sample`](Self::parse_optional_table_sample) — which
    /// keeps the `FROM`-suffix keyword peek and the `table_sample` feature gate — so the
    /// pipe `|> TABLESAMPLE` operator, gated only by `pipe_syntax`, reuses the identical
    /// body grammar without re-checking a `FROM`-relation gate.
    pub(super) fn parse_table_sample_tail(
        &mut self,
        start: Span,
    ) -> ParseResult<TableSample<D::Ext>> {
        let method = self.parse_object_name()?;
        self.expect_punct(Punctuation::LParen, "`(` after `TABLESAMPLE` method")?;
        let args = if self.peek_is_punct(Punctuation::RParen)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated_exprs()?
        };
        self.expect_punct(Punctuation::RParen, "`)` to close `TABLESAMPLE` arguments")?;
        let repeatable = if self.eat_contextual_keyword("REPEATABLE")? {
            self.expect_punct(Punctuation::LParen, "`(` after `REPEATABLE`")?;
            let expr = self.parse_expr()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `REPEATABLE`")?;
            Some(Box::new(expr))
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableSample {
            method,
            args,
            repeatable,
            meta,
        })
    }

    /// Parse the next join onto the current relation, or `None` if none follows.
    ///
    /// Recognizes `[INNER] JOIN`, `LEFT|RIGHT|FULL [OUTER] JOIN`, `CROSS JOIN`,
    /// `NATURAL [LEFT|RIGHT|FULL] JOIN`, (MySQL) `STRAIGHT_JOIN`, and (DuckDB)
    /// `ASOF [side] JOIN` / `POSITIONAL JOIN`. The `OUTER` keyword is accepted and
    /// dropped (the canonical shape records the side, not the spelling). When no join
    /// keyword leads, nothing is consumed and `None` ends the join chain.
    ///
    /// Recursion-guarded: a qualifier-requiring join's right operand
    /// may itself open a further unparenthesized join chain that must be fully
    /// read before *this* join's own `ON`/`USING` (`parse_join_right_operand`'s
    /// right-nesting), so a pathological `a JOIN b JOIN c JOIN d ...` chain
    /// recurses the Rust call stack one level per absorbed join — unlike the
    /// plain `while let Some(join) = self.parse_join()?` loop callers use for an
    /// ordinary (non-nesting) join chain, which never re-enters this method
    /// before returning and so costs no depth.
    pub(super) fn parse_join(&mut self) -> ParseResult<Option<Join<D::Ext>>> {
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard.parser().parse_join_inner()
    }

    /// Parse one join, one level deep under the recursion guard.
    fn parse_join_inner(&mut self) -> ParseResult<Option<Join<D::Ext>>> {
        let start = self.current_span()?;

        // NATURAL [LEFT|RIGHT|FULL [OUTER]] JOIN <factor>: the constraint is the
        // shared-column match, so no ON/USING follows.
        if self.peek_is_keyword(Keyword::Natural)? {
            let natural = self
                .advance()?
                .expect("peek_is_keyword confirmed a NATURAL token is present");
            // DuckDB `NATURAL SEMI JOIN` / `NATURAL ANTI JOIN`: SEMI/ANTI compose with
            // the NATURAL ref-type (engine-verified on 1.5.4). Checked before the side,
            // since SEMI/ANTI are `join_type`s, not sides; the shared-column match is
            // the constraint, so no ON/USING follows (as for the plain NATURAL join).
            if self.features().join_syntax.semi_anti_join {
                if let Some((semi_anti, _span)) = self.eat_semi_anti_keyword()? {
                    self.expect_keyword(Keyword::Join)?;
                    let operator_span = natural.span.union(self.preceding_span());
                    let relation = self.parse_table_factor()?;
                    let constraint = JoinConstraint::Natural {
                        meta: self.make_meta(natural.span),
                    };
                    let operator =
                        semi_anti.into_operator(false, constraint, self.make_meta(operator_span));
                    return Ok(Some(self.finish_join(start, relation, operator)));
                }
            }
            // `NATURAL INNER JOIN`: PostgreSQL's `join_type` admits an explicit `INNER`
            // (the default side). Like the `OUTER` noise word a side already encodes, it
            // is consumed and dropped — `INNER` and a bare `NATURAL JOIN` yield the same
            // `Inner` side — and it is mutually exclusive with a `LEFT`/`RIGHT`/`FULL`
            // side, so it is only checked when no side keyword follows.
            // `explicit` records a written redundant `INNER`/`OUTER` after `NATURAL`
            // (`NATURAL INNER JOIN`, `NATURAL LEFT OUTER JOIN`). `NATURAL CROSS JOIN`
            // normalizes to the bare natural inner join, so it records no `INNER`.
            let (side, explicit) = if self.eat_keyword(Keyword::Inner)? {
                (JoinSide::Inner, true)
            } else if self.features().join_syntax.natural_cross_join
                && self.eat_keyword(Keyword::Cross)?
            {
                // SQLite `NATURAL CROSS JOIN`: `CROSS` is the optimizer-hint spelling of
                // `INNER`, and `NATURAL` supplies the shared-column constraint, so it is a
                // natural inner join (engine-probed: same row/column shape as `NATURAL
                // JOIN`, not the cross product). Normalized to the canonical `Inner` side
                // (the `INNER` noise-word precedent above); renders back as `NATURAL JOIN`.
                (JoinSide::Inner, false)
            } else {
                self.eat_join_side()?
                    .map(|(side, _span, outer)| (side, outer))
                    .unwrap_or((JoinSide::Inner, false))
            };
            self.expect_keyword(Keyword::Join)?;
            let operator_span = natural.span.union(self.preceding_span());
            let relation = self.parse_table_factor()?;
            let constraint = JoinConstraint::Natural {
                meta: self.make_meta(natural.span),
            };
            let operator = side.into_operator(explicit, constraint, self.make_meta(operator_span));
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // CROSS JOIN <factor>: an unconstrained cross product. MSSQL's `CROSS APPLY`
        // shares the leading `CROSS` keyword but is a distinct lateral-correlated
        // operator over a right table factor — no `JOIN`, no `ON`/`USING`.
        if self.peek_is_keyword(Keyword::Cross)? {
            let cross = self
                .advance()?
                .expect("peek_is_keyword confirmed a CROSS token is present");
            // MSSQL CROSS APPLY <factor>: gated dialect data. When the gate is off,
            // `APPLY` is left for the `expect_keyword(Join)` below, which rejects it —
            // the clean parse divergence under a non-MSSQL preset. Like CROSS it never
            // carries a constraint (the correlation lives in the right factor's own
            // references), so no `ON`/`USING` is read.
            if self.features().join_syntax.apply_join && self.eat_keyword(Keyword::Apply)? {
                let operator_span = cross.span.union(self.preceding_span());
                let relation = self.parse_table_factor()?;
                let operator = JoinOperator::Apply {
                    kind: ApplyKind::Cross,
                    meta: self.make_meta(operator_span),
                };
                return Ok(Some(self.finish_join(start, relation, operator)));
            }
            self.expect_keyword(Keyword::Join)?;
            let operator_span = cross.span.union(self.preceding_span());
            let relation = self.parse_table_factor()?;
            let operator = JoinOperator::Cross {
                meta: self.make_meta(operator_span),
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // MSSQL OUTER APPLY <factor>: the outer-preserving flavour of CROSS APPLY —
        // the same lateral-correlated operator over a right table factor, but keeping
        // left rows whose right factor produces no rows (a `LEFT JOIN LATERAL … ON
        // TRUE` equivalence). Gated on the same `apply_join` data and, like CROSS
        // APPLY, it carries no `ON`/`USING` (the correlation lives in the right
        // factor's own references). `OUTER` never leads a join alone — the outer sides
        // are `LEFT`/`RIGHT`/`FULL`, which each eat a trailing optional `OUTER` inside
        // `eat_join_side`, so a bare leading `OUTER` is unambiguously the APPLY head.
        // The two-token lookahead confirms `APPLY` follows before consuming anything,
        // so a gated-off preset (or a stray leading `OUTER`) leaves the keyword
        // unconsumed to fall through to the final side dispatch, which returns `None`
        // and surfaces the clean pre-existing trailing-input reject.
        if self.features().join_syntax.apply_join
            && self.peek_is_keyword(Keyword::Outer)?
            && self.peek_nth_is_keyword(1, Keyword::Apply)?
        {
            let outer = self
                .advance()?
                .expect("peek_is_keyword confirmed an OUTER token is present");
            self.expect_keyword(Keyword::Apply)?;
            let operator_span = outer.span.union(self.preceding_span());
            let relation = self.parse_table_factor()?;
            let operator = JoinOperator::Apply {
                kind: ApplyKind::Outer,
                meta: self.make_meta(operator_span),
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // MySQL STRAIGHT_JOIN <factor> <constraint>: an inner join that also forces
        // the optimizer to read the left table before the right. Gated dialect data;
        // under a dialect without it (ANSI/PostgreSQL) `STRAIGHT_JOIN` is a
        // non-reserved word the preceding table factor already took as an alias, so
        // this never fires and the chain ends cleanly. It carries the same ON/USING
        // constraint grammar as a bare `JOIN`, recorded on the canonical `Inner`
        // operator with the `straight` surface tag set.
        if self.features().join_syntax.straight_join
            && self.peek_is_keyword(Keyword::StraightJoin)?
        {
            let token = self
                .advance()?
                .expect("peek_is_keyword confirmed a STRAIGHT_JOIN token is present");
            let relation = self.parse_table_factor()?;
            let constraint = self.parse_join_constraint()?;
            // The operator's span must cover its own `constraint` field (a whole-tree
            // span invariant, conformance/src/spans.rs), which — unlike NATURAL/CROSS,
            // which carry no constraint or a constraint whose span is already folded
            // into the keyword span above — is parsed after `token.span` is captured
            // and can extend well past it (an `ON <predicate>` or `USING (...)` list).
            let operator_span = token.span.union(constraint.span());
            let operator = JoinOperator::Inner {
                straight: true,
                // `STRAIGHT_JOIN` is its own keyword and is never spelled `INNER`.
                inner: false,
                constraint,
                meta: self.make_meta(operator_span),
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // DuckDB ASOF [INNER | LEFT|RIGHT|FULL [OUTER]] JOIN <factor> <constraint>:
        // the nearest-match temporal join, `ASOF` prefixing the whole side spelling.
        // Gated dialect data; the DuckDb preset also reserves `asof` as a ColId
        // (`DUCKDB_NONSTANDARD_JOIN_RESERVATION`) so the preceding factor's bare
        // alias cannot swallow it — under a dialect without the reservation the
        // alias reading wins first and this arm only fires after an explicit alias.
        // The constraint is mandatory: DuckDB *parse*-rejects a bare `ASOF JOIN`
        // (unlike the sibling side-join arm, whose `JoinConstraint::None` fallback
        // exists for MySQL's bare inner join), while NATURAL/CROSS composition is
        // parse-rejected by the engine and rejects here too (`NATURAL ASOF` fails the
        // NATURAL arm's mandatory `JOIN`; `ASOF CROSS` fails this arm's).
        if self.features().join_syntax.asof_join && self.peek_is_keyword(Keyword::Asof)? {
            let token = self
                .advance()?
                .expect("peek_is_keyword confirmed an ASOF token is present");
            // DuckDB `ASOF SEMI JOIN` / `ASOF ANTI JOIN`: ASOF composes with the
            // SEMI/ANTI `join_type`s too (engine-verified on 1.5.4). Checked before the
            // side, since SEMI/ANTI are not sides; the ON/USING constraint is mandatory
            // exactly as for the side-ASOF join below (ASOF/NATURAL never co-occur, so
            // `NATURAL` can never reach here — the semi/anti constraint is ON/USING).
            if self.features().join_syntax.semi_anti_join {
                if let Some((semi_anti, _span)) = self.eat_semi_anti_keyword()? {
                    self.expect_keyword(Keyword::Join)?;
                    let factor = self.parse_table_factor()?;
                    let relation = self.parse_join_right_operand(factor)?;
                    let constraint = self.parse_join_constraint()?;
                    if matches!(constraint, JoinConstraint::None { .. }) {
                        return Err(self.unexpected(
                            "an `ON` or `USING` constraint after `ASOF SEMI`/`ANTI JOIN`",
                        ));
                    }
                    let operator_span = token.span.union(constraint.span());
                    let operator =
                        semi_anti.into_operator(true, constraint, self.make_meta(operator_span));
                    return Ok(Some(self.finish_join(start, relation, operator)));
                }
            }
            let kind = if self.eat_keyword(Keyword::Inner)? {
                AsOfJoinKind::Inner
            } else if let Some((side, _span, _outer)) = self.eat_join_side()? {
                match side {
                    // `eat_join_side` never yields `Inner` (it only eats
                    // LEFT/RIGHT/FULL), but the match stays exhaustive.
                    JoinSide::Inner => AsOfJoinKind::Inner,
                    JoinSide::Left => AsOfJoinKind::Left,
                    JoinSide::Right => AsOfJoinKind::Right,
                    JoinSide::Full => AsOfJoinKind::Full,
                }
            } else {
                AsOfJoinKind::Inner
            };
            self.expect_keyword(Keyword::Join)?;
            let factor = self.parse_table_factor()?;
            let relation = self.parse_join_right_operand(factor)?;
            let constraint = self.parse_join_constraint()?;
            if matches!(constraint, JoinConstraint::None { .. }) {
                return Err(self.unexpected("an `ON` or `USING` constraint after `ASOF JOIN`"));
            }
            // Widen to cover `constraint` (see the STRAIGHT_JOIN arm above for why).
            let operator_span = token.span.union(constraint.span());
            let operator = JoinOperator::AsOf {
                kind,
                constraint,
                meta: self.make_meta(operator_span),
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // DuckDB POSITIONAL JOIN <factor>: row-position pairing. Like CROSS it never
        // carries a constraint — DuckDB parse-rejects a trailing `ON`/`USING` (left
        // unconsumed here, so it surfaces as trailing input, the CROSS mechanism) and
        // any side keyword between POSITIONAL and JOIN. Same gating and reservation
        // interplay as the ASOF arm above.
        if self.features().join_syntax.positional_join
            && self.peek_is_keyword(Keyword::Positional)?
        {
            let token = self
                .advance()?
                .expect("peek_is_keyword confirmed a POSITIONAL token is present");
            self.expect_keyword(Keyword::Join)?;
            let operator_span = token.span.union(self.preceding_span());
            let relation = self.parse_table_factor()?;
            let operator = JoinOperator::Positional {
                meta: self.make_meta(operator_span),
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // DuckDB (bare) SEMI JOIN | ANTI JOIN <factor> <constraint>: the semi-/anti-
        // join under the REGULAR ref-type (its NATURAL and ASOF compositions are handled
        // by those arms above). SEMI/ANTI are `join_type`s, not sides, so a leading
        // INNER/LEFT/RIGHT/FULL is engine-rejected — this arm only fires on a bare
        // SEMI/ANTI, and the standard side arm below never eats them. Like ASOF the
        // constraint is mandatory (a bare `SEMI JOIN` is an engine syntax error). Same
        // gating + DuckDb ColId/bare-alias reservation interplay as the ASOF arm: under
        // a dialect without the reservation the preceding factor's alias swallows the
        // word first and this fires only after an explicit alias.
        if self.features().join_syntax.semi_anti_join {
            if let Some((semi_anti, kw_span)) = self.eat_semi_anti_keyword()? {
                self.expect_keyword(Keyword::Join)?;
                let factor = self.parse_table_factor()?;
                let relation = self.parse_join_right_operand(factor)?;
                let constraint = self.parse_join_constraint()?;
                if matches!(constraint, JoinConstraint::None { .. }) {
                    return Err(
                        self.unexpected("an `ON` or `USING` constraint after `SEMI`/`ANTI JOIN`")
                    );
                }
                // Widen to cover `constraint` (see the STRAIGHT_JOIN arm above for why).
                let operator_span = kw_span.union(constraint.span());
                let operator =
                    semi_anti.into_operator(false, constraint, self.make_meta(operator_span));
                return Ok(Some(self.finish_join(start, relation, operator)));
            }
        }

        // Spark/Hive/Databricks (LEFT|RIGHT) (SEMI|ANTI) JOIN <factor> <constraint>:
        // the *sided* spelling of the semi-/anti-join — distinct from DuckDB's side-less
        // `SEMI`/`ANTI JOIN` (`SemiAntiSide::Sideless`, handled above) and gated apart on
        // `sided_semi_anti_join` because DuckDB parse-rejects the sided form
        // (engine-probed). Spark requires the explicit side and an `ON`/`USING`
        // constraint, and never composes with `NATURAL`/`ASOF`, so this builds
        // `asof: false` with a mandatory constraint. The two-token lookahead confirms
        // `SEMI`/`ANTI` follows the `LEFT`/`RIGHT` side before consuming anything, so a
        // gated-off preset (or a plain `LEFT`/`RIGHT [OUTER] JOIN`, whose next word is
        // `JOIN`/`OUTER`) leaves the keywords for the standard side arm below.
        // `LEFT`/`RIGHT` are reserved join sides, so the preceding factor's alias can
        // never swallow them (no reservation interplay, unlike the keyword-led ASOF/SEMI
        // pair). All four sided spellings (`LEFT`/`RIGHT` × `SEMI`/`ANTI`) ride this same
        // flag and `SemiAntiSide` axis.
        if self.features().join_syntax.sided_semi_anti_join
            && (self.peek_is_keyword(Keyword::Left)? || self.peek_is_keyword(Keyword::Right)?)
            && (self.peek_nth_is_keyword(1, Keyword::Semi)?
                || self.peek_nth_is_keyword(1, Keyword::Anti)?)
        {
            let side = if self.peek_is_keyword(Keyword::Right)? {
                SemiAntiSide::Right
            } else {
                SemiAntiSide::Left
            };
            let side_token = self
                .advance()?
                .expect("peek_is_keyword confirmed a LEFT/RIGHT token is present");
            let kind = self
                .eat_semi_anti_keyword()?
                .expect("the two-token lookahead confirmed a SEMI/ANTI keyword follows the side")
                .0;
            self.expect_keyword(Keyword::Join)?;
            let factor = self.parse_table_factor()?;
            let relation = self.parse_join_right_operand(factor)?;
            let constraint = self.parse_join_constraint()?;
            if matches!(constraint, JoinConstraint::None { .. }) {
                return Err(self.unexpected(match (side, kind) {
                    (SemiAntiSide::Left, SemiAntiKind::Semi) => {
                        "an `ON` or `USING` constraint after `LEFT SEMI JOIN`"
                    }
                    (SemiAntiSide::Left, SemiAntiKind::Anti) => {
                        "an `ON` or `USING` constraint after `LEFT ANTI JOIN`"
                    }
                    (SemiAntiSide::Right, SemiAntiKind::Semi) => {
                        "an `ON` or `USING` constraint after `RIGHT SEMI JOIN`"
                    }
                    (SemiAntiSide::Right, SemiAntiKind::Anti) => {
                        "an `ON` or `USING` constraint after `RIGHT ANTI JOIN`"
                    }
                    (SemiAntiSide::Sideless, _) => unreachable!("side was set to Left/Right above"),
                }));
            }
            // Widen to cover `constraint` (see the STRAIGHT_JOIN arm above for why).
            let operator_span = side_token.span.union(constraint.span());
            let meta = self.make_meta(operator_span);
            let operator = match kind {
                SemiAntiKind::Semi => JoinOperator::Semi {
                    asof: false,
                    side,
                    constraint,
                    meta,
                },
                SemiAntiKind::Anti => JoinOperator::Anti {
                    asof: false,
                    side,
                    constraint,
                    meta,
                },
            };
            return Ok(Some(self.finish_join(start, relation, operator)));
        }

        // [INNER] JOIN | LEFT|RIGHT|FULL [OUTER] JOIN | JOIN <factor> <constraint>.
        // `explicit` records the written redundant side keyword (`INNER` / `OUTER`).
        let (side, side_span, explicit) = if self.peek_is_keyword(Keyword::Inner)? {
            let token = self
                .advance()?
                .expect("peek_is_keyword confirmed an INNER token is present");
            (JoinSide::Inner, token.span, true)
        } else if let Some((side, span, outer)) = self.eat_join_side()? {
            (side, span, outer)
        } else if self.peek_is_keyword(Keyword::Join)? {
            (JoinSide::Inner, self.current_span()?, false)
        } else {
            return Ok(None);
        };
        self.expect_keyword(Keyword::Join)?;
        let operator_span = side_span.union(self.preceding_span());
        let factor = self.parse_table_factor()?;
        let relation = self.parse_join_right_operand(factor)?;
        let constraint = self.parse_join_constraint()?;
        // Widen to cover `constraint` (see the STRAIGHT_JOIN arm above for why): an
        // `ON`/`USING` constraint is parsed after `operator_span` is captured here and
        // regularly extends past it, and even the field-less `JoinConstraint::None`
        // marker sits at the join point after `relation`, past this keyword-only span.
        let operator_span = operator_span.union(constraint.span());
        let operator = side.into_operator(explicit, constraint, self.make_meta(operator_span));
        Ok(Some(self.finish_join(start, relation, operator)))
    }

    /// After a qualifier-requiring join's right-hand `factor`, absorb any
    /// further unparenthesized join chain that must bind *before* this join's
    /// own `ON`/`USING` — PostgreSQL's `table_ref: ... | joined_table`
    /// right-recursion.
    ///
    /// `a JOIN b JOIN c ON e1 ON e2` right-nests as `a JOIN (b JOIN c ON e1) ON
    /// e2`: an unqualified `a JOIN b` cannot reduce (`join_qual` is mandatory
    /// for a non-`NATURAL`/`CROSS` join) while a `JOIN` keyword still follows,
    /// so the grammar is forced to keep extending the right operand — the
    /// *nearest* `ON`/`USING` therefore closes the innermost, most-recently-
    /// opened join. `b JOIN c ON e1` is read whole here (recursing back through
    /// `parse_join`, one absorbed join per level) and wrapped as one
    /// [`TableFactor::NestedJoin`] factor before the caller reads *its own*
    /// qualifier (`ON e2`); confirmed against `pg_query`'s raw `JoinExpr` tree
    /// for this exact shape (`postgres_stacked_joins_right_nest_like_pg`,
    /// conformance/src/pg.rs).
    ///
    /// When no further join keyword follows, `factor` is already the complete
    /// right operand — the common, unambiguous case (`a JOIN b ON x JOIN c ON
    /// y`, where each `ON` immediately follows its own join) — and is returned
    /// unchanged with no extra allocation.
    ///
    /// Scoped to the qualifier-requiring joins only (`[INNER]`/`LEFT`/`RIGHT`/
    /// `FULL`/bare `JOIN`, all of which call this from the one branch above):
    /// `NATURAL`/`CROSS`/`STRAIGHT_JOIN` never call [`parse_join_constraint`](Self::parse_join_constraint)
    /// and so never hit the "still looking for a qualifier, but a `JOIN`
    /// arrived instead" fork this method resolves — a real `pg_query`-verified
    /// case for those is a separate, unstarted extension of this same idea.
    fn parse_join_right_operand(
        &mut self,
        factor: TableFactor<D::Ext>,
    ) -> ParseResult<TableFactor<D::Ext>> {
        // SQLite's `join-clause` is flat — each join takes exactly one immediately-
        // following constraint — so the PostgreSQL right-nesting that lets a second
        // stacked `ON`/`USING` bind to an outer join (`a JOIN b JOIN c ON p ON q`) is off.
        // The right operand is then not extended, and the second qualifier is left
        // unconsumed, surfacing as the syntax error SQLite reports
        // ([`JoinSyntax::stacked_join_qualifiers`](crate::ast::dialect::TableExpressionSyntax)).
        if !self.features().join_syntax.stacked_join_qualifiers {
            return Ok(factor);
        }
        let Some(first) = self.parse_join()? else {
            return Ok(factor);
        };
        let start = factor.span();
        let mut joins = thin_vec![first];
        while let Some(join) = self.parse_join()? {
            joins.push(join);
        }
        let span = start.union(self.preceding_span());
        let table = TableWithJoins {
            relation: factor,
            joins,
            meta: self.make_meta(span),
        };
        Ok(TableFactor::NestedJoin {
            table: Box::new(table),
            alias: None,
            meta: self.make_meta(span),
        })
    }

    /// Consume an optional outer-join side: `LEFT|RIGHT|FULL [OUTER]`.
    ///
    /// `None` consumes nothing, so a non-side token (e.g. a bare `JOIN`) is left
    /// for the caller. The returned `bool` records whether the optional `OUTER` noise
    /// keyword was written (the side already encodes outerness; the flag is fidelity
    /// only, for a source-faithful re-spell).
    fn eat_join_side(&mut self) -> ParseResult<Option<(JoinSide, Span, bool)>> {
        let start = self.current_span()?;
        let side = if self.eat_keyword(Keyword::Left)? {
            JoinSide::Left
        } else if self.eat_keyword(Keyword::Right)? {
            JoinSide::Right
        } else if self.features().join_syntax.full_outer_join && self.eat_keyword(Keyword::Full)? {
            // MySQL has no `FULL` join; with the gate off, `FULL` is not consumed as a join
            // side, so an already-aliased factor followed by `FULL [OUTER] JOIN` leaves the
            // keyword unconsumed and surfaces as the syntax error MySQL reports
            // ([`JoinSyntax::full_outer_join`]). A bare non-reserved `FULL` was
            // already taken as the preceding factor's alias, so this only governs the
            // join-side reading.
            JoinSide::Full
        } else {
            return Ok(None);
        };
        let outer = self.eat_keyword(Keyword::Outer)?; // optional; the side already encodes outerness
        Ok(Some((side, start.union(self.preceding_span()), outer)))
    }

    /// Consume a DuckDB `SEMI`/`ANTI` join-type keyword, if present.
    ///
    /// `None` consumes nothing. These are `join_type`s (not [`JoinSide`]s), so they
    /// never carry a side and never combine with `OUTER`; the `NATURAL`/`ASOF`
    /// composition and the constraint are handled at each call site.
    fn eat_semi_anti_keyword(&mut self) -> ParseResult<Option<(SemiAntiKind, Span)>> {
        let start = self.current_span()?;
        let kind = if self.eat_keyword(Keyword::Semi)? {
            SemiAntiKind::Semi
        } else if self.eat_keyword(Keyword::Anti)? {
            SemiAntiKind::Anti
        } else {
            return Ok(None);
        };
        Ok(Some((kind, start.union(self.preceding_span()))))
    }

    /// Parse a join constraint: `ON <expr>`, `USING ( <ident> [, …] )`, or none.
    fn parse_join_constraint(&mut self) -> ParseResult<JoinConstraint<D::Ext>> {
        if self.peek_is_keyword(Keyword::On)? {
            let start = self.current_span()?;
            self.advance()?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            Ok(JoinConstraint::On { expr, meta })
        } else if self.peek_is_keyword(Keyword::Using)? {
            let start = self.current_span()?;
            self.advance()?;
            self.expect_punct(Punctuation::LParen, "`(` after `USING`")?;
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `USING` columns")?;
            let alias = if self.peek_is_keyword(Keyword::As)? {
                if !self.features().table_expressions.join_using_alias {
                    return Err(self.unexpected("a join constraint supported by this dialect"));
                }
                self.advance()?;
                Some(self.parse_ident()?)
            } else {
                None
            };
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            Ok(JoinConstraint::Using {
                columns,
                alias,
                meta,
            })
        } else {
            let end = self.preceding_span().end();
            let meta = self.make_meta(Span::new(end, end));
            Ok(JoinConstraint::None { meta })
        }
    }

    /// Finish a [`Join`], spanning from `start` through the last consumed token.
    fn finish_join(
        &mut self,
        start: Span,
        relation: TableFactor<D::Ext>,
        operator: JoinOperator<D::Ext>,
    ) -> Join<D::Ext> {
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Join {
            relation,
            operator,
            meta,
        }
    }

    /// Parse a possibly-dotted object name: `a`, `a.b`, `a.b.c`.
    ///
    /// A dotted part is taken only when a `.` is *followed by a word*, so a
    /// trailing `.*` (a qualified wildcard) is left for the projection grammar and
    /// a stray `.` is not mistaken for a name continuation. Shared by table names
    /// here and column references in [`super::expr`].
    ///
    /// The leading part is gated by the caller for its position (a table name is a
    /// `ColId`, a column reference a `ColId`, …); a continuation part after a `.` is
    /// an attribute/qualified-name component (`ColLabel`), which admits every
    /// keyword — `SELECT t.order` is valid PostgreSQL — so it is parsed with the
    /// label gate.
    pub(super) fn parse_object_name(&mut self) -> ParseResult<ObjectName> {
        self.parse_object_name_with(self.features().reserved_column_name)
    }

    /// The maximum dotted parts a relation (table / index / view) name admits: three
    /// (`catalog.schema.table`) for the catalog-qualified presets, two (`schema.table`)
    /// for SQLite ([`FeatureSet::catalog_qualified_names`](crate::ast::dialect::FeatureSet::catalog_qualified_names)). Column references are a
    /// deeper, separate position and are not bounded here.
    pub(super) fn max_relation_name_parts(&self) -> usize {
        if self.features().catalog_qualified_names {
            3
        } else {
            2
        }
    }

    /// The `expected` text for an over-qualified relation name, matching
    /// [`max_relation_name_parts`](Self::max_relation_name_parts).
    pub(super) fn relation_name_depth_expected(&self) -> &'static str {
        if self.features().catalog_qualified_names {
            "a relation name of at most three parts (catalog.schema.table)"
        } else {
            "a relation name of at most two parts (schema.table)"
        }
    }

    /// Parse a relation (table / index / view) name for a DDL/DML *target* position,
    /// narrowing to SQLite's two-part `schema.table` limit. The catalog-qualified presets
    /// are unchanged: their DDL/DML relation names keep whatever the greedy
    /// [`parse_object_name`](Self::parse_object_name) admits (these positions carry no
    /// three-part cap of their own — unlike the FROM factor and the `TABLE` command, which
    /// use [`parse_relation_name`](Self::parse_relation_name)), so this only sheds SQLite's
    /// `a.b.c` over-acceptance. Column references are never routed here.
    pub(super) fn parse_target_relation_name(&mut self) -> ParseResult<ObjectName> {
        let start = self.current_span()?;
        let name = self.parse_relation_target_name()?;
        if !self.features().catalog_qualified_names && name.0.len() > 2 {
            let span = start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(
                span,
                "a relation name of at most two parts (schema.table)",
                found,
            ));
        }
        Ok(name)
    }

    /// The relation-target object name, admitting SQLite's single-quoted string-literal
    /// identifier spelling in each dotted part (`DELETE FROM 'table1'`, `'schema'.'table'`)
    /// under [`IdentifierSyntax::string_literal_identifiers`](crate::ast::dialect::IdentifierSyntax);
    /// otherwise the plain [`parse_object_name`](Self::parse_object_name).
    ///
    /// Confined to the relation-*target* position (DML/DDL) — the corpus-demanded site
    /// (`delete.test`). A bare string in a FROM factor or a function-name head is left to
    /// the standard grammar: SQLite has no corpus gap for the former and itself
    /// syntax-rejects the latter (`SELECT 'f'(1)`), so the shared object-name grammar is
    /// deliberately not widened.
    fn parse_relation_target_name(&mut self) -> ParseResult<ObjectName> {
        if !self.features().identifier_syntax.string_literal_identifiers {
            return self.parse_object_name();
        }
        let mut parts =
            thin_vec![self.parse_string_or_ident_admitting(self.features().reserved_column_name)?];
        while self.peek_is_punct(Punctuation::Dot)? {
            // A continuation part is a label word or, here, a string-literal name.
            let follows = self.peek_nth(1)?.is_some_and(|token| {
                self.token_can_be_label(token) || token.kind == TokenKind::String
            });
            if !follows {
                break;
            }
            self.advance()?; // `.`
            parts.push(self.parse_string_or_ident_admitting(self.features().reserved_as_label)?);
        }
        Ok(ObjectName(parts))
    }

    /// Parse one name that may be spelled as a single-quoted string literal (SQLite's
    /// string-literal identifier misfeature): a leading plain string constant is folded to
    /// an [`Ident`] with [`QuoteStyle::Single`]/`Double` via
    /// [`parse_string_alias_ident`](Self::parse_string_alias_ident) so the quotes
    /// round-trip, else the position's plain identifier (rejecting `reserved`). The caller
    /// gates the string form on the feature flag; a non-string token always takes the
    /// standard path.
    pub(super) fn parse_string_or_ident_admitting(
        &mut self,
        reserved: KeywordSet,
    ) -> ParseResult<Ident> {
        if let Some(ident) = self.parse_string_alias_ident()? {
            return Ok(ident);
        }
        self.parse_ident_admitting(reserved, "an identifier")
    }

    /// Parse a possibly-dotted name whose *leading* part is gated by `head_reserved`.
    ///
    /// The head's reject set is the caller's position (a table name is a `ColId`; an
    /// unqualified function name is a `function_name`, which admits the
    /// `type_func_name` class — `left(…)`, `join(…)` — that a bare `ColId` rejects).
    /// Continuation parts are attribute components (`ColLabel`), which admit every
    /// keyword, so a qualified `schema.func(…)` gates `schema` as the head and `func`
    /// as a label, matching PostgreSQL's `func_name: ColId indirection`.
    pub(super) fn parse_object_name_with(
        &mut self,
        head_reserved: KeywordSet,
    ) -> ParseResult<ObjectName> {
        let mut parts = thin_vec![self.parse_ident_admitting(head_reserved, "an identifier")?];
        while self.peek_is_punct(Punctuation::Dot)? {
            let follows_identifier = self
                .peek_nth(1)?
                .is_some_and(|token| self.token_can_be_label(token));
            if !follows_identifier {
                break;
            }

            self.advance()?; // `.`
            parts.push(self.parse_as_alias_ident()?);
        }
        Ok(ObjectName(parts))
    }

    /// Parse a single name identifier (`ColId`): a CTE/column/correlation name, a
    /// column-list entry, an `EXTRACT` field, and any other plain name position.
    pub(super) fn parse_ident(&mut self) -> ParseResult<Ident> {
        self.parse_ident_admitting(self.features().reserved_column_name, "an identifier")
    }

    /// Parse a `ColLabel` — an `AS`-introduced alias (`SELECT 1 AS <label>`) and a
    /// dotted-name continuation part (`schema.<part>`, `x.<part>`). PostgreSQL admits
    /// every keyword here (`SELECT a AS select` is valid), so its
    /// [`reserved_as_label`](crate::ast::dialect::FeatureSet::reserved_as_label) is empty;
    /// SQLite draws no `ColId`/`ColLabel` split and rejects its reserved words in this
    /// position too (`SELECT 1 AS delete`, `SELECT x.update`, `FROM schema.case`), so the
    /// reject set is dialect data, not a hardcoded [`KeywordSet::EMPTY`].
    pub(super) fn parse_as_alias_ident(&mut self) -> ParseResult<Ident> {
        self.parse_ident_admitting(self.features().reserved_as_label, "an alias")
    }

    /// Parse a bare alias (`BareColLabel`), written without `AS`, which rejects the
    /// `AS_LABEL` keywords (`OVER`, `FILTER`, …).
    pub(super) fn parse_bare_alias_ident(&mut self) -> ParseResult<Ident> {
        self.parse_ident_admitting(self.features().reserved_bare_alias, "an alias")
    }

    /// Parse a `type_function_name` identifier — PostgreSQL's function-name /
    /// routine-parameter-name class (`unreserved ∪ type_func_name`, the complement of
    /// [`ColId`](Self::parse_ident)). It admits a `type_func_name` keyword like `left`
    /// but rejects a `col_name` keyword like `int`, so a lone `int` in a `CREATE
    /// FUNCTION` parameter position is unambiguously the type, not the name — the reject
    /// set is [`reserved_type_name`](crate::ast::dialect::FeatureSet::reserved_type_name)
    /// (`col_name ∪ reserved`), which PostgreSQL shares between its type-name and
    /// `type_function_name` productions.
    pub(super) fn parse_type_function_name_ident(&mut self) -> ParseResult<Ident> {
        self.parse_ident_admitting(self.features().reserved_type_name, "a parameter name")
    }

    /// Parse a single identifier whose position rejects the keywords in `reserved`,
    /// preserving exact source spelling even when a keyword token is used.
    ///
    /// A quoted identifier (`"x"`, `` `x` ``, `[x]`) carries its delimiters and raw
    /// doubled escapes in the token, so it is materialized here: the
    /// [`QuoteStyle`] is read from the opening byte and the body is unescaped (see
    /// [`materialize_quoted_ident`]) before interning, with the style preserved on
    /// the [`Ident`] so it round-trips. Anything outside the position's identifier
    /// set is a precise error.
    ///
    /// A `U&"..."` Unicode-escaped identifier is the exception: the tokenizer emits it as
    /// one [`QuotedIdent`] token spanning the `U&` prefix, and this is the single choke
    /// point (every name/alias/column-ref position funnels through here) where its
    /// trailing `UESCAPE 'c'` clause is folded and its `<esc>XXXX` escapes are decoded —
    /// eagerly, so a malformed escape or an illegal `UESCAPE` delimiter is a parse reject,
    /// as PostgreSQL's `base_yylex` wrapper makes it. The interned symbol is the *decoded*
    /// name, so the identifier compares equal to its plain double-quoted equivalent —
    /// `U&"d0061ta"` is `data`, exactly as the engine resolves it — while the style is
    /// [`QuoteStyle::UnicodeDouble`], which retains the `U&"…" [UESCAPE 'c']` source
    /// spelling on the span so a source-fidelity render round-trips it verbatim.
    ///
    /// [`QuotedIdent`]: crate::tokenizer::TokenKind::QuotedIdent
    pub(super) fn parse_ident_admitting(
        &mut self,
        reserved: KeywordSet,
        expected: &'static str,
    ) -> ParseResult<Ident> {
        match self.peek()? {
            Some(token) if self.token_admissible(token, reserved) => {
                self.advance()?;
                let mut span = token.span;
                let (sym, quote) = match token.kind {
                    TokenKind::Word | TokenKind::Keyword(_) => {
                        (self.intern_identifier(token), QuoteStyle::None)
                    }
                    TokenKind::QuotedIdent if is_unicode_ident(self.span_text(token.span)) => {
                        // Fold any `UESCAPE 'c'` clause into the span first (the escape
                        // character it resolves is what the body decodes against), then
                        // decode+validate once — the reject boundary PostgreSQL enforces
                        // at parse time.
                        span = self.consume_optional_uescape(token.span)?;
                        let value =
                            materialize_unicode_ident(self.span_text(span)).ok_or_else(|| {
                                LexError::new(LexErrorKind::InvalidEscapeSequence, span)
                            })?;
                        (self.intern_text(&value), QuoteStyle::UnicodeDouble)
                    }
                    TokenKind::QuotedIdent => {
                        let (quote, text) = materialize_quoted_ident(self.span_text(token.span));
                        (self.intern_text(&text), quote)
                    }
                    TokenKind::Number => {
                        unreachable!("token_admissible rejected numbers")
                    }
                    TokenKind::String => {
                        unreachable!("token_admissible rejected strings")
                    }
                    TokenKind::Operator(_) => {
                        unreachable!("token_admissible rejected operators")
                    }
                    TokenKind::Punctuation(_) => {
                        unreachable!("token_admissible rejected punctuation")
                    }
                    TokenKind::Parameter => {
                        unreachable!("token_admissible rejected parameters")
                    }
                    TokenKind::PositionalColumn => {
                        unreachable!("token_admissible rejected positional columns")
                    }
                    TokenKind::Variable | TokenKind::StageReference => {
                        unreachable!("token_admissible rejected session variables")
                    }
                    TokenKind::Unknown => {
                        unreachable!("token_admissible rejected unknown tokens")
                    }
                };
                // `span` covers a folded `U&"..." UESCAPE 'c'` identifier; for every
                // other form it is just `token.span`.
                let meta = self.make_meta(span);
                Ok(Ident { sym, quote, meta })
            }
            _ => Err(self.unexpected(expected)),
        }
    }

    pub(super) fn parse_optional_table_alias(&mut self) -> ParseResult<Option<Box<TableAlias>>> {
        self.parse_optional_table_alias_impl(false)
    }

    /// [`parse_optional_table_alias`](Self::parse_optional_table_alias) for a *base table*
    /// factor. MySQL admits a column-list alias on a derived table / subquery but not on a
    /// base table (the base-vs-derived split
    /// [`base_table_alias_column_lists`](crate::ast::dialect::TableExpressionSyntax)), so the
    /// base-table position gates the column list separately from the derived positions.
    pub(super) fn parse_optional_base_table_alias(
        &mut self,
    ) -> ParseResult<Option<Box<TableAlias>>> {
        self.parse_optional_table_alias_impl(true)
    }

    /// True when a bare `WINDOW` at a table-alias position opens the SELECT-level
    /// named-window clause (`WINDOW <name> AS (…)`) rather than naming the table
    /// `window`. The head of a `windowdefn` is `<name> AS`, so that two-token lookahead
    /// is the exact discriminator SQLite's grammar uses (probed): `FROM t WINDOW w AS (…)`
    /// is the clause, while `FROM t window` / `FROM t window WHERE …` keep `window` as a
    /// bare correlation alias.
    fn peek_opens_window_clause(&mut self) -> ParseResult<bool> {
        if !self.peek_is_keyword(Keyword::Window)? {
            return Ok(false);
        }
        let name_ok = self
            .peek_nth(1)?
            .is_some_and(|token| self.token_can_be_column_name(token));
        Ok(name_ok && self.peek_nth_is_keyword(2, Keyword::As)?)
    }

    /// True when a bare `SETTINGS` at a table-alias position opens the ClickHouse
    /// query-tail `SETTINGS name = value, …` clause rather than naming the table
    /// `settings`. `SETTINGS` is a contextual (unreserved) keyword, so without this
    /// guard the bare-alias parser would swallow it as the table's correlation name and
    /// the tail clause could never parse (`FROM t SETTINGS x = 1`). The `name =` head is
    /// the exact discriminator — a bare `FROM t settings` with no following `ident =`
    /// stays an ordinary alias. Gated by
    /// [`QueryTailSyntax::settings_clause`](crate::ast::dialect::SelectSyntax) — off for
    /// every preset but Lenient, where `settings` remains a plain alias everywhere.
    pub(super) fn peek_opens_settings_clause(&mut self) -> ParseResult<bool> {
        if !self.features().query_tail_syntax.settings_clause {
            return Ok(false);
        }
        if !self.peek_is_contextual_keyword("SETTINGS")? {
            return Ok(false);
        }
        let name_ok = self
            .peek_nth(1)?
            .is_some_and(|token| self.token_can_be_column_name(token));
        Ok(name_ok && self.peek_nth_is_op(2, Operator::Eq)?)
    }

    /// The identifier set admitted for a ClickHouse `FORMAT` output-format name:
    /// [`token_can_be_column_name`](Self::token_can_be_column_name)'s `ColId` set widened
    /// by the otherwise-reserved `NULL` keyword, because `Null` is a documented ClickHouse
    /// output format (`FORMAT Null` discards the result). Clause keywords (`WHERE`, `JOIN`,
    /// …) stay rejected, so a bare `format` before one is still read as an alias.
    fn format_name_reserved(&self) -> KeywordSet {
        self.features()
            .reserved_column_name
            .difference(KeywordSet::from_keywords(&[Keyword::Null]))
    }

    /// Parse a ClickHouse `FORMAT` output-format name — a bare identifier that also admits
    /// the otherwise-reserved `NULL` keyword ([`format_name_reserved`](Self::format_name_reserved)).
    /// The name preserves its source spelling (`JSON` stays `JSON`, not lowered), so it
    /// round-trips case-sensitively as ClickHouse requires.
    pub(super) fn parse_format_name(&mut self) -> ParseResult<Ident> {
        self.parse_ident_admitting(self.format_name_reserved(), "a format name")
    }

    /// True when a bare `FORMAT` at an alias position opens the ClickHouse query-tail
    /// `FORMAT <name>` clause rather than naming the table `format`. `FORMAT` is a
    /// contextual (unreserved) keyword, so without this guard the bare-alias parser would
    /// swallow it as the table's correlation name and the tail clause could never parse
    /// (`FROM t FORMAT JSON`). The discriminator is a following format-name identifier — a
    /// bare `FROM t format` with no name head stays an ordinary alias. Gated by
    /// [`QueryTailSyntax::format_clause`](crate::ast::dialect::SelectSyntax) — off for every
    /// preset but Lenient, where `format` remains a plain alias everywhere.
    pub(super) fn peek_opens_format_clause(&mut self) -> ParseResult<bool> {
        if !self.features().query_tail_syntax.format_clause {
            return Ok(false);
        }
        if !self.peek_is_contextual_keyword("FORMAT")? {
            return Ok(false);
        }
        let reserved = self.format_name_reserved();
        Ok(self
            .peek_nth(1)?
            .is_some_and(|token| self.token_admissible(token, reserved)))
    }

    /// True when the cursor opens the Oracle-style hierarchical query clause — a
    /// `START WITH …` or `CONNECT BY …` head — under
    /// [`SelectSyntax::connect_by_clause`](crate::ast::dialect::SelectSyntax). A bare
    /// (`AS`-less) alias position declines these so the clause is reachable after
    /// `FROM t` (`FROM t CONNECT BY …`), the same clause-opening decline the `WINDOW`,
    /// `SETTINGS`, and `FORMAT` guards use. `CONNECT`/`START` are reserved words in the
    /// enabling engines (Snowflake reserves both by ANSI), but modelling the decline here
    /// rather than in the reserved sets keeps the clause reachable under the permissive
    /// Lenient union too, which reserves nothing.
    pub(super) fn peek_opens_hierarchical_clause(&mut self) -> ParseResult<bool> {
        if !self.features().select_syntax.connect_by_clause {
            return Ok(false);
        }
        Ok(
            (self.peek_is_keyword(Keyword::Start)?
                && self.peek_nth_is_keyword(1, Keyword::With)?)
                || (self.peek_is_keyword(Keyword::Connect)?
                    && self.peek_nth_is_keyword(1, Keyword::By)?),
        )
    }

    /// True when a bare (`AS`-less) `INDEXED` at a base-table alias position opens SQLite's
    /// `INDEXED BY <index>` index directive rather than naming the table `indexed`, under
    /// [`TableExpressionSyntax::indexed_by`](crate::ast::dialect::TableExpressionSyntax). A
    /// bare correlation-alias position declines the bare `INDEXED` keyword so the directive
    /// is reachable after `FROM t` (`FROM t INDEXED BY ix`) — the `WINDOW`/`CONNECT BY`
    /// clause-decline precedent. Unlike those clause guards, the decline fires on the bare
    /// `INDEXED` keyword *alone* (not a two-token head): SQLite tokenizes `INDEXED` as a
    /// keyword in this one position and commits to the directive, so a bare `FROM t indexed`
    /// with no trailing `BY` is an engine reject (measured on rusqlite 3.53.2), reproduced
    /// here by declining the alias and letting [`parse_optional_indexed_by`](Self::parse_optional_indexed_by)
    /// require the `BY`. Everywhere else `INDEXED` stays a plain identifier (it is not in any
    /// reserved set): `SELECT indexed`, `FROM t AS indexed`, and `indexed INT` all parse. The
    /// `NOT INDEXED` form needs no guard — `NOT` is already reserved as a bare SQLite alias.
    fn peek_opens_indexed_by_clause(&mut self) -> ParseResult<bool> {
        if !self.features().table_expressions.indexed_by {
            return Ok(false);
        }
        self.peek_is_keyword(Keyword::Indexed)
    }

    fn parse_optional_table_alias_impl(
        &mut self,
        base_table: bool,
    ) -> ParseResult<Option<Box<TableAlias>>> {
        // A table/correlation alias is a `ColId` (PostgreSQL's `alias_clause`),
        // even after `AS` — so `parse_ident` (the `ColId` gate) is correct here, and
        // a bare alias must not swallow a following `JOIN`/`WHERE`/… (all rejected
        // as a `ColId`). SQLite is the exception: its bare (`AS`-less) alias is the
        // narrow `ids` class (not the `nm` name class), so the JOIN keywords — admissible
        // as a *table name* — are reserved as a *bare alias*, keeping `FROM t cross JOIN u`
        // a CROSS JOIN. The `bare_table_alias_is_bare_label` gate routes the bare path to
        // the stricter `reserved_bare_alias` set there while the explicit `AS` alias keeps
        // the permissive `ColId` set (SQLite's `AS nm` admits the JOIN keywords).
        let explicit = self.eat_keyword(Keyword::As)?;
        // A bare (`AS`-less) `WINDOW <name> AS (…)` at an alias position opens the
        // SELECT-level named-window clause, not a correlation alias named `window` —
        // decline so the enclosing body reaches `parse_window_clause`. Where `WINDOW` is
        // non-reserved (SQLite) it is otherwise swallowed as a bare alias and the clause
        // never parses; where it is reserved (pg/MySQL, `*_RESERVED_KEYWORDS`) the
        // reserved-set check below already declines it, so this guard is a harmless no-op.
        // The discriminator is the windowdefn head `<name> AS`: SQLite still admits
        // `window` as a bare alias elsewhere (`FROM t window`, `FROM t window WHERE …`,
        // `FROM t window GROUP BY …`) — only the clause shape wins (probed on 3.43/3.53).
        // An explicit `AS window` stays an alias (SQLite rejects `FROM t AS window w AS
        // (…)`), so this never fires on the `explicit` path.
        if !explicit && self.peek_opens_window_clause()? {
            return Ok(None);
        }
        // A bare `SETTINGS name = …` at an alias position opens the ClickHouse query-tail
        // settings clause, not a correlation alias named `settings` — decline so the
        // enclosing query reaches `parse_settings`.
        if !explicit && self.peek_opens_settings_clause()? {
            return Ok(None);
        }
        // A bare `FORMAT <name>` at an alias position opens the ClickHouse query-tail
        // format clause, not a correlation alias named `format` — decline so the enclosing
        // query reaches `parse_format`.
        if !explicit && self.peek_opens_format_clause()? {
            return Ok(None);
        }
        // A bare `START WITH …` / `CONNECT BY …` at an alias position opens the
        // hierarchical query clause, not a correlation alias named `start`/`connect` —
        // decline so the enclosing body reaches `parse_hierarchical_clause`.
        if !explicit && self.peek_opens_hierarchical_clause()? {
            return Ok(None);
        }
        // A bare `INDEXED` at a base-table alias position opens SQLite's `INDEXED BY …` index
        // directive, not a correlation alias named `indexed` — decline so the base-table
        // factor reaches `parse_optional_indexed_by`. Restricted to the base-table position
        // (`base_table`): the directive attaches only to a real table, so a derived/function
        // factor's alias still admits `indexed`.
        if !explicit && base_table && self.peek_opens_indexed_by_clause()? {
            return Ok(None);
        }
        let bare_reserved = if self
            .features()
            .table_expressions
            .bare_table_alias_is_bare_label
        {
            self.features().reserved_bare_alias
        } else {
            self.features().reserved_column_name
        };
        if !explicit
            && !self
                .peek()?
                .is_some_and(|token| self.token_admissible(token, bare_reserved))
        {
            return Ok(None);
        }
        let start = self.current_span()?;
        // DuckDB admits a string-literal alias name, but only after an explicit `AS`
        // (`FROM t AS 't'`; a bare `FROM t 't'` is an engine reject, kept out by the
        // leading-string guard above, which never treats a string as a column start).
        let name = if explicit {
            self.parse_table_alias_ident()?
        } else {
            self.parse_ident_admitting(bare_reserved, "an identifier")?
        };
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            let table_expressions = self.features().table_expressions;
            // A base table admits a column-list alias only where both gates allow it: the
            // dialect must support column-list aliases at all, and the base-table position
            // must not be one MySQL restricts (it admits them on derived tables only).
            if !table_expressions.table_alias_column_lists
                || (base_table && !table_expressions.base_table_alias_column_lists)
            {
                return Err(self.unexpected("a table alias supported by this dialect"));
            }
            self.advance()?;
            let columns = self.parse_comma_separated(Self::parse_table_alias_ident)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the table alias column list",
            )?;
            columns
        } else {
            ThinVec::new()
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(TableAlias {
            name,
            columns,
            spelling: alias_spelling(explicit),
            meta,
        })))
    }

    /// Parse one table-alias identifier — the correlation name or a column-list entry —
    /// admitting DuckDB's single-quoted string spelling (`AS 't'`, `('k')`) under
    /// [`TableExpressionSyntax::string_literal_aliases`](crate::ast::dialect::TableExpressionSyntax),
    /// reusing the projection alias's [`parse_string_alias_ident`](Self::parse_string_alias_ident);
    /// otherwise a plain `ColId` identifier.
    fn parse_table_alias_ident(&mut self) -> ParseResult<Ident> {
        if self.features().table_expressions.string_literal_aliases {
            if let Some(ident) = self.parse_string_alias_ident()? {
                return Ok(ident);
            }
        }
        self.parse_ident()
    }
}

/// Map "was an explicit `AS` written" to the alias-introducer spelling tag: an
/// explicit keyword is [`AliasSpelling::As`], its absence a bare alias. The prefix
/// forms are tagged at their own parse sites, not here.
pub(crate) const fn alias_spelling(explicit_as: bool) -> AliasSpelling {
    if explicit_as {
        AliasSpelling::As
    } else {
        AliasSpelling::Bare
    }
}

/// Strip a quoted identifier's delimiters and collapse its doubled close into the
/// materialized [`QuoteStyle`] and body (the lexer keeps the delimiters
/// and raw escapes, so this happens at materialization, not in the tokenizer).
///
/// `raw` is the full delimited span the tokenizer produced, so its first byte is the
/// opening delimiter — `"`/`` ` ``/`[` — and its last byte the matching close. Every
/// SQL quote delimiter is a single ASCII byte. An embedded close is escaped by
/// doubling it (`""`, `` `` ``, `]]`); the asymmetric bracket style doubles only the
/// close `]`, never the open `[`. The borrow-only path mirrors `Render for Ident`:
/// the common identifier embeds no delimiter, so only a body that actually contains a
/// doubled close pays for an owned, collapsed copy.
pub(super) fn materialize_quoted_ident(raw: &str) -> (QuoteStyle, Cow<'_, str>) {
    let (style, doubled, single) = match raw.as_bytes()[0] {
        b'"' => (QuoteStyle::Double, "\"\"", "\""),
        b'`' => (QuoteStyle::Backtick, "``", "`"),
        b'[' => (QuoteStyle::Bracket, "]]", "]"),
        _ => unreachable!("a QuotedIdent span starts with a configured opening delimiter"),
    };
    let inner = &raw[1..raw.len() - 1];
    let text = if inner.contains(doubled) {
        Cow::Owned(inner.replace(doubled, single))
    } else {
        Cow::Borrowed(inner)
    };
    (style, text)
}

#[cfg(test)]
mod tests {
    use super::materialize_quoted_ident;
    use crate::ast::dialect::{
        FeatureDelta, FeatureSet, IdentifierQuote, Keyword, KeywordSet, TableExpressionSyntax,
        TableFactorSyntax,
    };
    use crate::ast::{
        ArgSyntax, DataType, DerivedSpelling, Expr, IndexHintAction, IndexHintKeyword,
        IndexHintScope, JoinConstraint, JoinOperator, NoExt, OnlySyntax, QuoteStyle,
        RelationInheritance, Resolver as _, SelectItem, SemiStructuredPathSegment, SetExpr,
        ShowRefKind, ShowRefTarget, Statement, TableFactor, TableVersion,
    };
    use crate::dialect::{BigQuery, Databricks, DuckDb, Lenient, Mssql, Postgres};
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};
    use crate::render::Renderer;
    use std::borrow::Cow;

    /// DuckDB's prefix colon alias in `FROM`: `FROM b : a` aliases the relation `a` as `b`
    /// (the alias precedes the table). It folds onto the factor's ordinary alias slot, and
    /// the gate is honoured as DATA — off for PostgreSQL, on for DuckDB.
    #[test]
    fn prefix_colon_alias_in_from_binds_alias_to_relation() {
        let parsed = parse_with("SELECT * FROM b : a", crate::ParseConfig::new(DuckDb))
            .expect("FROM prefix colon alias parses");
        let TableFactor::Table { name, alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a base-table factor");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "a");
        let alias = alias
            .as_ref()
            .expect("the prefix alias is attached to the relation");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "b");
        assert!(alias.columns.is_empty());

        // Off for PostgreSQL: a `:` at a table-factor head is a clean reject.
        assert!(parse_with("SELECT * FROM b : a", crate::ParseConfig::new(Postgres)).is_err());
    }

    /// The prefix alias is mutually exclusive with a trailing alias — DuckDB rejects both
    /// `FROM b : a AS c` and the bare `FROM b : a c` (probed on 1.5.4). The value's own
    /// alias parse still runs, so the second alias is caught as an unexpected token.
    #[test]
    fn prefix_colon_alias_in_from_rejects_a_trailing_alias() {
        assert!(parse_with("SELECT * FROM b : a AS c", crate::ParseConfig::new(DuckDb)).is_err());
        assert!(parse_with("SELECT * FROM b : a c", crate::ParseConfig::new(DuckDb)).is_err());
        // No chaining: `FROM b : c : a` is a syntax error (the inner factor is parsed
        // without re-entering the prefix-alias head).
        assert!(parse_with("SELECT * FROM b : c : a", crate::ParseConfig::new(DuckDb)).is_err());
    }

    /// `bare_table_alias_is_bare_label` is honoured as DATA, not a hardcoded dialect check:
    /// two FeatureSets identical but for the flag — with `cross` configured admissible as a
    /// `ColId` yet reserved as a bare alias (the SQLite shape) — resolve a bare `FROM t
    /// cross` oppositely. Flag OFF (`ColId`) reads `cross` as `t`'s correlation alias; flag
    /// ON (`BareColLabel`) reserves it there, leaving it for the CROSS JOIN grammar. This is
    /// the parser-side twin of the oracle-verified SQLite position-matrix in the conformance
    /// suite — proving the shared reserved-set table drives both, so a future dialect that
    /// flips the flag cannot bypass it.
    #[test]
    fn bare_table_alias_flag_routes_to_the_bare_alias_set() {
        // `cross`: dropped from the ColId reject set (admissible as a table/column name),
        // added to the bare-alias reject set (reserved as a bare alias) — the SQLite shape.
        const CROSS: KeywordSet = KeywordSet::from_keywords(&[Keyword::Cross]);
        const BASE: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .reserved_column_name(FeatureSet::ANSI.reserved_column_name.difference(CROSS))
                .reserved_bare_alias(FeatureSet::ANSI.reserved_bare_alias.union(CROSS)),
        );
        const FLAG_ON: FeatureSet = BASE.with(FeatureDelta::EMPTY.table_expressions(
            TableExpressionSyntax {
                bare_table_alias_is_bare_label: true,
                ..TableExpressionSyntax::ANSI
            },
        ));
        const OFF: FeatureDialect = FeatureDialect { features: &BASE };
        const ON: FeatureDialect = FeatureDialect { features: &FLAG_ON };
        let (off, on) = (OFF, ON);

        // Flag OFF (`ColId`): `cross` is an admissible bare `ColId` alias — `FROM t AS cross`.
        assert!(
            parse_with("SELECT * FROM t cross", crate::ParseConfig::new(off)).is_ok(),
            "flag off: a bare table alias is a ColId, so `cross` aliases `t`",
        );
        // Flag ON (`BareColLabel`): `cross` is reserved as a bare alias, so it is left for
        // the join grammar — a bare `FROM t cross` is an incomplete CROSS JOIN and rejects.
        assert!(
            parse_with("SELECT * FROM t cross", crate::ParseConfig::new(on)).is_err(),
            "flag on: `cross` is reserved as a bare alias, so `FROM t cross` is not an alias",
        );
        // ...and a complete CROSS JOIN parses under the flag (the guard).
        assert!(
            parse_with("SELECT * FROM t cross JOIN u", crate::ParseConfig::new(on),).is_ok(),
            "flag on: `cross` in join position is still the CROSS JOIN",
        );
        // The explicit `AS` alias stays a `ColId` either way — `cross` is admissible there.
        assert!(
            parse_with("SELECT * FROM t AS cross", crate::ParseConfig::new(on),).is_ok(),
            "the explicit `AS` alias keeps the ColId set, which admits `cross`",
        );
    }

    /// The [`TableVersion`] of the sole base-table factor, or a panic when the factor is
    /// not a versioned base table.
    fn sole_table_version(parsed: &Parsed) -> &TableVersion {
        let TableFactor::Table { version, .. } = &select_of(parsed).from[0].relation else {
            panic!("expected a base-table factor");
        };
        version
            .as_deref()
            .expect("the base table carries a version modifier")
    }

    /// `FOR SYSTEM_TIME AS OF <expr>` is a typed [`TableVersion::ForSystemTimeAsOf`] — the
    /// BigQuery/MSSQL point-in-time snapshot — and round-trips. Available to a planner
    /// without string inspection (the ticket's acceptance).
    #[test]
    fn table_version_for_system_time_as_of_is_typed() {
        let sql = "SELECT * FROM t FOR SYSTEM_TIME AS OF '2020-01-01'";

        let bq = parse_with(sql, crate::ParseConfig::new(BigQuery))
            .expect("BigQuery parses FOR SYSTEM_TIME AS OF");
        assert!(matches!(
            sole_table_version(&bq),
            TableVersion::ForSystemTimeAsOf { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient).render_parsed(&bq).expect("renders"),
            sql,
        );

        // MSSQL admits the same `AS OF` spelling (its temporal-table subset).
        let mssql = parse_with(sql, crate::ParseConfig::new(Mssql))
            .expect("MSSQL parses FOR SYSTEM_TIME AS OF");
        assert!(matches!(
            sole_table_version(&mssql),
            TableVersion::ForSystemTimeAsOf { .. }
        ));

        // The version binds before the alias, so a trailing correlation alias still attaches
        // to the table (the common MSSQL `… AS OF <ts> AS e` shape) and round-trips.
        let aliased = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME AS OF '2020-01-01' AS e",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses a version modifier followed by an alias");
        let TableFactor::Table {
            version: Some(_),
            alias: Some(alias),
            ..
        } = &select_of(&aliased).from[0].relation
        else {
            panic!("expected a versioned base table with a trailing alias");
        };
        assert_eq!(aliased.resolver().resolve(alias.name.sym), "e");
    }

    /// MSSQL's five temporal-table forms each land as their own [`TableVersion`] variant
    /// and round-trip. The `BETWEEN … AND` / `FROM … TO` endpoints are parsed at the
    /// range-predicate power, so the separating `AND`/`TO` stays a delimiter rather than
    /// folding into the endpoint expression (the round-trip proves it).
    #[test]
    fn table_version_mssql_temporal_forms_are_typed() {
        let from_to = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME FROM '2020-01-01' TO '2021-01-01'",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses FOR SYSTEM_TIME FROM..TO");
        assert!(matches!(
            sole_table_version(&from_to),
            TableVersion::ForSystemTimeFromTo { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&from_to)
                .expect("renders"),
            "SELECT * FROM t FOR SYSTEM_TIME FROM '2020-01-01' TO '2021-01-01'",
        );

        let between = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME BETWEEN '2020-01-01' AND '2021-01-01'",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses FOR SYSTEM_TIME BETWEEN..AND");
        assert!(matches!(
            sole_table_version(&between),
            TableVersion::ForSystemTimeBetween { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&between)
                .expect("renders"),
            "SELECT * FROM t FOR SYSTEM_TIME BETWEEN '2020-01-01' AND '2021-01-01'",
        );

        let contained = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME CONTAINED IN ('2020-01-01', '2021-01-01')",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses FOR SYSTEM_TIME CONTAINED IN");
        assert!(matches!(
            sole_table_version(&contained),
            TableVersion::ForSystemTimeContainedIn { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&contained)
                .expect("renders"),
            "SELECT * FROM t FOR SYSTEM_TIME CONTAINED IN ('2020-01-01', '2021-01-01')",
        );

        let all = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME ALL",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses FOR SYSTEM_TIME ALL");
        assert!(matches!(
            sole_table_version(&all),
            TableVersion::ForSystemTimeAll { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient).render_parsed(&all).expect("renders"),
            "SELECT * FROM t FOR SYSTEM_TIME ALL",
        );
    }

    /// Databricks/Delta `VERSION AS OF` / `TIMESTAMP AS OF` land as their own
    /// [`TableVersion`] variants and round-trip.
    #[test]
    fn table_version_databricks_version_and_timestamp_as_of() {
        let version = parse_with(
            "SELECT * FROM t VERSION AS OF 5",
            crate::ParseConfig::new(Databricks),
        )
        .expect("Databricks parses VERSION AS OF");
        assert!(matches!(
            sole_table_version(&version),
            TableVersion::VersionAsOf { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&version)
                .expect("renders"),
            "SELECT * FROM t VERSION AS OF 5",
        );

        let timestamp = parse_with(
            "SELECT * FROM t TIMESTAMP AS OF '2020-01-01'",
            crate::ParseConfig::new(Databricks),
        )
        .expect("Databricks parses TIMESTAMP AS OF");
        assert!(matches!(
            sole_table_version(&timestamp),
            TableVersion::TimestampAsOf { .. }
        ));
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&timestamp)
                .expect("renders"),
            "SELECT * FROM t TIMESTAMP AS OF '2020-01-01'",
        );
    }

    /// The `VERSION`/`TIMESTAMP AS OF` forms commit only after the full `AS OF` lookahead,
    /// so a bare `FROM t VERSION` still reads `VERSION` as the correlation alias rather than
    /// beginning a truncated clause.
    #[test]
    fn bare_version_word_still_aliases_the_table() {
        let parsed = parse_with(
            "SELECT * FROM t VERSION",
            crate::ParseConfig::new(Databricks),
        )
        .expect("bare VERSION aliases `t`");
        let TableFactor::Table {
            version: None,
            alias: Some(alias),
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a base table aliased `version`, with no version modifier");
        };
        // Databricks preserves identifier casing, so the bare alias keeps its source spelling.
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "VERSION");
    }

    /// The table-factor `FOR SYSTEM_TIME` and the query-level MSSQL `FOR XML` are
    /// position-partitioned, not token-partitioned: under the MSSQL preset — which enables
    /// both — they coexist in one query. The table version consumes `FOR SYSTEM_TIME AS OF
    /// …` immediately after the table name; the query tail's `FOR XML` is read only after
    /// the whole `FROM`/`WHERE`. The trigger `FOR SYSTEM_TIME` (vs `FOR XML`) is what keeps
    /// them apart, so neither clause ever swallows the other.
    #[test]
    fn table_version_and_query_level_for_xml_coexist_under_mssql() {
        let parsed = parse_with(
            "SELECT * FROM t FOR SYSTEM_TIME AS OF '2020-01-01' FOR XML AUTO",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses a table `FOR SYSTEM_TIME` alongside a query-level `FOR XML`");
        assert!(matches!(
            sole_table_version(&parsed),
            TableVersion::ForSystemTimeAsOf { .. }
        ));
        // A query-level `FOR XML` with no table version still parses — the version parser
        // declined it (the word after `FOR` is not `SYSTEM_TIME`), leaving it for the tail.
        assert!(
            parse_with(
                "SELECT * FROM t FOR XML AUTO",
                crate::ParseConfig::new(Mssql)
            )
            .is_ok()
        );
    }

    /// Off under a gate-off preset (PostgreSQL): the version keyword is left unconsumed, so
    /// `FOR SYSTEM_TIME` falls to the query-level `FOR` (locking) parser — which rejects it
    /// — and the `VERSION`/`TIMESTAMP AS OF` forms leave the trailing `AS OF …` unparsed.
    #[test]
    fn table_version_gate_off_rejects() {
        assert!(
            parse_with(
                "SELECT * FROM t FOR SYSTEM_TIME AS OF '2020-01-01'",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL has no table version: `FOR SYSTEM_TIME` is not a valid query-level FOR",
        );
        assert!(
            parse_with(
                "SELECT * FROM t VERSION AS OF 5",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL has no table version: the trailing `AS OF 5` is unparsed input",
        );
        assert!(
            parse_with(
                "SELECT * FROM t TIMESTAMP AS OF '2020-01-01'",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL has no table version: the trailing `AS OF …` is unparsed input",
        );
    }

    /// A PartiQL / SUPER table-position JSON path (`FROM src[0].a`) parses into a typed
    /// [`SemiStructuredPathSegment`] list on the base table and round-trips — the ticket's
    /// acceptance. The bracket-index root and the trailing `.key` / `[index]` suffixes each
    /// land as their own segment, so a planner reads the navigation directly. On for
    /// Snowflake / Redshift (sqlparser-rs's `supports_partiql`).
    #[test]
    fn table_json_path_partiql_navigation_is_typed() {
        use crate::ast::SemiStructuredPathSegment as Seg;
        use crate::dialect::{Redshift, Snowflake};
        use crate::parser::Dialect;

        fn sole_json_path(parsed: &Parsed) -> &[SemiStructuredPathSegment<NoExt>] {
            let TableFactor::Table { json_path, .. } = &select_of(parsed).from[0].relation else {
                panic!("expected a base table with a JSON path");
            };
            json_path
        }

        // Both `supports_partiql` presets read the path identically.
        fn check<D: Dialect<Ext = NoExt> + Copy>(dialect: D) {
            let sql = "SELECT * FROM src[0].a";
            let parsed = parse_with(sql, crate::ParseConfig::new(dialect))
                .expect("PartiQL table path parses");
            let path = sole_json_path(&parsed);
            assert!(
                matches!(path, [Seg::Index { .. }, Seg::Key { .. }]),
                "root is a bracket index, then a dot key",
            );
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
            );

            // A deeper alternating path round-trips segment-for-segment.
            let deep = "SELECT * FROM src[0].a[1].b";
            let parsed = parse_with(deep, crate::ParseConfig::new(dialect))
                .expect("deep PartiQL table path parses");
            assert!(matches!(
                sole_json_path(&parsed),
                [
                    Seg::Index { .. },
                    Seg::Key { .. },
                    Seg::Index { .. },
                    Seg::Key { .. }
                ]
            ));
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("renders"),
                deep,
            );

            // The path binds before the alias, so a trailing correlation alias still attaches.
            let aliased = parse_with(
                "SELECT * FROM src[0].a AS x",
                crate::ParseConfig::new(dialect),
            )
            .expect("path then alias parses");
            let TableFactor::Table {
                json_path,
                alias: Some(_),
                ..
            } = &select_of(&aliased).from[0].relation
            else {
                panic!("expected a path-bearing base table with a trailing alias");
            };
            assert_eq!(json_path.len(), 2);

            // A dotted `FROM src.a.b` has no leading `[`, so it stays a compound relation
            // name with an empty path — not a JSON path.
            let compound = parse_with("SELECT * FROM src.a.b", crate::ParseConfig::new(dialect))
                .expect("compound name parses");
            assert!(sole_json_path(&compound).is_empty());
        }

        check(Snowflake);
        check(Redshift);
    }

    /// The table-position JSON path is gated: with `table_json_path` off (ANSI/PostgreSQL)
    /// the `[` after a table name is unconsumed input, so the construct is a clean parse
    /// divergence rather than a silent accept.
    #[test]
    fn table_json_path_gate_off_rejects() {
        use crate::dialect::Ansi;

        assert!(
            parse_with("SELECT * FROM src[0].a", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI has no PartiQL table path: the trailing `[0].a` is unparsed input",
        );
        assert!(
            parse_with("SELECT * FROM src[0].a", crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL has no PartiQL table path: the trailing `[0].a` is unparsed input",
        );
    }

    /// ANSI plus MySQL-style backtick identifier quoting, for the symmetric
    /// non-`"` style ANSI itself does not enable.
    const BACKTICK: FeatureSet = FeatureSet::ANSI
        .with(FeatureDelta::EMPTY.identifier_quotes(&[IdentifierQuote::Symmetric('`')]));

    /// ANSI plus T-SQL-style bracket identifier quoting, the asymmetric style where
    /// only the close `]` doubles to escape.
    const BRACKET: FeatureSet = FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_quotes(&[
        IdentifierQuote::Asymmetric {
            open: '[',
            close: ']',
        },
    ]));

    const BACKTICK_DIALECT: FeatureDialect = FeatureDialect {
        features: &BACKTICK,
    };

    const BRACKET_DIALECT: FeatureDialect = FeatureDialect { features: &BRACKET };

    /// The single, unaliased column reference of a one-item projection.
    fn sole_column_name(parsed: &Parsed) -> &crate::ast::ObjectName {
        let [
            SelectItem::Expr {
                expr: Expr::Column { name, .. },
                alias: None,
                ..
            },
        ] = select_of(parsed).projection.as_slice()
        else {
            panic!("expected a single unaliased column projection");
        };
        name
    }

    /// The name of the sole plain table factor in the `FROM` clause.
    fn sole_table_name(parsed: &Parsed) -> &crate::ast::ObjectName {
        let TableFactor::Table { name, .. } = &select_of(parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        name
    }

    fn select_of(parsed: &Parsed) -> &crate::ast::Select<NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        select
    }

    #[test]
    fn left_join_using_carries_the_using_columns() {
        let parsed = parse_with(
            "SELECT * FROM t1 LEFT JOIN t2 USING (id)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("LEFT JOIN ... USING parses");
        let from = &select_of(&parsed).from[0];
        assert_eq!(from.joins.len(), 1);
        let JoinOperator::LeftOuter {
            constraint: JoinConstraint::Using { columns, .. },
            ..
        } = &from.joins[0].operator
        else {
            panic!("expected a LEFT OUTER join with a USING constraint");
        };
        assert_eq!(columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(columns[0].sym), "id");
    }

    #[test]
    fn left_outer_join_records_the_optional_outer_keyword() {
        // `LEFT OUTER JOIN` and `LEFT JOIN` are the same canonical operator variant; the
        // `outer` bool tag records the written keyword so a source-fidelity render
        // replays it (spelling-tags-keyword-operator-batch).
        let outer = parse_with(
            "SELECT * FROM t1 LEFT OUTER JOIN t2 ON t1.a = t2.a",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("LEFT OUTER JOIN parses");
        assert!(matches!(
            select_of(&outer).from[0].joins[0].operator,
            JoinOperator::LeftOuter {
                outer: true,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));
        let bare = parse_with(
            "SELECT * FROM t1 LEFT JOIN t2 ON t1.a = t2.a",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("LEFT JOIN parses");
        assert!(matches!(
            select_of(&bare).from[0].joins[0].operator,
            JoinOperator::LeftOuter { outer: false, .. },
        ));
    }

    #[test]
    fn cross_join_has_no_constraint() {
        let parsed = parse_with(
            "SELECT * FROM t1 CROSS JOIN t2",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CROSS JOIN parses");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::Cross { .. },
        ));
    }

    #[test]
    fn natural_right_join_records_side_and_natural_constraint() {
        let parsed = parse_with(
            "SELECT * FROM t1 NATURAL RIGHT JOIN t2",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("NATURAL RIGHT JOIN parses");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::RightOuter {
                constraint: JoinConstraint::Natural { .. },
                ..
            },
        ));
    }

    #[test]
    fn natural_inner_join_accepts_the_explicit_inner_keyword() {
        // PostgreSQL's `join_type` admits an explicit `INNER` after `NATURAL`; it is the
        // default side, so `NATURAL INNER JOIN` and a bare `NATURAL JOIN` yield the same
        // canonical `Inner` operator with a `Natural` constraint.
        let parsed = parse_with(
            "SELECT * FROM t1 NATURAL INNER JOIN t2",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("NATURAL INNER JOIN parses");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::Inner {
                straight: false,
                // The explicit `INNER` keyword is recorded so it round-trips.
                inner: true,
                constraint: JoinConstraint::Natural { .. },
                ..
            },
        ));
    }

    #[test]
    fn sqlite_natural_cross_join_normalizes_to_natural_inner() {
        use crate::dialect::{Postgres, Sqlite};

        // SQLite `NATURAL CROSS JOIN` is a natural inner join: `CROSS` is `INNER`'s
        // optimizer-hint spelling and `NATURAL` supplies the constraint (engine-probed on
        // rusqlite — shared-column equijoin shape, not the cross product). It normalizes
        // into the canonical Inner+Natural shape and renders back as `NATURAL JOIN`; the
        // round-trip oracle compares structure, so the elided `CROSS` re-parses identically.
        // Shape + render are asserted under `Lenient` (a render dialect that also enables the
        // flag); `Sqlite` — the corpus dialect, not a Tier-1 render target — is checked for
        // parse acceptance directly.
        let parsed = parse_with(
            "SELECT * FROM t1 NATURAL CROSS JOIN t2",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses NATURAL CROSS JOIN");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::Inner {
                straight: false,
                constraint: JoinConstraint::Natural { .. },
                ..
            },
        ));
        let rendered = Renderer::new(Lenient)
            .render_parsed(&parsed)
            .expect("renders the normalized join");
        assert_eq!(rendered, "SELECT * FROM t1 NATURAL JOIN t2");

        parse_with(
            "SELECT * FROM t1 NATURAL CROSS JOIN t2",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("SQLite parses NATURAL CROSS JOIN");
        // The gate is genuinely required: PostgreSQL parse-rejects it (engine-probed on 16),
        // its `NATURAL` arm falling through to the mandatory `JOIN` on the `CROSS` token.
        parse_with(
            "SELECT * FROM t1 NATURAL CROSS JOIN t2",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL rejects NATURAL CROSS JOIN");
    }

    #[test]
    fn mysql_straight_join_is_inner_with_the_straight_tag() {
        use crate::dialect::{Ansi, MySql};

        // MySQL `STRAIGHT_JOIN` is the canonical inner join with the `straight` surface
        // tag set; it carries the same `ON` constraint grammar as a bare `JOIN`.
        let parsed = parse_with(
            "SELECT * FROM a STRAIGHT_JOIN b ON a.x = b.x",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses STRAIGHT_JOIN");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::Inner {
                straight: true,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // Gated: without the dialect flag `STRAIGHT_JOIN` is a non-reserved word the
        // table factor takes as an alias, so `b` is leftover input and the parse fails.
        parse_with(
            "SELECT * FROM a STRAIGHT_JOIN b ON a.x = b.x",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no STRAIGHT_JOIN join operator");
    }

    #[test]
    fn duckdb_asof_join_records_the_kind_and_requires_a_constraint() {
        use crate::ast::AsOfJoinKind;
        use crate::dialect::DuckDb;

        // The `ASOF` prefix threads through the side spelling; the bare form is the
        // canonical Inner kind, a spelled side is recorded on the one `AsOf` variant.
        let parsed = parse_with(
            "SELECT * FROM a ASOF JOIN b ON a.t >= b.t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses ASOF JOIN");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::AsOf {
                kind: AsOfJoinKind::Inner,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));
        let parsed = parse_with(
            "SELECT * FROM a ASOF LEFT JOIN b ON a.t >= b.t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses ASOF LEFT JOIN");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::AsOf {
                kind: AsOfJoinKind::Left,
                ..
            },
        ));

        // DuckDB *parse*-rejects a constraint-less ASOF join (unlike the sibling
        // side-join arm, whose `None` fallback exists for MySQL's bare inner join).
        let err = parse_with(
            "SELECT * FROM a ASOF JOIN b",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("a bare ASOF JOIN needs its constraint");
        assert_eq!(
            err.expected.as_str(),
            "an `ON` or `USING` constraint after `ASOF JOIN`"
        );
    }

    #[test]
    fn duckdb_positional_join_takes_no_constraint() {
        use crate::dialect::DuckDb;

        let parsed = parse_with(
            "SELECT * FROM a POSITIONAL JOIN b",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses POSITIONAL JOIN");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::Positional { .. },
        ));
        // A trailing constraint is left unconsumed and rejects as leftover input,
        // the CROSS JOIN mechanism (DuckDB parse-rejects it too).
        parse_with(
            "SELECT * FROM a POSITIONAL JOIN b ON a.t = b.t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("POSITIONAL JOIN takes no ON constraint");
    }

    #[test]
    fn cross_apply_is_the_apply_operator_over_a_right_table_factor() {
        use crate::ast::ApplyKind;

        // MSSQL CROSS APPLY over a derived table: a lateral-correlated join, so the
        // right factor may reference the left source (`t.id`). It carries no
        // constraint (the correlation lives in the right factor's own references) and
        // is a distinct operator from CROSS JOIN — the `Apply` variant, not `Cross`.
        let parsed = parse_with(
            "SELECT * FROM t CROSS APPLY (SELECT x FROM u WHERE u.id = t.id) AS s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses CROSS APPLY over a derived table");
        let join = &select_of(&parsed).from[0].joins[0];
        assert!(matches!(
            join.operator,
            JoinOperator::Apply {
                kind: ApplyKind::Cross,
                ..
            },
        ));
        // The right factor is a derived table; CROSS APPLY implies the correlation, so
        // no explicit `LATERAL` keyword is written (`lateral: false`), unlike a
        // `CROSS JOIN LATERAL (…)` right factor.
        assert!(matches!(
            join.relation,
            TableFactor::Derived { lateral: false, .. },
        ));

        // A table-valued function is the other MSSQL APPLY right operand.
        let parsed = parse_with(
            "SELECT * FROM t CROSS APPLY f(t.x) AS g",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses CROSS APPLY over a table function");
        let join = &select_of(&parsed).from[0].joins[0];
        assert!(matches!(
            join.operator,
            JoinOperator::Apply {
                kind: ApplyKind::Cross,
                ..
            },
        ));
        assert!(matches!(
            join.relation,
            TableFactor::Function { lateral: false, .. },
        ));
    }

    #[test]
    fn cross_apply_stays_distinct_from_cross_join_and_lateral() {
        // The `CROSS` keyword forks on the following word: `APPLY` builds the MSSQL
        // `Apply` operator, `JOIN` the ordinary `Cross`. A `CROSS JOIN LATERAL (…)`
        // right factor is still the `Cross` operator carrying a `lateral` derived table
        // — the composition CROSS APPLY collapses into one operator, so the two must
        // not alias.
        let apply = parse_with(
            "SELECT * FROM a CROSS APPLY (SELECT 1) AS s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("CROSS APPLY parses");
        assert!(matches!(
            select_of(&apply).from[0].joins[0].operator,
            JoinOperator::Apply { .. },
        ));

        let cross = parse_with(
            "SELECT * FROM a CROSS JOIN b",
            crate::ParseConfig::new(Lenient),
        )
        .expect("CROSS JOIN parses");
        assert!(matches!(
            select_of(&cross).from[0].joins[0].operator,
            JoinOperator::Cross { .. },
        ));

        let cross_lateral = parse_with(
            "SELECT * FROM a CROSS JOIN LATERAL (SELECT 1) AS s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("CROSS JOIN LATERAL parses");
        let join = &select_of(&cross_lateral).from[0].joins[0];
        assert!(matches!(join.operator, JoinOperator::Cross { .. }));
        assert!(matches!(
            join.relation,
            TableFactor::Derived { lateral: true, .. },
        ));
    }

    #[test]
    fn cross_apply_round_trips() {
        for sql in [
            "SELECT * FROM t CROSS APPLY (SELECT x FROM u WHERE u.id = t.id) AS s",
            "SELECT * FROM t CROSS APPLY f(t.x) AS g",
            "SELECT * FROM t CROSS APPLY (SELECT 1) AS s CROSS APPLY g(s.y) AS h",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn cross_apply_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // No MSSQL preset exists yet and the other engines parse-reject `APPLY` in join
        // position, so the gate is off everywhere but Lenient. With it off, `APPLY`
        // falls to `expect_keyword(JOIN)` after `CROSS`, which rejects it.
        let sql = "SELECT * FROM t CROSS APPLY (SELECT 1) AS s";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects CROSS APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects CROSS APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects CROSS APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects CROSS APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects CROSS APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts CROSS APPLY"
        );
    }

    #[test]
    fn outer_apply_is_the_apply_operator_over_a_right_table_factor() {
        use crate::ast::ApplyKind;

        // MSSQL OUTER APPLY over a derived table: the outer-preserving flavour of the
        // same lateral-correlated join, so the right factor may reference the left
        // source (`t.id`). It carries no constraint (the correlation lives in the right
        // factor's own references) and records the `Outer` flavour on the shared
        // `Apply` operator — not a separate variant.
        let parsed = parse_with(
            "SELECT * FROM t OUTER APPLY (SELECT x FROM u WHERE u.id = t.id) AS s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses OUTER APPLY over a derived table");
        let join = &select_of(&parsed).from[0].joins[0];
        assert!(matches!(
            join.operator,
            JoinOperator::Apply {
                kind: ApplyKind::Outer,
                ..
            },
        ));
        // The right factor is a derived table; OUTER APPLY implies the correlation, so
        // no explicit `LATERAL` keyword is written (`lateral: false`).
        assert!(matches!(
            join.relation,
            TableFactor::Derived { lateral: false, .. },
        ));

        // A table-valued function is the other MSSQL APPLY right operand.
        let parsed = parse_with(
            "SELECT * FROM t OUTER APPLY f(t.x) AS g",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses OUTER APPLY over a table function");
        let join = &select_of(&parsed).from[0].joins[0];
        assert!(matches!(
            join.operator,
            JoinOperator::Apply {
                kind: ApplyKind::Outer,
                ..
            },
        ));
        assert!(matches!(
            join.relation,
            TableFactor::Function { lateral: false, .. },
        ));
    }

    #[test]
    fn outer_apply_stays_distinct_from_outer_join_and_lateral() {
        use crate::ast::ApplyKind;

        // A bare leading `OUTER` heads the MSSQL `Apply` operator, while the `OUTER` in
        // `LEFT|FULL OUTER JOIN` is the trailing spelling of a side (eaten and dropped
        // inside `eat_join_side`) — so `OUTER APPLY` must not alias with any outer side
        // join, and a `LEFT JOIN LATERAL (…)` right factor stays the ordinary side
        // operator carrying a `lateral` derived table.
        let apply = parse_with(
            "SELECT * FROM a OUTER APPLY (SELECT 1) AS s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("OUTER APPLY parses");
        assert!(matches!(
            select_of(&apply).from[0].joins[0].operator,
            JoinOperator::Apply {
                kind: ApplyKind::Outer,
                ..
            },
        ));

        let left_outer = parse_with(
            "SELECT * FROM a LEFT OUTER JOIN b ON a.id = b.id",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT OUTER JOIN parses");
        assert!(matches!(
            select_of(&left_outer).from[0].joins[0].operator,
            JoinOperator::LeftOuter { .. },
        ));

        let full_outer = parse_with(
            "SELECT * FROM a FULL OUTER JOIN b ON a.id = b.id",
            crate::ParseConfig::new(Lenient),
        )
        .expect("FULL OUTER JOIN parses");
        assert!(matches!(
            select_of(&full_outer).from[0].joins[0].operator,
            JoinOperator::FullOuter { .. },
        ));

        let left_lateral = parse_with(
            "SELECT * FROM a LEFT JOIN LATERAL (SELECT 1) AS s ON TRUE",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT JOIN LATERAL parses");
        let join = &select_of(&left_lateral).from[0].joins[0];
        assert!(matches!(join.operator, JoinOperator::LeftOuter { .. }));
        assert!(matches!(
            join.relation,
            TableFactor::Derived { lateral: true, .. },
        ));
    }

    #[test]
    fn outer_apply_round_trips() {
        for sql in [
            "SELECT * FROM t OUTER APPLY (SELECT x FROM u WHERE u.id = t.id) AS s",
            "SELECT * FROM t OUTER APPLY f(t.x) AS g",
            "SELECT * FROM t CROSS APPLY (SELECT 1) AS s OUTER APPLY g(s.y) AS h",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn outer_apply_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // No MSSQL preset exists yet and the other engines parse-reject `APPLY` in join
        // position, so the gate is off everywhere but Lenient. With it off the
        // two-token lookahead never fires, `OUTER` is left unconsumed at the join head,
        // the chain ends, and the trailing `OUTER APPLY …` rejects as leftover input.
        let sql = "SELECT * FROM t OUTER APPLY (SELECT 1) AS s";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects OUTER APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects OUTER APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects OUTER APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects OUTER APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects OUTER APPLY"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts OUTER APPLY"
        );
    }

    #[test]
    fn left_semi_is_the_sided_semi_operator_with_a_constraint() {
        use crate::ast::SemiAntiSide;

        // Spark/Hive `LEFT SEMI JOIN`: the sided spelling of the semi-join, recorded on
        // the shared `Semi` operator as `side: Left` (not a separate variant). Spark
        // requires an `ON`/`USING` constraint and never composes with `ASOF`, so the
        // operator carries `asof: false` and the parsed constraint.
        let on = parse_with(
            "SELECT * FROM a LEFT SEMI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT SEMI JOIN ... ON");
        assert!(matches!(
            select_of(&on).from[0].joins[0].operator,
            JoinOperator::Semi {
                asof: false,
                side: SemiAntiSide::Left,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // The other Spark constraint spelling — `USING (...)`.
        let using = parse_with(
            "SELECT * FROM a LEFT SEMI JOIN b USING (i)",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT SEMI JOIN ... USING");
        assert!(matches!(
            select_of(&using).from[0].joins[0].operator,
            JoinOperator::Semi {
                asof: false,
                side: SemiAntiSide::Left,
                constraint: JoinConstraint::Using { .. },
                ..
            },
        ));
    }

    #[test]
    fn left_semi_is_distinct_from_sideless_semi() {
        use crate::ast::SemiAntiSide;

        // The ticket's core distinctness: DuckDB's side-less `SEMI JOIN` and Spark's
        // `LEFT SEMI JOIN` are the same operator differing only on the `side` axis, so
        // they must not collapse. Under Lenient (both gates on) the two spellings record
        // `Sideless` vs `Left`. The side-less form on a bare factor would alias `a` (the
        // ANSI reserved model Lenient keeps), so the explicit `AS x` lets the semi arm
        // fire; `LEFT` is a reserved side keyword, so it never aliases.
        let sideless = parse_with(
            "SELECT * FROM a AS x SEMI JOIN b ON x.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses side-less SEMI JOIN after an explicit alias");
        assert!(matches!(
            select_of(&sideless).from[0].joins[0].operator,
            JoinOperator::Semi {
                side: SemiAntiSide::Sideless,
                ..
            },
        ));

        let left = parse_with(
            "SELECT * FROM a LEFT SEMI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT SEMI JOIN");
        assert!(matches!(
            select_of(&left).from[0].joins[0].operator,
            JoinOperator::Semi {
                side: SemiAntiSide::Left,
                ..
            },
        ));
    }

    #[test]
    fn left_semi_stays_distinct_from_left_join() {
        // The leading `LEFT` also heads `LEFT [OUTER] JOIN`; the following `SEMI` keyword
        // forks the two. Without `SEMI` the standard side arm builds `LeftOuter`, so the
        // sided-semi lookahead must not steal a plain outer join.
        let left_join = parse_with(
            "SELECT * FROM a LEFT JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT JOIN parses");
        assert!(matches!(
            select_of(&left_join).from[0].joins[0].operator,
            JoinOperator::LeftOuter { .. },
        ));

        let left_outer = parse_with(
            "SELECT * FROM a LEFT OUTER JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT OUTER JOIN parses");
        assert!(matches!(
            select_of(&left_outer).from[0].joins[0].operator,
            JoinOperator::LeftOuter { .. },
        ));
    }

    #[test]
    fn left_semi_round_trips() {
        for sql in [
            "SELECT * FROM a LEFT SEMI JOIN b ON a.i = b.i",
            "SELECT * FROM a LEFT SEMI JOIN b USING (i)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn left_semi_requires_a_constraint() {
        // Spark parse-rejects a bare `LEFT SEMI JOIN` with no `ON`/`USING`, mirroring the
        // side-less form's mandatory constraint. The unconsumed factor tail surfaces the
        // explicit constraint-required error.
        assert!(
            parse_with(
                "SELECT * FROM a LEFT SEMI JOIN b",
                crate::ParseConfig::new(Lenient)
            )
            .is_err(),
            "a bare LEFT SEMI JOIN without a constraint rejects"
        );
    }

    #[test]
    fn left_semi_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // The sided spelling rides `sided_semi_anti_join`, on only for Lenient. With it
        // off, `LEFT` is read as a plain outer-join side and the following `SEMI` is
        // leftover input -> reject. DuckDb is the load-bearing case: it has
        // `semi_anti_join` ON (side-less `SEMI JOIN` parses) yet still rejects the sided
        // spelling (engine-probed), proving the two gates are genuinely separate and the
        // side-less flag does not leak the sided form.
        let sql = "SELECT * FROM a LEFT SEMI JOIN b ON a.i = b.i";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects LEFT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects LEFT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects LEFT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects LEFT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects the sided LEFT SEMI JOIN despite accepting side-less SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts LEFT SEMI JOIN"
        );
    }

    #[test]
    fn left_anti_is_the_sided_anti_operator_with_a_constraint() {
        use crate::ast::SemiAntiSide;

        // Spark/Hive `LEFT ANTI JOIN`: the sided spelling of the anti-join, recorded on
        // the shared `Anti` operator as `side: Left` (not a separate variant), mirroring
        // `LEFT SEMI JOIN`. Spark requires an `ON`/`USING` constraint and never composes
        // with `ASOF`, so the operator carries `asof: false` and the parsed constraint.
        let on = parse_with(
            "SELECT * FROM a LEFT ANTI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT ANTI JOIN ... ON");
        assert!(matches!(
            select_of(&on).from[0].joins[0].operator,
            JoinOperator::Anti {
                asof: false,
                side: SemiAntiSide::Left,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // The other Spark constraint spelling — `USING (...)`.
        let using = parse_with(
            "SELECT * FROM a LEFT ANTI JOIN b USING (i)",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT ANTI JOIN ... USING");
        assert!(matches!(
            select_of(&using).from[0].joins[0].operator,
            JoinOperator::Anti {
                asof: false,
                side: SemiAntiSide::Left,
                constraint: JoinConstraint::Using { .. },
                ..
            },
        ));
    }

    #[test]
    fn left_anti_is_distinct_from_sideless_anti() {
        use crate::ast::SemiAntiSide;

        // DuckDB's side-less `ANTI JOIN` and Spark's `LEFT ANTI JOIN` are the same
        // operator differing only on the `side` axis, so they must not collapse. Under
        // Lenient (both gates on) the two spellings record `Sideless` vs `Left`. The
        // side-less form on a bare factor would alias `a` (the ANSI reserved model Lenient
        // keeps), so the explicit `AS x` lets the anti arm fire; `LEFT` is a reserved side
        // keyword, so it never aliases.
        let sideless = parse_with(
            "SELECT * FROM a AS x ANTI JOIN b ON x.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses side-less ANTI JOIN after an explicit alias");
        assert!(matches!(
            select_of(&sideless).from[0].joins[0].operator,
            JoinOperator::Anti {
                side: SemiAntiSide::Sideless,
                ..
            },
        ));

        let left = parse_with(
            "SELECT * FROM a LEFT ANTI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses LEFT ANTI JOIN");
        assert!(matches!(
            select_of(&left).from[0].joins[0].operator,
            JoinOperator::Anti {
                side: SemiAntiSide::Left,
                ..
            },
        ));
    }

    #[test]
    fn left_anti_stays_distinct_from_left_join() {
        // The leading `LEFT` also heads `LEFT [OUTER] JOIN`; the following `ANTI` keyword
        // forks the two. Without `ANTI` the standard side arm builds `LeftOuter`, so the
        // sided-anti lookahead must not steal a plain outer join.
        let left_join = parse_with(
            "SELECT * FROM a LEFT JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT JOIN parses");
        assert!(matches!(
            select_of(&left_join).from[0].joins[0].operator,
            JoinOperator::LeftOuter { .. },
        ));

        let left_outer = parse_with(
            "SELECT * FROM a LEFT OUTER JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("LEFT OUTER JOIN parses");
        assert!(matches!(
            select_of(&left_outer).from[0].joins[0].operator,
            JoinOperator::LeftOuter { .. },
        ));
    }

    #[test]
    fn left_anti_round_trips() {
        for sql in [
            "SELECT * FROM a LEFT ANTI JOIN b ON a.i = b.i",
            "SELECT * FROM a LEFT ANTI JOIN b USING (i)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn left_anti_requires_a_constraint() {
        // Spark parse-rejects a bare `LEFT ANTI JOIN` with no `ON`/`USING`, mirroring the
        // side-less form's mandatory constraint.
        assert!(
            parse_with(
                "SELECT * FROM a LEFT ANTI JOIN b",
                crate::ParseConfig::new(Lenient)
            )
            .is_err(),
            "a bare LEFT ANTI JOIN without a constraint rejects"
        );
    }

    #[test]
    fn left_anti_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // The sided spelling rides `sided_semi_anti_join`, on only for Lenient. With it
        // off, `LEFT` is read as a plain outer-join side and the following `ANTI` is
        // leftover input -> reject. DuckDb is the load-bearing case: it has
        // `semi_anti_join` ON (side-less `ANTI JOIN` parses) yet still rejects the sided
        // spelling (engine-probed on planner-parity-join-left-semi), proving the two gates
        // are genuinely separate and the side-less flag does not leak the sided form.
        let sql = "SELECT * FROM a LEFT ANTI JOIN b ON a.i = b.i";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects LEFT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects LEFT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects LEFT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects LEFT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects the sided LEFT ANTI JOIN despite accepting side-less ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts LEFT ANTI JOIN"
        );
    }

    #[test]
    fn right_semi_is_the_sided_semi_operator_with_a_constraint() {
        use crate::ast::SemiAntiSide;

        // Spark/Hive `RIGHT SEMI JOIN`: the right-sided spelling of the semi-join, recorded
        // on the shared `Semi` operator as `side: Right` (not a separate variant), mirroring
        // `LEFT SEMI JOIN`. Spark requires an `ON`/`USING` constraint and never composes
        // with `ASOF`, so the operator carries `asof: false` and the parsed constraint.
        let on = parse_with(
            "SELECT * FROM a RIGHT SEMI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT SEMI JOIN ... ON");
        assert!(matches!(
            select_of(&on).from[0].joins[0].operator,
            JoinOperator::Semi {
                asof: false,
                side: SemiAntiSide::Right,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // The other Spark constraint spelling — `USING (...)`.
        let using = parse_with(
            "SELECT * FROM a RIGHT SEMI JOIN b USING (i)",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT SEMI JOIN ... USING");
        assert!(matches!(
            select_of(&using).from[0].joins[0].operator,
            JoinOperator::Semi {
                asof: false,
                side: SemiAntiSide::Right,
                constraint: JoinConstraint::Using { .. },
                ..
            },
        ));
    }

    #[test]
    fn right_semi_is_distinct_from_sideless_semi() {
        use crate::ast::SemiAntiSide;

        // DuckDB's side-less `SEMI JOIN` and Spark's `RIGHT SEMI JOIN` are the same
        // operator differing only on the `side` axis, so they must not collapse. Under
        // Lenient (both gates on) the two spellings record `Sideless` vs `Right`. The
        // side-less form on a bare factor would alias `a` (the ANSI reserved model Lenient
        // keeps), so the explicit `AS x` lets the semi arm fire; `RIGHT` is a reserved side
        // keyword, so it never aliases.
        let sideless = parse_with(
            "SELECT * FROM a AS x SEMI JOIN b ON x.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses side-less SEMI JOIN after an explicit alias");
        assert!(matches!(
            select_of(&sideless).from[0].joins[0].operator,
            JoinOperator::Semi {
                side: SemiAntiSide::Sideless,
                ..
            },
        ));

        let right = parse_with(
            "SELECT * FROM a RIGHT SEMI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT SEMI JOIN");
        assert!(matches!(
            select_of(&right).from[0].joins[0].operator,
            JoinOperator::Semi {
                side: SemiAntiSide::Right,
                ..
            },
        ));
    }

    #[test]
    fn right_semi_stays_distinct_from_right_join() {
        // The leading `RIGHT` also heads `RIGHT [OUTER] JOIN`; the following `SEMI` keyword
        // forks the two. Without `SEMI` the standard side arm builds `RightOuter`, so the
        // sided-semi lookahead must not steal a plain outer join.
        let right_join = parse_with(
            "SELECT * FROM a RIGHT JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("RIGHT JOIN parses");
        assert!(matches!(
            select_of(&right_join).from[0].joins[0].operator,
            JoinOperator::RightOuter { .. },
        ));

        let right_outer = parse_with(
            "SELECT * FROM a RIGHT OUTER JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("RIGHT OUTER JOIN parses");
        assert!(matches!(
            select_of(&right_outer).from[0].joins[0].operator,
            JoinOperator::RightOuter { .. },
        ));
    }

    #[test]
    fn right_semi_round_trips() {
        for sql in [
            "SELECT * FROM a RIGHT SEMI JOIN b ON a.i = b.i",
            "SELECT * FROM a RIGHT SEMI JOIN b USING (i)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn right_semi_requires_a_constraint() {
        // Spark parse-rejects a bare `RIGHT SEMI JOIN` with no `ON`/`USING`, mirroring the
        // side-less form's mandatory constraint.
        assert!(
            parse_with(
                "SELECT * FROM a RIGHT SEMI JOIN b",
                crate::ParseConfig::new(Lenient)
            )
            .is_err(),
            "a bare RIGHT SEMI JOIN without a constraint rejects"
        );
    }

    #[test]
    fn right_semi_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // The sided spelling rides `sided_semi_anti_join`, on only for Lenient. With it
        // off, `RIGHT` is read as a plain outer-join side and the following `SEMI` is
        // leftover input -> reject. DuckDb is the load-bearing case: it has
        // `semi_anti_join` ON (side-less `SEMI JOIN` parses) yet still rejects the sided
        // spelling (engine-probed on planner-parity-join-left-semi), proving the two gates
        // are genuinely separate and the side-less flag does not leak the sided form.
        let sql = "SELECT * FROM a RIGHT SEMI JOIN b ON a.i = b.i";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects RIGHT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects RIGHT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects RIGHT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects RIGHT SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects the sided RIGHT SEMI JOIN despite accepting side-less SEMI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts RIGHT SEMI JOIN"
        );
    }

    #[test]
    fn right_anti_is_the_sided_anti_operator_with_a_constraint() {
        use crate::ast::SemiAntiSide;

        // Spark/Hive `RIGHT ANTI JOIN`: the right-sided spelling of the anti-join, recorded
        // on the shared `Anti` operator as `side: Right` (not a separate variant), mirroring
        // `LEFT ANTI JOIN`. Spark requires an `ON`/`USING` constraint and never composes
        // with `ASOF`, so the operator carries `asof: false` and the parsed constraint.
        let on = parse_with(
            "SELECT * FROM a RIGHT ANTI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT ANTI JOIN ... ON");
        assert!(matches!(
            select_of(&on).from[0].joins[0].operator,
            JoinOperator::Anti {
                asof: false,
                side: SemiAntiSide::Right,
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // The other Spark constraint spelling — `USING (...)`.
        let using = parse_with(
            "SELECT * FROM a RIGHT ANTI JOIN b USING (i)",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT ANTI JOIN ... USING");
        assert!(matches!(
            select_of(&using).from[0].joins[0].operator,
            JoinOperator::Anti {
                asof: false,
                side: SemiAntiSide::Right,
                constraint: JoinConstraint::Using { .. },
                ..
            },
        ));
    }

    #[test]
    fn right_anti_is_distinct_from_sideless_anti() {
        use crate::ast::SemiAntiSide;

        // DuckDB's side-less `ANTI JOIN` and Spark's `RIGHT ANTI JOIN` are the same
        // operator differing only on the `side` axis, so they must not collapse. Under
        // Lenient (both gates on) the two spellings record `Sideless` vs `Right`. The
        // side-less form on a bare factor would alias `a` (the ANSI reserved model Lenient
        // keeps), so the explicit `AS x` lets the anti arm fire; `RIGHT` is a reserved side
        // keyword, so it never aliases.
        let sideless = parse_with(
            "SELECT * FROM a AS x ANTI JOIN b ON x.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses side-less ANTI JOIN after an explicit alias");
        assert!(matches!(
            select_of(&sideless).from[0].joins[0].operator,
            JoinOperator::Anti {
                side: SemiAntiSide::Sideless,
                ..
            },
        ));

        let right = parse_with(
            "SELECT * FROM a RIGHT ANTI JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses RIGHT ANTI JOIN");
        assert!(matches!(
            select_of(&right).from[0].joins[0].operator,
            JoinOperator::Anti {
                side: SemiAntiSide::Right,
                ..
            },
        ));
    }

    #[test]
    fn right_anti_stays_distinct_from_right_join() {
        // The leading `RIGHT` also heads `RIGHT [OUTER] JOIN`; the following `ANTI` keyword
        // forks the two. Without `ANTI` the standard side arm builds `RightOuter`, so the
        // sided-anti lookahead must not steal a plain outer join.
        let right_join = parse_with(
            "SELECT * FROM a RIGHT JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("RIGHT JOIN parses");
        assert!(matches!(
            select_of(&right_join).from[0].joins[0].operator,
            JoinOperator::RightOuter { .. },
        ));

        let right_outer = parse_with(
            "SELECT * FROM a RIGHT OUTER JOIN b ON a.i = b.i",
            crate::ParseConfig::new(Lenient),
        )
        .expect("RIGHT OUTER JOIN parses");
        assert!(matches!(
            select_of(&right_outer).from[0].joins[0].operator,
            JoinOperator::RightOuter { .. },
        ));
    }

    #[test]
    fn right_anti_round_trips() {
        for sql in [
            "SELECT * FROM a RIGHT ANTI JOIN b ON a.i = b.i",
            "SELECT * FROM a RIGHT ANTI JOIN b USING (i)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn right_anti_requires_a_constraint() {
        // Spark parse-rejects a bare `RIGHT ANTI JOIN` with no `ON`/`USING`, mirroring the
        // side-less form's mandatory constraint.
        assert!(
            parse_with(
                "SELECT * FROM a RIGHT ANTI JOIN b",
                crate::ParseConfig::new(Lenient)
            )
            .is_err(),
            "a bare RIGHT ANTI JOIN without a constraint rejects"
        );
    }

    #[test]
    fn right_anti_is_gated_off_elsewhere() {
        use crate::dialect::{Ansi, MySql, Sqlite};

        // The sided spelling rides `sided_semi_anti_join`, on only for Lenient. With it
        // off, `RIGHT` is read as a plain outer-join side and the following `ANTI` is
        // leftover input -> reject. DuckDb is the load-bearing case: it has
        // `semi_anti_join` ON (side-less `ANTI JOIN` parses) yet still rejects the sided
        // spelling (engine-probed on planner-parity-join-left-semi), proving the two gates
        // are genuinely separate and the side-less flag does not leak the sided form.
        let sql = "SELECT * FROM a RIGHT ANTI JOIN b ON a.i = b.i";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects RIGHT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects RIGHT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects RIGHT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects RIGHT ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects the sided RIGHT ANTI JOIN despite accepting side-less ANTI JOIN"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts RIGHT ANTI JOIN"
        );
    }

    #[test]
    fn lenient_reads_a_bare_factor_asof_as_the_alias() {
        use crate::dialect::Lenient;

        // LENIENT enables the grammar flags but keeps the ANSI reserved model
        // (conflict-resolution rule 5), so on a bare factor the alias reading wins:
        // `asof` aliases `l` and the join parses as a plain inner join. The DuckDB
        // meaning needs the DuckDb preset's reservation. After an explicit alias the
        // word cannot alias, so the AsOf arm fires — the documented split.
        let parsed = parse_with(
            "SELECT * FROM l ASOF JOIN r ON l.t >= r.t",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses via the alias reading");
        let from = &select_of(&parsed).from[0];
        let TableFactor::Table {
            alias: Some(alias), ..
        } = &from.relation
        else {
            panic!("expected `asof` to be read as the factor's alias");
        };
        // Lenient's identity folding is `Casing::Preserve`, so the alias keeps the
        // source spelling.
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "ASOF");
        assert!(matches!(
            from.joins[0].operator,
            JoinOperator::Inner {
                straight: false,
                ..
            },
        ));

        let parsed = parse_with(
            "SELECT * FROM l AS a ASOF JOIN r ON a.t >= r.t",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses the AsOf join after an explicit alias");
        assert!(matches!(
            select_of(&parsed).from[0].joins[0].operator,
            JoinOperator::AsOf { .. },
        ));
    }

    #[test]
    fn comma_separated_from_yields_multiple_relations() {
        let parsed = parse_with(
            "SELECT * FROM t1, t2, t3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("comma FROM parses");
        assert_eq!(select_of(&parsed).from.len(), 3, "three table references");
    }

    #[test]
    fn from_relation_name_is_capped_at_three_parts() {
        // PostgreSQL caps a relation at catalog.schema.table (three parts); a fourth is
        // rejected (tighten-pg-overacceptance-trio). A column reference reaches four parts
        // through a different grammar position, so it stays accepted.
        parse_with("SELECT * FROM a.b.c", crate::ParseConfig::new(TestDialect))
            .expect("a three-part relation parses");
        let err = parse_with(
            "SELECT * FROM a.b.c.d",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("a four-part relation is rejected");
        assert_eq!(
            err.expected.as_str(),
            "a relation name of at most three parts (catalog.schema.table)"
        );
        parse_with(
            "SELECT a.b.c.d FROM t",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("a four-part column reference is unaffected");
    }

    #[test]
    fn sqlite_caps_relation_names_at_two_parts() {
        use crate::dialect::{Postgres, Sqlite};

        // SQLite relation (table / index) names are `schema.table` at most; a three-part
        // `a.b.c` is a syntax error in every relation position (engine-measured via
        // rusqlite). PostgreSQL admits the catalog-qualified three-part form.
        for sql in [
            "SELECT * FROM a.b.c",
            "SELECT x FROM a.b.c, e.f.g",
            "DROP INDEX a.b.c",
            "DROP TABLE a.b.c",
            "INSERT INTO a.b.c VALUES (1)",
            "UPDATE a.b.c SET x = 1",
            "DELETE FROM a.b.c",
            "CREATE TABLE a.b.c (x INTEGER)",
            "ALTER TABLE a.b.c RENAME TO d",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite caps relations at two parts {sql:?}"));
        }
        // Two-part stays accepted for SQLite; PostgreSQL keeps the three-part form; and a
        // column reference is a separate, deeper position (three parts unaffected).
        parse_with("SELECT * FROM a.b", crate::ParseConfig::new(Sqlite))
            .expect("SQLite accepts schema.table");
        parse_with("SELECT * FROM a.b.c", crate::ParseConfig::new(Postgres))
            .expect("PostgreSQL accepts catalog.schema.table");
        parse_with("SELECT a.b.c FROM t", crate::ParseConfig::new(Sqlite))
            .expect("a three-part column reference is unaffected");
    }

    #[test]
    fn sqlite_rejects_reserved_words_in_collabel_position() {
        use crate::dialect::{Postgres, Sqlite};

        // SQLite rejects a reserved word as an `AS`-alias and as a dotted-name continuation
        // part (`reserved_as_label`); PostgreSQL admits every keyword there.
        for sql in [
            "SELECT 1 AS delete",
            "SELECT 1 AS delete, 2 AS alter",
            "SELECT x AS INTO FROM bla",
            "SELECT x.update",
            "SELECT * FROM schema.case",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite reserves the ColLabel {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
        // A non-reserved keyword stays a usable label under SQLite.
        parse_with("SELECT 1 AS abs", crate::ParseConfig::new(Sqlite))
            .expect("SQLite admits a non-reserved AS-label");
    }

    #[test]
    fn sqlite_rejects_stacked_join_qualifiers() {
        use crate::dialect::{Postgres, Sqlite};

        // SQLite's join-clause is flat: a second stacked `ON`/`USING` is a syntax error
        // (`stacked_join_qualifiers`); PostgreSQL right-nests the qualifiers.
        for sql in [
            "SELECT 1 FROM a JOIN b JOIN c ON b.id = c.id ON a.id = b.id",
            "SELECT * FROM a JOIN b JOIN c USING (id) USING (id)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects stacked qualifiers {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
        // Each join with its own immediately-following constraint is unaffected.
        parse_with(
            "SELECT * FROM a JOIN b ON a.id = b.id JOIN c ON b.id = c.id",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("SQLite accepts non-stacked per-join constraints");
    }

    #[test]
    fn derived_table_subquery_with_alias() {
        let parsed = parse_with(
            "SELECT * FROM ( SELECT 1 ) AS s",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("derived table parses");
        let TableFactor::Derived {
            subquery, alias, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a derived table");
        };
        assert!(
            matches!(subquery.body, SetExpr::Select { .. }),
            "the subquery body is a SELECT",
        );
        let alias = alias.as_ref().expect("the derived table is aliased `s`");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "s");
    }

    #[test]
    fn table_name_can_be_qualified_and_aliased() {
        let parsed = parse_with(
            "SELECT * FROM s.t AS x",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("qualified table parses");
        let TableFactor::Table { name, alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        assert_eq!(name.0.len(), 2, "schema-qualified `s.t`");
        let alias = alias.as_ref().expect("aliased `x`");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "x");
    }

    #[test]
    fn table_alias_column_lists_are_structural() {
        let parsed = parse_with(
            "SELECT * FROM t AS x(a, b)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("table alias column list parses");
        let TableFactor::Table { alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a table factor");
        };
        let alias = alias.as_ref().expect("table alias exists");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "x");
        assert_eq!(alias.columns.len(), 2);
        assert_eq!(parsed.resolver().resolve(alias.columns[0].sym), "a");
        assert_eq!(parsed.resolver().resolve(alias.columns[1].sym), "b");
    }

    #[test]
    fn sqlite_table_valued_pragma_function_is_a_function_factor() {
        use crate::dialect::Sqlite;

        // SQLite's `table-or-subquery` grammar admits a generic function-in-FROM factor
        // (`pragma_table_info('t')` and the `json_each`/`generate_series` table-valued
        // functions). It parses to the same `TableFactor::Function` shape as PostgreSQL's;
        // table-valued-ness is a bind-time concern the parser does not model.
        for (sql, argc) in [
            ("SELECT * FROM pragma_table_info('t1')", 1usize),
            ("SELECT * FROM pragma_index_info('i1')", 1),
            ("SELECT name, type FROM pragma_table_list('v1')", 1),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Sqlite))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let TableFactor::Function {
                lateral,
                function,
                with_ordinality,
                ..
            } = &select_of(&parsed).from[0].relation
            else {
                panic!("{sql:?}: expected a table function factor");
            };
            assert!(!*lateral);
            assert!(!*with_ordinality);
            assert_eq!(function.args.len(), argc, "{sql:?}");
        }
    }

    #[test]
    fn postgres_lateral_table_function_with_ordinality_parses() {
        let parsed = parse_with(
            "SELECT * FROM LATERAL generate_series(1, 3) WITH ORDINALITY AS g(x, ord)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL lateral table function parses");
        let TableFactor::Function {
            lateral,
            function,
            with_ordinality,
            alias,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a table function");
        };
        assert!(*lateral);
        assert_eq!(function.args.len(), 2);
        assert!(*with_ordinality);
        let alias = alias.as_ref().expect("table function alias exists");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "g");
        assert_eq!(alias.columns.len(), 2);
    }

    #[test]
    fn postgres_rows_from_parses() {
        let parsed = parse_with(
            "SELECT * FROM LATERAL ROWS FROM (generate_series(1, 2), generate_series(3, 4)) WITH ORDINALITY AS r(a, b, ord)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL ROWS FROM parses");
        let TableFactor::RowsFrom {
            lateral,
            functions,
            with_ordinality,
            alias,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a ROWS FROM factor");
        };
        assert!(*lateral);
        assert_eq!(functions.len(), 2);
        assert!(*with_ordinality);
        assert_eq!(alias.as_ref().expect("alias").columns.len(), 3);
    }

    #[test]
    fn postgres_table_function_column_definition_list_is_typed() {
        let parsed = parse_with(
            "SELECT * FROM json_to_record('{}') AS x(a INTEGER, b TEXT)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("typed column definition list parses");
        let TableFactor::Function {
            function,
            alias,
            column_defs,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a table function");
        };
        assert_eq!(
            parsed.resolver().resolve(function.name.0[0].sym),
            "json_to_record"
        );
        let alias = alias.as_ref().expect("the function is aliased `x`");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "x");
        assert!(
            alias.columns.is_empty(),
            "typed definitions are recorded as column_defs, not alias columns",
        );
        assert_eq!(column_defs.len(), 2);
        assert_eq!(parsed.resolver().resolve(column_defs[0].name.sym), "a");
        assert!(matches!(column_defs[0].data_type, DataType::Integer { .. }));
        assert_eq!(parsed.resolver().resolve(column_defs[1].name.sym), "b");
        assert!(matches!(column_defs[1].data_type, DataType::Text { .. }));
    }

    #[test]
    fn postgres_table_function_column_definition_list_can_omit_the_correlation_name() {
        let parsed = parse_with(
            "SELECT * FROM json_to_record('{}') AS (a INTEGER, b TEXT)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("anonymous column definition list parses");
        let TableFactor::Function {
            alias, column_defs, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a table function");
        };
        assert!(
            alias.is_none(),
            "the `AS (coldef, ...)` form carries no correlation name",
        );
        assert_eq!(column_defs.len(), 2);
    }

    #[test]
    fn postgres_table_function_alias_column_list_stays_untyped() {
        // `AS x(a, b)` is an alias column-name list, never a typed definition list:
        // an entry without a type must stay an alias column.
        let parsed = parse_with(
            "SELECT * FROM generate_series(1, 3) AS x(a, b)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("alias column list parses");
        let TableFactor::Function {
            alias, column_defs, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a table function");
        };
        assert!(
            column_defs.is_empty(),
            "untyped names stay alias columns, not column definitions",
        );
        let alias = alias.as_ref().expect("aliased `x`");
        assert_eq!(alias.columns.len(), 2);
        assert_eq!(parsed.resolver().resolve(alias.columns[0].sym), "a");
        assert_eq!(parsed.resolver().resolve(alias.columns[1].sym), "b");
    }

    #[test]
    fn function_in_from_carries_named_arguments() {
        // A table-valued function in FROM position reuses the shared call-argument
        // grammar, so a PostgreSQL named argument (`name => value` / the deprecated
        // `name := value`) parses there for free and rides the function factor's
        // `FunctionCall`, not an ordinary scalar call. pg_query (PG 17) parse-accepts
        // every form here, including a mixed positional-then-named list.
        for (sql, expected) in [
            ("SELECT * FROM f(x => 1)", ArgSyntax::Arrow),
            ("SELECT * FROM f(x := 1)", ArgSyntax::ColonEquals),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect("named table-function arg parses");
            let TableFactor::Function { function, .. } = &select_of(&parsed).from[0].relation
            else {
                panic!("expected a table function factor, not a scalar call: {sql}");
            };
            assert_eq!(function.args.len(), 1);
            let arg = &function.args[0];
            let name = arg.name.expect("the argument is named");
            assert_eq!(parsed.resolver().resolve(name), "x");
            assert_eq!(arg.syntax, expected);
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("named table-function arg renders");
            assert_eq!(rendered, sql, "the named-argument arrow round-trips");
        }

        // A mixed positional-then-named list is admissible in the same position and
        // records the name only on the named tail.
        let parsed = parse_with(
            "SELECT * FROM generate_series(1, 3, step => 1)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("mixed positional/named table-function args parse");
        let TableFactor::Function { function, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a table function factor");
        };
        assert_eq!(function.args.len(), 3);
        assert_eq!(function.args[0].syntax, ArgSyntax::Positional);
        assert_eq!(function.args[2].syntax, ArgSyntax::Arrow);
    }

    #[test]
    fn function_in_from_named_arguments_are_gated() {
        use crate::dialect::Sqlite;

        // SQLite admits generic function-in-FROM (`table_functions`) but not PostgreSQL
        // named arguments (`named_argument` off), so `f(x => 1)` leaves the `=>` to the
        // expression grammar, where it surfaces as a clean parse error rather than a
        // named argument — the gate keeps a non-PostgreSQL dialect from silently
        // admitting the form (engine-probed: pg_query accepts, our SQLite preset rejects).
        for sql in ["SELECT * FROM f(x => 1)", "SELECT * FROM f(x := 1)"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite rejects the named-argument arrow in FROM: {sql}",
            );
        }
    }

    #[test]
    fn postgres_rows_from_items_carry_per_function_column_definitions() {
        let parsed = parse_with(
            "SELECT * FROM ROWS FROM (json_to_record('{}') AS (a INTEGER), generate_series(1, 2) AS (b INTEGER)) AS r",
            crate::ParseConfig::new(Postgres),
        )
        .expect("ROWS FROM with per-function column definitions parses");
        let TableFactor::RowsFrom {
            functions, alias, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a ROWS FROM factor");
        };
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].column_defs.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(functions[0].column_defs[0].name.sym),
            "a"
        );
        assert_eq!(functions[1].column_defs.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias `r`").name.sym),
            "r"
        );
    }

    #[test]
    fn table_function_column_definitions_round_trip_through_rendering() {
        for sql in [
            "SELECT * FROM json_to_record('{}') AS x(a INTEGER, b TEXT)",
            "SELECT * FROM json_to_record('{}') AS (a INTEGER, b TEXT)",
            "SELECT * FROM ROWS FROM (json_to_record('{}') AS (a INTEGER), generate_series(1, 2) AS (b INTEGER)) AS r",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect("column definition list parses");
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("column definition list renders");
            assert_eq!(rendered, sql);
        }
    }

    /// ANSI plus the BigQuery/ZetaSQL `unnest_with_offset` flag alone (over the
    /// PostgreSQL base, which enables `unnest` itself), isolating the framework gate from
    /// a future BigQuery preset — the `PIPE_SYNTAX_DIALECT` precedent for the `WITH OFFSET`
    /// tail no shipped dialect enables.
    const UNNEST_OFFSET_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.table_factor_syntax(TableFactorSyntax {
                unnest_with_offset: true,
                ..TableFactorSyntax::POSTGRES
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn postgres_unnest_is_a_first_class_factor() {
        // One arg, the multi-array zip, `WITH ORDINALITY`, and a `LATERAL` correlation —
        // each parses into `TableFactor::Unnest`, not the generic `TableFactor::Function`.
        for sql in [
            "SELECT * FROM UNNEST(ARRAY[1, 2, 3])",
            "SELECT * FROM UNNEST(a, b)",
            "SELECT * FROM UNNEST(ARRAY[1, 2, 3]) WITH ORDINALITY AS u(v, ord)",
            "SELECT * FROM t CROSS JOIN LATERAL UNNEST(t.arr) AS e(x)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let has_unnest = select_of(&parsed)
                .from
                .iter()
                .any(|table| matches!(table.relation, TableFactor::Unnest { .. }))
                || select_of(&parsed).from.iter().any(|table| {
                    table
                        .joins
                        .iter()
                        .any(|join| matches!(join.relation, TableFactor::Unnest { .. }))
                });
            assert!(
                has_unnest,
                "expected a first-class UNNEST factor in {sql:?}"
            );
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("UNNEST factor renders");
            assert_eq!(rendered, sql, "UNNEST round-trips");
        }
    }

    #[test]
    fn postgres_unnest_carries_its_fields() {
        let parsed = parse_with(
            "SELECT * FROM UNNEST(ARRAY[1, 2], ARRAY[3, 4]) WITH ORDINALITY AS u(a, b, ord)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("UNNEST parses");
        let TableFactor::Unnest {
            lateral,
            array_exprs,
            with_ordinality,
            alias,
            column_defs,
            with_offset,
            with_offset_alias,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a first-class UNNEST factor");
        };
        assert!(!*lateral);
        assert_eq!(array_exprs.len(), 2, "the two array arguments");
        assert!(*with_ordinality);
        assert!(column_defs.is_empty(), "an untyped alias column list");
        assert!(!*with_offset);
        assert!(with_offset_alias.is_none());
        let alias = alias.as_ref().expect("the UNNEST alias `u`");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "u");
        assert_eq!(alias.columns.len(), 3, "v, b, and the ordinality column");
    }

    #[test]
    fn duckdb_unnest_is_a_first_class_factor() {
        // The stock `DuckDb` preset has no Tier-1 render target, so the exact-text
        // round-trip parses and renders through a `FeatureDialect` wrapping its FeatureSet
        // (the `RenderDialect` shape the sibling DuckDb round-trip tests use).
        const DUCKDB: FeatureDialect = FeatureDialect {
            features: &FeatureSet::DUCKDB,
        };
        for sql in [
            "SELECT * FROM UNNEST([1, 2, 3])",
            "SELECT * FROM UNNEST([1, 2, 3]) WITH ORDINALITY AS t(x)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert!(
                matches!(
                    select_of(&parsed).from[0].relation,
                    TableFactor::Unnest { .. }
                ),
                "DuckDB `{sql}` is a first-class UNNEST factor",
            );
            let rendered = Renderer::new(DUCKDB)
                .render_parsed(&parsed)
                .expect("UNNEST factor renders");
            assert_eq!(rendered, sql);
        }
    }

    #[test]
    fn multiarg_unnest_is_a_bind_layer_boundary_not_parse_gated() {
        // Multi-array `UNNEST(a, b)` (and the degenerate `UNNEST()`) is a grammatically
        // valid first-class factor under EVERY preset that enables `unnest`. PostgreSQL
        // binds it (it zips the arrays); DuckDB and BigQuery reject it only at BIND —
        // function-catalog arity (`unnest(ANY)`), a *Binder Error*, never a parse error
        // (oracle-probed on DuckDB 1.5.4). The parser owns grammar shape, not function
        // arity, so it deliberately accepts the multi-array form under the DuckDb preset
        // rather than replicating a bind-layer residual as a parse gate
        // (`duckdb-unnest-multiarg-over-accept`). This pins that decision so a future
        // change cannot silently turn it into an over-reject of PostgreSQL's valid zip.
        const DUCKDB: FeatureDialect = FeatureDialect {
            features: &FeatureSet::DUCKDB,
        };
        let duckdb = parse_with(
            "SELECT * FROM UNNEST([1, 2], [3, 4])",
            crate::ParseConfig::new(DUCKDB),
        )
        .expect("DuckDb parse-accepts multi-array UNNEST (reject is bind-layer)");
        let postgres = parse_with(
            "SELECT * FROM unnest(ARRAY[1, 2], ARRAY[3, 4])",
            crate::ParseConfig::new(Postgres),
        )
        .expect("Postgres parse-accepts multi-array unnest (it zips)");
        for parsed in [duckdb, postgres] {
            let TableFactor::Unnest { array_exprs, .. } = &select_of(&parsed).from[0].relation
            else {
                panic!("expected a first-class UNNEST factor");
            };
            assert_eq!(
                array_exprs.len(),
                2,
                "both arrays are carried, arity ungated",
            );
        }
    }

    #[test]
    fn unnest_is_gated_and_a_bare_unnest_is_a_table_name() {
        use crate::dialect::{Ansi, MySql};

        // ANSI/MySQL have `unnest` off and `table_functions` off, so `UNNEST(…)` is a clean
        // parse error — the same reject any function-in-FROM gives there. (SQLite excluded:
        // its grammar admits generic function-in-FROM, so `table_functions` is on there —
        // `UNNEST(ARRAY[…])` still rejects, but on the `ARRAY[…]` expression, not the gate.)
        for dialect_name in ["ansi", "mysql"] {
            let sql = "SELECT * FROM UNNEST(ARRAY[1, 2, 3])";
            let result = match dialect_name {
                "ansi" => parse_with(sql, crate::ParseConfig::new(Ansi)),
                _ => parse_with(sql, crate::ParseConfig::new(MySql)),
            };
            assert!(result.is_err(), "{dialect_name} rejects the UNNEST factor");
        }

        // A bare `UNNEST` with no `(` is left to the named-table path as an ordinary
        // relation name (the interception fires only on `UNNEST (`).
        let parsed = parse_with(
            "SELECT * FROM unnest AS u",
            crate::ParseConfig::new(Postgres),
        )
        .expect("a bare UNNEST is a table name");
        assert!(
            matches!(
                select_of(&parsed).from[0].relation,
                TableFactor::Table { .. }
            ),
            "bare `UNNEST` is a plain table factor",
        );
    }

    #[test]
    fn unnest_with_offset_is_preset_less() {
        use crate::dialect::DuckDb;

        // `WITH OFFSET` accepts only under a dialect that enables the preset-less flag,
        // and round-trips the tail (`WITH OFFSET` and `WITH OFFSET AS <alias>`).
        for sql in [
            "SELECT * FROM UNNEST(ARRAY[1, 2, 3]) WITH OFFSET",
            "SELECT * FROM UNNEST(ARRAY[1, 2, 3]) WITH OFFSET AS pos",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(UNNEST_OFFSET_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let TableFactor::Unnest {
                with_offset,
                with_offset_alias,
                ..
            } = &select_of(&parsed).from[0].relation
            else {
                panic!("expected a first-class UNNEST factor");
            };
            assert!(*with_offset, "the WITH OFFSET tail is recorded");
            let has_alias = sql.contains(" AS ");
            assert_eq!(with_offset_alias.is_some(), has_alias);
            let rendered = Renderer::new(UNNEST_OFFSET_DIALECT)
                .render_parsed(&parsed)
                .expect("UNNEST WITH OFFSET renders");
            assert_eq!(rendered, sql, "WITH OFFSET round-trips");
        }

        // PostgreSQL and DuckDB (offset flag off) parse-reject the tail — engine-probed.
        for &(sql, dialect_name) in &[
            ("SELECT * FROM UNNEST(ARRAY[1, 2, 3]) WITH OFFSET", "pg"),
            ("SELECT * FROM UNNEST([1, 2, 3]) WITH OFFSET", "duckdb"),
        ] {
            let result = if dialect_name == "pg" {
                parse_with(sql, crate::ParseConfig::new(Postgres))
            } else {
                parse_with(sql, crate::ParseConfig::new(DuckDb))
            };
            assert!(
                result.is_err(),
                "{dialect_name} rejects WITH OFFSET (preset-less): {sql:?}",
            );
        }
    }

    #[test]
    fn postgres_only_and_tablesample_parse_with_alias_before_sample() {
        let parsed = parse_with(
            "SELECT * FROM ONLY (t) AS x TABLESAMPLE BERNOULLI (10) REPEATABLE (42)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL ONLY TABLESAMPLE parses");
        let TableFactor::Table {
            inheritance,
            alias,
            sample,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a sampled table factor");
        };
        assert!(matches!(
            inheritance,
            RelationInheritance::Only(OnlySyntax::Parenthesized)
        ));
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias").name.sym),
            "x"
        );
        let sample = sample.as_ref().expect("sample clause");
        assert_eq!(
            parsed.resolver().resolve(sample.method.0[0].sym),
            "BERNOULLI"
        );
        assert_eq!(sample.args.len(), 1);
        assert!(sample.repeatable.is_some());
    }

    #[test]
    fn postgres_descendant_star_table_factor_round_trips() {
        // `t *` is the explicit descendant-table spelling: structurally distinct
        // from a bare `t` (so it round-trips), yet the same relation semantically.
        for sql in [
            "SELECT * FROM t *",
            "SELECT * FROM t * AS x",
            "SELECT * FROM s.t *",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect("PostgreSQL descendant `*` parses");
            let TableFactor::Table { inheritance, .. } = &select_of(&parsed).from[0].relation
            else {
                panic!("expected a plain table factor");
            };
            assert_eq!(*inheritance, RelationInheritance::Descendants);
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("descendant `*` renders");
            assert_eq!(rendered, sql);
        }

        // The descendant `*` shares the `ONLY` inheritance gate, so a dialect
        // without PostgreSQL inheritance syntax rejects it.
        parse_with("SELECT * FROM t *", crate::ParseConfig::new(TestDialect))
            .expect_err("ANSI has no descendant-table `*` marker");
    }

    #[test]
    fn parenthesized_join_tree_can_be_join_relation() {
        let parsed = parse_with(
            "SELECT * FROM t JOIN (u JOIN v ON u.id = v.id) AS j ON t.id = j.id",
            crate::ParseConfig::new(Postgres),
        )
        .expect("parenthesized join tree parses");
        let join = &select_of(&parsed).from[0].joins[0];
        let TableFactor::NestedJoin { table, alias, .. } = &join.relation else {
            panic!("expected nested join relation");
        };
        assert_eq!(table.joins.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias").name.sym),
            "j"
        );
    }

    #[test]
    fn redundant_parenthesized_join_tree_collapses_to_join_grouping() {
        let parsed = parse_with(
            "SELECT * FROM ((t JOIN u ON TRUE)) AS j",
            crate::ParseConfig::new(Postgres),
        )
        .expect("redundantly parenthesized join tree parses");
        let TableFactor::NestedJoin { table, alias, .. } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected nested join relation");
        };
        assert!(matches!(&table.relation, TableFactor::Table { .. }));
        assert_eq!(table.joins.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias").name.sym),
            "j"
        );
    }

    #[test]
    fn parenthesized_set_operation_is_a_derived_table_in_from_position() {
        // parse-parenthesized-set-operation-operand-in-derived-table-from-position: the
        // fully-parenthesized render of a 3+-arm set-op derived table —
        // `FROM ((SELECT …) UNION …) x` — is a legal parenthesized query expression
        // (PostgreSQL `select_with_parens`), so it parses as a derived table whose body
        // is the set operation, not as a parenthesized joined table that chokes on the
        // trailing set-op keyword.
        let parsed = parse_with(
            "SELECT * FROM ((SELECT 1 UNION ALL SELECT 2) UNION ALL SELECT 3) AS x",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parenthesized set-op operand in derived-table position parses");
        let TableFactor::Derived {
            subquery, alias, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a derived table, not a parenthesized joined table");
        };
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias `x`").name.sym),
            "x"
        );
        // The body is the outer `UNION ALL`, whose left arm is the (parenthesized)
        // inner `UNION ALL` and whose right arm is the third SELECT — the same 3-arm
        // shape the unparenthesized `(SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3)`
        // builds, since the redundant grouping carries no AST node (ADR-0011).
        let SetExpr::SetOperation { left, right, .. } = &subquery.body else {
            panic!("expected the derived subquery body to be a set operation");
        };
        assert!(
            matches!(**left, SetExpr::SetOperation { .. }),
            "left arm is the inner set operation",
        );
        assert!(
            matches!(**right, SetExpr::Select { .. }),
            "right arm is the third SELECT",
        );
    }

    #[test]
    fn redundantly_parenthesized_subquery_is_a_derived_table() {
        // `((SELECT 1))` is `select_with_parens` nested in `select_with_parens` — a
        // query, not a parenthesized join — so the extra parens collapse to a plain
        // derived table rather than being rejected like `((t))`.
        let parsed = parse_with(
            "SELECT * FROM ((SELECT 1)) AS x",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("doubly parenthesized subquery parses as a derived table");
        let TableFactor::Derived {
            subquery, alias, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a derived table");
        };
        assert!(matches!(subquery.body, SetExpr::Select { .. }));
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias `x`").name.sym),
            "x"
        );
    }

    #[test]
    fn parenthesized_join_of_derived_tables_is_not_swallowed_as_a_query() {
        // The speculative parenthesized-query reading must rewind when the group is a
        // *join* whose first factor is itself a derived table: after `(SELECT 1) AS a`
        // the `JOIN` (not a set-op keyword) marks the group as `'(' joined_table ')'`,
        // so it stays a nested join — the query parse consumed only `(SELECT 1)`, never
        // reaching the group's closing `)`.
        let parsed = parse_with(
            "SELECT * FROM ((SELECT 1) AS a JOIN (SELECT 2) AS b ON TRUE) AS j",
            crate::ParseConfig::new(Postgres),
        )
        .expect("parenthesized join of derived tables parses");
        let TableFactor::NestedJoin { table, alias, .. } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a nested join, not a derived table");
        };
        assert!(
            matches!(table.relation, TableFactor::Derived { .. }),
            "the join's first factor is a derived table",
        );
        assert_eq!(table.joins.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("alias `j`").name.sym),
            "j"
        );
    }

    #[test]
    fn postgres_join_using_alias_is_constraint_alias() {
        let parsed = parse_with(
            "SELECT * FROM t JOIN u USING (id) AS merged",
            crate::ParseConfig::new(Postgres),
        )
        .expect("JOIN USING alias parses");
        let JoinOperator::Inner {
            constraint: JoinConstraint::Using { columns, alias, .. },
            ..
        } = &select_of(&parsed).from[0].joins[0].operator
        else {
            panic!("expected JOIN USING constraint");
        };
        assert_eq!(columns.len(), 1);
        assert_eq!(
            parsed
                .resolver()
                .resolve(alias.as_ref().expect("using alias").sym),
            "merged"
        );
    }

    #[test]
    fn postgres_rejects_lateral_before_plain_table_forms() {
        for sql in [
            "SELECT * FROM LATERAL t",
            "SELECT * FROM LATERAL ONLY t",
            "SELECT * FROM LATERAL ONLY (t)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres)).unwrap_err();
        }
    }

    #[test]
    fn postgres_rejects_parentheses_around_plain_table_references() {
        for sql in [
            "SELECT * FROM (t) AS x",
            "SELECT * FROM ((t)) AS x",
            "SELECT * FROM (t) JOIN u ON TRUE",
            "SELECT * FROM ((t JOIN u ON TRUE) AS inner_j)",
            "SELECT * FROM ((t JOIN u ON TRUE) AS inner_j) AS outer_j",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres)).unwrap_err();
        }
    }

    #[test]
    fn ansi_rejects_postgres_only_table_expression_extensions() {
        for sql in [
            "SELECT * FROM LATERAL (SELECT 1) AS s(a)",
            "SELECT * FROM generate_series(1, 3) AS g(x)",
            "SELECT * FROM json_to_record('{}') AS (a INTEGER)",
            "SELECT * FROM json_to_record('{}') AS x(a INTEGER)",
            "SELECT * FROM ROWS FROM (generate_series(1, 2)) AS r(a)",
            "SELECT * FROM ROWS FROM (json_to_record('{}') AS (a INTEGER)) AS r",
            "SELECT * FROM ONLY t AS x",
            "SELECT * FROM t AS x TABLESAMPLE SYSTEM (10)",
            "SELECT * FROM t JOIN u USING (id) AS merged",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect)).unwrap_err();
        }
    }

    #[test]
    fn non_reserved_keyword_can_be_a_table_name() {
        // `Nulls` is an unreserved keyword (`asc`/`desc` became reserved under the
        // PostgreSQL category model), so it is admissible as a bare `ColId`.
        let parsed = parse_with("SELECT * FROM Nulls", crate::ParseConfig::new(TestDialect))
            .expect("contextual keyword table name parses");
        let TableFactor::Table { name, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "Nulls");
    }

    #[test]
    fn reserved_keyword_cannot_be_a_table_name() {
        let err = parse_with("SELECT * FROM FROM", crate::ParseConfig::new(TestDialect))
            .expect_err("reserved keyword table name is rejected");
        assert_eq!(err.span, crate::ast::Span::new(14, 18));
    }

    // --- Quoted identifiers (prod-sql-quoted-identifiers) -------------------

    #[test]
    fn materialize_quoted_ident_strips_delimiters_and_collapses_doubled_close() {
        // Each style's delimiters are stripped and the doubled close is collapsed
        // (ADR-0006 defers this unescape to materialization). A body with no doubled
        // close stays borrow-only; only a collapsed one allocates.
        for (raw, style, text) in [
            ("\"x\"", QuoteStyle::Double, "x"),
            ("`x`", QuoteStyle::Backtick, "x"),
            ("[x]", QuoteStyle::Bracket, "x"),
        ] {
            let (got_style, got_text) = materialize_quoted_ident(raw);
            assert_eq!(got_style, style);
            assert_eq!(got_text, text);
            assert!(
                matches!(got_text, Cow::Borrowed(_)),
                "{raw:?} has no doubled close, so it should not allocate",
            );
        }

        for (raw, style, text) in [
            ("\"a\"\"b\"", QuoteStyle::Double, "a\"b"),
            ("`a``b`", QuoteStyle::Backtick, "a`b"),
            ("[a]]b]", QuoteStyle::Bracket, "a]b"),
        ] {
            let (got_style, got_text) = materialize_quoted_ident(raw);
            assert_eq!(got_style, style);
            assert_eq!(got_text, text);
            assert!(
                matches!(got_text, Cow::Owned(_)),
                "{raw:?} collapses a doubled close, so it allocates an owned body",
            );
        }

        // The asymmetric bracket open never doubles, so an inner `[` is literal.
        let (style, text) = materialize_quoted_ident("[a[b]");
        assert_eq!(style, QuoteStyle::Bracket);
        assert_eq!(text, "a[b");
        assert!(matches!(text, Cow::Borrowed(_)));
    }

    #[test]
    fn double_quoted_identifier_is_a_table_name_with_quote_style() {
        let parsed = parse_with("SELECT * FROM \"x\"", crate::ParseConfig::new(Postgres))
            .expect("double-quoted table name parses");
        let name = sole_table_name(&parsed);
        assert_eq!(name.0.len(), 1);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "x");
        assert_eq!(name.0[0].quote, QuoteStyle::Double);
    }

    #[test]
    fn backtick_identifier_is_a_column_reference_with_quote_style() {
        let parsed = parse_with("SELECT `c`", crate::ParseConfig::new(BACKTICK_DIALECT))
            .expect("backtick column ref parses");
        let name = sole_column_name(&parsed);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "c");
        assert_eq!(name.0[0].quote, QuoteStyle::Backtick);
    }

    #[test]
    fn bracket_identifier_is_a_table_name_with_quote_style() {
        let parsed = parse_with(
            "SELECT * FROM [t]",
            crate::ParseConfig::new(BRACKET_DIALECT),
        )
        .expect("bracket table name parses");
        let name = sole_table_name(&parsed);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "t");
        assert_eq!(name.0[0].quote, QuoteStyle::Bracket);
    }

    #[test]
    fn doubled_close_delimiter_round_trips_through_the_parser() {
        // `"a""b"` is one identifier whose body is `a"b`, not two adjacent items.
        let parsed = parse_with("SELECT \"a\"\"b\"", crate::ParseConfig::new(Postgres))
            .expect("doubled double-quote parses");
        assert_eq!(
            parsed
                .resolver()
                .resolve(sole_column_name(&parsed).0[0].sym),
            "a\"b"
        );

        let parsed = parse_with(
            "SELECT * FROM [a]]b]",
            crate::ParseConfig::new(BRACKET_DIALECT),
        )
        .expect("doubled bracket parses");
        assert_eq!(
            parsed.resolver().resolve(sole_table_name(&parsed).0[0].sym),
            "a]b"
        );
    }

    #[test]
    fn qualified_quoted_name_collects_each_quoted_part() {
        let parsed = parse_with(
            "SELECT * FROM \"s\".\"t\"",
            crate::ParseConfig::new(Postgres),
        )
        .expect("qualified quoted name parses");
        let name = sole_table_name(&parsed);
        assert_eq!(name.0.len(), 2);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "s");
        assert_eq!(parsed.resolver().resolve(name.0[1].sym), "t");
        assert!(name.0.iter().all(|part| part.quote == QuoteStyle::Double));
    }

    #[test]
    fn quoted_identifier_is_valid_as_an_alias() {
        let parsed = parse_with("SELECT 1 AS \"x\"", crate::ParseConfig::new(Postgres))
            .expect("quoted projection alias parses");
        let [
            SelectItem::Expr {
                alias: Some(alias), ..
            },
        ] = select_of(&parsed).projection.as_slice()
        else {
            panic!("expected an aliased projection");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "x");
        assert_eq!(alias.quote, QuoteStyle::Double);
    }

    #[test]
    fn quoting_lets_a_reserved_word_be_an_identifier() {
        // `FROM` is reserved and cannot be a bare table name, but quoting bypasses
        // reservation, so `"from"` is a perfectly good column name.
        let parsed = parse_with("SELECT \"from\"", crate::ParseConfig::new(Postgres))
            .expect("quoted reserved word parses");
        let name = sole_column_name(&parsed);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "from");
        assert_eq!(name.0[0].quote, QuoteStyle::Double);
    }

    #[test]
    fn quoted_identifiers_round_trip_through_rendering() {
        // The preserved [`QuoteStyle`] is re-escaped by the renderer (doubling the
        // close), so each style round-trips to its exact input. The conformance
        // corpus covers the double-quote forms under ANSI; backtick and bracket need
        // dialects that enable those styles, so they round-trip here.
        for sql in [
            "SELECT \"x\"",
            "SELECT \"a\"\"b\"",
            "SELECT * FROM \"s\".\"t\"",
            "SELECT 1 AS \"from\"",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect("double-quoted identifier parses");
            let rendered = Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("double-quoted identifier renders");
            assert_eq!(rendered, sql);
        }

        for sql in ["SELECT `c`", "SELECT `a``b`"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(BACKTICK_DIALECT))
                .expect("backtick identifier parses");
            let rendered = Renderer::new(BACKTICK_DIALECT)
                .render_parsed(&parsed)
                .expect("backtick identifier renders");
            assert_eq!(rendered, sql);
        }

        for sql in ["SELECT * FROM [t]", "SELECT * FROM [a]]b]"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(BRACKET_DIALECT))
                .expect("bracket identifier parses");
            let rendered = Renderer::new(BRACKET_DIALECT)
                .render_parsed(&parsed)
                .expect("bracket identifier renders");
            assert_eq!(rendered, sql);
        }
    }

    /// Borrow the first FROM relation as a `TableFactor::Table`'s fields.
    fn first_table(
        parsed: &Parsed,
    ) -> (
        &thin_vec::ThinVec<crate::ast::Ident>,
        &thin_vec::ThinVec<crate::ast::IndexHint>,
    ) {
        let TableFactor::Table {
            partition,
            index_hints,
            ..
        } = &select_of(parsed).from[0].relation
        else {
            panic!("expected a plain table factor");
        };
        (partition, index_hints)
    }

    #[test]
    fn mysql_index_hints_parse_after_alias_in_every_form() {
        use crate::dialect::{Ansi, MySql};

        // `USE INDEX (idx)` — the default action/keyword, no scope, one index.
        let parsed = parse_with(
            "SELECT a FROM th USE INDEX (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses USE INDEX");
        let (_, hints) = first_table(&parsed);
        assert!(matches!(
            hints[..],
            [crate::ast::IndexHint {
                action: IndexHintAction::Use,
                keyword: IndexHintKeyword::Index,
                scope: None,
                ..
            }],
        ));
        assert_eq!(hints[0].indexes.len(), 1);

        // `FORCE KEY FOR ORDER BY (idx)` — the `KEY` spelling with a scope.
        let parsed = parse_with(
            "SELECT a FROM th FORCE KEY FOR ORDER BY (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses FORCE KEY FOR ORDER BY");
        let (_, hints) = first_table(&parsed);
        assert!(matches!(
            hints[0],
            crate::ast::IndexHint {
                action: IndexHintAction::Force,
                keyword: IndexHintKeyword::Key,
                scope: Some(IndexHintScope::OrderBy),
                ..
            },
        ));

        // `IGNORE INDEX FOR JOIN` and `FOR GROUP BY` reach the other scopes.
        let parsed = parse_with(
            "SELECT a FROM th IGNORE INDEX FOR JOIN (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses IGNORE INDEX FOR JOIN");
        assert!(matches!(
            first_table(&parsed).1[0].scope,
            Some(IndexHintScope::Join)
        ));
        let parsed = parse_with(
            "SELECT a FROM th USE INDEX FOR GROUP BY (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses USE INDEX FOR GROUP BY");
        assert!(matches!(
            first_table(&parsed).1[0].scope,
            Some(IndexHintScope::GroupBy)
        ));

        // Several hints are juxtaposed (space-separated, no comma).
        let parsed = parse_with(
            "SELECT a FROM th USE INDEX (idx_a) IGNORE INDEX (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses juxtaposed hints");
        assert_eq!(first_table(&parsed).1.len(), 2);

        // `USE INDEX ()` — the empty list ("use no index") is valid only for `USE`.
        let parsed = parse_with(
            "SELECT a FROM th USE INDEX ()",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses USE INDEX ()");
        assert!(first_table(&parsed).1[0].indexes.is_empty());
        parse_with(
            "SELECT a FROM th FORCE INDEX ()",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("FORCE INDEX requires a non-empty list");
        parse_with(
            "SELECT a FROM th IGNORE INDEX ()",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("IGNORE INDEX requires a non-empty list");

        // Hints bind *after* the alias: `AS x` after a hint is leftover input, while the
        // alias-then-hint order parses.
        parse_with(
            "SELECT a FROM th USE INDEX (idx_a) AS x",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("an index hint must follow the alias, not precede it");
        parse_with(
            "SELECT a FROM th AS x USE INDEX (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("a hint follows the alias");

        // Gated: ANSI has no index-hint grammar, so the construct is a parse error.
        parse_with(
            "SELECT a FROM th USE INDEX (idx_a)",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no index hints");
    }

    /// Borrow the first FROM relation's MSSQL `WITH (...)` table-hint list.
    fn first_table_hints(parsed: &Parsed) -> &thin_vec::ThinVec<crate::ast::TableHint> {
        let TableFactor::Table { table_hints, .. } = &select_of(parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        table_hints
    }

    #[test]
    fn mssql_table_hints_parse_render_and_are_gated() {
        use crate::ast::{ForceSeekTarget, TableHint, TableHintKeyword};
        use crate::dialect::{Ansi, Lenient, Mssql, Postgres};

        // A single bare-keyword hint after the relation, typed into `TableHintKeyword`.
        let parsed = parse_with(
            "SELECT a FROM th WITH (NOLOCK)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses WITH (NOLOCK)");
        assert!(matches!(
            first_table_hints(&parsed)[..],
            [TableHint::Keyword {
                keyword: TableHintKeyword::NoLock,
                ..
            }],
        ));

        // Several comma-separated hints, mixing a keyword, the argument-bearing `INDEX`
        // form, and a bare `FORCESEEK`.
        let parsed = parse_with(
            "SELECT a FROM th WITH (HOLDLOCK, INDEX (ix_a, ix_b), FORCESEEK)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses a mixed hint list");
        let hints = first_table_hints(&parsed);
        assert_eq!(hints.len(), 3);
        assert!(matches!(
            hints[0],
            TableHint::Keyword {
                keyword: TableHintKeyword::HoldLock,
                ..
            }
        ));
        assert!(matches!(
            &hints[1],
            TableHint::Index { equals: false, indexes, .. } if indexes.len() == 2
        ));
        assert!(matches!(
            hints[2],
            TableHint::ForceSeek { target: None, .. }
        ));

        // `INDEX = ix` and `INDEX = (a, b)` — the `=` spelling, single and list.
        let parsed = parse_with(
            "SELECT a FROM th WITH (INDEX = ix_a)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses INDEX = ix");
        assert!(matches!(
            &first_table_hints(&parsed)[0],
            TableHint::Index { equals: true, indexes, .. } if indexes.len() == 1
        ));

        // `FORCESEEK ( ix ( col ) )` — the pinned index + column-prefix form.
        let parsed = parse_with(
            "SELECT a FROM th WITH (FORCESEEK (ix_a (c1, c2)))",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses FORCESEEK with a target");
        assert!(matches!(
            &first_table_hints(&parsed)[0],
            TableHint::ForceSeek {
                target: Some(ForceSeekTarget { columns, .. }),
                ..
            } if columns.len() == 2
        ));

        // An unrecognized word is preserved verbatim rather than over-rejecting.
        let parsed = parse_with(
            "SELECT a FROM th WITH (SOMEFUTUREHINT)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL preserves an unmodelled hint word");
        assert!(matches!(
            &first_table_hints(&parsed)[0],
            TableHint::Other { ident, .. }
                if parsed.resolver().resolve(ident.sym) == "SOMEFUTUREHINT"
        ));

        // Hints bind after the alias and the tablesample clause (`FROM t AS x WITH (...)`).
        let parsed = parse_with(
            "SELECT a FROM th AS x WITH (NOLOCK)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("a table hint follows the alias");
        assert_eq!(first_table_hints(&parsed).len(), 1);

        // Round-trip: each surface renders back to its canonical spelling. Rendered
        // through Lenient — a superset that admits the same hints and, unlike the MSSQL
        // preset (whose Tier-2 target render is a later ticket), implements
        // `RenderDialect`; the hint render is dialect-independent, so this exercises the
        // hint AST's round-trip faithfully.
        for sql in [
            "SELECT a FROM th WITH (NOLOCK)",
            "SELECT a FROM th WITH (HOLDLOCK, TABLOCKX)",
            "SELECT a FROM th WITH (INDEX (ix_a, ix_b))",
            "SELECT a FROM th WITH (INDEX = ix_a)",
            "SELECT a FROM th WITH (INDEX = (ix_a, ix_b))",
            "SELECT a FROM th WITH (FORCESEEK)",
            "SELECT a FROM th WITH (FORCESEEK (ix_a (c1, c2)))",
            "SELECT a FROM th AS x WITH (NOLOCK, READPAST)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .expect("Lenient parses the hint surface");
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("the hint surface renders");
            assert_eq!(rendered, sql, "round-trip mismatch for `{sql}`");
        }

        // Gated: ANSI and PostgreSQL have no table-hint grammar, so the trailing `WITH`
        // is unconsumed and the statement rejects (`WITH` stays CTE-only there).
        parse_with(
            "SELECT a FROM th WITH (NOLOCK)",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no table hints");
        parse_with(
            "SELECT a FROM th WITH (NOLOCK)",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL keeps `WITH` CTE-only");
    }

    #[test]
    fn mssql_table_hints_do_not_collide_with_the_cte_with_clause() {
        use crate::dialect::{Lenient, Mssql, Postgres};

        // The leading-`WITH` CTE clause and the trailing-`WITH` table hint sit at
        // different grammar positions, so both parse in one statement under a dialect
        // that admits table hints. The CTE `WITH` at statement start is never taken as a
        // hint, and the hint `WITH` on the base table is never taken as a CTE.
        let sql = "WITH c AS (SELECT 1) SELECT a FROM th WITH (NOLOCK)";
        let parsed = parse_with(sql, crate::ParseConfig::new(Mssql))
            .expect("MSSQL: CTE and table hint coexist");
        assert_eq!(first_table_hints(&parsed).len(), 1);
        // Lenient (the permissive superset) admits both, too.
        let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
            .expect("Lenient parses CTE + table hint");
        assert_eq!(first_table_hints(&parsed).len(), 1);
        // A plain leading CTE without any hint still parses cleanly under PostgreSQL,
        // proving the hint gate never disturbs the CTE reading.
        parse_with(
            "WITH c AS (SELECT 1) SELECT a FROM c",
            crate::ParseConfig::new(Postgres),
        )
        .expect("a bare CTE still parses where table hints are off");
    }

    /// Snowflake/Oracle's `TABLE(<expr>)` first-class table-expression factor
    /// (`planner-parity-table-factor-table-expr`) — distinct from a *named* table
    /// function (`FROM f(1)`, `TableFactor::Function`) and from the standalone
    /// `TABLE t` query-body form (`parse_table_command`, a different statement-position
    /// grammar entry this test never touches). No shipped engine ships this exact
    /// shape with a differential oracle, so it is gated Lenient-only.
    #[test]
    fn table_expr_factor_parses_renders_and_is_gated() {
        use crate::dialect::Ansi;

        // Lenient parses `TABLE(<expr>)` into the dedicated factor, capturing the inner
        // expression and no alias.
        let parsed = parse_with(
            "SELECT * FROM TABLE(f(1))",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses TABLE(...)");
        let TableFactor::TableExpr { expr, alias, .. } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a TableExpr factor");
        };
        assert!(
            matches!(**expr, Expr::Function { .. }),
            "expected the inner call to survive"
        );
        assert!(alias.is_none());

        // An alias — including the column-list form, since Lenient enables
        // `table_alias_column_lists` — attaches to the factor like any other.
        let parsed = parse_with(
            "SELECT * FROM TABLE(f(1)) AS t(a, b)",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses a TABLE(...) column-list alias");
        let TableFactor::TableExpr { alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a TableExpr factor");
        };
        let alias = alias.as_ref().expect("the AS t(a, b) alias is attached");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "t");
        assert_eq!(alias.columns.len(), 2);

        // Round-trip: the canonical spelling re-parses identically.
        for sql in [
            "SELECT * FROM TABLE(f(1))",
            "SELECT * FROM TABLE(f(1)) AS t",
            "SELECT * FROM TABLE(f(1)) AS t(a, b)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .expect("Lenient parses the TABLE(...) surface");
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("the TABLE(...) surface renders");
            assert_eq!(rendered, sql, "round-trip mismatch for `{sql}`");
        }

        // A bare `TABLE` (no immediately-following `(`) is unaffected by the flag: `TABLE`
        // is a globally reserved keyword, so it is never an admissible relation name
        // either way, and this construct is the unrelated statement-position `TABLE t`
        // query form the parser reaches through a different entry point entirely.
        assert!(parse_with("SELECT * FROM TABLE", crate::ParseConfig::new(Lenient)).is_err());
        assert!(parse_with("TABLE t", crate::ParseConfig::new(Lenient)).is_ok());

        // Gated off (ANSI/PostgreSQL/DuckDB): `TABLE(` falls through to the named-table
        // path, where the reserved `TABLE` keyword is not an admissible relation name —
        // the same clean parse error the construct already gave before this factor
        // existed (captured here so a future change to the flag cannot silently widen
        // acceptance without this test noticing).
        let err = parse_with("SELECT * FROM TABLE(f(1))", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no TABLE(...) factor")
            .to_string();
        assert_eq!(
            err, "expected a table name, function call, or `(`, found TABLE at bytes 14..19",
            "the flag-off rejection must stay byte-for-byte the error this construct already \
             gave before `TABLE(<expr>)` existed as a factor — a change here signals the flag \
             stopped gating the fallthrough",
        );
        parse_with(
            "SELECT * FROM TABLE(f(1))",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no TABLE(...) factor (engine-probed reject)");
        parse_with("SELECT * FROM TABLE(f(1))", crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB has no TABLE(...) factor (engine-probed reject)");
    }

    #[test]
    fn mysql_partition_selection_parses_before_alias() {
        use crate::dialect::{MySql, Postgres};

        // `PARTITION (p0, p1)` — a non-empty list between the name and the alias.
        let parsed = parse_with(
            "SELECT a FROM tp PARTITION (p0, p1)",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses partition selection");
        assert_eq!(first_table(&parsed).0.len(), 2);

        // Partition then alias parses; alias then partition is leftover input.
        let parsed = parse_with(
            "SELECT a FROM tp PARTITION (p0) AS x",
            crate::ParseConfig::new(MySql),
        )
        .expect("partition precedes the alias");
        let TableFactor::Table {
            partition, alias, ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a table factor");
        };
        assert_eq!(partition.len(), 1);
        assert!(alias.is_some());
        parse_with(
            "SELECT a FROM tp AS x PARTITION (p0)",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("PARTITION must precede the alias");

        // Partition and an index hint coexist (partition before alias, hint after).
        let parsed = parse_with(
            "SELECT a FROM tp PARTITION (p0) USE INDEX (idx_a)",
            crate::ParseConfig::new(MySql),
        )
        .expect("partition and index hint coexist");
        let (partition, hints) = first_table(&parsed);
        assert_eq!(partition.len(), 1);
        assert_eq!(hints.len(), 1);

        // Gated: without the MySQL gate, `PARTITION (p0)` is *not* partition selection.
        // `PARTITION` is a non-reserved word under PostgreSQL, so it reads as the table
        // alias with a `(p0)` derived-column list — the `partition` field stays empty
        // (a structural divergence, not accept/reject), demonstrating the gate is off.
        let pg = parse_with(
            "SELECT a FROM tp PARTITION (p0)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL reads PARTITION as an alias, not partition selection");
        let TableFactor::Table {
            partition, alias, ..
        } = &select_of(&pg).from[0].relation
        else {
            panic!("expected a table factor");
        };
        assert!(
            partition.is_empty(),
            "the MySQL partition gate is off under PostgreSQL"
        );
        assert!(alias.is_some(), "PARTITION was taken as the alias instead");
    }

    // ---- DuckDB DESCRIBE/SHOW/SUMMARIZE as a table source (`SHOW_REF`) --------------
    // (`duckdb-statement-in-query-position`)

    /// The first FROM relation of a single top-level SELECT.
    fn first_relation(parsed: &Parsed) -> &TableFactor<NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        &select.from[0].relation
    }

    #[test]
    fn describe_of_a_query_is_a_show_ref_table_source() {
        // `FROM (DESCRIBE <query>)` reads the utility as DuckDB's `SHOW_REF` table
        // source, wrapping the described query (a SELECT here); the alias trails.
        let parsed = parse_with(
            "SELECT column_name FROM (DESCRIBE SELECT 1 AS a, 2 AS b) t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DESCRIBE of a query as a table source parses");
        let TableFactor::ShowRef { show, alias, .. } = first_relation(&parsed) else {
            panic!("expected a ShowRef table factor");
        };
        assert!(matches!(show.kind, ShowRefKind::Describe));
        let ShowRefTarget::Query { query, .. } = &show.target else {
            panic!("expected a query target");
        };
        assert!(matches!(query.body, SetExpr::Select { .. }));
        assert!(
            alias.is_some(),
            "the `t` alias attaches to the ShowRef factor"
        );

        let short = parse_with(
            "SELECT column_name FROM (DESC SELECT 1 AS a) t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DESC of a query as a table source parses");
        let TableFactor::ShowRef { show, .. } = first_relation(&short) else {
            panic!("expected a ShowRef table factor");
        };
        assert!(matches!(show.kind, ShowRefKind::Desc));
    }

    #[test]
    fn describe_of_a_pivot_and_unpivot_query_parses() {
        // The described query can itself be a PIVOT/UNPIVOT (now a query body), so the
        // ShowRef target is a `SetExpr::Pivot`/`Unpivot` — the two families composing.
        let parsed = parse_with(
            "SELECT mode(column_type) FROM (DESCRIBE PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)::INTEGER)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DESCRIBE of a PIVOT parses");
        let TableFactor::ShowRef { show, .. } = first_relation(&parsed) else {
            panic!("expected a ShowRef table factor");
        };
        let ShowRefTarget::Query { query, .. } = &show.target else {
            panic!("expected a query target");
        };
        assert!(matches!(query.body, SetExpr::Pivot { .. }));

        let parsed = parse_with(
            "SELECT column_name, column_type FROM (DESCRIBE unpivot (select 42) on columns(*))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DESCRIBE of an UNPIVOT parses");
        let TableFactor::ShowRef { show, .. } = first_relation(&parsed) else {
            panic!("expected a ShowRef table factor");
        };
        assert!(matches!(
            show.target,
            ShowRefTarget::Query { ref query, .. } if matches!(query.body, SetExpr::Unpivot { .. }),
        ));
    }

    #[test]
    fn describe_of_a_table_is_a_named_show_ref() {
        // A bare table name after DESCRIBE is the `Name` target (not a query).
        let parsed = parse_with(
            "SELECT * FROM (DESCRIBE some_table) d",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DESCRIBE of a table parses");
        let TableFactor::ShowRef { show, .. } = first_relation(&parsed) else {
            panic!("expected a ShowRef table factor");
        };
        assert!(matches!(show.kind, ShowRefKind::Describe));
        assert!(matches!(show.target, ShowRefTarget::Name { .. }));

        let bare = parse_with("DESCRIBE", crate::ParseConfig::new(DuckDb))
            .expect("bare DESCRIBE is syntactically valid");
        let show = statement_show_ref(&bare);
        assert!(matches!(show.target, ShowRefTarget::Empty { .. }));
    }

    #[test]
    fn show_databases_is_a_named_show_ref() {
        // `FROM (SHOW databases) t` — SHOW always names its target.
        let parsed = parse_with("FROM (SHOW databases) t", crate::ParseConfig::new(DuckDb))
            .expect("SHOW as a table source parses");
        // Bare FROM-first select: dig the relation out of the FROM-first body.
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let TableFactor::ShowRef { show, alias, .. } = &select.from[0].relation else {
            panic!("expected a ShowRef table factor");
        };
        assert!(matches!(show.kind, ShowRefKind::Show));
        assert!(matches!(show.target, ShowRefTarget::Name { .. }));
        assert!(alias.is_some());
    }

    #[test]
    fn show_ref_round_trips() {
        // DuckDb is not a render target; the round-trip renders under Lenient (which
        // also accepts the show_ref), proving the shape round-trips.
        for sql in [
            "SELECT column_name FROM (DESCRIBE SELECT 1) AS t",
            "SELECT * FROM (SHOW databases) AS t",
            "SELECT * FROM (DESCRIBE monthly_sales) AS d",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn show_ref_is_rejected_where_the_gate_is_off() {
        // Off the `show_ref` gate, `DESCRIBE`/`SHOW` inside FROM parens is neither a
        // query start nor a joined table, so PostgreSQL rejects it — a clean divergence.
        for sql in [
            "SELECT * FROM (DESCRIBE SELECT 1) t",
            "SELECT * FROM (SHOW databases) t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects the show_ref table source {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
                "Lenient accepts the show_ref table source {sql:?}",
            );
        }
    }

    // ---- DuckDB DESCRIBE/SUMMARIZE as a top-level statement (`describe_summarize`) ----

    fn statement_show_ref(parsed: &Parsed) -> &crate::ast::ShowRef<NoExt> {
        let [Statement::ShowRef { show, .. }] = parsed.statements() else {
            panic!(
                "expected one Statement::ShowRef, got {:?}",
                parsed.statements()
            );
        };
        show
    }

    #[test]
    fn describe_and_summarize_statements_wrap_a_query_or_a_name() {
        // The statement form is DuckDB's `SHOW_REF` at statement position: a query target
        // (`DESCRIBE SELECT …`, `SUMMARIZE SELECT …`) or a bare table name
        // (`SUMMARIZE t`), reusing the same `ShowRef` core as the `FROM (…)` table factor.
        let q = parse_with("DESCRIBE SELECT 42 AS a", crate::ParseConfig::new(DuckDb))
            .expect("DESCRIBE <query>");
        let show = statement_show_ref(&q);
        assert!(matches!(show.kind, ShowRefKind::Describe));
        assert!(matches!(show.target, ShowRefTarget::Query { .. }));

        let d = parse_with("DESC SELECT 42 AS a", crate::ParseConfig::new(DuckDb))
            .expect("DESC <query>");
        let show = statement_show_ref(&d);
        assert!(matches!(show.kind, ShowRefKind::Desc));
        assert!(matches!(show.target, ShowRefTarget::Query { .. }));

        let s = parse_with("SUMMARIZE SELECT 42 AS a", crate::ParseConfig::new(DuckDb))
            .expect("SUMMARIZE <query>");
        let show = statement_show_ref(&s);
        assert!(matches!(show.kind, ShowRefKind::Summarize));
        assert!(matches!(show.target, ShowRefTarget::Query { .. }));

        let n = parse_with("SUMMARIZE arrays", crate::ParseConfig::new(DuckDb))
            .expect("SUMMARIZE <table>");
        let show = statement_show_ref(&n);
        assert!(matches!(show.kind, ShowRefKind::Summarize));
        assert!(matches!(show.target, ShowRefTarget::Name { .. }));
    }

    #[test]
    fn describe_summarize_statement_round_trips() {
        // DuckDb is not a render target; render under Lenient (which shares the gate). The
        // statement form renders *without* the table-factor's parentheses.
        for sql in [
            "DESCRIBE SELECT 42 AS a",
            "SUMMARIZE SELECT 42 AS a",
            "DESCRIBE arrays",
            "DESCRIBE",
            "DESC arrays",
            "DESC",
            "SUMMARIZE arrays",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn describe_summarize_statement_is_gated_and_bounded() {
        // Off the gate (PostgreSQL): neither keyword is a statement leader — `DESCRIBE`
        // falls through to the unknown-statement error, `SUMMARIZE` is never a leader.
        assert!(parse_with("DESCRIBE SELECT 1", crate::ParseConfig::new(Postgres)).is_err());
        assert!(parse_with("SUMMARIZE arrays", crate::ParseConfig::new(Postgres)).is_err());

        // Reject boundaries DuckDB itself enforces (oracle-probed on 1.5.4): the named
        // target takes no trailing clause, and bare `SUMMARIZE` has no target.
        for sql in [
            "SUMMARIZE arrays WHERE a > 1",
            "SUMMARIZE arrays LIMIT 1",
            "SUMMARIZE",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?}"
            );
        }
    }

    #[test]
    fn json_table_carries_its_column_kinds() {
        use crate::ast::JsonTableColumn;
        let parsed = parse_with(
            "SELECT * FROM JSON_TABLE(js, '$' AS r PASSING 1 AS x COLUMNS (\
             id FOR ORDINALITY, \
             a int PATH '$.a' WITH WRAPPER NULL ON EMPTY ERROR ON ERROR, \
             e int EXISTS PATH '$.b' TRUE ON ERROR, \
             NESTED PATH '$.c' AS n COLUMNS (d text)) ERROR ON ERROR)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("JSON_TABLE parses");
        let TableFactor::JsonTable { json_table, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a JSON_TABLE factor");
        };
        assert!(json_table.path_name.is_some(), "the `AS r` row-path name");
        assert_eq!(json_table.passing.len(), 1);
        assert!(json_table.on_error.is_some(), "the top-level ON ERROR");
        assert_eq!(json_table.columns.len(), 4);
        assert!(matches!(
            json_table.columns[0],
            JsonTableColumn::ForOrdinality { .. }
        ));
        assert!(matches!(
            json_table.columns[1],
            JsonTableColumn::Regular { .. }
        ));
        assert!(matches!(
            json_table.columns[2],
            JsonTableColumn::Exists { .. }
        ));
        let JsonTableColumn::Nested { columns, .. } = &json_table.columns[3] else {
            panic!("the fourth column is a NESTED sub-table");
        };
        assert_eq!(columns.len(), 1, "the nested column list recurses");
    }

    #[test]
    fn xml_table_carries_its_fields() {
        use crate::ast::XmlTableColumn;
        let parsed = parse_with(
            "SELECT * FROM XMLTABLE(XMLNAMESPACES('u' AS n, DEFAULT 'd'), '/root' \
             PASSING BY REF doc COLUMNS a int PATH 'x' DEFAULT 5 NOT NULL, o FOR ORDINALITY)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("XMLTABLE parses");
        let TableFactor::XmlTable { xml_table, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected an XMLTABLE factor");
        };
        assert_eq!(xml_table.namespaces.len(), 2);
        assert!(
            xml_table.namespaces[1].name.is_none(),
            "the DEFAULT namespace is unnamed"
        );
        assert!(xml_table.passing_mechanism_before.is_some(), "`BY REF`");
        assert_eq!(xml_table.columns.len(), 2);
        let XmlTableColumn::Regular {
            path,
            default,
            not_null,
            ..
        } = &xml_table.columns[0]
        else {
            panic!("the first column is a regular column");
        };
        assert!(path.is_some() && default.is_some());
        assert_eq!(*not_null, Some(true));
        assert!(matches!(
            xml_table.columns[1],
            XmlTableColumn::ForOrdinality { .. }
        ));
    }

    #[test]
    fn json_table_and_xml_table_round_trip() {
        for sql in [
            "SELECT * FROM JSON_TABLE(js, '$' COLUMNS (a INTEGER PATH '$.a', NESTED PATH '$.b' COLUMNS (c TEXT)))",
            "SELECT * FROM JSON_TABLE(js, '$' AS r PASSING 1 AS x COLUMNS (id FOR ORDINALITY, e INTEGER EXISTS PATH '$' UNKNOWN ON ERROR) EMPTY ARRAY ON ERROR)",
            "SELECT * FROM XMLTABLE('/root' PASSING doc COLUMNS a INTEGER PATH 'x' NOT NULL, o FOR ORDINALITY) AS t(x, y)",
            "SELECT * FROM XMLTABLE(('/a' || '/b') PASSING BY REF doc COLUMNS a INTEGER)",
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
    fn json_table_and_xml_table_are_gated_off_elsewhere() {
        use crate::dialect::Ansi;
        // Under a preset with the gates off, the keyword heads fall to the ordinary
        // function/name path, which rejects at the `COLUMNS`/`PASSING` clause — matching the
        // engines (DuckDB probed, ANSI has no such form). Lenient (gates on) accepts.
        for sql in [
            "SELECT * FROM JSON_TABLE(js, '$' COLUMNS (a int))",
            "SELECT * FROM XMLTABLE('/root' PASSING doc COLUMNS a int)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
                "Lenient accepts {sql:?}"
            );
        }
    }

    #[test]
    fn open_json_carries_its_fields() {
        use crate::ast::OpenJsonColumn;
        let parsed = parse_with(
            "SELECT * FROM OPENJSON(@doc, '$.items') \
             WITH (id INT '$.id', name NVARCHAR '$.name', raw NVARCHAR '$.raw' AS JSON, plain INT)",
            crate::ParseConfig::new(Mssql),
        )
        .expect("MSSQL parses OPENJSON WITH");
        let TableFactor::OpenJson { open_json, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected an OPENJSON factor");
        };
        assert!(open_json.path.is_some(), "the `, '$.items'` row path");
        assert_eq!(open_json.columns.len(), 4);
        // A `<path>` string is optional and `AS JSON` is a per-column marker.
        let OpenJsonColumn { path, as_json, .. } = &open_json.columns[0];
        assert!(path.is_some(), "`id INT '$.id'` carries its column path");
        assert!(!as_json);
        assert!(open_json.columns[2].as_json, "`raw … AS JSON`");
        assert!(
            open_json.columns[3].path.is_none() && !open_json.columns[3].as_json,
            "`plain INT` has neither a path nor AS JSON"
        );
    }

    #[test]
    fn open_json_round_trips() {
        // The default-schema form (no `WITH`), the single-argument form (no path), and the
        // full `WITH (…)` schema with a column path and the `AS JSON` marker all round-trip
        // from their spans (spelling fidelity).
        for sql in [
            "SELECT * FROM OPENJSON(@doc)",
            "SELECT * FROM OPENJSON(@doc, '$.items')",
            "SELECT * FROM OPENJSON(@doc, '$.items') WITH (id INTEGER '$.id', raw NVARCHAR '$.raw' AS JSON, plain INTEGER) AS j",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Mssql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            // Parsed under MSSQL; rendered under Lenient (the `RenderDialect` superset the
            // other MSSQL round-trip tests use) — the stored spelling round-trips either way.
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn open_json_is_gated_off_elsewhere() {
        use crate::dialect::Ansi;
        // With `open_json` off, `OPENJSON(` falls to the ordinary function/name path, which
        // rejects at the `WITH (…)` clause tail. MSSQL and Lenient (gate on) accept.
        let sql = "SELECT * FROM OPENJSON(doc, '$.a') WITH (id INT)";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Mssql)).is_ok(),
            "MSSQL accepts {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Lenient)).is_ok(),
            "Lenient accepts {sql:?}"
        );
    }

    #[test]
    fn duckdb_bare_from_values_is_a_derived_table_factor() {
        // DuckDB's bare `FROM VALUES (…) AS t(cols)` is the derived-table node tagged
        // `BareValues`: a `VALUES` body plus the mandatory alias, no wrapping parens.
        let parsed = parse_with(
            "SELECT c1 FROM VALUES ('CS', 'Bachelor'), ('CS', 'PhD') AS t(c1, c2)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses a bare FROM VALUES table factor");
        let TableFactor::Derived {
            lateral,
            subquery,
            alias,
            spelling,
            ..
        } = &select_of(&parsed).from[0].relation
        else {
            panic!("expected a derived table factor");
        };
        assert!(!*lateral);
        assert!(matches!(spelling, DerivedSpelling::BareValues));
        let SetExpr::Values { values, .. } = &subquery.body else {
            panic!("the bare-values body is a VALUES constructor");
        };
        assert_eq!(values.rows.len(), 2, "two VALUES rows");
        // The alias the parser required, carrying the column list.
        let alias = alias
            .as_ref()
            .expect("a bare FROM VALUES carries its alias");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "t");
        assert_eq!(alias.columns.len(), 2);
        assert_eq!(parsed.resolver().resolve(alias.columns[0].sym), "c1");
    }

    #[test]
    fn duckdb_bare_from_values_accepts_the_aliased_forms() {
        // Every alias spelling DuckDB accepts (probed on 1.5.4): `AS t`, bare `t`,
        // `AS t(cols)`, and the FROM-first / CTE / CREATE-TABLE-AS positions.
        for sql in [
            "SELECT * FROM VALUES (1, 2) AS t",
            "SELECT * FROM VALUES (1, 2) t",
            "SELECT * FROM VALUES (1, 2) AS t(a, b)",
            "SELECT a FROM VALUES (1, 2), (3, 4) t(a, b) WHERE a > 1",
            "FROM VALUES (1), (2) t(x)",
            "WITH c AS (FROM VALUES (1), (2) AS c(a)) SELECT * FROM c",
            "CREATE TABLE t1 AS FROM VALUES ('A', 1), ('B', 3) t(a, b)",
        ] {
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        }
    }

    #[test]
    fn duckdb_bare_from_values_requires_an_alias() {
        // DuckDB parse-*requires* an alias on the bare row list: a bare `FROM VALUES (1)`
        // is a syntax error (`FROM VALUES (1) t` accepts; probed on 1.5.4), so a missing
        // alias rejects here rather than becoming an unaliased relation.
        for sql in [
            "SELECT * FROM VALUES (1, 2)",
            "SELECT * FROM VALUES (1, 2) WHERE true",
            "FROM VALUES (1), (2)",
        ] {
            let err = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("a bare FROM VALUES needs an alias {sql:?}"));
            assert_eq!(
                err.expected.as_str(),
                "a table alias after a bare `FROM VALUES` row list",
            );
        }
    }

    #[test]
    fn bare_from_values_is_gated_off_outside_duckdb() {
        use crate::dialect::Ansi;

        // Off the `from_values` gate, `VALUES` is not a table name, so a bare
        // `FROM VALUES` is the clean "expected a table name" reject the other dialects
        // give — the parenthesized `FROM (VALUES …)` derived table stays accepted.
        for sql in [
            "SELECT * FROM VALUES (1, 2) AS t",
            "SELECT a FROM VALUES (1), (2) t(a)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects a bare FROM VALUES {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects a bare FROM VALUES {sql:?}",
            );
        }
        // The parenthesized derived table is a different, always-on path — still accepted.
        parse_with(
            "SELECT * FROM (VALUES (1, 2)) AS t",
            crate::ParseConfig::new(Ansi),
        )
        .expect("the parenthesized VALUES derived table stays accepted");
    }

    #[test]
    fn duckdb_bare_from_values_round_trips_without_parentheses() {
        // DuckDb is not a render target; the round-trip renders under Lenient (which also
        // enables `from_values`), proving the `BareValues` spelling re-emits `VALUES (…)`
        // with no wrapping parentheses and the alias trailing.
        for sql in [
            "SELECT c1 FROM VALUES ('CS', 'Bachelor'), ('CS', 'PhD') AS t(c1, c2)",
            "SELECT * FROM VALUES (1, 2) AS t(a, b)",
            "SELECT a FROM VALUES (1), (2) AS t(a) ORDER BY a",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn duckdb_string_literal_table_alias_forms() {
        // DuckDB admits a single-quoted string in table-alias position (probed on 1.5.4):
        // the correlation name after `AS` and each entry of the alias column list. The
        // string's value becomes the identifier, its `'` quote recorded so it round-trips.
        let parsed = parse_with(
            "SELECT t.k FROM integers AS 't'('k') ORDER BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses a string-literal table alias with a string column list");
        let TableFactor::Table { alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        let alias = alias.as_ref().expect("the aliased table carries its alias");
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "t");
        assert_eq!(alias.name.quote, QuoteStyle::Single);
        assert_eq!(alias.columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(alias.columns[0].sym), "k");
        assert_eq!(alias.columns[0].quote, QuoteStyle::Single);

        // The bare-name variant keeps the string only in the column list (`t('k')`).
        let parsed = parse_with(
            "SELECT t.k FROM integers t('k') ORDER BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDb parses a bare alias name with a string column list");
        let TableFactor::Table { alias, .. } = &select_of(&parsed).from[0].relation else {
            panic!("expected a plain table factor");
        };
        let alias = alias.as_ref().expect("the aliased table carries its alias");
        assert_eq!(
            alias.name.quote,
            QuoteStyle::None,
            "bare name is an identifier"
        );
        assert_eq!(alias.columns[0].quote, QuoteStyle::Single);

        // Lenient is the render target (DuckDb is not): the string alias name and string
        // column re-emit verbatim with their `'` quotes. (The bare-name spelling `t('k')`
        // is separately canonicalized to `AS t('k')` by the always-emit-`AS` renderer, a
        // pre-existing behaviour, so only the explicit-`AS` forms round-trip identically.)
        for sql in [
            "SELECT * FROM integers AS 't'('k')",
            "SELECT * FROM integers AS 't'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Lenient))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(Lenient)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn duckdb_string_literal_table_alias_requires_as() {
        // DuckDB accepts the string alias name only after an explicit `AS`: a bare
        // `FROM integers 't'` is an engine reject (probed on 1.5.4), because the string
        // is not a column-name start, so the alias parse never begins and the string is
        // leftover input.
        assert!(
            parse_with(
                "SELECT * FROM integers 't'",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err(),
            "DuckDb rejects a bare (no-`AS`) string table alias",
        );
    }

    #[test]
    fn string_literal_table_alias_gated_off_outside_duckdb() {
        use crate::dialect::{Ansi, MySql};

        // The string spelling is DuckDb-only in table position. MySQL is the sharp case:
        // it accepts a string *column* alias yet rejects a string *table* alias
        // (engine-measured on mysql:8), so the flag is deliberately off there and both
        // forms reject — the string is not a `ColId`.
        for sql in ["SELECT * FROM t AS 'x'", "SELECT * FROM t AS 't'('k')"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL rejects {sql:?}"
            );
        }
    }

    #[test]
    fn postgres_rejects_windowed_function_in_from() {
        use crate::dialect::Postgres;
        // A function in FROM is PostgreSQL's `func_expr_windowless`: no `OVER`, `FILTER`, or
        // `WITHIN GROUP` (`SELECT * FROM rank() OVER (…)` is a syntax error).
        for sql in [
            "SELECT * FROM rank() OVER (ORDER BY random())",
            "SELECT * FROM count(*) FILTER (WHERE true)",
            "SELECT * FROM percentile_cont(0.5) WITHIN GROUP (ORDER BY x)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("a function in FROM is windowless: {sql:?}"));
        }
        // Plain table functions — arguments, a `DISTINCT` quantifier, an in-parenthesis
        // `ORDER BY` — are `func_application` and still parse.
        for sql in [
            "SELECT * FROM generate_series(1, 10)",
            "SELECT * FROM count(DISTINCT x)",
            "SELECT * FROM string_agg(x, ',' ORDER BY x)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        }
    }

    #[test]
    fn window_clause_outranks_a_bare_window_correlation_alias() {
        use crate::ast::Resolver as _;
        use crate::dialect::Sqlite;

        // `WINDOW` is non-reserved in SQLite, so the FROM alias parser used to swallow it
        // as a bare table alias — shadowing the SELECT-level named-window clause, which
        // `parse_window_clause` was wired for but could never reach. The clause wins only
        // on the windowdefn head `<name> AS`, matching SQLite's grammar (probed on
        // 3.43/3.53). Single named window, `t` unaliased:
        let parsed = parse_with(
            "SELECT sum(b) OVER w FROM t WINDOW w AS (ORDER BY a)",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("named WINDOW clause parses");
        let select = select_of(&parsed);
        assert_eq!(select.windows.len(), 1, "one named window `w`");
        assert_eq!(parsed.resolver().resolve(select.windows[0].name.sym), "w");
        let TableFactor::Table { alias: None, .. } = &select.from[0].relation else {
            panic!("`t` must be unaliased — `WINDOW` opens the clause, not a `window` alias");
        };

        // Multiple named windows in one clause.
        let parsed = parse_with(
            "SELECT sum(b) OVER w1, count(*) OVER w2 FROM t WINDOW w1 AS (ORDER BY a), w2 AS (ORDER BY b)",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("multi-window clause parses");
        assert_eq!(select_of(&parsed).windows.len(), 2);

        // A bare `window` that is NOT the clause head stays a correlation alias — SQLite
        // admits `window` as a bare alias (`FROM t window`, `FROM t window WHERE …`).
        let parsed = parse_with(
            "SELECT * FROM t window WHERE a > 0",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("`window` binds as a bare correlation alias");
        let select = select_of(&parsed);
        assert!(select.windows.is_empty(), "no WINDOW clause here");
        let TableFactor::Table {
            alias: Some(alias), ..
        } = &select.from[0].relation
        else {
            panic!("`window` must bind as `t`'s correlation alias");
        };
        assert_eq!(parsed.resolver().resolve(alias.name.sym), "window");

        // An explicit `AS window` is definitely the alias, so a trailing windowdefn is a
        // reject — SQLite rejects `FROM t AS window w AS (…)`; only the bare form pivots.
        assert!(
            parse_with(
                "SELECT * FROM t AS window w AS (ORDER BY a)",
                crate::ParseConfig::new(Sqlite)
            )
            .is_err(),
            "explicit `AS window` is an alias; the trailing windowdefn must reject",
        );
    }
}
