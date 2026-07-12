-- SPDX-License-Identifier: MIT
--
-- sqlglot complex-query corpus: unnest_subqueries optimizer fixtures (GENERATED — do not edit by hand).
-- Source: tests/fixtures/optimizer/unnest_subqueries.sql @ sqlglot fd6d4d61c25e7918118fc22c5579098a86a58e10
-- Extraction: strip comment lines, split on the statement terminator, keep
-- the even-indexed (input) statements (the odd-indexed entries are sqlglot's
-- optimized output). See README.md / PROVENANCE.toml for full provenance.
--
-- 38 statements, one per terminated entry, verbatim from upstream.

SELECT * FROM x WHERE x.a = (SELECT SUM(y.a) AS a FROM y);

SELECT * FROM x WHERE x.a IN (SELECT y.a AS a FROM y);

SELECT * FROM x WHERE x.a IN (SELECT y.b AS b FROM y);

SELECT * FROM x WHERE x.a = ANY (SELECT y.a AS a FROM y);

SELECT * FROM x WHERE x.a = (SELECT SUM(y.b) AS b FROM y WHERE x.a = y.a);

SELECT * FROM x WHERE x.a > (SELECT SUM(y.b) AS b FROM y WHERE x.a = y.a);

SELECT * FROM x WHERE x.a <> ANY (SELECT y.a AS a FROM y WHERE y.a = x.a);

SELECT * FROM x WHERE x.a NOT IN (SELECT y.a AS a FROM y WHERE y.a = x.a);

SELECT * FROM x WHERE x.a IN (SELECT y.a AS a FROM y WHERE y.b = x.a);

SELECT * FROM x WHERE x.a < (SELECT SUM(y.a) AS a FROM y WHERE y.a = x.a and y.a = x.b and y.b <> x.d);

SELECT * FROM x WHERE EXISTS (SELECT y.a AS a, y.b AS b FROM y WHERE x.a = y.a);

SELECT * FROM x WHERE x.a IN (SELECT y.a AS a FROM y LIMIT 10);

SELECT * FROM x.a WHERE x.a IN (SELECT y.a AS a FROM y OFFSET 10);

SELECT * FROM x.a WHERE x.a IN (SELECT y.a AS a, y.b AS b FROM y);

SELECT * FROM x.a WHERE x.a > ANY (SELECT y.a FROM y);

SELECT * FROM x WHERE x.a = (SELECT SUM(y.c) AS c FROM y WHERE y.a = x.a LIMIT 10);

SELECT * FROM x WHERE x.a = (SELECT SUM(y.c) AS c FROM y WHERE y.a = x.a OFFSET 10);

SELECT * FROM x WHERE x.a > ALL (SELECT y.c AS c FROM y WHERE y.a = x.a);

SELECT * FROM x WHERE x.a > (SELECT COUNT(*) as d FROM y WHERE y.a = x.a);

SELECT * FROM x WHERE x.a = SUM(SELECT 1);

SELECT * FROM x WHERE x.a IN (SELECT max(y.b) AS b FROM y GROUP BY y.a);

SELECT x.a > (SELECT SUM(y.a) AS b FROM y) FROM x;

SELECT (SELECT MAX(t2.c1) AS c1 FROM t2 WHERE t2.c2 = t1.c2 AND t2.c3 <= TRUNC(t1.c3)) AS c FROM t1;

SELECT s.t AS t FROM s WHERE 1 IN (SELECT t.a AS a FROM t WHERE t.b > 1);

SELECT s.t FROM s WHERE 1 IN (SELECT MAX(t.a) AS t1 FROM t);

SELECT s.t FROM s WHERE 1 IN (SELECT MAX(t.a) + 1 AS t1 FROM t);

SELECT BIT_COUNT(EXISTS(SELECT 1 WHERE FALSE)) AS col FROM t0;

SELECT EXISTS (SELECT 1 WHERE FALSE) AS ref0 FROM t1, t0 GROUP BY t0.c2;

SELECT EXISTS (SELECT 1 WHERE TRUE) AS ref0 FROM t1, t0 GROUP BY t0.c2;

SELECT EXISTS (SELECT 1 WHERE FALSE) AS ref0, EXISTS (SELECT 1 WHERE TRUE) AS ref1 FROM t1, t0 GROUP BY t0.c2;

SELECT EXISTS (SELECT 1 WHERE FALSE) AS ref0 FROM t1 GROUP BY t1.c0 HAVING COUNT(*) > 0;

WITH t2 AS (SELECT CAST(t1.c1 AS BIGINT) AS ref1 FROM GENERATE_SERIES((SELECT MAX(x.a) FROM x AS x), 10, 1) AS t1(c1)) SELECT t2.ref1 AS ref1 FROM t2 AS t2;

WITH t2 AS (SELECT t1.c1 FROM UNNEST((SELECT ARRAY(x.a) FROM x)) AS t1(c1)) SELECT t2.c1 FROM t2;

SELECT t1.c1 > (SELECT SUM(y.a) AS b FROM y) FROM x JOIN GENERATE_SERIES((SELECT MAX(x.a) FROM x AS x), 10, 1) AS t1(c1) ON t1.c1 > x.a;

SELECT COALESCE((SELECT MAX(b.val) FROM t b WHERE b.val < a.val AND b.id = a.id), a.val) AS result FROM t a;

SELECT * FROM x WHERE x.a IN (SELECT y.a AS a FROM y UNION ALL SELECT z.a AS a FROM z);

SELECT * FROM x WHERE x.a NOT IN (SELECT y.a AS a FROM y);

SELECT * FROM x WHERE x.a NOT IN (SELECT y.a AS a FROM y UNION ALL SELECT z.a AS a FROM z);

