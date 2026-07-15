// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! M2 SQLite + DuckDB accept/reject differential oracles.
//!
//! The M2 dialect milestone (ADR-0015) extends the pluggable
//! [`AcceptRejectOracle`] seam with two real in-process engines:
//!
//! - [`SqliteOracle`] — `rusqlite` (bundled SQLite), paired with the fitted
//!   [`Sqlite`](squonk::dialect::Sqlite) dialect.
//! - [`DuckDbOracle`] — thin system-`libduckdb` FFI ([`crate::duckdb_ffi`]), paired with the fitted
//!   [`DuckDb`](squonk::dialect::DuckDb) dialect.
//!
//! Both are opt-in behind the `oracle-engines` cargo feature so the default build
//! links no system library and needs no environment. See `conformance/Cargo.toml`.
//!
//! # Prepare-only, never execute
//!
//! [`AcceptRejectOracle::verdict`] calls
//! only `Connection::prepare` — the engine parses and binds the statement but never
//! runs it, so a `SELECT`/`INSERT` under test has no side effect
//! (`oracles_never_execute` proves the provisioned table stays empty after preparing
//! an `INSERT`). One caveat: DuckDB's `prepare` *executes* every statement but the
//! last of a multi-statement string, so the differential must only ever be handed a
//! single statement (`corpus_is_single_statement` enforces it).
//!
//! # PrepareBind semantics and the setup driver
//!
//! Both engines are [`OracleSemantics::PrepareBind`]:
//! `prepare` resolves names against the session schema, so an unknown table/column
//! *rejects*. Our parser does not bind, so comparing over schema-dependent SQL
//! (`SELECT a FROM t1`) against an empty database yields a **false** divergence — we
//! accept, the engine rejects "no such table" (`setup_driver_prevents_false_divergence`
//! demonstrates exactly this). The differential therefore runs over two disjoint
//! curated corpora:
//!
//! - [`SCHEMA_INDEPENDENT_ACCEPT`]/[`SCHEMA_INDEPENDENT_REJECT`] — no object names, so
//!   comparable against a bare in-memory database ([`SqliteOracle::new`]).
//! - [`SCHEMA_DEPENDENT_ACCEPT`] — references [`SCHEMA_SETUP_SQL`], provisioned first
//!   via the setup driver ([`SqliteOracle::with_schema`]).
//!
//! # Dialect pairing (scope decision)
//!
//! SQLite pairs with its fitted [`Sqlite`](squonk::dialect::Sqlite) preset
//! (`sqlite-featureset-preset` closed the FeatureSet-expressible families the phase-0
//! sweep surfaced), so a SQLite-side gap here is a genuine grammar family, never a
//! preset-fitting artifact. DuckDB likewise pairs with its fitted
//! [`DuckDb`](squonk::dialect::DuckDb) preset (`duckdb-featureset-preset`) — the
//! PostgreSQL-derived dialect carrying the DuckDB numeric-radix widening and the
//! empty-target-list tightening — which replaced the earlier `Postgres` stopgap. Both
//! pairings surface **no** real accept/reject divergence in the "too permissive"
//! direction; the divergences that exist are all the other way — the engine accepts
//! dialect syntax our pairing does not yet parse (see [`SQLITE_GRAMMAR_GAPS`] /
//! [`DUCKDB_GRAMMAR_GAPS`]), a grammar gap owned by the family children. The M2
//! milestone is visible in the dialect matrix via
//! `coverage::render_m2_oracle_matrix`.
//!
//! # Accept/reject only
//!
//! Structural parity (a neutral parse-tree shape, e.g. DuckDB `json_serialize_sql`)
//! is intentionally out of scope — see `prod-conformance-structural-oracle` and the
//! [`oracle`](crate::oracle) module contract.

use crate::duckdb_ffi::Connection as DuckDbConnection;
use rusqlite::Connection as SqliteConnection;
use squonk::parse_with;

use crate::oracle::{AcceptRejectOracle, OracleSemantics, OracleUnavailable, OracleVerdict};
use crate::sqlite_ffi::SqliteSegmentation;

/// The bundled-SQLite (`rusqlite`) prepare-only accept/reject oracle, paired with the
/// fitted [`Sqlite`](squonk::dialect::Sqlite) dialect.
///
/// In-process, so it never reports [`OracleUnavailable`] from
/// [`verdict`](AcceptRejectOracle::verdict); only opening the in-memory database can
/// fail (an infrastructure error surfaced at construction).
pub struct SqliteOracle {
    conn: SqliteConnection,
}

impl SqliteOracle {
    /// A bare in-memory database with no schema — for the schema-independent corpus.
    pub fn new() -> Result<Self, OracleUnavailable> {
        let conn = SqliteConnection::open_in_memory()
            .map_err(|err| OracleUnavailable(format!("sqlite open_in_memory failed: {err}")))?;
        Ok(Self { conn })
    }

    /// The setup driver: an in-memory database with `setup_sql` (DDL) executed to
    /// provision the schema the schema-dependent corpus references. The DDL is the
    /// *only* statement ever executed; corpus statements are only `prepare`d.
    pub fn with_schema(setup_sql: &str) -> Result<Self, OracleUnavailable> {
        let oracle = Self::new()?;
        oracle
            .conn
            .execute_batch(setup_sql)
            .map_err(|err| OracleUnavailable(format!("sqlite schema setup failed: {err}")))?;
        Ok(oracle)
    }
}

impl AcceptRejectOracle for SqliteOracle {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::PrepareBind
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        // `prepare` parses + binds without executing; a rejected statement is a
        // verdict, never an `OracleUnavailable` (which is reserved for infra faults).
        Ok(OracleVerdict::from_accepts(self.conn.prepare(sql).is_ok()))
    }
}

/// The DuckDB (thin system-`libduckdb` FFI) prepare-only accept/reject
/// oracle, paired with the fitted [`DuckDb`](squonk::dialect::DuckDb) dialect.
///
/// In-process once the shared library loads, so like [`SqliteOracle`] it reports
/// [`OracleUnavailable`] only at construction (a failed connection open).
pub struct DuckDbOracle {
    conn: DuckDbConnection,
}

impl DuckDbOracle {
    /// A bare in-memory database with no schema — for the schema-independent corpus.
    pub fn new() -> Result<Self, OracleUnavailable> {
        let conn = DuckDbConnection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// The setup driver — as [`SqliteOracle::with_schema`], provisioning the schema
    /// the schema-dependent corpus references before any `prepare`.
    pub fn with_schema(setup_sql: &str) -> Result<Self, OracleUnavailable> {
        let oracle = Self::new()?;
        oracle.conn.execute_batch(setup_sql)?;
        Ok(oracle)
    }

    /// The linked `libduckdb` version string (`duckdb_library_version` via the FFI
    /// connection), for the "oracle actually ran" CI evidence the parity test emits.
    pub fn version(&self) -> String {
        self.conn.version()
    }
}

impl AcceptRejectOracle for DuckDbOracle {
    fn name(&self) -> &'static str {
        "duckdb"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::PrepareBind
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        // Single-statement only: DuckDB's `prepare` executes all but the last
        // statement of a multi-statement string (see module docs); the corpus is
        // one statement per entry, enforced by `corpus_is_single_statement`.
        Ok(OracleVerdict::from_accepts(self.conn.prepare_ok(sql)))
    }
}

/// Schema provisioned by the setup driver before the [`SCHEMA_DEPENDENT_ACCEPT`]
/// corpus is compared. Kept to two small integer/text tables that both engines
/// accept, so the differential exercises name resolution without engine-specific DDL.
pub const SCHEMA_SETUP_SQL: &str = "CREATE TABLE t1(a INTEGER, b INTEGER, c INTEGER, d INTEGER, e INTEGER); \
     CREATE TABLE t2(f INTEGER, g VARCHAR)";

