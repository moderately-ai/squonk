// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Never-execute statement-segmentation observation for the SQLite raw-byte
//! differential, over rusqlite's raw `sqlite3` handle.
//!
//! # Why an FFI shim at all
//!
//! The raw-byte differential ([`crate::fuzz::sqlite_differential_raw_bytes`]) must,
//! when both parsers accept, compare *statement counts* — the segmentation-aware
//! check the strengthened PostgreSQL differential (`pg_accept_reject_divergence`)
//! carries, because a boolean `accept == accept` masks a splitter over-acceptance
//! (the statement-splitter class the instrument exists to catch). rusqlite's safe
//! [`Connection::prepare`](rusqlite::Connection::prepare) compiles only the *first*
//! statement of a multi-statement string and silently ignores the tail, so it can
//! neither count statements nor even validate the whole input. The C API's
//! `sqlite3_prepare_v2` exposes the `pzTail` out-pointer that walks statement by
//! statement, but rusqlite does not surface it — hence this thin, additive shim over
//! the raw handle ([`Connection::handle`](rusqlite::Connection::handle)).
//!
//! # Never execute
//!
//! [`segment`] compiles each statement with `sqlite3_prepare_v2` and immediately
//! `sqlite3_finalize`s it WITHOUT ever calling `sqlite3_step`, so no statement runs
//! and no side effect occurs — the SQLite analogue of the DuckDB arm's
//! extract-without-prepare discipline ([`crate::duckdb_ffi::Connection::extract_statement_count`]).
//! `pzTail` iteration is the only public way to segment SQLite SQL without executing.
//!
//! # The PrepareBind seam and the parse/bind boundary
//!
//! Unlike libpg_query (parse-only) and DuckDB's `extract_statements` (parse-only),
//! `sqlite3_prepare_v2` is [`PrepareBind`](crate::oracle::OracleSemantics::PrepareBind):
//! it resolves object names against the (empty, for the differential) schema, so
//! `SELECT * FROM t` fails with `no such table: t`. Our parser does not bind, so a
//! naive boolean would read that as a false over-acceptance on every unresolved name.
//!
//! The resolution is sound, not a heuristic guess: **a name-resolution error is
//! positive proof the statement parsed** — SQLite only reaches binding after a clean
//! parse. So [`segment`] treats a resolution-class prepare failure as a *parse
//! accept* (the input is syntactically valid), while a genuine tokenizer/parser error
//! stays a reject. The classifier (`is_resolution_error`) is a conservative
//! allowlist of SQLite's resolution-error message stems: an *unrecognized* message
//! defaults to **reject**, so the failure mode is a loud, triageable false divergence
//! (surfaced by the fuzzer, then added here), never a silently-swallowed
//! over-acceptance — the direction that matters for a correctness instrument. The
//! list is expected to grow as soak surfaces new resolution messages.
//!
//! A resolution failure also blocks `pzTail` advancement (a failed
//! `sqlite3_prepare_v2` does not set the tail), so once one statement binds-fails the
//! remaining statements cannot be counted. [`segment`] reports that as
//! [`SqliteSegmentation::Accept`] with `count_reliable = false`, and the differential
//! then compares only the boolean, never the (truncated) count — the honest
//! boundary: SQLite statement counts are comparable exactly when every statement
//! resolves cleanly (the schema-independent common case the fuzzer mostly explores).

// Calls into `rusqlite::ffi` (libsqlite3-sys) are `unsafe`; confined to this module.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use rusqlite::Connection;
use rusqlite::ffi;

/// The parse-only, never-execute segmentation verdict for a raw SQL input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqliteSegmentation {
    /// SQLite's tokenizer/parser rejected the input (a genuine syntax error, or an
    /// interior NUL at the C-string boundary). Carries the engine message.
    Reject(String),
    /// SQLite parsed the input. `count` is the number of top-level statements seen;
    /// `count_reliable` is false when a resolution-class failure truncated `pzTail`
    /// iteration before the end of input, so only the boolean accept is trustworthy.
    Accept { count: usize, count_reliable: bool },
}

/// Segment `sql` into top-level statements using `sqlite3_prepare_v2` + `pzTail`,
/// finalizing each without executing it (see the module docs).
///
/// `conn` should be a bare, schema-less in-memory database for the raw-byte
/// differential, so a resolution failure means "references a name that does not
/// exist" (proof of parse) rather than a schema-specific reject.
pub fn segment(conn: &Connection, sql: &str) -> SqliteSegmentation {
    segment_with_retry(conn, sql, true)
}

