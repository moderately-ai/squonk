// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! M3 MySQL/MariaDB accept/reject differential oracle — an **external server**, no
//! daemon linked into this build.
//!
//! The M3 dialect milestone (ADR-0015, ADR-0017) extends the pluggable
//! [`AcceptRejectOracle`] seam with a real MySQL engine, paired with the
//! [`MySql`](squonk::dialect::MySql) dialect. It mirrors
//! the M2 engines (`m2` — not an intra-doc link, as it is behind the sibling
//! `oracle-engines` feature) in shape: same trait, same [`OracleUnavailable`] skip
//! semantics, same schema-independent/setup-driver corpus split, with one deliberate
//! structural difference below.
//!
//! # Licensing: the server is external, never linked or vendored
//!
//! MySQL/MariaDB are GPL, and the licensing policy is that GPL is never linked into the
//! build graph nor vendored into the repo — an external process at most. So instead of
//! linking `libmysqlclient` or vendoring MariaDB's `sql_yacc.yy`, a real
//! `mysqld`/`mariadbd` runs as a **separate process** (a CI service container, or a
//! local `brew` install) and we speak its wire protocol with the pure-Rust,
//! MIT/Apache-2.0 `mysql` crate. Nothing GPL enters the build graph. This is why
//! [`MySqlOracle`] connects over the network at construction and reports
//! [`OracleUnavailable`] (a skip) whenever no server is reachable — unlike the
//! in-process M2 engines, which are unavailable only if a bundled/linked library fails
//! to open.
//!
//! # Prepare-only, never execute
//!
//! [`verdict`](AcceptRejectOracle::verdict) calls only `Queryable::prep` — a server-side
//! `COM_STMT_PREPARE`, which parses and binds the statement but does **not** run it, so a
//! `SELECT`/`INSERT` under test has no side effect (`oracle_never_executes` proves the
//! provisioned table stays empty after preparing an `INSERT`). Executing corpus
//! statements is banned specifically because DDL auto-commits on a real server; the one
//! sanctioned exception is the setup driver, which executes its schema DDL and nothing
//! else (see below).
//!
//! # PrepareBind semantics and the setup driver
//!
//! [`OracleSemantics::PrepareBind`]: the server resolves names against the session
//! schema, so an unknown table/column *rejects*. Our parser does not bind, so comparing
//! over schema-dependent SQL (`SELECT a FROM t1`) against an empty server yields a
//! **false** divergence — we accept, the server rejects "no such table"
//! (`setup_driver_prevents_false_divergence` demonstrates it). The differential
//! therefore runs over two disjoint curated corpora:
//!
//! - [`SCHEMA_INDEPENDENT_ACCEPT`]/[`SCHEMA_INDEPENDENT_REJECT`] — no object names, so
//!   comparable against a bare connection ([`MySqlOracle::new`]).
//! - [`SCHEMA_DEPENDENT_ACCEPT`] — references [`MYSQL_SCHEMA_SETUP_SQL`], provisioned
//!   first via the setup driver ([`MySqlOracle::with_schema`]).
//!
//! # A REJECT verdict is any server error on `PREPARE`
//!
//! The wire client normalizes many failure kinds; classifying them would be brittle, so
//! any error returned by `prep` is a [`Reject`](OracleVerdict::Reject) verdict. Only a
//! failure to *connect* (at construction) is [`OracleUnavailable`] — the infrastructure
//! skip. A connection that drops mid-run therefore reads as a reject, which is
//! acceptable: the CI "oracle actually ran" guard catches a wholesale server outage.
//!
//! # Accept/reject only
//!
//! Structural parity (a neutral parse-tree shape) is intentionally out of scope — see
//! the [`oracle`](crate::oracle) module contract.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use mysql::prelude::Queryable;
use mysql::{Conn, Opts};

use crate::oracle::{AcceptRejectOracle, OracleSemantics, OracleUnavailable, OracleVerdict};

/// Environment variable naming the external server the oracle connects to.
pub const MYSQL_ORACLE_URL_ENV: &str = "MYSQL_ORACLE_URL";

/// Default server URL when [`MYSQL_ORACLE_URL_ENV`] is unset: a local `mysqld` with an
/// empty-password `root` (the `mysql:8` CI service and a default `brew` install both
/// match this), no database selected.
pub const DEFAULT_MYSQL_ORACLE_URL: &str = "mysql://root@127.0.0.1:3306";

/// The MySQL-compatible schema the setup driver provisions before the
/// [`SCHEMA_DEPENDENT_ACCEPT`] corpus is compared. A dedicated database is created and
/// selected first because the default URL selects none, and `CREATE TABLE` needs one;
/// `VARCHAR` carries an explicit length because MySQL rejects an unsized `VARCHAR`
/// (where SQLite/DuckDB accept it, so the M2 setup SQL cannot be reused verbatim).
/// `IF NOT EXISTS` keeps it idempotent across the auto-committed re-runs a real server
/// accumulates. Executed statement-by-statement by [`MySqlOracle::with_schema`]. `ft1`
/// carries a `FULLTEXT` index over its two `TEXT` columns so the `MATCH (…) AGAINST (…)`
/// accept corpus (mysql-match-against) binds — a `MATCH` over columns with no covering
/// full-text index is a *binding* reject (`ER_FT_MATCHING_KEY_NOT_FOUND`), not a grammar
/// one, so the accepts are schema-dependent.
pub const MYSQL_SCHEMA_SETUP_SQL: &str = "CREATE DATABASE IF NOT EXISTS squonk_oracle; \
     USE squonk_oracle; \
     CREATE TABLE IF NOT EXISTS t1(a INTEGER, b INTEGER, c INTEGER, d INTEGER, e INTEGER); \
     CREATE TABLE IF NOT EXISTS t2(f INTEGER, g VARCHAR(255)); \
     CREATE TABLE IF NOT EXISTS ft1(a TEXT, b TEXT, FULLTEXT ftidx(a, b))";

/// The external-server MySQL prepare-only accept/reject oracle, paired with the
/// [`MySql`](squonk::dialect::MySql) dialect.
///
/// The connection is opened at construction; a failure there is [`OracleUnavailable`]
/// (an infrastructure skip). `mysql::Conn`'s `prep`/`query_drop` take `&mut self`, so
/// the connection lives behind a [`RefCell`] to keep the shared-reference
/// [`verdict`](AcceptRejectOracle::verdict) signature.
pub struct MySqlOracle {
    conn: RefCell<Conn>,
}

/// A liveness-checked wire verdict from [`MySqlOracle::wire_verdict`], carrying the
/// server's error CODE on the reject path. `Accept` when `PREPARE` succeeded; `Reject`
/// with the `ER_*` code when the server answered with a trustworthy per-statement error
/// packet. The code is what lets the sweep classify a reject's reason (syntax vs binding)
/// off MySQL's coded wire packet rather than a brittle message-string match — the one
/// engine where the split is read authoritatively off the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireVerdict {
    /// The server prepared (parsed + bound) the statement.
    Accept,
    /// The server rejected it, carrying the `ER_*` server error code.
    Reject(u16),
}

impl WireVerdict {
    /// Whether this verdict is an [`Accept`](Self::Accept).
    pub fn accepts(self) -> bool {
        matches!(self, WireVerdict::Accept)
    }
}

impl MySqlOracle {
    /// The configured server URL: [`MYSQL_ORACLE_URL_ENV`] if set, else
    /// [`DEFAULT_MYSQL_ORACLE_URL`].
    fn url() -> String {
        std::env::var(MYSQL_ORACLE_URL_ENV).unwrap_or_else(|_| DEFAULT_MYSQL_ORACLE_URL.to_string())
    }

