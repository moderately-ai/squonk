-- SPDX-License-Identifier: PostgreSQL
--
-- PostgreSQL-regression-derived guide fixtures.
--
-- These statements are intentionally not in regress-supported.sql yet. They are
-- kept as concrete source-backed gaps so parser work can promote them into the
-- executable structural corpus instead of rediscovering omitted examples.
--
-- Each case uses:
--   -- case: stable-case-id
--   -- source: upstream-file.sql:line
--   -- ticket: <stable provenance label>

-- case: create-table-setup
-- source: join.sql:6
-- ticket: prod-pg-map-ddl-dml
CREATE TABLE J1_TBL (
  i integer,
  j integer,
  t text
);

-- case: insert-values-setup
-- source: join.sql:18
-- ticket: prod-pg-map-ddl-dml
INSERT INTO J1_TBL VALUES (1, 4, 'one');

-- case: analyze-utility
-- source: select.sql:62
-- ticket: prod-sql-copy-explain-utility
ANALYZE onek2;

-- case: table-alias-column-list
-- source: join.sql:52
-- ticket: prod-sql-select-advanced-joins
SELECT *
  FROM J1_TBL AS t1 (a, b, c);

-- case: row-value-in-values
-- source: select.sql:133
-- ticket: prod-sql-subquery-predicates
select * from onek
    where (unique1,ten) in (values (1,1), (20,0), (99,9), (17,99))
    order by unique1;

-- case: lateral-values
-- source: select.sql:149
-- ticket: prod-sql-select-advanced-joins
SELECT * FROM nocols n, LATERAL (VALUES(n.*)) v;

-- case: trim-special-form
-- source: union.sql:90
-- ticket: prod-sql-expr-case-functions
SELECT f1 AS five FROM TEXT_TBL
UNION
SELECT f1 FROM VARCHAR_TBL
UNION
SELECT TRIM(TRAILING FROM f1) FROM CHAR_TBL
ORDER BY 1;
