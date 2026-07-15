// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Utility-statement AST nodes (ADR-0012): COPY, EXPLAIN, DESCRIBE, KILL, PRAGMA, ATTACH, DETACH, CHECKPOINT, LOAD, VACUUM, REINDEX.

use super::{
    DataType, Expr, Extension, Ident, Limit, Literal, NoExt, ObjectName, SelectItem, SetAssignment,
    SetParameterValue, Statement, UpdateAssignment, ValuesItem,
};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// A PostgreSQL `COPY` bulk data-transfer statement.
///
/// Moves rows between an external location and either a table or the result of a
/// query: `COPY {<table> [(cols)] | (<query>)} {FROM | TO} {'file' | STDIN | STDOUT
/// | PROGRAM 'cmd'} [ [WITH] (options) | <legacy options> ]`. The two row sources
/// ride [`source`](Self::source); the query source makes this node generic over the
/// extension `X`, since it embeds a [`Statement`].
///
/// One canonical shape: the legacy un-parenthesized option list and the
/// modern parenthesized list share one [`options`](Self::options) list, with the
/// [`parenthesized`](Self::parenthesized) surface tag recording which spelling was
/// written so the construct round-trips. `FROM`/`TO` is recorded by
/// [`direction`](Self::direction) (always `TO` for the query source, which
/// PostgreSQL forbids `FROM` on); the `STDIN`/`STDOUT` keyword that is only
/// semantically valid for one direction is preserved verbatim rather than gated
/// (PostgreSQL accepts either with either direction).
///
/// A handful of legacy table-only surfaces ride dedicated fields rather than the
/// generic option list: the very-old `opt_binary` prefix ([`binary`](Self::binary)),
/// the `[USING] DELIMITERS '<str>'` clause ([`delimiters`](Self::delimiters)), and
/// the `COPY <table> FROM '<file>' WHERE <predicate>` filter ([`filter`](Self::filter),
/// `FROM`-only — PostgreSQL rejects it on `COPY TO`). None are valid on the query
/// source, which the parser enforces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CopyStatement<X: Extension = NoExt> {
    /// The very-old `opt_binary` prefix (`COPY BINARY <table> ...`), distinct from
    /// the `BINARY` [option](CopyOption). Table source only. `true` renders the
    /// leading `BINARY` keyword.
    pub binary: bool,
    /// Input source for this syntax.
    pub source: CopySource<X>,
    /// Whether the copy is `FROM` (load) or `TO` (export); see [`CopyDirection`].
    pub direction: CopyDirection,
    /// Object targeted by this syntax.
    pub target: CopyTarget,
    /// The legacy `[USING] DELIMITERS '<str>'` clause (PostgreSQL `copy_delimiter`),
    /// written between the endpoint and the options; `None` when absent. Only the
    /// delimiter string is kept: the optional `USING` is non-load-bearing
    /// (PostgreSQL `opt_using`) and canonicalizes away like the `WITH` before the
    /// options and the `AS` in `DELIMITER AS '<str>'`.
    pub delimiters: Option<Literal>,
    /// Whether the parenthesized `WITH (opt, ...)` option-list spelling was used
    /// (the always-present surface tag); `false` is the legacy
    /// space-separated list (`CSV`, `DELIMITER ','`). Irrelevant when
    /// [`options`](Self::options) is empty, where neither spelling renders.
    pub parenthesized: bool,
    /// Options supplied in source order.
    pub options: ThinVec<CopyOption>,
    /// The `WHERE <predicate>` row filter of a `COPY <table> FROM <source>`; `None`
    /// when absent. PostgreSQL admits it for `COPY FROM` only (it errors "WHERE
    /// clause not allowed with COPY TO"), so the parser only ever populates this on
    /// a `FROM` table source.
    pub filter: Option<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The row source of a [`CopyStatement`]: a table (with an optional column list) or