    /// Open a connection, mapping any failure (bad URL or unreachable server) to
    /// [`OracleUnavailable`] so callers skip rather than fail.
    fn connect() -> Result<Conn, OracleUnavailable> {
        let url = Self::url();
        let opts = Opts::from_url(&url).map_err(|err| {
            OracleUnavailable(format!("invalid {MYSQL_ORACLE_URL_ENV} {url:?}: {err}"))
        })?;
        Conn::new(opts)
            .map_err(|err| OracleUnavailable(format!("mysql connect to {url} failed: {err}")))
    }

    /// A bare connection with no schema provisioned — for the schema-independent corpus.
    pub fn new() -> Result<Self, OracleUnavailable> {
        Ok(Self {
            conn: RefCell::new(Self::connect()?),
        })
    }

    /// The setup driver: a connection with `setup_sql` (DDL) executed to provision the
    /// schema the schema-dependent corpus references. The DDL is the *only* thing ever
    /// executed on this connection — and that is a deliberate exception to the
    /// never-execute rule (DDL auto-commits on a real server); corpus statements are
    /// only `prep`ared. `setup_sql` is run one `;`-separated statement at a time because
    /// a single `query_drop` is one statement over the wire.
    pub fn with_schema(setup_sql: &str) -> Result<Self, OracleUnavailable> {
        let mut conn = Self::connect()?;
        for statement in setup_sql.split(';') {
            let statement = statement.trim();
            if statement.is_empty() {
                continue;
            }
            conn.query_drop(statement).map_err(|err| {
                OracleUnavailable(format!("mysql schema setup failed on {statement:?}: {err}"))
            })?;
        }
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }

    /// The server version string (`SELECT VERSION()`), for the "oracle actually ran"
    /// evidence a production-family inventory sweep records. A plain read-only query
    /// (never a DDL execute), so it is the same sanctioned exception to the prepare-only
    /// rule as the setup driver's schema DDL — safe against the shared server. `None` when
    /// the query fails.
    pub fn server_version(&self) -> Option<String> {
        self.conn
            .borrow_mut()
            .query_first("SELECT VERSION()")
            .ok()
            .flatten()
    }

    /// A liveness-checked verdict for the swept sweep, carrying the server's error CODE on
    /// the reject path: `Ok(`[`WireVerdict::Accept`]`)` / `Ok(`[`WireVerdict::Reject`]`(code))`
    /// when the server delivered a trustworthy answer, `Err(`[`OracleConnectionLost`]`)`
    /// when the connection died mid-sweep. Unlike [`verdict`](AcceptRejectOracle::verdict) —
    /// which folds *every* `prep` error into a `Reject` (acceptable for the start-gated
    /// curated corpora, where a wholesale outage is caught by the CI "oracle actually
    /// ran" guard) — this preserves the connection-vs-statement split (see
    /// [`classify_prep_error`]) so a 1,500-statement sweep can abort on a dying oracle
    /// instead of tallying its per-statement connection errors as garbage rejects. The
    /// carried code lets the sweep read a reject's reason (syntax vs binding) off MySQL's
    /// coded error packet rather than a message string.
    pub fn wire_verdict(&self, sql: &str) -> Result<WireVerdict, OracleConnectionLost> {
        match self.conn.borrow_mut().prep(sql) {
            Ok(_) => Ok(WireVerdict::Accept),
            Err(err) => match classify_prep_error(&err) {
                PrepErrorClass::StatementReject => match err {
                    mysql::Error::MySqlError(server) => Ok(WireVerdict::Reject(server.code)),
                    // The one non-packet statement reject: the driver's client-side
                    // MixedParams check (a `:name` + `?` statement). Verified against live
                    // mysql:8 — the raw statement is `ER_PARSE_ERROR` (`:name` is not MySQL
                    // syntax) — so carry that code.
                    mysql::Error::DriverError(mysql::DriverError::MixedParams) => Ok(
                        WireVerdict::Reject(mysql::ServerError::ER_PARSE_ERROR as u16),
                    ),
                    _ => unreachable!(
                        "classify_prep_error returns StatementReject only for a MySqlError \
                         packet or the MixedParams client check"
                    ),
                },
                PrepErrorClass::ConnectionLost => Err(OracleConnectionLost(err.to_string())),
            },
        }
    }

    /// A COM_QUERY *define-not-execute* verdict for a routine/trigger/event DDL statement —
    /// the DDL a `PREPARE` cannot answer for (a `CREATE PROCEDURE` under the prepare oracle is
    /// `ER_UNSUPPORTED_PS` (1295), which is grammar-*positive* but blind to a body syntax
    /// error). `ddl_verdict` instead *runs* the statement via `query_drop` (COM_QUERY) so the
    /// server's stored-program parser actually processes the body: a valid definition
    /// `Accept`s, a body syntax error is `Reject(ER_PARSE_ERROR=1064)`, the probe-proven
    /// channel.
    ///
    /// # Quarantine, isolation, and cleanup
    ///
    /// This is a DELIBERATE, quarantined exception to the never-execute rule (the reason the
    /// prepare corpora stay `prep`-only): a DDL statement is *defining*, not row-mutating, and
    /// it is run inside a freshly `CREATE`d, uniquely-named scratch **database** (never the
    /// shared oracle schema) that is `DROP`ped unconditionally at the end — so nothing the
    /// statement defines survives the call, and concurrent calls never collide (the unique id
    /// mixes the process id and a monotonic counter). Cleanup runs on every path, including a
    /// statement reject, so a rejected body still tears its scratch database down; only a
    /// mid-call connection loss (which the surrounding sweep aborts on anyway) can leave one
    /// behind, and the `zzp_ddl_scratch_%` name is self-identifying for a sweep-teardown. This
    /// method is authored-corpus-only and oracle-mysql-gated — it never touches the PREPARE
    /// corpora, whose no-execute contract is load-bearing.
    pub fn ddl_verdict(&self, sql: &str) -> Result<WireVerdict, OracleConnectionLost> {
        self.ddl_verdict_with_setup(&[], sql)
    }

    /// [`ddl_verdict`](Self::ddl_verdict) with extra `setup` statements run in the scratch
    /// database (in order) before the statement under test — the same connection-class
    /// contract as the bare provisioning (a healthy server never rejects them). This lets a
    /// body-bearing DDL that references a schema object (a `CREATE TRIGGER … ON <table>`, whose
    /// accept path needs the table to exist) be evidenced in the same quarantined,
    /// unconditionally-torn-down scratch database; a setup failure is a connection loss so a
    /// leaking sweep aborts rather than silently skewing the verdict.
    pub fn ddl_verdict_with_setup(
        &self,
        setup: &[&str],
        sql: &str,
    ) -> Result<WireVerdict, OracleConnectionLost> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let scratch = format!(
            "zzp_ddl_scratch_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        );
        let mut conn = self.conn.borrow_mut();
        // Provision + select the scratch database, then run the caller's setup. A failure here
        // is connection-class (the sweep aborts): a healthy server never rejects a fresh unique
        // `CREATE DATABASE` or a well-formed setup statement.
        let provisioning = [
            format!("CREATE DATABASE {scratch}"),
            format!("USE {scratch}"),
        ];
        for stmt in provisioning
            .iter()
            .map(String::as_str)
            .chain(setup.iter().copied())
        {
            if let Err(err) = conn.query_drop(stmt) {
                // Best-effort teardown before surfacing the loss.
                let _ = conn.query_drop(format!("DROP DATABASE IF EXISTS {scratch}"));
                return Err(OracleConnectionLost(format!(
                    "ddl scratch setup {stmt:?}: {err}"
                )));
            }
        }
        // The statement under test runs against the scratch database; its verdict is captured
        // before the mandatory teardown so a rejecting body is still cleaned up.
        let verdict = match conn.query_drop(sql) {
            Ok(()) => Ok(WireVerdict::Accept),
            Err(err) => match classify_prep_error(&err) {
                PrepErrorClass::StatementReject => match err {
                    mysql::Error::MySqlError(server) => Ok(WireVerdict::Reject(server.code)),
                    _ => Ok(WireVerdict::Reject(
                        mysql::ServerError::ER_PARSE_ERROR as u16,
                    )),
                },
                PrepErrorClass::ConnectionLost => Err(OracleConnectionLost(err.to_string())),
            },
        };
        // Mandatory cleanup on every non-lost path (the scratch database, and every routine it
        // holds, is dropped). A teardown failure downgrades the verdict to a connection loss so
        // a leaking sweep aborts rather than silently accreting scratch databases.
        if let Err(err) = conn.query_drop(format!("DROP DATABASE IF EXISTS {scratch}")) {
            return Err(OracleConnectionLost(format!("ddl scratch teardown: {err}")));
        }
        verdict
    }
}

