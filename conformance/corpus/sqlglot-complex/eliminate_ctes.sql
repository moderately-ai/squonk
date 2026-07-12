-- SPDX-License-Identifier: MIT
--
-- sqlglot complex-query corpus: eliminate_ctes optimizer fixtures (GENERATED — do not edit by hand).
-- Source: tests/fixtures/optimizer/eliminate_ctes.sql @ sqlglot fd6d4d61c25e7918118fc22c5579098a86a58e10
-- Extraction: strip comment lines, split on the statement terminator, keep
-- the even-indexed (input) statements (the odd-indexed entries are sqlglot's
-- optimized output). See README.md / PROVENANCE.toml for full provenance.
--
-- 6 statements, one per terminated entry, verbatim from upstream.

WITH q AS (
  SELECT
    a
  FROM x
)
SELECT
  a
FROM x;

SELECT
  a
FROM (
  WITH q AS (
    SELECT
      a
    FROM x
  )
  SELECT a FROM x
);

WITH q AS (
  SELECT
    a
  FROM x
), r AS (
  SELECT
    a
  FROM q
)
SELECT
  a
FROM x;

WITH q AS (
  SELECT
    a
  FROM y
)
SELECT
  a
FROM x AS q
WHERE
  a IN (
    SELECT
      a
    FROM q
  );

WITH q AS (
  SELECT
    a
  FROM y
), q2 AS (
  SELECT
    a
  FROM y
)
SELECT
  a
FROM q2 AS q
WHERE
  a IN (
    SELECT
      a
    FROM q
  );

WITH t1 AS (
  SELECT
    1 AS foo
), t2 AS (
  SELECT
    1 AS foo
)
SELECT
  *
FROM t1
LEFT ANTI JOIN t2
  ON t1.foo = t2.foo;