fn segment_with_retry(
    conn: &Connection,
    sql: &str,
    retry_in_active_transaction: bool,
) -> SqliteSegmentation {
    // Interior NUL rejects at the C-string boundary — the same contract libpg_query
    // and the DuckDB arm enforce, so the NUL-in-SQL class is armed for SQLite too.
    let Ok(c_sql) = CString::new(sql) else {
        return SqliteSegmentation::Reject("SQL contains interior NUL".to_owned());
    };

    // SAFETY: `conn` owns a live `sqlite3`; `handle()` borrows it for the duration of
    // this call. Each prepared statement is finalized before the next iteration and
    // never stepped. `zSql` stays alive in `c_sql` for the whole loop.
    unsafe {
        let db = conn.handle();
        let mut cursor: *const c_char = c_sql.as_ptr();
        let mut count = 0usize;

        loop {
            let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
            let mut tail: *const c_char = ptr::null();
            let rc = ffi::sqlite3_prepare_v2(db, cursor, -1, &mut stmt, &mut tail);

            if rc != ffi::SQLITE_OK {
                let msg = errmsg(db);
                // COMMIT/END/ROLLBACK can bind-fail solely because this never-execute
                // connection has no active transaction. That failure leaves `pzTail`
                // unset, which can hide a malformed suffix. Retry the same bytes on a
                // scratch connection whose only executed setup is `BEGIN`; the input
                // itself is still prepared and finalized without stepping, and SQLite
                // can now expose the real tail boundary.
                if retry_in_active_transaction && needs_active_transaction(&msg) {
                    if let Ok(retry) = Connection::open_in_memory() {
                        if retry.execute_batch("BEGIN").is_ok() {
                            return segment_with_retry(&retry, sql, false);
                        }
                    }
                }
                // A resolution error proves the statement parsed; count is now
                // truncated (a failed prepare leaves `pzTail` unset), so the boolean
                // accept holds but the count does not.
                return if is_resolution_error(&msg) {
                    SqliteSegmentation::Accept {
                        count,
                        count_reliable: false,
                    }
                } else {
                    SqliteSegmentation::Reject(msg)
                };
            }

            // A NULL `stmt` with SQLITE_OK is an empty statement (a bare `;`, or
            // trailing whitespace / comment) — not counted. A non-NULL statement is
            // finalized WITHOUT stepping, so nothing executes.
            if !stmt.is_null() {
                count += 1;
                ffi::sqlite3_finalize(stmt);
            }

            // Advance over the just-consumed statement. Stop at end of input (`tail`
            // at the terminating NUL) or if the tail failed to progress — either
            // guard rules out an infinite loop on a pathological empty tail.
            if tail.is_null() || tail == cursor || *tail == 0 {
                break;
            }
            cursor = tail;
        }

        SqliteSegmentation::Accept {
            count,
            count_reliable: true,
        }
    }
}

fn needs_active_transaction(msg: &str) -> bool {
    msg.contains("cannot commit")
        || msg.contains("cannot rollback")
        || msg.contains("no transaction is active")
}

/// Whether an `sqlite3_errmsg` string is a name-resolution / semantic error (the
/// statement parsed) rather than a tokenizer/parser syntax error.
///
/// A conservative allowlist of SQLite's resolution-stage message stems. Unrecognized
/// messages default to **reject** (treated as a syntax error) so an over-acceptance
/// is never silently swallowed — see the module docs. Extend this list when the soak
/// surfaces a new resolution message as a false divergence.
fn is_resolution_error(msg: &str) -> bool {
    // Tokenizer/parser diagnostics quote the offending input after `near`, so broad
    // semantic stems must never classify their quoted text. For example, the syntax
    // error for a bare `"the same"` contains that phrase even though no resolution
    // stage was reached.
    if (msg.starts_with("near ") && msg.ends_with(": syntax error"))
        || msg.starts_with("unrecognized token:")
    {
        return false;
    }
    // Lower-cased once; SQLite's messages are stable lower-case English.
    const RESOLUTION_STEMS: &[&str] = &[
        "no such table",
        "no such column",
        "no such function",
        "no such collation sequence",
        "no such index",
        "no such module",
        "no such view",
        "no such trigger",
        "no such savepoint",
        "no such schema",
        "ambiguous column name",
        "has no column named",
        "table has no column",
        "row value misused",
        "misuse of aggregate",
        "misuse of window function",
        "misuse of aliased",
        "wrong number of arguments",
        "sub-select returns",
        "only a single result allowed",
        "aggregate functions are not allowed",
        "window functions may not be used",
        "second argument to nth_value",
        "is only available",
        "no query solution",
        "the same",
        "cannot use window functions",
        "not authorized",
        "too many",
        "unsafe use of",
        "may not be modified",
        "cannot modify",
        "no tables specified",
        "term out of range",
        "clause is required before",
        "partial index",
        "cannot open",
        "unknown or unsupported join type",
        "raise() may only be used",
        "default value of column",
        "generated column",
        "cannot add",
        "cannot commit",
        "cannot start a transaction within a transaction",
        "cannot rollback",
        "no transaction is active",
        "circularly defined",
        "recursive reference",
        "does not match number of result columns",
    ];
    RESOLUTION_STEMS.iter().any(|stem| msg.contains(stem))
}

/// The current `sqlite3_errmsg` for `db` as an owned lossy string.
///
/// # Safety
///
/// `db` must be a valid, open `sqlite3` handle.
unsafe fn errmsg(db: *mut ffi::sqlite3) -> String {
    unsafe {
        let raw = ffi::sqlite3_errmsg(db);
        if raw.is_null() {
            String::new()
        } else {
            CStr::from_ptr(raw).to_string_lossy().into_owned()
        }
    }
}