// ---------------------------------------------------------------------------
// Oracle-death detection: connection-class vs statement-class prep failures
// ---------------------------------------------------------------------------
//
// A WIRE oracle can DIE mid-sweep (the 2026-07 incident: a disk-full host killed the
// `mysql:8` container, after which every `prep` returned a connection error). The bare
// [`verdict`](AcceptRejectOracle::verdict) above folds every `prep` error into a
// `Reject`, so a long sweep tallies those per-statement connection errors as rejects,
// minting plausible-looking garbage pins (the incident "measured" gap 4->0 / shadowed
// 393->451 purely from the dying container). [`classify_prep_error`] splits the two so
// the sweep can abort loudly on the FIRST connection-class error. Start-of-sweep
// liveness (`oracle_or_skip!`) is unchanged and is the deliberate contrast: an ABSENT
// oracle is a clean skip; a DYING oracle is a loud abort. See ADR-0014/0015 and
// the oracle liveness contract.

/// The connection-vs-statement classification of a `mysql` error — from `prep`, or from
/// `connect` in the refused-connection unit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepErrorClass {
    /// The server parsed and bound the statement and answered with a coded error
    /// *packet* — a trustworthy per-statement reject.
    StatementReject,
    /// No trustworthy verdict: the socket died (io / broken-pipe / server-gone), the
    /// wire desynced, a timeout fired, or the server announced it is going away.
    /// Mid-sweep this MUST abort the sweep, never tally as a reject.
    ConnectionLost,
}

/// Classify a `mysql` crate error as a per-statement reject vs a connection-class
/// failure — the heart of oracle-death detection.
///
/// Conservative by construction: a [`StatementReject`](PrepErrorClass::StatementReject) is
/// ONLY a clean server error *packet* ([`mysql::Error::MySqlError`] whose code is a genuine
/// statement-level rejection) or the driver's content-deterministic `MixedParams` client
/// check (below). Every other error kind is
/// [`ConnectionLost`](PrepErrorClass::ConnectionLost):
/// - `IoError` — broken pipe / connection reset / server gone;
/// - `CodecError` — wire-protocol desync (a truncated packet from a dying server);
/// - `DriverError` — connect failure / timeout / packet-out-of-sync;
/// - any (cfg-gated) `TlsError`, absorbed by the `_` arm the minimal-rust build needs;
/// - a `MySqlError` whose code is in the server shutdown/abort family (a liveness signal
///   wearing an error-packet coat — see `is_server_going_away`).
///
/// The one carved-out `DriverError` is [`MixedParams`](mysql::DriverError::MixedParams): the
/// client refuses to prepare a statement mixing `:name` and `?` params by inspecting the
/// query BYTES, before any I/O — so it is content-deterministic and can never be a symptom
/// of a dying server (which produces `IoError`/`CodecError`/packet-desync instead). MySQL
/// has no `:name` param, so the raw statement is a server syntax reject anyway; treating it
/// as a statement reject keeps a single pathological corpus line from spuriously aborting
/// the sweep.
///
/// This matches the crate's own `Error::is_connectivity_error` split, tightened by the
/// shutdown/abort-code reclassification and the `MixedParams` carve-out. "Anything not
/// clearly a statement-level error is connection-class", so a novel or ambiguous error can
/// never be silently tallied as a reject.
pub fn classify_prep_error(err: &mysql::Error) -> PrepErrorClass {
    match err {
        mysql::Error::MySqlError(server) if !is_server_going_away(server.code) => {
            PrepErrorClass::StatementReject
        }
        mysql::Error::DriverError(mysql::DriverError::MixedParams) => {
            PrepErrorClass::StatementReject
        }
        _ => PrepErrorClass::ConnectionLost,
    }
}

/// MySQL server error-packet codes that announce the SERVER is going away rather than
/// rejecting the statement. A `PREPARE` that returns one of these is a liveness signal,
/// not a per-statement verdict, so it is connection-class. Restricted to the unambiguous
/// shutdown/abort family — none can arise from preparing a valid statement against a
/// healthy server, so this reclassification carries no false-positive risk on a live run.
fn is_server_going_away(code: u16) -> bool {
    use mysql::ServerError::*;
    code == ER_SERVER_SHUTDOWN as u16
        || code == ER_NORMAL_SHUTDOWN as u16
        || code == ER_SHUTDOWN_COMPLETE as u16
        || code == ER_FORCING_CLOSE as u16
        || code == ER_ABORTING_CONNECTION as u16
        || code == ER_NEW_ABORTING_CONNECTION as u16
}

/// A mid-sweep connection loss: the oracle came up (so this is NOT an
/// [`OracleUnavailable`] start-of-sweep skip) but then died. The carried string is the
/// underlying `mysql` error, for the loud liveness-abort message. Only a WIRE oracle can
/// produce this — the in-process M1/M2 oracles hold no live connection to lose.
#[derive(Clone, Debug)]
pub struct OracleConnectionLost(pub String);

impl AcceptRejectOracle for MySqlOracle {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::PrepareBind
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        // Server-side PREPARE parses + binds without executing; any error is a reject
        // verdict (never classified), never an `OracleUnavailable` (infra only).
        Ok(OracleVerdict::from_accepts(
            self.conn.borrow_mut().prep(sql).is_ok(),
        ))
    }
}

/// A triaged M3 accept/reject divergence knowingly tolerated: a statement where the
/// MySQL server and our [`MySql`](squonk::dialect::MySql) dialect disagree. Every
/// entry must name an a non-empty provenance label; the tests assert each still
/// diverges, so a fixed gap cannot stay silently allowlisted (mirrors
/// `m2::M2_DIVERGENCE_ALLOWLIST` and `pg::PG_DIVERGENCE_ALLOWLIST` — not intra-doc
/// links, as `m2` is behind the sibling `oracle-engines` feature).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M3DivergenceAllowlistEntry {
    pub sql: &'static str,
    pub ticket: &'static str,
    pub reason: &'static str,
}

/// Current M3 accept/reject divergences allowed by the oracle.
///
/// Empty: `array` is admitted as an identifier and rejected only as a call head (the
/// `function_only` reservation class from `mysql-reserved-word-set-8-4-over-rejections`),
/// so both sides agree on `SELECT 1 AS array` (an accept) and on `SELECT array(1)` (a reject),
/// pinned in the schema-independent corpora below. The machinery stays in place for the next real entry;
/// the `divergence_allowlist_entries_still_diverge_and_are_ticketed` test is vacuous while
/// empty (ADR-0015: a fix forces removal, never a re-pin).
pub const M3_DIVERGENCE_ALLOWLIST: &[M3DivergenceAllowlistEntry] = &[];

