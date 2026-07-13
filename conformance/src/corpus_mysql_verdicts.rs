// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! MySQL accept/reject **parity gate** over the vendored corpora + authored MySQL feature
//! probes (`mysql-oracle-at-scale`).
//!
//! The MySQL analogue of [`corpus_sqlite_verdicts`](crate::corpus_sqlite_verdicts) and
//! [`corpus_duckdb_verdicts`](crate::corpus_duckdb_verdicts) — it adopts the same shared
//! [`Verdict`]/[`Quadrant`]/[`RejectReason`] model from
//! [`verdict_harness`](crate::verdict_harness) — with two deliberate differences:
//!
//! - **The parser side is the fitted [`MySql`] preset**, not a nearest stand-in:
//!   MySQL's dialect data already ships, so a coverage gap here is the TRUE residual
//!   the programme drives to zero, not a preset-fitting artifact.
//! - **The oracle is the external-server [`MySqlOracle`]** (m3, wire protocol,
//!   PREPARE-only): unreachable-server runs skip cleanly ([`OracleUnavailable`]),
//!   and the nightly CI job's "oracle actually ran" guard is what makes the skip
//!   impossible where a server is declared.
//!
//! # From the bespoke `Bucket` tally to the shared reject-reason split
//!
//! Phase 0 ran a bespoke three-bucket tally (`Agree` / `CoverageGap` / `SchemaShadowed`)
//! that could not tell a harmless name-resolution reject from a REAL syntax
//! over-acceptance: its ~948 "schema-shadowed" statements conflated the two. This pass
//! replaces it with the shared model, splitting the *we accept ∧ server rejects* bucket by
//! the server's reject reason:
//!
//! - **The reject reason splits binding from syntax — read off the coded packet.** MySQL is
//!   the one engine whose wire protocol delivers coded error packets, so the split is read
//!   from the CODE (authoritative), not a message-string heuristic:
//!   [`classify_mysql_code`] maps `ER_PARSE_ERROR` (1064) → [`Syntax`](RejectReason::Syntax),
//!   the name-resolution / already-exists / does-not-exist family →
//!   [`Binding`](RejectReason::Binding), and every other code →
//!   [`Other`](RejectReason::Other) (conservative — an unknown code is never tallied as
//!   Syntax). This requires the m3 oracle's [code-carrying reject path](MySqlOracle::wire_verdict).
//! - **A syntax reject is the correctness signal — pinned per corpus.** *We accept ∧
//!   MySQL syntax-rejects* is a real validator-correctness divergence (schema-independent,
//!   so no schema masks or causes it). The first classified run minted **113** of them
//!   (sqlglot 90 / sqllogictest 15 / sqlglot-complex 8) — the broad multi-dialect grammar
//!   accepting syntax MySQL lacks (`CAST(a AS INT)`, 3-part names, `COUNT()`, `FETCH FIRST`,
//!   `WITHIN GROUP`, `ARRAY()`, `GENERATED … AS IDENTITY`, …), the same class as the SQLite
//!   sweep's 133. They are accounted two ways, mirroring the SQLite sweep: singular cases go
//!   in the exact-SQL ledger [`MYSQL_DIVERGENCE_ALLOWLIST`] (the PG-ledger clone), and the
//!   multi-dialect *bulk* is pinned at corpus granularity via each corpus's
//!   [`Corpus::pinned_over_accept`], owned by [`MYSQL_OVER_ACCEPTANCE_TICKET`]. Either way a
//!   NEW over-acceptance drifts a pin and fails; a family tightened in the parser drifts it
//!   the other way — nothing stays silently allowlisted (the restored anti-regression floor).
//! - **The binding residual is counted, not silenced (the STOP fallback).** The corpora
//!   reference thousands of arbitrary multi-dialect identifiers against a bare connection
//!   with no database selected, so their name-resolution rejects stay a counted, pinned
//!   residual — never a synthesized "clean" schema that would fake unblinding. A
//!   `sqllogictest` positional replay onto a scratch database (the sqlite
//!   `SelfContainedReplay` analogue) is a deferred OPTIONAL phase — it would *execute*
//!   corpus-derived DDL server-side, which the never-execute contract forbids without
//!   sign-off — so every corpus stays bare here (`schema_accepts` mirrors `bare_accepts`),
//!   and `newly_comparable` is structurally 0.
//! - **Coverage gaps stay a green-by-counts inventory.** *MySQL accepts ∧ we reject* is the
//!   expected residual for the still-growing dialect; per-corpus counts are pinned and the
//!   full inventory is printed on every run.
//!
//! The mid-sweep oracle-death abort is load-bearing and preserved intact: every wire call
//! flows through [`verdict_or_abort`], so a dying oracle aborts the sweep at the first
//! affected statement instead of tallying its per-statement connection errors as garbage
//! rejects (see the oracle liveness contract).

use crate::m3::{MYSQL_SCHEMA_SETUP_SQL, MySqlOracle, OracleConnectionLost, WireVerdict};
use crate::oracle::OracleUnavailable;
use crate::verdict_harness::{
    Cell, DivergenceEntry, GapClass, Probe, Quadrant, RejectReason, Verdict,
    assert_entries_are_ticketed, assert_entries_still_diverge, check_probe_group,
    sqlglot_complex_statements, sqlglot_identity_lines, sqllogictest_lines, ticket_exists,
};
use squonk::dialect::MySql;
use squonk::parse_with;

/// Schema-independent probes (no object names): comparable against a bare
/// connection. Weighted toward the MySQL-only surface the umbrella ticket lists.
const BARE_PROBES: &[Probe] = &[
    // -- Controls: MySQL-distinctive syntax the fitted preset already parses. --
    Probe {
        sql: "SELECT 1 XOR 0",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 3 DIV 2",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 < 2 < 3",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 0x1F + 1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 LIMIT 5, 10",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 AS `x`",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT @user_var",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT @@version_comment",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT _utf8mb4'x'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT N'x'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT X'1F'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT B'101'",
        class: GapClass::Control,
    },
    // MySQL folds the vertical tab (`0x0b`, `\v`) as ignorable whitespace — its flex
    // `space` set is `[ \t\n\r\f\v]` — so `SELECT\x0b1` prepares as `SELECT 1`. The fitted
    // preset now carries `0x0b` in its whitespace class (mysql-whitespace-vertical-tab,
    // `MYSQL_BYTE_CLASSES`), flipping this from a lexical under-acceptance to a control.
    // The oracle side is the live regression: `check_probe_group` asserts mysql:8 prepares
    // it AND the fitted preset accepts it (Control ⟺ we accept).
    Probe {
        sql: "SELECT\u{0b}1",
        class: GapClass::Control,
    },
    // Same-line adjacent string literals concatenate in MySQL, parsed via
    // `StringLiteralSyntax::same_line_adjacent_concat` — a control. MySQL lexes `"…"` as a
    // string, so a single/double mix concatenates too (`'a' "b"` → `'ab'`).
    Probe {
        sql: "SELECT 'a' 'b'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 'a' \"b\"",
        class: GapClass::Control,
    },
    // A bare (`AS`-less) string in projection-alias position is the column name, parsed via
    // `SelectSyntax::bare_alias_string_literals` (mysql-bare-string-alias-vs-adjacent-concat)
    // — a control. It only reaches the alias branch after a NON-string expression, so it
    // does not collide with the concatenation above.
    Probe {
        sql: "SELECT 1 'x'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 \"x\"",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT GROUP_CONCAT('a')",
        class: GapClass::Control,
    },
    // The NULL-safe equality operator (`<=>`): parsed as `IsNotDistinctFrom` under the
    // `null_safe_equals` tokenizer gate — a control.
    Probe {
        sql: "SELECT 1 <=> NULL",
        class: GapClass::Control,
    },
    // GROUP_CONCAT's ORDER BY/SEPARATOR argument tails — parsed via the `FunctionCall.separator`
    // field + gate, so both are controls.
    Probe {
        sql: "SELECT GROUP_CONCAT('a' SEPARATOR ',')",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT GROUP_CONCAT(1 ORDER BY 1 SEPARATOR ',')",
        class: GapClass::Control,
    },
    // Versioned comments (`/*!NNNNN … */`) are conditional inclusion — the server
    // executes the body — lexed as live input under the fitted preset via
    // `CommentSyntax::versioned_comments` + `nested_block_comments`, so the whole family are
    // controls. Every form below was verified to PREPARE (with the expected value) on live mysql:8.
    Probe {
        sql: "SELECT /*!40101 1 */",
        class: GapClass::Control,
    },
    Probe {
        // The no-version spelling includes unconditionally.
        sql: "SELECT /*! 1 */",
        class: GapClass::Control,
    },
    Probe {
        // Mid-statement inclusion: the region is a token-level seam, not a
        // statement wrapper (evaluates to 2 on the server).
        sql: "SELECT 1 /*!50503 + 1 */",
        class: GapClass::Control,
    },
    Probe {
        // The flagship real-world form: a SELECT modifier only newer servers see.
        sql: "SELECT /*!50000 STRAIGHT_JOIN */ 1",
        class: GapClass::Control,
    },
    Probe {
        // A version above the modelled 8.4 bound is DISCARDED like the engine
        // discards it — the deliberately malformed body proves the region is
        // skipped, not lexed (the server prepares this as `SELECT 2`).
        sql: "SELECT /*!99999 1 + */ 2",
        class: GapClass::Control,
    },
    Probe {
        // An inner plain comment consumes its own terminator; the region
        // continues to the next `*/` (evaluates to 2 on the server).
        sql: "SELECT /*!40101 1 /* c */ + 1 */",
        class: GapClass::Control,
    },
    Probe {
        // MySQL block comments do not nest: the first `*/` closes, so the inner
        // `/*` is comment text and the `1` is live (a nesting scanner would
        // reject this as unterminated — the sibling half of the comment-shape fix).
        sql: "SELECT /* a /* b */ 1",
        class: GapClass::Control,
    },
    // The bitwise-operator family (`bitwise-operators-cross-dialect-gap`,
    // `OperatorSyntax::bitwise_operators`) — the fitted preset parses these, so they are
    // controls, verified by the class-agreement assert in `mysql_probes_match_recorded_class`.
    Probe {
        sql: "SELECT 1 & 2",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 | 2",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 << 2",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT ~1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT 1 ^ 2",
        class: GapClass::Control,
    },
    // KILL and the DESCRIBE/DESC EXPLAIN synonyms — parsed via the
    // `utility_syntax.kill`/`describe` gates + the `KillStatement`/`ExplainStatement`
    // spelling tag, so they are controls.
    Probe {
        sql: "KILL 5",
        class: GapClass::Control,
    },
    Probe {
        sql: "KILL CONNECTION 5",
        class: GapClass::Control,
    },
    Probe {
        sql: "KILL QUERY '123'",
        class: GapClass::Control,
    },
    Probe {
        sql: "DESCRIBE SELECT 1",
        class: GapClass::Control,
    },
    Probe {
        sql: "DESC SELECT 1",
        class: GapClass::Control,
    },
    // The dedicated window functions (`mysql-reserved-window-function-names`): reserved
    // words now admitted as call heads with their mandatory `OVER`. The five
    // rank/distribution functions take no arguments and `NTILE` exactly one, so these forms
    // need no schema — they flip from a pre-existing over-rejection to controls (the fitted
    // preset now parses them; the class-agreement assert proves it against live mysql:8).
    Probe {
        sql: "SELECT ROW_NUMBER() OVER ()",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT RANK() OVER ()",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT DENSE_RANK() OVER ()",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT PERCENT_RANK() OVER ()",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CUME_DIST() OVER ()",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT NTILE(4) OVER ()",
        class: GapClass::Control,
    },
    // The faithful MySQL `cast_type` production (mysql-faithful-cast-type-production): the
    // extended cast targets that parse as user-defined names — `YEAR` (8.0.22+) and the
    // spatial types (8.0.17+) — plus the `SIGNED`/`UNSIGNED [INTEGER]` inert-tail spelling,
    // now all parsed by the fitted preset, so they flip from over-rejections to controls.
    // The `NULL` source is what lets the spatial casts fully PREPARE on a bare connection
    // (an `x`/int source only reaches a binding/semantic reject, which `check_probe_group`
    // treats as a non-accept); the class-agreement assert proves the preset accepts each.
    Probe {
        sql: "SELECT CAST(NULL AS YEAR)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS POINT)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS GEOMETRYCOLLECTION)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(1 AS SIGNED INTEGER)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(1 AS UNSIGNED INTEGER)",
        class: GapClass::Control,
    },
    // The MySQL character-set type annotation on a `CHAR`/`CHARACTER` cast target
    // (mysql-char-charset-annotation): the `opt_charset_with_opt_binary` production —
    // `CHARACTER SET <name>`, the `CHARSET` synonym, the `ASCII`/`UNICODE`/`BYTE`
    // shortcuts, and the trailing `BINARY` collation modifier in either order — now parsed
    // by the fitted preset onto the `DataType::Character` node, so they flip from
    // over-rejections to controls. Engine-verified to PREPARE on mysql:8.4; the `NULL`
    // source keeps each schema-independent. (`COLLATE` is rejected in cast position — a
    // column attribute, not part of `cast_type` — so it is not a probe here.)
    Probe {
        sql: "SELECT CAST(NULL AS CHAR(5) CHARACTER SET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR(5) CHARSET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR ASCII)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR UNICODE)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR BYTE)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR(5) BINARY)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR BINARY CHARACTER SET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT CAST(NULL AS CHAR CHARACTER SET utf8mb4 BINARY)",
        class: GapClass::Control,
    },
    // The MySQL stored-routine catalogue listing `SHOW {FUNCTION | PROCEDURE} STATUS [LIKE |
    // WHERE]` (mysql-show-function-status): the fitted preset now parses it onto
    // `ShowTarget::RoutineStatus` under `show_routine_status`, so the accept forms flip from
    // over-rejections to controls. Engine-probed on mysql:8: all six PREPARE (Accept), while
    // `SHOW FUNCTION STATUS FROM db`, bare `SHOW FUNCTIONS`, and bare `SHOW PROCEDURE` all
    // `ER_PARSE_ERROR` — so there is no `{FROM | IN}` qualifier and the object keyword plus
    // `STATUS` are mandatory. The class-agreement assert (`check_probe_group`) proves the
    // preset accepts each ⟺ mysql:8 prepares it.
    Probe {
        sql: "SHOW FUNCTION STATUS",
        class: GapClass::Control,
    },
    Probe {
        sql: "SHOW PROCEDURE STATUS",
        class: GapClass::Control,
    },
    Probe {
        sql: "SHOW FUNCTION STATUS LIKE 'a%'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SHOW PROCEDURE STATUS LIKE 'a%'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SHOW FUNCTION STATUS WHERE Db = 'x'",
        class: GapClass::Control,
    },
    Probe {
        sql: "SHOW PROCEDURE STATUS WHERE Db = 'x'",
        class: GapClass::Control,
    },
];

/// The sweep's schema: m3's base tables plus the index / partition fixtures the
/// hint and partition-selection probes need. `IF NOT EXISTS` keeps re-runs
/// idempotent (the setup driver auto-commits on a real server); the index rides the
/// table DDL because MySQL has no `CREATE INDEX IF NOT EXISTS`.
const MYSQL_SWEEP_SETUP_SQL: &str = "CREATE DATABASE IF NOT EXISTS squonk_oracle; \
     USE squonk_oracle; \
     CREATE TABLE IF NOT EXISTS t1(a INTEGER, b INTEGER, c INTEGER, d INTEGER, e INTEGER); \
     CREATE TABLE IF NOT EXISTS t2(f INTEGER, g VARCHAR(255)); \
     CREATE TABLE IF NOT EXISTS ft1(a TEXT, b TEXT, FULLTEXT ftidx(a, b)); \
     CREATE TABLE IF NOT EXISTS th(a INTEGER, b INTEGER, KEY idx_a (a)); \
     CREATE TABLE IF NOT EXISTS tp(a INTEGER) PARTITION BY HASH(a) PARTITIONS 4";

/// Schema-dependent probes compared behind the setup driver.
const SETUP_DRIVEN_PROBES: &[Probe] = &[
    // -- Controls. --
    Probe {
        sql: "SELECT t1.a FROM t1 STRAIGHT_JOIN t2 ON t1.a = t2.f",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM t1 GROUP BY a WITH ROLLUP",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT INTO t1(a) VALUES (1) ON DUPLICATE KEY UPDATE a = VALUES(a)",
        class: GapClass::Control,
    },
    Probe {
        sql: "REPLACE INTO t1(a) VALUES (1)",
        class: GapClass::Control,
    },
    // Index hints: a table-factor tail family (USE/FORCE/IGNORE INDEX|KEY) — parsed via
    // `TableExpressionSyntax::index_hints` + the `TableFactor::Table.index_hints` field, so
    // all three are controls.
    Probe {
        sql: "SELECT a FROM th USE INDEX (idx_a) WHERE a = 1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM th FORCE INDEX (idx_a) WHERE a = 1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM th IGNORE INDEX (idx_a) WHERE a = 1",
        class: GapClass::Control,
    },
    // Locking clauses: a query-tail family (FOR UPDATE/SHARE [OF …] and the legacy
    // LOCK IN SHARE MODE spelling) — parsed via the canonical `Query.locking` clause +
    // `QueryTailSyntax::locking_clauses`, so they are controls.
    Probe {
        sql: "SELECT a FROM t1 FOR UPDATE",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM t1 FOR SHARE",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM t1 FOR UPDATE OF t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT a FROM t1 LOCK IN SHARE MODE",
        class: GapClass::Control,
    },
    // INSERT … SET assignment form: parsed by the fitted preset.
    Probe {
        sql: "INSERT INTO t1 SET a = 1, b = 2",
        class: GapClass::Control,
    },
    // The 8.0.19+ ODKU row alias replacing VALUES() — parsed via `Insert.row_alias`, so a control.
    Probe {
        sql: "INSERT INTO t1(a) VALUES (1) AS new_row ON DUPLICATE KEY UPDATE a = new_row.a",
        class: GapClass::Control,
    },
    // UPDATE/DELETE ORDER BY + LIMIT tails — parsed via `mutation_syntax.update_delete_tails`
    // + the `order_by`/`limit` fields.
    Probe {
        sql: "UPDATE t1 SET a = 1 ORDER BY a LIMIT 1",
        class: GapClass::Control,
    },
    Probe {
        sql: "DELETE FROM t1 ORDER BY a LIMIT 1",
        class: GapClass::Control,
    },
    // Partition selection on a table factor — parsed via
    // `TableExpressionSyntax::partition_selection` + the `TableFactor::Table.partition`
    // field, so a control.
    Probe {
        sql: "SELECT a FROM tp PARTITION (p0)",
        class: GapClass::Control,
    },
    // Generated-column DDL: the GENERATED ALWAYS long form and MySQL's keywordless
    // `col type AS (expr)` shorthand both parse, the shorthand via
    // `schema_change_syntax.generated_column_shorthand` + the spelling tag, so a control.
    Probe {
        sql: "CREATE TABLE tg_short (a INT, b INT AS (a + 1) STORED)",
        class: GapClass::Control,
    },
    // Integer display width `(M)` on a built-in integer — parsed via
    // `TypeNameSyntax::integer_display_width` + the `display_width` field on the integer
    // variants, so these are controls.
    // The width binds to the inner integer; the `UNSIGNED ZEROFILL` postfix wraps it via
    // the existing numeric-modifier node. MySQL deprecated the width in 8.0.17+ (save
    // `TINYINT(1)`) but still parses it — the live probe re-verifies the server accepts
    // each when the CI `mysql:8` returns (PREPARE-only, so no table is created).
    Probe {
        sql: "CREATE TABLE tw_iw (a INT(11))",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tw_iw_mods (a INT(10) UNSIGNED ZEROFILL)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tw_iw_tiny (a TINYINT(1))",
        class: GapClass::Control,
    },
    // The MySQL `{DESCRIBE|DESC|EXPLAIN} <table>` table-metadata overload — parsed via
    // `utility_syntax.describe` + the `DescribeStatement` node, so a control. Setup-driven because the server
    // rejects it on a bare connection ("no database selected").
    Probe {
        sql: "DESCRIBE t1",
        class: GapClass::Control,
    },
    // The value/offset window functions (`mysql-reserved-window-function-names`): reserved
    // call heads whose fixed arity takes a value argument, so they bind a real column and
    // are setup-driven. `LEAD`/`LAG` carry the optional `, offset [, default]` tail;
    // `NTH_VALUE` takes exactly two. The fitted preset admits the head and enforces the window
    // grammar, so they are controls.
    Probe {
        sql: "SELECT LEAD(a) OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT LEAD(a, 2, 0) OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT LAG(a, 1) OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT FIRST_VALUE(a) OVER (ORDER BY b) FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT LAST_VALUE(a) OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT NTH_VALUE(a, 2) OVER () FROM t1",
        class: GapClass::Control,
    },
    // The window-function post-`)` tail (`mysql-window-function-tail-grammar`): the
    // null-treatment window functions admit `RESPECT NULLS` after the argument `)`, and
    // `NTH_VALUE` additionally admits `FROM FIRST` (before the null treatment). These flip
    // from pre-existing over-rejections to controls — the fitted preset now parses the tail
    // in the post-`)` position, and the class-agreement assert proves it against live
    // mysql:8. (`IGNORE NULLS` and `FROM LAST` stay mutual rejects — a 1235 feature reject
    // on the server, a parse reject in the preset — so they are not probes.)
    Probe {
        sql: "SELECT LEAD(a) RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT LAG(a) RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT FIRST_VALUE(a) RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT LAST_VALUE(a) RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT NTH_VALUE(a, 2) RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT NTH_VALUE(a, 2) FROM FIRST OVER () FROM t1",
        class: GapClass::Control,
    },
    Probe {
        sql: "SELECT NTH_VALUE(a, 2) FROM FIRST RESPECT NULLS OVER () FROM t1",
        class: GapClass::Control,
    },
    // The MySQL character-set type annotation in column-definition position
    // (mysql-char-charset-annotation): the same `opt_charset_with_opt_binary` annotation the
    // cast-target controls exercise, on every string-typed column the probes proved admits
    // it — `CHAR`/`VARCHAR`/`CHARACTER` (and their `VARYING` spellings), the TEXT LOB size
    // family, and `ENUM`/`SET` — now parsed by the fitted preset onto the shared
    // `CharsetAnnotation` node. Setup-driven because a bare connection rejects
    // `CREATE TABLE` for "no database selected"; PREPARE-only, so no table is created. (The
    // free-floating `COLLATE` column attribute MySQL also accepts is a separate feature on
    // a separate node — the ticket's noted residual — so it is not a control here.)
    Probe {
        sql: "CREATE TABLE tcs (c CHAR(5) CHARACTER SET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c VARCHAR(5) ASCII)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c CHARACTER(5) UNICODE)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c CHAR(5) BINARY)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c CHAR VARYING(5) CHARACTER SET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c VARCHAR(5) BYTE)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c TEXT CHARACTER SET utf8mb4 BINARY)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c TINYTEXT ASCII)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c LONGTEXT BINARY)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c ENUM('a') CHARACTER SET utf8mb4)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE TABLE tcs (c SET('a') BYTE)",
        class: GapClass::Control,
    },
];

