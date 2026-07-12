-- SPDX-License-Identifier: MIT
--
-- sqlglot complex-query corpus: merge_subqueries optimizer fixtures (GENERATED — do not edit by hand).
-- Source: tests/fixtures/optimizer/merge_subqueries.sql @ sqlglot fd6d4d61c25e7918118fc22c5579098a86a58e10
-- Extraction: strip comment lines, split on the statement terminator, keep
-- the even-indexed (input) statements (the odd-indexed entries are sqlglot's
-- optimized output). See README.md / PROVENANCE.toml for full provenance.
--
-- 67 statements, one per terminated entry, verbatim from upstream.

SELECT a, b FROM (SELECT a, b FROM x);

SELECT c * 2 AS d FROM (SELECT a + b AS c FROM x);

SELECT c + d AS e FROM (SELECT a + b AS c, a AS d FROM x);

WITH cte AS (SELECT a * b AS c, a AS d FROM x) SELECT c + d AS e FROM cte;

SELECT 2 * foo AS bar FROM (SELECT CAST(b AS DOUBLE) AS foo FROM x);

SELECT foo * 2 AS bar FROM (SELECT (1 + 2 + 3) AS foo FROM x);

SELECT a, b FROM (SELECT a, b FROM x AS q) AS r;

SELECT a, b FROM (SELECT a, b FROM (SELECT a, b FROM x));

SELECT a, SUM(b) AS b FROM (SELECT a, b FROM x WHERE a > 1) GROUP BY a;

SELECT a, c FROM (SELECT a, b FROM x WHERE a > 1) AS x JOIN y ON x.b = y.b;

SELECT a, c FROM (SELECT a, b FROM x WHERE a > 1) AS x JOIN y ON x.b = y.b;

SELECT a, c FROM x JOIN (SELECT b, c FROM y) AS y ON x.b = y.b;

SELECT a, c FROM (SELECT a, c FROM x JOIN y ON x.b = y.b);

SELECT a, c FROM (SELECT q.a, q.b FROM x AS q) AS x JOIN y AS q ON x.b = q.b;

SELECT x.a, q.c FROM (SELECT a, x.b FROM x JOIN y AS q ON x.b = q.b) AS x JOIN y AS q ON x.b = q.b;

SELECT x.a, q.c, r.c FROM (SELECT q.a, r.b FROM x AS q JOIN y AS r ON q.b = r.b) AS x JOIN y AS q ON x.b = q.b JOIN y AS r ON x.b = r.b ORDER BY x.a, q.c, r.c;

SELECT r.b FROM (SELECT b FROM x AS x) AS q JOIN (SELECT b FROM x) AS r ON q.b = r.b;

SELECT x.a, y.c FROM x JOIN (SELECT b, c FROM y WHERE c > 1) AS y ON x.b = y.b ORDER BY x.a;

SELECT x.a, y.c FROM (SELECT a FROM x) AS x, (SELECT c FROM y) AS y;

SELECT x.a, x.c FROM (SELECT x.a, z.c FROM x, y AS z) AS x;

SELECT * FROM (SELECT * FROM (SELECT * FROM x)) ORDER BY a LIMIT 1;

WITH x AS (SELECT a, b FROM main.x) SELECT a, b FROM x;

WITH y AS (SELECT a, b FROM x) SELECT a, b FROM y AS z;

WITH x2 AS (SELECT a FROM main.x), x3 AS (SELECT a FROM x2) SELECT a FROM x3;

WITH x AS (SELECT a, b FROM main.x WHERE a > 1) SELECT a, SUM(b) AS b FROM x GROUP BY a;

WITH x2 AS (SELECT a, b FROM x WHERE a > 1) SELECT a, c FROM x2 AS x JOIN y ON x.b = y.b;

WITH y AS (SELECT a, b FROM x AS q) SELECT a, b FROM y AS z;

SELECT * FROM (WITH x AS (SELECT a, b FROM main.x) SELECT a, b FROM x);

SELECT a FROM (SELECT a FROM (SELECT COALESCE(a) AS a FROM x LEFT JOIN y ON x.a = y.b) AS x) AS x;