/// Statements real MySQL accepts on `PREPARE` and our [`MySql`](squonk::dialect::MySql)
/// dialect parses, with no object names — comparable against a bare connection
/// ([`MySqlOracle::new`]). Beyond a portable baseline, these exercise MySQL-distinctive
/// surface: the left-associative comparison chains
/// (`mysql-comparison-operators-are-left-associative` — `1 < 2 < 3` is legal MySQL,
/// rejected by ANSI/PostgreSQL), the `DIV`/`MOD`/`XOR` keyword operators, `0x` hex
/// literals, the `LIMIT <offset>, <count>` comma form, and backtick identifiers.
pub const SCHEMA_INDEPENDENT_ACCEPT: &[&str] = &[
    "SELECT 1",
    "SELECT 1 + 2 * 3",
    "SELECT (1 + 2) * 3",
    "SELECT 1, 2, 3",
    "SELECT 'hello'",
    "SELECT 1 AS x",
    "SELECT abs(-5)",
    "SELECT length('abc')",
    "SELECT 1 UNION SELECT 2",
    "SELECT 1 UNION ALL SELECT 2",
    "SELECT CASE WHEN 1 > 0 THEN 'a' ELSE 'b' END",
    "SELECT DISTINCT 1",
    "SELECT 1 IN (1, 2, 3)",
    "SELECT 1 BETWEEN 0 AND 2",
    "SELECT coalesce(NULL, 1)",
    // MySQL-distinctive, rejected by ANSI/PostgreSQL or spelled differently there:
    "SELECT 1 < 2 < 3",
    "SELECT 1 = 2 = 3",
    "SELECT 1 <> 2 <> 3",
    "SELECT 3 DIV 2",
    "SELECT 5 MOD 2",
    "SELECT 1 XOR 0",
    "SELECT 0x1F + 1",
    "SELECT 1 LIMIT 5, 10",
    "SELECT 1 AS `x`",
    // The three 8.4 over-rejections closed by mysql-reserved-word-set-8-4-over-rejections,
    // engine-verified to PREPARE on mysql:8.4.10. `array` is an identifier in every
    // non-function position (its `function_only` reservation covers only `array(...)`, pinned
    // as a reject below); the deprecated MASTER_* replication grammar 8.4 removed leaves both
    // `master_bind` and `master_ssl_verify_server_cert` as ordinary identifiers.
    "SELECT 1 AS array",
    "SELECT 1 AS master_bind",
    "SELECT 1 AS master_ssl_verify_server_cert",
    // MySQL utility statements (mysql-utility-statements-kill-describe), all
    // engine-verified to PREPARE on a bare `mysql:8`: `KILL [CONNECTION|QUERY] <id>` with
    // an expression id (integer, string, `@user` variable), and the `DESCRIBE`/`DESC`
    // EXPLAIN synonyms for a table-free query.
    "KILL 5",
    "KILL CONNECTION 5",
    "KILL QUERY '123'",
    "KILL @id",
    "DESCRIBE SELECT 1",
    "DESC SELECT 1",
    // MySQL's `CONVERT` special form (mysql-convert-function), engine-verified accepts on
    // mysql:8.4.10. The comma form `CONVERT(expr, type)` admits the same restricted
    // `cast_type` set as `CAST` (incl. the now-modelled charset-annotated `CHAR`); the
    // `USING` form transcodes to a charset name (an `ident_or_text` or the `BINARY`
    // transcoding name). The operand is a full expression and the two forms nest.
    "SELECT CONVERT(1, SIGNED)",
    "SELECT CONVERT('x', CHAR(10))",
    "SELECT CONVERT('x', CHAR(10) CHARACTER SET utf8mb4)",
    "SELECT CONVERT('1.5', DECIMAL(10, 2))",
    "SELECT CONVERT('x' USING utf8mb4)",
    "SELECT CONVERT('x' USING binary)",
    "SELECT CONVERT(1 + 2 USING utf8mb4)",
    "SELECT CONVERT(CONVERT('x' USING utf8mb4), CHAR)",
    // MySQL's query-position `VALUES ROW( ... )` table-value constructor
    // (parse-mysql-values-do-use), engine-verified to PREPARE on mysql:8.4.10. Every row is
    // the explicit `ROW( ... )` spelling (a bare `(1)` row is `ER_PARSE_ERROR`, pinned as a
    // reject below); the constructor composes with a query tail and a set operation, and a
    // `DEFAULT` element is grammar-legal (it resolver-rejects, so it stays out of this
    // PREPARE-accept corpus).
    "VALUES ROW(1, 2)",
    "VALUES ROW(1, 2), ROW(3, 4)",
    "VALUES ROW(1), ROW(2) ORDER BY 1 LIMIT 1",
    "VALUES ROW(1) UNION VALUES ROW(2)",
    // MySQL's `DO <expr-list>` evaluate-and-discard statement (parse-mysql-values-do-use), a
    // distinct behaviour on the `DO` keyword from PostgreSQL's code block. The grammar is
    // `DO select_item_list`, so a select alias (`DO 1 AS x`) parses exactly as the engine
    // PREPAREs it; all engine-verified to PREPARE on mysql:8.4.10.
    "DO 1 + 1",
    "DO 1, 2, 3",
    "DO SLEEP(0)",
    "DO 1 AS x",
    // Bare (`AS`-less) string projection alias + its adjacent-string-concat overlap
    // (mysql-bare-string-alias-vs-adjacent-concat), all engine-measured on mysql:8.4.10.
    // A string after a NON-string expression is a bare alias (`SELECT 1 'x'` names the
    // column `x`); a string after a STRING is a concatenation continuation
    // (`SELECT 'a' 'b'` is the one value `'ab'`) — the split resolves by parse ordering, the
    // continuation folding before the alias parser runs. MySQL lexes `"…"` as a string, so
    // it too concatenates (`'a' "b"` → `'ab'`) and is a bare-alias spelling (`SELECT 1 "x"`).
    "SELECT 1 'x'",
    "SELECT NULL 'x'",
    "SELECT (1 + 1) 'x'",
    "SELECT 1 \"x\"",
    "SELECT 'a' 'b' 'c'",
    "SELECT 'a' \"b\"",
    "SELECT \"a\" \"b\"",
    "SELECT N'a' 'b'",
    "SELECT _utf8'a' 'b'",
];

