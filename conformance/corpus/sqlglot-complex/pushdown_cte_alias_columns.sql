-- SPDX-License-Identifier: MIT
--
-- sqlglot complex-query corpus: pushdown_cte_alias_columns optimizer fixtures (GENERATED — do not edit by hand).
-- Source: tests/fixtures/optimizer/pushdown_cte_alias_columns.sql @ sqlglot fd6d4d61c25e7918118fc22c5579098a86a58e10
-- Extraction: strip comment lines, split on the statement terminator, keep
-- the even-indexed (input) statements (the odd-indexed entries are sqlglot's
-- optimized output). See README.md / PROVENANCE.toml for full provenance.
--
-- 6 statements, one per terminated entry, verbatim from upstream.

WITH y(c) AS (SELECT SUM(a) FROM (SELECT 1 a) AS x HAVING c > 0) SELECT c FROM y;

WITH y(c) AS (SELECT SUM(a) as d FROM (SELECT 1 a) AS x HAVING c > 0) SELECT c FROM y;

WITH x(c) AS (SELECT SUM(1) a HAVING c > 0 LIMIT 1) SELECT * FROM x;

WITH x(c) AS ((SELECT 1 a) HAVING c > 0) SELECT * FROM x;

WITH x(c) AS ((SELECT SUM(1) a) HAVING c > 0 LIMIT 1) SELECT * FROM x;

WITH x(c) AS (SELECT SUM(a) FROM x HAVING c > 0 UNION ALL SELECT SUM(a) FROM y HAVING c > 0) SELECT * FROM x;

