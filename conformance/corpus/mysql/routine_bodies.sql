-- SPDX-License-Identifier: CC0-1.0
-- Self-authored MySQL stored-routine body oracle corpus (parse-mysql-routine-ddl).
--
-- QUARANTINED from the PREPARE-only corpora: a CREATE PROCEDURE/FUNCTION is not
-- preparable (ER_UNSUPPORTED_PS 1295 under PREPARE — grammar-positive but blind to the
-- routine body), so these are evidenced through the COM_QUERY `MySqlOracle::ddl_verdict`
-- define-not-execute channel instead: each statement is run inside a fresh, uniquely-named
-- scratch database that is dropped unconditionally afterward, so nothing survives and the
-- never-execute PREPARE corpora are untouched.
--
-- Each `-- accept:` / `-- reject:` header names the expected server verdict for the single
-- following statement line: `accept` == the server defined the routine (a valid body);
-- `reject` == the server's stored-program parser rejected the body with ER_PARSE_ERROR
-- (1064) — a genuine grammar error in the body, the channel the spike proved. The scratch
-- database isolates every case, so all statements may reuse the names `p`/`f`.

-- accept:
CREATE PROCEDURE p() BEGIN END
-- accept:
CREATE PROCEDURE p(IN a INT, OUT b INT, INOUT c INT) BEGIN SET b = a; SET c = a; END
-- accept:
CREATE PROCEDURE p() LANGUAGE SQL NOT DETERMINISTIC MODIFIES SQL DATA SQL SECURITY INVOKER COMMENT 'doc' BEGIN SELECT 1; END
-- accept:
CREATE PROCEDURE p() BEGIN DECLARE v INT DEFAULT 0; WHILE v < 3 DO SELECT v; END WHILE; END
-- accept:
CREATE PROCEDURE p() BEGIN IF 1 THEN SELECT 1; ELSE SELECT 2; END IF; END
-- accept:
CREATE FUNCTION f() RETURNS INT DETERMINISTIC RETURN 1
-- accept:
CREATE FUNCTION f(x INT) RETURNS INT DETERMINISTIC BEGIN RETURN x + 1; END
-- accept: LEAVE/ITERATE resolve a loop label lexically in scope
CREATE PROCEDURE p() BEGIN lp: LOOP ITERATE lp; LEAVE lp; END LOOP lp; END
-- accept: LEAVE resolves an enclosing BEGIN...END block label from a nested loop
CREATE PROCEDURE p() BEGIN blk: BEGIN lp: LOOP LEAVE blk; END LOOP lp; END blk; END

-- reject: a misspelled statement keyword inside the compound body (ER_PARSE_ERROR 1064)
CREATE PROCEDURE p() BEGIN SELCT 1; END
-- reject: an `IF` with no closing `END IF` (ER_PARSE_ERROR 1064)
CREATE PROCEDURE p() BEGIN IF 1 THEN SELECT 1; END
-- reject: `LEAVE` with no target label (ER_PARSE_ERROR 1064)
CREATE PROCEDURE p() BEGIN LEAVE; END
-- reject: a stored function with no RETURN value expression (ER_PARSE_ERROR 1064)
CREATE FUNCTION f() RETURNS INT DETERMINISTIC RETURN
-- reject: MySQL's routine LANGUAGE admits only the bare word SQL; the string spelling
-- LANGUAGE 'SQL' is ER_PARSE_ERROR 1064 (routine-language-name-word-or-sconst boundary)
CREATE PROCEDURE p() LANGUAGE 'SQL' BEGIN SELECT 1; END
