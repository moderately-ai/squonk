// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Thin safe wrapper over [`libduckdb_sys`] for the DuckDB oracles.
//!
//! # Why not `duckdb-rs`?
//!
//! The ergonomic `duckdb` crate makes `arrow` an unconditional runtime dependency
//! (~11 crates + heavy compile). The oracles only need open / prepare / execute
//! setup DDL / single-cell string query (`json_serialize_sql`). Using the official
//! C-API crate [`libduckdb_sys`] keeps Arrow out of the graph while still using
//! maintained bindgen bindings (not a hand-rolled `extern "C"` surface).
//!
//! # Prepare semantics (parity with duckdb-rs)
//!
//! DuckDB's multi-statement `prepare` **executes** every statement but the last.
//! duckdb-rs mirrors that via `duckdb_extract_statements` +
//! `duckdb_prepare_extracted_statement` + `duckdb_execute_prepared` for the
//! intermediates. We do the same so the oracle pins stay stable.
//!
//! `libduckdb-sys` still carries unconditional *build*-deps (reqwest/tar/zip for
//! optional download of prebuilt libs). With `DUCKDB_LIB_DIR` set (`.cargo/config.toml`)
//! that download never runs; the compile cost of those build-deps remains a residual
//! until upstream gates them. That is still a large win vs duckdb-rs (no Arrow).
//!
//! See `duckdb-oracle-thin-prepare-binding`.

// Calls into libduckdb_sys are `unsafe`; confined to this oracle-only module.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::c_void;
use std::ptr;

use libduckdb_sys::{
    DuckDBSuccess, duckdb_close, duckdb_connect, duckdb_connection, duckdb_database,
    duckdb_destroy_extracted, duckdb_destroy_prepare, duckdb_destroy_result, duckdb_disconnect,
    duckdb_execute_prepared, duckdb_extract_statements, duckdb_extract_statements_error,
    duckdb_extracted_statements, duckdb_free, duckdb_library_version, duckdb_open,
    duckdb_prepare_error, duckdb_prepare_extracted_statement, duckdb_prepared_statement,
    duckdb_query, duckdb_result, duckdb_result_error, duckdb_value_varchar,
};

use crate::oracle::OracleUnavailable;

/// An owned in-process DuckDB connection over the system-linked `libduckdb`.
///
/// Drop closes the connection and database. Exclusive ownership only — DuckDB
/// connections are not safe for concurrent use.
pub struct Connection {
    db: duckdb_database,
    conn: duckdb_connection,
}

// nextest may move the oracle across threads; exclusive ownership is fine.
unsafe impl Send for Connection {}

impl Connection {
    /// Open an in-memory database (`duckdb_open(nullptr, …)`).
    pub fn open_in_memory() -> Result<Self, OracleUnavailable> {
        // SAFETY: out-params are stack-allocated nulls; on success the C API owns
        // the heap objects until `duckdb_close`/`duckdb_disconnect`.
        unsafe {
            let mut db: duckdb_database = ptr::null_mut();
            if duckdb_open(ptr::null(), &mut db) != DuckDBSuccess || db.is_null() {
                return Err(OracleUnavailable("duckdb_open(:memory:) failed".to_owned()));
            }
            let mut conn: duckdb_connection = ptr::null_mut();
            if duckdb_connect(db, &mut conn) != DuckDBSuccess || conn.is_null() {
                duckdb_close(&mut db);
                return Err(OracleUnavailable("duckdb_connect failed".to_owned()));
            }
            Ok(Self { db, conn })
        }
    }

    /// Execute one or more statements (setup DDL).
    ///
    /// Uses `duckdb_query` (not the Arrow-query path duckdb-rs uses for
    /// `execute_batch`) — equivalent for DDL/setup and keeps Arrow out of our
    /// link surface.
    pub fn execute_batch(&self, sql: &str) -> Result<(), OracleUnavailable> {
        let c_sql = CString::new(sql)
            .map_err(|_| OracleUnavailable("setup SQL contains interior NUL".to_owned()))?;
        unsafe {
            let mut result = mem::zeroed::<duckdb_result>();
            let state = duckdb_query(self.conn, c_sql.as_ptr(), &mut result);
            if state != DuckDBSuccess {
                let msg = cstr_or(duckdb_result_error(&mut result), "duckdb_query failed");
                duckdb_destroy_result(&mut result);
                return Err(OracleUnavailable(format!(
                    "duckdb schema setup failed: {msg}"
                )));
            }
            duckdb_destroy_result(&mut result);
            Ok(())
        }
    }

    /// Prepare-only accept/reject: `true` if prepare of the **last** extracted
    /// statement succeeds (after executing any intermediate statements — duckdb-rs
    /// parity).
    pub fn prepare_ok(&self, sql: &str) -> bool {
        self.prepare_err(sql).is_none()
    }

