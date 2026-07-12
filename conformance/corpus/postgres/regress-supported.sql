-- SPDX-License-Identifier: PostgreSQL
--
-- PostgreSQL-regression-derived supported subset.
-- Source checkout:
--   https://github.com/postgres/postgres
-- Source files:
--   src/test/regress/sql/select.sql
--   src/test/regress/sql/with.sql
--   src/test/regress/sql/union.sql
--   src/test/regress/sql/join.sql
--
-- Statements with uppercase fixture table names from join.sql are case-normalized
-- to lowercase because the current structural oracle compares resolved
-- identifiers exactly; unquoted PostgreSQL identifier folding is tracked outside
-- this corpus ticket.

SELECT * FROM onek
   WHERE onek.unique1 < 10
   ORDER BY onek.unique1;

SELECT onek2.* FROM onek2 WHERE onek2.unique1 < 10;

SELECT onek2.unique1, onek2.stringu1 FROM onek2
   WHERE onek2.unique1 > 980;

SELECT p.name, p.age FROM person* p;

select foo from (select 1 offset 0) as foo;

select foo from (select null offset 0) as foo;

VALUES (1,2), (3,4+4), (7,77.7);

select * from onek, (values(147, 'RFAAAA'), (931, 'VJAAAA')) as v (i, j)
    WHERE onek.unique1 = v.i and onek.stringu1 = v.j;

SELECT * FROM foo ORDER BY f1;

SELECT * FROM foo ORDER BY f1 ASC;

SELECT * FROM foo ORDER BY f1 NULLS FIRST;

SELECT * FROM foo ORDER BY f1 DESC NULLS LAST;

WITH q1(x,y) AS (SELECT 1,2)
SELECT * FROM q1, q1 AS q2;

SELECT 1 AS two UNION SELECT 2 ORDER BY 1;

SELECT 1 AS two UNION ALL SELECT 2;

SELECT 1.1 AS two UNION (SELECT 2 UNION ALL SELECT 2) ORDER BY 1;

SELECT 1 AS one UNION SELECT 1.0::float8 ORDER BY 1;

SELECT q2 FROM int8_tbl INTERSECT SELECT q1 FROM int8_tbl ORDER BY 1;

SELECT q2 FROM int8_tbl INTERSECT ALL SELECT q1 FROM int8_tbl ORDER BY 1;

SELECT q2 FROM int8_tbl EXCEPT SELECT q1 FROM int8_tbl ORDER BY 1;

SELECT q2 FROM int8_tbl EXCEPT ALL SELECT q1 FROM int8_tbl ORDER BY 1;

SELECT q1 FROM int8_tbl EXCEPT ALL SELECT q1 FROM int8_tbl FOR NO KEY UPDATE;

select *
  from j1_tbl cross join j2_tbl;

select *
  from j1_tbl inner join j2_tbl using (i);

select *
  from j1_tbl join j2_tbl on (j1_tbl.i <= j2_tbl.k);

select *
  from j1_tbl natural join j2_tbl;

select *
  from j1_tbl left outer join j2_tbl using (i)
  order by i, k, t;

select *
  from j1_tbl right join j2_tbl using (i);

select *
  from j1_tbl full join j2_tbl using (i)
  order by i, k, t;

select * from tenk1 a, tenk1 b
where exists(select * from tenk1 c
             where b.twothousand = c.twothousand and b.fivethous <> c.fivethous)
      and a.tenthous = b.tenthous and a.tenthous < 5000;

SELECT onek.unique1, onek.stringu1 FROM onek
   WHERE onek.unique1 < 20
   ORDER BY unique1 using >;

-- The `TABLE name` explicit-table set-operation member (select.sql:140), now parsed
-- (parse-pg-table-command-and-empty-select): `TABLE int8_tbl` is a query primary that
-- composes as a `UNION ALL` operand alongside `VALUES` and `SELECT`.
VALUES (1,2), (3,4+4), (7,77.7)
UNION ALL
SELECT 2+2, 57
UNION ALL
TABLE int8_tbl;