/// Map a MySQL server error CODE onto the shared [`RejectReason`] trichotomy. MySQL is the
/// one engine whose wire protocol delivers coded error packets, so the split is read from
/// the CODE (authoritative), never a message-string heuristic. Conservative by
/// construction: only `ER_PARSE_ERROR` is [`Syntax`](RejectReason::Syntax); the
/// name-resolution / already-exists / does-not-exist family is
/// [`Binding`](RejectReason::Binding); and every OTHER code — including any unknown one —
/// is [`Other`](RejectReason::Other), so an unrecognized code can never be tallied as a
/// syntax over-acceptance. Each mapping is verified against the live `mysql:8` by
/// [`mysql_reject_codes_classify_by_family`].
fn classify_mysql_code(code: u16) -> RejectReason {
    use mysql::ServerError::*;
    if code == ER_PARSE_ERROR as u16 {
        RejectReason::Syntax
    } else if code == ER_NO_SUCH_TABLE as u16          // 1146 unknown table (db selected)
        || code == ER_BAD_FIELD_ERROR as u16           // 1054 unknown column
        || code == ER_NO_DB_ERROR as u16               // 1046 no database selected
        || code == ER_BAD_DB_ERROR as u16              // 1049 unknown database
        || code == ER_UNKNOWN_TABLE as u16             // 1109 unknown table in ...
        || code == ER_TABLE_EXISTS_ERROR as u16        // 1050 table already exists
        || code == ER_BAD_TABLE_ERROR as u16           // 1051 unknown table (DROP)
        || code == ER_DB_CREATE_EXISTS as u16          // 1007 database already exists
        || code == ER_DB_DROP_EXISTS as u16            // 1008 database does not exist
        || code == ER_DUP_FIELDNAME as u16             // 1060 duplicate column name
        || code == ER_DUP_KEYNAME as u16               // 1061 duplicate key name
        || code == ER_NONUNIQ_TABLE as u16
    // 1066 not-unique table/alias
    {
        RejectReason::Binding
    } else {
        RejectReason::Other
    }
}

/// A vendored corpus swept against the bare oracle, with re-baselineable pins.
struct Corpus {
    label: &'static str,
    statements: Vec<&'static str>,
    /// Total swept statements (anti-vanishing).
    pinned_total: usize,
    /// Coverage gaps (MySQL accepts, the fitted preset rejects) — the clean residual
    /// inventory this programme drives to zero. Drift here is the meaningful signal.
    pinned_gaps: usize,
    /// Syntax over-acceptances (we accept, MySQL syntax-rejects) *not* individually
    /// listed in [`MYSQL_DIVERGENCE_ALLOWLIST`] — the multi-dialect backlog owned by
    /// [`MYSQL_OVER_ACCEPTANCE_TICKET`]. Pinned so no NEW over-acceptance can appear
    /// silently, and a family tightened in the parser drifts a pin (ledger staleness at
    /// corpus granularity — closing them one-by-one by exact SQL is impractical: the
    /// sqlglot-complex over-acceptances are 100-line TPC-DS queries).
    pinned_over_accept: usize,
}

fn corpora() -> Vec<Corpus> {
    vec![
        Corpus {
            label: "sqlglot",
            statements: sqlglot_identity_lines(),
            pinned_total: 955,
            pinned_gaps: 0,
            // Zero over-acceptances: every family this corpus exercises is tightened in the
            // fitted `MySql` preset, so a parser loosening drifts this pin. One deliberate,
            // non-obvious call: the standalone unit-less `INTERVAL "is"` is closed because
            // MySQL has no typed interval literal at all
            // (`ExpressionSyntax::typed_interval_literal` off — the operator reader declines
            // the unit-less form and the literal path rejects it), NOT a position gate —
            // mysql:8 also admits `INTERVAL` in `DATE_ADD`-family special-function args and
            // window-frame bounds, indistinguishable from the invalid standalone position in
            // a general parser, so a position gate would over-REJECT valid MySQL.
            pinned_over_accept: 0,
        },
        Corpus {
            label: "sqllogictest",
            statements: sqllogictest_lines(),
            pinned_total: 373,
            pinned_gaps: 0,
            // Zero over-acceptances. The non-obvious case: MySQL's default `IGNORE_SPACE`-off
            // tokenizer demotes a spaced built-in aggregate to a general call where the
            // aggregate-only argument forms (`*`, leading `DISTINCT`/`ALL`, aggregate
            // `ORDER BY`/`SEPARATOR`) are illegal
            // (`AggregateCallSyntax::aggregate_args_require_adjacent_paren`, a parser-lookahead
            // adjacency check — NOT a tokenizer change, so a normal-arg spaced call like
            // `count (1)` still parses and stays a binding residual).
            pinned_over_accept: 0,
        },
        Corpus {
            label: "sqlglot-complex",
            statements: sqlglot_complex_statements(),
            pinned_total: 238,
            pinned_gaps: 0,
            // Zero over-acceptances: the `FULL OUTER JOIN` (MySQL has only `LEFT`/`RIGHT` outer
            // joins), base-table column-list alias (`t1 AS t1(x, y)`), and bare
            // `OFFSET`-without-`LIMIT` scalar-subquery operand forms this corpus exercises are
            // all tightened, so a parser loosening drifts this pin.
            pinned_over_accept: 0,
        },
    ]
}

/// Minimum distinct statements the sweep covers (umbrella target).
const MIN_SWEPT: usize = 1_000;

/// Whether the fitted MySQL preset accepts `sql`.
fn ours_accepts(sql: &str) -> bool {
    parse_with(sql, squonk::ParseConfig::new(MySql)).is_ok()
}

/// Turn a liveness-checked wire verdict into a trustworthy [`WireVerdict`] — or abort the
/// WHOLE sweep if the oracle connection died mid-sweep. A partial sweep must never reach
/// the pinned asserts: a dying wire oracle mints plausible-looking garbage (the 2026-07
/// incident tallied a disk-full `mysql:8`'s per-statement connection errors as rejects,
/// "measuring" a clean gap/shadow delta from pure container death). `locator` names where
/// the death struck (corpus + statement index, or the probe group) so the panic is
/// actionable. Start-of-sweep liveness is a clean skip; this mid-sweep death is the loud
/// abort — the distinction is the whole point (see
/// the oracle liveness contract).
fn verdict_or_abort(
    verdict: Result<WireVerdict, OracleConnectionLost>,
    locator: impl std::fmt::Display,
    sql: &str,
) -> WireVerdict {
    match verdict {
        Ok(verdict) => verdict,
        Err(OracleConnectionLost(cause)) => panic!(
            "mysql oracle died mid-sweep at {locator} ({sql:?}): {cause}\n\
             aborting the sweep — a dying wire oracle must never tally garbage verdicts. \
             See the oracle liveness documentation",
        ),
    }
}

/// Whether the live server accepts `sql`, aborting the sweep on mid-sweep death (see
/// [`verdict_or_abort`]). Used by the probe groups, which need only the accept bool.
fn accepts_or_abort(
    verdict: Result<WireVerdict, OracleConnectionLost>,
    locator: impl std::fmt::Display,
    sql: &str,
) -> bool {
    verdict_or_abort(verdict, locator, sql).accepts()
}

/// Whether the live MySQL server accepts `sql`, aborting the sweep if the connection died.
/// Every probe wire query flows through here, so a mid-sweep oracle death is caught at the
/// first affected statement instead of being folded into a `false` (reject) verdict.
fn mysql_accepts(oracle: &MySqlOracle, locator: impl std::fmt::Display, sql: &str) -> bool {
    accepts_or_abort(oracle.wire_verdict(sql), locator, sql)
}

/// Locates a swept statement for the liveness-abort message (`"<corpus> #<index>"`).
/// Formats lazily — only when a death actually panics — so the happy path pays no
/// per-statement string allocation.
struct SweepLocation<'a> {
    corpus: &'a str,
    index: usize,
}

impl std::fmt::Display for SweepLocation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} #{}", self.corpus, self.index)
    }
}

/// Bind `$name` to a reachable oracle, or skip: the wire connection is
/// infrastructure, so no server (local dev) is a skip, never a failure — the
/// nightly CI guard greps for the `skipping mysql` marker to prove the enforced
/// run actually ran.
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

// --- Over-acceptance ledger (the shared PG-ledger pattern, exact SQL) ------------------

/// Current MySQL over-acceptances allowed by the gate (exact SQL, staleness enforced) —
/// each a [`DivergenceEntry`]: a statement our fitted `MySql` surface accepts that MySQL
/// *syntax*-rejects, a real validator-correctness divergence we knowingly tolerate (fixing
/// it is parser-crate tightening, outside this conformance ticket). The escape hatch for
/// individually-triaged singular cases; the multi-dialect *bulk* (113 at this baseline) is
/// instead accounted at corpus granularity via each corpus's [`Corpus::pinned_over_accept`],
/// owned by [`MYSQL_OVER_ACCEPTANCE_TICKET`], because exact-SQL-listing 100-line TPC-DS
/// queries is impractical (the DuckDB family-count discipline for a large divergence set).
pub const MYSQL_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = MYSQL_DIVERGENCE_ALLOWLIST_ENTRIES;

/// The tracking ticket that owns the multi-dialect syntax over-acceptance backlog the
/// at-scale gate baselined (the fitted `MySql` preset admits syntax MySQL rejects;
/// tightening it is parser-crate work, out of this conformance ticket's scope). Every
/// corpus's [`Corpus::pinned_over_accept`] count is "allowlisted" by this ticket — a closed
/// family drifts a pin and forces a reviewed re-baseline, so nothing stays silently
/// allowlisted.
const MYSQL_OVER_ACCEPTANCE_TICKET: &str = "mysql-preset-over-acceptance-residual";

/// The at-scale parity gate: over-acceptance is accounted (per-corpus pinned + exact-SQL
/// allowlisted), coverage gaps stay a green-by-counts inventory, and the binding residual is
/// counted + pinned (the reject-reason split's win). The full over-acceptance and
/// coverage-gap inventories are printed so the child tickets can re-derive them from the log
/// on every run.
#[test]
fn mysql_corpus_parity_over_bare_oracle() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let corpora = corpora();

    let mut total = 0usize;
    let mut quad = Quadrant::default();
    let mut allowlisted_over_accept = 0usize; // in the exact-SQL MYSQL_DIVERGENCE_ALLOWLIST
    let mut over_accept_other_samples: Vec<(&str, u16)> = Vec::new();
    let mut per_corpus_gap: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut per_corpus_over_accept: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut inventory: Vec<(&str, &str)> = Vec::new(); // (corpus, gap sql)
    let mut over_accept_list: Vec<(&str, &str)> = Vec::new(); // (corpus, over-accepted sql)

    // Each corpus is swept in ONE pass so the detail print below needs no second oracle
    // round-trip (which would both waste ~1,500 wire queries and reopen the
    // mid-sweep-death window).
    for corpus in &corpora {
        assert_eq!(
            corpus.statements.len(),
            corpus.pinned_total,
            "{} corpus statement count changed; if intentional, update the pin",
            corpus.label,
        );
        total += corpus.statements.len();

        for (index, &sql) in corpus.statements.iter().enumerate() {
            let locator = SweepLocation {
                corpus: corpus.label,
                index,
            };
            let wire = verdict_or_abort(oracle.wire_verdict(sql), locator, sql);
            let bare_accepts = wire.accepts();
            let bare_reason = match wire {
                WireVerdict::Accept => RejectReason::Other,
                WireVerdict::Reject(code) => classify_mysql_code(code),
            };
            let v = Verdict {
                ours: ours_accepts(sql),
                bare_accepts,
                schema_accepts: bare_accepts,
                bare_reason,
            };
            match quad.record(&v) {
                Cell::CoverageGap => {
                    *per_corpus_gap.entry(corpus.label).or_default() += 1;
                    inventory.push((corpus.label, sql));
                }
                Cell::OverAcceptSyntax => {
                    if MYSQL_DIVERGENCE_ALLOWLIST.iter().any(|e| e.sql == sql) {
                        allowlisted_over_accept += 1;
                    } else {
                        *per_corpus_over_accept.entry(corpus.label).or_default() += 1;
                        over_accept_list.push((corpus.label, sql));
                    }
                }
                Cell::OverAcceptOther if over_accept_other_samples.len() < 20 => {
                    if let WireVerdict::Reject(code) = wire {
                        over_accept_other_samples.push((sql, code));
                    }
                }
                _ => {}
            }
        }
    }

    total += BARE_PROBES.len() + SETUP_DRIVEN_PROBES.len();

    let Quadrant {
        agree_accept,
        agree_reject_syntax,
        agree_reject_binding,
        coverage_gap,
        over_accept_syntax,
        over_accept_binding,
        over_accept_other,
        newly_comparable,
        comparable,
    } = quad;
    let residual = total - comparable;

    eprintln!("\n=== MySQL parity gate (fitted MySql vs MySqlOracle, bare wire oracle) ===");
    eprintln!("  total statements (incl. probes)  {total}");
    eprintln!("  agree accept (A/A)               {agree_accept}");
    eprintln!("  agree reject syntax (R/R)        {agree_reject_syntax}   <- mutual syntax reject");
    eprintln!(
        "  agree reject binding (R/R)       {agree_reject_binding}   <- masked residual (binding/other)"
    );
    eprintln!(
        "  COVERAGE GAP (R/A)               {coverage_gap}   <- MySQL syntax we reject (inventory)"
    );
    eprintln!(
        "  over-accept SYNTAX (A/R)         {over_accept_syntax}   <- REAL over-acceptance ({allowlisted_over_accept} exact-SQL allowlisted, rest corpus-pinned)"
    );
    eprintln!(
        "  over-accept binding (A/R)        {over_accept_binding}   <- residual (schema miss / no db)"
    );
    eprintln!(
        "  over-accept other  (A/R)         {over_accept_other}   <- semantic reject (not syntax; not ledgered)"
    );
    eprintln!("  comparable                       {comparable} / {total}  (residual {residual})");
    eprintln!(
        "  newly comparable vs bare         {newly_comparable}   <- 0: no provisioning (phase-2 replay deferred)"
    );
    eprintln!("\n  coverage gaps / over-acceptances by corpus:");
    for corpus in &corpora {
        let gap = per_corpus_gap.get(corpus.label).copied().unwrap_or(0);
        let over = per_corpus_over_accept
            .get(corpus.label)
            .copied()
            .unwrap_or(0);
        eprintln!(
            "    {:<16} gap {gap:>4} (pin {})   over-accept {over:>4} (pin {})",
            corpus.label, corpus.pinned_gaps, corpus.pinned_over_accept,
        );
    }

    if !over_accept_other_samples.is_empty() {
        eprintln!("\n  over-accept OTHER samples (verify none are hidden syntax rejects):");
        for (sql, code) in &over_accept_other_samples {
            eprintln!("    [ER {code}] {sql:?}");
        }
    }

    // Full over-acceptance list — the backlog MYSQL_OVER_ACCEPTANCE_TICKET owns, printed
    // so it is re-derivable from the test log (never a loose note).
    eprintln!(
        "\n  over-acceptance backlog ({}) [{MYSQL_OVER_ACCEPTANCE_TICKET}]:",
        over_accept_list.len()
    );
    for (corpus, sql) in &over_accept_list {
        eprintln!("    A/R-syntax [{corpus}] {sql:?}");
    }

    // Full coverage-gap inventory — the printed block the child tickets classify.
    eprintln!("\n  coverage-gap inventory ({}):", inventory.len());
    for (corpus, sql) in &inventory {
        eprintln!("    GAP [{corpus}] {sql:?}");
    }

    // --- GATE 1: over-acceptance is fully accounted (zero UNaccounted) ---
    // Every "we accept ∧ MySQL syntax-rejects" is either an exact-SQL allowlist entry
    // (staleness-checked in its own test) or counted against its corpus's pin, owned by the
    // tracking ticket. The first classified run minted 113 (sqlglot 90 / sqllogictest 15 /
    // sqlglot-complex 8) — the broad multi-dialect grammar accepting syntax MySQL lacks
    // (`CAST(a AS INT)`, 3-part names, `COUNT()`, `FETCH FIRST`, `WITHIN GROUP`, `ARRAY()`,
    // `GENERATED … AS IDENTITY`, …), the same class as the SQLite sweep's 133. A NEW
    // over-acceptance drifts a corpus pin; a family tightened in the parser drifts it the
    // other way — neither can pass silently (the restored anti-regression floor).
    assert!(
        ticket_exists(MYSQL_OVER_ACCEPTANCE_TICKET),
        "over-acceptance backlog ticket {MYSQL_OVER_ACCEPTANCE_TICKET} must exist",
    );
    for corpus in &corpora {
        let got = per_corpus_over_accept
            .get(corpus.label)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            got, corpus.pinned_over_accept,
            "{} over-acceptance count drifted (we accept, MySQL syntax-rejects): a new \
             over-acceptance appeared or the fitted preset tightened one off — triage against \
             {MYSQL_OVER_ACCEPTANCE_TICKET} and re-baseline the pin (or add an exact-SQL \
             MYSQL_DIVERGENCE_ALLOWLIST entry for a singular case)",
            corpus.label,
        );
    }

    // --- GATE 2: coverage gaps stay a green-by-counts inventory (per corpus) ---
    for corpus in &corpora {
        let got = per_corpus_gap.get(corpus.label).copied().unwrap_or(0);
        assert_eq!(
            got, corpus.pinned_gaps,
            "{} coverage-gap count drifted; a MySQL family closed under the fitted preset or a \
             fixture changed — re-baseline the pin and update the child-ticket inventory",
            corpus.label,
        );
    }

    // --- GATE 3: the name-resolution residual (the ticket's classification win) ---
    // These four cells are the honest bare-oracle residual the reject-reason split now
    // isolates from the syntax signal: the corpora carry no vendored schema (synthesizing
    // one for thousands of arbitrary multi-dialect identifiers is intractable — the
    // ticket's STOP rule), so binding rejects stay counted, and `newly_comparable` is
    // structurally 0 (no provisioning; the phase-2 replay is deferred). The syntax
    // over-acceptance is pinned per corpus in GATE 1; the remaining whole-count cells
    // (`agree_reject_syntax`, `agree_accept`, `comparable`, `residual`) stay report-only —
    // printed, not asserted, as they move with the still-open tightening work. A pin drifts
    // if the corpus, engine, or fitted preset changes.
    assert_eq!(
        (
            newly_comparable,
            over_accept_binding,
            over_accept_other,
            agree_reject_binding,
        ),
        (
            NEWLY_COMPARABLE_PIN,
            OVER_ACCEPT_BINDING_PIN,
            OVER_ACCEPT_OTHER_PIN,
            AGREE_REJECT_BINDING_PIN,
        ),
        "name-resolution residual counts drifted (newly_comparable, over_accept_binding, \
         over_accept_other, agree_reject_binding); re-baseline",
    );
    // `agree_accept` / `agree_reject_syntax` / `comparable` / `residual` are printed above
    // for the record but not asserted: they move as the over-acceptance families tighten.

    assert!(
        total >= MIN_SWEPT,
        "sweeps {total} statements, below the {MIN_SWEPT} target",
    );
}