/// Statements both the MySQL server and our `MySql` dialect reject — syntactic errors,
/// so the reject is a genuine parse failure (not a `PrepareBind` name-resolution
/// reject).
pub const SCHEMA_INDEPENDENT_REJECT: &[&str] = &[
    "SELECT",
    "SELECT FROM",
    "SELECT 1 +",
    "SELECT * FROM",
    "SELCT 1",
    "INSERT INTO",
    "FROM",
    "SELECT (",
    "123 456",
    // `KILL` with no id is a syntax error both sides (engine-verified reject on mysql:8).
    "KILL",
    // The MySQL `cast_type` boundary (mysql-faithful-cast-type-production): a name that is a
    // valid column type but NOT a cast target is `ER_PARSE_ERROR` (1064) in cast position —
    // both the server and the fitted `MySql` preset reject. Bare `GEOMETRY` (a column type,
    // unlike its `GEOMETRYCOLLECTION` sibling), `YEAR`/spatial with a tail (they take none),
    // and the common `VARCHAR`/`TIMESTAMP`/`INT` column types are the representative rejects.
    "SELECT CAST(1 AS GEOMETRY)",
    "SELECT CAST(1 AS YEAR(4))",
    "SELECT CAST(1 AS VARCHAR)",
    "SELECT CAST(1 AS TIMESTAMP)",
    // The charset-annotation boundary in cast position (mysql-char-charset-annotation): the
    // annotation rides `CHAR`/`CHARACTER` only, so `NCHAR CHARACTER SET x` (the national
    // forms fix their charset) and `VARCHAR(5) CHARACTER SET x` (`VARCHAR` is not a
    // `cast_type`) are `ER_PARSE_ERROR` (1064) on mysql:8.4 — the annotated type parses but
    // the cast-target gate / closing `)` still rejects, so both sides reject.
    "SELECT CAST(1 AS NCHAR CHARACTER SET utf8mb4)",
    "SELECT CAST(1 AS VARCHAR(5) CHARACTER SET utf8mb4)",
    // The `CONVERT` special form's reject boundary (mysql-convert-function), engine-verified
    // `ER_PARSE_ERROR` (1064) on mysql:8.4: the comma form shares CAST's `cast_type` gate
    // (`INT`/`VARCHAR` reject in `CONVERT` position just as in `CAST`), the `USING` form
    // needs a charset operand, and the `AS`-cast spelling and a third comma argument are not
    // `CONVERT` grammar.
    "SELECT CONVERT(1, INT)",
    "SELECT CONVERT('x', VARCHAR(10))",
    "SELECT CONVERT('x' USING)",
    "SELECT CONVERT('x' AS CHAR)",
    "SELECT CONVERT('x', CHAR, BINARY)",
    // The four words MySQL 8.4 reserves that the 8.0 list missed
    // (as-label-position-aware-reserved-split): each is fully reserved (1064 as an AS
    // alias), so `SelectSyntax::as_alias_rejects_reserved` reroutes the projection alias
    // to `reserved_bare_alias` and both sides reject. Out of the vendored corpora, so
    // these hand-picked lines are the regression floor for the reservation. `INTERSECT`/
    // `PARALLEL`/`QUALIFY`/`TABLESAMPLE` alone (not as an alias) also reject syntactically.
    "SELECT 1 AS intersect",
    "SELECT 1 AS parallel",
    "SELECT 1 AS qualify",
    "SELECT 1 AS tablesample",
    // `array` is reserved as a *call head* only (mysql-reserved-word-set-8-4-over-rejections
    // `function_only` class): `array(...)` is 1064 on mysql:8.4.10 while `SELECT 1 AS array`
    // (accept corpus above) prepares — both directions pinned. This is the reject direction
    // that keeps the four sqlglot `ARRAY(1,2,3)`/`ARRAY(ARRAY(…))`/`MAP(ARRAY(…))`/
    // `MAX(ARRAY(…))` over-acceptances closed.
    "SELECT array(1)",
    "SELECT ARRAY(1, 2, 3)",
    // The statement-level `CREATE TABLE … LIKE` reject boundary
    // (mysql-create-table-like-statement), engine-verified `ER_PARSE_ERROR` (1064) on
    // mysql:8.4.10 — all SYNTACTIC (reject before name binding, so schema-independent): a missing
    // source, trailing table options after the bare form, a co-element beside `(LIKE …)`, a
    // second `LIKE`, and the PostgreSQL `INCLUDING`/`EXCLUDING` copy options (MySQL carries a bare
    // source name only). Both the server and the fitted `MySql` preset reject.
    "CREATE TABLE ctlike_r1 LIKE",
    "CREATE TABLE ctlike_r2 LIKE t1 ENGINE=InnoDB",
    "CREATE TABLE ctlike_r3 (LIKE t1, x INT)",
    "CREATE TABLE ctlike_r4 (x INT, LIKE t1)",
    "CREATE TABLE ctlike_r5 (LIKE t1, LIKE t2)",
    "CREATE TABLE ctlike_r6 (LIKE t1 INCLUDING ALL)",
    // The `MATCH (…) AGAINST (…)` full-text special form's SYNTACTIC reject boundary
    // (mysql-match-against), engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each
    // rejects before name binding, so it is schema-independent: an empty or non-column-ref
    // column list, an empty/absent `AGAINST` operand, a comparison operand (the operand is a
    // `bit_expr`, which excludes comparisons), and every modifier tail outside the four
    // documented combinations. Both the server and the fitted `MySql` preset reject.
    "SELECT MATCH() AGAINST('x')",
    "SELECT MATCH(a + 1) AGAINST('x')",
    "SELECT MATCH('lit') AGAINST('x')",
    "SELECT MATCH(a, b) AGAINST()",
    "SELECT MATCH(a, b) AGAINST(1 > 2)",
    "SELECT MATCH(a, b) AGAINST('x' IN BOOLEAN MODE WITH QUERY EXPANSION)",
    "SELECT MATCH(a, b) AGAINST('x' IN QUERY EXPANSION)",
    "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE)",
    "SELECT MATCH(a, b) AGAINST('x' WITH EXPANSION)",
    // The prefix-typed / temporal literal *value*-kind reject boundary
    // (typed-literal-value-sconst-per-engine), engine-verified `ER_PARSE_ERROR` (1064) on
    // mysql:8.4.10 — each rejects at parse, so it is schema-independent. MySQL's temporal
    // literal (`DATE`/`TIME`/`TIMESTAMP '…'`) takes a plain character-string value only; a
    // bit-string (`B'…'`/`X'…'`), national (`N'…'`), or charset-introducer (`_utf8'…'`)
    // value in that position is 1064 — matching PostgreSQL's `Sconst`-only rule and DuckDB.
    // The fitted `MySql` preset over-accepted these (its typed-literal path took any string);
    // the value gate now rejects them, so both the server and the preset reject.
    "SELECT DATE B'1'",
    "SELECT DATE X'ab'",
    "SELECT DATE N'x'",
    "SELECT DATE _utf8'x'",
    "SELECT TIMESTAMP X'ab'",
    "SELECT TIME B'1'",
    // The `VALUES`/`DO`/`USE` reject boundaries (parse-mysql-values-do-use), engine-verified
    // `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse (before name binding), so
    // it is schema-independent. A query-position `VALUES` row must be the explicit `ROW( ... )`
    // form (a bare `(1)` row is 1064); a bare `DO`/`USE` has no operand; and MySQL's `USE ident`
    // takes a single unqualified schema, so a dotted `USE a.b` is 1064. Both the server and the
    // fitted `MySql` preset reject.
    "VALUES (1)",
    "VALUES (1, 2), (3, 4)",
    "DO",
    "USE",
    "USE a.b",
    // The prepared-statement lifecycle reject boundaries (parse-mysql-prepare-execute),
    // engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it
    // is schema-independent. A `prepare_src` is a string or a `@`-variable only (never an
    // expression, a bare statement, or a `@@` system variable); the `EXECUTE` arguments ride
    // `USING @var` only (never a value or a parenthesized list, and never `@@`); the release
    // verb's `PREPARE` keyword is mandatory (a bare `DEALLOCATE s` is 1064, unlike DuckDB),
    // takes exactly one name, and admits no `IF EXISTS` guard. Both the server and the
    // fitted `MySql` preset reject.
    "PREPARE s FROM 1+1",
    "PREPARE s FROM SELECT 1",
    "PREPARE s FROM @@global.x",
    "PREPARE s",
    "EXECUTE s USING 1",
    "EXECUTE s USING",
    "EXECUTE s (1)",
    "EXECUTE s USING @@x",
    "DEALLOCATE s",
    "DEALLOCATE PREPARE s, t",
    "DROP PREPARE IF EXISTS s",
    // The LOCK/UNLOCK reject boundaries (parse-mysql-lock-tables-instance), engine-verified
    // `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it is
    // schema-independent (the grammar-positive shapes instead reach 1046/1295, name-binding /
    // PREPARE-protocol verdicts past the parser). The per-table lock kind is mandatory
    // (`LOCK TABLES t1` and a trailing alias with no kind are 1064), the table list is
    // non-empty, a bare `LOCK`/`UNLOCK` is nothing, and MySQL 8 has no `LOW_PRIORITY WRITE`
    // lock kind (a historical pre-8.0 spelling) — `LOW_PRIORITY` is a MySQL reserved word,
    // which is exactly how the fitted preset rejects it (the bare-alias position refuses
    // reserved words, then the mandatory-kind expectation fails). Both the server and the
    // fitted `MySql` preset reject.
    "LOCK TABLES t1",
    "LOCK TABLES t1 xyz",
    "LOCK TABLES",
    "LOCK",
    "UNLOCK",
    "LOCK TABLES t1 LOW_PRIORITY WRITE",
    // The HANDLER cursor-family reject boundaries (parse-mysql-handler), engine-verified
    // `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it is
    // schema-independent. Only `OPEN` takes a schema-qualified `table_ident`; `READ`/`CLOSE`
    // take a bare `ident` (a dotted name is 1064). A bare (indexless) `READ` scan admits only
    // `FIRST`/`NEXT` — `PREV`/`LAST` require a named index. The key-seek operator set is
    // exactly `= >= <= > <` (`<>`/`!=` reject), and the value list is non-empty (`()` rejects).
    // Both the server and the fitted `MySql` preset reject.
    "HANDLER db.t READ FIRST",
    "HANDLER db.t CLOSE",
    "HANDLER t READ PREV",
    "HANDLER t READ LAST",
    "HANDLER t READ idx <> (1)",
    "HANDLER t READ idx != (1)",
    "HANDLER t READ idx = ()",
    // The server-administration reject boundaries (parse-mysql-server-admin), engine-verified
    // `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it is
    // schema-independent. `SHUTDOWN`/`RESTART` are nullary (a trailing operand is 1064); CLONE
    // LOCAL requires `DATA DIRECTORY` and CLONE INSTANCE requires a `:<port>` that abuts the
    // account with no whitespace (a space on either side of the `:` is 1064, a raw-offset
    // adjacency check in the grammar); IMPORT TABLE / BINLOG take strings, not bare idents; and
    // HELP takes exactly one operand. Both the server and the fitted `MySql` preset reject.
    "SHUTDOWN 1",
    "RESTART 1",
    "CLONE LOCAL 'd'",
    "CLONE LOCAL DATA DIRECTORY",
    "CLONE INSTANCE FROM u@h IDENTIFIED BY 'p'",
    "CLONE INSTANCE FROM u@h :3306 IDENTIFIED BY 'p'",
    "CLONE INSTANCE FROM u@h: 3306 IDENTIFIED BY 'p'",
    "IMPORT TABLE FROM f",
    "HELP 'a' 'b'",
    "BINLOG garbage",
    // The INSTALL/UNINSTALL plugin/component reject boundaries (parse-mysql-plugin-component),
    // engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it is
    // schema-independent. A plugin is exactly one bare `ident` with a mandatory string `SONAME`
    // (a quoted-string name, a bare-ident SONAME, a missing SONAME, and a comma list are all
    // 1064); a component URN is a string, never a bare ident. The `INSTALL COMPONENT … SET`
    // scope set is exactly `GLOBAL`/`PERSIST` (`SESSION`/`LOCAL`/`PERSIST_ONLY` are 1064, as
    // are `@`/`@@` sigil variables), the value grammar has no `DEFAULT` sentinel, and
    // `UNINSTALL COMPONENT` takes no `SET` tail. Both the server and the fitted `MySql`
    // preset reject.
    "INSTALL PLUGIN p",
    "INSTALL PLUGIN 'p' SONAME 'lib.so'",
    "INSTALL PLUGIN p SONAME lib",
    "INSTALL PLUGIN p SONAME 'lib.so', q SONAME 'x.so'",
    "UNINSTALL PLUGIN p, q",
    "UNINSTALL PLUGIN 'p'",
    "INSTALL COMPONENT file",
    "INSTALL COMPONENT 'x' SET SESSION v = 1",
    "INSTALL COMPONENT 'x' SET LOCAL v = 1",
    "INSTALL COMPONENT 'x' SET PERSIST_ONLY v = 1",
    "INSTALL COMPONENT 'x' SET @v = 1",
    "INSTALL COMPONENT 'x' SET @@v = 1",
    "INSTALL COMPONENT 'x' SET v = DEFAULT",
    "UNINSTALL COMPONENT file",
    "UNINSTALL COMPONENT 'x' SET v = 1",
    // The `XA` distributed-transaction reject boundaries (parse-mysql-xa-transactions),
    // engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it
    // is schema-independent. `xid` is mandatory where the grammar names it and its `formatID`
    // is numeric and only after a `bqual` (a bare decimal `gtrid` and a string `formatID` both
    // reject); the suffix keywords bind to their own verb only (`JOIN`/`RESUME` to START,
    // `SUSPEND` to END, `ONE PHASE` to COMMIT), and `RECOVER` takes no `xid` and requires both
    // words of `CONVERT XID`. Both the server and the fitted `MySql` preset reject.
    "XA START",
    "XA START 42",
    "XA START 'g', 'b', 'c'",
    "XA START 'g' JOIN RESUME",
    "XA START 'gtrid' SUSPEND",
    "XA END 'gtrid' JOIN",
    "XA END 'g' FOR MIGRATE",
    "XA COMMIT 'gtrid' TWO PHASE",
    "XA PREPARE 'gtrid' ONE PHASE",
    "XA RECOVER 'gtrid'",
    "XA RECOVER CONVERT",
    // The replication-administration reject boundaries (parse-mysql-replication),
    // engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it
    // is schema-independent. MySQL 8.4 removed the legacy `MASTER`/`SLAVE` spellings (`CHANGE
    // MASTER TO`, `START`/`STOP SLAVE`, and every `MASTER_*` option name), and the compression
    // option is the *plural* `SOURCE_COMPRESSION_ALGORITHMS` (the singular is 1064). The option
    // and rule lists are non-empty; a filter table name must be schema-qualified (a bare `t1`
    // is 1064); `REPLICATE_REWRITE_DB` pairs are doubly parenthesized (a single-paren `(a, b)`
    // is 1064). `STOP REPLICA` takes no `UNTIL`/connection tail; `GROUP_REPLICATION`'s options
    // are comma-separated (a space between them is 1064) and `STOP GROUP_REPLICATION` takes
    // none; and a `START REPLICA … UNTIL` GTID condition may only lead the list (a GTID after a
    // comma is 1064). Both the server and the fitted `MySql` preset reject.
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
    // MySQL has no typed interval literal (mysql-interval-literal-path-ansi-spellings),
    // engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10 — each rejects at parse, so it
    // is schema-independent. The ANSI `TO` composite and `(p)` unit precision reject in the
    // standalone AND `+`/`-` operand positions alike (only the operator-position
    // `INTERVAL <expr> <underscore-unit>` is MySQL grammar), and a unit-less `INTERVAL '1'`
    // rejects too. Both the server and the fitted `MySql` preset
    // (`ExpressionSyntax::typed_interval_literal` off) reject.
    "SELECT INTERVAL '1' HOUR TO SECOND",
    "SELECT INTERVAL '1' SECOND(3)",
    "SELECT INTERVAL '1-2' YEAR TO MONTH",
    "SELECT '2020-01-01' - INTERVAL '1' HOUR TO SECOND",
    "SELECT '2020-01-01' - INTERVAL '1' SECOND(3)",
    "SELECT INTERVAL '1'",
    // The bare-string-alias / adjacent-concat reject boundary
    // (mysql-bare-string-alias-vs-adjacent-concat), all engine-verified `ER_PARSE_ERROR`
    // (1064) on mysql:8.4.10 — each rejects at parse. A bare string alias takes no second
    // string (`SELECT 1 'x' 'y'` — `'x'` is the alias, `'y'` is stray); only an *unprefixed*
    // string continues a concatenation, so a prefixed `_charset'…'`/`N'…'`/bit second segment
    // is neither a continuation nor a bare alias.
    "SELECT 1 'x' 'y'",
    "SELECT 'a' _utf8'b'",
    "SELECT _utf8'a' _utf8'b'",
    "SELECT 'a' N'b'",
];