WITH x2 AS (SELECT COALESCE(a) AS a FROM x LEFT JOIN y ON x.a = y.b) SELECT a FROM (SELECT a FROM x2 AS x) AS x;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x WHERE x.b = 1) AS x FULL OUTER JOIN (SELECT y.b AS b FROM y AS y WHERE y.b = 2) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x) AS x FULL OUTER JOIN (SELECT y.b AS b FROM y AS y) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x WHERE x.b = 1) AS x LEFT JOIN (SELECT y.b AS b FROM y AS y WHERE y.b = 2) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x) AS x LEFT JOIN (SELECT y.b AS b FROM y AS y) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x WHERE x.b = 1) AS x RIGHT JOIN (SELECT y.b AS b FROM y AS y WHERE y.b = 2) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x) AS x RIGHT JOIN (SELECT y.b AS b FROM y AS y) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x WHERE x.b = 1) AS x INNER JOIN (SELECT y.b AS b FROM y AS y WHERE y.b = 2) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x) AS x INNER JOIN (SELECT y.b AS b FROM y AS y) AS y ON x.b = y.b;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x WHERE x.b = 1) AS x CROSS JOIN (SELECT y.b AS b FROM y AS y WHERE y.b = 2) AS y;

SELECT x.b AS b, y.b AS b2 FROM (SELECT x.b AS b FROM x AS x) AS x CROSS JOIN (SELECT y.b AS b FROM y AS y) AS y;

WITH m AS (SELECT x.a, x.b FROM x), n AS (SELECT y.b, y.c FROM y), joined as (SELECT /*+ BROADCAST(k) */ m.a, k.c FROM m JOIN n AS k ON m.b = k.b) SELECT joined.a, joined.c FROM joined;

WITH m AS (SELECT x.a, x.b FROM x), n AS (SELECT y.b, y.c FROM y), joined as (SELECT /*+ BROADCAST(m, n) */ m.a, n.c FROM m JOIN n ON m.b = n.b) SELECT joined.a, joined.c FROM joined;

WITH m AS (SELECT x.a, x.b FROM x), n AS (SELECT y.b, y.c FROM y), joined as (SELECT /*+ BROADCAST(m), MERGE(m, n) */ m.a, n.c FROM m JOIN n ON m.b = n.b) SELECT joined.a, joined.c FROM joined;

WITH m AS (SELECT x.a, x.b FROM x), n AS (SELECT y.b, y.c FROM y), joined as (SELECT /*+ BROADCAST(m), MERGE(m, n) */ m.a, n.c FROM m JOIN n ON m.b = n.b) SELECT /*+ COALESCE(3) */ joined.a, joined.c FROM joined;

SELECT
    subquery.a,
    subquery.c
FROM (
    SELECT /*+ BROADCAST(m), MERGE(m, n) */ m.a, n.c FROM (SELECT x.a, x.b FROM x) AS m JOIN (SELECT y.b, y.c FROM y) AS n ON m.b = n.b
) AS subquery;

SELECT /*+ BROADCAST(x) */
  x.a,
  x.c
FROM (
  SELECT
    x.a,
    x.c
  FROM (
    SELECT
      x.a,
      COUNT(1) AS c
    FROM x
    GROUP BY x.a
  ) AS x
) AS x;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
  ORDER BY x.a, x.b, row_num
)
SELECT
  t1.a,
  t1.b
FROM
  t1
WHERE
  row_num = 1;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
)
SELECT
  t1.a,
  t1.b
FROM t1 JOIN y ON t1.a = y.c AND t1.row_num = 1;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
)
SELECT
  SUM(t1.row_num) as total_rows
FROM
  t1;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
)
SELECT
  t1.row_num AS row_num,
  SUM(t1.a) AS total
FROM
  t1
GROUP BY t1.row_num
ORDER BY t1.row_num;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
)
SELECT
  t1.row_num AS row_num,
  t1.a AS a
FROM
  t1
ORDER BY t1.row_num, t1.a;

WITH t1 AS (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) - 1 AS row_num
  FROM
    x
)
SELECT
  t1.row_num AS row_num,
  t1.a AS a
FROM
  t1
ORDER BY t1.row_num, t1.a;

with t1 as (
  SELECT
    x.a,
    x.b,
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
  ORDER BY x.a, x.b, row_num
)
SELECT
  t1.a,
  t1.b,
  t1.row_num