// --- Baselined pins (measured against live mysql:8; deterministic across runs) ----------
//
// The name-resolution residual the reject-reason split isolates. Re-baselineable by design:
// a corpus/fixture tweak or a preset change that moves a binding verdict drifts one and
// fails loudly. The syntax-over-acceptance counts (113: sqlglot 90 / sqllogictest 15 /
// sqlglot-complex 8) are pinned per corpus on `Corpus::pinned_over_accept` (GATE 1) — the
// anti-regression floor that `mysql-preset-over-acceptance-tightening` drives down.
const NEWLY_COMPARABLE_PIN: usize = 0;
// Window-function pins (mysql-reserved-window-function-names): admitting the 11 reserved
// window-function names as call heads lets 34 corpus lines parse that the reserved-head
// reject otherwise masks. Their window calls are valid, but MySQL still rejects each
// downstream for a
// *pre-existing, separately-owned* over-acceptance the admitted head exposes — an unaliased
// derived table `FROM (SELECT 1)` (1248), a positional window `ORDER BY 1`, an unknown table
// (1146) — always a binding/semantic reason, never 1064. So those land in the binding/other
// residual (over_accept_binding, over_accept_other) rather than the mutual-reject column
// (agree_reject_binding). The syntax-over-acceptance floor (GATE 1) holds at 0 per corpus:
// this pass adds no A/R-syntax divergence, only unmasks binding/other residual the
// reject-reason split counts.
//
// `over_accept_binding` (837) also counts the two vendored `CREATE PROCEDURE`/`CREATE FUNCTION`
// routines the fitted preset parses under `parse-mysql-routine-ddl` while MySQL binding-rejects
// each (a missing referent, not 1064): a routine-DDL coverage gain that classifies as
// binding-over-accept, not syntax, so the GATE 1 syntax floor stays 0.
//
// The other movers into these columns share one shape: the fitted preset parses a
// MySQL-specific form (`USE <schema>`, account-based `GRANT`/`REVOKE`, the `CREATE VIEW`
// definition-option prefix `ALGORITHM = …`/`DEFINER = …`/`SQL SECURITY …`, the scope-prefixed
// `SET GLOBAL <var> = <value>` assignment) that the bare PREPARE-only oracle rejects for a
// non-syntax reason — a missing referent, an unknown variable, or `ER_UNSUPPORTED_PS`
// (1295, grammar-positive but not preparable, an "other" reject) — so each classifies as
// binding/other over-acceptance, never 1064, and the GATE 1 syntax floor stays 0. Swept
// `LOAD DATA` corpus lines are Hive `LOAD DATA … INPATH` forms the `INFILE`-only MySQL grammar
// rejects, so they do not move this quadrant. Pins below are measured against the live bare oracle.
const OVER_ACCEPT_BINDING_PIN: usize = 840;
const OVER_ACCEPT_OTHER_PIN: usize = 37;
const AGREE_REJECT_BINDING_PIN: usize = 34;

/// Individually-triaged singular over-acceptances (the PG-ledger clone). Empty at this
/// baseline: the multi-dialect over-acceptance bulk (113) is accounted at corpus granularity
/// via [`Corpus::pinned_over_accept`] under [`MYSQL_OVER_ACCEPTANCE_TICKET`] — exact-SQL-listing
/// 100-line TPC-DS queries is impractical; the machinery here holds the singular,
/// individually-tracked cases.
const MYSQL_DIVERGENCE_ALLOWLIST_ENTRIES: &[DivergenceEntry] = &[];

/// The authored probes: every entry must PREPARE on the live server, and the
/// recorded [`GapClass`] must agree with the fitted preset's verdict (`Control` ⟺
/// already parsed), so the classification cannot silently rot as the parser grows.
#[test]
fn mysql_probes_match_recorded_class() {
    oracle_or_skip!(bare = MySqlOracle::new());
    oracle_or_skip!(provisioned = MySqlOracle::with_schema(MYSQL_SWEEP_SETUP_SQL));

    let mut gaps = 0usize;
    for (label, oracle, probes) in [
        ("bare", &bare, BARE_PROBES),
        ("setup-driven", &provisioned, SETUP_DRIVEN_PROBES),
    ] {
        eprintln!("[{label} mysql probes]:");
        gaps += check_probe_group(
            "mysql",
            probes,
            |sql| mysql_accepts(oracle, format_args!("{label} probe"), sql),
            ours_accepts,
        );
    }
    eprintln!("  probe coverage gaps: {gaps}");
    assert_eq!(
        gaps, PROBE_GAP_PIN,
        "probe gap count drifted; re-baseline and update the child tickets"
    );
}

/// Pinned probe coverage-gap count (probes the fitted preset rejects while the live server
/// prepares them): 0 — every authored probe the live server accepts is parsed by the fitted
/// preset.
const PROBE_GAP_PIN: usize = 0;

/// The code→reason classifier verified against the live server: each statement rejects on a
/// bare connection with a known coded packet, so [`classify_mysql_code`]'s table cannot
/// silently rot.
#[test]
fn mysql_reject_codes_classify_by_family() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    // (sql that rejects on a BARE connection — no database selected, expected reason).
    let cases: &[(&str, RejectReason)] = &[
        ("SELCT 1", RejectReason::Syntax), // ER_PARSE_ERROR 1064
        ("SELECT no_such_col_xyz", RejectReason::Binding), // ER_BAD_FIELD_ERROR 1054
        ("SELECT a FROM no_such_tbl_xyz", RejectReason::Binding), // ER_NO_DB_ERROR 1046 (bare)
        ("SELECT (SELECT 1, 2)", RejectReason::Other), // ER_OPERAND_COLUMNS 1241
    ];
    for (sql, expected) in cases {
        let code = match oracle.wire_verdict(sql).expect("bare probe answers") {
            WireVerdict::Reject(code) => code,
            WireVerdict::Accept => panic!("expected {sql:?} to reject on a bare connection"),
        };
        // `assert!` (not `assert_eq!`): `RejectReason` lives in the shared, read-only
        // `verdict_harness` and derives no `Debug`, so compare via its `PartialEq`.
        assert!(
            classify_mysql_code(code) == *expected,
            "{sql:?} rejected with code {code} but classified to the wrong reject reason",
        );
    }
}

/// The m3 base schema stays a strict prefix of the sweep schema, so the two setup
/// drivers provision compatibly on one server (the sweep only ADDS fixtures).
#[test]
fn sweep_schema_extends_the_m3_schema() {
    assert!(
        MYSQL_SWEEP_SETUP_SQL.starts_with(MYSQL_SCHEMA_SETUP_SQL.trim_end_matches(';')),
        "the sweep setup SQL must extend m3's base schema, not diverge from it",
    );
}

/// Cloned from `pg::PG_DIVERGENCE_ALLOWLIST`: every allowlisted over-acceptance must name a
/// real ticket and still actually diverge (we accept ∧ MySQL syntax-rejects), so a fixed
/// over-acceptance cannot stay silently allowlisted. Vacuous while the ledger is empty, but
/// keeps the machinery in place for the first real entry.
#[test]
fn mysql_divergence_allowlist_entries_name_tickets_and_still_diverge() {
    assert_entries_are_ticketed(MYSQL_DIVERGENCE_ALLOWLIST);
    oracle_or_skip!(oracle = MySqlOracle::new());
    assert_entries_still_diverge(MYSQL_DIVERGENCE_ALLOWLIST, |entry| {
        let ours = ours_accepts(entry.sql);
        match oracle.wire_verdict(entry.sql) {
            Ok(WireVerdict::Reject(code)) => {
                ours && classify_mysql_code(code) == RejectReason::Syntax
            }
            _ => false,
        }
    });
}

/// The recorded round-trip trade for versioned comments (`mysql-versioned-comments`,
/// pre-made in the ticket design): the `/*!NNNNN` / `*/` wrapper is comment TRIVIA —
/// out-of-band by ADR-0005, with the lossless alternative recorded as a deliberate
/// no-go (ADR-0020) — so the render emits the INCLUDED body plainly, dropping the
/// wrapper exactly as every other comment form is dropped. `parse(render(x)) == x`
/// still holds (the reparse sees the already-included body); what is lost is the
/// wrapper *spelling* — its version number stays offset-recoverable through
/// `ParseConfig::capture_trivia`, and byte-level wrapper fidelity is the deferred
/// `prod-render-byte-fidelity-marker-spike` seam.
#[test]
fn versioned_comment_wrapper_is_dropped_on_render_by_recorded_trade() {
    use crate::corpus_roundtrip::{Roundtrip, roundtrip};
    use squonk_ast::render::RenderMode;

    let sql = "SELECT /*!50000 STRAIGHT_JOIN */ 1";
    let parsed =
        parse_with(sql, squonk::ParseConfig::new(MySql)).expect("the versioned body is live input");
    let rendered = crate::render_statements(&parsed, RenderMode::Canonical);
    assert!(
        rendered.contains("STRAIGHT_JOIN") && !rendered.contains("/*!"),
        "the included body renders without the versioned wrapper: {rendered:?}",
    );
    // The structural round-trip contract is unaffected by the wrapper loss.
    assert!(matches!(roundtrip(sql, MySql), Roundtrip::Ok));
}

/// The tally path is untouched by the liveness guard: a trustworthy verdict flows
/// through as its accept bool (accept -> true, reject -> false). Server-free — the
/// verdict is injected — so this runs even with no `mysql:8` reachable.
#[test]
fn accept_and_reject_verdicts_do_not_abort() {
    assert!(accepts_or_abort(
        Ok(WireVerdict::Accept),
        "sqlglot #0",
        "SELECT 1"
    ));
    assert!(!accepts_or_abort(
        // ER_PARSE_ERROR — a trustworthy per-statement reject.
        Ok(WireVerdict::Reject(1064)),
        "sqlglot #0",
        "SELCT 1"
    ));
}

/// The load-bearing behaviour: a mid-sweep connection loss aborts the sweep loudly
/// instead of tallying a garbage reject. Server-free — the [`OracleConnectionLost`] is
/// injected, so no dying container is needed to prove the abort fires (the live path is
/// exercised when the CI `mysql:8` returns). Pairs with m3's classification tests, which
/// prove a dead wire maps to `ConnectionLost` in the first place.
#[test]
#[should_panic(expected = "died mid-sweep")]
fn connection_loss_aborts_the_sweep() {
    let _ = accepts_or_abort(
        Err(OracleConnectionLost("IoError { broken pipe }".to_string())),
        "sqlglot #42",
        "SELECT 1",
    );
}

// =====================================================================================
// Flag-aware generative differential (oracle-parity-mysql)
// =====================================================================================

use crate::properties::dialect_features::{
    MYSQL_FEATURE_PROBES, MYSQL_FEATURE_SEEDS, arb_feature_statement,
};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use squonk::Dialect;

const MYSQL_GENERATIVE_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[];

/// Setup for generative fragments that reference `t` (MySQL PrepareBind needs a DB).
const MYSQL_FEATURE_SCHEMA_SQL: &str = "CREATE DATABASE IF NOT EXISTS squonk_oracle;      USE squonk_oracle;      CREATE TABLE IF NOT EXISTS t (a INTEGER PRIMARY KEY, b TEXT, c INTEGER)";

fn mysql_generative_divergence(
    bare: &MySqlOracle,
    provisioned: &MySqlOracle,
    sql: &str,
) -> Option<String> {
    let ours = parse_with(sql, squonk::ParseConfig::new(MySql)).is_ok();
    let bare_ok = mysql_accepts(bare, "generative-bare", sql);
    let schema_ok = mysql_accepts(provisioned, "generative-schema", sql);
    let theirs = bare_ok || schema_ok;
    if ours == theirs {
        return None;
    }
    if MYSQL_GENERATIVE_DIVERGENCE_ALLOWLIST
        .iter()
        .any(|e| e.sql == sql)
    {
        return None;
    }
    Some(if ours && !theirs {
        format!("over-acceptance: we accept, mysql rejects: {sql:?}")
    } else {
        format!("coverage gap: mysql accepts, we reject: {sql:?}")
    })
}

#[test]
fn mysql_feature_generative_differential_replays_committed_seeds() {
    oracle_or_skip!(bare = MySqlOracle::new());
    oracle_or_skip!(provisioned = MySqlOracle::with_schema(MYSQL_FEATURE_SCHEMA_SQL));
    let divergences: Vec<String> = MYSQL_FEATURE_SEEDS
        .iter()
        .filter_map(|&sql| mysql_generative_divergence(&bare, &provisioned, sql))
        .collect();
    assert!(
        divergences.is_empty(),
        "MySQL generative differential found {} un-ledgered divergence(s):\n  {}",
        divergences.len(),
        divergences.join("\n  "),
    );
}

#[test]
fn mysql_feature_generative_differential_explores_flag_aware_surface() {
    oracle_or_skip!(bare = MySqlOracle::new());
    oracle_or_skip!(provisioned = MySqlOracle::with_schema(MYSQL_FEATURE_SCHEMA_SQL));
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 256,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &[0xC1; 32]),
    );
    let strategy = arb_feature_statement(MySql.features(), MYSQL_FEATURE_PROBES);
    for _ in 0..256 {
        let tree = strategy.new_tree(&mut runner).expect("strategy ok");
        let (_family, sql) = tree.current();
        if let Some(detail) = mysql_generative_divergence(&bare, &provisioned, &sql) {
            panic!("MySQL generative differential: {detail}");
        }
    }
}

#[test]
fn mysql_generative_allowlist_entries_name_tickets_and_still_diverge() {
    assert_entries_are_ticketed(MYSQL_GENERATIVE_DIVERGENCE_ALLOWLIST);
}

// --- Authored top-level statement-family inventory (spec-coverage-mysql-authored-production-inventory)
//
// The MySQL analogue of the PostgreSQL `stmt`-production coverage measurement
// (`corpus_pg_verdicts`), adapted around the GPL licensing boundary. PostgreSQL's
// denominator is `stmt-productions.txt`, extracted by a committed script from the
// permissively-licensed `gram.y`. MySQL's grammar (`sql_yacc.yy`) is GPL and must never be
// vendored or extracted-from into this repo, so the denominator here is instead an ORIGINAL
// authored inventory: the family SET was derived from a local, read-only reading of MySQL
// 8.4.10's `simple_statement` production for the FACT of which top-level families exist (no
// grammar bytes copied), and every probe is original minimal SQL authored in
// `corpus/mysql/families.sql`. The reproducibility story is that documented fact-derivation
// plus the live-oracle probes below — not an extraction script. `corpus/mysql/{README.md,
// PROVENANCE.toml}` record the boundary; the corpus is CC0-1.0 self-authored.
//
// Two axes are tracked SEPARATELY, mirroring the PG probe table's engine-vs-squonk split:
// - Engine reach: every family is verified against the live m3 oracle. Because m3 is
//   PREPARE-only, a grammar-valid family reaches one of three non-syntax wire outcomes —
//   PREPARE (accept), ER_UNSUPPORTED_PS (1295: grammar-valid but not preparable, the large
//   administrative/stored-program surface), or a binding reject of a `zzp_*` placeholder.
//   Only ER_PARSE_ERROR (1064) means "not recognized", which for an authored probe is an
//   inventory bug. So "MySQL recognizes the family" == the outcome is not `Syntax`.
// - squonk reach: `ours_accepts` (the fitted `MySql` preset) per family, partitioned into
//   supported vs the measured, pinned uncovered set. Engine reach does NOT imply squonk
//   implements the family — the uncovered set is the release-blocking coverage residual.

/// The self-authored MySQL top-level statement-family probe corpus (CC0-1.0). One
/// `-- family:` header + one probe line per family; see `corpus/mysql/README.md`.
const MYSQL_STATEMENT_FAMILIES: &str = include_str!("../corpus/mysql/families.sql");

/// Total authored top-level statement families. A measured pin: a probe/header vanishing
/// from `families.sql` (or a new family added) drifts this and forces a reviewed re-baseline.
const MYSQL_STATEMENT_FAMILY_COUNT: usize = 110;

/// Statement families the fitted `MySql` preset does NOT parse — the measured, pinned
/// coverage residual (release-blocker). Engine-recognized (all 110 are) but not yet
/// implemented by squonk; each is a candidate follow-up child. Re-baselined only after a
/// reviewed fresh run: a family the parser gains drops out (drift fails the sweep so it is
/// promoted deliberately, never silently), a regression adds one.
const MYSQL_UNCOVERED_STATEMENT_FAMILIES: &[&str] = &[];

/// Parse `families.sql` into ordered `(family, probe_sql)` pairs. A `-- family:` header names
/// a family; the single following non-comment, non-blank line is its probe. Panics on a
/// malformed corpus (a header with no probe, a probe with no header) so a bad edit fails
/// loudly instead of miscounting.
fn mysql_statement_families() -> Vec<(&'static str, &'static str)> {
    let mut families: Vec<(&'static str, &'static str)> = Vec::new();
    let mut pending: Option<&'static str> = None;
    for line in MYSQL_STATEMENT_FAMILIES.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("-- family:") {
            assert!(
                pending.is_none(),
                "statement family {:?} has no probe before the next header",
                pending.unwrap(),
            );
            pending = Some(name.trim());
        } else if line.is_empty() || line.starts_with("--") {
            continue;
        } else {
            let family = pending.take().unwrap_or_else(|| {
                panic!("probe {line:?} in families.sql has no preceding `-- family:` header")
            });
            families.push((family, line));
        }
    }
    assert!(
        pending.is_none(),
        "trailing statement family {:?} has no probe",
        pending,
    );
    families
}

/// The live MySQL grammar's verdict on one authored family probe, read off the PREPARE-only
/// wire oracle's coded packet. Only [`Syntax`](Self::Syntax) (ER_PARSE_ERROR 1064) means the
/// family is unrecognized; the other three are all positive grammar evidence (see the section
/// header). [`Other`](Self::Other) carries any unexpected code verbatim so it can never be
/// silently folded into a grammar signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FamilyEngineOutcome {
    /// The server PREPAREd it — parse + bind succeeded (PS-supported family).
    Prepared,
    /// ER_UNSUPPORTED_PS (1295): grammar-valid but declined by the PREPARE protocol — the
    /// administrative / stored-program surface. The server parsed it before answering.
    UnsupportedPs,
    /// A binding reject (unknown `zzp_*` object / no database) — parsed, then name resolution
    /// failed.
    Binding,
    /// ER_PARSE_ERROR (1064): not valid MySQL grammar. For an authored probe, an inventory bug.
    Syntax,
    /// Any other coded reject, recorded verbatim. Observed once: `Other(1305)`
    /// (ER_SP_DOES_NOT_EXIST) for `CALL zzp_p()` — a name-resolution reject of the absent
    /// placeholder routine, outside [`classify_mysql_code`]'s conservative binding set, so
    /// recorded honestly as Other rather than folded into a grammar signal.
    Other(u16),
}

