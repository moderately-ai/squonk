// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Byte-keyed lexer class data for the parser hot loop.

/// ASCII whitespace byte (may *start* a whitespace run).
pub const CLASS_WHITESPACE: u8 = 1 << 0;
/// First byte of an unquoted identifier.
pub const CLASS_IDENTIFIER_START: u8 = 1 << 1;
/// Non-first byte of an unquoted identifier.
pub const CLASS_IDENTIFIER_CONTINUE: u8 = 1 << 2;
/// ASCII decimal digit.
pub const CLASS_DIGIT: u8 = 1 << 3;
/// Operator spelling byte.
pub const CLASS_OPERATOR: u8 = 1 << 4;
/// Whitespace-run *continuation* byte: it extends an already-open whitespace run
/// but cannot start one. SQLite's vertical tab is the sole member — SQLite's
/// tokenizer marks `0x0b` illegal as a token start (`aiClass[0x0b]` is not
/// `CC_SPACE`, so a lone or token-leading `\v` is an "unrecognized token") yet
/// `sqlite3Isspace(0x0b)` is true, so a `\v` is swallowed when it *follows* another
/// space in a run (measured: `"\x20\x0b"` accepts, `"\x0b"` and `"SELECT\x0b\x20 1"`
/// reject). The scanner reads this in the run-extension predicate only, never at
/// run entry (see the tokenizer's `skip_trivia`).
pub const CLASS_WHITESPACE_CONTINUE: u8 = 1 << 5;
/// Structural punctuation byte.
pub const CLASS_PUNCTUATION: u8 = 1 << 6;
/// Statement-boundary "trim" whitespace: it folds as whitespace like
/// [`CLASS_WHITESPACE`], but is legal *only* as leading or trailing trivia of a
/// statement (adjacent to the input start, a `;` separator, or the input end) — a
/// member wedged between two content items of one statement is a hard error; comments
/// count as content items for this boundary rule.
/// DuckDB's vertical tab is the sole member: DuckDB trims `0x0b` at each
/// `;`-segment's edges but its statement parser rejects an interior `\v` (measured:
/// `"\x0bSELECT 1"`, `"SELECT 1\x0b"`, `"SELECT 1;\x0b"` accept; `"SELECT\x0b1"`,
/// `"SELECT 1\x0bSELECT 2"`, `"SELECT\x20\x0b1"` reject). The tokenizer's
/// `skip_trivia` folds the byte, then rejects a boundary byte that a content token
/// both precedes and follows.
pub const CLASS_WHITESPACE_BOUNDARY: u8 = 1 << 7;

/// Raw byte-class table storage.
pub type ByteClassTable = [u8; 256];

/// Const byte-class table shared by M1 dialects.
pub const BYTE_CLASSES: ByteClassTable = build_byte_classes();

/// Dialect-owned byte-class data for the tokenizer hot loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteClasses {
    table: ByteClassTable,
    /// Cached: does the table mark any byte [`CLASS_WHITESPACE_BOUNDARY`]? Lets the
    /// tokenizer skip the interior-boundary guard entirely for every dialect but
    /// DuckDB, keeping the whitespace hot loop untouched where the guard cannot fire.
    has_boundary: bool,
}

impl ByteClasses {
    /// Standard M1 byte classes: the ANSI baseline shared by every preset except
    /// PostgreSQL/MySQL (which layer the vertical tab as full whitespace) and
    /// SQLite/DuckDB (which layer their measured position-dependent vertical-tab
    /// rules — see [`SQLITE_BYTE_CLASSES`] / [`DUCKDB_BYTE_CLASSES`]).
    pub const STANDARD: Self = Self {
        table: BYTE_CLASSES,
        has_boundary: false,
    };

    /// Return all classes for `byte`.
    pub const fn byte_class(&self, byte: u8) -> u8 {
        self.table[byte as usize]
    }

    /// Return true if `byte` has any class in `mask`.
    pub const fn has_class(&self, byte: u8, mask: u8) -> bool {
        self.byte_class(byte) & mask != 0
    }