    /// Prepare and, on failure, return the engine error string (reject classification).
    pub fn prepare_err(&self, sql: &str) -> Option<String> {
        let Ok(c_sql) = CString::new(sql) else {
            return Some("SQL contains interior NUL".to_owned());
        };
        unsafe {
            let mut extracted: duckdb_extracted_statements = ptr::null_mut();
            let n = duckdb_extract_statements(self.conn, c_sql.as_ptr(), &mut extracted);
            if n == 0 {
                let msg = if extracted.is_null() {
                    "duckdb_extract_statements failed".to_owned()
                } else {
                    let m = cstr_or(
                        duckdb_extract_statements_error(extracted),
                        "duckdb_extract_statements failed",
                    );
                    duckdb_destroy_extracted(&mut extracted);
                    m
                };
                return Some(msg);
            }

            // Execute intermediates (duckdb-rs prepare semantics).
            for i in 0..n.saturating_sub(1) {
                if let Err(msg) = self.execute_extracted(extracted, i) {
                    duckdb_destroy_extracted(&mut extracted);
                    return Some(msg);
                }
            }

            // Prepare the last statement only — do not execute it.
            let last = n - 1;
            let mut stmt: duckdb_prepared_statement = ptr::null_mut();
            let state = duckdb_prepare_extracted_statement(self.conn, extracted, last, &mut stmt);
            duckdb_destroy_extracted(&mut extracted);
            if state == DuckDBSuccess {
                if !stmt.is_null() {
                    duckdb_destroy_prepare(&mut stmt);
                }
                return None;
            }
            let msg = if stmt.is_null() {
                "duckdb_prepare_extracted_statement failed".to_owned()
            } else {
                let m = cstr_or(duckdb_prepare_error(stmt), "prepare failed");
                duckdb_destroy_prepare(&mut stmt);
                m
            };
            Some(msg)
        }
    }

    /// Parse-only statement segmentation: how many top-level statements DuckDB's
    /// parser splits `sql` into, computed WITHOUT preparing, binding, or executing
    /// any of them.
    ///
    /// This is the never-execute-safe observation the raw-byte differential needs.
    /// [`prepare_err`](Self::prepare_err) (the accept/reject oracle path) *executes*
    /// every statement but the last of a multi-statement string — a never-execute
    /// violation the curated corpus dodges by being single-statement
    /// (`corpus_is_single_statement`), but arbitrary fuzz bytes are not single
    /// statements. `duckdb_extract_statements` runs only the PARSER — it produces the
    /// statement list the preparer would later consume — so nothing is bound or run.
    /// This method deliberately never calls `duckdb_prepare_extracted_statement` /
    /// `duckdb_execute_prepared`: it extracts, reads the count, and destroys, so no
    /// side effect and no name resolution ever happens (the never-execute proof for
    /// the DuckDB arm of the differential).
    ///
    /// Extraction being parse-only also makes it the *right* comparison basis: an
    /// unresolved object name (`SELECT * FROM t`) still parses, matching our own
    /// parse-only parser — the same [`OracleSemantics::ParseOnly`](crate::oracle::OracleSemantics::ParseOnly)
    /// footing libpg_query gives the PostgreSQL differential, without the
    /// [`PrepareBind`](crate::oracle::OracleSemantics::PrepareBind) false-divergence
    /// problem `prepare` would import.
    ///
    /// `Ok(n)` is a parse success splitting into `n` statements — `n >= 1`, or `0`
    /// for an input the parser reads as empty (whitespace / comments / bare `;`).
    /// `Err(msg)` is a genuine parse (syntax) failure. An interior NUL rejects at the
    /// C-string boundary, matching libpg_query and [`prepare_err`](Self::prepare_err).
    ///
    /// The empty-vs-error distinction is the error *pointer*, measured against
    /// libduckdb 1.5.4: a zero-statement extraction sets the extracted handle
    /// non-null with a NULL `duckdb_extract_statements_error` (no error) for an empty
    /// or comment-only input, and a non-null `Parser Error: …` string for a real
    /// syntax error — so `Ok(0)` and `Err` are told apart by the error pointer, not a
    /// message-string heuristic.
    pub fn extract_statement_count(&self, sql: &str) -> Result<usize, String> {
        let Ok(c_sql) = CString::new(sql) else {
            return Err("SQL contains interior NUL".to_owned());
        };
        // SAFETY: `c_sql` outlives the call; `extracted` is an out-param the C API
        // fills (non-null even on a parse error) and we always destroy it before
        // returning.
        unsafe {
            let mut extracted: duckdb_extracted_statements = ptr::null_mut();
            let n = duckdb_extract_statements(self.conn, c_sql.as_ptr(), &mut extracted);
            if n == 0 {
                // A null handle is an allocation/infra failure, not a parse verdict.
                if extracted.is_null() {
                    return Err("duckdb_extract_statements returned a null handle".to_owned());
                }
                let err_ptr = duckdb_extract_statements_error(extracted);
                let result = if err_ptr.is_null() {
                    // No error => an empty / comment-only input the parser reads as
                    // zero statements (accept), matching our parser.
                    Ok(0)
                } else {
                    let msg = cstr_or(err_ptr, "duckdb_extract_statements failed");
                    // Measured on libduckdb 1.5.4: unknown `PRAGMA <name>` returns
                    // `n == 0` with a *Catalog* error (`Pragma Function with name … does
                    // not exist`), not a Parser Error — extract validates the pragma
                    // name against the catalog even though the rest of the API is
                    // parse-only (unresolved tables/functions still extract as `Ok(1)`).
                    // Treat catalog-only failures as parse accept of one statement so
                    // the differential stays on parse footing rather than inventing a
                    // false `duckdb=reject` for syntactically valid PRAGMA forms.
                    if msg.starts_with("Catalog Error:") {
                        Ok(1)
                    } else {
                        Err(msg)
                    }
                };
                duckdb_destroy_extracted(&mut extracted);
                return result;
            }
            duckdb_destroy_extracted(&mut extracted);
            Ok(n as usize)
        }
    }