/// Classify a PREPARE-only wire verdict into the family-reach outcome. Every outcome except
/// [`Syntax`](FamilyEngineOutcome::Syntax) is positive evidence the MySQL grammar recognizes
/// the family.
fn classify_family_outcome(verdict: WireVerdict) -> FamilyEngineOutcome {
    use mysql::ServerError::{ER_PARSE_ERROR, ER_UNSUPPORTED_PS};
    match verdict {
        WireVerdict::Accept => FamilyEngineOutcome::Prepared,
        WireVerdict::Reject(code) if code == ER_PARSE_ERROR as u16 => FamilyEngineOutcome::Syntax,
        WireVerdict::Reject(code) if code == ER_UNSUPPORTED_PS as u16 => {
            FamilyEngineOutcome::UnsupportedPs
        }
        WireVerdict::Reject(code) if classify_mysql_code(code) == RejectReason::Binding => {
            FamilyEngineOutcome::Binding
        }
        WireVerdict::Reject(code) => FamilyEngineOutcome::Other(code),
    }
}

/// The authored inventory is well-formed and its squonk-coverage residual is pinned.
/// Server-free (no oracle needed), so the release-blocking uncovered-family pin is enforced on
/// every run, not only when a `mysql:8` is reachable.
#[test]
fn mysql_statement_family_inventory_is_authored_and_pinned() {
    use std::collections::BTreeSet;

    let families = mysql_statement_families();
    assert_eq!(
        families.len(),
        MYSQL_STATEMENT_FAMILY_COUNT,
        "authored MySQL statement-family count drifted; review families.sql and re-baseline \
         MYSQL_STATEMENT_FAMILY_COUNT",
    );
    let names: BTreeSet<&str> = families.iter().map(|(family, _)| *family).collect();
    assert_eq!(
        names.len(),
        families.len(),
        "duplicate family name in families.sql (each family is probed exactly once)",
    );

    let uncovered: Vec<&str> = families
        .iter()
        .filter(|(_, sql)| !ours_accepts(sql))
        .map(|(family, _)| *family)
        .collect();
    let covered = families.len() - uncovered.len();
    eprintln!(
        "squonk MySql preset covers {covered}/{} authored statement families ({:.1}%)",
        families.len(),
        100.0 * covered as f64 / families.len() as f64,
    );
    eprintln!("  UNCOVERED (measured, pinned): {uncovered:?}");
    assert_eq!(
        uncovered, MYSQL_UNCOVERED_STATEMENT_FAMILIES,
        "squonk MySQL statement-family coverage drifted; review the covered and uncovered \
         sets before re-baselining MYSQL_UNCOVERED_STATEMENT_FAMILIES",
    );
}

/// Every authored family is verified against the live MySQL oracle: the server must recognize
/// it (no ER_PARSE_ERROR), and the server VERSION() is captured as the "oracle actually ran"
/// evidence. Prints the full per-family engine/squonk classification so the inventory is a
/// reviewable, re-derivable record. Skips cleanly (the `skipping mysql` marker) when no server
/// is reachable — the nightly guard makes that skip impossible where a server is declared.
#[test]
fn mysql_statement_family_inventory_has_live_oracle_probes() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    eprintln!("mysql statement-family inventory: oracle server VERSION() = {version:?}");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    let families = mysql_statement_families();
    let mut prepared = 0usize;
    let mut unsupported_ps = 0usize;
    let mut binding = 0usize;
    let mut other = 0usize;
    let mut syntax_failures: Vec<(&str, &str, WireVerdict)> = Vec::new();
    for (family, sql) in &families {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), *family, sql);
        let outcome = classify_family_outcome(verdict);
        eprintln!(
            "  engine={outcome:?} squonk={:<5} [{family}] {sql:?}",
            ours_accepts(sql),
        );
        match outcome {
            FamilyEngineOutcome::Prepared => prepared += 1,
            FamilyEngineOutcome::UnsupportedPs => unsupported_ps += 1,
            FamilyEngineOutcome::Binding => binding += 1,
            FamilyEngineOutcome::Other(_) => other += 1,
            FamilyEngineOutcome::Syntax => syntax_failures.push((*family, *sql, verdict)),
        }
    }
    eprintln!(
        "MySQL {version} engine reach over {} authored families: prepared={prepared} \
         unsupported_ps={unsupported_ps} binding={binding} other={other} syntax={}",
        families.len(),
        syntax_failures.len(),
    );
    assert!(
        syntax_failures.is_empty(),
        "authored probe(s) syntax-rejected (ER_PARSE_ERROR 1064) by MySQL — fix the inventory \
         SQL so every family is grammar-valid: {syntax_failures:?}",
    );
    assert_eq!(
        prepared + unsupported_ps + binding + other,
        families.len(),
        "engine outcomes must partition the family inventory",
    );
    // Measured engine-reach summary pin (MySQL 8.4.10, m3 PREPARE-only, provisioned schema).
    // The load-bearing invariant is syntax==0 above (every family grammar-recognized); this
    // split is the recorded classification, drifting only if MySQL's PREPARE-protocol coverage
    // changes across a server bump or a probe's binding shape moves — review before
    // re-baselining. `other=1` is CALL's ER_SP_DOES_NOT_EXIST (1305), see FamilyEngineOutcome.
    assert_eq!(
        (prepared, unsupported_ps, binding, other),
        (50, 59, 0, 1),
        "engine-reach summary drifted; re-baseline after review",
    );
}

/// The MySQL `CALL sp_name opt_paren_expr_list` server-side evidence (`parse-mysql-call`): every
/// authored shape is run through the PREPARE-only [`wire_verdict`](MySqlOracle::wire_verdict)
/// channel against a schema-provisioned oracle (so an unqualified name resolves past the no-db
/// stage). A grammar-valid shape must *not* `Reject(ER_PARSE_ERROR=1064)` — it parses, then
/// resolves to the absent routine (`ER_SP_DOES_NOT_EXIST=1305`, a grammar-positive binding
/// reject) — and the fitted `MySql` preset must parse it; a syntax error must `Reject(1064)` and
/// the preset must reject it too. The measured boundary (mysql:8.4.10): the parenthesized argument
/// list is optional (bare `CALL p`, empty `CALL p()`, and filled forms all grammar-accept), the
/// name is at most `db.proc` (a three-part `a.b.c` 1064-rejects), a trailing comma and a bare
/// `SELECT` argument 1064-reject, and a bare `CALL` with no name 1064-rejects. Oracle-mysql-gated;
/// skips cleanly with no server.
#[test]
fn mysql_call_bare_and_parenthesized_forms_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;
    // Grammar-valid shapes: the server parses them (any non-1064 verdict — here 1305 for the
    // absent placeholder routine) and the fitted preset accepts.
    let accepts = [
        "CALL zzp_p",
        "CALL zzp_p()",
        "CALL zzp_p(1, 2)",
        "CALL zzp_p(1 + 2, CONCAT('a', 'b'))",
        "CALL squonk_oracle.zzp_p(1)",
    ];
    for sql in accepts {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "call_accept", sql);
        assert_ne!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must grammar-accept the valid CALL {sql:?} (got {verdict:?})",
        );
        assert!(
            ours_accepts(sql),
            "the fitted MySql preset must parse the valid CALL {sql:?}",
        );
    }
    // Syntax errors: the server 1064-rejects (parsed before binding) and the preset rejects too.
    let rejects = [
        "CALL zzp_p(1,)",
        "CALL zzp_p(,)",
        "CALL zzp_p(SELECT 1)",
        "CALL a.b.c(1)",
        "CALL",
        "CALL (SELECT 1)",
    ];
    for sql in rejects {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "call_reject", sql);
        assert_eq!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must 1064-reject the CALL syntax error {sql:?} (got {verdict:?})",
        );
        assert!(
            !ours_accepts(sql),
            "the fitted MySql preset must reject the CALL syntax error {sql:?}",
        );
    }
    eprintln!(
        "mysql CALL wire_verdict evidence: {} grammar-accepted (1305 absent-routine), {} 1064-rejected \
         (PREPARE-only, schema-provisioned)",
        accepts.len(),
        rejects.len(),
    );
}

/// The MySQL view definition-option surface (`parse-mysql-alter-view`): the shared
/// `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` prefix on `CREATE VIEW`, and the whole
/// `ALTER VIEW` redefinition. Every grammar-valid shape is run through the PREPARE-only
/// [`wire_verdict`](MySqlOracle::wire_verdict) channel: a valid form must *not*
/// `Reject(ER_PARSE_ERROR=1064)` — a `CREATE VIEW` PREPAREs (`Accept`), and an `ALTER VIEW` of a
/// missing view is `ER_UNSUPPORTED_PS=1295` (grammar-positive: the parser accepted it, the
/// PREPARE protocol declines the statement) — and the fitted `MySql` preset must parse it.
///
/// The measured boundary (mysql:8.4.10): the prefix order is fixed (algorithm, then definer,
/// then SQL security — a permutation 1064-rejects), the options precede the `VIEW` keyword (an
/// option after it 1064-rejects), a bad `ALGORITHM` value 1064-rejects, `ALTER` takes neither
/// `OR REPLACE` nor `IF EXISTS`, and `CREATE` admits `OR REPLACE` before the prefix (but not
/// after the algorithm). Oracle-mysql-gated; skips cleanly with no server.
#[test]
fn mysql_view_definition_options_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;
    // Grammar-valid: the server does not 1064-reject (a `CREATE VIEW` PREPAREs; an `ALTER VIEW`
    // of the absent `zzp_v` is 1295), and the fitted preset accepts.
    let accepts = [
        "CREATE ALGORITHM = UNDEFINED VIEW zzp_v AS SELECT 1",
        "CREATE ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "CREATE ALGORITHM = TEMPTABLE VIEW zzp_v AS SELECT 1",
        "CREATE DEFINER = root VIEW zzp_v AS SELECT 1",
        "CREATE DEFINER = 'root'@'localhost' VIEW zzp_v AS SELECT 1",
        "CREATE DEFINER = CURRENT_USER VIEW zzp_v AS SELECT 1",
        "CREATE DEFINER = CURRENT_USER() VIEW zzp_v AS SELECT 1",
        "CREATE SQL SECURITY DEFINER VIEW zzp_v AS SELECT 1",
        "CREATE SQL SECURITY INVOKER VIEW zzp_v AS SELECT 1",
        "CREATE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW zzp_v AS SELECT 1",
        "CREATE OR REPLACE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW zzp_v \
         AS SELECT 1",
        "ALTER VIEW zzp_v AS SELECT 1",
        "ALTER VIEW zzp_v (a) AS SELECT 1",
        "ALTER ALGORITHM = UNDEFINED VIEW zzp_v AS SELECT 1",
        "ALTER ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "ALTER ALGORITHM = TEMPTABLE VIEW zzp_v AS SELECT 1",
        "ALTER DEFINER = root VIEW zzp_v AS SELECT 1",
        "ALTER DEFINER = CURRENT_USER() VIEW zzp_v AS SELECT 1",
        "ALTER SQL SECURITY DEFINER VIEW zzp_v AS SELECT 1",
        "ALTER SQL SECURITY INVOKER VIEW zzp_v AS SELECT 1",
        "ALTER ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW zzp_v AS SELECT 1",
        "ALTER DEFINER = root SQL SECURITY INVOKER VIEW zzp_v AS SELECT 1",
        "ALTER VIEW zzp_v AS SELECT 1 WITH CHECK OPTION",
        "ALTER VIEW zzp_v AS SELECT 1 WITH CASCADED CHECK OPTION",
        "ALTER VIEW zzp_v AS SELECT 1 WITH LOCAL CHECK OPTION",
    ];
    for sql in accepts {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "view_option_accept", sql);
        assert_ne!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must grammar-accept the valid view form {sql:?} (got {verdict:?})",
        );
        assert!(
            ours_accepts(sql),
            "the fitted MySql preset must parse the valid view form {sql:?}",
        );
    }
    // Syntax errors: the server 1064-rejects and the preset rejects too.
    let rejects = [
        "CREATE ALGORITHM = BOGUS VIEW zzp_v AS SELECT 1",
        "CREATE DEFINER = root ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "CREATE SQL SECURITY INVOKER ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "CREATE ALGORITHM = MERGE OR REPLACE VIEW zzp_v AS SELECT 1",
        "CREATE SQL SECURITY VIEW zzp_v AS SELECT 1",
        "CREATE VIEW ALGORITHM = MERGE zzp_v AS SELECT 1",
        "ALTER OR REPLACE VIEW zzp_v AS SELECT 1",
        "ALTER VIEW IF EXISTS zzp_v AS SELECT 1",
        "ALTER DEFINER = root ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "ALTER SQL SECURITY INVOKER ALGORITHM = MERGE VIEW zzp_v AS SELECT 1",
        "ALTER SQL SECURITY INVOKER DEFINER = root VIEW zzp_v AS SELECT 1",
        "ALTER VIEW ALGORITHM = MERGE zzp_v AS SELECT 1",
        "ALTER ALGORITHM = BOGUS VIEW zzp_v AS SELECT 1",
        "ALTER ALGORITHM VIEW zzp_v AS SELECT 1",
    ];
    for sql in rejects {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "view_option_reject", sql);
        assert_eq!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must 1064-reject the view syntax error {sql:?} (got {verdict:?})",
        );
        assert!(
            !ours_accepts(sql),
            "the fitted MySql preset must reject the view syntax error {sql:?}",
        );
    }
    eprintln!(
        "mysql view definition-option evidence: {} grammar-accepted (CREATE PREPAREs / ALTER 1295), \
         {} 1064-rejected (PREPARE-only, schema-provisioned)",
        accepts.len(),
        rejects.len(),
    );
}

/// The self-authored, QUARANTINED MySQL stored-routine body corpus (CC0-1.0), evidenced
/// through the COM_QUERY define-not-execute [`MySqlOracle::ddl_verdict`] channel rather than
/// the never-execute PREPARE oracle. One `-- accept:` / `-- reject:` header per statement.
const MYSQL_ROUTINE_BODIES: &str = include_str!("../corpus/mysql/routine_bodies.sql");

/// The self-authored MySQL scheduled-event DDL corpus (CC0-1.0). Same `-- accept:` / `-- reject:`
/// header format as [`MYSQL_ROUTINE_BODIES`], evidenced through the identical COM_QUERY
/// `ddl_verdict` channel; see `corpus/mysql/event_ddl.sql`.
const MYSQL_EVENT_DDL: &str = include_str!("../corpus/mysql/event_ddl.sql");

/// Parse `routine_bodies.sql` into `(expect_accept, sql)` pairs: an `-- accept:` / `-- reject:`
/// header names the expected server verdict for the single following non-comment line. Panics
/// on a malformed corpus (a header with no statement, a statement with no header) so a bad edit
/// fails loudly.
fn mysql_routine_body_cases() -> Vec<(bool, &'static str)> {
    parse_accept_reject_corpus(MYSQL_ROUTINE_BODIES, "routine-body")
}

/// Parse a `-- accept:` / `-- reject:` headered corpus into `(expect_accept, sql)` pairs: each
/// header names the expected server verdict for the single following non-comment line. Panics on
/// a malformed corpus (a header with no statement, a statement with no header) so a bad edit fails
/// loudly. Shared by the routine-body and event-DDL `ddl_verdict` evidence corpora.
fn parse_accept_reject_corpus(src: &'static str, kind: &str) -> Vec<(bool, &'static str)> {
    let mut cases: Vec<(bool, &'static str)> = Vec::new();
    let mut pending: Option<bool> = None;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-- accept:") {
            assert!(
                pending.is_none(),
                "{kind} header with no statement before the next"
            );
            pending = Some(true);
        } else if trimmed.starts_with("-- reject:") {
            assert!(
                pending.is_none(),
                "{kind} header with no statement before the next"
            );
            pending = Some(false);
        } else if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        } else {
            let expect_accept = pending
                .take()
                .unwrap_or_else(|| panic!("{kind} statement {trimmed:?} has no header"));
            cases.push((expect_accept, trimmed));
        }
    }
    assert!(pending.is_none(), "trailing {kind} header has no statement");
    cases
}

/// The routine-DDL server-side evidence: every authored routine body is run through the
/// COM_QUERY define-not-execute [`ddl_verdict`](MySqlOracle::ddl_verdict) channel — a valid
/// definition must `Accept` (and our fitted `MySql` preset must parse it), a body syntax error
/// must `Reject(ER_PARSE_ERROR=1064)` (and our preset must reject it too). This is the spike's
/// probe-proven channel; it is oracle-mysql-gated and skips cleanly with no server, running in
/// the nightly oracle lane. Each `ddl_verdict` provisions and tears down its own scratch
/// database, so it is quarantined from the never-execute PREPARE corpora.
#[test]
fn mysql_routine_body_ddl_verdict_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;
    let cases = mysql_routine_body_cases();
    assert!(
        !cases.is_empty(),
        "the routine-body corpus must be non-empty"
    );
    let (mut accepts, mut rejects) = (0usize, 0usize);
    for (index, (expect_accept, sql)) in cases.iter().enumerate() {
        let location = SweepLocation {
            corpus: "routine_bodies",
            index,
        };
        let verdict = verdict_or_abort(oracle.ddl_verdict(sql), &location, sql);
        if *expect_accept {
            assert_eq!(
                verdict,
                WireVerdict::Accept,
                "the server must define the valid routine {sql:?} (got {verdict:?})",
            );
            assert!(
                ours_accepts(sql),
                "our fitted MySql preset must parse the valid routine {sql:?}",
            );
            accepts += 1;
        } else {
            assert_eq!(
                verdict,
                WireVerdict::Reject(er_parse_error),
                "the server must 1064-reject the body syntax error {sql:?} (got {verdict:?})",
            );
            assert!(
                !ours_accepts(sql),
                "our fitted MySql preset must reject the body syntax error {sql:?}",
            );
            rejects += 1;
        }
    }
    eprintln!(
        "mysql routine-body ddl_verdict evidence: {accepts} accepted, {rejects} 1064-rejected \
         (COM_QUERY define-not-execute, scratch-database isolated)",
    );
}

/// The CREATE TRIGGER server-side evidence (`parse-mysql-trigger-ddl`): every authored trigger
/// runs through the COM_QUERY define-not-execute
/// [`ddl_verdict_with_setup`](MySqlOracle::ddl_verdict_with_setup) channel, which provisions the
/// trigger's target table in the scratch database first (a trigger's *accept* path binds its
/// table, unlike a bodyless routine). A valid trigger must `Accept` and the fitted `MySql`
/// preset must parse it; a header/body syntax error must `Reject(ER_PARSE_ERROR=1064)` — parsed
/// before binding, so it 1064s even with the table present — and the preset must reject it too.
/// Oracle-mysql-gated; skips cleanly with no server.
#[test]
fn mysql_trigger_ddl_verdict_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;
    // The trigger's target table, provisioned in each scratch database before the trigger.
    let setup: &[&str] = &["CREATE TABLE zzp_tt (a INT, b INT)"];
    let accepts = [
        "CREATE TRIGGER zzp_tr BEFORE INSERT ON zzp_tt FOR EACH ROW BEGIN END",
        "CREATE TRIGGER zzp_tr AFTER UPDATE ON zzp_tt FOR EACH ROW INSERT INTO zzp_tt VALUES (1, 2)",
        "CREATE DEFINER = CURRENT_USER TRIGGER zzp_tr BEFORE DELETE ON zzp_tt FOR EACH ROW BEGIN END",
    ];
    for sql in accepts {
        let verdict = verdict_or_abort(
            oracle.ddl_verdict_with_setup(setup, sql),
            "trigger_accept",
            sql,
        );
        assert_eq!(
            verdict,
            WireVerdict::Accept,
            "the server must define the valid trigger {sql:?} (got {verdict:?})",
        );
        assert!(
            ours_accepts(sql),
            "our fitted MySql preset must parse the valid trigger {sql:?}",
        );
    }
    let rejects = [
        // A malformed body (`IF` with no condition) — a parse error before table binding.
        "CREATE TRIGGER zzp_tr BEFORE INSERT ON zzp_tt FOR EACH ROW BEGIN IF END",
        // `INSTEAD OF` timing and statement-level (`FOR EACH STATEMENT`) triggers do not exist.
        "CREATE TRIGGER zzp_tr INSTEAD OF INSERT ON zzp_tt FOR EACH ROW BEGIN END",
        "CREATE TRIGGER zzp_tr BEFORE INSERT ON zzp_tt FOR EACH STATEMENT BEGIN END",
    ];
    for sql in rejects {
        let verdict = verdict_or_abort(
            oracle.ddl_verdict_with_setup(setup, sql),
            "trigger_reject",
            sql,
        );
        assert_eq!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must 1064-reject the malformed trigger {sql:?} (got {verdict:?})",
        );
        assert!(
            !ours_accepts(sql),
            "our fitted MySql preset must reject the malformed trigger {sql:?}",
        );
    }
    eprintln!(
        "mysql trigger ddl_verdict evidence: {} accepted, {} 1064-rejected \
         (COM_QUERY define-not-execute, scratch table provisioned)",
        accepts.len(),
        rejects.len(),
    );
}