    /// Whether any byte carries [`CLASS_WHITESPACE_BOUNDARY`] — the tokenizer's cheap
    /// gate for the interior-boundary guard (true only for the DuckDB preset).
    pub const fn has_boundary_whitespace(&self) -> bool {
        self.has_boundary
    }

    /// Return a copy with `mask` added to `byte`.
    pub const fn with_class(mut self, byte: u8, mask: u8) -> Self {
        let index = byte as usize;
        self.table[index] |= mask;
        if mask & CLASS_WHITESPACE_BOUNDARY != 0 {
            self.has_boundary = true;
        }
        self
    }
}

/// Standard byte classes used by the builtin dialect presets.
pub const STANDARD_BYTE_CLASSES: ByteClasses = ByteClasses::STANDARD;

/// PostgreSQL byte classes: [`STANDARD_BYTE_CLASSES`] with the vertical tab
/// (`0x0b`, `\v`) added to [`CLASS_WHITESPACE`].
///
/// PostgreSQL's flex scanner folds its full `space` set `[ \t\n\r\f\v]` as ignorable
/// whitespace. The vertical tab is the one member the shared [`STANDARD_BYTE_CLASSES`]
/// table omits: that table derives whitespace from Rust's [`u8::is_ascii_whitespace`],
/// which covers `\t \n \x0c \r <space>` but not `0x0b`. Probing the engines directly
/// shows the vertical tab is *dialect-specific* rather than a shared control byte:
/// PostgreSQL folds it as ordinary whitespace everywhere (a lone `0x0b` parses as an
/// empty statement, `SELECT\x0b1` as `SELECT 1`), whereas SQLite folds it only as a
/// run continuation and DuckDB only as statement-trim (their own tables — see
/// [`SQLITE_BYTE_CLASSES`] / [`DUCKDB_BYTE_CLASSES`]), and ANSI keeps it strict. So
/// full whitespace-class membership rides only PostgreSQL's (and MySQL's) table.
pub const POSTGRES_BYTE_CLASSES: ByteClasses =
    STANDARD_BYTE_CLASSES.with_class(0x0b, CLASS_WHITESPACE);

/// MySQL byte classes: [`STANDARD_BYTE_CLASSES`] with the vertical tab
/// (`0x0b`, `\v`) added to [`CLASS_WHITESPACE`], exactly as [`POSTGRES_BYTE_CLASSES`].
///
/// MySQL's tokenizer folds the same flex-style `space` set `[ \t\n\r\f\v]` as ignorable
/// whitespace, and the vertical tab is the one member the shared [`STANDARD_BYTE_CLASSES`]
/// table omits (that table derives whitespace from Rust's [`u8::is_ascii_whitespace`],
/// which covers `\t \n \x0c \r <space>` but not `0x0b`). Probing the live `mysql:8` oracle
/// directly confirms it: a lone `0x0b` prepares as an empty statement and `SELECT\x0b1`
/// prepares as `SELECT 1`, so MySQL folds it as ordinary whitespace like PostgreSQL —
/// the second dialect to layer it fully onto the baseline. SQLite and DuckDB fold it only
/// position-dependently (their own tables — see [`SQLITE_BYTE_CLASSES`] /
/// [`DUCKDB_BYTE_CLASSES`]), and ANSI stays strict.
pub const MYSQL_BYTE_CLASSES: ByteClasses =
    STANDARD_BYTE_CLASSES.with_class(0x0b, CLASS_WHITESPACE);