/// Statements both accept **after** [`MYSQL_SCHEMA_SETUP_SQL`] is provisioned. Every
/// referenced name exists in that schema, so the server binds cleanly and its accept
/// matches ours. Includes the schema-dependent form of the left-associative comparison chain
/// (`SELECT a < b < c FROM t1`) and the MySQL `STRAIGHT_JOIN` hint.
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
    "SELECT a < b < c FROM t1",
    "SELECT t1.a FROM t1 STRAIGHT_JOIN t2 ON t1.a = t2.f",
    // The MySQL `{DESCRIBE|DESC|EXPLAIN} <table> [<column>]` table-metadata overload,
    // engine-verified to PREPARE once a database is selected (the bare connection rejects
    // for "no database selected", so these are schema-dependent): all three keyword
    // spellings, plus the optional column narrowing.
    "DESCRIBE t1",
    "DESC t1",
    "EXPLAIN t1",
    "DESCRIBE t1 a",
    // MySQL's statement-level `CREATE TABLE … LIKE <source>` table-clone body
    // (mysql-create-table-like-statement), engine-verified to PREPARE on mysql:8.4.10.
    // Schema-dependent: the source is name-resolved (PrepareBind), so it references the
    // provisioned `t1`. Both the bare and parenthesized spellings, `IF NOT EXISTS`, `TEMPORARY`,
    // and a qualified source all accept; distinct from the PostgreSQL copy element.
    "CREATE TABLE ctlike_a LIKE t1",
    "CREATE TABLE ctlike_b (LIKE t1)",
    "CREATE TABLE IF NOT EXISTS ctlike_c LIKE t1",
    "CREATE TEMPORARY TABLE ctlike_d LIKE t1",
    "CREATE TABLE ctlike_e LIKE squonk_oracle.t1",
    // MySQL's full-text `MATCH (<col>, …) AGAINST (<expr> [<modifier>])` special form
    // (mysql-match-against), engine-verified to PREPARE on mysql:8.4.10 against `ft1`'s
    // `FULLTEXT` index. Schema-dependent: without a covering full-text index the same
    // grammar *binds*-rejects (`ER_FT_MATCHING_KEY_NOT_FOUND`), so the accept needs the
    // provisioned index. Covers the default (no modifier) and all four documented
    // modifier combinations, qualified columns, and a `bit_expr` operand.
    "SELECT MATCH(a, b) AGAINST('x') FROM ft1",
    "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE MODE) FROM ft1",
    "SELECT MATCH(a, b) AGAINST('x' IN NATURAL LANGUAGE MODE WITH QUERY EXPANSION) FROM ft1",
    "SELECT MATCH(a, b) AGAINST('x' IN BOOLEAN MODE) FROM ft1",
    "SELECT MATCH(a, b) AGAINST('x' WITH QUERY EXPANSION) FROM ft1",
    "SELECT MATCH(ft1.a, ft1.b) AGAINST(concat('x', 'y')) FROM ft1",
    "SELECT 1 FROM ft1 WHERE MATCH(a, b) AGAINST('x' IN BOOLEAN MODE)",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::accept_reject_divergence;
    use squonk::dialect::MySql;
    use squonk::parse_with;

    /// Bind `$name` to a reachable [`MySqlOracle`], or skip the test: unlike the
    /// in-process M2 engines, this oracle connects over the wire, so no reachable server
    /// (local dev without one) is an infrastructure skip, never a failure — mirroring
    /// DuckDB's `OracleUnavailable` handling. The `skipping mysql` marker is what the CI
    /// "oracle actually ran" guard greps for.
    macro_rules! oracle_or_skip {
        ($name:ident = $ctor:expr) => {
            let $name = match $ctor {
                Ok(oracle) => oracle,
                Err(OracleUnavailable(reason)) => {
                    eprintln!("skipping mysql differential: {reason}");
                    return;
                }
            };
        };
    }

    #[test]
    fn oracle_is_prepare_bind_when_reachable() {
        // The declared semantics govern which corpus is comparable (module contract), so
        // they are part of the interface. Asserted only when a server is reachable,
        // since construction connects.
        oracle_or_skip!(oracle = MySqlOracle::new());
        assert_eq!(oracle.name(), "mysql");
        assert_eq!(oracle.semantics(), OracleSemantics::PrepareBind);
    }

    #[test]
    fn mysql_accept_reject_parity_over_curated_corpus() {
        oracle_or_skip!(bare = MySqlOracle::new());
        // Machine-readable "oracle actually ran" evidence the nightly workflow greps for
        // (oracle-nightly.yml); emitted only on the ran path, so its absence trips the guard.
        eprintln!(
            "oracle-ran: mysql (server VERSION() = {:?})",
            bare.server_version()
        );
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_INDEPENDENT_REJECT)
        {
            assert_eq!(
                accept_reject_divergence(sql, MySql, &bare),
                None,
                "mysql/MySql accept-reject divergence on schema-independent {sql:?}",
            );
        }
        oracle_or_skip!(provisioned = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
        for sql in SCHEMA_DEPENDENT_ACCEPT {
            assert_eq!(
                accept_reject_divergence(sql, MySql, &provisioned),
                None,
                "mysql/MySql accept-reject divergence on schema-dependent {sql:?}",
            );
        }
    }

    #[test]
    fn setup_driver_prevents_false_divergence() {
        // The load-bearing reason the seam carries `semantics()`: a `PrepareBind` oracle
        // over schema-dependent SQL against an *empty* server yields a false divergence
        // (we accept, the server rejects "no such table"); provisioning the schema first
        // removes it.
        let probe = "SELECT a FROM t1";
        oracle_or_skip!(bare = MySqlOracle::new());
        assert!(
            accept_reject_divergence(probe, MySql, &bare).is_some(),
            "unprovisioned mysql must falsely diverge on {probe:?}",
        );
        oracle_or_skip!(provisioned = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
        assert!(
            accept_reject_divergence(probe, MySql, &provisioned).is_none(),
            "provisioned mysql must agree on {probe:?}",
        );
    }

    #[test]
    fn oracle_never_executes() {
        // The verdict must `prep` without executing: preparing an `INSERT` leaves the
        // provisioned table empty.
        oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
        assert!(
            oracle
                .verdict("INSERT INTO t1 VALUES (1, 2, 3, 4, 5)")
                .unwrap()
                .accepts(),
            "the INSERT must prepare (accept)",
        );
        let rows: i64 = oracle
            .conn
            .borrow_mut()
            .query_first("SELECT count(*) FROM t1")
            .expect("count query runs")
            .expect("count returns a row");
        assert_eq!(rows, 0, "preparing the INSERT must not have executed it");
    }

    #[test]
    fn corpus_is_single_statement() {
        // A corpus entry with a top-level `;` cannot be a single server-side prepared
        // statement (`COM_STMT_PREPARE` is one statement). None of ours contain one.
        // (`MYSQL_SCHEMA_SETUP_SQL` is intentionally multi-statement — it is *executed*
        // by the setup driver, split on `;`, not handed to `verdict` — so it is
        // excluded here.)
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_INDEPENDENT_REJECT)
            .chain(SCHEMA_DEPENDENT_ACCEPT)
        {
            assert!(
                !sql.contains(';'),
                "corpus entries must be single statements: {sql:?}",
            );
        }
    }

    #[test]
    fn parser_side_matches_corpus_labels() {
        // Our half of the differential, runnable with no server: the accept corpora must
        // parse under `MySql` and the reject corpus must not. This is what keeps the
        // curated corpus honest on local runs where the server-side check skips — a
        // mislabelled entry fails here instead of surfacing only in nightly CI.
        for sql in SCHEMA_INDEPENDENT_ACCEPT
            .iter()
            .chain(SCHEMA_DEPENDENT_ACCEPT)
        {
            assert!(
                parse_with(sql, MySql).is_ok(),
                "MySql should parse the accept-corpus entry {sql:?}",
            );
        }
        for sql in SCHEMA_INDEPENDENT_REJECT {
            assert!(
                parse_with(sql, MySql).is_err(),
                "MySql should reject the reject-corpus entry {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_reads_rollup_group_by_as_a_function_call() {
        use squonk_ast::{Expr, GroupByItem, SetExpr, Statement};

        // MySQL has no standard grouping sets (`grouping_sets` off), so `ROLLUP (a, b)`
        // in GROUP BY falls through to the expression grammar as an ordinary function
        // call — MySQL resolves it as a stored-function reference. The empty grouping
        // set `()` is not a valid expression there, so it is rejected. (MySQL's own
        // grouping surface is the distinct trailing `WITH ROLLUP`, not modelled here.)
        let parsed = parse_with("SELECT a FROM t GROUP BY ROLLUP (a, b)", MySql)
            .expect("MySQL parses rollup as a function call");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT");
        };
        assert!(matches!(
            &select.group_by[0],
            GroupByItem::Expr {
                expr: Expr::Function { .. },
                ..
            },
        ));
        assert!(
            parse_with("SELECT a FROM t GROUP BY ()", MySql).is_err(),
            "MySQL rejects the empty grouping set (no grouping-set surface)",
        );
    }

    #[test]
    fn divergence_allowlist_entries_still_diverge_and_are_ticketed() {
        // Mirrors the M2/PG allowlists: every allowlisted divergence must name a real
        // ticket and still actually diverge. Vacuous while the allowlist is empty, but
        // keeps the machinery in place for the first real entry.
        oracle_or_skip!(oracle = MySqlOracle::new());
        for entry in M3_DIVERGENCE_ALLOWLIST {
            assert!(
                !entry.ticket.trim().is_empty(),
                "allowlisted divergence needs a provenance label: {} ({})",
                entry.ticket,
                entry.reason,
            );
            assert!(
                accept_reject_divergence(entry.sql, MySql, &oracle).is_some(),
                "allowlisted case no longer diverges: the MySQL server and our MySql dialect now agree, \
                 so the divergence is fixed — SWEEP this entry (delete it from M3_DIVERGENCE_ALLOWLIST), \
                 never re-pin or edit it to keep it allowlisted (ADR-0015: a fix forces removal): {:?}",
                entry.sql,
            );
        }
    }

    #[test]
    fn refused_connection_classifies_as_connection_lost() {
        // The ticket-named fixture, exercising the classification split with NO live
        // server: bind an ephemeral port then drop the listener, so nothing listens
        // there — a connect to it is guaranteed-refused. A refused connection is the
        // archetypal connection-class failure; classifying it as anything other than
        // [`PrepErrorClass::ConnectionLost`] is exactly the oracle-death bug (a dead
        // wire tallying as a statement reject).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("listener addr").port();
        drop(listener); // release the port — now nothing listens on it

        // Bound the connect so a pathological host can never hang the unit test.
        let url = format!("mysql://root@127.0.0.1:{port}?tcp_connect_timeout_ms=2000");
        let opts = Opts::from_url(&url).expect("well-formed url");
        let err = Conn::new(opts).expect_err("connecting to a dead port must fail");

        assert_eq!(
            classify_prep_error(&err),
            PrepErrorClass::ConnectionLost,
            "a refused connection must be connection-class (abort), never a statement \
             reject (tally): {err:?}",
        );
    }

    #[test]
    fn server_error_packet_classifies_as_statement_reject() {
        // The other half of the split — without it, a classifier that always answered
        // ConnectionLost would pass the refused-connection test. A real server error
        // PACKET (here ER_PARSE_ERROR, the syntax reject) means the server was alive
        // enough to parse and answer, so it is a trustworthy per-statement verdict.
        let syntax_reject = mysql::Error::MySqlError(mysql::MySqlError {
            state: "42000".to_string(),
            code: mysql::ServerError::ER_PARSE_ERROR as u16,
            message: "You have an error in your SQL syntax".to_string(),
        });
        assert_eq!(
            classify_prep_error(&syntax_reject),
            PrepErrorClass::StatementReject,
        );
    }

    #[test]
    fn mixed_params_client_error_classifies_as_statement_reject() {
        // The mysql client refuses to PREPARE a statement mixing `:name` and `?` params by
        // inspecting the query bytes BEFORE any server round-trip — content-deterministic,
        // never a liveness signal — so it must be a statement reject, not a bogus
        // oracle-death abort. (A single sqlglot corpus line, `SELECT :hello, ? FROM x
        // LIMIT :my_limit`, triggers it; the raw statement is ER_PARSE_ERROR on the server
        // since `:name` is not MySQL.)
        let mixed = mysql::Error::DriverError(mysql::DriverError::MixedParams);
        assert_eq!(classify_prep_error(&mixed), PrepErrorClass::StatementReject);
    }

    #[test]
    fn server_going_away_packet_classifies_as_connection_lost() {
        // A server error packet is NOT automatically trustworthy: the shutdown/abort
        // family is a liveness signal (the server is going away), so it is
        // connection-class even though it arrives as a coded error. This is what makes
        // the disk-full-then-shutdown incident abort rather than tally, even on the code
        // path where the dying server manages one last error packet before the socket
        // drops.
        for code in [
            mysql::ServerError::ER_SERVER_SHUTDOWN,
            mysql::ServerError::ER_NORMAL_SHUTDOWN,
            mysql::ServerError::ER_SHUTDOWN_COMPLETE,
            mysql::ServerError::ER_FORCING_CLOSE,
            mysql::ServerError::ER_ABORTING_CONNECTION,
            mysql::ServerError::ER_NEW_ABORTING_CONNECTION,
        ] {
            let going_away = mysql::Error::MySqlError(mysql::MySqlError {
                state: "HY000".to_string(),
                code: code as u16,
                message: "server going away".to_string(),
            });
            assert_eq!(
                classify_prep_error(&going_away),
                PrepErrorClass::ConnectionLost,
                "server-going-away code {code:?} must be connection-class",
            );
        }
    }
}