/// The event-DDL server-side evidence, mirroring
/// [`mysql_routine_body_ddl_verdict_evidence`]: every authored `CREATE/ALTER/DROP EVENT` case is
/// run through the COM_QUERY define-not-execute [`ddl_verdict`](MySqlOracle::ddl_verdict) channel
/// — a valid definition must `Accept` (and the fitted `MySql` preset must parse it), a syntax
/// error must `Reject(ER_PARSE_ERROR=1064)` (and the preset must reject it too). Oracle-mysql
/// gated; skips cleanly with no server; each `ddl_verdict` provisions and tears down its own
/// scratch database, quarantined from the never-execute PREPARE corpora.
#[test]
fn mysql_event_ddl_verdict_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;
    let cases = parse_accept_reject_corpus(MYSQL_EVENT_DDL, "event-ddl");
    assert!(!cases.is_empty(), "the event-DDL corpus must be non-empty");
    let (mut accepts, mut rejects) = (0usize, 0usize);
    for (index, (expect_accept, sql)) in cases.iter().enumerate() {
        let location = SweepLocation {
            corpus: "event_ddl",
            index,
        };
        let verdict = verdict_or_abort(oracle.ddl_verdict(sql), &location, sql);
        if *expect_accept {
            assert_eq!(
                verdict,
                WireVerdict::Accept,
                "the server must define the valid event {sql:?} (got {verdict:?})",
            );
            assert!(
                ours_accepts(sql),
                "our fitted MySql preset must parse the valid event {sql:?}",
            );
            accepts += 1;
        } else {
            assert_eq!(
                verdict,
                WireVerdict::Reject(er_parse_error),
                "the server must 1064-reject the syntax error {sql:?} (got {verdict:?})",
            );
            assert!(
                !ours_accepts(sql),
                "our fitted MySql preset must reject the syntax error {sql:?}",
            );
            rejects += 1;
        }
    }
    eprintln!(
        "mysql event-ddl ddl_verdict evidence: {accepts} accepted, {rejects} 1064-rejected \
         (COM_QUERY define-not-execute, scratch-database isolated)",
    );
}

/// Per-sub-command live-oracle parity for the MySQL `SHOW` family (`parse-mysql-show-family`,
/// completed by `parse-mysql-show-remainder`).
///
/// The family inventory above tracks `SHOW` as one row (representative `SHOW DATABASES`); this
/// sweep exercises every sub-command the fitted `MySql` preset now parses — the ~40 the
/// show-family landing covered plus the five deferred productions (`SHOW GRANTS FOR …`,
/// `SHOW CREATE USER`, `SHOW PROFILE`, `SHOW {BINLOG | RELAYLOG} EVENTS`) — holding each to
/// two-sided parity against the live m3 oracle: the server must recognize the grammar (any
/// non-`Syntax` PREPARE-only outcome — most administrative `SHOW`s are `ER_UNSUPPORTED_PS`
/// 1295, grammar-valid but not preparable; the account/log/profile remainder is `Accept`,
/// preparable), and the fitted preset must parse it. A probe the oracle syntax-rejects (1064)
/// or the preset rejects is a real divergence. Self-authored probes (CC0), grammar-valid by
/// construction from the pinned GPL grammar read; every one is also a parser round-trip case
/// in `parser::util::tests::show_admin_round_trips`. Skips cleanly when no server is reachable.
#[test]
fn mysql_show_subcommand_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    // Every probe is grammar-valid MySQL 8.4 (facts-derived) and parsed by the `MySql`
    // preset; the sweep proves the two agree against the live server.
    const SHOW_SUBCOMMAND_PROBES: &[&str] = &[
        "SHOW DATABASES",
        "SHOW DATABASES LIKE 'a%'",
        "SHOW SCHEMAS",
        "SHOW CHARSET",
        "SHOW CHARACTER SET LIKE 'utf8%'",
        "SHOW COLLATION",
        "SHOW GLOBAL STATUS",
        "SHOW SESSION STATUS LIKE 'Threads%'",
        "SHOW GLOBAL VARIABLES",
        "SHOW VARIABLES LIKE 'max%'",
        "SHOW EVENTS",
        "SHOW TABLE STATUS",
        "SHOW OPEN TABLES",
        "SHOW TRIGGERS",
        "SHOW FULL TRIGGERS",
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
        "SHOW GRANTS FOR 'u'@'localhost'",
        "SHOW GRANTS FOR CURRENT_USER",
        "SHOW GRANTS FOR CURRENT_USER()",
        "SHOW GRANTS FOR u USING r",
        "SHOW GRANTS FOR u USING r1, r2",
        "SHOW CREATE USER u",
        "SHOW CREATE USER 'u'@'localhost'",
        "SHOW CREATE USER CURRENT_USER",
        "SHOW CREATE USER CURRENT_USER()",
        "SHOW PROFILE",
        "SHOW PROFILE ALL",
        "SHOW PROFILE CPU, MEMORY",
        "SHOW PROFILE BLOCK IO",
        "SHOW PROFILE CONTEXT SWITCHES",
        "SHOW PROFILE PAGE FAULTS",
        "SHOW PROFILE IPC, SWAPS, SOURCE",
        "SHOW PROFILE ALL FOR QUERY 1",
        "SHOW PROFILE CPU FOR QUERY 1 LIMIT 5",
        "SHOW PROFILE LIMIT 5 OFFSET 2",
        "SHOW PROFILE LIMIT 2, 5",
        "SHOW BINLOG EVENTS",
        "SHOW BINLOG EVENTS IN 'log'",
        "SHOW BINLOG EVENTS FROM 4",
        "SHOW BINLOG EVENTS IN 'log' FROM 4 LIMIT 10",
        "SHOW BINLOG EVENTS LIMIT 2, 10",
        "SHOW BINLOG EVENTS LIMIT 10 OFFSET 2",
        "SHOW RELAYLOG EVENTS",
        "SHOW RELAYLOG EVENTS IN 'log' FROM 4 LIMIT 10",
        "SHOW RELAYLOG EVENTS FOR CHANNEL 'c'",
        "SHOW RELAYLOG EVENTS IN 'log' FROM 4 LIMIT 2, 10 FOR CHANNEL 'c'",
        "SHOW WARNINGS LIMIT 5 OFFSET 2",
        "SHOW CREATE VIEW v",
        "SHOW CREATE DATABASE db",
        "SHOW CREATE EVENT e",
        "SHOW CREATE PROCEDURE p",
        "SHOW CREATE FUNCTION f",
        "SHOW CREATE TRIGGER t",
        "SHOW INDEX FROM t",
        "SHOW KEYS FROM t",
        "SHOW EXTENDED INDEXES FROM t",
        "SHOW WARNINGS",
        "SHOW WARNINGS LIMIT 5",
        "SHOW ERRORS LIMIT 1, 5",
        "SHOW COUNT(*) WARNINGS",
        "SHOW COUNT(*) ERRORS",
        "SHOW ENGINE INNODB STATUS",
        "SHOW ENGINE INNODB MUTEX",
        "SHOW REPLICA STATUS",
        "SHOW PROCEDURE CODE p",
        "SHOW FUNCTION CODE f",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in SHOW_SUBCOMMAND_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "SHOW sub-command", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) SHOW probe(s) — fix the probe \
         SQL so every sub-command is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected SHOW probe(s) the grammar recognizes: {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} SHOW sub-command parity: {} probes, all engine-recognized and parsed",
        SHOW_SUBCOMMAND_PROBES.len(),
    );
}

/// Per-verb live-oracle parity for the MySQL admin-table maintenance and `RENAME` families
/// (`parse-mysql-table-maintenance`).
///
/// The family inventory above tracks each verb as one representative row; this sweep
/// exercises the per-verb option surface the fitted `MySql` preset now parses — the
/// `NO_WRITE_TO_BINLOG | LOCAL` prefix, the `TABLE`/`TABLES` synonym, multi-table lists, the
/// CHECK/REPAIR repeatable option lists, the single CHECKSUM mode, the ANALYZE histogram
/// tails, and the `RENAME TABLE`/`RENAME USER` rename lists. Each is held to two-sided parity
/// against the live m3 oracle: the server must recognize the grammar (any non-`Syntax`
/// PREPARE-only outcome — these verbs are not preparable, so `ER_UNSUPPORTED_PS` 1295, or a
/// catalogue error where the object is resolved), and the fitted preset must parse it. A
/// probe the oracle syntax-rejects (1064) or the preset rejects is a real divergence.
/// Self-authored probes (CC0), grammar-valid by construction from the pinned GPL grammar read;
/// every one is also a parser round-trip case in
/// `parser::util::tests::table_maintenance_parses_and_round_trips` /
/// `rename_parses_and_round_trips`. Skips cleanly when no server is reachable.
#[test]
fn mysql_table_maintenance_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const MAINTENANCE_PROBES: &[&str] = &[
        "ANALYZE TABLE t1",
        "ANALYZE NO_WRITE_TO_BINLOG TABLE t1",
        "ANALYZE LOCAL TABLE t1, t2",
        "ANALYZE TABLES t1",
        "ANALYZE TABLE t1 UPDATE HISTOGRAM ON c1, c2",
        "ANALYZE TABLE t1 UPDATE HISTOGRAM ON c1 WITH 16 BUCKETS",
        "ANALYZE TABLE t1 DROP HISTOGRAM ON c1",
        "CHECK TABLE t1",
        "CHECK TABLE t1 FOR UPGRADE",
        "CHECK TABLE t1, t2 QUICK FAST MEDIUM EXTENDED CHANGED",
        "CHECKSUM TABLE t1",
        "CHECKSUM TABLE t1 QUICK",
        "CHECKSUM TABLE t1 EXTENDED",
        "OPTIMIZE TABLE t1",
        "OPTIMIZE NO_WRITE_TO_BINLOG TABLE t1, t2",
        "REPAIR TABLE t1",
        "REPAIR LOCAL TABLE t1 QUICK EXTENDED USE_FRM",
        "RENAME TABLE a TO b",
        "RENAME TABLE a TO b, c TO d",
        "RENAME USER u@localhost TO v@localhost",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in MAINTENANCE_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "table-maintenance verb", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) maintenance probe(s) — fix the \
         probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected maintenance probe(s) the grammar recognizes: {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} table-maintenance parity: {} probes, all engine-recognized and parsed",
        MAINTENANCE_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL prepared-statement lifecycle
/// (`parse-mysql-prepare-execute`): `PREPARE ... FROM {'text' | @var}`, `EXECUTE ... [USING
/// @var, ...]`, and `{DEALLOCATE | DROP} PREPARE`.
///
/// The family inventory above tracks the three verbs as one representative row each; this
/// sweep exercises the source/argument surface the fitted `MySql` preset now parses — the
/// string vs `@`-variable `prepare_src` (in each `ident_or_text` quote spelling: bare,
/// `'…'`, `"…"`, `` `…` ``), the optional `USING` variable list, and the
/// `deallocate_or_drop` verb synonym. Each is held to two-sided parity against the live m3
/// oracle: the server must recognize the grammar, and the fitted preset must parse it. The
/// PREPARE-only channel makes this family its own small irony — *none* of the lifecycle is
/// preparable over the binary protocol, so every grammar-valid probe (including a
/// PREPARE-of-PREPARE source string) answers `ER_UNSUPPORTED_PS` 1295, never `Prepared`; a
/// 1064 (or a preset reject) is a real divergence. The measured *reject* boundaries (an
/// expression `prepare_src`, a `@@` system variable in either position, a parenthesized
/// `EXECUTE` list, a bare `DEALLOCATE name`, `DROP PREPARE IF EXISTS`) live in
/// `m3::SCHEMA_INDEPENDENT_REJECT`, both-reject-verified there. Self-authored probes (CC0),
/// grammar-valid by construction from the pinned GPL grammar read (`sql_yacc.yy` `prepare` /
/// `prepare_src` / `execute` / `execute_var_list` / `deallocate_or_drop`); every one is also
/// a parser round-trip case in `parser::util::tests::prepare_from_family_round_trips`. Skips
/// cleanly when no server is reachable.
#[test]
fn mysql_prepared_statement_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const PREPARED_STATEMENT_PROBES: &[&str] = &[
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
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    let mut prepared: Vec<&str> = Vec::new();
    for sql in PREPARED_STATEMENT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "prepared-statement form", sql);
        match classify_family_outcome(verdict) {
            FamilyEngineOutcome::Syntax => syntax_rejected.push(sql),
            // The lifecycle is not preparable, so a `Prepared` here means the channel's
            // semantics changed under us — surface it rather than silently absorbing it.
            FamilyEngineOutcome::Prepared => prepared.push(sql),
            _ => {}
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) prepared-statement probe(s) — \
         fix the probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        prepared.is_empty(),
        "MySQL {version} PREPAREd lifecycle probe(s) the protocol is documented to decline \
         (ER_UNSUPPORTED_PS) — the wire semantics drifted, review before re-baselining: \
         {prepared:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected prepared-statement probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} prepared-statement parity: {} probes, all engine-recognized and parsed",
        PREPARED_STATEMENT_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL `DROP {DATABASE | SCHEMA}` and `DROP INDEX … ON`
/// families (`parse-mysql-drop-database-index`). Two directions:
///
/// * every grammar-valid probe (the single-name database drop in both keyword spellings and the
///   index drop with its mandatory `ON <table>` and the full `ALGORITHM`/`LOCK` option surface —
///   with/without `=`, both orderings) must be engine-recognized (no `ER_PARSE_ERROR`) *and*
///   parsed by the fitted `MySql` preset; and
/// * every authored *syntax*-reject boundary (a comma list or `CASCADE` on the database drop, a
///   dotted database name, a missing/`RESTRICT` `ON`, a dotted index name, a repeated
///   `ALGORITHM`/`LOCK`) must be `ER_PARSE_ERROR` (1064) on the server *and* rejected by the preset.
///
/// The two `DROP DATABASE`/`DROP INDEX` family rows in the inventory above track the bare form;
/// this sweep is the option-surface evidence. An unknown `ALGORITHM`/`LOCK` *value* is deliberately
/// absent from both sets: the server treats it as a grammar-positive *binding* reject
/// (`ER_UNKNOWN_ALTER_ALGORITHM`/`ER_UNKNOWN_ALTER_LOCK`, not 1064), while the preset models only
/// the bind-valid value set and rejects it one stage earlier — a mismatch of reject *stage*, not of
/// accept/reject, so it belongs to neither the must-parse nor the must-1064 boundary. Skips cleanly
/// when no server is reachable.
#[test]
fn mysql_drop_database_index_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const GRAMMAR_VALID_PROBES: &[&str] = &[
        "DROP DATABASE zzp_db",
        "DROP SCHEMA zzp_db",
        "DROP DATABASE IF EXISTS zzp_db",
        "DROP SCHEMA IF EXISTS zzp_db",
        "DROP DATABASE `zzp weird`",
        "DROP INDEX zzp_ix ON t1",
        "DROP INDEX zzp_ix ON zzp_db.t1",
        "DROP INDEX zzp_ix ON t1 ALGORITHM = DEFAULT",
        "DROP INDEX zzp_ix ON t1 ALGORITHM = INPLACE",
        "DROP INDEX zzp_ix ON t1 ALGORITHM INSTANT",
        "DROP INDEX zzp_ix ON t1 ALGORITHM = COPY",
        "DROP INDEX zzp_ix ON t1 LOCK = NONE",
        "DROP INDEX zzp_ix ON t1 LOCK SHARED",
        "DROP INDEX zzp_ix ON t1 LOCK = EXCLUSIVE",
        "DROP INDEX zzp_ix ON t1 LOCK = DEFAULT",
        "DROP INDEX zzp_ix ON t1 ALGORITHM = COPY LOCK = SHARED",
        "DROP INDEX zzp_ix ON t1 LOCK NONE ALGORITHM DEFAULT",
    ];
    // Syntax rejects: `ER_PARSE_ERROR` (1064) on the server AND rejected by the preset.
    const SYNTAX_REJECT_PROBES: &[&str] = &[
        "DROP DATABASE a, b",
        "DROP SCHEMA a, b",
        "DROP DATABASE a CASCADE",
        "DROP DATABASE a RESTRICT",
        "DROP DATABASE zzp_db.x",
        "DROP INDEX zzp_ix",
        "DROP INDEX a.b ON t1",
        "DROP INDEX zzp_ix ON t1 RESTRICT",
        "DROP INDEX zzp_ix ON t1 ALGORITHM = COPY ALGORITHM = INPLACE",
        "DROP INDEX zzp_ix ON t1 LOCK = NONE LOCK = SHARED",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in GRAMMAR_VALID_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "drop database/index form", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) drop probe(s) — fix the probe SQL \
         so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected drop probe(s) the grammar recognizes: {ours_rejected:?}",
    );

    let mut engine_accepted: Vec<&str> = Vec::new();
    let mut ours_accepted: Vec<&str> = Vec::new();
    for sql in SYNTAX_REJECT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "drop database/index reject", sql);
        if classify_family_outcome(verdict) != FamilyEngineOutcome::Syntax {
            engine_accepted.push(sql);
        }
        if ours_accepts(sql) {
            ours_accepted.push(sql);
        }
    }
    assert!(
        engine_accepted.is_empty(),
        "MySQL {version} did NOT syntax-reject (expected ER_PARSE_ERROR 1064) probe(s) — the \
         reject boundary drifted, review before re-baselining: {engine_accepted:?}",
    );
    assert!(
        ours_accepted.is_empty(),
        "fitted MySql preset accepted drop probe(s) the grammar syntax-rejects: {ours_accepted:?}",
    );
    eprintln!(
        "mysql {version} drop database/index parity: {} grammar-valid + {} syntax-reject probes, \
         two-sided verified",
        GRAMMAR_VALID_PROBES.len(),
        SYNTAX_REJECT_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL `FLUSH` and `PURGE BINARY LOGS`
/// server-administration families (`parse-mysql-flush-purge`).
///
/// The family inventory above tracks each verb as one representative row; this sweep exercises
/// the option surface the fitted `MySql` preset now parses — the `NO_WRITE_TO_BINLOG | LOCAL`
/// prefix, the `{TABLE | TABLES}` synonym with its optional table list and `WITH READ LOCK`/
/// `FOR EXPORT` lock, every keyword flush target, comma-separated lists, the `RELAY LOGS FOR
/// CHANNEL` qualifier, and both `PURGE` target clauses. Each is held to two-sided parity
/// against the live m3 oracle: the server must recognize the grammar (any non-`Syntax`
/// PREPARE-only outcome — `FLUSH` is preparable so most probes `Prepared`, `PURGE` answers
/// `ER_UNSUPPORTED_PS` 1295, a `FLUSH TABLES <list>` binding-errors), and the fitted preset
/// must parse it. A probe the oracle syntax-rejects (1064) or the preset rejects is a real
/// divergence. The measured *reject* boundaries (the removed `HOSTS`/bare `RESOURCES`/`QUERY
/// CACHE` targets, `FLUSH TABLES FOR EXPORT` with no list, `TABLES` inside the keyword list,
/// the dropped `PURGE MASTER LOGS` synonym, a bare `PURGE BINARY LOGS`) are pinned in
/// `parser::util::tests::flush_and_purge_reject_malformed`. Self-authored probes (CC0),
/// grammar-valid by construction from the pinned GPL grammar read (`sql_yacc.yy` `flush` /
/// `flush_option` / `purge`); every one is also a parser round-trip case in
/// `parser::util::tests::flush_and_purge_parse_and_round_trip`. Skips cleanly when no server
/// is reachable.
#[test]
fn mysql_flush_purge_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const FLUSH_PURGE_PROBES: &[&str] = &[
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
        "PURGE BINARY LOGS TO 'log.000001'",
        "PURGE BINARY LOGS BEFORE '2000-01-01 00:00:00'",
        "PURGE BINARY LOGS BEFORE NOW()",
        // The motivating form of `mysql-interval-arithmetic-expr-gap`: `BEFORE` delegates to
        // the expression grammar, which now reads the MySQL operator-position interval
        // (`NOW() - INTERVAL 3 DAY`) rather than rejecting the bare-integer amount.
        "PURGE BINARY LOGS BEFORE NOW() - INTERVAL 3 DAY",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in FLUSH_PURGE_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "FLUSH/PURGE form", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) FLUSH/PURGE probe(s) — fix the \
         probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected FLUSH/PURGE probe(s) the grammar recognizes: {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} FLUSH/PURGE parity: {} probes, all engine-recognized and parsed",
        FLUSH_PURGE_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL operator-position interval `INTERVAL <expr> <unit>`
/// (`mysql-interval-arithmetic-expr-gap`) — MySQL's `Item_date_add_interval` operand.
///
/// The measured accept surface (mysql:8.4.10): the additive `+`/`-` operand on either side of a
/// `bit_expr` and the leading `INTERVAL … <unit> + expr` form; an arbitrary amount expression
/// (integer, decimal, negative, `?`, `@var`, `(expr)`, a bare `n + 1`, a quoted string); and the
/// whole underscore unit vocabulary — the simple `MICROSECOND`…`YEAR` (plus `WEEK`, `QUARTER`)
/// and the composites (`SECOND_MICROSECOND`, `HOUR_SECOND`, `DAY_HOUR`, `YEAR_MONTH`, the four
/// `*_MICROSECOND`), the composites also carrying a colon/dash string operand. Every probe rides
/// a `SELECT`, which PREPAREs cleanly, so the oracle answers `Prepared` (any non-`Syntax` outcome
/// proves the grammar recognizes it); the fitted `MySql` preset must parse each. The reject
/// boundary (a standalone `INTERVAL 3 DAY`, a leading `INTERVAL 3 DAY - x`, a unit-less amount,
/// an ANSI `TO`/precision spelling on an integer amount) is pinned in
/// `parser::expr::tests::mysql_interval_operator_boundary_rejects_and_fallthrough`; the *position*
/// rejects are a deliberate over-acceptance (the node is a primary — see
/// `ExpressionSyntax::mysql_interval_operator`). Self-authored probes (CC0), grammar-valid by
/// construction from the pinned GPL grammar read (`sql_yacc.yy` `bit_expr`/`interval`). Skips
/// cleanly when no server is reachable.
#[test]
fn mysql_interval_operator_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const INTERVAL_PROBES: &[&str] = &[
        // positions
        "SELECT NOW() - INTERVAL 3 DAY",
        "SELECT NOW() + INTERVAL 3 DAY",
        "SELECT INTERVAL 3 DAY + NOW()",
        "SELECT NOW() - INTERVAL 1 DAY - INTERVAL 1 HOUR",
        "SELECT DATE_ADD(NOW(), INTERVAL 1 DAY)",
        "SELECT GREATEST(NOW(), NOW() - INTERVAL 1 DAY)",
        "SELECT CASE WHEN 1 THEN NOW() - INTERVAL 1 DAY END",
        // operand surface
        "SELECT NOW() - INTERVAL 1.5 DAY",
        "SELECT NOW() - INTERVAL -3 DAY",
        "SELECT NOW() - INTERVAL ? DAY",
        "SELECT NOW() - INTERVAL @x DAY",
        "SELECT NOW() - INTERVAL (3 + 1) DAY",
        "SELECT NOW() - INTERVAL 3 + 1 DAY",
        "SELECT NOW() - INTERVAL '3' DAY",
        // simple units
        "SELECT NOW() + INTERVAL 1 MICROSECOND",
        "SELECT NOW() + INTERVAL 1 SECOND",
        "SELECT NOW() + INTERVAL 1 MINUTE",
        "SELECT NOW() + INTERVAL 1 HOUR",
        "SELECT NOW() + INTERVAL 1 DAY",
        "SELECT NOW() + INTERVAL 1 WEEK",
        "SELECT NOW() + INTERVAL 1 MONTH",
        "SELECT NOW() + INTERVAL 1 QUARTER",
        "SELECT NOW() + INTERVAL 1 YEAR",
        // composite units (with string operands where composite)
        "SELECT NOW() + INTERVAL '1.5' SECOND_MICROSECOND",
        "SELECT NOW() + INTERVAL '1:2.5' MINUTE_MICROSECOND",
        "SELECT NOW() + INTERVAL '1:2' MINUTE_SECOND",
        "SELECT NOW() + INTERVAL '1:2:3.4' HOUR_MICROSECOND",
        "SELECT NOW() + INTERVAL '1:2:3' HOUR_SECOND",
        "SELECT NOW() + INTERVAL '1:2' HOUR_MINUTE",
        "SELECT NOW() + INTERVAL '2 1:2:3.4' DAY_MICROSECOND",
        "SELECT NOW() + INTERVAL '2 1:2:3' DAY_SECOND",
        "SELECT NOW() + INTERVAL '2 1:2' DAY_MINUTE",
        "SELECT NOW() + INTERVAL '2 1' DAY_HOUR",
        "SELECT NOW() + INTERVAL '3-2' YEAR_MONTH",
        // the motivating PURGE-BEFORE offset delegates to this same expr grammar
        "DO NOW() - INTERVAL 3 DAY",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in INTERVAL_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "INTERVAL operator form", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) INTERVAL probe(s) — fix the probe \
         SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected INTERVAL operator probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} INTERVAL operator parity: {} probes, all engine-recognized and parsed",
        INTERVAL_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL `HANDLER` low-level cursor family