    /// Run `sql` and return the first column of the first row as a UTF-8 string
    /// (structural oracle: `SELECT json_serialize_sql('…')`).
    pub fn query_string(&self, sql: &str) -> Result<String, OracleUnavailable> {
        let c_sql = CString::new(sql)
            .map_err(|_| OracleUnavailable("query SQL contains interior NUL".to_owned()))?;
        unsafe {
            let mut result = mem::zeroed::<duckdb_result>();
            let state = duckdb_query(self.conn, c_sql.as_ptr(), &mut result);
            if state != DuckDBSuccess {
                let msg = cstr_or(duckdb_result_error(&mut result), "duckdb_query failed");
                duckdb_destroy_result(&mut result);
                return Err(OracleUnavailable(msg));
            }
            let raw = duckdb_value_varchar(&mut result, 0, 0);
            if raw.is_null() {
                duckdb_destroy_result(&mut result);
                return Err(OracleUnavailable(
                    "duckdb_value_varchar returned null".to_owned(),
                ));
            }
            let s = CStr::from_ptr(raw).to_string_lossy().into_owned();
            duckdb_free(raw.cast::<c_void>());
            duckdb_destroy_result(&mut result);
            Ok(s)
        }
    }

    /// The linked `libduckdb` version string (`duckdb_library_version`), read for the
    /// "oracle actually ran" CI evidence — the DuckDB analogue of the MySQL oracle's
    /// server-version probe. Proves not just that `libduckdb` linked but that the loaded
    /// library answers a call, and records which version linked (catching silent drift).
    /// The pointer is a static, library-owned string (no free); empty only on the
    /// never-observed null return.
    pub fn version(&self) -> String {
        // SAFETY: `duckdb_library_version` takes no arguments and returns a pointer to a
        // static, library-owned string; borrowed only long enough to copy out.
        unsafe {
            let raw = duckdb_library_version();
            if raw.is_null() {
                String::new()
            } else {
                CStr::from_ptr(raw).to_string_lossy().into_owned()
            }
        }
    }

    /// Prepare + execute one extracted statement (intermediate of a multi-statement
    /// prepare). Returns the engine error string on failure.
    unsafe fn execute_extracted(
        &self,
        extracted: duckdb_extracted_statements,
        index: u64,
    ) -> Result<(), String> {
        unsafe {
            let mut stmt: duckdb_prepared_statement = ptr::null_mut();
            let state = duckdb_prepare_extracted_statement(self.conn, extracted, index, &mut stmt);
            if state != DuckDBSuccess {
                let msg = if stmt.is_null() {
                    "prepare intermediate failed".to_owned()
                } else {
                    let m = cstr_or(duckdb_prepare_error(stmt), "prepare intermediate failed");
                    duckdb_destroy_prepare(&mut stmt);
                    m
                };
                return Err(msg);
            }
            let mut result = mem::zeroed::<duckdb_result>();
            let state = duckdb_execute_prepared(stmt, &mut result);
            let err = if state != DuckDBSuccess {
                Some(cstr_or(
                    duckdb_result_error(&mut result),
                    "execute intermediate failed",
                ))
            } else {
                None
            };
            duckdb_destroy_prepare(&mut stmt);
            duckdb_destroy_result(&mut result);
            match err {
                Some(msg) => Err(msg),
                None => Ok(()),
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            if !self.conn.is_null() {
                duckdb_disconnect(&mut self.conn);
            }
            if !self.db.is_null() {
                duckdb_close(&mut self.db);
            }
        }
    }
}

fn cstr_or(ptr: *const std::os::raw::c_char, fallback: &str) -> String {
    if ptr.is_null() {
        return fallback.to_owned();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}
