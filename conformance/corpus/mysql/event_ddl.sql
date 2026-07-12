-- SPDX-License-Identifier: CC0-1.0
-- Self-authored MySQL scheduled-event DDL oracle corpus (parse-mysql-event-ddl).
--
-- QUARANTINED from the PREPARE-only corpora, exactly like `routine_bodies.sql`: CREATE/ALTER/
-- DROP EVENT is not preparable (ER_NO_DB_ERROR 1046 / ER_UNSUPPORTED_PS 1295 under PREPARE —
-- the administrative surface), so these are evidenced through the COM_QUERY
-- `MySqlOracle::ddl_verdict` define-not-execute channel: each statement runs inside a fresh,
-- uniquely-named scratch database dropped unconditionally afterward, so nothing survives and
-- the never-execute PREPARE corpora are untouched.
--
-- Each `-- accept:` / `-- reject:` header names the expected server verdict for the single
-- following statement line: `accept` == the server defined the event (a valid statement);
-- `reject` == the server's parser rejected it with ER_PARSE_ERROR (1064), a genuine grammar
-- error. The scratch database isolates every case, so all statements may reuse the name `e`.
--
-- Only server-Accept and server-1064 outcomes belong here (the harness asserts exactly those).
-- Cases that are grammar-valid but bind-reject (a missing-event ALTER/DROP is 1539; an event
-- body `RETURN` is 1313; `EVERY … DAY_MICROSECOND` is 1235-unsupported) are evidenced at the
-- parser round-trip layer instead — they are neither a define-time Accept nor a 1064 syntax
-- reject, so they cannot ride this channel.

-- The AT one-shot schedule and a BEGIN … END body.
-- accept:
CREATE EVENT e ON SCHEDULE AT NOW() DO BEGIN END
-- The IF NOT EXISTS guard and a single-statement (SELECT) body.
-- accept:
CREATE EVENT IF NOT EXISTS e ON SCHEDULE AT NOW() DO SELECT 1
-- The DEFINER = <user> prefix (reusing the shared routine account reference).
-- accept:
CREATE DEFINER = root EVENT e ON SCHEDULE AT NOW() DO BEGIN END
-- accept:
CREATE DEFINER = CURRENT_USER EVENT e ON SCHEDULE AT NOW() DO BEGIN END
-- The recurring EVERY schedule with a STARTS window bound.
-- accept:
CREATE EVENT e ON SCHEDULE EVERY 1 HOUR STARTS NOW() DO BEGIN END
-- A MySQL composite interval unit (underscore spelling), reusing the shared IntervalFields.
-- accept:
CREATE EVENT e ON SCHEDULE EVERY 2 DAY_HOUR DO BEGIN END
-- The full clause set in fixed grammar order: completion, status, comment.
-- accept:
CREATE EVENT e ON SCHEDULE AT NOW() ON COMPLETION NOT PRESERVE ENABLE COMMENT 'c' DO BEGIN END
-- Both replica spellings the 8.4 server still admits (SLAVE deprecated, REPLICA current).
-- accept:
CREATE EVENT e ON SCHEDULE AT NOW() DISABLE ON SLAVE DO BEGIN END
-- accept:
CREATE EVENT e ON SCHEDULE AT NOW() DISABLE ON REPLICA DO BEGIN END
-- The clause order is fixed: a COMMENT before the status clause is a syntax error.
-- reject:
CREATE EVENT e ON SCHEDULE AT NOW() COMMENT 'c' ENABLE DO BEGIN END
-- ALTER EVENT requires at least one clause; a bare target is a syntax error.
-- reject:
ALTER EVENT e
-- DROP EVENT names exactly one event; a comma list is a syntax error.
-- reject:
DROP EVENT a, b