/// (`parse-mysql-handler`): `HANDLER <t> OPEN [[AS] alias]`, `HANDLER <t> CLOSE`, and the
/// three `HANDLER <t> READ` shapes — bare `{FIRST | NEXT}` scan, named-index `{FIRST | NEXT |
/// PREV | LAST}` traversal, and named-index `<op> (values)` key seek — each with the optional
/// `WHERE`/`LIMIT` tail.
///
/// The family inventory above tracks HANDLER as one representative row (`HANDLER t1 OPEN`);
/// this sweep exercises the whole grammar surface the fitted `MySql` preset now parses.
/// HANDLER is not preparable over the binary protocol, so — like the prepared-statement
/// lifecycle — every grammar-valid probe answers a non-syntax outcome, never `Prepared`: a
/// bare-connection unqualified `OPEN` is `ER_NO_DB_ERROR` 1046 (a binding reject), every other
/// form is `ER_UNSUPPORTED_PS` 1295. Only `ER_PARSE_ERROR` 1064 is "not recognized", which
/// for an authored probe is an inventory bug. The measured *reject* boundaries — a dotted
/// `READ`/`CLOSE` table, a bare-scan `PREV`/`LAST`, a `<>`/`!=` key operator, and an empty
/// `()` value list — live in `m3::SCHEMA_INDEPENDENT_REJECT`, both-reject-verified there.
/// Self-authored probes (CC0), grammar-valid by construction from the pinned GPL grammar read
/// (`sql_yacc.yy` `handler_stmt` / `handler_scan_function` / `handler_rkey_function` /
/// `handler_rkey_mode`); every one is also a parser round-trip case in
/// `parser::query::tests::handler_family_round_trips`. Skips cleanly when no server is
/// reachable.
#[test]
fn mysql_handler_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const HANDLER_PROBES: &[&str] = &[
        // OPEN: optional `[AS] alias`, schema-qualified table admitted.
        "HANDLER t OPEN",
        "HANDLER t OPEN AS a",
        "HANDLER t OPEN a",
        "HANDLER db.t OPEN",
        "HANDLER db.t OPEN AS a",
        // CLOSE (unqualified table only).
        "HANDLER t CLOSE",
        // READ bare scan: FIRST/NEXT, with optional WHERE/LIMIT.
        "HANDLER t READ FIRST",
        "HANDLER t READ NEXT",
        "HANDLER t READ FIRST WHERE a > 1",
        "HANDLER t READ NEXT WHERE a > 1 LIMIT 5",
        // READ named-index traversal: FIRST/NEXT/PREV/LAST.
        "HANDLER t READ idx FIRST",
        "HANDLER t READ idx NEXT",
        "HANDLER t READ idx PREV",
        "HANDLER t READ idx LAST",
        "HANDLER t READ idx FIRST WHERE a > 1 LIMIT 5",
        "HANDLER t READ `PRIMARY` FIRST",
        // READ named-index key seek: `= >= <= > <` over a value list.
        "HANDLER t READ idx = (1)",
        "HANDLER t READ idx >= (1)",
        "HANDLER t READ idx <= (1)",
        "HANDLER t READ idx > (1)",
        "HANDLER t READ idx < (1)",
        "HANDLER t READ idx = (1, 2)",
        "HANDLER t READ idx = (1 + 1)",
        "HANDLER t READ idx = (@v)",
        "HANDLER t READ idx = (DEFAULT)",
        "HANDLER t READ `PRIMARY` = (1)",
        "HANDLER t READ idx = (1) WHERE a > 1 LIMIT 2, 5",
        "HANDLER t READ idx = (1) LIMIT 5 OFFSET 2",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    let mut prepared: Vec<&str> = Vec::new();
    for sql in HANDLER_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "handler form", sql);
        match classify_family_outcome(verdict) {
            FamilyEngineOutcome::Syntax => syntax_rejected.push(sql),
            // HANDLER is not preparable, so a `Prepared` here means the channel's semantics
            // changed under us — surface it rather than silently absorbing it.
            FamilyEngineOutcome::Prepared => prepared.push(sql),
            _ => {}
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) HANDLER probe(s) — fix the probe \
         SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        prepared.is_empty(),
        "MySQL {version} PREPAREd HANDLER probe(s) the protocol is documented to decline \
         (ER_UNSUPPORTED_PS) — the wire semantics drifted, review before re-baselining: \
         {prepared:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected HANDLER probe(s) the grammar recognizes: {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} HANDLER parity: {} probes, all engine-recognized and parsed",
        HANDLER_PROBES.len(),
    );
}

/// The `LOAD DATA`/`LOAD XML` grammar-boundary server-side evidence (`parse-mysql-load-data-xml`).
///
/// `LOAD DATA` is not preparable — the PREPARE oracle answers `ER_UNSUPPORTED_PS` for a valid
/// statement, blind to a clause-order syntax error — so evidence is gathered through the COM_QUERY
/// [`ddl_verdict`](MySqlOracle::ddl_verdict) channel against a fresh scratch database. The probe
/// never has its file or target table, so a *grammar-valid* statement reaches a runtime reject
/// (`ER_NO_SUCH_TABLE` 1146, or `ER_LOAD_DATA_LOCAL_INFILE_DISABLED` 3948 for a `LOCAL` form) —
/// never `ER_PARSE_ERROR` 1064 — while a *malformed* one is 1064 at parse time, before the table
/// is ever consulted. So "the server recognizes the grammar" == the verdict is not `Reject(1064)`,
/// and each probe asserts the fitted `MySql` preset agrees (parses the grammar-valid forms, rejects
/// the 1064 forms). The measured boundary (mysql:8.4.10): the clause train is strictly
/// order-sensitive (any out-of-order clause is 1064), `FIELDS`/`COLUMNS` and `LINES`/`ROWS`
/// spellings are interchangeable, and every clause — `FIELDS`/`LINES` under `XML`,
/// `ROWS IDENTIFIED BY` under `DATA` — parses under either format (the format restriction is
/// semantic, enforced post-parse). Oracle-mysql-gated; skips cleanly with no server.
#[test]
fn mysql_load_data_grammar_boundary_evidence() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;

    // `(grammar_valid, sql)`: `true` == the server must NOT `Reject(1064)` (and our preset parses
    // it); `false` == the server 1064s at parse (and our preset rejects it). Every clause is
    // engine-measured; see the doc comment for the boundary summary.
    const LOAD_DATA_PROBES: &[(bool, &str)] = &[
        // Grammar-valid: the classic clause train under both DATA and XML.
        (true, "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1"),
        (
            true,
            "LOAD DATA LOW_PRIORITY INFILE 'zzp.tsv' INTO TABLE t1",
        ),
        (true, "LOAD DATA CONCURRENT INFILE 'zzp.tsv' INTO TABLE t1"),
        (true, "LOAD DATA LOCAL INFILE 'zzp.tsv' INTO TABLE t1"),
        (true, "LOAD DATA INFILE 'zzp.tsv' REPLACE INTO TABLE t1"),
        (true, "LOAD DATA INFILE 'zzp.tsv' IGNORE INTO TABLE t1"),
        (true, "LOAD DATA INFILE 'zzp.tsv' INTO TABLE db.t1"),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 PARTITION (p0, p1)",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 CHARACTER SET utf8mb4",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 FIELDS TERMINATED BY ','",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 COLUMNS TERMINATED BY ','",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' ESCAPED BY '\\\\'",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 LINES STARTING BY '>' TERMINATED BY '\\n'",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 IGNORE 2 LINES",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 IGNORE 2 ROWS",
        ),
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 (a, @v) SET b = @v, c = DEFAULT",
        ),
        (true, "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 ()"),
        (
            true,
            "LOAD DATA LOW_PRIORITY LOCAL INFILE 'zzp.tsv' REPLACE INTO TABLE t1 PARTITION (p0) CHARACTER SET utf8mb4 FIELDS TERMINATED BY ',' OPTIONALLY ENCLOSED BY '\"' ESCAPED BY '\\\\' LINES STARTING BY '>' TERMINATED BY '\\n' IGNORE 1 LINES (a, @v) SET b = @v",
        ),
        (true, "LOAD XML INFILE 'zzp.xml' INTO TABLE t1"),
        (true, "LOAD XML LOCAL INFILE 'zzp.xml' INTO TABLE t1"),
        (
            true,
            "LOAD XML INFILE 'zzp.xml' INTO TABLE t1 CHARACTER SET utf8mb4 ROWS IDENTIFIED BY '<row>' IGNORE 1 ROWS (a, @v) SET b = @v",
        ),
        // `ROWS IDENTIFIED BY` under DATA and `FIELDS`/`LINES` under XML are grammar-shared
        // (semantic-only restriction), so they parse under both formats.
        (
            true,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 ROWS IDENTIFIED BY '<row>'",
        ),
        (
            true,
            "LOAD XML INFILE 'zzp.xml' INTO TABLE t1 FIELDS TERMINATED BY ','",
        ),
        // 1064: out-of-order clauses (the train is order-sensitive).
        (
            false,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 FIELDS TERMINATED BY ',' CHARACTER SET utf8mb4",
        ),
        (
            false,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 LINES TERMINATED BY '\\n' FIELDS TERMINATED BY ','",
        ),
        (
            false,
            "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 CHARACTER SET utf8mb4 PARTITION (p0)",
        ),
        // 1064: a bare `FIELDS`/`LINES` with no sub-clause.
        (false, "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 FIELDS"),
        (false, "LOAD DATA INFILE 'zzp.tsv' INTO TABLE t1 LINES"),
        // 1064: mutually-exclusive duplicate handlers, and the lock modifier after `LOCAL`.
        (
            false,
            "LOAD DATA INFILE 'zzp.tsv' REPLACE IGNORE INTO TABLE t1",
        ),
        (
            false,
            "LOAD DATA LOCAL LOW_PRIORITY INFILE 'zzp.tsv' INTO TABLE t1",
        ),
    ];

    let mut divergent: Vec<(&str, WireVerdict, bool)> = Vec::new();
    let (mut valid, mut rejected) = (0usize, 0usize);
    for (index, (expect_grammar_valid, sql)) in LOAD_DATA_PROBES.iter().enumerate() {
        let location = SweepLocation {
            corpus: "load_data_boundary",
            index,
        };
        let verdict = verdict_or_abort(oracle.ddl_verdict(sql), &location, sql);
        let server_grammar_valid = verdict != WireVerdict::Reject(er_parse_error);
        let ours = ours_accepts(sql);
        if server_grammar_valid != *expect_grammar_valid || ours != *expect_grammar_valid {
            divergent.push((sql, verdict, ours));
        }
        if *expect_grammar_valid {
            valid += 1;
        } else {
            rejected += 1;
        }
    }
    assert!(
        divergent.is_empty(),
        "MySQL {version} LOAD DATA grammar-boundary divergence (sql, server_verdict, ours_accepts) — \
         the server 1064-vs-not split and the fitted MySql preset must agree with the pinned \
         expectation: {divergent:?}",
    );
    eprintln!(
        "mysql {version} LOAD DATA grammar boundary: {valid} grammar-valid, {rejected} 1064-rejected \
         (COM_QUERY define-not-execute, scratch-database isolated), preset in full agreement",
    );
}

/// Per-form live-oracle parity for the six MySQL server-administration families
/// (`parse-mysql-server-admin`): `SHUTDOWN`; `RESTART`; `CLONE {LOCAL | INSTANCE}`; `IMPORT
/// TABLE FROM …`; `HELP <topic>`; `BINLOG '<base64>'`.
///
/// The family inventory above tracks each as one representative row; this sweep exercises the
/// whole grammar surface the fitted `MySql` preset now parses. PREPARE-only, and deliberately
/// so: `SHUTDOWN`/`RESTART` would *execute* under COM_QUERY on this privileged connection and
/// `BINLOG` could apply an event, so every probe is a `prep` (parse+validate, never execute) —
/// no `ddl_verdict` here. Under PREPARE the outcomes are mixed but every one is grammar-positive
/// (never `Syntax` 1064): `SHUTDOWN`/`RESTART`/`CLONE`/`IMPORT TABLE`/`HELP` are declined by the
/// PREPARE protocol as `ER_UNSUPPORTED_PS` 1295 (parsed, then PS-declined), while `BINLOG` **is**
/// preparable and PREPAREs a grammar-valid payload (`Prepared`; the base64 decode/apply are
/// execution-time, never reached). Only `ER_PARSE_ERROR` 1064 is "not recognized", which for an
/// authored probe is an inventory bug. The measured *reject* boundaries — a nullary keyword with
/// an operand, a CLONE LOCAL without `DATA DIRECTORY`, a CLONE INSTANCE without an abutting
/// `:<port>`, a bare-ident IMPORT TABLE / BINLOG operand, and a two-operand HELP — live in
/// `m3::SCHEMA_INDEPENDENT_REJECT`, both-reject-verified there. Self-authored probes (CC0),
/// grammar-valid by construction from the pinned GPL grammar read (`sql_yacc.yy` `shutdown_stmt`
/// / `restart_server_stmt` / `clone_stmt` / `import_stmt` / `help` / `binlog_base64_event`);
/// every one is also a parser round-trip case in
/// `parser::util::tests::server_admin_family_round_trips`. Skips cleanly when no server is
/// reachable.
#[test]
fn mysql_server_admin_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    // A base64 event string the PREPARE parser accepts as a well-formed BINLOG operand; the
    // payload is never decoded or applied (that is an execution-time concern PREPARE never
    // reaches), so this is safe against a live server.
    const SERVER_ADMIN_PROBES: &[&str] = &[
        // SHUTDOWN / RESTART (nullary).
        "SHUTDOWN",
        "RESTART",
        // CLONE LOCAL, with and without the optional `=`.
        "CLONE LOCAL DATA DIRECTORY 'zzp_d'",
        "CLONE LOCAL DATA DIRECTORY = 'zzp_d'",
        // CLONE INSTANCE: named/bare/CURRENT_USER donor, quoted account, the optional
        // `DATA DIRECTORY [=]` and `REQUIRE [NO] SSL` tails.
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p'",
        "CLONE INSTANCE FROM 'u'@'h':3306 IDENTIFIED BY 'p'",
        "CLONE INSTANCE FROM u:3306 IDENTIFIED BY 'p'",
        "CLONE INSTANCE FROM CURRENT_USER:3306 IDENTIFIED BY 'p'",
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY 'zzp_d'",
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY = 'zzp_d'",
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' REQUIRE SSL",
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' REQUIRE NO SSL",
        "CLONE INSTANCE FROM u@h:3306 IDENTIFIED BY 'p' DATA DIRECTORY 'zzp_d' REQUIRE SSL",
        // IMPORT TABLE: single and multi-file string lists.
        "IMPORT TABLE FROM 'zzp_a.sdi'",
        "IMPORT TABLE FROM 'zzp_a.sdi', 'zzp_b.sdi'",
        // HELP: bare-identifier and quoted-string operands.
        "HELP contents",
        "HELP 'contents'",
        // BINLOG: a well-formed base64 operand (never decoded under PREPARE).
        "BINLOG 'YWJj'",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in SERVER_ADMIN_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "server-admin form", sql);
        if let FamilyEngineOutcome::Syntax = classify_family_outcome(verdict) {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) server-admin probe(s) — fix the \
         probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected server-admin probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} server-admin parity: {} probes, all engine-recognized and parsed",
        SERVER_ADMIN_PROBES.len(),
    );
}