/// A triaged M2 accept/reject divergence: a statement where a real engine and our
/// paired dialect disagree, that we knowingly tolerate. Every entry must name an
/// a non-empty provenance label; the tests assert each still diverges, so a fixed
/// gap cannot stay silently allowlisted (mirrors `pg::PG_DIVERGENCE_ALLOWLIST`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M2DivergenceAllowlistEntry {
    pub engine: &'static str,
    pub sql: &'static str,
    pub ticket: &'static str,
    pub reason: &'static str,
}

/// Current M2 accept/reject divergences allowed by the oracles.
///
/// Empty: measured over the curated corpora and the vendored sqllogictest corpus, the
/// SQLite/DuckDB engines and our fitted `Sqlite`/`DuckDb` pairings never disagree on
/// acceptance in the "we accept, the engine rejects" direction. The other direction — the engine
/// accepting dialect syntax we do not yet parse — is a *grammar gap*, tracked
/// separately ([`SQLITE_GRAMMAR_GAPS`]/[`DUCKDB_GRAMMAR_GAPS`]), not a divergence.
pub const M2_DIVERGENCE_ALLOWLIST: &[M2DivergenceAllowlistEntry] = &[];

/// Whether a divergence for `engine`/`sql` is named in [`M2_DIVERGENCE_ALLOWLIST`].
///
/// The keying mirrors [`pg::pg_divergence_allowlisted`](crate::pg::pg_divergence_allowlisted)
/// (per-engine, per-string, ticket-named): the raw-byte differentials compose it so a
/// triaged, ticketed divergence stays green while every un-triaged one panics.
pub fn m2_divergence_allowlisted(engine: &str, sql: &str) -> bool {
    M2_DIVERGENCE_ALLOWLIST
        .iter()
        .any(|entry| entry.engine == engine && entry.sql == sql)
}

/// The parse-only, segmentation-aware accept/reject divergence for the SQLite
/// raw-byte differential — `Some(detail)` when real SQLite and our fitted
/// [`Sqlite`](squonk::dialect::Sqlite) preset disagree on whether `sql` parses,
/// or (when both accept and SQLite's statement count is reliable) on the top-level
/// statement count, else `None`.
///
/// Never executes: the count comes from `sqlite3_prepare_v2` + `pzTail` with each
/// statement finalized un-stepped ([`crate::sqlite_ffi`]). The comparison basis is
/// *syntactic* acceptance — a SQLite name-resolution failure reads as an accept
/// (proof of parse), matching our parse-only parser — so the differential does not
/// import the [`PrepareBind`](crate::oracle::OracleSemantics::PrepareBind)
/// false-divergence problem the curated corpus manages with a provisioned schema. The
/// count half is compared only when SQLite's iteration was not resolution-truncated
/// (`sqlite_ffi` module docs).
///
/// Segmentation-aware, not merely boolean: this is the SQLite arm of the
/// statement-splitter hunt the strengthened PostgreSQL differential
/// ([`pg::pg_accept_reject_divergence`](crate::pg::pg_accept_reject_divergence))
/// carries — a boolean `accept == accept` masks a mis-split, so counts are compared
/// when both accept.
///
/// Raw: no allowlist applied (callers compose [`m2_divergence_allowlisted`]).
pub fn sqlite_raw_bytes_divergence(conn: &SqliteConnection, sql: &str) -> Option<String> {
    let ours = parse_with(sql, squonk::ParseConfig::new(squonk::dialect::Sqlite))
        .ok()
        .map(|p| p.statements().len());
    let (accepts, count, reliable) = match crate::sqlite_ffi::segment(conn, sql) {
        SqliteSegmentation::Reject(_) => (false, 0, false),
        SqliteSegmentation::Accept {
            count,
            count_reliable,
        } => (true, count, count_reliable),
    };
    raw_bytes_divergence("sqlite", ours, accepts, count, reliable)
}

/// The parse-only, segmentation-aware accept/reject divergence for the DuckDB
/// raw-byte differential — `Some(detail)` when real DuckDB and our fitted
/// [`DuckDb`](squonk::dialect::DuckDb) preset disagree on whether `sql` parses,
/// or (when both accept) on the top-level statement count, else `None`.
///
/// Never executes: the count comes from `duckdb_extract_statements` — the parser, not
/// the preparer — so no statement is bound or run, sidestepping the "DuckDB executes
/// all but the last statement of a multi-statement prepare" hazard the accept/reject
/// oracle path carries ([`crate::duckdb_ffi::Connection::extract_statement_count`]).
/// Extraction is parse-only, so an unresolved name still parses — the same
/// [`ParseOnly`](crate::oracle::OracleSemantics::ParseOnly) footing as PostgreSQL —
/// and DuckDB's count is always reliable (no resolution truncation, unlike SQLite).
///
/// Raw: no allowlist applied (callers compose [`m2_divergence_allowlisted`]).
pub fn duckdb_raw_bytes_divergence(conn: &DuckDbConnection, sql: &str) -> Option<String> {
    let ours = parse_with(sql, squonk::ParseConfig::new(squonk::dialect::DuckDb))
        .ok()
        .map(|p| p.statements().len());
    let (accepts, count) = match conn.extract_statement_count(sql) {
        Ok(count) => (true, count),
        Err(_) => (false, 0),
    };
    raw_bytes_divergence("duckdb", ours, accepts, count, true)
}

/// Owner: `duckdb-extract-nonascii-swallow-allowlist`.
///
/// Whether the raw-byte DuckDB differential's divergence on `sql` is the DuckDB
/// **non-ASCII-swallow** engine-API quirk — a `duckdb=accept, squonk=reject`
/// divergence to pre-filter, not a parser gap.
///
/// Measured against libduckdb 1.5.4, `duckdb_extract_statements` treats an
/// unrecognized non-ASCII character that falls *outside* a string literal, quoted
/// identifier, or comment as insignificant (whitespace-like) and silently drops it.
/// A bare run (`"ǧ"` = `c7 a7`, `"ش"` = `d8 b4`) extracts to zero statements with a
/// NULL error pointer — indistinguishable from empty input at the C API — and a run
/// between/after real statements (`"SELECT 1;ش"`, `"ش;SELECT 1"`) extracts to a count
/// that omits the dropped bytes. Our tokenizer instead reads a non-ASCII letter as an
/// identifier character, so it rejects such input as a syntax error; the differential
/// then reports engine-accept / ours-reject. Bare unrecognized *ASCII* (`"@"`, `"~"`)
/// errors properly on DuckDB's side, so this is specific to non-ASCII.
///
/// The measured boundary (probe evidence on this ticket):
///
/// | input class                                   | DuckDB        | ours     | swallow |
/// |-----------------------------------------------|---------------|----------|---------|
/// | bare non-ASCII (any block: Latin/Arabic/CJK/…)| `Ok(0)`       | reject   | yes     |
/// | non-ASCII after/between `;`-terminated stmts  | `Ok(n)`       | reject   | yes     |
/// | non-ASCII inside a `'…'` / `"…"` / comment     | `Ok(1)`       | accept   | no      |
/// | non-ASCII adjacent to a token (`"شSELECT 1"`)  | `Err` (parse) | reject   | no      |
/// | `U&'\…'` / `U&"…"` unicode escapes             | `Err` (unimpl)| accept   | no      |
/// | bare/leading unrecognized ASCII (`"@"`, `"x"`) | `Err` (parse) | reject   | no      |
///
/// Proven per input, never assumed, so every *other* non-ASCII divergence stays
/// visible: a both-accept case (non-ASCII inside a string/identifier/comment) has
/// `ours = Some` and returns `false` here immediately, and a DuckDB reject (the `U&`
/// escapes) is `Err` and also returns `false`. The proof that DuckDB truly swallowed:
/// with the bare-context non-ASCII removed (string/quoted-identifier/comment content
/// preserved by `strip_bare_context_nonascii`), DuckDB's accepted statement count is
/// unchanged **and** our parser then agrees with it — i.e. the non-ASCII contributed
/// nothing to DuckDB's parse. Any non-ASCII byte DuckDB treats as *significant* would
/// change that count (or leave our post-strip parse still rejecting), so it stays a
/// reported divergence rather than being suppressed here.
pub fn duckdb_nonascii_swallow(conn: &DuckDbConnection, sql: &str) -> bool {
    if sql.is_ascii() {
        return false;
    }
    // Swallow direction only: DuckDB accepts, our parser rejects. A both-accept case
    // (non-ASCII inside a string/identifier/comment) has `ours = Some` and is left
    // fully visible; a DuckDB reject (the `U&` escapes) is `Err` below and left visible.
    if parse_with(sql, squonk::ParseConfig::new(squonk::dialect::DuckDb)).is_ok() {
        return false;
    }
    let Ok(engine_count) = conn.extract_statement_count(sql) else {
        return false;
    };
    let stripped = strip_bare_context_nonascii(sql);
    // Nothing stripped => every non-ASCII byte lived inside a protected
    // (string/identifier/comment) span, so this is not the bare-context swallow.
    if stripped == sql {
        return false;
    }
    let Ok(stripped_count) = conn.extract_statement_count(&stripped) else {
        return false;
    };
    let stripped_ours = parse_with(&stripped, squonk::ParseConfig::new(squonk::dialect::DuckDb))
        .ok()
        .map(|p| p.statements().len());
    // The swallow proof: DuckDB's count is unchanged by removing the non-ASCII (it
    // dropped those bytes) and, with them gone, our parser accepts the same count.
    engine_count == stripped_count && stripped_ours == Some(engine_count)
}