/// a parenthesized query.
///
/// PostgreSQL's query source is a parenthesized `PreparableStmt` — a `SELECT`,
/// `INSERT`, `UPDATE`, `DELETE`, or `MERGE` (the parser gates the inner statement to
/// the kinds it models). It is only valid with `TO`, never `FROM`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CopySource<X: Extension = NoExt> {
    /// `COPY <table> [(col, ...)]`: the named relation and its optional column list
    /// (empty when none was written).
    Table {
        /// Table referenced by this syntax.
        table: ObjectName,
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A parenthesized query source (`COPY (<query>) TO …`), valid only with `TO`.
    Query {
        /// Query governed by this node.
        query: Box<Statement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL copy direction forms represented by the AST.
pub enum CopyDirection {
    /// `COPY … FROM` — load data into the table.
    From,
    /// `COPY … TO` — export the table's data.
    To,
}

/// The external endpoint of a `COPY`: a file, the client stream, or a subprocess.
///
/// `STDIN`/`STDOUT` are split so each renders the exact keyword written.
/// PostgreSQL's grammar admits either with either direction (the `TO STDIN` /
/// `FROM STDOUT` mismatch is a later semantic check, not a parse error), so the
/// keyword the parser saw is recorded as-is rather than normalized by direction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CopyTarget {
    /// A server-side file path (`COPY … '<path>'`).
    File {
        /// Path supplied by this syntax.
        path: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The client input stream (`FROM STDIN`).
    Stdin {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The client output stream (`TO STDOUT`).
    Stdout {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A subprocess whose stdio is piped (`COPY … PROGRAM '<cmd>'`).
    Program {
        /// Command text supplied by this syntax.
        command: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One option in a `COPY` option list, e.g. `FORMAT csv`, `DELIMITER ','`, a bare
/// `HEADER`, DuckDB's `ROW_GROUP_SIZE 100000`, or `FORCE_QUOTE (a, b)`.
///
/// The generic key/value escape hatch of the COPY option axis: PostgreSQL's generic
/// option grammar is `ColLabel copy_generic_opt_arg`, so the [`name`](Self::name)
/// admits any word (including keywords such as `NULL`) and the argument is optional.
/// This one `name` + optional [`CopyOptionValue`] shape is a deliberate reshape
/// (ADR-0011) of sqlparser-rs's fixed per-option enums (`CopyOption::Format(Ident)`,
/// `ForceQuote(Vec<Ident>)`, …): rather than enumerate PostgreSQL's known option
/// keywords as variants, every option — PostgreSQL, DuckDB, or an unknown dialect
/// word — rides the same generic pair, so a dialect's format/option vocabulary
/// (DuckDB `COPY … (FORMAT PARQUET, COMPRESSION 'zstd', PARTITION_BY (y, m))`) is
/// carried as typed data without a per-keyword variant. The value *shapes*
/// PostgreSQL's `copy_generic_opt_arg` admits are the [`CopyOptionValue`] axis.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CopyOption {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Value supplied by this syntax.
    pub value: Option<CopyOptionValue>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The argument shapes a [`CopyOption`] can carry — the extensible axis of the COPY
/// option machinery.
///
/// Mirrors PostgreSQL's `copy_generic_opt_arg` alternatives (`opt_boolean_or_string`,
/// `NumericOnly`, `'*'`, and `'(' copy_generic_opt_arg_list ')'`), which pg_query
/// accepts under the plain `copy` gate, plus the legacy [`Force`](Self::Force)
/// compound. DuckDB's and the Snowflake-adjacent parity target's format/option values
/// are a subset of these same shapes, so no widening feature flag is needed. This
/// enum is the pre-staged extension point for the `COPY INTO` sibling: a keyed nested
/// option list (Snowflake `FILE_FORMAT = (TYPE = CSV …)`) becomes an *additive*
/// variant here (a `ThinVec<CopyOption>` payload), riding the same
/// [`CopyOption`]/[`CopyStatement`] machinery rather than reshaping it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CopyOptionValue {
    /// A bareword argument kept as its source word (`FORMAT csv`, `HEADER true`,
    /// DuckDB `FORMAT PARQUET`). PostgreSQL `opt_boolean_or_string`.
    Word {
        /// Identifier-form value.
        word: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A string argument (`DELIMITER ','`, `NULL ''`, DuckDB `COMPRESSION 'zstd'`).
    String {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A numeric argument (`HEADER 1`, DuckDB `ROW_GROUP_SIZE 100000`), including a
    /// sign-folded negative (`-1`) or decimal (`1.5`). PostgreSQL `NumericOnly`; the
    /// sign is folded into the literal's span so it round-trips whole.
    Number {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The bare `*` argument of a generic parenthesized option (`FORCE_QUOTE *`).
    /// PostgreSQL `'*'`; distinct from the legacy [`Force`](Self::Force) all-columns
    /// form, which spells `*` as an empty column list.
    Star {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A parenthesized argument list (`FORCE_QUOTE (a, b)`, DuckDB `PARTITION_BY (y,
    /// m)`). PostgreSQL `'(' copy_generic_opt_arg_list ')'`. The parser only ever
    /// emits [`Word`](Self::Word)/[`String`](Self::String)/[`Number`](Self::Number)
    /// list items (PostgreSQL's list items are `opt_boolean_or_string`; DuckDB adds
    /// numerics); the recursive type keeps one axis rather than a parallel item enum.
    List {
        /// Values in source order.
        values: ThinVec<CopyOptionValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The `{QUOTE | NULL | NOT NULL} {<column-list> | *}` payload of a legacy
    /// `FORCE` option (PostgreSQL `copy_opt_item`), whose compound keyword the
    /// generic `<name> [<value>]` shape cannot carry: the [`name`](CopyOption::name)
    /// is the leading `FORCE` word and this value holds the sub-keyword (which
    /// `kind` distinguishes) and its target. An empty
    /// `columns` list is the `*` (all-columns) form (PostgreSQL
    /// `A_Star`) — unambiguous because PostgreSQL's `columnList` is never empty.
    Force {
        /// Whether the force applies to all columns or a named list; see [`ForceKind`].
        kind: ForceKind,
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A keyed nested option list — the space-separated `<key> = <value>` pairs
    /// inside a Snowflake `FILE_FORMAT = (TYPE = CSV FIELD_DELIMITER = ',')` (or a
    /// `FORMAT_NAME = '...'`) argument. Each element is a [`CopyOption`], so the
    /// nested option vocabulary rides the same machinery as the outer list rather
    /// than a parallel node — the additive extension point ADR-0011 pre-staged on
    /// this enum. Distinct from [`List`](Self::List), whose elements are bare
    /// comma-separated values (`FORCE_QUOTE (a, b)`): an `OptionList` element is a
    /// `key = value` pair and the elements are space-separated. Only
    /// [`CopyIntoStatement`] emits this (via its `= (...)` option grammar);
    /// PostgreSQL/DuckDB `COPY` never does.
    OptionList {
        /// Options supplied in source order.
        options: ThinVec<CopyOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which legacy `FORCE` option a [`CopyOptionValue::Force`] carries.
///
/// A tag (no `meta`): the sub-keyword's span is subsumed by the enclosing
/// [`CopyOptionValue::Force`], exactly as [`CopyDirection`]/[`ExplainFormat`] ride
/// their parent's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForceKind {
    /// `FORCE QUOTE` — force-quote the listed columns (`COPY TO`, CSV).
    Quote,
    /// `FORCE NULL` — match the null string on the listed columns (`COPY FROM`, CSV).
    Null,
    /// `FORCE NOT NULL` — never match the null string on the listed columns
    /// (`COPY FROM`, CSV).
    NotNull,
}

/// A Snowflake `COPY INTO` bulk load/unload statement.
///
/// Moves rows between a table and a location: `COPY INTO <target> FROM <source>
/// [<option> = <value> ...]`. Unlike PostgreSQL's [`CopyStatement`] — which is a
/// `{FROM | TO} <endpoint>` transfer with comma-separated options — the direction
/// is fixed (`INTO <target> FROM <source>`) and the load-vs-unload sense is carried
/// by *which side* is the table: a table [`target`](Self::target) with a location
/// [`source`](Self::source) loads, a location target with a table/query source
/// unloads. It is a sibling statement rather than a variant of [`CopyStatement`]
/// because the two share only the `COPY` keyword: the endpoint model, the option
/// spelling (`KEY = VALUE`, space-separated, with a nested
/// [`OptionList`](CopyOptionValue::OptionList) for `FILE_FORMAT`), and the clause set
/// (`FILES`, `PATTERN`, `VALIDATION_MODE`) are disjoint. This mirrors sqlparser-rs's
/// separate `CopyIntoSnowflake` node.
///
/// Every trailing clause — `FILE_FORMAT = (...)`, `FILES = (...)`, `PATTERN = '...'`,
/// `VALIDATION_MODE = ...`, and the copy options (`ON_ERROR = ...`, `FORCE = TRUE`, …)
/// — rides the one generic [`options`](Self::options) list as a [`CopyOption`] whose
/// value shape captures its argument, exactly the ADR-0011 reshape
/// [`CopyStatement`] uses for PostgreSQL/DuckDB options: no per-keyword field.
///
/// The `@<stage>` internal/named-stage source-target sigil is not modelled yet — it
/// needs a tokenizer stage-reference surface (Snowflake enables no `@` lexer path
/// today) — so [`CopyIntoTarget`]/[`CopyIntoSource`] carry the table and external
/// string-location forms (and the query source), with the stage sigil a tracked
/// follow-up.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CopyIntoStatement<X: Extension = NoExt> {
    /// Object targeted by this syntax.
    pub target: CopyIntoTarget,
    /// Input source for this syntax.
    pub source: CopyIntoSource<X>,
    /// The trailing `<key> = <value>` clauses (`FILE_FORMAT`, `FILES`, `PATTERN`,
    /// `VALIDATION_MODE`, and the copy options), space-separated in source. Each is a
    /// [`CopyOption`] whose [`value`](CopyOption::value) captures the argument shape;
    /// `FILE_FORMAT`'s nested list rides [`CopyOptionValue::OptionList`], `FILES`'s
    /// comma list rides [`CopyOptionValue::List`]. Empty when none were written.
    pub options: ThinVec<CopyOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `INTO <target>` destination of a [`CopyIntoStatement`]: a table (loading) or
/// an external location string (unloading).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CopyIntoTarget {
    /// `COPY INTO [<db>.<schema>.]<table> [(col, ...)]`: the loaded relation and its
    /// optional column list (empty when none was written).
    Table {
        /// Table referenced by this syntax.
        table: ObjectName,
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `COPY INTO '<url>'`: an external location string (`'s3://bucket/path/'`).
    External {
        /// External location referenced by this syntax.
        location: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `COPY INTO @stage[/path]`: a Snowflake stage reference (token span text).
    Stage {
        /// Stage reference used by this syntax.
        reference: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `FROM <source>` origin of a [`CopyIntoStatement`]: a table, an external
/// location string, or a parenthesized transformation query.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CopyIntoSource<X: Extension = NoExt> {
    /// `FROM [<db>.<schema>.]<table>`: an unloaded relation.
    Table {
        /// Table referenced by this syntax.
        table: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FROM '<url>'`: an external location string to load from.
    External {
        /// External location referenced by this syntax.
        location: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FROM @stage[/path]`: a Snowflake stage reference.
    Stage {
        /// Stage reference used by this syntax.
        reference: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FROM ( <query> )`: a transformation query (Snowflake's `COPY INTO <table>
    /// FROM (SELECT ... )` load form). The inner is gated to the query kinds we
    /// model, like [`CopySource::Query`].
    Query {
        /// Query governed by this node.
        query: Box<Statement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A DuckDB `EXPORT DATABASE ['<db>' TO] '<path>' [<copy-options>]` statement: write
/// the catalogue as a directory of `CREATE`/`INSERT`/`COPY` scripts plus data files.
///
/// The two grammar forms (DuckDB `ExportStmt`) are `EXPORT DATABASE '<path>'
/// <copy_options>` and `EXPORT DATABASE <db> TO '<path>' <copy_options>` — the
/// named-database form threads the catalogue name through a *required* `TO` before the
/// path (`EXPORT DATABASE db '<path>'` without `TO` is a parser error, probed on
/// 1.5.4). The presence of [`database`](Self::database) therefore reconstructs the `TO`
/// on render — a `Some` is the `<db> TO '<path>'` spelling, a `None` the bare
/// `'<path>'` — so no separate `to` surface tag is carried.
///
/// The options reuse the PostgreSQL `COPY` option axis verbatim: DuckDB's grammar
/// gives `ExportStmt` the same `copy_options` production `COPY` uses (a legacy
/// space-separated `copy_opt_list` *or* a parenthesized generic list), so the
/// [`options`](Self::options) list is [`CopyOption`]s and [`parenthesized`](Self::parenthesized)
/// records which spelling was written — exactly [`CopyStatement`]'s pair. Unlike `COPY`
/// there is no leading `WITH` (`EXPORT DATABASE '<path>' WITH (...)` is a parser error,
/// probed on 1.5.4), so the parenthesized form renders bare `(...)`. Non-generic: the
/// path is a string literal and [`CopyOption`] carries no extension nodes, so — like
/// [`DetachStatement`] — this node needs no `X`. Gated together with its
/// [`ImportStatement`] inverse by
/// [`UtilitySyntax::export_import_database`](crate::dialect::UtilitySyntax).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExportStatement {
    /// The catalogue (database) name of the `EXPORT DATABASE <db> TO '<path>'` form;
    /// `None` is the bare `EXPORT DATABASE '<path>'` form. A `Some` reconstructs the
    /// required `TO` on render.
    pub database: Option<Ident>,
    /// The destination directory path — a string literal (DuckDB `Sconst`).
    pub path: Literal,
    /// Whether the parenthesized `(opt, ...)` option-list spelling was used (the
    /// surface tag); `false` is the legacy space-separated list (`FORMAT` is not in
    /// that fixed set, but `HEADER`/`CSV`/`DELIMITER '...'` are). Irrelevant when
    /// [`options`](Self::options) is empty, where neither spelling renders — mirroring
    /// [`CopyStatement::parenthesized`].
    pub parenthesized: bool,
    /// Copy options in source order; see [`CopyOption`]. Empty when none were written.
    pub options: ThinVec<CopyOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A DuckDB `IMPORT DATABASE '<path>'` statement: replay the `CREATE`/`INSERT`/`COPY`
/// scripts an [`ExportStatement`] wrote to a directory, reconstructing the catalogue.
///
/// The other half of DuckDB's database export/import round-trip, sharing the
/// [`UtilitySyntax::export_import_database`](crate::dialect::UtilitySyntax) gate (one
/// dialect unit — a dialect that exports databases imports them). The grammar (DuckDB
/// `ImportStmt`) is exactly `IMPORT DATABASE '<path>'`: a single string path and no
/// options (`IMPORT DATABASE '<path>' (...)` is a parser error, probed on 1.5.4).
/// Non-generic, like [`DetachStatement`]: a path literal carries no expressions or
/// extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ImportStatement {
    /// The source directory path — a string literal (DuckDB `Sconst`).
    pub path: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `EXPLAIN` statement: report the planner's query plan for an inner statement.
///
/// Covers both option spellings: the legacy `EXPLAIN [ANALYZE] [VERBOSE]
/// <statement>` keyword prefix and the modern parenthesized list `EXPLAIN ( option
/// [, ...] ) <statement>`. They share one [`options`](Self::options) list; the
/// [`parenthesized`](Self::parenthesized) surface tag records which
/// spelling was written so the construct round-trips, since the legacy prefix can
/// only carry [`Analyze`](ExplainOption::Analyze)/[`Verbose`](ExplainOption::Verbose).
///
/// MySQL spells the same query-plan statement `EXPLAIN`, `DESCRIBE`, or `DESC`
/// interchangeably ([`DESCRIBE SELECT 1`](ExplainKeyword) is `EXPLAIN SELECT 1`), so the
/// leading keyword rides the [`spelling`](Self::spelling) surface tag; PostgreSQL only
/// has `EXPLAIN`, so its parser always sets [`Explain`](ExplainKeyword::Explain). The
/// separate MySQL *table*-metadata overload (`DESCRIBE <table>`) is a different shape —
/// a table name rather than an inner statement — and rides its own
/// [`DescribeStatement`], not this node.
///
/// Which inner statements PostgreSQL actually allows after `EXPLAIN` (a query,
/// `INSERT`/`UPDATE`/`DELETE`, `CREATE TABLE AS`, …) is a semantic restriction left
/// to a later pass; the grammar accepts any [`Statement`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExplainStatement<X: Extension = NoExt> {
    /// Which leading keyword spelled the statement (`EXPLAIN`/`DESCRIBE`/`DESC`);
    /// always [`Explain`](ExplainKeyword::Explain) outside MySQL.
    pub spelling: ExplainKeyword,
    /// Whether the parenthesized form was present in the source.
    pub parenthesized: bool,
    /// Options supplied in source order.
    pub options: ThinVec<ExplainOption>,
    /// Statement governed by this node.
    pub statement: Box<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One option of an [`ExplainStatement`].
///
/// `ANALYZE`/`VERBOSE` carry an optional boolean/word argument (`ANALYZE`,
/// `ANALYZE true`) that is always absent in the legacy keyword-prefix spelling;
/// `FORMAT` names an output format; any other PostgreSQL option (`COSTS`,
/// `BUFFERS`, `SETTINGS`, `WAL`, `TIMING`, `SUMMARY`, …) rides
/// [`Other`](Self::Other) with its optional argument.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ExplainOption {
    /// `ANALYZE` — actually run the statement and report real row counts and timings.
    Analyze {
        /// Value supplied by this syntax.
        value: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `VERBOSE` — include additional plan detail.
    Verbose {
        /// Value supplied by this syntax.
        value: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FORMAT <fmt>` — select the plan output format.
    Format {
        /// The output format (`TEXT`/`XML`/`JSON`/`YAML`); see [`ExplainFormat`].
        format: ExplainFormat,
        /// Source location and node identity.
        meta: Meta,
    },
    /// Any other named option, with its optional bareword/boolean argument.
    Other {
        /// Name referenced by this syntax.
        name: Ident,
        /// Value supplied by this syntax.
        value: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL explain format forms represented by the AST.
pub enum ExplainFormat {
    /// `TEXT` — human-readable plan output (the default).
    Text,
    /// `XML` plan output.
    Xml,
    /// `JSON` plan output.
    Json,
    /// `YAML` plan output.
    Yaml,
}

/// Which leading keyword spelled an EXPLAIN-family statement: the canonical `EXPLAIN`,
/// or MySQL's `DESCRIBE`/`DESC` synonyms.
///
/// A surface tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`ExplainStatement`]/[`DescribeStatement`], exactly as [`ExplainFormat`] rides its
/// parent's span. MySQL accepts all three spellings for both the query-plan form
/// ([`ExplainStatement`]) and the table-metadata form ([`DescribeStatement`]);
/// PostgreSQL has only `EXPLAIN`, so its parser produces only
/// [`Explain`](Self::Explain).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ExplainKeyword {
    /// The canonical `EXPLAIN` keyword (the only spelling PostgreSQL accepts).
    Explain,
    /// MySQL's `DESCRIBE` synonym.
    Describe,
    /// MySQL's abbreviated `DESC` synonym.
    Desc,
}

/// A MySQL `{DESCRIBE | DESC | EXPLAIN} <table> [<column> | '<pattern>']` table-metadata
/// statement (MySQL-specific; gated by
/// [`ShowSyntax::describe`](crate::dialect::UtilitySyntax)).
///
/// MySQL overloads the EXPLAIN-family keywords: followed by an explainable statement they
/// plan a query ([`ExplainStatement`]); followed by a table name they instead report that
/// table's column metadata (the `SHOW COLUMNS FROM <table>` information), which is this
/// node. All three keyword spellings reach both forms, so the leading keyword rides the
/// [`keyword`](Self::keyword) [`ExplainKeyword`] surface tag for round-trip fidelity, as
/// [`ExplainStatement`] carries it. The optional trailing argument narrows the output to
/// one column or a `LIKE`-pattern set ([`column`](Self::column)).
///
/// This form has no PostgreSQL analogue and carries a table name rather than an inner
/// [`Statement`], so it is a distinct node, not a spelling folded onto
/// [`ExplainStatement`]. Non-generic, like [`DetachStatement`]: a table name and an
/// optional column/pattern carry no expressions or extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DescribeStatement {
    /// Which leading keyword was written (`DESCRIBE`/`DESC`/`EXPLAIN`).
    pub keyword: ExplainKeyword,
    /// The table whose columns are described (`table_ident`, possibly schema-qualified —
    /// `DESCRIBE db.t`).
    pub table: ObjectName,
    /// The optional trailing `<column> | '<pattern>'` narrowing; `None` describes every
    /// column.
    pub column: Option<DescribeColumn>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The optional trailing argument of a [`DescribeStatement`]: a single column name or a
/// `LIKE`-style wildcard pattern.
///
/// MySQL's `opt_describe_column` is `ident | text_string`: a bare identifier names one
/// column (`DESCRIBE t col`), a string is a pattern the column list is filtered against
/// (`DESCRIBE t 'a%'`). A dotted name, a `*`, and a second argument are all MySQL syntax
/// errors, so only these two shapes are modelled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DescribeColumn {
    /// A specific column name to describe.
    Name {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `LIKE` wildcard pattern selecting columns to describe.
    Wild {
        /// Pattern matched by this syntax.
        pattern: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `KILL [CONNECTION | QUERY] <id>` statement: terminate a server thread or the
/// statement it is running (MySQL-specific; gated by
/// [`UtilitySyntax::kill`](crate::dialect::UtilitySyntax)).
///
/// The optional [`CONNECTION`/`QUERY`](KillTarget) keyword selects whether the whole
/// connection or just its current query is killed; bare `KILL <id>` defaults to
/// `CONNECTION`, and the [`target`](Self::target) tag keeps the three spellings distinct
/// so they round-trip. The thread id is a full expression in MySQL's grammar
/// (`KILL @id`, `KILL 1 + 1`, and a string `KILL '5'` all prepare), so it rides an
/// [`Expr`], which makes this node generic over the extension `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct KillStatement<X: Extension = NoExt> {
    /// Which thread scope the `CONNECTION`/`QUERY` keyword selected, or that none was
    /// written (the bare form).
    pub target: KillTarget,
    /// The thread/connection id expression.
    pub id: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The optional scope keyword of a [`KillStatement`].
///
/// A surface tag (no `meta`), riding the parent's span like [`CopyDirection`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum KillTarget {
    /// Bare `KILL <id>` — no keyword written (MySQL defaults it to `CONNECTION`).
    Unspecified,
    /// `KILL CONNECTION <id>` — terminate the whole connection.
    Connection,
    /// `KILL QUERY <id>` — terminate only the connection's current statement.
    Query,
}

/// A MySQL `INSTALL` server-administration statement (gated by
/// [`UtilitySyntax::plugin_component_statements`](crate::dialect::UtilitySyntax)) — one of the
/// two forms of `sql_yacc.yy` `install_stmt`. The inverse [`UninstallStatement`] shares the
/// same `plugin_component_statements` gate (an install/uninstall pair, like `ATTACH`/`DETACH`).
///
/// Generic over `X` because the `COMPONENT` form's optional `SET` tail carries [`Expr`]s.
/// MySQL grammar-accepts every well-formed shape but cannot *prepare* the `COMPONENT` forms
/// over the binary protocol (`ER_UNSUPPORTED_PS` 1295) — a bind-time verdict the parse layer
/// does not model; the `PLUGIN` form does prepare.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum InstallStatement<X: Extension = NoExt> {
    /// `INSTALL PLUGIN <name> SONAME <soname>` — load a single plugin from a shared-library
    /// file. The name is a bare/back-quoted [`Ident`] (a quoted-string name is
    /// `ER_PARSE_ERROR` on mysql:8), the `SONAME` a required string [`Literal`]. Exactly one
    /// plugin — a comma list is rejected.
    Plugin {
        /// The plugin name (`ident`, not a quoted string).
        name: Ident,
        /// The `SONAME` shared-library file, a string literal.
        soname: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `INSTALL COMPONENT <urn> [, <urn> …] [SET <var-assignment> [, …]]` — load one or more
    /// components by URN, with an optional `SET` tail of scoped configuration-variable
    /// assignments. Each URN is a string [`Literal`] (`TEXT_STRING_sys_list`); the list is
    /// non-empty.
    Component {
        /// The component URNs, a non-empty list of string literals.
        urns: ThinVec<Literal>,
        /// The optional `SET` tail, empty when none was written; see
        /// [`InstallComponentSetElement`].
        set: ThinVec<InstallComponentSetElement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `UNINSTALL` server-administration statement (gated by
/// [`UtilitySyntax::plugin_component_statements`](crate::dialect::UtilitySyntax)) — the
/// `sql_yacc.yy` `uninstall` rule, the inverse of [`InstallStatement`]. Neither form carries
/// an expression, so — unlike its install counterpart — this node is not generic (the
/// [`PragmaStatement`] precedent).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UninstallStatement {
    /// `UNINSTALL PLUGIN <name>` — unload a single plugin by name (a bare/back-quoted
    /// [`Ident`]; exactly one, a comma list is `ER_PARSE_ERROR` on mysql:8).
    Plugin {
        /// The plugin name (`ident`, not a quoted string).
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `UNINSTALL COMPONENT <urn> [, <urn> …]` — unload one or more components by URN. Unlike
    /// `INSTALL COMPONENT` there is no `SET` tail.
    Component {
        /// The component URNs, a non-empty list of string literals.
        urns: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One assignment in an `INSTALL COMPONENT … SET` tail (`sql_yacc.yy` `install_set_value`):
/// `[GLOBAL | PERSIST] <name> {= | :=} <value>`. A strictly narrower grammar than the general
/// MySQL `SET` — the scope is only the `install_option_type` set (`GLOBAL`/`PERSIST`, default
/// `GLOBAL`; `SESSION`/`LOCAL`/`PERSIST_ONLY` and `@`/`@@` sigils are `ER_PARSE_ERROR` on
/// mysql:8), and the value is only `install_set_rvalue` (`ON` or an [`Expr`]; `DEFAULT` and the
/// other `SET` sentinels reject). The name and assignment operator do reuse the general `SET`
/// machinery (`lvalue_variable`, [`SetAssignment`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct InstallComponentSetElement<X: Extension = NoExt> {
    /// The scope keyword, or `None` for the implicit default (which MySQL treats as `GLOBAL`);
    /// see [`InstallComponentSetScope`].
    pub scope: Option<InstallComponentSetScope>,
    /// The variable name (`lvalue_variable`: a one- or two-part `name[.name]`).
    pub name: ObjectName,
    /// The `=` / `:=` assignment operator (the two are synonyms).
    pub assignment: SetAssignment,
    /// The assigned value; see [`InstallComponentSetValue`].
    pub value: InstallComponentSetValue<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The scope keyword of an [`InstallComponentSetElement`] — the `install_option_type` set. A
/// surface tag (no `meta`), riding the parent's span like [`KillTarget`]. `None` at the parent
/// (not a variant here) is the implicit default, which MySQL resolves to `GLOBAL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum InstallComponentSetScope {
    /// `GLOBAL` — set the global system variable.
    Global,
    /// `PERSIST` — set and persist the global system variable.
    Persist,
}

/// The value of an [`InstallComponentSetElement`] — `sql_yacc.yy` `install_set_rvalue`, exactly
/// `ON` or an expression (the general `SET`'s `DEFAULT`/`ALL`/`BINARY`/`ROW`/`SYSTEM` sentinels
/// are not admitted here).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum InstallComponentSetValue<X: Extension = NoExt> {
    /// The `ON` keyword (folded to the string `"ON"` by the server; kept as a tag so it
    /// round-trips).
    On {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A value expression.
    Expr {
        /// The value expression.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `HANDLER` low-level cursor statement (gated by
/// [`UtilitySyntax::handler_statements`](crate::dialect::UtilitySyntax)) — direct,
/// index-level read access to a table's storage engine that bypasses the optimizer
/// (`sql_yacc.yy` `handler_stmt`).
///
/// One node covers the three verbs on a single opened table, distinguished by
/// [`operation`](Self::operation): `HANDLER <t> OPEN [[AS] alias]`, `HANDLER <t> READ …`,
/// and `HANDLER <t> CLOSE`. The table's name arity differs by verb and is a parse-time
/// constraint, not a field: `OPEN` takes a possibly schema-qualified `table_ident`
/// (`HANDLER db.t OPEN`), while `READ`/`CLOSE` take a bare unqualified `ident` (`HANDLER
/// db.t CLOSE` is `ER_PARSE_ERROR` on mysql:8), so the parser admits a dotted
/// [`table`](Self::table) only under `OPEN`. Generic over `X`: the `READ` key list and
/// `WHERE` filter carry [`Expr`]s.
///
/// MySQL grammar-accepts every well-formed shape but cannot *prepare* a `HANDLER` over the
/// binary protocol (`ER_UNSUPPORTED_PS` 1295; a bare-connection `HANDLER … OPEN` against no
/// default database is `ER_NO_DB_ERROR` 1046) — a bind-time verdict the parse layer does
/// not model.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct HandlerStatement<X: Extension = NoExt> {
    /// The handler's table. Schema-qualified only under the `OPEN` verb (parse-enforced).
    pub table: ObjectName,
    /// Which verb this statement performs; see [`HandlerOperation`].
    pub operation: HandlerOperation<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The verb of a [`HandlerStatement`]: `OPEN`, `CLOSE`, or a `READ` with its selector and
/// shared `WHERE`/`LIMIT` tail.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerOperation<X: Extension = NoExt> {
    /// `HANDLER <t> OPEN [ [AS] <alias> ]` — open a handler on the table, optionally
    /// aliased. [`as_keyword`](Self::Open::as_keyword) records whether the optional `AS`
    /// was written (meaningful only when [`alias`](Self::Open::alias) is `Some`), so the
    /// two spellings round-trip.
    Open {
        /// The optional handler alias; `None` for a bare `HANDLER <t> OPEN`.
        alias: Option<Ident>,
        /// Whether the optional `AS` keyword preceded the alias.
        as_keyword: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `HANDLER <t> CLOSE` — close an open handler on the table.
    Close {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `HANDLER <t> READ <selector> [WHERE <expr>] [LIMIT …]` — fetch rows through the
    /// handler. The `WHERE` filter and `LIMIT` are shared across every read shape (see
    /// [`selector`](Self::Read::selector)); each is `None` when the source writes none.
    Read {
        /// Which rows this read selects; see [`HandlerReadSelector`].
        selector: HandlerReadSelector<X>,
        /// The optional `WHERE` filter, applied after the storage-engine read.
        selection: Option<Expr<X>>,
        /// The optional `LIMIT` clause (`LIMIT n`, `LIMIT off, n`, `LIMIT n OFFSET off` — the
        /// `LimitOffset`/`CommaOffset` shapes of [`Limit`]).
        limit: Option<Limit<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The row-selection shape of a `HANDLER … READ` (`sql_yacc.yy` `handler_scan_function` /
/// `handler_rkey_function` / `handler_rkey_mode`), independent of the shared `WHERE`/`LIMIT`
/// tail carried by [`HandlerOperation::Read`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerReadSelector<X: Extension = NoExt> {
    /// `READ { FIRST | NEXT }` — a full scan in storage order with no index named.
    /// Restricted to `FIRST`/`NEXT` ([`HandlerScanDirection`]); `PREV`/`LAST` require a
    /// named index (`HANDLER t READ PREV` is `ER_PARSE_ERROR` on mysql:8).
    Scan {
        /// The scan direction (`FIRST`/`NEXT` only).
        direction: HandlerScanDirection,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `READ <index> { FIRST | NEXT | PREV | LAST }` — traverse a named index in the given
    /// direction ([`HandlerIndexDirection`], the wider `PREV`/`LAST` set). A `PRIMARY` key
    /// is spelled with the reserved word quoted (`` READ `PRIMARY` ``).
    Index {
        /// The index name.
        index: Ident,
        /// The traversal direction (`FIRST`/`NEXT`/`PREV`/`LAST`).
        direction: HandlerIndexDirection,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `READ <index> <op> ( <value> [, …] )` — seek a named index by key: position at the
    /// first row whose index value satisfies [`comparison`](Self::Key::comparison) against
    /// the key tuple. The key is a non-empty `expr_or_default` list ([`ValuesItem`], so a
    /// bare `DEFAULT` element is admitted like the INSERT values path); `( )` is
    /// `ER_PARSE_ERROR` on mysql:8.
    Key {
        /// The index name.
        index: Ident,
        /// The key comparison operator; see [`HandlerKeyComparison`].
        comparison: HandlerKeyComparison,
        /// The key tuple, a non-empty list of value expressions (`DEFAULT` admitted).
        key: ThinVec<ValuesItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The traversal direction of an indexless `HANDLER … READ { FIRST | NEXT }` scan
/// ([`HandlerReadSelector::Scan`]) — the narrow `handler_scan_function` set. A surface tag
/// (no `meta`), riding the parent's span like [`KillTarget`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerScanDirection {
    /// `FIRST` — the first row in storage order.
    First,
    /// `NEXT` — the next row after the handler's current position.
    Next,
}

/// The traversal direction of an indexed `HANDLER … READ <index> { FIRST | NEXT | PREV |
/// LAST }` ([`HandlerReadSelector::Index`]) — the `handler_rkey_function` set, which adds
/// `PREV`/`LAST` over [`HandlerScanDirection`]. A surface tag (no `meta`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerIndexDirection {
    /// `FIRST` — the first row by the index's key order.
    First,
    /// `NEXT` — the next row in index order.
    Next,
    /// `PREV` — the previous row in index order.
    Prev,
    /// `LAST` — the last row by the index's key order.
    Last,
}

/// The key comparison of a `HANDLER … READ <index> <op> (…)` seek
/// ([`HandlerReadSelector::Key`]) — `sql_yacc.yy` `handler_rkey_mode`. A surface tag (no
/// `meta`). The set is exactly `= >= <= > <`; `<>`/`!=` are `ER_PARSE_ERROR` on mysql:8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerKeyComparison {
    /// `=` — the first row whose index value equals the key.
    Eq,
    /// `>=` — the first row whose index value is greater than or equal to the key.
    GreaterOrEqual,
    /// `<=` — the last row whose index value is less than or equal to the key.
    LessOrEqual,
    /// `>` — the first row whose index value is strictly greater than the key.
    Greater,
    /// `<` — the last row whose index value is strictly less than the key.
    Less,
}

/// A MySQL `CLONE` statement (gated by [`UtilitySyntax::clone`](crate::dialect::UtilitySyntax))
/// — provision a data directory from either the running server or a remote donor
/// (`sql_yacc.yy` `clone_stmt`).
///
/// The single `CLONE` leading keyword splits into two grammar-disjoint forms, so they ride the
/// axis as variants rather than one option-soup node: [`Local`](Self::Local) copies the current
/// instance's data into a new directory, and [`Instance`](Self::Instance) streams a remote
/// donor's data over the wire. Non-generic — every operand is an account name, a string
/// literal, or a port number, never an embedded expression.
///
/// Neither form is preparable over the binary protocol (live mysql:8.4.10: both are
/// `ER_UNSUPPORTED_PS` 1295, grammar-*positive* but PS-declined) — a bind-time verdict the
/// parse layer does not model.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CloneStatement {
    /// `CLONE LOCAL DATA DIRECTORY [=] '<dir>'` — clone the running instance into a new local
    /// directory. The `DATA DIRECTORY` clause is mandatory (a bare `CLONE LOCAL '<dir>'` is
    /// `ER_PARSE_ERROR` on mysql:8).
    Local {
        /// The mandatory target `DATA DIRECTORY [=] '<dir>'`.
        data_directory: CloneDataDirectory,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CLONE INSTANCE FROM <user>[@<host>]:<port> IDENTIFIED BY '<pw>' [DATA DIRECTORY [=]
    /// '<dir>'] [REQUIRE [NO] SSL]` — clone a remote donor instance. The `:<port>` is
    /// mandatory and must abut the donor account with no surrounding whitespace (a space on
    /// either side of the `:` is `ER_PARSE_ERROR` on mysql:8, a raw-offset adjacency check in
    /// the grammar).
    Instance {
        /// The donor account — MySQL's full `user` axis (`<user>[@<host>]` or `CURRENT_USER`),
        /// so the shared [`AccountName`] node is reused.
        source: AccountName,
        /// The donor port; a `ulong_num` integer literal, spelling preserved.
        port: Literal,
        /// The donor password from `IDENTIFIED BY '<pw>'` — a string literal.
        password: Literal,
        /// The optional target `DATA DIRECTORY [=] '<dir>'`; `None` when omitted.
        data_directory: Option<CloneDataDirectory>,
        /// The `REQUIRE [NO] SSL` transport requirement; see [`CloneSsl`].
        ssl: CloneSsl,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `DATA DIRECTORY [=] '<dir>'` target of a [`CloneStatement`] — mandatory in the `LOCAL`
/// form, optional in the `INSTANCE` form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CloneDataDirectory {
    /// Whether the optional `=` was written between `DATA DIRECTORY` and the path, so the two
    /// spellings round-trip.
    pub equals: bool,
    /// The directory path — a string literal (`TEXT_STRING_filesystem`).
    pub path: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `REQUIRE [NO] SSL` transport requirement of a `CLONE INSTANCE` statement (`sql_yacc.yy`
/// `opt_ssl`) — a surface tag (no `meta`), riding the parent's span like [`KillTarget`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CloneSsl {
    /// No `REQUIRE` clause was written (`SSL_TYPE_NOT_SPECIFIED`).
    Unspecified,
    /// `REQUIRE SSL` — encrypted transport is required.
    Require,
    /// `REQUIRE NO SSL` — encrypted transport is disabled.
    RequireNo,
}

/// A MySQL `IMPORT TABLE FROM '<file>' [, '<file>' …]` statement (gated by
/// [`UtilitySyntax::import_table`](crate::dialect::UtilitySyntax)) — recreate tables from
/// serialized `.sdi` metadata files written by a discarded tablespace (`sql_yacc.yy`
/// `import_stmt`).
///
/// The operand is a non-empty comma-separated list of **string** literals
/// (`TEXT_STRING_sys_list`); a bare identifier is `ER_PARSE_ERROR` on mysql:8, so
/// [`files`](Self::files) is a [`Literal`] list, not a name list. Distinct from DuckDB's
/// `IMPORT DATABASE '<dir>'` ([`ImportStatement`]) — both spell the leading `IMPORT`, but the
/// second keyword (`TABLE` vs `DATABASE`) and their separate gates keep them apart. Not
/// preparable over the binary protocol (live mysql:8.4.10: `ER_UNSUPPORTED_PS` 1295).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ImportTableStatement {
    /// The non-empty list of `.sdi` metadata file paths — string literals in source order.
    pub files: ThinVec<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `HELP '<topic>'` statement (gated by
/// [`UtilitySyntax::help_statement`](crate::dialect::UtilitySyntax)) — look up a term in the
/// server's help tables (`sql_yacc.yy` `help`).
///
/// The operand is a single `ident_or_text` (`sql_yacc.yy`), so a bare identifier (`HELP
/// contents`) and a quoted string (`HELP 'contents'`) are both accepted and fold to one
/// [`Ident`] whose quote style round-trips from its span — exactly the [`AccountName`] name
/// treatment. Exactly one operand: a bare `HELP` and a two-operand `HELP 'a' 'b'` are both
/// `ER_PARSE_ERROR` on mysql:8. Not preparable over the binary protocol (live mysql:8.4.10:
/// `ER_UNSUPPORTED_PS` 1295).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct HelpStatement {
    /// The help topic — a bare-or-quoted `ident_or_text`, folded to an [`Ident`].
    pub topic: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `BINLOG '<base64-event>'` statement (gated by
/// [`UtilitySyntax::binlog`](crate::dialect::UtilitySyntax)) — replay a base64-encoded binary
/// log event, the format `mysqlbinlog --base64-output` emits (`sql_yacc.yy`
/// `binlog_base64_event`).
///
/// The operand is a single string literal (`TEXT_STRING_sys`); a bare identifier is
/// `ER_PARSE_ERROR` on mysql:8. Unlike the other server-administration families, `BINLOG` **is**
/// preparable over the binary protocol (live mysql:8.4.10: `PREPARE` accepts a grammar-valid
/// payload; the base64 decode and event application happen only at *execution*, which the
/// parse/prepare layer never reaches), so the family evidence records a `Prepared` outcome, not
/// `ER_UNSUPPORTED_PS`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct BinlogStatement {
    /// The base64-encoded event payload — a string literal.
    pub event: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A SQLite `PRAGMA [<schema> .] <name> [= <value> | (<value>)]` configuration
/// statement (SQLite-specific; gated by
/// [`UtilitySyntax::pragma`](crate::dialect::UtilitySyntax)).
///
/// One canonical shape covers all three surface forms: a bare read
/// (`PRAGMA user_version`) is `value: None`, and the assignment (`= <value>`) and
/// call (`(<value>)`) spellings share one [`value`](Self::value) slot with the
/// [`parenthesized`](Self::parenthesized) surface tag recording which was written —
/// the [`CopyStatement::parenthesized`] pattern, never a second node. The value
/// grammar is SQLite's `signed-number | name | string-literal`, exactly the shape
/// [`SetParameterValue`] models for `SET` (SQLite's `PRAGMA` is its session-config
/// surface), so that node is reused rather than minting a parallel one. A general
/// expression is *not* admitted (`PRAGMA cache_size = 1 + 2` is a SQLite syntax
/// error), which is why the slot is not an [`Expr`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PragmaStatement {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The written value; `None` for the bare interrogative form.
    pub value: Option<SetParameterValue>,
    /// Whether the value was written in the call form `(<value>)` (`true`) or the
    /// assignment form `= <value>` (`false`); irrelevant when [`value`](Self::value)
    /// is `None`, where neither spelling renders.
    pub parenthesized: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A SQLite `ATTACH [DATABASE] <expr> AS <schema>` statement (SQLite-specific;
/// gated — together with its [`DetachStatement`] inverse — by
/// [`UtilitySyntax::attach`](crate::dialect::UtilitySyntax)).
///
/// The database source is a full expression in SQLite's grammar (usually a string
/// literal, but `ATTACH 'a' || '.db' AS x` is legal), which makes this node generic
/// over the extension `X`. The optional `DATABASE` noise keyword is
/// round-trip-significant, so it rides the
/// [`database_keyword`](Self::database_keyword) surface tag. The schema
/// alias is modelled as an [`Ident`] — SQLite's grammar technically admits an
/// expression there too (`AS 'aux'`), a permissiveness we deliberately do not model.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AttachStatement<X: Extension = NoExt> {
    /// Whether the optional `DATABASE` keyword was written; `true` renders it.
    pub database_keyword: bool,
    /// Object targeted by this syntax.
    pub target: Expr<X>,
    /// The attached/detached schema (database) name.
    pub schema: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `DETACH [DATABASE] [IF EXISTS] <schema>` statement — the [`AttachStatement`]
/// inverse, sharing its [`UtilitySyntax::attach`](crate::dialect::UtilitySyntax)
/// gate (one dialect unit) and its `DATABASE` surface tag. Non-generic, like
/// [`CommentOnStatement`](super::CommentOnStatement): a schema name carries no
/// expressions or extension nodes.
///
/// The `IF EXISTS` guard is a DuckDB extension gated by
/// [`UtilitySyntax::detach_if_exists`](crate::dialect::UtilitySyntax) (SQLite's
/// `DETACH` has no such guard). DuckDB admits it only *after* the `DATABASE`
/// keyword — `DETACH DATABASE IF EXISTS x` parses but `DETACH IF EXISTS x` is a
/// parser error (probed on 1.5.4) — so [`if_exists`](Self::if_exists) is `true`
/// only alongside [`database_keyword`](Self::database_keyword).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DetachStatement {
    /// Whether the optional `DATABASE` keyword was written; `true` renders it.
    pub database_keyword: bool,
    /// Whether an `IF EXISTS` guard was written (DuckDB; requires
    /// [`database_keyword`](Self::database_keyword)). SQLite always leaves this `false`.
    pub if_exists: bool,
    /// The attached/detached schema (database) name.
    pub schema: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `[FORCE] CHECKPOINT [<database>]` write-ahead-log flush statement.
///
/// PostgreSQL has the bare `CHECKPOINT` (no operands, gated by
/// [`MaintenanceSyntax::checkpoint`](crate::dialect::UtilitySyntax)); DuckDB extends it
/// with an optional `FORCE` modifier and an optional single database name, both gated
/// by [`MaintenanceSyntax::checkpoint_database`](crate::dialect::UtilitySyntax). The
/// database is a single bare [`Ident`] — DuckDB rejects a dotted `CHECKPOINT a.b` and
/// a quoted-string operand at parse time (probed on 1.5.4). Non-generic, like
/// [`DetachStatement`]: no expressions or extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CheckpointStatement {
    /// Whether the `FORCE` modifier preceded `CHECKPOINT` (DuckDB).
    pub force: bool,
    /// The database to checkpoint; `None` for the bare form. A single bare name, not a
    /// dotted [`ObjectName`] (DuckDB rejects `CHECKPOINT a.b`).
    pub database: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `LOAD <extension>` extension/shared-library load statement.
///
/// PostgreSQL loads a shared library by string path (`LOAD 'plpgsql'`); DuckDB loads
/// an extension by a string (`LOAD 'tpch'`, `LOAD 'path/e.duckdb_extension'`) *or* a
/// bare name (`LOAD tpch`). [`UtilitySyntax::load_extension`](crate::dialect::UtilitySyntax)
/// gates the string form (both dialects);
/// [`UtilitySyntax::load_bare_name`](crate::dialect::UtilitySyntax) admits the DuckDB
/// bare-identifier argument. Non-generic: the argument is a name or a string literal,
/// never a general expression (DuckDB rejects `LOAD 1 + 1`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadStatement {
    /// Object targeted by this syntax.
    pub target: LoadTarget,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The argument of a [`LoadStatement`]: a bare extension name or a string path.
///
/// The two forms round-trip distinctly — `LOAD tpch` keeps its bare spelling and
/// `LOAD 'tpch'` its quoted one — so the parser records which was written rather than
/// canonicalizing to one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadTarget {
    /// A bare identifier extension name (DuckDB `LOAD tpch`).
    Name {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A string-literal path or name (`LOAD 'plpgsql'`, `LOAD 'path/e.duckdb_extension'`).
    Path {
        /// Path supplied by this syntax.
        path: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A DuckDB `UPDATE EXTENSIONS [( <name>, ... )]` extension-refresh statement
/// (DuckDB-specific; gated by
/// [`UtilitySyntax::update_extensions`](crate::dialect::UtilitySyntax)).
///
/// Refreshes installed extensions from their repository; the optional parenthesized
/// list restricts the refresh to the named extensions, and its absence updates every
/// installed extension. Non-generic, like [`ReindexStatement`]: the operands are bare
/// extension names — DuckDB's `opt_column_list` (`ColId` list), a quoted or unquoted
/// identifier, never a string, dotted name, or general expression (all engine-probed
/// rejects on 1.5.4). `extensions` is empty for the bare `UPDATE EXTENSIONS`; when the
/// list is written it carries at least one name (`UPDATE EXTENSIONS ()` is a DuckDB
/// parser error), so an empty vector round-trips as the bare form unambiguously.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UpdateExtensionsStatement {
    /// The extensions to refresh; empty for the bare `UPDATE EXTENSIONS` (all
    /// installed). When non-empty, the written parenthesized `( <name>, ... )` list.
    pub extensions: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `VACUUM …` database-maintenance statement — SQLite's `VACUUM [<schema>] [INTO
/// <expr>]` compaction *and* DuckDB's `VACUUM [ANALYZE] [<table> [(<col>, …)]]`
/// statistics/compaction, one node with each dialect's operands on their own gated
/// fields (the `DetachStatement`/`CheckpointStatement` precedent: extend the shared
/// node with dialect-gated fields rather than mint a dialect-named variant).
///
/// The three SQLite utility maintenance statements (`VACUUM`/`REINDEX`/`ANALYZE`) each
/// ride their own leading-keyword gate rather than one shared flag: they are
/// independent statements, not an inverse pair like `ATTACH`/`DETACH`, so they follow
/// the `copy`/`comment_on` precedent (separate flags even though every shipped dialect
/// toggles them together). The leading `VACUUM` is dispatched under
/// [`MaintenanceSyntax::vacuum`](crate::dialect::MaintenanceSyntax) (SQLite) *or*
/// [`MaintenanceSyntax::vacuum_analyze`](crate::dialect::MaintenanceSyntax) (DuckDB).
///
/// The two dialects' operands are disjoint and each rides its own gate, so at most one
/// dialect's fields are populated by a given parse — under every preset, *including*
/// the permissive union with both gates on. With both gates on the accepted language is
/// the exact union of the two grammars, not their cross product: engine-measured
/// (SQLite 3.x + DuckDB 1.5.4), every hybrid tail (`VACUUM ANALYZE … INTO`, a column
/// list or dotted name before `INTO`) is rejected by BOTH engines, so the parser admits
/// the `INTO` tail only on a SQLite-shaped prefix and a taken `INTO`'s single-part name
/// populates [`schema`](Self::schema), never [`table`](Self::table).
/// - **SQLite** ([`schema`](Self::schema) + [`into`](Self::into)): the grammar
///   (`parse.y`: `cmd ::= VACUUM nm INTO expr`) admits an optional single database name
///   — not a dotted [`ObjectName`], since `VACUUM main.t` is a SQLite syntax error — and
///   an optional `INTO <filename>` whose target is a full expression (`VACUUM INTO 'a' ||
///   '.db'` is legal), which makes this node generic over `X`.
/// - **DuckDB** ([`analyze`](Self::analyze) + [`table`](Self::table) +
///   [`columns`](Self::columns)): an optional `ANALYZE` option — the surviving VACUUM
///   option, spelled either bare (`VACUUM ANALYZE`) or as a parenthesized option list
///   (`VACUUM (ANALYZE)`), the two tracked distinctly on [`VacuumAnalyze`] for
///   round-trip fidelity — an optional *qualified* table (`VACUUM db.t` is legal, unlike
///   SQLite), and an optional parenthesized column list (present only alongside a table,
///   always non-empty — `VACUUM t ()` is a parser error). `ANALYZE` is the *only* option
///   the grammar admits at either layer: 1.5.4's parser rejects `NOWAIT`/`SKIP_TOAST`/any
///   unknown option and the boolean-argument form `(ANALYZE true)`, and its transform
///   throws `NotImplementedException` on `FULL`/`FREEZE`/`VERBOSE`/`disable_page_skipping`
///   — so those never parse and never prepare (engine-measured; see the parser's
///   `parse_vacuum_statement`).
///
/// The invariant is a *parser* guarantee, not a structural one: the representable
/// invalid states (both [`schema`](Self::schema) and [`table`](Self::table) populated,
/// [`columns`](Self::columns) without a table, an empty column list) are deliberately
/// unguarded against manual construction and serde deserialization. This crate has no
/// node-level structural-validation hook, and sibling nodes' shape claims (e.g.
/// [`AnalyzeStatement`]'s "always non-empty" column list) carry the same parse-output
/// scope; a bespoke debug assert on this one node would be an inconsistent one-off. The
/// renderer emits whatever is populated, in the fixed `ANALYZE`, name, columns, `INTO`
/// order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct VacuumStatement<X: Extension = NoExt> {
    /// SQLite: the database (schema) name to compact; `None` for the bare `VACUUM`. A
    /// single name, not schema-qualified (SQLite rejects `VACUUM main.t`). Always `None`
    /// under DuckDB (whose table operand rides [`table`](Self::table)).
    pub schema: Option<Ident>,
    /// SQLite: the `INTO <filename>` target expression; `None` when the clause is absent.
    /// A full expression (SQLite `INTO expr`), not merely a string literal. Always `None`
    /// under DuckDB (no `INTO` form).
    pub into: Option<Expr<X>>,
    /// DuckDB: the `ANALYZE` option and how it was spelled (`VACUUM ANALYZE` vs the
    /// parenthesized `VACUUM (ANALYZE)`), the one vacuum option 1.5.4 admits; `None` when
    /// absent. See [`VacuumAnalyze`]. Always `None` under SQLite.
    pub analyze: Option<VacuumAnalyze>,
    /// DuckDB: the table to vacuum (`VACUUM db.t` or `VACUUM 'table name'`); `None`
    /// when absent. A possibly dotted [`ObjectName`], unlike SQLite's single-ident
    /// [`schema`](Self::schema). Always `None` under SQLite.
    pub table: Option<ObjectName>,
    /// DuckDB: the parenthesized column list restricting the vacuum/analyze; `None` when
    /// the clause is absent. Present only alongside [`table`](Self::table), and always
    /// non-empty (`VACUUM t ()` is a parser error). Always `None` under SQLite.
    pub columns: Option<ThinVec<Ident>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How DuckDB's `ANALYZE` vacuum option was spelled — a round-trip spelling tag, like
/// [`TableKeyword`]. DuckDB 1.5.4 accepts the option two ways with identical meaning
/// (both set the engine's single analyze flag), so the distinction is purely syntactic
/// and exists only to render the input back verbatim.
///
/// `ANALYZE` is the sole option either spelling admits: the parenthesized list rejects
/// every other option keyword and the boolean-argument form (`(ANALYZE true)`) at the
/// parser, and `FULL`/`FREEZE`/`VERBOSE`/`disable_page_skipping` at the transform
/// (engine-measured on libduckdb 1.5.4). A list of repeated `ANALYZE`
/// (`VACUUM (ANALYZE, ANALYZE)`) is accepted and canonicalized to the single
/// [`Parenthesized`](Self::Parenthesized) form — the repeats are semantically
/// idempotent, so no count is preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum VacuumAnalyze {
    /// The bare keyword form, `VACUUM ANALYZE`.
    Keyword,
    /// The parenthesized option-list form, `VACUUM (ANALYZE)`.
    Parenthesized,
}

/// A SQLite `REINDEX [<collation> | [<schema> .] <table-or-index>]`
/// index-rebuild statement (SQLite-specific; gated by
/// [`MaintenanceSyntax::reindex`](crate::dialect::UtilitySyntax)).
///
/// The optional target is a possibly schema-qualified name (SQLite `REINDEX nm dbnm` —
/// a collation, table, or index name; the three are disambiguated by catalogue
/// lookup, not syntax, so one [`ObjectName`] slot covers all). Non-generic, like
/// [`DetachStatement`]: a name carries no expressions. `None` is the bare `REINDEX`
/// (rebuild everything).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ReindexStatement {
    /// Object targeted by this syntax.
    pub target: Option<ObjectName>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `ANALYZE …` statistics-gathering statement — SQLite's `ANALYZE [<schema> |
/// [<schema> .] <table-or-index>]` *and* DuckDB's `ANALYZE [<table> [(<col>, …)]]`,
/// gated by [`MaintenanceSyntax::analyze`](crate::dialect::MaintenanceSyntax).
///
/// Structurally the [`ReindexStatement`] shape — an optional possibly-qualified name
/// (SQLite `ANALYZE nm dbnm`; DuckDB `ANALYZE qualified_name`) shared on
/// [`target`](Self::target) — but a distinct statement with its own gate and node (the
/// `Vacuum`/`Reindex`/`Analyze` trio are three separate statements, not one parametric
/// family). `None` [`target`](Self::target) is the bare `ANALYZE` (analyze the whole
/// database).
///
/// DuckDB extends the target with an optional parenthesized column list
/// ([`columns`](Self::columns), gated by
/// [`MaintenanceSyntax::analyze_columns`](crate::dialect::MaintenanceSyntax)); DuckDB's
/// `VERBOSE` option never parses (1.5.4's transform throws
/// `NotImplementedException`), so no verbose surface is modelled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AnalyzeStatement {
    /// Object targeted by this syntax.
    pub target: Option<ObjectName>,
    /// DuckDB: the parenthesized column list restricting the analyze; `None` when the
    /// clause is absent. Present only alongside a [`target`](Self::target), and always
    /// non-empty (`ANALYZE t ()` is a parser error). Always `None` under SQLite.
    pub columns: Option<ThinVec<Ident>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL table-administration maintenance statement — one of the five admin-table
/// verbs `{ANALYZE | CHECK | CHECKSUM | OPTIMIZE | REPAIR} {TABLE | TABLES} <table-list>
/// [options]` (gated by
/// [`MaintenanceSyntax::table_maintenance`](crate::dialect::MaintenanceSyntax)).
///
/// The five verbs share one shape — a verb, an optional `NO_WRITE_TO_BINLOG | LOCAL`
/// binlog-suppression prefix (ANALYZE/OPTIMIZE/REPAIR only), the `TABLE`/`TABLES`
/// keyword, a comma-separated table list, and per-verb trailing options. That shared
/// spine (the [`table_keyword`](Self::table_keyword) synonym tag and the
/// [`tables`](Self::tables) list) is hoisted here once, and the verb rides the
/// [`kind`](Self::kind) axis carrying only what differs, rather than five bespoke
/// statement nodes. Non-generic: every operand is a name, keyword flag, column list, or
/// integer literal — no embedded expression.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct TableMaintenanceStatement {
    /// The verb and its per-verb options; see [`TableMaintenanceKind`].
    pub kind: TableMaintenanceKind,
    /// Whether the shared table keyword was written `TABLE` or its `TABLES` synonym.
    pub table_keyword: TableKeyword,
    /// The comma-separated target table list (each schema-qualifiable).
    pub tables: ThinVec<ObjectName>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The verb of a [`TableMaintenanceStatement`] and its per-verb options — the axis on
/// which the five MySQL admin-table verbs differ.
///
/// The `NO_WRITE_TO_BINLOG | LOCAL` prefix is carried only by the verbs whose grammar
/// admits it (`ANALYZE`/`OPTIMIZE`/`REPAIR`; `CHECK`/`CHECKSUM` have none). `CHECK` and
/// `REPAIR` take an order-preserving, repeatable option list (MySQL OR's the flags but
/// the written order/repeats round-trip from the [`ThinVec`]); `CHECKSUM` takes a single
/// mutually-exclusive option; `OPTIMIZE` takes none.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableMaintenanceKind {
    /// `ANALYZE [NO_WRITE_TO_BINLOG | LOCAL] {TABLE | TABLES} <list> [histogram-tail]`.
    Analyze {
        /// The optional binlog-suppression prefix.
        no_write_to_binlog: Option<NoWriteToBinlog>,
        /// The optional `{UPDATE | DROP} HISTOGRAM ON <cols>` tail.
        histogram: Option<AnalyzeHistogram>,
        /// Source location and node identity (the whole statement span; the hoisted
        /// spine sits between the prefix and the tail, so the verb payload has no
        /// contiguous sub-span of its own — the `VACUUM` dual-`meta` convention).
        meta: Meta,
    },
    /// `CHECK {TABLE | TABLES} <list> [<check-option> ...]`.
    Check {
        /// The repeatable, order-preserving check-type option list.
        options: ThinVec<CheckTableOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CHECKSUM {TABLE | TABLES} <list> [QUICK | EXTENDED]`.
    Checksum {
        /// The single optional `QUICK`/`EXTENDED` mode.
        option: Option<ChecksumTableOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OPTIMIZE [NO_WRITE_TO_BINLOG | LOCAL] {TABLE | TABLES} <list>`.
    Optimize {
        /// The optional binlog-suppression prefix.
        no_write_to_binlog: Option<NoWriteToBinlog>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REPAIR [NO_WRITE_TO_BINLOG | LOCAL] {TABLE | TABLES} <list> [<repair-option> ...]`.
    Repair {
        /// The optional binlog-suppression prefix.
        no_write_to_binlog: Option<NoWriteToBinlog>,
        /// The repeatable, order-preserving repair-type option list.
        options: ThinVec<RepairTableOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `TABLE` vs its interchangeable `TABLES` synonym in the admin-table verbs — a
/// round-trip spelling tag (MySQL's `table_or_tables`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableKeyword {
    /// The singular `TABLE` spelling.
    Table,
    /// The plural `TABLES` spelling.
    Tables,
}

/// The MySQL `NO_WRITE_TO_BINLOG | LOCAL` binlog-suppression prefix on
/// `ANALYZE`/`OPTIMIZE`/`REPAIR TABLE`.
///
/// `LOCAL` is an exact synonym of `NO_WRITE_TO_BINLOG` (both set the same
/// don't-replicate flag), so the written spelling rides this tag for round-trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NoWriteToBinlog {
    /// The `NO_WRITE_TO_BINLOG` spelling.
    NoWriteToBinlog,
    /// The `LOCAL` synonym spelling.
    Local,
}

/// The `ANALYZE TABLE ... {UPDATE | DROP} HISTOGRAM ON <cols>` histogram-management tail.
///
/// Only one tail may appear. The 8.4 extensions to the `UPDATE` form — `{AUTO | MANUAL}
/// UPDATE` and `USING DATA '<json>'` — are deliberately not modelled here; the measured
/// grammar this node covers is `UPDATE HISTOGRAM ON <cols> [WITH <n> BUCKETS]` and `DROP
/// HISTOGRAM ON <cols>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AnalyzeHistogram {
    /// `UPDATE HISTOGRAM ON <cols> [WITH <n> BUCKETS]`.
    Update {
        /// The columns whose histogram is (re)built.
        columns: ThinVec<Ident>,
        /// The optional `WITH <n> BUCKETS` bucket count (an unsigned integer literal).
        buckets: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP HISTOGRAM ON <cols>`.
    Drop {
        /// The columns whose histogram is dropped.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A `CHECK TABLE` check-type option (`opt_mi_check_types`).
///
/// Grammatically repeatable and order-free (MySQL OR's the flags); the parser preserves
/// the written order and repeats in the option list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CheckTableOption {
    /// `FOR UPGRADE`.
    ForUpgrade,
    /// `QUICK`.
    Quick,
    /// `FAST`.
    Fast,
    /// `MEDIUM`.
    Medium,
    /// `EXTENDED`.
    Extended,
    /// `CHANGED`.
    Changed,
}

/// A `CHECKSUM TABLE` option (`opt_checksum_type`) — single and mutually exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ChecksumTableOption {
    /// `QUICK`.
    Quick,
    /// `EXTENDED`.
    Extended,
}

/// A `REPAIR TABLE` repair-type option (`mi_repair_type`).
///
/// Grammatically repeatable and order-free (MySQL OR's the flags); the parser preserves
/// the written order and repeats in the option list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RepairTableOption {
    /// `QUICK`.
    Quick,
    /// `EXTENDED`.
    Extended,
    /// `USE_FRM`.
    UseFrm,
}

/// A MySQL `CACHE INDEX` statement — assign a table's indexes to a named key cache (gated by
/// [`UtilitySyntax::key_cache_statements`](crate::dialect::UtilitySyntax)).
///
/// The MyISAM-era key-cache management pair, with [`LoadIndexStatement`]. Two grammar arms
/// share the `<table> [<key-list>]` per-table shape but are mutually exclusive on the
/// partition axis (`sql_yacc.yy` `keycache_stmt`), so they ride the [`CacheIndexTargets`]
/// enum rather than an optional-partition field that could pair a partition with a table
/// list: `CACHE INDEX <t> [<keys>][, <t> [<keys>] ...] IN <cache>` (the multi-table
/// [`Tables`](CacheIndexTargets::Tables) arm, no partition) and `CACHE INDEX <t> PARTITION
/// (...) [<keys>] IN <cache>` (the single-table [`Partition`](CacheIndexTargets::Partition)
/// arm). Measured on mysql:8.4.10: a table list with a `PARTITION` clause, and a `PARTITION`
/// written after the key list, both `ER_PARSE_ERROR`. Carries no [`Expr`], so it is not
/// generic over `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CacheIndexStatement {
    /// The table(s) whose indexes are assigned, and — in the partitioned arm — the selected
    /// partitions; see [`CacheIndexTargets`].
    pub targets: CacheIndexTargets,
    /// The destination key cache named by the trailing `IN <cache>`; see [`KeyCacheName`].
    pub cache: KeyCacheName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The table target(s) of a [`CacheIndexStatement`] — the list-vs-partition axis of MySQL's
/// `keycache_stmt`.
///
/// The two arms are mutually exclusive: the multi-table list carries no partition, and the
/// partitioned form is restricted to a single table. Modelling them as one enum keeps the
/// invalid "table list + partition" state unrepresentable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CacheIndexTargets {
    /// `<t> [<keys>][, <t> [<keys>] ...]` — one or more tables, each with an optional key
    /// list, and no partitioning (the `keycache_list` arm; a single unpartitioned table is
    /// the one-element list).
    Tables {
        /// The per-table assignments in source order (always non-empty).
        tables: ThinVec<CacheIndexTable>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<t> PARTITION (...) [<keys>]` — a single table restricted to the named partitions,
    /// with an optional key list written *after* the partition clause (the `adm_partition`
    /// arm; the grammar admits no table list here).
    Partition {
        /// The single target table.
        table: ObjectName,
        /// The `PARTITION (ALL | <names>)` selection; see [`PartitionSelection`].
        partition: PartitionSelection,
        /// The optional trailing `{INDEX | KEY} (<keys>)` list; see [`CacheIndexKeyList`].
        keys: Option<CacheIndexKeyList>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<table> [{INDEX | KEY} (<keys>)]` assignment in a [`CacheIndexTargets::Tables`] list
/// (MySQL's `assign_to_keycache`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CacheIndexTable {
    /// The (schema-qualifiable) table name.
    pub table: ObjectName,
    /// The optional `{INDEX | KEY} (<keys>)` list; see [`CacheIndexKeyList`].
    pub keys: Option<CacheIndexKeyList>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `LOAD INDEX INTO CACHE` statement — preload a table's index blocks into its key
/// cache (gated by
/// [`UtilitySyntax::key_cache_statements`](crate::dialect::UtilitySyntax)).
///
/// The preload half of the key-cache pair with [`CacheIndexStatement`]; it takes no `IN
/// <cache>` (the index is loaded into the table's already-assigned cache). Same list-vs-
/// partition exclusivity (`sql_yacc.yy` `preload_stmt`), plus a per-table `IGNORE LEAVES`
/// flag written *after* the key list — see [`LoadIndexTargets`]. Measured on mysql:8.4.10:
/// `IGNORE LEAVES` before the key list, a partition with a table list, and a trailing `IN
/// <cache>` all `ER_PARSE_ERROR`. Carries no [`Expr`], so it is not generic over `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadIndexStatement {
    /// The table(s) whose indexes are preloaded, and — in the partitioned arm — the selected
    /// partitions; see [`LoadIndexTargets`].
    pub targets: LoadIndexTargets,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The table target(s) of a [`LoadIndexStatement`] — the list-vs-partition axis of MySQL's
/// `preload_stmt`, mirroring [`CacheIndexTargets`] with a per-table `IGNORE LEAVES` flag.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadIndexTargets {
    /// `<t> [<keys>] [IGNORE LEAVES][, ...]` — one or more tables, each with an optional key
    /// list and `IGNORE LEAVES` flag, and no partitioning (the `preload_list` arm).
    Tables {
        /// The per-table preloads in source order (always non-empty).
        tables: ThinVec<LoadIndexTable>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<t> PARTITION (...) [<keys>] [IGNORE LEAVES]` — a single table restricted to the
    /// named partitions (the partitioned `preload_stmt` arm; no table list here).
    Partition {
        /// The single target table.
        table: ObjectName,
        /// The `PARTITION (ALL | <names>)` selection; see [`PartitionSelection`].
        partition: PartitionSelection,
        /// The optional trailing `{INDEX | KEY} (<keys>)` list; see [`CacheIndexKeyList`].
        keys: Option<CacheIndexKeyList>,
        /// Whether the trailing `IGNORE LEAVES` flag (skip non-leaf index blocks) was written.
        ignore_leaves: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<table> [{INDEX | KEY} (<keys>)] [IGNORE LEAVES]` preload in a
/// [`LoadIndexTargets::Tables`] list (MySQL's `preload_keys`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadIndexTable {
    /// The (schema-qualifiable) table name.
    pub table: ObjectName,
    /// The optional `{INDEX | KEY} (<keys>)` list; see [`CacheIndexKeyList`].
    pub keys: Option<CacheIndexKeyList>,
    /// Whether the trailing `IGNORE LEAVES` flag was written (skip non-leaf index blocks).
    pub ignore_leaves: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The optional `{INDEX | KEY} (<key>[, ...])` list shared by the key-cache statements
/// (MySQL's `opt_cache_key_list` in its non-empty form).
///
/// `INDEX` and `KEY` are exact synonyms — the written spelling rides
/// [`keyword`](Self::keyword) for round-trip. The parenthesized list may be empty
/// (`INDEX ()`); the parentheses are always written, so an empty [`keys`](Self::keys) is
/// distinct from the clause's absence (the enclosing `Option<CacheIndexKeyList>` being
/// `None`). Each key is an index name, or the `PRIMARY` keyword naming the primary key —
/// admitted here unquoted (`key_usage_element: ident | PRIMARY_SYM`) and preserved as an
/// unquoted [`Ident`] (a backtick-quoted `` `PRIMARY` `` round-trips as a quoted name).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CacheIndexKeyList {
    /// Whether the list was introduced with `INDEX` or its `KEY` synonym.
    pub keyword: CacheIndexKeyword,
    /// The index names in source order; possibly empty (the `INDEX ()` form).
    pub keys: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `INDEX` vs `KEY` spelling of a [`CacheIndexKeyList`] — exact synonyms, recorded so the
/// written keyword round-trips. A surface tag (no `meta`, like [`TableKeyword`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CacheIndexKeyword {
    /// The `INDEX` spelling.
    Index,
    /// The `KEY` spelling.
    Key,
}

/// The destination key cache of a [`CacheIndexStatement`]'s `IN <cache>` clause (MySQL's
/// `key_cache_name`): a named cache or the `DEFAULT` server key cache.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum KeyCacheName {
    /// `IN <cache_name>` — a named key cache.
    Named {
        /// The key-cache name identifier.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IN DEFAULT` — the server's built-in default key cache (`default_key_cache_base`).
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `PARTITION (ALL | <name>[, ...])` selection of a partitioned key-cache statement
/// (MySQL's `adm_partition` / `all_or_alt_part_name_list`): every partition, or a named set.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PartitionSelection {
    /// `PARTITION (ALL)` — every partition of the table.
    All {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PARTITION (<name>[, ...])` — the named partitions (always non-empty).
    Names {
        /// The partition-name identifiers in source order.
        names: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL standalone object-rename statement — `RENAME TABLE <a> TO <b>[, ...]` or
/// `RENAME USER <u> TO <v>[, ...]` (gated by
/// [`UtilitySyntax::rename_statement`](crate::dialect::UtilitySyntax)).
///
/// The two forms share the `RENAME <keyword> <old> TO <new>[, ...]` rename-list shape but
/// carry different element types — schema-qualifiable table names vs. MySQL account names
/// — so they ride the axis as distinct variants rather than one option-soup list. This is
/// the *standalone* `RENAME` statement; the `ALTER TABLE ... RENAME TO` sub-clause is a
/// separate construct.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RenameStatement {
    /// `RENAME {TABLE | TABLES} <from> TO <to>[, <from> TO <to> ...]`.
    Table {
        /// Whether the keyword was written `TABLE` or its `TABLES` synonym.
        table_keyword: TableKeyword,
        /// The `<from> TO <to>` table-rename mappings in source order.
        renames: ThinVec<TableRename>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RENAME USER <from> TO <to>[, <from> TO <to> ...]`.
    User {
        /// The `<from> TO <to>` user-rename mappings in source order.
        renames: ThinVec<UserRename>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<from> TO <to>` table-rename mapping in a `RENAME TABLE` statement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct TableRename {
    /// The existing table name.
    pub from: ObjectName,
    /// The new table name.
    pub to: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<from> TO <to>` account-rename mapping in a `RENAME USER` statement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UserRename {
    /// The existing account name.
    pub from: AccountName,
    /// The new account name.
    pub to: AccountName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL account name — the full `user` grammar axis: a named `<user>[@<host>]` account
/// or the `CURRENT_USER [()]` self-reference.
///
/// This is the shared account-reference node the whole MySQL user/role surface rides —
/// `RENAME USER`, `CREATE`/`ALTER`/`DROP USER`, `CREATE`/`DROP ROLE`, and (via the same
/// spellings) role lists. Each name part of a named account is a MySQL `ident_or_text` (a
/// bare/backtick identifier or a quoted `'…'`/`"…"` string), folded to an [`Ident`] whose
/// quote style round-trips from its span (`'u'@'localhost'`, `` u@`localhost` ``,
/// `u@localhost`). A bare user with no `@host` leaves [`host`](AccountName::Account::host)
/// `None` — the server reads that as `@'%'`, but the absent host is preserved as written
/// rather than materialised.
///
/// [`Definer`](super::Definer) is the parallel *minimal* account reference a routine header
/// carries (`DEFINER = <user>`); it mirrors this node's `Account`/`CurrentUser` split and is
/// slated to fold into this shared node once the routine/trigger/event landings that consume
/// it converge.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AccountName {
    /// `<user>[@<host>]` — a named account. Both parts round-trip their source spelling
    /// (quote style included) from their [`Ident`] spans.
    Account {
        /// The user-name part (before any `@`).
        user: Ident,
        /// The optional `@<host>` part; `None` for a bare user name.
        host: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CURRENT_USER [()]` — the session's current account. Never valid as a *role* name, but
    /// accepted anywhere the grammar's `user` non-terminal is (user lists, `DEFAULT ROLE`
    /// targets bind it as an account, not a role).
    CurrentUser {
        /// Whether the empty `()` call-style parentheses were written.
        parens: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `FLUSH [NO_WRITE_TO_BINLOG | LOCAL] <target>` server-administration statement
/// (gated by [`UtilitySyntax::flush`](crate::dialect::UtilitySyntax)).
///
/// `FLUSH` reloads or clears one of the server's internal caches or logs. The optional
/// `NO_WRITE_TO_BINLOG | LOCAL` prefix — shared with the admin-table verbs, so it reuses
/// [`NoWriteToBinlog`] — suppresses binary-log replication of the flush. *What* is flushed
/// rides the [`FlushTarget`] axis: MySQL's `flush_options` grammar splits into the
/// `{TABLE | TABLES} [<list>] [WITH READ LOCK | FOR EXPORT]` form and a comma-separated list
/// of keyword targets, and the two are mutually exclusive — `FLUSH TABLES, LOGS` and
/// `FLUSH LOGS, TABLES` are both `ER_PARSE_ERROR` on mysql:8.4.10, `TABLES` never joining the
/// list. Non-generic: every operand is a table name, keyword flag, or channel string — no
/// embedded expression.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FlushStatement {
    /// The optional `NO_WRITE_TO_BINLOG | LOCAL` binlog-suppression prefix.
    pub no_write_to_binlog: Option<NoWriteToBinlog>,
    /// What is flushed; see [`FlushTarget`].
    pub target: FlushTarget,
    /// Source location and node identity.
    pub meta: Meta,
}

/// What a [`FlushStatement`] flushes — MySQL's mutually-exclusive `flush_options` split.
///
/// The `{TABLE | TABLES}` form ([`Tables`](Self::Tables)) and the comma-separated keyword
/// list ([`Options`](Self::Options)) are distinct grammar alternatives, not two shapes of one
/// list: `TABLES` cannot appear inside the option list and the list members cannot follow
/// `TABLES` (both `ER_PARSE_ERROR` on mysql:8.4.10), so they ride the axis as two variants
/// rather than one option-soup list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FlushTarget {
    /// `{TABLE | TABLES} [<tbl>[, ...]] [WITH READ LOCK | FOR EXPORT]`.
    ///
    /// The table list may be empty (`FLUSH TABLES`, `FLUSH TABLES WITH READ LOCK`). `FOR
    /// EXPORT` *requires* a non-empty list — `FLUSH TABLES FOR EXPORT` is `ER_PARSE_ERROR`
    /// (`ER_NO_TABLES_USED`) on mysql:8.4.10 — while `WITH READ LOCK` admits the empty list;
    /// the parser enforces that boundary and never produces [`ForExport`](FlushTablesLock::ForExport)
    /// with an empty [`tables`](Self::Tables::tables). The `TABLE`/`TABLES` spelling reuses
    /// the shared [`TableKeyword`] synonym tag.
    Tables {
        /// Whether the keyword was written `TABLE` or its `TABLES` synonym.
        table_keyword: TableKeyword,
        /// The comma-separated target table list (each schema-qualifiable); empty when no
        /// list was written.
        tables: ThinVec<ObjectName>,
        /// The optional trailing `WITH READ LOCK | FOR EXPORT` lock clause.
        lock: Option<FlushTablesLock>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A comma-separated list of keyword flush targets (`FLUSH LOGS`, `FLUSH PRIVILEGES,
    /// STATUS`); see [`FlushOption`]. Always non-empty — the grammar's `flush_options_list`
    /// carries at least one member.
    Options {
        /// The keyword targets in source order.
        options: ThinVec<FlushOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The trailing lock clause on the `FLUSH TABLES` form — MySQL's `opt_flush_lock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FlushTablesLock {
    /// `WITH READ LOCK` — flush and hold a global read lock (accepted with or without a
    /// table list).
    WithReadLock,
    /// `FOR EXPORT` — flush and lock the named tables for a transportable-tablespace export
    /// (requires a non-empty table list; see [`FlushTarget::Tables`]).
    ForExport,
}

/// One keyword target in a [`FlushTarget::Options`] list — MySQL's `flush_option`.
///
/// Every member is a bare keyword or keyword pair except [`RelayLogs`](Self::RelayLogs),
/// which carries an optional `FOR CHANNEL '<name>'` qualifier. Measured against MySQL 8.4.10
/// (`sql_yacc.yy` `flush_option`): the pre-8.0 `QUERY CACHE`, `DES_KEY_FILE`, and `HOSTS`
/// targets are gone (all `ER_PARSE_ERROR`), and `USER_RESOURCES` is the sole accepted
/// spelling of the user-resource reset — bare `RESOURCES` is `ER_PARSE_ERROR` even though the
/// yacc token is *named* `RESOURCES`, because the lexer maps only the `USER_RESOURCES` keyword
/// text onto it. A spanned enum (each variant carries its own [`Meta`], like [`ShowTarget`]):
/// [`RelayLogs`](Self::RelayLogs) holds a spanned channel [`Literal`], so every sibling
/// records its own list-element span too.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FlushOption {
    /// `PRIVILEGES` — reload the grant tables.
    Privileges {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `LOGS` — close and reopen all log files.
    Logs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `BINARY LOGS` — rotate the binary log.
    BinaryLogs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ENGINE LOGS` — flush the storage-engine logs.
    EngineLogs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ERROR LOGS` — close and reopen the error log.
    ErrorLogs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `GENERAL LOGS` — close and reopen the general query log.
    GeneralLogs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SLOW LOGS` — close and reopen the slow query log.
    SlowLogs {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RELAY LOGS [FOR CHANNEL '<name>']` — rotate the relay log, optionally for one named
    /// replication channel (`opt_channel`).
    RelayLogs {
        /// The optional `FOR CHANNEL '<name>'` qualifier (a string literal); `None` when no
        /// channel was named.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `STATUS` — reset the session status counters.
    Status {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `USER_RESOURCES` — reset the per-account resource counters.
    UserResources {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OPTIMIZER_COSTS` — reload the optimizer cost constants.
    OptimizerCosts {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `PURGE BINARY LOGS {TO '<log>' | BEFORE <datetime>}` binary-log purge statement
/// (gated by [`UtilitySyntax::purge_binary_logs`](crate::dialect::UtilitySyntax)).
///
/// Deletes binary-log files up to a named file or a cutoff time. The `BINARY` keyword is
/// fixed: MySQL 8.4 removed the deprecated `MASTER` synonym (`PURGE MASTER LOGS` is
/// `ER_PARSE_ERROR` on mysql:8.4.10), so there is no spelling axis. Exactly one target clause
/// is required — a bare `PURGE BINARY LOGS` is `ER_PARSE_ERROR` — riding the [`PurgeTarget`]
/// axis. The `BEFORE` form takes a full datetime *expression* (`BEFORE NOW() - INTERVAL 3 DAY`
/// parses), which makes this node generic over the extension `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PurgeStatement<X: Extension = NoExt> {
    /// The required `TO '<log>'` or `BEFORE <datetime>` target clause; see [`PurgeTarget`].
    pub target: PurgeTarget<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The required target clause of a [`PurgeStatement`] — MySQL's `purge_option`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PurgeTarget<X: Extension = NoExt> {
    /// `TO '<log>'` — purge every binary log up to (not including) the named file.
    To {
        /// The binary-log file name (a string literal, `TEXT_STRING_sys`).
        log: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `BEFORE <datetime>` — purge every binary log written before the cutoff time.
    Before {
        /// The cutoff datetime expression (`'2000-01-01 00:00:00'`, `NOW() - INTERVAL 3 DAY`,
        /// ...).
        datetime: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A `USE <schema>` (MySQL) / `USE <catalog> [. <schema>]` (DuckDB) catalog/schema-switch
/// statement (gated by
/// [`UtilitySyntax::use_statement`](crate::dialect::UtilitySyntax)).
///
/// Sets the default catalog and schema for subsequent unqualified names — DuckDB's
/// engine implements it as a `SET schema`, MySQL's as `SQLCOM_CHANGE_DB`, but syntactically
/// it is its own statement. The name arity is dialect data gated by
/// [`UtilitySyntax::use_qualified_name`](crate::dialect::UtilitySyntax): DuckDB accepts
/// `USE db` and `USE db.schema` but rejects a three-part `USE a.b.c` at parse time
/// (`Expected "USE database" or "USE database.schema"`), while MySQL's `USE ident` takes a
/// single unqualified schema and `ER_PARSE_ERROR`s any dotted name (engine-measured on
/// mysql:8) — so this holds an [`ObjectName`] of one or two [`Ident`]s and the parser
/// enforces whichever bound the dialect does. Non-generic, like [`DetachStatement`]: a
/// qualified name carries no expressions or extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UseStatement {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `PREPARE <name> [ ( <type> [, ...] ) ] AS <statement>` prepared-statement
/// definition (DuckDB; gated by
/// [`UtilitySyntax::prepared_statements`](crate::dialect::UtilitySyntax)).
///
/// Binds a session-scoped name to a parameterized statement (`PREPARE v1 AS SELECT
/// 'Test' LIMIT ?`). The body embeds a [`Statement`], which makes this node generic
/// over the extension `X`, and follows the [`ExplainStatement`] contract:
/// the grammar accepts *any* [`Statement`] and leaves the preparable-kind restriction
/// to a later pass — DuckDB rejects a non-preparable body at bind, not parse.
///
/// The prepared-statement name is a bare [`Ident`], not a dotted [`ObjectName`]: the
/// name lives in a flat session namespace, not the catalogue. DuckDB rejects the
/// PostgreSQL `PREPARE name ( <type> [, ...] ) AS ...` argument-type list ("Prepared
/// statement argument types are not supported, use CAST"), so
/// [`parameter_types`](Self::parameter_types) is gated by its own
/// [`prepare_typed_parameters`](crate::dialect::UtilitySyntax::prepare_typed_parameters)
/// flag, independent of the base `PREPARE`/`EXECUTE`/`DEALLOCATE` dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PrepareStatement<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The PostgreSQL parenthesized parameter-type list (`PREPARE p(int, text) AS
    /// ...`), in source order; empty when the parentheses were not written.
    /// PostgreSQL rejects an empty written `()`, so an empty list unambiguously means
    /// "clause absent" and needs no separate surface tag — the same
    /// absent-vs-empty-not-written equivalence [`ExecuteStatement::args`] uses.
    pub parameter_types: ThinVec<DataType<X>>,
    /// The statement bound to the name; any [`Statement`] parses (the
    /// [`ExplainStatement`] "grammar accepts any statement" contract).
    pub statement: Box<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `PREPARE <name> FROM {'<text>' | @<var>}` prepared-statement definition (gated by
/// [`UtilitySyntax::prepared_statements_from`](crate::dialect::UtilitySyntax)).
///
/// A *different shape on the same `PREPARE` keyword* from DuckDB/PostgreSQL's typed
/// [`PrepareStatement`]: MySQL binds the name to a statement *source* — an opaque string
/// literal or a user-variable reference read at prepare time (`sql_yacc.yy` `prepare_src`) —
/// **not** an inline-parsed [`Statement`], and takes neither the `AS` keyword nor a
/// parameter-type list. The source text is never re-parsed here (the placeholders `?` it
/// carries are a run-time protocol concern), so this node holds no expression or statement
/// children and is non-generic, like [`DeallocateStatement`]. The two `PREPARE` behaviours
/// never coexist in one preset (each arms at most one gate), the split mirroring the
/// [`DoStatement`]/[`DoExpressionsStatement`] `DO`-keyword split.
///
/// MySQL grammar-accepts every well-formed source shape but cannot *prepare* the outer
/// `PREPARE` itself over the binary protocol (`ER_UNSUPPORTED_PS` 1295) — a bind-time
/// verdict the parse layer does not model, exactly the [`PrepareStatement`]
/// "grammar accepts, bind restricts" contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PrepareFromStatement {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The statement source bound to the name; see [`PrepareSource`].
    pub source: PrepareSource,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The source of a MySQL [`PrepareFromStatement`] — `sql_yacc.yy` `prepare_src`, either an
/// inline string literal or a user-variable reference. A spanned enum node: each arm
/// round-trips its own spelling, so the two are recorded as written rather than folded to a
/// single string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PrepareSource {
    /// `PREPARE name FROM '<text>'` — the opaque statement source string (`TEXT_STRING_sys`),
    /// kept verbatim and never re-parsed here (like [`DoArg::Body`]). Its spelling — quote
    /// style and escapes — round-trips from the [`Literal`].
    Text {
        /// The statement-source [`Literal`] (a [`LiteralKind::String`](crate::ast::LiteralKind)).
        source: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PREPARE name FROM @<var>` — the source read from a user variable at prepare time
    /// (`'@' ident_or_text`, MySQL's `prepared_stmt_code_is_varref`). The variable's name is
    /// held without the `@` sigil, its quote style preserved for round-trip; `@@`-prefixed
    /// system variables are rejected by the parser (`ER_PARSE_ERROR` on mysql:8).
    Variable {
        /// The user-variable name (without the `@` sigil).
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `EXECUTE <name> [ ( <arg> [, ...] ) ]` prepared-statement invocation (DuckDB;
/// gated by [`UtilitySyntax::prepared_statements`](crate::dialect::UtilitySyntax)).
///
/// Runs a [`PrepareStatement`]-bound name, supplying its parameters positionally
/// (`EXECUTE v1(1)`). The arguments are full expressions, which makes this node generic
/// over the extension `X`. A bare `EXECUTE v1` (no argument list) leaves
/// [`args`](Self::args) empty; DuckDB rejects an empty `EXECUTE v1()` as a syntax error,
/// so an empty list unambiguously means "no parentheses written" and needs no separate
/// surface tag — one shape covers the absent-list form, and the parser refuses the empty
/// `()` that would otherwise collide with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExecuteStatement<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The positional argument expressions, in source order; empty for the bare
    /// `EXECUTE v1` form (no argument list). Never an empty written `()`, which DuckDB
    /// rejects.
    pub args: ThinVec<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `EXECUTE <name> [USING @<var> [, ...]]` prepared-statement invocation (gated by
/// [`UtilitySyntax::prepared_statements_from`](crate::dialect::UtilitySyntax)).
///
/// A *different argument surface on the same `EXECUTE` keyword* from DuckDB's parenthesized
/// positional-expression [`ExecuteStatement`]: MySQL supplies parameters through a `USING`
/// clause whose members are strictly *user-variable references* (`sql_yacc.yy`
/// `execute_var_list` → `execute_var_ident: '@' ident_or_text`), never arbitrary
/// expressions — `EXECUTE s USING 1` and `EXECUTE s USING @@sys` are both `ER_PARSE_ERROR`
/// on mysql:8, and there is no `EXECUTE s(...)` parenthesized form. So this node holds a
/// list of variable-name [`Ident`]s, not [`Expr`]s, and is non-generic. A bare `EXECUTE s`
/// (no `USING`) leaves [`using`](Self::using) empty; MySQL has no empty-`USING` spelling, so
/// an empty list unambiguously means the clause was absent.
///
/// Like [`PrepareFromStatement`], the statement grammar-parses but cannot be prepared over
/// the binary protocol (`ER_UNSUPPORTED_PS` 1295) — a bind verdict the parse layer leaves
/// untouched.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExecuteUsingStatement {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The `USING` user-variable references, in source order; each is a variable name held
    /// without its `@` sigil (quote style preserved for round-trip). Empty for the bare
    /// `EXECUTE name` form (no `USING` clause).
    pub using: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `CALL <name> [ ( [ <arg> [, ...] ] ) ]` routine invocation (DuckDB, MySQL; gated by
/// [`UtilitySyntax::call`](crate::dialect::UtilitySyntax)).
///
/// Invokes a table function or stored procedure as a top-level statement
/// (`CALL pragma_table_info('t')`, `CALL my_proc(1, 2)`). The arguments are full
/// expressions, which makes this node generic over the extension `X`.
///
/// The parenthesized argument list is *mandatory* for DuckDB — it rejects a bare
/// `CALL pragma_version` (syntax error) but accepts an empty `CALL pragma_version()` — and
/// *optional* for MySQL, whose grammar (`CALL_SYM sp_name opt_paren_expr_list`) admits a
/// bare `CALL my_proc` with no argument list at all (verified on mysql:8.4.10 — the bare
/// form resolves to ER_SP_DOES_NOT_EXIST, a grammar-positive binding reject). The
/// [`parenthesized`](Self::parenthesized) surface flag distinguishes the two written forms
/// so the source round-trips: `false` is the MySQL bare form (`CALL p`, always empty
/// [`args`](Self::args)), `true` is the parenthesized form (`CALL p()` / `CALL p(1, 2)`,
/// which is the only DuckDB shape). The bare form is gated by
/// [`UtilitySyntax::call_bare_name`](crate::dialect::UtilitySyntax).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CallStatement<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The argument expressions, in source order; may be empty (`CALL f()`) when
    /// [`parenthesized`](Self::parenthesized) is `true`, and is always empty for the bare
    /// MySQL form (`CALL f`).
    pub args: ThinVec<Expr<X>>,
    /// Whether a parenthesized argument list was written. `true` renders the `(...)` (empty
    /// or not); `false` is the MySQL bare `CALL name` form with no argument list, which
    /// renders just the name. Always `true` for DuckDB, whose parentheses are mandatory.
    pub parenthesized: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `{DEALLOCATE | DROP} [PREPARE] <name>` prepared-statement release (DuckDB, gated by
/// [`UtilitySyntax::prepared_statements`](crate::dialect::UtilitySyntax); MySQL, gated by
/// [`UtilitySyntax::prepared_statements_from`](crate::dialect::UtilitySyntax)).
///
/// Frees a [`PrepareStatement`]/[`PrepareFromStatement`]-bound name. Non-generic, like
/// [`DetachStatement`]: a name plus two surface flags carries no expressions or extension
/// nodes. Neither dialect has a `DEALLOCATE ALL` (DuckDB rejects it: "DEALLOCATE requires a
/// name"; MySQL's grammar takes a single `ident`), so the target is always a single
/// [`Ident`], never an all-marker.
///
/// The `PREPARE` keyword's role differs by dialect and is a parse-time constraint, not a
/// node field: DuckDB's `DEALLOCATE [PREPARE] name` makes it optional (round-tripped by
/// [`prepare_keyword`](Self::prepare_keyword)), while MySQL's `deallocate_or_drop PREPARE
/// ident` makes it *mandatory* (a bare `DEALLOCATE name` is `ER_PARSE_ERROR` on mysql:8),
/// so the MySQL parser always sets the flag. The leading-verb spelling
/// ([`keyword`](Self::keyword)) is MySQL's `deallocate_or_drop` synonym choice; DuckDB has
/// only the `DEALLOCATE` spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DeallocateStatement {
    /// Which leading verb spelled this release: MySQL's `deallocate_or_drop` accepts
    /// `DEALLOCATE PREPARE name` and the `DROP PREPARE name` synonym interchangeably, and
    /// the spelling round-trips. Always [`DeallocateKeyword::Deallocate`] for DuckDB, whose
    /// grammar has no `DROP PREPARE` form.
    pub keyword: DeallocateKeyword,
    /// Whether the `PREPARE` keyword was written (`DEALLOCATE PREPARE v1` vs `DEALLOCATE
    /// v1`); a round-trip surface tag. Optional for DuckDB (both forms accepted), always
    /// `true` for MySQL (the keyword is mandatory there).
    pub prepare_keyword: bool,
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The leading verb of a [`DeallocateStatement`] — MySQL's `deallocate_or_drop` synonym
/// choice. A round-trip surface tag: `DROP PREPARE name` and `DEALLOCATE PREPARE name` are
/// the same statement on mysql:8, differing only in this spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DeallocateKeyword {
    /// The `DEALLOCATE` spelling (DuckDB and MySQL).
    Deallocate,
    /// MySQL's `DROP` synonym (`DROP PREPARE name`).
    Drop,
}

/// A PostgreSQL `DO [LANGUAGE <lang>] '<body>'` anonymous code block (gated by
/// [`UtilitySyntax::do_statement`](crate::dialect::UtilitySyntax)).
///
/// Runs an inline procedural-language block without defining a routine. The body is an
/// opaque source string in the target language ([`LiteralKind::String`](crate::ast::LiteralKind)
/// — dollar-quoted in practice), never re-parsed here: like
/// [`FunctionOption::As`](crate::ast::FunctionOption) the body's grammar is the target
/// language, not SQL, so the PL text is not smuggled into the AST.
///
/// The shape mirrors [`CreateFunction`](crate::ast::CreateFunction)'s option cluster
/// rather than a fixed `{ language, body }` pair, because PostgreSQL's raw grammar is the
/// same free option list: `DO dostmt_opt_list`, where `dostmt_opt_list` is a *non-empty*
/// sequence of items each either an `Sconst` body or `LANGUAGE <word>`, in any order and
/// with no arity limit. The parser (`makeDefElem("as"/"language", …)`) accepts a repeated
/// or missing body and a repeated language — `DO LANGUAGE plpgsql` (language, no body),
/// `DO $$a$$ $$b$$` (two bodies), and `DO 'x' LANGUAGE a LANGUAGE b` (two languages) all
/// parse — and defers the "exactly one body, at most one language" check to execution
/// (`ExecuteDoStmt`), exactly as the [`PrepareStatement`] "grammar accepts any statement,
/// bind restricts the kind" contract defers its own semantic check. Modelling the list
/// faithfully is what keeps raw-parse accept/reject parity with libpg_query; collapsing it
/// to a single body/language pair would over-reject those three forms. The list is
/// non-empty (a bare `DO` is a syntax error), which the parser enforces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DoStatement {
    /// The `dostmt_opt_list` items in source order; always non-empty.
    pub args: ThinVec<DoArg>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One item of a [`DoStatement`]'s `dostmt_opt_list` — a body string or a `LANGUAGE`
/// clause. The two forms and their source order round-trip, so the parser records each as
/// written rather than folding them into a canonical `{ language, body }` pair.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DoArg {
    /// An inline-body `Sconst` item — PostgreSQL's `makeDefElem("as", …)`. The body is
    /// the opaque source [`Literal`] (kept, not re-parsed, like
    /// [`FunctionOption::As`](crate::ast::FunctionOption)); there is no `AS` keyword in the
    /// `DO` syntax, so the variant is named for the block body it carries.
    Body {
        /// Statement or query body governed by this node.
        body: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `LANGUAGE <name>` item — PostgreSQL's `makeDefElem("language", …)`. The name is a
    /// `NonReservedWord_or_Sconst` ([`LanguageName`]): a bare word or a string constant, as
    /// in the `CREATE FUNCTION` `LANGUAGE` clause. A reserved word (a bit/hex/national string
    /// constant, since those are not `Sconst`) is the syntax error PostgreSQL reports.
    Language {
        /// Name referenced by this syntax.
        name: LanguageName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A routine `LANGUAGE <name>` operand (PostgreSQL's `NonReservedWord_or_Sconst`): a bare
/// non-reserved word or a string constant. Shared by the two positions that spell it the same
/// way — the [`DoStatement`] `DO … LANGUAGE <name>` argument and the `CREATE FUNCTION`
/// [`FunctionOption::Language`](crate::ast::FunctionOption) clause. PostgreSQL folds both
/// spellings to the same language name internally, but the surface form is kept distinct so it
/// round-trips — the same `NonReservedWord_or_Sconst` shape as
/// [`ExtensionVersion`](crate::ast::ExtensionVersion).
///
/// The string arm admits only an `Sconst` (a plain, `E'...'`, `U&'...'`, or dollar-quoted
/// constant); a bit-string (`b'...'`/`x'...'`) or national (`N'...'`) constant is not an
/// `Sconst`, so — like the code-block body — it is the syntax error PostgreSQL reports. The
/// string spelling is a PostgreSQL surface: MySQL's routine `LANGUAGE` admits only the bare
/// word `SQL` (`LANGUAGE 'SQL'` is `ER_PARSE_ERROR` on mysql:8), so the parser gates the string
/// arm on [`IndexAlterSyntax::routine_language_string`](crate::dialect::IndexAlterSyntax::routine_language_string).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LanguageName {
    /// A bare non-reserved word (`LANGUAGE plpgsql`).
    Word {
        /// Identifier-form language name.
        word: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A string constant (`LANGUAGE 'plpgsql'`).
    String {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `DO <expr> [, <expr> ...]` evaluate-and-discard statement (gated by
/// [`UtilitySyntax::do_expression_list`](crate::dialect::UtilitySyntax)).
///
/// A *different behaviour on the same `DO` keyword* from PostgreSQL's anonymous code block
/// ([`DoStatement`]): MySQL's `DO` evaluates a list of expressions purely for their side
/// effects and throws the results away (`DO SLEEP(1)`, `DO @x := 1`), whereas PostgreSQL's
/// `DO` runs an opaque procedural-language body string. The two never coexist in one dialect
/// (each preset arms exactly one gate), the split mirroring the transaction-`BEGIN`
/// vs compound-block-`BEGIN` dialect-gated arms.
///
/// The grammar is literally `DO select_item_list` (mysql `sql_yacc.yy` `do_stmt`), so the
/// items are [`SelectItem`]s, not bare [`Expr`]s: MySQL grammar-accepts a select alias
/// (`DO 1 AS x` PREPAREs) and a wildcard (`DO *`, `DO t.*`) here exactly as in a projection.
/// Reusing the projection-item node keeps raw-parse acceptance aligned with the engine; the
/// wildcard forms bind-reject (`DO *` is `ER_NO_TABLES_USED`), but that is a resolver verdict
/// the parse layer does not model, not a syntax reject. The list is non-empty (a bare `DO` is
/// `ER_PARSE_ERROR`), which the parser enforces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DoExpressionsStatement<X: Extension = NoExt> {
    /// The evaluated expression list in source order; always non-empty.
    pub items: ThinVec<SelectItem<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `LOCK {TABLES | TABLE} <tbl> [[AS] <alias>] <lock-kind> [, ...]` explicit
/// table-locking statement (gated by
/// [`UtilitySyntax::lock_tables`](crate::dialect::UtilitySyntax)).
///
/// This is *one of two distinct behaviours a dialect can attach to the leading `LOCK`
/// keyword*, the split modelled the [`DoExpressionsStatement`] / [`DoStatement`] way (a
/// behaviour-named gate per reading, never both armed in one preset). MySQL's `LOCK TABLES`
/// names a per-table lock **kind** (`READ`, `READ LOCAL`, `WRITE`) on each table in a list —
/// the shape captured here. PostgreSQL's `LOCK TABLE`, by contrast, takes a single
/// statement-level lock **mode** clause (`IN ACCESS SHARE MODE`, `NOWAIT`, …) over a relation
/// list; that reading is not implemented, but when it is it takes its own node behind its own
/// `LOCK`-keyword gate, so the two never collide. The `MySql`/`Lenient` presets arm
/// [`lock_tables`](crate::dialect::UtilitySyntax::lock_tables); every other preset leaves the
/// leading `LOCK` keyword undispatched (an unknown statement).
///
/// The grammar is `LOCK table_or_tables table_lock_list` (mysql `sql_yacc.yy` `lock` /
/// `table_lock_list`), so the `TABLES` (plural) and `TABLE` (singular) spellings are
/// interchangeable and preserved on [`plural`](Self::plural) to round-trip. Each
/// [`TableLock`] carries a mandatory lock kind — a bare `LOCK TABLES t1` with no kind is
/// `ER_PARSE_ERROR` (engine-measured on mysql:8.4.10), which the parser enforces. The list is
/// non-empty. Non-generic, like [`UseStatement`]: a table lock carries no expressions or
/// extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LockTablesStatement {
    /// The keyword spelling: `true` for `LOCK TABLES` (plural), `false` for `LOCK TABLE`
    /// (singular). Both are grammar-equal; preserved so the exact spelling round-trips.
    pub plural: bool,
    /// The locked tables in source order; always non-empty.
    pub tables: ThinVec<TableLock>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One entry of a MySQL [`LockTablesStatement`] table list: a table name, an optional alias,
/// and its mandatory [`TableLockKind`] (`table_ident opt_table_alias lock_option` in mysql
/// `sql_yacc.yy` `table_lock`). The alias is a single identifier (`opt_as ident`); whether it
/// was written with the `AS` keyword is not modelled because MySQL discards it, so the
/// renderer emits the canonical `AS`-less spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct TableLock {
    /// The table being locked.
    pub name: ObjectName,
    /// The optional table alias (`[AS] <alias>`).
    pub alias: Option<Ident>,
    /// The lock kind acquired on the table (mandatory).
    pub kind: TableLockKind,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The lock kind a MySQL [`TableLock`] acquires (`lock_option` in mysql `sql_yacc.yy`). Only
/// these three spellings are grammar-valid on mysql:8.4.10 — the historical (pre-8.0)
/// `LOW_PRIORITY WRITE` modifier is `ER_PARSE_ERROR` there (engine-measured), so it is
/// deliberately not a variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableLockKind {
    /// `READ` — a shared read lock.
    Read,
    /// `READ LOCAL` — a shared read lock permitting concurrent non-conflicting inserts.
    ReadLocal,
    /// `WRITE` — an exclusive write lock.
    Write,
}

/// A MySQL `UNLOCK {TABLES | TABLE}` statement releasing all table locks held by the session
/// (gated by [`UtilitySyntax::lock_tables`](crate::dialect::UtilitySyntax), the release
/// counterpart of [`LockTablesStatement`]).
///
/// Carries no table list — MySQL's `UNLOCK` releases everything the session holds — only the
/// `TABLES`/`TABLE` spelling, preserved on [`plural`](Self::plural) to round-trip
/// (`unlock: UNLOCK_SYM table_or_tables` in mysql `sql_yacc.yy`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UnlockTablesStatement {
    /// The keyword spelling: `true` for `UNLOCK TABLES` (plural), `false` for `UNLOCK TABLE`
    /// (singular). Both are grammar-equal; preserved so the exact spelling round-trips.
    pub plural: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL instance-wide backup lock statement — `LOCK INSTANCE FOR BACKUP` (acquire) or
/// `UNLOCK INSTANCE` (release) — gated by
/// [`UtilitySyntax::lock_instance`](crate::dialect::UtilitySyntax).
///
/// One node for the pair, distinguished by [`acquire`](Self::acquire), because neither side
/// carries any other payload (mysql `sql_yacc.yy` `lock`/`unlock`: both alternatives build a
/// bare `Sql_cmd_{lock,unlock}_instance`): a two-struct split would add an empty node for the
/// release half. Distinct from [`LockTablesStatement`]/[`UnlockTablesStatement`], which take a
/// per-table lock-kind list — the instance lock is a single server-wide DDL-blocking lock
/// with fixed spelling on both sides, so there is nothing else to model. Both spellings are
/// grammar-positive on mysql:8.4.10 (`ER_UNSUPPORTED_PS` under the PREPARE oracle — parsed,
/// then declined by the PREPARE protocol, never `ER_PARSE_ERROR`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct InstanceLockStatement {
    /// `true` for `LOCK INSTANCE FOR BACKUP` (acquire), `false` for `UNLOCK INSTANCE`
    /// (release).
    pub acquire: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `LOAD {DATA | XML} … INFILE … INTO TABLE …` bulk-import statement (gated by
/// [`UtilitySyntax::load_data`](crate::dialect::UtilitySyntax)).
///
/// One node covers both the `DATA` (delimited text) and `XML` readings, distinguished by
/// [`format`](Self::format): the mysql `sql_yacc.yy` `load_stmt` rule is a *single*
/// `data_or_xml` production whose whole clause train is grammatically shared, so at the parse
/// layer the two forms differ only in the `DATA`/`XML` keyword. The clauses MySQL restricts to
/// one reading (`FIELDS`/`LINES` are meaningless under `XML`, `ROWS IDENTIFIED BY` under
/// `DATA`) are *semantic* restrictions the server enforces only after it has parsed the whole
/// statement and resolved the table — every clause parses under either format (engine-measured
/// on mysql:8.4.10: a `LOAD XML … FIELDS TERMINATED BY ','` reaches `ER_NO_SUCH_TABLE` 1146,
/// not `ER_PARSE_ERROR` 1064), so this node does not gate them by format and the parse layer
/// leaves that binding verdict untouched (the [`PrepareStatement`] "grammar accepts, bind
/// restricts" contract).
///
/// The clause train is strictly *order-sensitive* (engine-measured: any out-of-order clause is
/// `ER_PARSE_ERROR` 1064), so the node's field order is the grammar's canonical order and the
/// renderer emits that order. The optional clauses are `None`/empty when absent. Generic over
/// the extension `X` because the [`set`](Self::set) assignments carry full expressions.
///
/// Scope boundary: the MySQL 8.4 secondary-engine bulk-load extension clauses on the same
/// `load_stmt` rule — the `FROM` keyword, `URL`/`S3` source types, `COUNT n`,
/// `IN PRIMARY KEY ORDER`, `COMPRESSION`, `PARALLEL`, `MEMORY`, and `ALGORITHM = BULK` — are a
/// distinct cloud-bulk-load feature family and are intentionally not modelled here (they take
/// their own follow-up; see the ticket close note). This node is the classic documented
/// `LOAD DATA` / `LOAD XML` surface.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadDataStatement<X: Extension = NoExt> {
    /// Whether the source is delimited text (`LOAD DATA`) or XML (`LOAD XML`); see
    /// [`LoadDataFormat`].
    pub format: LoadDataFormat,
    /// The optional table-lock concurrency modifier (`LOW_PRIORITY` / `CONCURRENT`); see
    /// [`LoadDataConcurrency`]. `None` for the default lock (`load_data_lock` %empty).
    pub concurrency: Option<LoadDataConcurrency>,
    /// `true` when the `LOCAL` keyword was written (the file is read from the client rather
    /// than the server host).
    pub local: bool,
    /// The `INFILE '<path>'` source-file string literal (`TEXT_STRING_filesystem`); only the
    /// `INFILE` source type is modelled (see the type-level scope note).
    pub file: Literal,
    /// The optional duplicate-key handling (`REPLACE` / `IGNORE`); see [`LoadDataDuplicate`].
    /// `None` for the default (error on duplicate).
    pub on_duplicate: Option<LoadDataDuplicate>,
    /// The `INTO TABLE <name>` destination table.
    pub table: ObjectName,
    /// The `PARTITION (<name> [, ...])` partition list; empty when the clause is absent. The
    /// list is never a written empty `()` — MySQL requires at least one partition name.
    pub partitions: ThinVec<Ident>,
    /// The `CHARACTER SET <name>` charset override (`opt_load_data_charset`); `None` when
    /// absent. Held as a single [`Ident`] (a charset name, not a dotted [`ObjectName`]).
    pub charset: Option<Ident>,
    /// The `ROWS IDENTIFIED BY '<tag>'` row element tag (`opt_xml_rows_identified_by`); `None`
    /// when absent. Grammar-shared by both formats (see the type-level note) though only
    /// meaningful under `XML`.
    pub rows_identified_by: Option<Literal>,
    /// The `{FIELDS | COLUMNS} …` field-format clause; `None` when absent. See
    /// [`LoadDataFields`].
    pub fields: Option<LoadDataFields>,
    /// The `LINES …` line-format clause; `None` when absent. See [`LoadDataLines`].
    pub lines: Option<LoadDataLines>,
    /// The `IGNORE <n> {LINES | ROWS}` header-skip clause; `None` when absent. See
    /// [`LoadDataIgnoreRows`].
    pub ignore_rows: Option<LoadDataIgnoreRows>,
    /// The parenthesized `(col_or_var [, ...])` target list; empty when absent (or a written
    /// empty `()`, which MySQL folds to absent — `'(' ')'` yields the same nullptr as an
    /// omitted list). See [`LoadDataFieldOrVar`].
    pub columns: ThinVec<LoadDataFieldOrVar>,
    /// The `SET col = {expr | DEFAULT} [, ...]` post-load assignments; empty when absent.
    /// Reuses [`UpdateAssignment`] exactly as `INSERT … SET` does (mysql `load_data_set_elem`
    /// is `simple_ident_nospvar equal expr_or_default`, the single-column-assignment shape); a
    /// tuple assignment is not grammar-valid here, and the fitted `MySql` preset never emits
    /// one (its `multi_column_assignment` gate is off).
    pub set: ThinVec<UpdateAssignment<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Whether a [`LoadDataStatement`] reads delimited text (`LOAD DATA`) or XML (`LOAD XML`) —
/// the mysql `sql_yacc.yy` `data_or_xml` production.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadDataFormat {
    /// `LOAD DATA` — a delimited-text file (`FILETYPE_CSV`).
    Data,
    /// `LOAD XML` — an XML file (`FILETYPE_XML`).
    Xml,
}

/// The optional table-lock concurrency modifier of a [`LoadDataStatement`] (mysql
/// `sql_yacc.yy` `load_data_lock`). The default (no keyword) is a plain write lock and is
/// modelled as `None` on the statement, so this enum carries only the two written spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadDataConcurrency {
    /// `LOW_PRIORITY` — defer the load until no clients are reading the table.
    LowPriority,
    /// `CONCURRENT` — allow other clients to read the table during the load.
    Concurrent,
}

/// The optional duplicate-key handling of a [`LoadDataStatement`] (mysql `sql_yacc.yy`
/// `opt_duplicate` / `duplicate`). The default (no keyword) errors on a duplicate and is
/// modelled as `None`, so this enum carries only the two written spellings. `REPLACE` and
/// `IGNORE` are mutually exclusive — writing both is `ER_PARSE_ERROR` (engine-measured).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadDataDuplicate {
    /// `REPLACE` — replace existing rows that collide on a unique key.
    Replace,
    /// `IGNORE` — skip input rows that collide on a unique key.
    Ignore,
}

/// The `{FIELDS | COLUMNS} …` field-format clause of a [`LoadDataStatement`] (mysql
/// `sql_yacc.yy` `opt_field_term` / `field_term_list`).
///
/// The `FIELDS` and `COLUMNS` keywords are interchangeable synonyms; the written spelling
/// rides [`spelling`](Self::spelling) so it round-trips. At least one of the three sub-clauses
/// is present — a bare `FIELDS` with no sub-clause is `ER_PARSE_ERROR` (engine-measured), which
/// the parser enforces. Each sub-clause may appear in any order and the parser folds it onto
/// the matching field (a repeat is last-wins, mirroring the grammar's `merge_field_separators`);
/// the renderer emits the canonical `TERMINATED` / `[OPTIONALLY] ENCLOSED` / `ESCAPED` order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadDataFields {
    /// The interchangeable keyword spelling (`FIELDS` vs `COLUMNS`); see
    /// [`LoadFieldsSpelling`].
    pub spelling: LoadFieldsSpelling,
    /// `TERMINATED BY '<string>'` — the field separator; `None` when absent.
    pub terminated_by: Option<Literal>,
    /// `[OPTIONALLY] ENCLOSED BY '<char>'` — the field quoting; `None` when absent. See
    /// [`LoadDataEnclosed`] (the `OPTIONALLY` modifier rides it, so an `OPTIONALLY` without an
    /// `ENCLOSED BY` is unrepresentable).
    pub enclosed_by: Option<LoadDataEnclosed>,
    /// `ESCAPED BY '<char>'` — the escape character; `None` when absent.
    pub escaped_by: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The interchangeable keyword spelling of a [`LoadDataFields`] clause (mysql `sql_yacc.yy`:
/// `opt_field_term` spells the same clause `COLUMNS`, while the documented form is `FIELDS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadFieldsSpelling {
    /// The `FIELDS` spelling (the documented keyword).
    Fields,
    /// The `COLUMNS` spelling (the grammar synonym).
    Columns,
}

/// The `[OPTIONALLY] ENCLOSED BY '<char>'` sub-clause of a [`LoadDataFields`] clause (mysql
/// `sql_yacc.yy` `field_term`: the `ENCLOSED BY` and `OPTIONALLY ENCLOSED BY` alternatives).
/// Bundling the `OPTIONALLY` modifier with its value makes an `OPTIONALLY` with no `ENCLOSED
/// BY` unrepresentable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadDataEnclosed {
    /// `true` when the `OPTIONALLY` modifier was written (`OPTIONALLY ENCLOSED BY`).
    pub optionally: bool,
    /// The enclosing-character string literal.
    pub value: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `LINES …` line-format clause of a [`LoadDataStatement`] (mysql `sql_yacc.yy`
/// `opt_line_term` / `line_term_list`).
///
/// At least one of the two sub-clauses is present — a bare `LINES` with no sub-clause is
/// `ER_PARSE_ERROR` (engine-measured), which the parser enforces. Each sub-clause may appear in
/// any order (a repeat is last-wins, mirroring `merge_line_separators`); the renderer emits the
/// canonical `STARTING` / `TERMINATED` order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadDataLines {
    /// `STARTING BY '<string>'` — the common line prefix to strip; `None` when absent.
    pub starting_by: Option<Literal>,
    /// `TERMINATED BY '<string>'` — the line separator; `None` when absent.
    pub terminated_by: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `IGNORE <n> {LINES | ROWS}` header-skip clause of a [`LoadDataStatement`] (mysql
/// `sql_yacc.yy` `opt_ignore_lines` / `lines_or_rows`). The `LINES` and `ROWS` keywords are
/// interchangeable; the written spelling rides [`unit`](Self::unit) so it round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoadDataIgnoreRows {
    /// The number of leading rows to skip (a `NUM` token), kept as an unsigned-integer
    /// [`Literal`] so the exact spelling round-trips.
    pub count: Literal,
    /// The interchangeable unit keyword (`LINES` vs `ROWS`); see [`LoadDataIgnoreUnit`].
    pub unit: LoadDataIgnoreUnit,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The interchangeable unit keyword of a [`LoadDataIgnoreRows`] clause (mysql `sql_yacc.yy`
/// `lines_or_rows`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadDataIgnoreUnit {
    /// The `LINES` spelling.
    Lines,
    /// The `ROWS` spelling.
    Rows,
}

/// One entry of a [`LoadDataStatement`] target list (mysql `sql_yacc.yy` `field_or_var`): a
/// destination column name, or a user variable (`@name`) that captures the raw field value for
/// use in the `SET` clause. A spanned enum so each spelling round-trips its own form and a
/// column is never confused with a variable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LoadDataFieldOrVar {
    /// A destination column (`simple_ident_nospvar`).
    Column {
        /// The column name.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A user variable (`@name`) capturing the field's raw value; the name is held without the
    /// `@` sigil, its quote style preserved for round-trip.
    Variable {
        /// The user-variable name (without the `@` sigil).
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A typed `SHOW TABLES` utility statement (MySQL/DuckDB; gated by
/// [`ShowSyntax::show_tables`](crate::dialect::UtilitySyntax)).
///
/// Distinct from the generic session `SHOW <var>`
/// ([`SessionStatement::Show`](crate::ast::SessionStatement)), which reads one
/// configuration parameter, and from the parenthesized `(SHOW <name>)` table source
/// ([`TableFactor::ShowRef`](crate::ast::TableFactor)), which only appears inside a
/// `FROM (…)` and produces a relation. `SHOW TABLES` is a top-level catalogue-listing
/// *statement*, so it is its own node — the MECE split spelled out on
/// [`show_tables`](crate::dialect::ShowSyntax::show_tables).
///
/// This is the opener of the typed-`SHOW` family (`SHOW COLUMNS`, `SHOW CREATE TABLE`,
/// `SHOW FUNCTIONS` join it): the listed thing rides the [`ShowTarget`] axis. That axis
/// is an enum with *per-variant* fields — the [`SessionStatement`](crate::ast::SessionStatement)
/// / [`ShowRefTarget`](crate::ast::ShowRefTarget) idiom, **not** a flat `kind` tag like
/// [`ApplyKind`](crate::ast::ApplyKind): the `SHOW` subforms carry genuinely different
/// payloads (`TABLES` takes a `FROM`/filter, `CREATE TABLE` a table name, `FUNCTIONS` a
/// scope keyword, schema qualifier, and name-or-regex filter), so a shared tag would
/// force an option-soup lowest common denominator. A sibling is then a new [`ShowTarget`]
/// variant plus its own gate, leaving the existing variants untouched.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ShowStatement<X: Extension = NoExt> {
    /// Object targeted by this syntax.
    pub target: ShowTarget<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// What a [`ShowStatement`] lists — the extensible axis of the typed-`SHOW` family.
///
/// The cross-dialect subforms [`Tables`](Self::Tables), [`Columns`](Self::Columns),
/// [`Functions`](Self::Functions), and [`RoutineStatus`](Self::RoutineStatus) each carry a
/// genuinely distinct payload (see [`ShowStatement`] for why this is a per-variant enum
/// rather than a flat `kind` tag). The MySQL server-administration / catalogue family folds
/// its ~40 near-identical sub-commands onto data axes instead: [`Listing`](Self::Listing)
/// and [`Bare`](Self::Bare) carry a sub-command discriminator, [`Create`](Self::Create) a
/// [`ShowCreateKind`], and [`Index`](Self::Index), [`Engine`](Self::Engine),
/// [`ReplicaStatus`](Self::ReplicaStatus), [`Diagnostics`](Self::Diagnostics), and
/// [`RoutineCode`](Self::RoutineCode) the handful with their own operands. The account /
/// diagnostics remainder rides its own operand-bearing variants: [`Grants`](Self::Grants) and
/// [`CreateUser`](Self::CreateUser) (the shared [`AccountName`] `user` grammar),
/// [`Profile`](Self::Profile) (a resource-type list plus `FOR QUERY` / `LIMIT`), and
/// [`LogEvents`](Self::LogEvents) (the `BINLOG` / `RELAYLOG` event dump).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowTarget<X: Extension = NoExt> {
    /// `SHOW [EXTENDED] [FULL] [ALL] TABLES [{FROM | IN} <db>] [LIKE '<pat>' | WHERE <expr>]`.
    ///
    /// The leading modifiers are dialect-split unions: `EXTENDED`/`FULL` are MySQL's
    /// (`SHOW FULL TABLES` adds a table-type column), `ALL` is DuckDB's (`SHOW ALL TABLES`
    /// lists every attached database's tables). Each is an independent optional keyword,
    /// so they ride separate bools rather than one axis; no shipped dialect mixes them.
    /// The `{FROM | IN} <db>` qualifier and the `LIKE`/`WHERE` filter are MySQL's (DuckDB
    /// accepts only `FROM <schema>`); the single [`show_tables`](crate::dialect::ShowSyntax::show_tables)
    /// gate accepts the union permissively, the DESCRIBE/PRAGMA single-flag-utility precedent.
    Tables {
        /// MySQL `EXTENDED` — also list hidden tables left by a failed `ALTER`.
        extended: bool,
        /// MySQL `FULL` — add the `Table_type` column.
        full: bool,
        /// DuckDB `ALL` — list tables across every attached database.
        all: bool,
        /// The optional `{FROM | IN} <db>` schema qualifier.
        from: Option<ShowFrom>,
        /// The optional trailing `LIKE '<pat>'` / `WHERE <expr>` narrowing.
        filter: Option<ShowFilter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW [EXTENDED] [FULL] {COLUMNS | FIELDS} {FROM | IN} <tbl> [{FROM | IN} <db>]
    /// [LIKE '<pat>' | WHERE <expr>]` (MySQL; gated by
    /// [`show_columns`](crate::dialect::ShowSyntax::show_columns)).
    ///
    /// Unlike [`Tables`](Self::Tables), the `{FROM | IN}` qualifier is *mandatory* — it
    /// names the table whose columns are listed — and the grammar has a *second*, optional
    /// `{FROM | IN} <db>` naming the database (equivalent to writing `db.tbl` in the
    /// [`table`](Self::Columns::table) slot). There is no `ALL` modifier here; DuckDB has
    /// no `SHOW COLUMNS` grammar at all (engine-probed reject on 1.5.4), so this is a
    /// MySQL-only subform. `FIELDS` is an exact synonym of `COLUMNS`; the written spelling
    /// rides the [`ShowColumnsSpelling`] surface tag so the statement round-trips.
    Columns {
        /// MySQL `EXTENDED` — also list hidden columns MySQL maintains internally.
        extended: bool,
        /// MySQL `FULL` — add the collation, privileges, and comment columns.
        full: bool,
        /// Which keyword named the listing: `COLUMNS` or its `FIELDS` synonym.
        spelling: ShowColumnsSpelling,
        /// The mandatory `{FROM | IN} <tbl>` qualifier naming the target table.
        table: ShowFrom,
        /// The optional second `{FROM | IN} <db>` qualifier naming the database.
        database: Option<ShowFrom>,
        /// The optional trailing `LIKE '<pat>'` / `WHERE <expr>` narrowing.
        filter: Option<ShowFilter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW CREATE {TABLE | VIEW | DATABASE [IF NOT EXISTS] | EVENT | PROCEDURE | FUNCTION
    /// | TRIGGER} <name>` — the DDL that would recreate the named object (MySQL). The
    /// `TABLE` spelling is gated by
    /// [`show_create_table`](crate::dialect::ShowSyntax::show_create_table); every other
    /// object kind is gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin).
    ///
    /// The object kind rides the [`kind`](Self::Create::kind) axis as DATA rather than
    /// forcing one bespoke variant per keyword — the `SHOW CREATE …` subforms are
    /// structurally identical (two fixed keywords plus one schema-qualifiable name), so a
    /// per-keyword variant would be pure duplication. `IF NOT EXISTS` is a `DATABASE`-only
    /// guard ([`if_not_exists`](Self::Create::if_not_exists), always `false` for the other
    /// kinds). `SHOW CREATE USER` is deliberately excluded: its operand is a MySQL user
    /// specification (`'user'@'host'`), not an [`ObjectName`], so it rides its own
    /// [`CreateUser`](Self::CreateUser) variant over the shared [`AccountName`] grammar.
    Create {
        /// Which object kind followed `CREATE`; see [`ShowCreateKind`].
        kind: ShowCreateKind,
        /// The target object name (schema-qualifiable).
        name: ObjectName,
        /// The `DATABASE`-only `IF NOT EXISTS` guard; `false` for every other kind.
        if_not_exists: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW [{USER | SYSTEM | ALL}] FUNCTIONS [{FROM | IN} <schema>]
    /// [[LIKE] {<function_name> | '<regex>'}]` (Spark / Databricks; gated by
    /// [`show_functions`](crate::dialect::ShowSyntax::show_functions)).
    ///
    /// The only shipped engine with a bare `SHOW FUNCTIONS` listing is Spark/Databricks,
    /// and it carries the full grammar: an optional [`kind`](Self::Functions::kind) scope
    /// keyword (`USER`/`SYSTEM`/`ALL`) *before* `FUNCTIONS`, an optional `{FROM | IN}`
    /// schema qualifier, and an optional trailing name-or-regex narrowing whose `LIKE`
    /// keyword is itself optional. MySQL's `SHOW FUNCTION STATUS` is a *different*
    /// statement (a routine catalogue over `mysql.proc`, not a bare `SHOW FUNCTIONS`) —
    /// modelled by its own [`RoutineStatus`](Self::RoutineStatus) variant; DuckDB has no
    /// `SHOW FUNCTIONS` grammar (`SHOW <name>` there is a `DESCRIBE` alias — `SHOW
    /// functions` describes a table named `functions`, engine-probed on 1.5.4).
    Functions {
        /// The optional `USER` / `SYSTEM` / `ALL` scope keyword written before `FUNCTIONS`.
        kind: Option<ShowFunctionsScope>,
        /// The optional `{FROM | IN} <schema>` schema qualifier.
        from: Option<ShowFrom>,
        /// The optional trailing `[LIKE] {<function_name> | '<regex>'}` narrowing.
        filter: Option<ShowFunctionsFilter>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW {FUNCTION | PROCEDURE} STATUS [LIKE '<pat>' | WHERE <expr>]` — the stored-routine
    /// catalogue listing (MySQL; gated by
    /// [`show_routine_status`](crate::dialect::ShowSyntax::show_routine_status)).
    ///
    /// A *different* statement from the Spark/Databricks [`Functions`](Self::Functions)
    /// listing: different keywords (the singular `FUNCTION`/`PROCEDURE` plus a mandatory
    /// `STATUS`, not a bare plural `FUNCTIONS`) and a different payload (a row per stored
    /// routine from `information_schema.routines`, not the bare function names). It carries
    /// no scope keyword and no `{FROM | IN}` qualifier — `SHOW FUNCTION STATUS FROM db` is
    /// `ER_PARSE_ERROR` on mysql:8 (engine-probed) — only the optional `LIKE`/`WHERE`
    /// narrowing, which reuses the shared [`ShowFilter`] (MySQL's mutually-exclusive `LIKE
    /// '<pat>' | WHERE <expr>`, exactly as `SHOW TABLES`/`SHOW COLUMNS`). The `FUNCTION`
    /// vs `PROCEDURE` object keyword rides the [`ShowRoutineKind`] surface tag so the
    /// statement round-trips.
    RoutineStatus {
        /// Which stored-routine kind was named: `FUNCTION` or `PROCEDURE`.
        kind: ShowRoutineKind,
        /// The optional trailing `LIKE '<pat>'` / `WHERE <expr>` narrowing.
        filter: Option<ShowFilter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL catalogue/server listing that admits the shared `[LIKE '<pat>' | WHERE
    /// <expr>]` tail — `SHOW DATABASES`, `SHOW EVENTS`, `SHOW [GLOBAL | SESSION] STATUS`,
    /// `SHOW TRIGGERS`, and their siblings (gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The sub-command rides the [`kind`](Self::Listing::kind) axis as DATA: every member
    /// shares the trailing [`ShowFilter`], the members that accept an optional `{FROM | IN}
    /// <db>` qualifier share [`from`](Self::Listing::from), and each sub-command's own
    /// small scalar payload (a `GLOBAL`/`SESSION` scope, a `FULL` flag, a spelling bit)
    /// rides the [`ShowListing`] discriminator, so the many near-identical listings collapse
    /// into one node instead of one bespoke variant each. The single [`from`](Self::Listing::from)
    /// admits the qualifier permissively (`SHOW DATABASES`/`SHOW STATUS` take none — the
    /// parser leaves those `None`), the DESCRIBE/PRAGMA single-field precedent.
    Listing {
        /// Which listing was named, plus its sub-command-specific scalar payload; see
        /// [`ShowListing`].
        kind: ShowListing,
        /// The optional `{FROM | IN} <db>` schema qualifier (only `EVENTS`, `TABLE STATUS`,
        /// `OPEN TABLES`, and `TRIGGERS` accept one; `None` for every other member).
        from: Option<ShowFrom>,
        /// The optional trailing `LIKE '<pat>'` / `WHERE <expr>` narrowing.
        filter: Option<ShowFilter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `SHOW` sub-command that takes no filter and no name operand — `SHOW PLUGINS`,
    /// `SHOW [STORAGE] ENGINES`, `SHOW PRIVILEGES`, `SHOW [FULL] PROCESSLIST`, `SHOW BINARY
    /// LOGS`, and their siblings (gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The sub-command rides the [`kind`](Self::Bare::kind) axis as DATA; the only payloads
    /// any member carries are single leading-keyword flags (`STORAGE` before `ENGINES`,
    /// `FULL` before `PROCESSLIST`), folded into the [`ShowBare`] variant.
    Bare {
        /// Which bare sub-command was named; see [`ShowBare`].
        kind: ShowBare,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW GRANTS [FOR <user> [USING <role> [, …]]]` — the privilege listing for an account
    /// (MySQL; gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// Bare `SHOW GRANTS` (the session's own privileges) leaves [`user`](Self::Grants::user)
    /// `None` and [`using_roles`](Self::Grants::using_roles) empty. `FOR <user>` names the
    /// account whose grants are shown; the optional `USING <role list>` (only valid after
    /// `FOR` — `SHOW GRANTS USING …` is `ER_PARSE_ERROR` on mysql:8.4.10) restricts the
    /// listing to the named active roles. Both the user and each role are the shared
    /// [`AccountName`] `user` grammar (a named `'u'@'host'` account or `CURRENT_USER [()]`),
    /// so this reuses the account-reference axis the DCL landings build. `using_roles` is
    /// non-empty only when `USING` was written, which the grammar allows only when
    /// [`user`](Self::Grants::user) is `Some`.
    Grants {
        /// The `FOR <user>` account, or `None` for bare `SHOW GRANTS`.
        user: Option<AccountName>,
        /// The `USING <role> [, …]` active-role restriction; empty when no `USING` was
        /// written (which the grammar permits only when [`user`](Self::Grants::user) is set).
        using_roles: ThinVec<AccountName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW CREATE USER <user>` — the `CREATE USER` statement that would recreate an account
    /// (MySQL; gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// A sibling of [`Create`](Self::Create) held apart because its operand is the shared
    /// [`AccountName`] `user` specification (`'u'@'host'` or `CURRENT_USER [()]`), not an
    /// [`ObjectName`] — the exact reason the SHOW-family landing deferred it to the user-spec
    /// grammar (see [`ShowCreateKind`], where `USER` is absent).
    CreateUser {
        /// The account to recreate (the shared `user` grammar).
        user: AccountName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW PROFILE [<type> [, …]] [FOR QUERY <n>] [LIMIT …]` — the per-statement resource
    /// profile (MySQL; gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The optional [`types`](Self::Profile::types) list selects which resource columns to
    /// report (MySQL's `profile_defs`; empty when none written — the server defaults to a
    /// standard set). `FOR QUERY <n>` ([`query`](Self::Profile::query)) profiles a specific
    /// entry from `SHOW PROFILES` rather than the last statement, and the shared
    /// [`ShowLimit`] tail narrows the row set. The three clauses are order-fixed:
    /// `SHOW PROFILE ALL FOR QUERY 1` parses but `SHOW PROFILE FOR QUERY 1 ALL` and
    /// `SHOW PROFILE LIMIT 5 FOR QUERY 1` are both `ER_PARSE_ERROR` on mysql:8.4.10.
    /// Distinct from the bare [`ShowBare::Profiles`] catalogue listing (`SHOW PROFILES`).
    Profile {
        /// The `profile_defs` resource-type list, in source order; empty when none written.
        types: ThinVec<ShowProfileType>,
        /// The `FOR QUERY <n>` query-id selector (an integer [`Literal`]); `None` when absent.
        query: Option<Literal>,
        /// The optional trailing `LIMIT …` narrowing.
        limit: Option<ShowLimit>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW {BINLOG | RELAYLOG} EVENTS [IN '<log>'] [FROM <pos>] [LIMIT …] [FOR CHANNEL
    /// '<channel>']` — the binary- / relay-log event dump (MySQL; gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The two spellings share one payload — an optional `IN '<log>'` log-file name, an
    /// optional `FROM <pos>` start position, and the shared [`ShowLimit`] tail — so they ride
    /// this one variant with the spelling as DATA on [`relay`](Self::LogEvents::relay), the
    /// SHOW family's fold-near-identical-sub-commands precedent. The sole grammar difference
    /// is the trailing `FOR CHANNEL '<channel>'`: it is `RELAYLOG`-only, so
    /// [`channel`](Self::LogEvents::channel) is `Some` only when [`relay`](Self::LogEvents::relay)
    /// is `true` (`SHOW BINLOG EVENTS FOR CHANNEL …` is `ER_PARSE_ERROR` on mysql:8.4.10). The
    /// clause order is fixed: `SHOW BINLOG EVENTS FROM 4 IN '<log>'` is `ER_PARSE_ERROR` (the
    /// `IN` must precede the `FROM`).
    LogEvents {
        /// The log spelling: `false` for `BINLOG`, `true` for `RELAYLOG`.
        relay: bool,
        /// The `IN '<log>'` log-file name (a string [`Literal`]); `None` when absent.
        log_name: Option<Literal>,
        /// The `FROM <pos>` start position (an integer [`Literal`]); `None` when absent.
        position: Option<Literal>,
        /// The optional trailing `LIMIT …` narrowing.
        limit: Option<ShowLimit>,
        /// The `FOR CHANNEL '<channel>'` replication-channel qualifier (a string [`Literal`]);
        /// `Some` only for `RELAYLOG` (see [`relay`](Self::LogEvents::relay)).
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW [EXTENDED] {INDEX | INDEXES | KEYS} {FROM | IN} <tbl> [{FROM | IN} <db>]
    /// [WHERE <expr>]` — the index-listing statement (MySQL; gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// Structurally the `{FROM | IN}`-qualified sibling of [`Columns`](Self::Columns): a
    /// mandatory table qualifier, an optional database qualifier, and a filter — but the
    /// filter here is `WHERE`-only (the MySQL grammar admits no `LIKE` on `SHOW INDEX`), so
    /// the parser only ever builds a [`ShowFilter::Where`]. `KEYS`/`INDEX`/`INDEXES` are
    /// exact synonyms whose written spelling rides the [`ShowIndexSpelling`] tag.
    Index {
        /// Which keyword named the listing: `INDEX`, `INDEXES`, or `KEYS`.
        spelling: ShowIndexSpelling,
        /// MySQL `EXTENDED` — also list hidden indexes MySQL maintains internally.
        extended: bool,
        /// The mandatory `{FROM | IN} <tbl>` qualifier naming the target table.
        table: ShowFrom,
        /// The optional second `{FROM | IN} <db>` qualifier naming the database.
        database: Option<ShowFrom>,
        /// The optional trailing `WHERE <expr>` narrowing (never `LIKE`).
        filter: Option<ShowFilter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW ENGINE {<name> | ALL} {STATUS | MUTEX | LOGS}` — a storage-engine diagnostic
    /// dump (MySQL; gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The engine operand ([`engine`](Self::Engine::engine)) is a named engine, or `None`
    /// for the `ALL` wildcard; the requested artefact ([`artifact`](Self::Engine::artifact))
    /// is one of the three fixed report keywords. Distinct from the bare
    /// [`ShowBare::Engines`] catalogue listing, which takes no operand.
    Engine {
        /// The named storage engine, or `None` for the `ALL` wildcard.
        engine: Option<Ident>,
        /// Which per-engine report was requested; see [`ShowEngineArtifact`].
        artifact: ShowEngineArtifact,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW REPLICA STATUS [FOR CHANNEL '<channel>']` — the replication-applier status
    /// (MySQL; gated by [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The optional [`channel`](Self::ReplicaStatus::channel) names a replication channel
    /// (`FOR CHANNEL '<name>'`). MySQL 8.4 removed the deprecated `SHOW SLAVE STATUS`
    /// terminology, so there is no spelling axis here.
    ReplicaStatus {
        /// The optional `FOR CHANNEL '<channel>'` qualifier.
        channel: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW {WARNINGS | ERRORS} [LIMIT [<offset>,] <row_count>]` and `SHOW COUNT(*)
    /// {WARNINGS | ERRORS}` — the diagnostics-area readouts (MySQL; gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// The `WARNINGS` vs `ERRORS` choice rides [`kind`](Self::Diagnostics::kind).
    /// [`count`](Self::Diagnostics::count) records the `COUNT(*)` cardinality form, which is
    /// mutually exclusive with a [`limit`](Self::Diagnostics::limit) in the grammar — the
    /// parser never sets both.
    Diagnostics {
        /// Which diagnostics list was named: `WARNINGS` or `ERRORS`.
        kind: ShowDiagnosticKind,
        /// Whether this is the `SHOW COUNT(*) …` cardinality form (no `LIMIT`).
        count: bool,
        /// The optional `LIMIT [<offset>,] <row_count>` narrowing (never set when
        /// [`count`](Self::Diagnostics::count) is `true`).
        limit: Option<ShowLimit>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW {PROCEDURE | FUNCTION} CODE <name>` — the compiled stored-routine instruction
    /// dump (MySQL debug builds; gated by
    /// [`show_admin`](crate::dialect::ShowSyntax::show_admin)).
    ///
    /// Shares the [`ShowRoutineKind`] object-keyword axis with
    /// [`RoutineStatus`](Self::RoutineStatus); the operand is the (schema-qualifiable)
    /// routine name.
    RoutineCode {
        /// Which stored-routine kind was named: `FUNCTION` or `PROCEDURE`.
        kind: ShowRoutineKind,
        /// The (optionally schema-qualified) routine name.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which stored-routine kind a [`ShowTarget::RoutineStatus`] listing named: `FUNCTION` or
/// `PROCEDURE` (MySQL `SHOW {FUNCTION | PROCEDURE} STATUS`).
///
/// A surface tag (no `meta`, like [`ShowColumnsSpelling`]): the keyword's span is subsumed
/// by the enclosing [`ShowStatement`]. Recorded only so the written object keyword
/// round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowRoutineKind {
    /// `FUNCTION` — list stored functions.
    Function,
    /// `PROCEDURE` — list stored procedures.
    Procedure,
}

/// The optional scope keyword before `FUNCTIONS` in a [`ShowTarget::Functions`] listing:
/// `USER`, `SYSTEM`, or `ALL` (Spark / Databricks).
///
/// A surface tag (no `meta`, like [`ShowColumnsSpelling`]): the keyword's span is subsumed
/// by the enclosing [`ShowStatement`]. Recorded only so the written scope round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowFunctionsScope {
    /// `USER` — user-defined functions only.
    User,
    /// `SYSTEM` — system (built-in) functions only.
    System,
    /// `ALL` — both user and system functions.
    All,
}

/// The optional trailing narrowing of a [`ShowTarget::Functions`] listing:
/// `[LIKE] {<function_name> | '<regex_pattern>'}` (Spark / Databricks).
///
/// A distinct type from [`ShowFilter`] because that models MySQL's mutually-exclusive
/// `LIKE '<pat>' | WHERE <expr>` (a mandatory keyword, no bare-name form, and a `WHERE`
/// predicate `SHOW FUNCTIONS` does not accept). Here the `LIKE` keyword is *optional* and
/// the operand is either a bare (optionally qualified) function name or a quoted regex
/// string; [`like`](Self::Name::like) records whether the keyword was written so the
/// statement round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowFunctionsFilter {
    /// `[LIKE] <function_name>` — a bare, optionally-qualified function name.
    Name {
        /// Whether the optional `LIKE` keyword preceded the name.
        like: bool,
        /// The (optionally qualified) function name to match.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `[LIKE] '<regex_pattern>'` — a quoted regex-pattern string.
    Regex {
        /// Whether the optional `LIKE` keyword preceded the pattern.
        like: bool,
        /// The quoted regex-pattern string literal.
        pattern: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which keyword named a [`ShowTarget::Columns`] listing: `COLUMNS` or its exact `FIELDS`
/// synonym (MySQL).
///
/// A surface tag (no `meta`, like [`ShowFromKeyword`]): the keyword's span is subsumed by
/// the enclosing [`ShowStatement`]. Recorded only so the written spelling round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowColumnsSpelling {
    /// Source used the `COLUMNS` spelling.
    Columns,
    /// Source used the `FIELDS` spelling.
    Fields,
}

/// A `{FROM | IN} <name>` object qualifier in the typed-`SHOW` family.
///
/// Used for the [`ShowTarget::Tables`] database qualifier and for both the mandatory
/// table and optional database qualifiers of [`ShowTarget::Columns`], so [`name`](Self::name)
/// is a generic [`ObjectName`] rather than a database- or table-specific field. MySQL
/// accepts either keyword interchangeably; DuckDB accepts only `FROM`. The
/// [`keyword`](Self::keyword) surface tag records which was written so the statement
/// round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ShowFrom {
    /// Whether `FROM` or `IN` introduced the database; see [`ShowFromKeyword`].
    pub keyword: ShowFromKeyword,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which keyword introduced a [`ShowFrom`]: `FROM` or its `IN` synonym.
///
/// A surface tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`ShowFrom`], as [`ExplainKeyword`] rides its parent's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowFromKeyword {
    /// `SHOW … FROM <db>`.
    From,
    /// `SHOW … IN <db>` — a synonym for `FROM`.
    In,
}

/// The optional trailing narrowing of a [`ShowTarget::Tables`] or [`ShowTarget::Columns`]:
/// a `LIKE` name pattern or a `WHERE` predicate (MySQL).
///
/// The two are mutually exclusive in the grammar. `LIKE` takes a string pattern
/// ([`Literal`]); `WHERE` takes a general predicate ([`Expr`]) over the result columns,
/// which is why this enum — and thus [`ShowTarget`]/[`ShowStatement`] — is generic over
/// the extension `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowFilter<X: Extension = NoExt> {
    /// A `LIKE '<pattern>'` name filter.
    Like {
        /// Pattern matched by this syntax.
        pattern: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `WHERE <predicate>` filter (MySQL).
    Where {
        /// Predicate that controls this clause.
        predicate: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which object kind a [`ShowTarget::Create`] recreates: `TABLE`, `VIEW`, `DATABASE`,
/// `EVENT`, `PROCEDURE`, `FUNCTION`, or `TRIGGER` (MySQL `SHOW CREATE <kind> <name>`).
///
/// A surface tag (no `meta`, like [`ShowRoutineKind`]): the keyword's span is subsumed by
/// the enclosing [`ShowStatement`]. `USER` is absent — its operand is a user specification,
/// not an [`ObjectName`], so `SHOW CREATE USER` rides its own
/// [`ShowTarget::CreateUser`] variant over the shared [`AccountName`] grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowCreateKind {
    /// `SHOW CREATE TABLE <tbl>`.
    Table,
    /// `SHOW CREATE VIEW <view>`.
    View,
    /// `SHOW CREATE {DATABASE | SCHEMA} [IF NOT EXISTS] <db>`.
    Database {
        /// Whether the `SCHEMA` synonym spelling was written in place of `DATABASE`.
        schema: bool,
    },
    /// `SHOW CREATE EVENT <event>`.
    Event,
    /// `SHOW CREATE PROCEDURE <proc>`.
    Procedure,
    /// `SHOW CREATE FUNCTION <func>`.
    Function,
    /// `SHOW CREATE TRIGGER <trigger>`.
    Trigger,
}

/// Which catalogue/server listing a [`ShowTarget::Listing`] named, plus that sub-command's
/// own small *scalar* payload (MySQL). Every member admits the shared `[LIKE | WHERE]` tail
/// and the optional `{FROM | IN} <db>` qualifier carried by the enclosing
/// [`ShowTarget::Listing`].
///
/// A surface discriminator (no `meta`, `Copy` like [`ShowScope`]): the `{FROM | IN}`
/// qualifier — the only spanned payload — lives on the enclosing variant, leaving this enum
/// a pure keyword tag plus scalar flags whose spans are subsumed by the [`ShowStatement`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowListing {
    /// `SHOW {DATABASES | SCHEMAS}` (takes no `{FROM | IN}` qualifier).
    Databases {
        /// Whether the `SCHEMAS` synonym spelling was written in place of `DATABASES`.
        schemas: bool,
    },
    /// `SHOW {CHARACTER SET | CHARSET}` (takes no `{FROM | IN}` qualifier).
    CharacterSet {
        /// Whether the one-word `CHARSET` spelling was written in place of `CHARACTER SET`.
        charset: bool,
    },
    /// `SHOW COLLATION` (takes no `{FROM | IN}` qualifier).
    Collation,
    /// `SHOW [GLOBAL | SESSION | LOCAL] STATUS` (takes no `{FROM | IN}` qualifier).
    Status {
        /// The optional `GLOBAL`/`SESSION`/`LOCAL` scope; see [`ShowScope`].
        scope: Option<ShowScope>,
    },
    /// `SHOW [GLOBAL | SESSION | LOCAL] VARIABLES` (takes no `{FROM | IN}` qualifier).
    Variables {
        /// The optional `GLOBAL`/`SESSION`/`LOCAL` scope; see [`ShowScope`].
        scope: Option<ShowScope>,
    },
    /// `SHOW EVENTS [{FROM | IN} <db>]`.
    Events,
    /// `SHOW TABLE STATUS [{FROM | IN} <db>]`.
    TableStatus,
    /// `SHOW OPEN TABLES [{FROM | IN} <db>]`.
    OpenTables,
    /// `SHOW [FULL] TRIGGERS [{FROM | IN} <db>]`.
    Triggers {
        /// MySQL `FULL` — add the `sql_mode`, definer, and character-set columns.
        full: bool,
    },
}

/// The optional `GLOBAL`/`SESSION`/`LOCAL` scope keyword on `SHOW … STATUS` / `SHOW …
/// VARIABLES` (MySQL `opt_var_type`).
///
/// A surface tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`ShowStatement`]. `LOCAL` is an exact synonym of `SESSION` in MySQL, kept distinct here
/// only so the written spelling round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowScope {
    /// `GLOBAL` — the server-wide value.
    Global,
    /// `SESSION` — the current session's value.
    Session,
    /// `LOCAL` — a synonym for `SESSION`.
    Local,
}

/// A MySQL `SHOW` sub-command taking no filter and no name operand; the discriminant of
/// [`ShowTarget::Bare`]. The only payloads are single leading-keyword flags.
///
/// A surface tag (no `meta`): each keyword's span is subsumed by the enclosing
/// [`ShowStatement`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowBare {
    /// `SHOW PLUGINS`.
    Plugins,
    /// `SHOW [STORAGE] ENGINES`.
    Engines {
        /// Whether the optional `STORAGE` keyword preceded `ENGINES`.
        storage: bool,
    },
    /// `SHOW PRIVILEGES`.
    Privileges,
    /// `SHOW PROFILES`.
    Profiles,
    /// `SHOW [FULL] PROCESSLIST`.
    Processlist {
        /// MySQL `FULL` — show the full `Info` column instead of truncating it.
        full: bool,
    },
    /// `SHOW BINARY LOGS`.
    BinaryLogs,
    /// `SHOW REPLICAS`.
    Replicas,
    /// `SHOW BINARY LOG STATUS`.
    BinaryLogStatus,
}

/// Which keyword named a [`ShowTarget::Index`] listing: `INDEX`, `INDEXES`, or `KEYS`
/// (MySQL `keys_or_index`).
///
/// A surface tag (no `meta`, like [`ShowColumnsSpelling`]): the three are exact synonyms,
/// recorded only so the written spelling round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowIndexSpelling {
    /// Source used the singular `INDEX` spelling.
    Index,
    /// Source used the plural `INDEXES` spelling.
    Indexes,
    /// Source used the `KEYS` spelling.
    Keys,
}

/// Which per-engine report a [`ShowTarget::Engine`] dump requested: `STATUS`, `MUTEX`, or
/// `LOGS` (MySQL).
///
/// A surface tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`ShowStatement`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowEngineArtifact {
    /// `SHOW ENGINE <e> STATUS`.
    Status,
    /// `SHOW ENGINE <e> MUTEX`.
    Mutex,
    /// `SHOW ENGINE <e> LOGS`.
    Logs,
}

/// Which diagnostics list a [`ShowTarget::Diagnostics`] readout named: `WARNINGS` or
/// `ERRORS` (MySQL).
///
/// A surface tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`ShowStatement`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowDiagnosticKind {
    /// `SHOW WARNINGS` / `SHOW COUNT(*) WARNINGS`.
    Warnings,
    /// `SHOW ERRORS` / `SHOW COUNT(*) ERRORS`.
    Errors,
}

/// One resource-type selector in a [`ShowTarget::Profile`] `profile_defs` list (MySQL).
///
/// A surface tag (no `meta`, `Copy` like [`ShowScope`]): every member is a pure keyword or
/// keyword pair with no operand, so the list's span is subsumed by the enclosing
/// [`ShowStatement`]. MySQL folds duplicates into a bitmask at bind time; the parser keeps
/// the written list order and multiplicity so the statement round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowProfileType {
    /// `ALL` — every available profile column.
    All,
    /// `BLOCK IO` — block input/output counts.
    BlockIo,
    /// `CONTEXT SWITCHES` — voluntary and involuntary context switches.
    ContextSwitches,
    /// `CPU` — user and system CPU time.
    Cpu,
    /// `IPC` — messages sent and received.
    Ipc,
    /// `MEMORY` — memory usage (currently unimplemented server-side, still grammar-valid).
    Memory,
    /// `PAGE FAULTS` — major and minor page faults.
    PageFaults,
    /// `SOURCE` — the source-file function names, files, and line numbers.
    Source,
    /// `SWAPS` — swap counts.
    Swaps,
}

/// The shared `LIMIT` narrowing on the MySQL `SHOW` family — MySQL's `opt_limit_clause`
/// (`SHOW {WARNINGS | ERRORS}`, `SHOW PROFILE`, `SHOW {BINLOG | RELAYLOG} EVENTS`).
///
/// Models all three surface forms of `limit_options`:
///
/// * `LIMIT <row_count>` — no offset ([`offset`](Self::offset) `None`).
/// * `LIMIT <offset>, <row_count>` — the comma form, offset written first
///   ([`offset_keyword`](Self::offset_keyword) `false`).
/// * `LIMIT <row_count> OFFSET <offset>` — the `OFFSET`-keyword form
///   ([`offset_keyword`](Self::offset_keyword) `true`).
///
/// The two offset spellings are semantically identical (`LIMIT 2, 5` == `LIMIT 5 OFFSET 2`);
/// [`offset_keyword`](Self::offset_keyword) records which was written so the statement
/// round-trips, and is always `false` when [`offset`](Self::offset) is `None`. Both operands
/// are integer [`Literal`]s (MySQL's `limit_option` also admits a `?` param-marker or a
/// user-variable, deferred — the whole `SHOW` family parses only integer limits today).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ShowLimit {
    /// The optional `<offset>` — present for both the comma and `OFFSET`-keyword forms.
    pub offset: Option<Literal>,
    /// When [`offset`](Self::offset) is present, whether it was written with the `OFFSET`
    /// keyword (`LIMIT <row_count> OFFSET <offset>`) rather than the comma form
    /// (`LIMIT <offset>, <row_count>`); always `false` when there is no offset.
    pub offset_keyword: bool,
    /// The `<row_count>` — the sole operand, the second operand of the comma form, or the
    /// first operand of the `OFFSET`-keyword form.
    pub row_count: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}