/// The MySQL MyISAM key-cache pair (`parse-mysql-cache-index`) — `CACHE INDEX` and
/// `LOAD INDEX INTO CACHE` — measured against the schema-provisioned PREPARE-only oracle. A
/// grammar-valid shape must *not* `Reject(ER_PARSE_ERROR=1064)` (it parses, then PREPAREs or
/// binds) and the fitted `MySql` preset must parse it; a syntax error must `Reject(1064)` and
/// the preset must reject it too. The measured boundaries (mysql:8.4.10, `sql_yacc.yy`
/// `keycache_stmt`/`preload_stmt`): the multi-table list and the single-table `PARTITION`
/// form are mutually exclusive (a table list with `PARTITION`, either order, 1064-rejects);
/// `PARTITION` precedes the key list (a `PARTITION` written after the `{INDEX|KEY}(...)` list
/// 1064-rejects); `INDEX`/`KEY` are synonyms and the name list may be empty (`INDEX ()`) or
/// name `PRIMARY`; `IN {<name> | DEFAULT}` is mandatory on `CACHE INDEX`; `LOAD INDEX` takes
/// a per-table `IGNORE LEAVES` *after* the key list (before it 1064-rejects) and no `IN
/// <cache>` (a trailing `IN` 1064-rejects). Oracle-mysql-gated; skips cleanly with no server.
#[test]
fn mysql_cache_load_index_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::with_schema(MYSQL_SCHEMA_SETUP_SQL));
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );
    let er_parse_error = mysql::ServerError::ER_PARSE_ERROR as u16;

    // Grammar-valid shapes: the server parses them (any non-1064 verdict) and the preset accepts.
    let accepts = [
        // CACHE INDEX — list arm.
        "CACHE INDEX t1 IN zzp_kc",
        "CACHE INDEX t1, t2 IN zzp_kc",
        "CACHE INDEX t1 INDEX (a) IN zzp_kc",
        "CACHE INDEX t1 KEY (a) IN zzp_kc",
        "CACHE INDEX t1 INDEX () IN zzp_kc",
        "CACHE INDEX t1 KEY () IN zzp_kc",
        "CACHE INDEX t1 INDEX (PRIMARY) IN zzp_kc",
        "CACHE INDEX t1 INDEX (a, b) IN zzp_kc",
        "CACHE INDEX t1 INDEX (a), t2 KEY (f) IN zzp_kc",
        "CACHE INDEX squonk_oracle.t1 IN zzp_kc",
        "CACHE INDEX t1 IN DEFAULT",
        // CACHE INDEX — partition arm (PARTITION precedes the key list).
        "CACHE INDEX t1 PARTITION (p1) IN zzp_kc",
        "CACHE INDEX t1 PARTITION (ALL) IN zzp_kc",
        "CACHE INDEX t1 PARTITION (p1, p2) IN zzp_kc",
        "CACHE INDEX t1 PARTITION (p1) INDEX (a) IN zzp_kc",
        // LOAD INDEX INTO CACHE — list arm.
        "LOAD INDEX INTO CACHE t1",
        "LOAD INDEX INTO CACHE t1, t2",
        "LOAD INDEX INTO CACHE t1 INDEX (a)",
        "LOAD INDEX INTO CACHE t1 KEY (a)",
        "LOAD INDEX INTO CACHE t1 INDEX ()",
        "LOAD INDEX INTO CACHE t1 INDEX (PRIMARY)",
        "LOAD INDEX INTO CACHE t1 IGNORE LEAVES",
        "LOAD INDEX INTO CACHE t1 INDEX (a) IGNORE LEAVES",
        "LOAD INDEX INTO CACHE t1 IGNORE LEAVES, t2 INDEX (f)",
        "LOAD INDEX INTO CACHE squonk_oracle.t1",
        // LOAD INDEX INTO CACHE — partition arm.
        "LOAD INDEX INTO CACHE t1 PARTITION (p1)",
        "LOAD INDEX INTO CACHE t1 PARTITION (ALL)",
        "LOAD INDEX INTO CACHE t1 PARTITION (p1) INDEX (a) IGNORE LEAVES",
    ];
    for sql in accepts {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "cache/load index accept", sql);
        assert_ne!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must grammar-accept the valid key-cache statement {sql:?} (got {verdict:?})",
        );
        assert!(
            ours_accepts(sql),
            "the fitted MySql preset must parse the valid key-cache statement {sql:?}",
        );
    }

    // Syntax errors: the server 1064-rejects (parsed before binding) and the preset rejects too.
    let rejects = [
        // Partition and table list are mutually exclusive.
        "CACHE INDEX t1, t2 PARTITION (p1) IN zzp_kc",
        "CACHE INDEX t1 PARTITION (p1), t2 IN zzp_kc",
        "LOAD INDEX INTO CACHE t1 PARTITION (p1), t2",
        // PARTITION must precede the key list.
        "CACHE INDEX t1 INDEX (a) PARTITION (p1) IN zzp_kc",
        // IGNORE LEAVES follows the key list, never precedes it.
        "LOAD INDEX INTO CACHE t1 IGNORE LEAVES INDEX (a)",
        // `IN <cache>` is mandatory on CACHE INDEX and forbidden on LOAD INDEX.
        "CACHE INDEX t1",
        "CACHE INDEX IN zzp_kc",
        "CACHE INDEX t1 IN",
        "LOAD INDEX INTO CACHE t1 IN zzp_kc",
        // A missing table / bare keyword tail.
        "LOAD INDEX INTO CACHE",
        "LOAD INDEX INTO CACHE t1 IGNORE",
    ];
    for sql in rejects {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "cache/load index reject", sql);
        assert_eq!(
            verdict,
            WireVerdict::Reject(er_parse_error),
            "the server must 1064-reject the key-cache syntax error {sql:?} (got {verdict:?})",
        );
        assert!(
            !ours_accepts(sql),
            "the fitted MySql preset must reject the key-cache syntax error {sql:?}",
        );
    }
    eprintln!(
        "mysql {version} CACHE/LOAD INDEX parity: {} grammar-accepted, {} 1064-rejected \
         (PREPARE-only, schema-provisioned)",
        accepts.len(),
        rejects.len(),
    );
}

/// Per-form live-oracle parity for the MySQL replication-administration family
/// (`parse-mysql-replication`): `CHANGE REPLICATION SOURCE TO <options>`, `CHANGE REPLICATION
/// FILTER <rules>`, `START`/`STOP REPLICA`, and `START`/`STOP GROUP_REPLICATION`.
///
/// The family inventory above tracks these as five representative rows; this sweep exercises
/// the whole grammar surface the fitted `MySql` preset now parses. Unlike the HANDLER/XA
/// families, the classic replication verbs ARE preparable over the binary protocol, so `CHANGE
/// REPLICATION SOURCE/FILTER` and `START`/`STOP REPLICA` answer `Prepared` (the server parses
/// AND binds them); only `GROUP_REPLICATION` is `ER_UNSUPPORTED_PS` 1295 (the Group Replication
/// plugin is not preparable). A `START REPLICA … UNTIL` with an incoherent coordinate set is a
/// grammar-positive `ER_BAD_REPLICA_UNTIL_COND` 1277 (a binding reject), so it is not swept
/// here — the coordinate coherence is a semantic check, not grammar. Only `ER_PARSE_ERROR`
/// 1064 is "not recognized", which for an authored probe is an inventory bug. The measured
/// *reject* boundaries — the 8.4-removed `MASTER`/`SLAVE` spellings, the singular compression
/// option, empty option/rule lists, a bare filter table, single-paren rewrite pairs, the `STOP
/// REPLICA` UNTIL/connection ban, the space-separated GROUP option, and the UNTIL GTID-tail
/// restriction — live in `m3::SCHEMA_INDEPENDENT_REJECT`, both-reject-verified there. Self-
/// authored probes (CC0), grammar-valid by construction from the pinned GPL grammar read
/// (`sql_yacc.yy` `change_replication_stmt` / `source_def` / `filter_def` / `start_replica_stmt`
/// / `group_replication`); every one is also a parser round-trip case in
/// `parser::util::tests::replication_family_round_trips`. Skips cleanly when no server is
/// reachable.
#[test]
fn mysql_replication_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const REPLICATION_PROBES: &[&str] = &[
        // CHANGE REPLICATION SOURCE — string / numeric / bool-ish / exotic value shapes.
        "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h'",
        "CHANGE REPLICATION SOURCE TO SOURCE_PORT = 3306",
        "CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'h', SOURCE_PORT = 3306",
        "CHANGE REPLICATION SOURCE TO SOURCE_LOG_FILE = 'f', SOURCE_LOG_POS = 4",
        "CHANGE REPLICATION SOURCE TO RELAY_LOG_FILE = 'r', RELAY_LOG_POS = 8",
        "CHANGE REPLICATION SOURCE TO SOURCE_AUTO_POSITION = 1",
        "CHANGE REPLICATION SOURCE TO GET_SOURCE_PUBLIC_KEY = 1",
        "CHANGE REPLICATION SOURCE TO SOURCE_SSL = 1",
        "CHANGE REPLICATION SOURCE TO SOURCE_HEARTBEAT_PERIOD = 1.5",
        "CHANGE REPLICATION SOURCE TO SOURCE_COMPRESSION_ALGORITHMS = 'zstd'",
        "CHANGE REPLICATION SOURCE TO SOURCE_ZSTD_COMPRESSION_LEVEL = 3",
        "CHANGE REPLICATION SOURCE TO SOURCE_TLS_CIPHERSUITES = 'x'",
        "CHANGE REPLICATION SOURCE TO SOURCE_TLS_CIPHERSUITES = NULL",
        "CHANGE REPLICATION SOURCE TO IGNORE_SERVER_IDS = (1, 2, 3)",
        "CHANGE REPLICATION SOURCE TO IGNORE_SERVER_IDS = ()",
        "CHANGE REPLICATION SOURCE TO PRIVILEGE_CHECKS_USER = 'u'@'h'",
        "CHANGE REPLICATION SOURCE TO PRIVILEGE_CHECKS_USER = NULL",
        "CHANGE REPLICATION SOURCE TO REQUIRE_TABLE_PRIMARY_KEY_CHECK = ON",
        "CHANGE REPLICATION SOURCE TO REQUIRE_TABLE_PRIMARY_KEY_CHECK = GENERATE",
        "CHANGE REPLICATION SOURCE TO ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS = OFF",
        "CHANGE REPLICATION SOURCE TO ASSIGN_GTIDS_TO_ANONYMOUS_TRANSACTIONS = LOCAL",
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
        // GROUP REPLICATION — comma-separated options, both verbs (1295, not preparable).
        "START GROUP_REPLICATION",
        "STOP GROUP_REPLICATION",
        "START GROUP_REPLICATION USER = 'u'",
        "START GROUP_REPLICATION USER = 'u', PASSWORD = 'p'",
        "START GROUP_REPLICATION USER = 'u', PASSWORD = 'p', DEFAULT_AUTH = 'a'",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in REPLICATION_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "replication form", sql);
        if matches!(
            classify_family_outcome(verdict),
            FamilyEngineOutcome::Syntax
        ) {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) replication probe(s) — fix the \
         probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected replication probe(s) the grammar recognizes: {ours_rejected:?}",
    );
    eprintln!(
        "mysql {version} replication parity: {} probes, all engine-recognized and parsed",
        REPLICATION_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL server (`CREATE`/`ALTER`/`DROP SERVER`), `ALTER
/// INSTANCE`, and `ALTER {DATABASE | SCHEMA}` option families
/// (`parse-mysql-server-instance-database-ddl`).
///
/// The family inventory above tracks each as one representative row; this sweep exercises the
/// whole grammar surface the fitted `MySql` preset now parses. `ALTER INSTANCE` is preparable,
/// so its forms `Prepared`; the server and database DDL are not (`ER_UNSUPPORTED_PS` 1295, or a
/// bare-connection `ALTER DATABASE` with no name `ER_NO_DB_ERROR` 1046) — all grammar-positive
/// (non-`Syntax`). The measured *reject* boundaries are held to two-sided parity too: an empty/
/// absent `OPTIONS` list, a wrong server-option value type (`HOST 123`, `PORT '3306'`), an
/// unknown option, a comma-list `DROP SERVER`, a wrong `ALTER INSTANCE` keyword or a rollback
/// tail on the wrong action, a `READ ONLY 2` (only `ternary_option` `0`/`1`/`DEFAULT` bind), a
/// `DEFAULT READ ONLY` prefix, and a dotted database name — each `ER_PARSE_ERROR` 1064 on the
/// server and rejected by the preset. Self-authored probes (CC0), grammar-valid by construction
/// from the pinned GPL grammar read (`sql_yacc.yy` `create_server` / `server_option` /
/// `alter_instance_action` / `alter_database_option`); every accept form round-trips
/// byte-identically in `parser::ddl::tests::server_instance_database_ddl_round_trips` and every
/// reject in `parser::ddl::tests::server_instance_database_ddl_reject_boundaries`. Skips cleanly
/// when no server is reachable.
#[test]
fn mysql_server_instance_database_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const GRAMMAR_VALID_PROBES: &[&str] = &[
        // CREATE / ALTER / DROP SERVER.
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'localhost')",
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'h', DATABASE 'd', \
         USER 'u', PASSWORD 'p', SOCKET 'sk', OWNER 'o', PORT 3306)",
        "CREATE SERVER 'zzp_srv' FOREIGN DATA WRAPPER 'w' OPTIONS (PORT 3306)",
        "ALTER SERVER zzp_s OPTIONS (HOST 'h2')",
        "ALTER SERVER zzp_s OPTIONS (PORT 3307, USER 'u')",
        "DROP SERVER zzp_s",
        "DROP SERVER IF EXISTS zzp_s",
        // ALTER INSTANCE (preparable — these `Prepared`).
        "ALTER INSTANCE ROTATE INNODB MASTER KEY",
        "ALTER INSTANCE ROTATE BINLOG MASTER KEY",
        "ALTER INSTANCE RELOAD TLS",
        "ALTER INSTANCE RELOAD TLS NO ROLLBACK ON ERROR",
        "ALTER INSTANCE RELOAD TLS FOR CHANNEL ch",
        "ALTER INSTANCE RELOAD TLS FOR CHANNEL ch NO ROLLBACK ON ERROR",
        "ALTER INSTANCE RELOAD KEYRING",
        "ALTER INSTANCE ENABLE INNODB REDO_LOG",
        "ALTER INSTANCE DISABLE INNODB REDO_LOG",
        // ALTER {DATABASE | SCHEMA} option list.
        "ALTER DATABASE zzp_db CHARACTER SET utf8mb4",
        "ALTER DATABASE zzp_db CHARACTER SET = utf8mb4",
        "ALTER DATABASE zzp_db DEFAULT CHARACTER SET utf8mb4",
        "ALTER DATABASE zzp_db CHARSET utf8mb4",
        "ALTER DATABASE zzp_db CHARACTER SET binary",
        "ALTER DATABASE zzp_db COLLATE utf8mb4_bin",
        "ALTER DATABASE zzp_db DEFAULT COLLATE = utf8mb4_bin",
        "ALTER DATABASE zzp_db ENCRYPTION 'Y'",
        "ALTER DATABASE zzp_db DEFAULT ENCRYPTION = 'N'",
        "ALTER DATABASE zzp_db READ ONLY 1",
        "ALTER DATABASE zzp_db READ ONLY = 0",
        "ALTER DATABASE zzp_db READ ONLY DEFAULT",
        "ALTER DATABASE zzp_db CHARACTER SET utf8mb4 COLLATE utf8mb4_bin",
        "ALTER DATABASE CHARACTER SET utf8mb4",
        "ALTER SCHEMA zzp_db CHARACTER SET utf8mb4",
    ];
    // Syntax rejects: `ER_PARSE_ERROR` (1064) on the server AND rejected by the preset.
    const SYNTAX_REJECT_PROBES: &[&str] = &[
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS ()",
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql",
        "CREATE SERVER zzp_s OPTIONS (HOST 'h')",
        "ALTER SERVER zzp_s",
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS (PORT '3306')",
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 123)",
        "CREATE SERVER zzp_s FOREIGN DATA WRAPPER mysql OPTIONS (FOO 'bar')",
        "DROP SERVER a, b",
        "ALTER INSTANCE ROTATE FOO MASTER KEY",
        "ALTER INSTANCE ENABLE INNODB FOO",
        "ALTER INSTANCE RELOAD TLS FOR CHANNEL 'ch'",
        "ALTER INSTANCE ROTATE INNODB MASTER KEY NO ROLLBACK ON ERROR",
        "ALTER DATABASE zzp_db",
        "ALTER DATABASE zzp_db READ ONLY 2",
        "ALTER DATABASE zzp_db DEFAULT READ ONLY 1",
        "ALTER DATABASE zzp_db.x CHARACTER SET utf8mb4",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in GRAMMAR_VALID_PROBES {
        let verdict = verdict_or_abort(
            oracle.wire_verdict(sql),
            "server/instance/database form",
            sql,
        );
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) probe(s) — fix the probe SQL so \
         every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected probe(s) the grammar recognizes: {ours_rejected:?}",
    );

    let mut engine_accepted: Vec<&str> = Vec::new();
    let mut ours_accepted: Vec<&str> = Vec::new();
    for sql in SYNTAX_REJECT_PROBES {
        let verdict = verdict_or_abort(
            oracle.wire_verdict(sql),
            "server/instance/database reject",
            sql,
        );
        if classify_family_outcome(verdict) != FamilyEngineOutcome::Syntax {
            engine_accepted.push(sql);
        }
        if ours_accepts(sql) {
            ours_accepted.push(sql);
        }
    }
    assert!(
        engine_accepted.is_empty(),
        "MySQL {version} did NOT syntax-reject (expected ER_PARSE_ERROR 1064) probe(s) — the \
         reject boundary drifted, review before re-baselining: {engine_accepted:?}",
    );
    assert!(
        ours_accepted.is_empty(),
        "fitted MySql preset accepted probe(s) the grammar syntax-rejects: {ours_accepted:?}",
    );
    eprintln!(
        "mysql {version} server/instance/database parity: {} grammar-valid + {} syntax-reject \
         probes, two-sided verified",
        GRAMMAR_VALID_PROBES.len(),
        SYNTAX_REJECT_PROBES.len(),
    );
}