/// SQLite byte classes: [`STANDARD_BYTE_CLASSES`] with the vertical tab (`0x0b`,
/// `\v`) marked [`CLASS_WHITESPACE_CONTINUE`] — a whitespace-run *continuation* that
/// cannot *start* a run.
///
/// SQLite does not fold the vertical tab as ordinary whitespace the way PostgreSQL
/// and MySQL do; its rule is position-dependent, measured against the bundled
/// `rusqlite` (3.x) oracle. SQLite's tokenizer classes `0x0b` as illegal to *begin*
/// a token (`aiClass[0x0b]` is not `CC_SPACE`, so a lone or token-leading `\v` is an
/// "unrecognized token"), yet `sqlite3Isspace(0x0b)` is true, so the space-run loop
/// swallows a `\v` that *follows* another whitespace byte. Net: `"\x20\x0b"` and
/// `"SELECT\x20\x0b1"` accept (the `\v` rides an open run), while lone `"\x0b"`,
/// `"\x0bSELECT 1"`, and `"SELECT\x0b1"` reject (the `\v` would have to start a run).
/// The tokenizer models this by reading `CLASS_WHITESPACE_CONTINUE` only in the
/// run-extension predicate, never at run entry, so no other dialect is affected.
pub const SQLITE_BYTE_CLASSES: ByteClasses =
    STANDARD_BYTE_CLASSES.with_class(0x0b, CLASS_WHITESPACE_CONTINUE);

/// DuckDB byte classes: [`STANDARD_BYTE_CLASSES`] with the vertical tab (`0x0b`,
/// `\v`) marked both [`CLASS_WHITESPACE`] and [`CLASS_WHITESPACE_BOUNDARY`] — it
/// folds as whitespace but is legal only as statement-boundary trim.
///
/// DuckDB's rule, measured against the linked `libduckdb` (1.5.x) oracle, is also
/// position-dependent but *different* from SQLite's: DuckDB trims `0x0b` at the
/// leading and trailing edges of every `;`-delimited statement (so `"\x0b"`,
/// `"\x0bSELECT 1"`, `"SELECT 1\x0b"`, `"SELECT 1;\x0b"`, and `"SELECT 1;\x0bSELECT 2"`
/// all accept) while its statement parser rejects a `\v` interior to a statement's
/// content (`"SELECT\x0b1"`, `"SELECT 1\x0bSELECT 2"`, and even `"SELECT\x20\x0b1"`
/// reject — unlike SQLite, adjacency to a real space does not rescue an interior
/// `\v`). The tokenizer folds the byte as whitespace, then the
/// [`CLASS_WHITESPACE_BOUNDARY`] guard rejects a boundary byte that statement content
/// (including comments) both precedes and follows.
pub const DUCKDB_BYTE_CLASSES: ByteClasses =
    STANDARD_BYTE_CLASSES.with_class(0x0b, CLASS_WHITESPACE | CLASS_WHITESPACE_BOUNDARY);

/// Return all classes for `byte`.
pub const fn byte_class(byte: u8) -> u8 {
    STANDARD_BYTE_CLASSES.byte_class(byte)
}

/// Return true if `byte` has any class in `mask`.
pub const fn has_class(byte: u8, mask: u8) -> bool {
    STANDARD_BYTE_CLASSES.has_class(byte, mask)
}

const fn build_byte_classes() -> ByteClassTable {
    let mut table = [0; 256];
    let mut index = 0;

    while index < 256 {
        let byte = index as u8;
        let mut class = 0;

        if byte.is_ascii_whitespace() {
            class |= CLASS_WHITESPACE;
        }

        if is_identifier_start(byte) {
            class |= CLASS_IDENTIFIER_START;
        }

        if is_identifier_continue(byte) {
            class |= CLASS_IDENTIFIER_CONTINUE;
        }

        if byte.is_ascii_digit() {
            class |= CLASS_DIGIT;
        }

        if is_operator(byte) {
            class |= CLASS_OPERATOR;
        }

        if is_punctuation(byte) {
            class |= CLASS_PUNCTUATION;
        }

        table[index] = class;
        index += 1;
    }

    table
}

const fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit() || byte >= 0x80
}

const fn is_operator(byte: u8) -> bool {
    matches!(
        byte,
        b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'!' | b'|' | b'&' | b'^' | b'~'
    )
}

const fn is_punctuation(byte: u8) -> bool {
    // `:` is structural punctuation: a lone `:` is the array-slice separator and
    // `::` is the PostgreSQL typecast operator (the scanner munches `::` first).
    matches!(
        byte,
        b'(' | b')' | b',' | b';' | b'.' | b'[' | b']' | b'{' | b'}' | b':'
    )
}