/// Remove every non-ASCII byte that lies *outside* a single-quoted string, a
/// double-quoted identifier, or a `--` / `/* */` comment, preserving the bytes inside
/// those spans verbatim (companion to [`duckdb_nonascii_swallow`]).
///
/// A deliberately small lexical scanner over the standard-SQL string/comment surface
/// DuckDB shares — not a full tokenizer — because its output is only ever a *candidate*
/// the caller re-validates against the engine (an over- or under-strip fails the
/// count-invariance check there and simply declines to suppress, never blinds the
/// instrument). Standard `''` / `""` doubling keeps a quote's own escape inside the
/// span; a backslash is an ordinary byte (DuckDB strings are standard-conforming).
fn strip_bare_context_nonascii(sql: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut state = State::Normal;
    let mut block_depth = 0_u32;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match state {
            State::Normal => {
                if b == b'\'' {
                    state = State::SingleQuote;
                    out.push(b);
                } else if b == b'"' {
                    state = State::DoubleQuote;
                    out.push(b);
                } else if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
                    state = State::LineComment;
                    out.push(b);
                    out.push(b'-');
                    i += 2;
                    continue;
                } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    state = State::BlockComment;
                    block_depth = 1;
                    out.push(b);
                    out.push(b'*');
                    i += 2;
                    continue;
                } else if b < 0x80 {
                    out.push(b);
                }
                // else: a bare-context non-ASCII byte — dropped.
            }
            State::SingleQuote => {
                out.push(b);
                if b == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        out.push(b'\'');
                        i += 2;
                        continue;
                    }
                    state = State::Normal;
                }
            }
            State::DoubleQuote => {
                out.push(b);
                if b == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        out.push(b'"');
                        i += 2;
                        continue;
                    }
                    state = State::Normal;
                }
            }
            State::LineComment => {
                out.push(b);
                // DuckDB's line-comment body ends at either newline byte. Ending at
                // carriage return is essential here: a following bare non-ASCII token
                // is swallowed by `extract_statements`, not protected comment text.
                if b == b'\n' || b == b'\r' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                out.push(b);
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    out.push(b'*');
                    block_depth = block_depth
                        .checked_add(1)
                        .expect("block-comment nesting depth exceeds u32::MAX");
                    i += 2;
                    continue;
                }
                if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    out.push(b'/');
                    block_depth -= 1;
                    if block_depth == 0 {
                        state = State::Normal;
                    }
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    // Every retained byte is ASCII (dropped bytes are exactly the bare-context
    // non-ASCII) or lives inside a preserved span, so the result is valid UTF-8.
    String::from_utf8(out).expect("stripping bare-context non-ASCII keeps valid UTF-8")
}

/// The shared accept/reject + segmentation comparison for both raw-byte differentials
/// (the M2 analogue of [`pg::pg_accept_reject_divergence`](crate::pg::pg_accept_reject_divergence)).
///
/// `ours` is `Some(count)` when our parser accepts, `None` when it rejects. When both
/// accept and `engine_count_reliable`, a statement-count mismatch is a segmentation
/// divergence; otherwise only the boolean acceptance is compared.
fn raw_bytes_divergence(
    engine_name: &str,
    ours: Option<usize>,
    engine_accepts: bool,
    engine_count: usize,
    engine_count_reliable: bool,
) -> Option<String> {
    match (ours, engine_accepts) {
        (None, false) => None,
        (Some(_), false) => Some(format!("{engine_name}=reject, squonk=accept")),
        (None, true) => Some(format!("{engine_name}=accept, squonk=reject")),
        (Some(our_count), true) => {
            (engine_count_reliable && our_count != engine_count).then(|| {
                format!(
                    "segmentation: both accept, {engine_name}={engine_count} statement(s), \
                 squonk={our_count}"
                )
            })
        }
    }
}

/// Statements both engines and both paired dialects accept without any schema.
/// Comparable against a bare in-memory database ([`SqliteOracle::new`]).
pub const SCHEMA_INDEPENDENT_ACCEPT: &[&str] = &[
    "SELECT 1",
    "SELECT 1 + 2 * 3",
    "SELECT (1 + 2) * 3",
    "SELECT 1, 2, 3",
    "SELECT 'hello'",
    "SELECT 'it''s'",
    "SELECT 1 AS x",
    "SELECT abs(-5)",
    "SELECT length('abc')",
    "SELECT 1 UNION SELECT 2",
    "SELECT 1 UNION ALL SELECT 2",
    "SELECT 1 EXCEPT SELECT 2",
    "SELECT 1 INTERSECT SELECT 2",
    "VALUES (1), (2), (3)",
    "SELECT CASE WHEN 1 > 0 THEN 'a' ELSE 'b' END",
    "SELECT 1 = 1",
    "SELECT NOT (1 = 1)",
    "SELECT 3 / 2",
    "SELECT 5 % 2",
    "SELECT 'a' || 'b'",
    "SELECT count(*) FROM (SELECT 1) AS s",
    "SELECT min(x), max(x) FROM (SELECT 1 AS x UNION SELECT 2) AS s",
    "SELECT 1 WHERE 1 = 1",
    "SELECT DISTINCT 1",
    "SELECT 1 ORDER BY 1",
    "SELECT -1",
    "SELECT 1.5",
    "SELECT 1 IN (1, 2, 3)",
    "SELECT 1 BETWEEN 0 AND 2",
    "SELECT coalesce(NULL, 1)",
    "SELECT cast('1' AS INTEGER)",
    "SELECT nullif(1, 2)",
];

