-- SQLite-specific feature probe corpus (self-authored; see README.md).
-- Every statement is schema-independent: a bare in-memory SQLite prepare()
-- accepts it with no provisioned schema, so the sweep compares it cleanly
-- against SqliteOracle::new(). Blank lines and `--` comment lines are ignored
-- by the loader; each remaining line is one statement (no trailing `;`).
-- Grouped by the grammar family the sweep's divergence inventory reports.

-- identifiers: SQLite accepts backtick and [bracket] quoted identifiers, and
-- (the DQS misfeature) falls a double-quoted token back to a string constant
-- when it resolves to no identifier.
SELECT 1 AS `back_ticked`
SELECT 1 AS [bracketed]
SELECT `back_ticked` FROM (SELECT 1 AS `back_ticked`) AS `sub`
SELECT [bracketed] FROM (SELECT 1 AS [bracketed]) AS [sub]
SELECT "double quoted string fallback"

-- operators: the `==` equality spelling, `IS`/`IS NOT` over non-NULL operands,
-- the built-in GLOB pattern operator, and bitwise operators.
SELECT 1 == 1
SELECT 1 IS 1
SELECT 1 IS NOT 2
SELECT 'abc' GLOB 'a*'
SELECT 'abc' NOT GLOB 'a*'
SELECT 2 | 1, 3 & 2, ~5, 1 << 4, 8 >> 1

-- numeric literals: hexadecimal integers.
SELECT 0x1F
SELECT 0xFF + 1

-- bind parameters: SQLite accepts `?`, `?NNN`, `:name`, `@name`, and `$name`.
SELECT ?
SELECT ?1
SELECT :name
SELECT @name
SELECT $value
SELECT :a + :b * ?3

-- row limiting: the `LIMIT <count>, <offset>` two-argument comma spelling.
SELECT 1 LIMIT 2, 3

-- config / utility statements: PRAGMA (bare, assignment, and call forms),
-- ATTACH (with and without the optional DATABASE keyword), VACUUM, REINDEX,
-- and ANALYZE.
PRAGMA user_version
PRAGMA foreign_keys = ON
PRAGMA journal_mode = WAL
PRAGMA cache_size = -2000
PRAGMA optimize
PRAGMA table_info(sqlite_master)
ATTACH DATABASE ':memory:' AS aux
ATTACH ':memory:' AS aux2
VACUUM
VACUUM main
REINDEX
ANALYZE
ANALYZE sqlite_master

-- CREATE TABLE: AUTOINCREMENT, WITHOUT ROWID, STRICT, typeless columns, an
-- arbitrary (affinity) type name, an integer display width on a built-in integer
-- (INT(11)-style, MySQL-canonical, absorbed by SQLite affinity), column COLLATE,
-- generated columns (both the shorthand and GENERATED ALWAYS forms), a column-level
-- ON CONFLICT clause, an inline PRIMARY KEY sort order, a parenthesized DEFAULT
-- expression, IF NOT EXISTS, and a TEMP table.
CREATE TABLE tbl_auto(a INTEGER PRIMARY KEY AUTOINCREMENT)
CREATE TABLE tbl_without_rowid(a INTEGER PRIMARY KEY, b TEXT) WITHOUT ROWID
CREATE TABLE tbl_strict(a INTEGER, b TEXT) STRICT
CREATE TABLE tbl_typeless(a, b, c)
CREATE TABLE tbl_affinity(a BANANA, b CARROT(3))
CREATE TABLE tbl_int_width(a INT(11))
CREATE TABLE tbl_int_width_default(a INT(11) NOT NULL DEFAULT -1)
CREATE TABLE tbl_bigint_width(a BIGINT(20))
CREATE TABLE tbl_collate(a TEXT COLLATE NOCASE)
CREATE TABLE tbl_gen_short(a INTEGER, b AS (a * 2))
CREATE TABLE tbl_gen_stored(a INTEGER, b INTEGER GENERATED ALWAYS AS (a + 1) STORED)
CREATE TABLE tbl_col_conflict(a INTEGER UNIQUE ON CONFLICT REPLACE)
CREATE TABLE tbl_pk_desc(id INTEGER PRIMARY KEY DESC)
CREATE TABLE tbl_default_expr(a TEXT DEFAULT (datetime('now')))
CREATE TABLE IF NOT EXISTS tbl_if_not_exists(a INTEGER)
CREATE TEMP TABLE tbl_temp(a INTEGER)

-- CREATE VIEW over a constant select (schema-independent).
CREATE VIEW v_const AS SELECT 1 AS a
CREATE VIEW IF NOT EXISTS v_const2 AS SELECT 2 AS b

-- value expressions: boolean keywords, the datetime keywords, the iif() builtin,
-- an explicit CAST to an arbitrary affinity type, the JSON `->` accessor, and a
-- schema-independent recursive CTE.
SELECT TRUE, FALSE
SELECT CURRENT_TIME, CURRENT_DATE, CURRENT_TIMESTAMP
SELECT iif(1 > 0, 'yes', 'no')
SELECT CAST(1 AS BANANA)
SELECT '{"a":1}' -> '$.a'
WITH RECURSIVE cnt(x) AS (VALUES(1) UNION ALL SELECT x + 1 FROM cnt WHERE x < 5) SELECT x FROM cnt