/// The MySQL tablespace / logfile-group storage-DDL family server-side evidence
/// (`parse-mysql-tablespace-logfile-ddl`).
///
/// None of these NDB/InnoDB DDL statements is preparable, so the PREPARE oracle answers
/// `ER_UNSUPPORTED_PS` (1295 — grammar-positive but not preparable) for every grammar-valid form;
/// only `ER_PARSE_ERROR` (1064) means "not recognized". So each accept probe asserts the server
/// did NOT 1064 it (and the fitted `MySql` preset parses it), and each reject probe asserts the
/// server 1064-rejected it (and the preset rejects it too).
///
/// Measured boundary (mysql:8.4.10): the `=` is optional on every option, options are order-free
/// and comma-optional, and `ENGINE` takes an optional `STORAGE` prefix and an ident-or-string
/// value. The size literal `size_number` admits a plain integer or a single-`K`/`M`/`G`-suffixed
/// integer written with no intervening space (`16 M` is 1064; `16MB` is `ER_WRONG_SIZE_NUMBER`
/// 1531, a semantic reject the preset also rejects — asserted at parse level in
/// `parser::ddl::tests`, not here). The option set is per-context: `UNDO TABLESPACE` takes
/// `ENGINE` alone (any other option is 1064), `ALTER TABLESPACE` excludes `FILE_BLOCK_SIZE`,
/// `RENAME TO` takes no trailing option, the bare `ALTER TABLESPACE` form needs at least one
/// option, `ADD UNDOFILE`/`ADD DATAFILE` (for `UNDO`) is mandatory, and `ALTER LOGFILE GROUP`
/// excludes `COMMENT`. Oracle-mysql-gated; skips cleanly with no server.
#[test]
fn mysql_tablespace_logfile_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    // Grammar-valid forms: every option, `=`-optionality, `STORAGE ENGINE`, size suffixes and
    // plain byte counts, `USE LOGFILE GROUP`, the `UNDO` variants, and comma-separated options.
    const ACCEPT_PROBES: &[&str] = &[
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd'",
        "CREATE TABLESPACE ts",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' FILE_BLOCK_SIZE = 8192",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' FILE_BLOCK_SIZE 8192",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENCRYPTION = 'Y'",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = InnoDB",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE InnoDB",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = 'InnoDB'",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' STORAGE ENGINE = ndbcluster",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' AUTOEXTEND_SIZE = 4M",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 134217728",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 16k",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' MAX_SIZE = 2G",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' EXTENT_SIZE = 1M",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' NODEGROUP = 0",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' WAIT",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' NO_WAIT",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' COMMENT = 'hi'",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE_ATTRIBUTE = '{}'",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M ENGINE = InnoDB",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M, ENGINE = InnoDB",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = InnoDB INITIAL_SIZE = 128M",
        "CREATE TABLESPACE ts USE LOGFILE GROUP lg INITIAL_SIZE = 128M",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' USE LOGFILE GROUP lg",
        "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu'",
        "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu' ENGINE = InnoDB",
        "ALTER TABLESPACE ts ADD DATAFILE 'ts2.ibd'",
        "ALTER TABLESPACE ts DROP DATAFILE 'ts2.ibd'",
        "ALTER TABLESPACE ts RENAME TO ts2",
        "ALTER TABLESPACE ts INITIAL_SIZE = 128M",
        "ALTER TABLESPACE ts AUTOEXTEND_SIZE = 4M",
        "ALTER TABLESPACE ts ENGINE = InnoDB",
        "ALTER TABLESPACE ts ENCRYPTION = 'Y'",
        "ALTER TABLESPACE ts WAIT",
        "ALTER TABLESPACE ts ADD DATAFILE 'ts2.ibd' ENGINE = InnoDB",
        "ALTER UNDO TABLESPACE ut SET ACTIVE",
        "ALTER UNDO TABLESPACE ut SET INACTIVE",
        "ALTER UNDO TABLESPACE ut SET ACTIVE ENGINE = InnoDB",
        "DROP TABLESPACE ts",
        "DROP TABLESPACE ts ENGINE = InnoDB",
        "DROP TABLESPACE ts ENGINE InnoDB",
        "DROP TABLESPACE ts WAIT",
        "DROP TABLESPACE ts ENGINE = InnoDB WAIT",
        "DROP UNDO TABLESPACE ut",
        "DROP UNDO TABLESPACE ut ENGINE = InnoDB",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat'",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' INITIAL_SIZE = 16M",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' UNDO_BUFFER_SIZE = 8M",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' REDO_BUFFER_SIZE = 8M",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' NODEGROUP = 0",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' WAIT",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' COMMENT = 'x'",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' ENGINE = ndbcluster",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' INITIAL_SIZE = 16M ENGINE = InnoDB",
        "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat'",
        "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' INITIAL_SIZE = 16M",
        "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' ENGINE = ndbcluster",
        "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' WAIT",
        "DROP LOGFILE GROUP lg",
        "DROP LOGFILE GROUP lg ENGINE = ndbcluster",
        "DROP LOGFILE GROUP lg ENGINE = ndbcluster WAIT",
    ];

    // Forms the server `ER_PARSE_ERROR` (1064)-rejects at parse — the per-context option
    // restrictions, mandatory datafile/undofile, `RENAME` taking no options, the empty bare-alter
    // form, and the size-literal adjacency rule.
    const SYNTAX_REJECT_PROBES: &[&str] = &[
        "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu' INITIAL_SIZE = 128M",
        "CREATE UNDO TABLESPACE ut",
        "ALTER UNDO TABLESPACE ut SET INACTIVE INITIAL_SIZE = 1M",
        "DROP UNDO TABLESPACE ut WAIT",
        "ALTER TABLESPACE ts FILE_BLOCK_SIZE = 8192",
        "ALTER TABLESPACE ts RENAME TO ts2 ENGINE = InnoDB",
        "ALTER TABLESPACE ts",
        "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' FILE_BLOCK_SIZE = 8192",
        "CREATE LOGFILE GROUP lg",
        "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' COMMENT = 'x'",
        "ALTER LOGFILE GROUP lg",
        "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 16 M",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut prepared: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in ACCEPT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "tablespace/logfile form", sql);
        match classify_family_outcome(verdict) {
            FamilyEngineOutcome::Syntax => syntax_rejected.push(sql),
            // Not preparable, so a `Prepared` verdict means the channel's semantics changed.
            FamilyEngineOutcome::Prepared => prepared.push(sql),
            _ => {}
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) tablespace/logfile probe(s) — fix \
         the probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        prepared.is_empty(),
        "MySQL {version} PREPAREd tablespace/logfile probe(s) the protocol is documented to \
         decline (ER_UNSUPPORTED_PS) — the wire semantics drifted, review before re-baselining: \
         {prepared:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected tablespace/logfile probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );

    let mut server_accepted: Vec<&str> = Vec::new();
    let mut ours_accepted: Vec<&str> = Vec::new();
    for sql in SYNTAX_REJECT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "tablespace/logfile reject", sql);
        if classify_family_outcome(verdict) != FamilyEngineOutcome::Syntax {
            server_accepted.push(sql);
        }
        if ours_accepts(sql) {
            ours_accepted.push(sql);
        }
    }
    assert!(
        server_accepted.is_empty(),
        "MySQL {version} did NOT syntax-reject (ER_PARSE_ERROR 1064) probe(s) expected to be a \
         grammar boundary — review before re-baselining: {server_accepted:?}",
    );
    assert!(
        ours_accepted.is_empty(),
        "fitted MySql preset accepted tablespace/logfile probe(s) the grammar 1064-rejects: \
         {ours_accepted:?}",
    );

    eprintln!(
        "mysql {version} tablespace/logfile parity: {} grammar-valid + {} 1064-boundary probes, \
         preset in full agreement",
        ACCEPT_PROBES.len(),
        SYNTAX_REJECT_PROBES.len(),
    );
}

/// Per-form live-oracle parity for the MySQL `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT`
/// family (`parse-mysql-plugin-component`): `INSTALL PLUGIN <ident> SONAME <string>`,
/// `UNINSTALL PLUGIN <ident>`, `INSTALL COMPONENT <string-urn-list> [SET <scoped
/// assignments>]`, and `UNINSTALL COMPONENT <string-urn-list>`.
///
/// The family inventory above tracks each as one representative row; this sweep exercises the
/// whole grammar surface the fitted `MySql` preset now parses. The `PLUGIN` forms are
/// preparable (they `Prepared` against the bare connection — name resolution happens at
/// execute); the `COMPONENT` forms are not (`ER_UNSUPPORTED_PS` 1295) — all grammar-positive
/// (non-`Syntax`). The measured *reject* boundaries are held to two-sided parity too: exactly
/// one plugin per statement with an `ident` (never string) name and a mandatory string
/// `SONAME`; string (never bare-ident) component URNs; the `SET` tail's scope set exactly
/// `GLOBAL`/`PERSIST` (`SESSION`/`LOCAL`/`PERSIST_ONLY` and the `@`/`@@` sigils are 1064) with
/// no `DEFAULT` value sentinel; and no `SET` tail on `UNINSTALL COMPONENT` — each
/// `ER_PARSE_ERROR` 1064 on the server and rejected by the preset. Self-authored probes (CC0),
/// grammar-valid by construction from the pinned GPL grammar read (`sql_yacc.yy`
/// `install_stmt` / `uninstall` / `install_option_type` / `install_set_rvalue` /
/// `TEXT_STRING_sys_list`); every accept form round-trips byte-identically in
/// `parser::dcl::tests::install_uninstall_family_round_trips` and every reject in
/// `parser::dcl::tests::install_uninstall_family_reject_edge_cases`. Skips cleanly when no
/// server is reachable.
#[test]
fn mysql_plugin_component_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const GRAMMAR_VALID_PROBES: &[&str] = &[
        // INSTALL / UNINSTALL PLUGIN (preparable).
        "INSTALL PLUGIN zzp_pl SONAME 'zzp_plugin.so'",
        "INSTALL PLUGIN `zzp_pl` SONAME 'zzp_plugin.so'",
        "UNINSTALL PLUGIN zzp_pl",
        "UNINSTALL PLUGIN `zzp_pl`",
        // INSTALL COMPONENT: URN list arity, the scoped SET tail (ER_UNSUPPORTED_PS 1295).
        "INSTALL COMPONENT 'file://zzp_c'",
        "INSTALL COMPONENT 'file://zzp_c', 'file://zzp_d'",
        "INSTALL COMPONENT 'file://zzp_c', 'file://zzp_d', 'file://zzp_e'",
        "INSTALL COMPONENT 'file://zzp_c' SET v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET GLOBAL v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET PERSIST v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET comp.v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET GLOBAL comp.v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET v = ON",
        "INSTALL COMPONENT 'file://zzp_c' SET v = OFF",
        "INSTALL COMPONENT 'file://zzp_c' SET v = 'str'",
        "INSTALL COMPONENT 'file://zzp_c' SET v = 1 + 2",
        "INSTALL COMPONENT 'file://zzp_c' SET v := 1",
        "INSTALL COMPONENT 'file://zzp_c' SET GLOBAL v = 1, PERSIST w.x = ON, y = 2",
        "INSTALL COMPONENT 'file://zzp_c', 'file://zzp_d' SET v = 1",
        // UNINSTALL COMPONENT (ER_UNSUPPORTED_PS 1295).
        "UNINSTALL COMPONENT 'file://zzp_c'",
        "UNINSTALL COMPONENT 'file://zzp_c', 'file://zzp_d'",
    ];
    // Syntax rejects: `ER_PARSE_ERROR` (1064) on the server AND rejected by the preset.
    const SYNTAX_REJECT_PROBES: &[&str] = &[
        "INSTALL PLUGIN zzp_pl",
        "INSTALL PLUGIN 'zzp_pl' SONAME 'zzp_plugin.so'",
        "INSTALL PLUGIN zzp_pl SONAME zzp_lib",
        "INSTALL PLUGIN zzp_pl SONAME 'a.so', zzp_q SONAME 'b.so'",
        "UNINSTALL PLUGIN zzp_pl, zzp_q",
        "UNINSTALL PLUGIN 'zzp_pl'",
        "INSTALL COMPONENT zzp_c",
        "INSTALL COMPONENT 'file://zzp_c' SET SESSION v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET LOCAL v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET PERSIST_ONLY v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET @v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET @@v = 1",
        "INSTALL COMPONENT 'file://zzp_c' SET v = DEFAULT",
        "UNINSTALL COMPONENT zzp_c",
        "UNINSTALL COMPONENT 'file://zzp_c' SET v = 1",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in GRAMMAR_VALID_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "plugin/component form", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) plugin/component probe(s) — fix \
         the probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected plugin/component probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );

    let mut engine_accepted: Vec<&str> = Vec::new();
    let mut ours_accepted: Vec<&str> = Vec::new();
    for sql in SYNTAX_REJECT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "plugin/component reject", sql);
        if classify_family_outcome(verdict) != FamilyEngineOutcome::Syntax {
            engine_accepted.push(sql);
        }
        if ours_accepts(sql) {
            ours_accepted.push(sql);
        }
    }
    assert!(
        engine_accepted.is_empty(),
        "MySQL {version} did NOT syntax-reject (expected ER_PARSE_ERROR 1064) plugin/component \
         probe(s) — the reject boundary drifted, review before re-baselining: {engine_accepted:?}",
    );
    assert!(
        ours_accepted.is_empty(),
        "fitted MySql preset accepted plugin/component probe(s) the grammar syntax-rejects: \
         {ours_accepted:?}",
    );
    eprintln!(
        "mysql {version} plugin/component parity: {} grammar-valid + {} syntax-reject probes, \
         two-sided verified",
        GRAMMAR_VALID_PROBES.len(),
        SYNTAX_REJECT_PROBES.len(),
    );
}

/// The spatial-reference-system + resource-group family server-side evidence
/// (`parse-mysql-srs-resource-group-ddl`): every authored shape runs through the PREPARE-only
/// [`wire_verdict`](MySqlOracle::wire_verdict) channel. Neither DDL family is preparable — a
/// grammar-valid form answers `ER_UNSUPPORTED_PS` 1295 (or a post-parse semantic code: 3708
/// `ER_SRS_MISSING_MANDATORY_ATTRIBUTE` for an attribute-less/bare SRS, 3709
/// `ER_SRS_MULTIPLE_ATTRIBUTE_DEFINITIONS` for a repeated attribute, 3652
/// `ER_INVALID_VCPU_RANGE`-family for an out-of-host-range VCPU, 1690 `ER_DATA_OUT_OF_RANGE`
/// for a u64-max srid) — while `SET RESOURCE GROUP` PREPAREs outright. Only `ER_PARSE_ERROR`
/// 1064 is "not grammar". The measured boundaries (mysql:8.4.10): SRS attributes permute
/// freely and repeat at the grammar level; `OR REPLACE` and `IF NOT EXISTS` are exclusive
/// branches (together = 1064); the srid is `real_ulonglong_num` (hex accepts; signed/decimal
/// 1064); `ORGANIZATION` requires `IDENTIFIED BY <int>`; the resource-group option train is
/// fixed-order, `TYPE` is mandatory, `CREATE` takes no `FORCE`; `VCPU`/`THREAD_PRIORITY`
/// values are `NUM`-typed (hex 1064) while `SET … FOR` thread ids are `real_ulong_num` (hex
/// accepts); `VCPU` ranges and thread ids take `opt_comma` separators (whitespace-only lists
/// grammar-accept); `THREAD_PRIORITY` admits a negative value. Each probe asserts the fitted
/// `MySql` preset agrees on both sides. Every grammar-valid form is also a parser round-trip
/// case (`parser::ddl::tests::srs_resource_group_ddl_round_trips`,
/// `parser::dcl::tests::mysql_set_resource_group_parses_and_round_trips`). Self-authored
/// probes (CC0), grammar-valid by construction from the pinned GPL grammar read
/// (`sql_yacc.yy` `create_srs_stmt` / `srs_attributes` / `drop_srs_stmt` /
/// `create_resource_group_stmt` / `alter_resource_group_stmt` / `drop_resource_group_stmt` /
/// `set_resource_group_stmt`, zero copied bytes). Skips cleanly with no server.
#[test]
fn mysql_srs_resource_group_live_oracle_parity() {
    oracle_or_skip!(oracle = MySqlOracle::new());
    let version = oracle
        .server_version()
        .expect("live oracle must answer SELECT VERSION()");
    assert!(
        version.starts_with("8."),
        "oracle server version {version:?} is not the expected MySQL 8.x line",
    );

    const GRAMMAR_VALID_PROBES: &[&str] = &[
        // CREATE SPATIAL REFERENCE SYSTEM: attribute order freedom, repeats, bare form,
        // OR REPLACE / IF NOT EXISTS branches, hex + u64-max srid, hex organization id.
        "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001 DEFINITION 'w' NAME 'z'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001 DESCRIPTION 'd' ORGANIZATION 'o' \
         IDENTIFIED BY 5 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'a' NAME 'b'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001",
        "CREATE OR REPLACE SPATIAL REFERENCE SYSTEM 990001 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM IF NOT EXISTS 990001 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 0x10 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 18446744073709551615 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o' IDENTIFIED BY 0x10",
        // DROP SPATIAL REFERENCE SYSTEM.
        "DROP SPATIAL REFERENCE SYSTEM 990001",
        "DROP SPATIAL REFERENCE SYSTEM IF EXISTS 990001",
        // CREATE RESOURCE GROUP: `[=]` spellings, VCPU range shapes (comma, whitespace,
        // single, mixed), signed THREAD_PRIORITY, states.
        "CREATE RESOURCE GROUP g TYPE = USER",
        "CREATE RESOURCE GROUP g TYPE USER",
        "CREATE RESOURCE GROUP g TYPE = SYSTEM",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-3",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU 0-3",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0,1,2",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-2,4,6-8",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0 1 2",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 5",
        "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = 5",
        "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = -5",
        "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY 5",
        "CREATE RESOURCE GROUP g TYPE = USER ENABLE",
        "CREATE RESOURCE GROUP g TYPE = USER DISABLE",
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-3 THREAD_PRIORITY = 5 ENABLE",
        // ALTER RESOURCE GROUP: every clause optional, FORCE independent of the state.
        "ALTER RESOURCE GROUP g",
        "ALTER RESOURCE GROUP g VCPU = 0-3",
        "ALTER RESOURCE GROUP g THREAD_PRIORITY = 5",
        "ALTER RESOURCE GROUP g ENABLE",
        "ALTER RESOURCE GROUP g DISABLE",
        "ALTER RESOURCE GROUP g DISABLE FORCE",
        "ALTER RESOURCE GROUP g ENABLE FORCE",
        "ALTER RESOURCE GROUP g FORCE",
        "ALTER RESOURCE GROUP g VCPU 1-2 THREAD_PRIORITY 3 DISABLE",
        "ALTER RESOURCE GROUP g VCPU = 0-3 THREAD_PRIORITY = 5 ENABLE FORCE",
        // DROP RESOURCE GROUP.
        "DROP RESOURCE GROUP g",
        "DROP RESOURCE GROUP g FORCE",
        // SET RESOURCE GROUP (PREPAREs): bare, comma and whitespace id lists, hex id.
        "SET RESOURCE GROUP g",
        "SET RESOURCE GROUP g FOR 1",
        "SET RESOURCE GROUP g FOR 1, 2, 3",
        "SET RESOURCE GROUP g FOR 1 2 3",
        "SET RESOURCE GROUP g FOR 0x10",
    ];

    const SYNTAX_REJECT_PROBES: &[&str] = &[
        // OR REPLACE and IF NOT EXISTS are exclusive grammar branches.
        "CREATE OR REPLACE SPATIAL REFERENCE SYSTEM IF NOT EXISTS 990001 NAME 'z'",
        // The srid is an unsigned integer (`real_ulonglong_num`).
        "CREATE SPATIAL REFERENCE SYSTEM -1 NAME 'z' DEFINITION 'w'",
        "CREATE SPATIAL REFERENCE SYSTEM 1.5 NAME 'z' DEFINITION 'w'",
        // ORGANIZATION requires `IDENTIFIED BY <integer>`.
        "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o'",
        "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o' IDENTIFIED BY 'x'",
        // Single-srid drop: no comma list.
        "DROP SPATIAL REFERENCE SYSTEM 990001, 990002",
        // TYPE is mandatory and closed; the option train is fixed-order; no CREATE FORCE.
        "CREATE RESOURCE GROUP g",
        "CREATE RESOURCE GROUP g TYPE = FOO",
        "CREATE RESOURCE GROUP g TYPE = USER ENABLE VCPU = 0-3",
        "CREATE RESOURCE GROUP g TYPE = USER ENABLE FORCE",
        // VCPU / THREAD_PRIORITY values are `NUM`-typed: no hex (unlike the srid slots).
        "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0x1",
        "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = 0x1",
        // Single-name drop: no comma list.
        "DROP RESOURCE GROUP g, h",
    ];

    let mut syntax_rejected: Vec<&str> = Vec::new();
    let mut ours_rejected: Vec<&str> = Vec::new();
    for sql in GRAMMAR_VALID_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "srs/resource-group form", sql);
        if classify_family_outcome(verdict) == FamilyEngineOutcome::Syntax {
            syntax_rejected.push(sql);
        }
        if !ours_accepts(sql) {
            ours_rejected.push(sql);
        }
    }
    assert!(
        syntax_rejected.is_empty(),
        "MySQL {version} syntax-rejected (ER_PARSE_ERROR 1064) SRS/resource-group probe(s) — \
         fix the probe SQL so every form is grammar-valid: {syntax_rejected:?}",
    );
    assert!(
        ours_rejected.is_empty(),
        "fitted MySql preset rejected SRS/resource-group probe(s) the grammar recognizes: \
         {ours_rejected:?}",
    );

    let mut engine_accepted: Vec<&str> = Vec::new();
    let mut ours_accepted: Vec<&str> = Vec::new();
    for sql in SYNTAX_REJECT_PROBES {
        let verdict = verdict_or_abort(oracle.wire_verdict(sql), "srs/resource-group reject", sql);
        if classify_family_outcome(verdict) != FamilyEngineOutcome::Syntax {
            engine_accepted.push(sql);
        }
        if ours_accepts(sql) {
            ours_accepted.push(sql);
        }
    }
    assert!(
        engine_accepted.is_empty(),
        "MySQL {version} did NOT syntax-reject (expected ER_PARSE_ERROR 1064) probe(s) — the \
         reject boundary drifted, review before re-baselining: {engine_accepted:?}",
    );
    assert!(
        ours_accepted.is_empty(),
        "fitted MySql preset accepted probe(s) the grammar syntax-rejects: {ours_accepted:?}",
    );
    eprintln!(
        "mysql {version} srs/resource-group parity: {} grammar-valid + {} syntax-reject \
         probes, two-sided verified",
        GRAMMAR_VALID_PROBES.len(),
        SYNTAX_REJECT_PROBES.len(),
    );
}