FROM
  t1;

WITH t AS (SELECT t1.x AS x, t1.y AS y, t2.a AS a, t2.b AS b FROM t1 AS t1(x, y) CROSS JOIN t2 AS t2(a, b) ORDER BY t2.a) SELECT t.x AS x, t.y AS y, t.a AS a, t.b AS b FROM t AS t;

with t1 as (
  SELECT
    ROW_NUMBER() OVER (PARTITION BY x.a ORDER BY x.a) as row_num
  FROM
    x
)
SELECT
  t2.row_num
FROM
  t1 AS t2
WHERE
  t2.row_num = 2;

WITH t1 AS (
  SELECT
    a1.cola
  FROM
    VALUES (1) AS a1(cola)
), t2 AS (
  SELECT
    a2.cola
  FROM
    VALUES (1) AS a2(cola)
)
SELECT /*+ BROADCAST(t2) */
  t1.cola,
  t2.cola,
FROM
  t1
  JOIN
  t2
  ON
    t1.cola = t2.cola;

WITH i AS (
  SELECT
    x.a AS a
  FROM x AS x
), j AS (
  SELECT
    x.a,
    x.b
  FROM x AS x
), k AS (
  SELECT
    j.a,
    j.b
  FROM j AS j
)
SELECT
  i.a,
  k.b
FROM i AS i
LEFT JOIN k AS k
ON  i.a = k.a;

WITH i AS (
  SELECT
    x.a AS a
  FROM y AS y
  JOIN x AS x
    ON y.b = x.b
)
SELECT
  x.a AS a
FROM x AS x
LEFT JOIN i AS i
  ON x.a = i.a;

WITH _q_0 AS (SELECT t1.c AS c FROM t1 AS t1) SELECT * FROM (_q_0 AS _q_0 CROSS JOIN t2 AS t2);

WITH _q_0 AS (
  SELECT
    x.a AS a
  FROM x AS x
), y_2 AS (
  SELECT
    y.b AS b
  FROM y AS y
)
SELECT
  y.b AS b
FROM (
  _q_0 AS _q_0
    JOIN y_2 AS y
      ON _q_0.a = y.b
);

WITH q AS (
  SELECT
    y.b AS a
  FROM y AS y
)
SELECT
  q.a AS a
FROM x AS q
WHERE
  q.a IN (
    SELECT
      q.a AS a
    FROM q AS q
  );

WITH q AS (
  SELECT
    x.a AS a
  FROM x
  ORDER BY x.a
)
SELECT
  q.a AS a
FROM q
UNION ALL
SELECT
  1 AS a;

WITH tbl AS (select 1 as id)
SELECT
  id
FROM (
  SELECT OTBL.id
  FROM (
    SELECT OTBL.id
    FROM (
      SELECT OTBL.id
      FROM tbl AS OTBL
      LEFT OUTER JOIN tbl AS ITBL ON OTBL.id = ITBL.id
    ) AS OTBL
    LEFT OUTER JOIN tbl AS ITBL ON OTBL.id = ITBL.id
  ) AS OTBL
  LEFT OUTER JOIN tbl AS ITBL ON OTBL.id = ITBL.id
) AS ITBL;

WITH i AS (
  SELECT
    a
  FROM (
    SELECT 1 a
  ) AS conflict
), j AS (
  SELECT 1 AS a
)
SELECT
  i.a,
  conflict.a
FROM i
LEFT JOIN j AS conflict
  ON i.a = conflict.a;

with cte as (
    select x.a * x.b as mult from x
)
select cte.mult from cte;

WITH t0 AS (
  SELECT
    5 AS id
), t1 AS (
  SELECT
    1 AS id,
    'US' AS cid
), t2 AS (
  SELECT
    1 AS id,
    'US' AS cid
)
SELECT
  t0.id,
  t3.cid AS cid
FROM t0
INNER JOIN (
  SELECT
    t1.id,
    t2.cid
  FROM t1
  RIGHT JOIN t2
    ON t1.cid = t2.cid
) AS t3
  ON t0.id = t3.id;

WITH t1 AS (SELECT 1 AS col) SELECT a, SUM(b) AS b FROM (SELECT 6 AS a, col AS b FROM t1) AS t GROUP BY a ORDER BY a;