/// Statements both engines and both paired dialects reject — syntactic errors, so the
/// reject is a genuine parse failure (not a `PrepareBind` name-resolution reject).
///
/// The bare empty-target-list `SELECT` is included again: it was excluded while DuckDB
/// paired with the `Postgres` stopgap (whose raw grammar accepts a bare `SELECT`,
/// matching libpg_query — the `parse-pg-table-command-and-empty-select` change), which
/// made it a non-universal reject. The fitted `DuckDb` preset
/// (`duckdb-featureset-preset`) rejects empty-target `SELECT` again, so it is once more a
/// universal reject shared by every pairing here — the closure that comment anticipated.
pub const SCHEMA_INDEPENDENT_REJECT: &[&str] = &[
    "SELECT",
    "SELECT FROM",
    "123 456",
    "SELECT 1 +",
    "SELECT * FROM",
    "SELCT 1",
    "SELECT 1 WHERE",
    "INSERT INTO",
    "SELECT ()",
    "FROM",
    // Cross-dialect VACUUM hybrids: SQLite rejects the DuckDB-shaped prefix at the
    // grammar (`near "t"` / `near "("` syntax errors) and DuckDB rejects every SQLite
    // `INTO` tail with a parser error, so mixing the two operand grammars is a
    // universal syntactic reject — the boundary the Lenient union preserves.
    "VACUUM ANALYZE t (a) INTO 'f'",
    "VACUUM t (a) INTO 'f'",
];

/// Statements both engines and both paired dialects accept **after** [`SCHEMA_SETUP_SQL`]
/// is provisioned. Every referenced name exists in that schema, so the engine binds
/// cleanly and its accept matches ours — the setup driver is what makes this parity
/// real rather than a false divergence.
pub const SCHEMA_DEPENDENT_ACCEPT: &[&str] = &[
    "SELECT a FROM t1",
    "SELECT a, b, c, d, e FROM t1",
    "SELECT a + b FROM t1 WHERE c > 0",
    "SELECT * FROM t1",
    "SELECT count(*) FROM t1",
    "SELECT a FROM t1 ORDER BY a",
    "SELECT a FROM t1 GROUP BY a HAVING count(*) > 1",
    "SELECT a FROM t1 LIMIT 5",
    "SELECT a FROM t1 WHERE a IN (SELECT f FROM t2)",
    "SELECT t1.a, t2.g FROM t1 JOIN t2 ON t1.a = t2.f",
    "SELECT a FROM t1 UNION SELECT f FROM t2",
    "SELECT max(a) FROM t1",
    "SELECT g FROM t2 WHERE g LIKE 'a%'",
    "SELECT DISTINCT a FROM t1",
    "SELECT a FROM t1 WHERE a IS NOT NULL",
];

/// SQLite syntax the engine accepts but our fitted
/// [`Sqlite`](squonk::dialect::Sqlite) preset does not yet parse — *grammar gaps*,
/// not accept/reject divergences. Reported separately (ADR-0015): closing a gap is
/// future dialect coverage, whereas a divergence would be a bug. The tests assert each
/// is still a gap, so newly-added support fails loudly and prompts triaging the case in
/// the SQLite feature-probe sweep (`corpus_sqlite_verdicts`, a test-only module).
///
/// The FeatureSet-expressible families the sweep proved needed — `==`, `GLOB`, the
/// `LIMIT <offset>, <count>` comma form — now parse under the fitted preset and have
/// left this list (`sqlite-featureset-preset`), as has the `PRAGMA` statement
/// (`sqlite-pragma-attach-statements` built the canonical `Statement::Pragma` /
/// `Attach` / `Detach` nodes; its sweep coverage lives in `corpus_sqlite_verdicts`,
/// and it stays out of [`SCHEMA_INDEPENDENT_ACCEPT`] because that list's contract is
/// *both* engines accept — DuckDB bind-rejects an unknown pragma). The bitwise operators
/// left this list too (`bitwise-operators-cross-dialect-gap`,
/// `OperatorSyntax::bitwise_operators`). The `?NNN` numbered parameters landed last
/// (`sqlite-lexer-under-acceptance-bundle`, `ParameterSyntax::numbered_question`), so the
/// list is now empty — every schema-independent SQLite family the sweep surfaced parses.
pub const SQLITE_GRAMMAR_GAPS: &[&str] = &[];

/// DuckDB syntax the engine accepts but our fitted
/// [`DuckDb`](squonk::dialect::DuckDb) pairing does not yet parse — *grammar gaps*,
/// as [`SQLITE_GRAMMAR_GAPS`]. The fitted preset closed the FeatureSet-expressible forms
/// (the `0x`/`0o`/`0b` radix integers and `_` digit separators) and
/// `duckdb-collection-literals` closed the `[…]` list / `{…}` struct literals, so those
/// have left this list — the literals could not move into the *shared*
/// [`SCHEMA_INDEPENDENT_ACCEPT`] corpus because SQLite reads `[1, 2, 3]` as a
/// bracket-quoted identifier (its pairing would binding-reject it), so their accept
/// coverage rides the DuckDB-only lanes (`corpus_duckdb_verdicts`, the structural
/// goldens, and the parser/coverage tests). The `//` integer-division operator
/// (`duckdb-operator-and-literal-gaps` added the `//` and `==` symbol spellings as
/// tokenizer/parser gates folding onto the canonical `IntegerDivide`/`Eq` operators) is
/// DuckDB-accepted, so its coverage rides the DuckDB-only lanes and this list is empty.
pub const DUCKDB_GRAMMAR_GAPS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duckdb_nonascii_strip_honours_carriage_return_comment_termination() {
        assert_eq!(strip_bare_context_nonascii("--\r𑌬"), "--\r");
        assert_eq!(strip_bare_context_nonascii("--\n𑌬"), "--\n");
        assert_eq!(strip_bare_context_nonascii("--𑌬"), "--𑌬");
        assert_eq!(strip_bare_context_nonascii("/*;/**/\";*/ؚ"), "/*;/**/\";*/",);
    }
    use crate::oracle::accept_reject_divergence;
    use crate::pg::PgQueryOracle;
    use squonk::dialect::{DuckDb, Postgres, Sqlite};
    use squonk::parse_with;

    struct TableDecorationProbe {
        name: &'static str,
        sql: &'static str,
        sqlite: bool,
        postgres: bool,
        duckdb: bool,
        mysql: bool,
    }

    const TABLE_DECORATION_PROBES: &[TableDecorationProbe] = &[
        TableDecorationProbe {
            name: "typeless columns",
            sql: "CREATE TABLE t (a, b)",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "named column collate",
            sql: "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "column conflict clause",
            sql: "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "inline primary-key sort order",
            sql: "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "joined autoincrement",
            sql: "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "without rowid",
            sql: "CREATE TABLE t (a INTEGER PRIMARY KEY) WITHOUT ROWID",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
        TableDecorationProbe {
            name: "strict table",
            sql: "CREATE TABLE t (a INTEGER) STRICT",
            sqlite: true,
            postgres: false,
            duckdb: false,
            mysql: false,
        },
    ];

    #[test]
    fn oracle_semantics_are_prepare_bind() {
        // The declared semantics govern which corpus is comparable (module contract),
        // so they are part of the interface, asserted explicitly.
        assert_eq!(
            SqliteOracle::new().unwrap().semantics(),
            OracleSemantics::PrepareBind
        );
        assert_eq!(
            DuckDbOracle::new().unwrap().semantics(),
            OracleSemantics::PrepareBind
        );
        assert_eq!(SqliteOracle::new().unwrap().name(), "sqlite");
        assert_eq!(DuckDbOracle::new().unwrap().name(), "duckdb");
    }

    #[test]
    fn table_decoration_probes_match_oracle_boundary() {
        let sqlite = SqliteOracle::new().unwrap();
        eprintln!("sqlite runtime {}", rusqlite::version());
        for probe in TABLE_DECORATION_PROBES {
            assert_eq!(
                sqlite.verdict(probe.sql).unwrap().accepts(),
                probe.sqlite,
                "SQLite oracle boundary changed for {}: {:?}",
                probe.name,
                probe.sql,
            );
            assert_eq!(
                PgQueryOracle.verdict(probe.sql).unwrap().accepts(),
                probe.postgres,
                "PostgreSQL oracle boundary changed for {}: {:?}",
                probe.name,
                probe.sql,
            );
            eprintln!(
                "{} | sqlite={} postgres={} | {}",
                probe.name, probe.sqlite, probe.postgres, probe.sql
            );
        }

        let duckdb = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping DuckDB table-decoration boundary: {reason}");
                return;
            }
        };
        for probe in TABLE_DECORATION_PROBES {
            assert_eq!(
                duckdb.verdict(probe.sql).unwrap().accepts(),
                probe.duckdb,
                "DuckDB oracle boundary changed for {}: {:?}",
                probe.name,
                probe.sql,
            );
            eprintln!("{} | duckdb={}", probe.name, probe.duckdb);
        }
    }

    #[cfg(feature = "oracle-mysql")]
    #[test]
    fn table_decoration_probes_match_mysql_oracle_boundary() {
        let mysql = match crate::m3::MySqlOracle::with_schema(crate::m3::MYSQL_SCHEMA_SETUP_SQL) {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping MySQL table-decoration boundary: {reason}");
                return;
            }
        };
        for probe in TABLE_DECORATION_PROBES {
            assert_eq!(
                mysql.verdict(probe.sql).unwrap().accepts(),
                probe.mysql,
                "MySQL oracle boundary changed for {}: {:?}",
                probe.name,
                probe.sql,
            );
            eprintln!("{} | mysql={}", probe.name, probe.mysql);
        }
    }

    #[test]
    fn sqlite_accept_reject_parity_over_curated_corpus() {
        let bare = SqliteOracle::new().unwrap();
        // Machine-readable "oracle actually ran" evidence the nightly workflow greps for
        // (oracle-nightly.yml); emitted only on the ran path, so its absence trips the guard.
        eprintln!("oracle-ran: sqlite (rusqlite {})", rusqlite::version());
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_INDEPENDENT_REJECT)
        {
            assert_eq!(
                accept_reject_divergence(sql, Sqlite, &bare),
                None,
                "sqlite/Sqlite accept-reject divergence on schema-independent {sql:?}",
            );
        }
        let provisioned = SqliteOracle::with_schema(SCHEMA_SETUP_SQL).unwrap();
        for sql in SCHEMA_DEPENDENT_ACCEPT {
            assert_eq!(
                accept_reject_divergence(sql, Sqlite, &provisioned),
                None,
                "sqlite/Sqlite accept-reject divergence on schema-dependent {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_accept_reject_parity_over_curated_corpus() {
        let bare = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            // An unreachable engine is an infrastructure skip, never a failure.
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb differential: {reason}");
                return;
            }
        };
        // Machine-readable "oracle actually ran" evidence the nightly workflow greps for
        // (oracle-nightly.yml); emitted only on the ran path, so its absence trips the guard.
        eprintln!("oracle-ran: duckdb (libduckdb {})", bare.version());
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_INDEPENDENT_REJECT)
        {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &bare),
                None,
                "duckdb/DuckDb accept-reject divergence on schema-independent {sql:?}",
            );
        }
        let provisioned = DuckDbOracle::with_schema(SCHEMA_SETUP_SQL).unwrap();
        for sql in SCHEMA_DEPENDENT_ACCEPT {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &provisioned),
                None,
                "duckdb/DuckDb accept-reject divergence on schema-dependent {sql:?}",
            );
        }
    }

    /// DuckDB `COPY … TO/FROM <file> (<options>)` file-format/option clauses parse
    /// under our `DuckDb` preset in accept-parity with the real DuckDB parser
    /// (`planner-parity-copy-file-format-options`). Covers the value shapes beyond the
    /// bareword/string forms — `FORMAT PARQUET`, string (`COMPRESSION`), numeric
    /// (`ROW_GROUP_SIZE`), the `*` and parenthesized-list arguments (`FORCE_QUOTE (…)`,
    /// `PARTITION_BY (…)`) — that the option-value axis now carries as typed data.
    #[test]
    fn duckdb_copy_option_clauses_parse_in_accept_parity() {
        let oracle = match DuckDbOracle::with_schema(
            "CREATE TABLE t (a INTEGER, b INTEGER, year INTEGER, month INTEGER)",
        ) {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb copy differential: {reason}");
                return;
            }
        };
        for sql in [
            "COPY t TO 'f.parquet' (FORMAT PARQUET)",
            "COPY t TO 'f.parquet' (FORMAT PARQUET, COMPRESSION 'zstd')",
            "COPY t TO 'f.csv' (FORMAT csv, HEADER)",
            "COPY t TO 'f.csv' (FORMAT csv, HEADER 1)",
            "COPY t TO 'f.csv' (FORMAT csv, HEADER true)",
            "COPY t TO 'f.parquet' (FORMAT PARQUET, ROW_GROUP_SIZE 100000)",
            "COPY t TO 'f.csv' (FORMAT csv, FORCE_QUOTE (a, b))",
            "COPY t TO 'f.csv' (FORMAT csv, FORCE_QUOTE *)",
            "COPY t TO 'out' (FORMAT csv, PARTITION_BY (year, month))",
        ] {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &oracle),
                None,
                "duckdb/DuckDb accept-reject divergence on COPY option clause {sql:?}",
            );
        }
    }

    /// DuckDB's parenthesized `VACUUM (<options>)` option list parses under our `DuckDb`
    /// preset in accept/reject-parity with the real 1.5.4 engine
    /// (`duckdb-vacuum-paren-option-list`). Measured on the live oracle, `ANALYZE` is the
    /// sole option either layer admits: the accept forms prepare (a real table lets the
    /// operand-bearing forms bind), while every other spelling rejects — `FULL`/`FREEZE`/
    /// `VERBOSE`/`disable_page_skipping` at the transform (`NotImplementedException`), and
    /// `NOWAIT`/`SKIP_TOAST`/an unknown option/the boolean-argument form/a mixed or empty
    /// list/an `INTO` tail at the parser. The keyword form (`VACUUM ANALYZE`) rides
    /// `duckdb_vacuum_analyze_parse_and_round_trip`; this pins the paren surface against the
    /// engine (the reject forms are `duckdb=reject, squonk=reject` parity, not divergence).
    #[test]
    fn duckdb_vacuum_paren_option_list_in_accept_parity() {
        let oracle = match DuckDbOracle::with_schema("CREATE TABLE t (a INTEGER, b INTEGER)") {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb vacuum-paren differential: {reason}");
                return;
            }
        };
        // Accept-parity: only `ANALYZE` (case-insensitive, repeatable), optionally with a
        // provisioned table and column list.
        for sql in [
            "VACUUM (ANALYZE)",
            "VACUUM (analyze)",
            "VACUUM (ANALYZE, ANALYZE)",
            "VACUUM (ANALYZE) t",
            "VACUUM (ANALYZE) t (a)",
            "VACUUM (ANALYZE) t (a, b)",
        ] {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &oracle),
                None,
                "duckdb/DuckDb accept divergence on VACUUM paren option list {sql:?}",
            );
        }
        // Reject-parity: every non-`ANALYZE` option, the boolean-argument form, a mixed or
        // empty list, and an `INTO` tail after a paren prefix — all `duckdb=reject`.
        for sql in [
            "VACUUM (FULL)",
            "VACUUM (FREEZE)",
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
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &oracle),
                None,
                "duckdb/DuckDb reject divergence on VACUUM paren option list {sql:?}",
            );
        }
    }

    /// Every DuckDB `ALTER`-family form this ticket lands parse-accepts under the fitted
    /// preset and render-round-trips (always-on, no oracle needed):
    /// `ALTER DATABASE … SET ALIAS TO`, the `ALTER SEQUENCE …` option list (reusing the
    /// `CREATE SEQUENCE` `IdentityOption` core plus the ALTER-only `RESTART`/`AS`/`OWNED BY`
    /// leads), and `ALTER {TABLE|VIEW|SEQUENCE} … SET SCHEMA`
    /// (`parse-duckdb-alter-statements`).
    #[test]
    fn duckdb_alter_family_parses_and_round_trips() {
        let forms = [
            "ALTER DATABASE test_db SET ALIAS TO renamed_db",
            "ALTER DATABASE IF EXISTS non_existent SET ALIAS TO something_else",
            "ALTER SEQUENCE IF EXISTS seq OWNED BY x",
            "ALTER SEQUENCE s RESTART",
            "ALTER SEQUENCE s RESTART WITH 10",
            "ALTER SEQUENCE s RESTART 5",
            "ALTER SEQUENCE s START WITH 5 INCREMENT BY 2 MAXVALUE 100 CYCLE",
            "ALTER SEQUENCE s NO MAXVALUE",
            "ALTER SEQUENCE s CACHE 20",
            "ALTER SEQUENCE s AS SMALLINT",
            "ALTER SEQUENCE s OWNED BY NONE",
            "ALTER SEQUENCE s OWNED BY tbl.col",
            "ALTER TABLE t SET SCHEMA s",
            "ALTER VIEW v SET SCHEMA s",
            "ALTER SEQUENCE seq SET SCHEMA s",
        ];
        for sql in forms {
            assert!(
                parse_with(sql, squonk::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDb preset must parse-accept {sql:?}",
            );
        }
        // A bare `ALTER SEQUENCE s` (no option) is a syntax error, as DuckDB's `SeqOptList`
        // is one-or-more; `SET ALIAS TO` is the only `ALTER DATABASE` form (a `RENAME TO`
        // or a `SET FOO TO` is a syntax error, engine-measured).
        for sql in [
            "ALTER SEQUENCE s",
            "ALTER DATABASE d RENAME TO e",
            "ALTER DATABASE d SET FOO TO e",
        ] {
            assert!(
                parse_with(sql, squonk::ParseConfig::new(DuckDb)).is_err(),
                "DuckDb preset must reject {sql:?}",
            );
        }
        crate::corpus_roundtrip::assert_accepted_lines_round_trip(&forms.join("\n"), DuckDb);
    }

    /// The `ALTER`-family forms DuckDB's `PrepareBind` binder actually accepts stay in
    /// accept-parity with the real engine (`parse-duckdb-alter-statements`). Only the
    /// binder-reachable forms are testable this way — `ALTER … SET SCHEMA` is
    /// binder-unimplemented in 1.5.4 (its parse-reach is proven via `json_serialize_sql` in
    /// `corpus_duckdb_verdicts`), and every non-`OWNED BY` sequence option binds
    /// "option not supported yet".
    #[test]
    fn duckdb_alter_binder_reachable_forms_in_accept_parity() {
        let oracle = match DuckDbOracle::with_schema("ATTACH ':memory:' AS test_db") {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb alter-family differential: {reason}");
                return;
            }
        };
        for sql in [
            "ALTER DATABASE test_db SET ALIAS TO renamed_db",
            "ALTER DATABASE IF EXISTS non_existent SET ALIAS TO something_else",
            "ALTER SEQUENCE IF EXISTS seq OWNED BY x",
        ] {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &oracle),
                None,
                "duckdb/DuckDb accept-reject divergence on ALTER form {sql:?}",
            );
        }
    }

    #[test]
    fn setup_driver_prevents_false_divergence() {
        // The load-bearing reason the seam carries `semantics()`: a `PrepareBind` oracle
        // over schema-dependent SQL against an *empty* database yields a false
        // divergence (we accept, the engine rejects "no such table"); provisioning the
        // schema first removes it. Demonstrated for both engines.
        let probe = "SELECT a FROM t1";

        let bare_sqlite = SqliteOracle::new().unwrap();
        assert!(
            accept_reject_divergence(probe, Sqlite, &bare_sqlite).is_some(),
            "unprovisioned sqlite must falsely diverge on {probe:?}",
        );
        let provisioned_sqlite = SqliteOracle::with_schema(SCHEMA_SETUP_SQL).unwrap();
        assert!(
            accept_reject_divergence(probe, Sqlite, &provisioned_sqlite).is_none(),
            "provisioned sqlite must agree on {probe:?}",
        );

        if let Ok(bare_duck) = DuckDbOracle::new() {
            assert!(
                accept_reject_divergence(probe, DuckDb, &bare_duck).is_some(),
                "unprovisioned duckdb must falsely diverge on {probe:?}",
            );
            let provisioned_duck = DuckDbOracle::with_schema(SCHEMA_SETUP_SQL).unwrap();
            assert!(
                accept_reject_divergence(probe, DuckDb, &provisioned_duck).is_none(),
                "provisioned duckdb must agree on {probe:?}",
            );
        }
    }

    #[test]
    fn oracles_never_execute() {
        // The verdict must `prepare` without executing: preparing an `INSERT` leaves the
        // provisioned table empty. Proven directly for SQLite (results are readable
        // without the stripped DuckDB result-handling tree); the DuckDB `verdict` takes
        // the identical `prepare`-only path, and `corpus_is_single_statement` rules out
        // the one DuckDB case (multi-statement `prepare`) that would execute.
        let oracle = SqliteOracle::with_schema(SCHEMA_SETUP_SQL).unwrap();
        assert!(
            oracle
                .verdict("INSERT INTO t1 VALUES (1, 2, 3, 4, 5)")
                .unwrap()
                .accepts(),
            "the INSERT must prepare (accept)"
        );
        let rows: i64 = oracle
            .conn
            .query_row("SELECT count(*) FROM t1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0, "preparing the INSERT must not have executed it");
    }

    #[test]
    fn corpus_is_single_statement() {
        // DuckDB's `prepare` executes all but the last statement of a multi-statement
        // string, so a corpus entry containing a top-level `;` would silently execute
        // and break the never-execute guarantee. None of ours do. (`SCHEMA_SETUP_SQL`
        // is intentionally multi-statement — it is *executed* by the setup driver, not
        // handed to `verdict` — so it is excluded here.)
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_INDEPENDENT_REJECT)
            .chain(SCHEMA_DEPENDENT_ACCEPT)
            .chain(SQLITE_GRAMMAR_GAPS)
            .chain(DUCKDB_GRAMMAR_GAPS)
        {
            assert!(
                !sql.contains(';'),
                "corpus entries must be single statements: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_extract_counts_are_parse_only_and_never_execute() {
        let conn = match DuckDbConnection::open_in_memory() {
            Ok(conn) => conn,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb extract-count probe: {reason}");
                return;
            }
        };
        // Segmentation: `extract_statements` splits on `;` at the parser, never
        // executing (the never-execute basis for the DuckDB raw-byte differential).
        assert_eq!(conn.extract_statement_count("SELECT 1"), Ok(1));
        assert_eq!(conn.extract_statement_count("SELECT 1; SELECT 2"), Ok(2));
        assert_eq!(
            conn.extract_statement_count("VALUES (1); VALUES (2); VALUES (3)"),
            Ok(3)
        );
        // Empty / whitespace / comment-only inputs read as zero statements (accept),
        // matching our parser — NOT a parse error.
        assert_eq!(conn.extract_statement_count(""), Ok(0));
        assert_eq!(conn.extract_statement_count("   "), Ok(0));
        assert_eq!(conn.extract_statement_count("-- just a comment"), Ok(0));
        // Parse-only: an unresolved object name still parses (no binding), the property
        // that makes extraction the right comparison basis for our parse-only parser.
        assert_eq!(
            conn.extract_statement_count("SELECT * FROM nonexistent_table"),
            Ok(1)
        );
        assert_eq!(
            conn.extract_statement_count("SELECT unknown_function(1)"),
            Ok(1)
        );
        // A genuine syntax error rejects.
        assert!(conn.extract_statement_count("SELCT 1").is_err());
        assert!(conn.extract_statement_count("SELECT [1, 2").is_err());
        // Never-execute proof (direct side-effect check, the DuckDB analogue of
        // `oracles_never_execute`): extracting a CREATE TABLE must not create it.
        assert_eq!(
            conn.extract_statement_count("CREATE TABLE probe_never_created (a INTEGER)"),
            Ok(1)
        );
        let present = conn
            .query_string(
                "SELECT count(*) FROM information_schema.tables \
                 WHERE table_name = 'probe_never_created'",
            )
            .unwrap();
        assert_eq!(
            present, "0",
            "extract_statement_count must parse the CREATE without executing it",
        );
    }

    #[test]
    fn sqlite_segment_counts_are_parse_only_and_never_execute() {
        use crate::sqlite_ffi::{SqliteSegmentation, segment};
        let conn = SqliteConnection::open_in_memory().unwrap();

        let accept = |count: usize| SqliteSegmentation::Accept {
            count,
            count_reliable: true,
        };

        // Segmentation via `pzTail`, statements finalized un-stepped (never execute).
        assert_eq!(segment(&conn, "SELECT 1"), accept(1));
        assert_eq!(segment(&conn, "SELECT 1; SELECT 2"), accept(2));
        assert_eq!(segment(&conn, "SELECT 1; VALUES (2); SELECT 3"), accept(3));
        // Empty statements (a bare `;`, trailing/leading) are skipped, not counted —
        // matching our parser's statement count.
        assert_eq!(segment(&conn, ""), accept(0));
        assert_eq!(segment(&conn, "   "), accept(0));
        assert_eq!(segment(&conn, "SELECT 1;"), accept(1));
        assert_eq!(segment(&conn, "SELECT 1;; SELECT 2"), accept(2));
        assert_eq!(segment(&conn, ";; SELECT 1"), accept(1));
        // A genuine syntax error rejects.
        assert!(matches!(
            segment(&conn, "SELCT 1"),
            SqliteSegmentation::Reject(_)
        ));
        assert!(matches!(
            segment(&conn, "SELECT 1 SELECT 2"),
            SqliteSegmentation::Reject(_)
        ));
        // A name-resolution failure reads as a parse accept (proof the statement
        // parsed), with the count marked unreliable because `pzTail` cannot advance
        // past the failed prepare — so the differential compares only the boolean.
        assert_eq!(
            segment(&conn, "SELECT * FROM nonexistent_table"),
            SqliteSegmentation::Accept {
                count: 0,
                count_reliable: false,
            }
        );

        // Never-execute proof: provision a table, then `segment` an INSERT — preparing
        // it un-stepped must leave the table empty (the SQLite analogue of
        // `oracles_never_execute`).
        conn.execute_batch("CREATE TABLE probe (a INTEGER)")
            .unwrap();
        assert_eq!(segment(&conn, "INSERT INTO probe VALUES (1)"), accept(1));
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM probe", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            rows, 0,
            "segment() must prepare without executing the INSERT"
        );
    }

    #[test]
    fn raw_bytes_divergence_compares_accept_then_segmentation() {
        // The shared comparator logic, engine-free: both-reject agrees; a boolean
        // disagreement diverges either way; both-accept diverges only on a reliable
        // count mismatch (an unreliable count is not compared).
        assert_eq!(raw_bytes_divergence("e", None, false, 0, false), None);
        assert!(raw_bytes_divergence("e", Some(1), false, 0, false).is_some());
        assert!(raw_bytes_divergence("e", None, true, 1, true).is_some());
        assert_eq!(raw_bytes_divergence("e", Some(2), true, 2, true), None);
        assert!(raw_bytes_divergence("e", Some(1), true, 2, true).is_some());
        // Unreliable engine count: both accept, count mismatch tolerated (SQLite
        // resolution-truncation path).
        assert_eq!(raw_bytes_divergence("e", Some(1), true, 2, false), None);
    }

    #[test]
    fn sqlite_syntax_error_text_cannot_match_a_resolution_stem() {
        let conn = SqliteConnection::open_in_memory().unwrap();
        for sql in ["\"the same\"", "`term out of range%"] {
            assert!(matches!(
                crate::sqlite_ffi::segment(&conn, sql),
                SqliteSegmentation::Reject(_),
            ));
            assert_eq!(
                sqlite_raw_bytes_divergence(&conn, sql),
                None,
                "both SQLite and the fitted dialect reject {sql:?}",
            );
        }
    }

    #[test]
    fn sqlite_grammar_gaps_are_gaps_not_divergences() {
        // Reported separately from accept/reject divergences: the engine accepts these
        // and our fitted Sqlite pairing rejects them (unimplemented dialect grammar). If
        // our parser gains the syntax this fails, prompting a move into the accept corpus.
        let oracle = SqliteOracle::new().unwrap();
        for sql in SQLITE_GRAMMAR_GAPS {
            assert!(
                oracle.verdict(sql).unwrap().accepts(),
                "sqlite should accept the grammar-gap case {sql:?}",
            );
            assert!(
                parse_with(sql, squonk::ParseConfig::new(Sqlite)).is_err(),
                "{sql:?} now parses under the fitted Sqlite preset; triage it in corpus_sqlite_verdicts",
            );
        }
    }

    #[test]
    fn duckdb_grammar_gaps_are_gaps_not_divergences() {
        let oracle = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping duckdb grammar-gap check: {reason}");
                return;
            }
        };
        for sql in DUCKDB_GRAMMAR_GAPS {
            assert!(
                oracle.verdict(sql).unwrap().accepts(),
                "duckdb should accept the grammar-gap case {sql:?}",
            );
            assert!(
                parse_with(sql, squonk::ParseConfig::new(DuckDb)).is_err(),
                "{sql:?} now parses under the fitted DuckDb preset; move it into SCHEMA_INDEPENDENT_ACCEPT",
            );
        }
    }

    #[test]
    fn divergence_allowlist_entries_still_diverge_and_are_ticketed() {
        // Mirrors `pg::PG_DIVERGENCE_ALLOWLIST`: every allowlisted divergence must name a
        // real ticket and still actually diverge, so a fixed gap cannot stay silently
        // allowlisted. Vacuously holds while the allowlist is empty, but keeps the
        // machinery in place for the first real entry.
        for entry in M2_DIVERGENCE_ALLOWLIST {
            assert!(
                !entry.ticket.trim().is_empty(),
                "allowlisted divergence needs a provenance label: {} ({})",
                entry.ticket,
                entry.reason,
            );
            let divergence = match entry.engine {
                "sqlite" => {
                    accept_reject_divergence(entry.sql, Sqlite, &SqliteOracle::new().unwrap())
                }
                "duckdb" => match DuckDbOracle::new() {
                    Ok(oracle) => accept_reject_divergence(entry.sql, DuckDb, &oracle),
                    Err(_) => continue,
                },
                other => panic!("unknown allowlist engine {other:?}"),
            };
            assert!(
                divergence.is_some(),
                "allowlisted case no longer diverges: the engine and our dialect now agree, \
                 so the divergence is fixed — SWEEP this entry (delete it from M2_DIVERGENCE_ALLOWLIST), \
                 never re-pin or edit it to keep it allowlisted (ADR-0015: a fix forces removal): {:?}",
                entry.sql,
            );
        }
    }

    #[test]
    fn oracle_rows_match_coverage_matrix() {
        // Weld the default-build M2 oracle matrix (coverage::M2_ORACLE_ROWS, rendered
        // engine-free into the dialect-matrix snapshot) to the real oracle impls, so the
        // planning artifact cannot drift from the engines it describes.
        for row in crate::coverage::M2_ORACLE_ROWS {
            let (name, semantics) = match row.engine {
                "sqlite" => {
                    let oracle = SqliteOracle::new().unwrap();
                    (oracle.name(), oracle.semantics())
                }
                "duckdb" => match DuckDbOracle::new() {
                    Ok(oracle) => {
                        // `semantics()` does not borrow the connection, so read it before
                        // moving on; `name()` is a static string.
                        let semantics = oracle.semantics();
                        let name = oracle.name();
                        (name, semantics)
                    }
                    Err(_) => continue,
                },
                other => panic!("unknown matrix engine {other:?}"),
            };
            assert_eq!(name, row.engine, "matrix engine name mismatch");
            assert_eq!(
                semantics,
                OracleSemantics::PrepareBind,
                "matrix declares {} as {}, but the oracle is not PrepareBind",
                row.engine,
                row.semantics,
            );
            assert_eq!(
                row.semantics, "prepare_bind",
                "matrix semantics id must be prepare_bind for {}",
                row.engine,
            );
        }
    }

    /// The SQL-standard `(s1, e1) OVERLAPS (s2, e2)` period predicate — a PostgreSQL-only
    /// accept among the shipped engines (DuckDB, SQLite, and MySQL reject the syntax
    /// outright). The accept rows are the two operand spellings PostgreSQL admits (a bare
    /// parenthesized pair and explicit `ROW(...)`); the reject rows pin the operand-shape
    /// and non-chaining rules PostgreSQL enforces at parse (scalar operands, a
    /// single-element grouping, a wrong-arity or re-parenthesized row, and a chain).
    /// Engine-probed pg_query 6.1 / PG-17, DuckDB 1.5.4, SQLite bundled.
    const OVERLAPS_PROBES: &[&str] = &[
        "SELECT (a, b) OVERLAPS (c, d)",
        "SELECT ROW(a, b) OVERLAPS ROW(c, d)",
        "SELECT a OVERLAPS b",
        "SELECT (a) OVERLAPS (b)",
        "SELECT (a, b, c) OVERLAPS (d, e, f)",
        "SELECT ((a, b)) OVERLAPS (c, d)",
        "SELECT (a, b) OVERLAPS (c, d) OVERLAPS (e, f)",
    ];

    #[test]
    fn overlaps_period_predicate_matches_oracle_boundary() {
        // Both the default PostgreSQL preset (which enables the predicate) and a
        // non-PostgreSQL preset (SQLite, which does not) must track their oracle — so the
        // gate honours dialect data in both directions, not just where it is on.
        for sql in OVERLAPS_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, Postgres, &PgQueryOracle),
                None,
                "PostgreSQL parser/oracle divergence for {sql:?}",
            );
            assert_eq!(
                accept_reject_divergence(sql, Sqlite, &SqliteOracle::new().unwrap()),
                None,
                "SQLite parser/oracle divergence for {sql:?}",
            );
        }

        let duckdb = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping DuckDB OVERLAPS boundary: {reason}");
                return;
            }
        };
        for sql in OVERLAPS_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &duckdb),
                None,
                "DuckDB parser/oracle divergence for {sql:?}",
            );
        }
    }

    #[cfg(feature = "oracle-mysql")]
    #[test]
    fn overlaps_period_predicate_matches_mysql_oracle_boundary() {
        use squonk::dialect::MySql;
        let mysql = match crate::m3::MySqlOracle::with_schema(crate::m3::MYSQL_SCHEMA_SETUP_SQL) {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping MySQL OVERLAPS boundary: {reason}");
                return;
            }
        };
        for sql in OVERLAPS_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, MySql, &mysql),
                None,
                "MySQL parser/oracle divergence for {sql:?}",
            );
        }
    }

    /// Line comments end at a bare `\r` (carriage return) in PostgreSQL and DuckDB — their
    /// flex scanner's comment body is `[^\n\r]*` — but only at `\n` in SQLite and MySQL,
    /// which read a `\r` as ordinary comment content
    /// (tokenizer-line-comment-terminator-set, a measured dialect-data divergence carried
    /// by `CommentSyntax::line_comment_ends_at_carriage_return`). Each probe places a
    /// candidate byte between a `-- c` comment and a trailing `FROM`: if the byte ends the
    /// comment, `FROM` is live and `SELECT 1 FROM` is a syntax error (reject); otherwise the
    /// tail is trivia and `SELECT 1` accepts. Our fitted presets carry the flag per engine,
    /// so each parser now agrees with its oracle across the whole set — a wrong flag would
    /// surface here as an accept/reject divergence. `\n` ends the comment everywhere;
    /// `\x0b`/`\x0c`/space never do. Engine-probed pg_query 6.1, DuckDB 1.5.4, SQLite
    /// bundled, mysql:8.
    const CR_TERMINATOR_PROBES: &[&str] = &[
        "SELECT 1 -- c\rFROM", // CR: pg/duckdb end it (reject); sqlite/mysql do not (accept)
        "SELECT 1 -- c\nFROM", // LF: every engine ends it (reject)
        "SELECT 1 -- c\r\nFROM", // CRLF: every engine ends it at the `\n` (reject)
        "SELECT 1 -- c\x0bFROM", // vertical tab: no engine ends it (accept)
        "SELECT 1 -- c\x0cFROM", // form feed: no engine ends it (accept)
        "SELECT 1 -- c FROM",  // a plain space never ends a comment (accept) — a control
    ];

    #[test]
    fn line_comment_carriage_return_terminator_matches_oracle_boundary() {
        // Both a CR-terminating preset (PostgreSQL) and a `\n`-only one (SQLite) must track
        // their oracle, so the dialect data is honoured in both directions.
        for sql in CR_TERMINATOR_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, Postgres, &PgQueryOracle),
                None,
                "PostgreSQL parser/oracle divergence for {sql:?}",
            );
            assert_eq!(
                accept_reject_divergence(sql, Sqlite, &SqliteOracle::new().unwrap()),
                None,
                "SQLite parser/oracle divergence for {sql:?}",
            );
        }

        let duckdb = match DuckDbOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping DuckDB CR-terminator boundary: {reason}");
                return;
            }
        };
        for sql in CR_TERMINATOR_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, DuckDb, &duckdb),
                None,
                "DuckDB parser/oracle divergence for {sql:?}",
            );
        }
    }

    #[cfg(feature = "oracle-mysql")]
    #[test]
    fn line_comment_carriage_return_terminator_matches_mysql_oracle_boundary() {
        use squonk::dialect::MySql;
        // Schema-independent probes (`SELECT 1` / `SELECT 1 FROM`), so a bare connection is
        // enough — no database needs selecting.
        let mysql = match crate::m3::MySqlOracle::new() {
            Ok(oracle) => oracle,
            Err(OracleUnavailable(reason)) => {
                eprintln!("skipping MySQL CR-terminator boundary: {reason}");
                return;
            }
        };
        for sql in CR_TERMINATOR_PROBES {
            assert_eq!(
                accept_reject_divergence(sql, MySql, &mysql),
                None,
                "MySQL parser/oracle divergence for {sql:?}",
            );
        }
    }
}
