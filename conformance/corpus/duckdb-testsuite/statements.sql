PRAGMA enable_verification
CREATE SCHEMA test
CREATE TABLE test.tbl(col INTEGER)
INSERT INTO test.tbl VALUES (1), (2), (3)
SELECT test.tbl.col FROM test.tbl
CREATE SCHEMA t
CREATE TABLE t.t(t ROW(t INTEGER))
INSERT INTO t.t VALUES ({'t': 42})
SELECT t FROM t.t
SELECT t.t FROM t.t
SELECT t.t.t FROM t.t
SELECT t.t.t.t FROM t.t
SELECT #1 FROM range(1)
SELECT #1+#2 FROM range(1) tbl, range(1) tbl2
SELECT #1 FROM (SELECT * FROM range(1)) tbl
CREATE TABLE integers("COL1" INTEGER, "COL2" INTEGER)
CREATE TABLE tbl AS SELECT * FROM integers
SELECT name FROM pragma_table_info('tbl') ORDER BY name
DROP TABLE tbl
CREATE TABLE tbl AS SELECT COL1, COL2 FROM integers
CREATE TABLE tbl AS SELECT integers.COL1, integers.COL2 FROM integers
CREATE SCHEMA s1
CREATE TABLE s1.integers("COL1" INTEGER, "COL2" INTEGER)
CREATE TABLE tbl AS SELECT s1.integers.COL1, s1.integers.COL2 FROM s1.integers
CREATE TABLE s1.tbl(i INTEGER)
SELECT s1.tbl.i FROM s1.tbl
SELECT j : 42
select column_name from (describe SELECT j : 42)
SELECT "j" : 42
SELECT "hel lo" : 42
select column_name from (describe SELECT "hel lo" : 42)
SELECT j1 : 42, 42 AS j2, 42 j3
CREATE TABLE a (i INTEGER)
INSERT INTO a VALUES (42)
SELECT j : i FROM a
SELECT "j" : "i" FROM a
SELECT * FROM b : a
SELECT * FROM "b" : a
CREATE TABLE integers(i INTEGER)
SELECT * FROM integers
CREATE TABLE t (t TEXT)
INSERT INTO t VALUES ('foo'), ('bar'), ('baz')
CREATE SCHEMA s
create table s.a as select 'hello' as col1
create view s.b as select * from s.a
select s.b.col1 from s.b
select b.col1 from s.b
CREATE TABLE integers AS SELECT i FROM range(10) tbl(i)
CREATE TABLE integers2 AS SELECT i FROM range(10) tbl(i)
CREATE VIEW integers_empty AS SELECT * FROM integers WHERE rowid>100
CREATE VIEW integers2_empty AS SELECT * FROM integers WHERE rowid>100
CREATE VIEW empty_join AS SELECT * FROM integers JOIN integers2_empty USING (i)
SELECT COUNT(*) FROM integers_empty JOIN integers2 USING (i)
SELECT COUNT(*) FROM integers_empty JOIN integers2 ON (integers_empty.i>integers2.i)
SELECT COUNT(*) FROM integers_empty JOIN integers2 ON (integers_empty.i<>integers2.i)
SELECT COUNT(*) FROM integers_empty JOIN integers2 ON (integers_empty.i<>integers2.i OR integers_empty.i+1<>integers2.i)
SELECT * FROM integers_empty JOIN integers2 USING (i)
SELECT COUNT(*) FROM integers_empty LEFT JOIN integers2 USING (i)
SELECT * FROM integers_empty LEFT JOIN integers2 USING (i)
SET default_null_order='nulls_first'
CREATE TABLE test (a INTEGER, b INTEGER)
INSERT INTO test VALUES (4, 1), (2, 2)
CREATE TABLE test2 (b INTEGER, c INTEGER)
INSERT INTO test2 VALUES (1, 2), (3, 0)
SELECT * FROM test JOIN test2 ON test.a+test2.c=test.b+test2.b
SELECT * FROM test LEFT JOIN test2 ON test.a+test2.c=test.b+test2.b ORDER BY 1
SELECT * FROM test RIGHT JOIN test2 ON test.a+test2.c=test.b+test2.b ORDER BY 1
SELECT * FROM test FULL OUTER JOIN test2 ON test.a+test2.c=test.b+test2.b ORDER BY 1
WITH lhs(i, j, k) AS (VALUES (100, 10, 1), (200, 20, 2) ), rhs(p, q, r) AS (VALUES (100, 10, 1), (200, 20, 2) ) SELECT lhs.*, rhs.* FROM lhs, rhs WHERE i <= p AND j <> q AND k IS DISTINCT FROM r
WITH lhs(i, j, k) AS (VALUES (100, 10, 1), (200, 20, 2) ), rhs(p, q, r) AS (VALUES (100, 10, 1), (200, 20, 2) ) SELECT lhs.*, rhs.* FROM lhs, rhs WHERE i <= p AND k >= r AND j <= q ORDER BY i
CREATE TABLE wide AS ( SELECT i, 10 * (i + 0) AS c0, 10 * (i + 1) AS c1, 10 * (i + 2) AS c2, 10 * (i + 3) AS c3, 10 * (i + 4) AS c4, 10 * (i + 5) AS c5, 10 * (i + 6) AS c6, 10 * (i + 7) AS c7, 10 * (i + 8) AS c8, 10 * (i + 9) AS c9 FROM range(1, 10) tbl(i) )
SELECT * FROM wide
CREATE TABLE limits AS ( SELECT 100 + (i * 17 % 100) AS z FROM range(1, 10) tbl(i) )
SELECT z FROM limits
SELECT i, z FROM wide, limits WHERE c0 < z AND c1 < z AND c2 < z AND c3 < z AND c4 < z AND c5 < z AND c6 < z AND c7 < z AND c8 < z AND c9 < z ORDER BY 1, 2
CREATE TABLE wide_nulls AS ( SELECT i, c0, c1, c2, CASE WHEN i % 7 = 0 THEN NULL ELSE c3 END AS c3, c4, c5, c6, c7, CASE WHEN i % 5 = 0 THEN NULL ELSE c8 END AS c8, c9 FROM wide )
SELECT * FROM wide_nulls
CREATE TABLE limits_nulls AS ( SELECT CASE WHEN z % 9 = 0 THEN NULL ELSE z END AS z FROM limits )
SELECT * FROM limits_nulls
SELECT i, z FROM wide_nulls, limits_nulls WHERE c0 < z AND c1 < z AND c2 < z AND c3 < z AND c4 < z AND c5 < z AND c6 < z AND c7 < z AND c8 < z AND c9 < z ORDER BY 1, 2
CREATE TABLE left_table (id INTEGER, val INTEGER)
INSERT INTO left_table VALUES (1, 10), (2, 20)
CREATE TABLE right_table (id INTEGER, category INTEGER)
INSERT INTO right_table VALUES (1, 100), (2, 200)
CREATE TABLE empty_table (id INTEGER, category INTEGER)
SELECT * FROM left_table l INNER JOIN right_table r ON TRUE
SELECT * FROM left_table l LEFT JOIN right_table r ON TRUE
SELECT * FROM left_table l RIGHT JOIN right_table r ON TRUE
SELECT * FROM left_table l FULL OUTER JOIN right_table r ON TRUE
SELECT * FROM left_table l SEMI JOIN right_table r ON TRUE
SELECT * FROM left_table l SEMI JOIN empty_table e ON TRUE
SELECT * FROM left_table l ANTI JOIN right_table r ON TRUE
INSERT INTO integers VALUES (1), (2), (3), (NULL)
SELECT * FROM (SELECT SUM(i) AS x FROM integers) a, (SELECT SUM(i) AS x FROM integers) b WHERE a.x=b.x
CREATE TABLE groups(i INTEGER, j INTEGER)
INSERT INTO groups VALUES (1, 1), (2, 1), (3, 2), (NULL, 2)
SELECT a.j,a.x,a.y,b.y FROM (SELECT j, MIN(i) AS y, SUM(i) AS x FROM groups GROUP BY j) a, (SELECT j, MIN(i) AS y, SUM(i) AS x FROM groups GROUP BY j) b WHERE a.j=b.j AND a.x=b.x ORDER BY a.j
CREATE VIEW list_int AS SELECT i, i%2 as i2, [i, i + 1, i + 2] as l3 FROM range(10) tbl(i)
select lhs.*, rhs.* from list_int lhs, list_int rhs where lhs.i2 = rhs.i2 and lhs.l3 <> rhs.l3 order by lhs.i, rhs.i
select lhs.*, rhs.* from list_int lhs, list_int rhs where lhs.i2 = rhs.i2 and lhs.l3 <= rhs.l3 order by lhs.i, rhs.i
select lhs.*, rhs.* from list_int lhs, list_int rhs where lhs.i2 = rhs.i2 and lhs.l3 < rhs.l3 order by lhs.i, rhs.i
select lhs.*, rhs.* from list_int lhs, list_int rhs where lhs.i2 = rhs.i2 and lhs.l3 >= rhs.l3 order by lhs.i, rhs.i
select lhs.*, rhs.* from list_int lhs, list_int rhs where lhs.i2 = rhs.i2 and lhs.l3 > rhs.l3 order by lhs.i, rhs.i
pragma verify_external
CREATE TABLE test1 (i INT, s1 VARCHAR, s2 VARCHAR)
INSERT INTO test1 VALUES (1, 'thisisareallylongstring', 'thisisareallylongstringtoo')
CREATE TABLE test2 (i INT, s1 VARCHAR, s2 VARCHAR)
INSERT INTO test2 VALUES (1, 'longstringsarecool', 'coolerthanshortstrings')
SELECT t1.i, t1.s1, t1.s2, t2.s1, t2.s2 FROM test1 t1, test2 t2 WHERE t1.i = t2.i
create or replace table xx as select w from (values ('a'),('b'),('c'),('d'),('e')) t(w)
select w from (from xx limit 4) CROSS JOIN (select 1 as f1) p WHERE w IN ( SELECT 'a' UNION SELECT 'b' UNION SELECT 'c' WHERE p.f1 = 1 UNION SELECT 'd' WHERE p.f1 = 1 )
CREATE TABLE t1 (a INTEGER, b INTEGER)
INSERT INTO t1 VALUES (1, 2)
CREATE TABLE t2 (a INTEGER, c INTEGER)
INSERT INTO t2 VALUES (1, 3), (2, 4)
SELECT * FROM t1 NATURAL JOIN t2
SELECT t1.a, t1.b, t2.c FROM t1 NATURAL JOIN t2
SELECT t1.a, t1.b, t2.c FROM t1 NATURAL JOIN t2 ORDER BY t2.a
CREATE TABLE t3 (a INTEGER, b INTEGER, c INTEGER)
INSERT INTO t3 VALUES (1, 2, 3)
SELECT * FROM t1 NATURAL JOIN t3
SELECT * FROM t3 NATURAL JOIN t2
SELECT * FROM t1 NATURAL JOIN t2 NATURAL JOIN t3
CREATE TABLE unit2( price INTEGER, amount_sold INTEGER, total_profit INTEGER GENERATED ALWAYS AS (price * amount_sold) VIRTUAL, also_total_profit INTEGER GENERATED ALWAYS AS (total_profit) VIRTUAL )
INSERT INTO unit2 SELECT i, 20 FROM range(1000) t(i)
SELECT * FROM unit2 JOIN (VALUES (2000)) t(total_profit) USING (total_profit)
SELECT * FROM unit2 JOIN (VALUES (2000)) t(total_profit) ON (t.total_profit = unit2.total_profit AND t.total_profit=unit2.also_total_profit)
CREATE TABLE integers AS SELECT CASE WHEN i%2=0 THEN NULL ELSE i END i FROM range(1000) t(i)
SELECT * FROM integers JOIN (SELECT MAX(i) AS max_i FROM integers) ON i=max_i
SELECT * FROM integers RIGHT JOIN (SELECT MAX(i) AS max_i FROM integers) ON i=max_i
SELECT COUNT(*), COUNT(max_i) IS NOT NULL FROM ( SELECT * FROM integers LEFT JOIN (SELECT MAX(i) AS max_i FROM integers) ON i=max_i )
SELECT COUNT(*), COUNT(max_i) IS NOT NULL FROM ( SELECT * FROM integers FULL OUTER JOIN (SELECT MAX(i) AS max_i FROM integers) ON i=max_i )
SELECT * FROM integers WHERE i=(SELECT MAX(i) FROM integers)
SELECT * FROM integers WHERE i IN (SELECT MAX(i) FROM integers)
SELECT * FROM integers WHERE i IN (997, 999)
SELECT COUNT(*), SUM(CASE WHEN in_result THEN 1 ELSE 0 END) FROM (SELECT i IN (SELECT MAX(i) FROM integers) AS in_result FROM integers)
CREATE TABLE bigtbl AS SELECT i%2 AS small_key, i%10 AS medium_key, i AS val FROM range(10000) t(i) ORDER BY small_key, medium_key
CREATE TABLE smalltbl AS SELECT i small_key FROM range(2) t(i)
CREATE TABLE mediumtbl AS SELECT i medium_key FROM range(10) t(i)
SELECT COUNT(*) FROM bigtbl JOIN smalltbl USING (small_key) JOIN mediumtbl USING (medium_key)
SELECT COUNT(*) FROM bigtbl JOIN (FROM smalltbl WHERE small_key=1) smalltbl USING (small_key) JOIN mediumtbl USING (medium_key)
SELECT COUNT(*) FROM bigtbl JOIN smalltbl USING (small_key) JOIN (FROM mediumtbl WHERE medium_key=1) mediumtbl USING (medium_key)
SELECT COUNT(*) FROM bigtbl JOIN (FROM smalltbl WHERE small_key=1) smalltbl USING (small_key) JOIN (FROM mediumtbl WHERE medium_key=1) mediumtbl USING (medium_key)
SELECT COUNT(*) FROM bigtbl JOIN (FROM smalltbl WHERE small_key=1) smalltbl USING (small_key) JOIN (FROM mediumtbl WHERE medium_key=1) mediumtbl ON (mediumtbl.medium_key=smalltbl.small_key)
CREATE TABLE t1(c0 INT4, c1 VARCHAR)
CREATE TABLE t2(c0 TINYINT, PRIMARY KEY(c0))
INSERT INTO t1(c0) VALUES (14161972)
INSERT INTO t1(c0, c1) VALUES (-1.438515327E9, 4.43806148E8)
SELECT * FROM t1 INNER JOIN t2 ON t1.c0 = t2.c0
CREATE TABLE t(t_k0 TINYINT)
INSERT INTO t VALUES (-128), (127)
CREATE TABLE u(u_k0 TINYINT)
INSERT INTO u VALUES (-128), (127)
SELECT t_k0, u_k0 FROM t, u WHERE t_k0 = u_k0
DROP TABLE t
DROP TABLE u
CREATE TABLE t(t_k0 SMALLINT)
INSERT INTO t VALUES (-32768), (32767)
CREATE TABLE u(u_k0 SMALLINT)
INSERT INTO u VALUES (-32768), (32767)
CREATE TABLE t(t_k0 INTEGER)
CREATE TABLE smalltable AS SELECT 1::INTEGER a
CREATE TABLE bigtable AS SELECT a::INTEGER a FROM generate_series(0, 10000, 1) tbl(a), generate_series(0, 9, 1) tbl2(b)
SELECT COUNT(*) FROM bigtable JOIN smalltable USING (a)
SELECT COUNT(*) FROM bigtable JOIN smalltable USING (a) JOIN smalltable t3 USING (a)
SELECT COUNT(*) FROM bigtable JOIN smalltable USING (a) JOIN smalltable t3 USING (a) JOIN smalltable t4 USING (a)
SELECT * FROM bigtable JOIN smalltable USING (a)
create table t1(i integer)
create table t2(j integer)
create table t3(k integer)
create table t4(l integer)
insert into t1 values (1)
insert into t2 values (1)
insert into t3 values (2), (3)
insert into t4 values (2), (3)
select * from t1 join t2 on (i=j), t3 join t4 on (k=l) order by 1, 2, 3, 4
select * from t1 join t2 on (i=j), t3 join t4 on (i+k=j+l)
select * from t1 join t2 on (i=j), lateral (select * from t3 join t4 on (i+k=j+l)) t(x)
CREATE TABLE test (id INTEGER, l VARCHAR[])
INSERT INTO test SELECT i, case when (i/1000)%2=0 then ARRAY[1::VARCHAR, 1::VARCHAR, 1::VARCHAR] else ARRAY[2::VARCHAR, 2::VARCHAR] end FROM generate_series(0, 1999, 1) tbl(i)
SELECT * FROM test AS t1 LEFT JOIN test AS t2 ON t1.id=t2.id WHERE t1.l!=t2.l or t1.id!=t2.id
INSERT INTO t1(c0) VALUES (-18), (NULL)
INSERT INTO t2(c0) VALUES (NULL)
SELECT * FROM t1 INNER JOIN t2 ON ((t1.c0 NOT BETWEEN t2.c0 AND t2.c0) IS NULL)
INSERT INTO test VALUES (11, 1), (12, 2), (13, 3)
CREATE TABLE test2 (a INTEGER, c INTEGER)
INSERT INTO test2 VALUES (11, 1), (12, 1), (13, 4)
SELECT test.a, b, c FROM test, test2 WHERE test.a = test2.a AND test.b <> test2.c ORDER BY test.a
SELECT test.a, b, c FROM test, test2 WHERE test.a = test2.a AND test.b < test2.c ORDER BY test.a
SELECT test.a, b, c FROM test, test2 WHERE test.a = test2.a AND test.b <= test2.c ORDER BY test.a
SELECT test.a, b, c FROM test, test2 WHERE test.a = test2.a AND test.b > test2.c ORDER BY test.a
SELECT test.a, b, c FROM test, test2 WHERE test.a = test2.a AND test.b >= test2.c ORDER BY test.a
DROP TABLE test
DROP TABLE test2
CREATE TABLE test (a INTEGER, b INTEGER, str VARCHAR)
INSERT INTO test VALUES (11, 1, 'a'), (12, 2, 'b'), (13, 3, 'c')
CREATE TABLE left_table (id INTEGER, val INTEGER, amount INTEGER)
INSERT INTO left_table VALUES (1, 1, 50), (2, 1, 75), (3, 2, 60), (4, 2, 90), (5, 3, 100)
CREATE TABLE right_table (id INTEGER, category INTEGER, budget INTEGER)
INSERT INTO right_table VALUES (1, 1, 1000), (2, 2, 2000), (3, 1, 1500)
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND l.val > 1 AND r.category = 1
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND r.category = 1
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND l.val > 1
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND true
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND false
SELECT * FROM left_table l INNER JOIN right_table r ON l.id = r.id AND l.amount + r.budget > 1100
INSERT INTO test2 VALUES (1, 10), (1, 20), (2, 30)
SELECT a, test.b, c FROM test, test2 WHERE test.b = test2.b ORDER BY c
SELECT a, test.b, c FROM test, test2 WHERE test.b=test2.b AND test.a-1=test2.c
SELECT a, (SELECT test.a), c FROM test, test2 WHERE test.b = test2.b ORDER BY c
SELECT a, test.b, c FROM test INNER JOIN test2 ON test.b = test2.b ORDER BY c
SELECT a, test.b, c FROM test INNER JOIN test2 ON test2.b = test.b ORDER BY c
SELECT a, test.b, c FROM test INNER JOIN test2 ON test2.b = test.b and test.b = 2
SELECT a, test.b, c FROM test INNER JOIN test2 ON test2.b = test.b and 2 = 2 ORDER BY c
SELECT a, test.b, c FROM test INNER JOIN test2 ON test.b = 2 ORDER BY c
SELECT a, test.b, c FROM test INNER JOIN test2 ON NULL = 2
SELECT * FROM (VALUES (1)) tbl(i) JOIN (VALUES (1)) tbl2(j) ON (i=j)
SELECT * FROM (VALUES (1), (2)) tbl(i) JOIN (VALUES (1), (2)) tbl2(j) ON (i=j) WHERE i+j=2
pragma verify_parallelism
CREATE TABLE test2 AS SELECT * FROM repeat(1, 10*1024) t1(b), (SELECT 10) t2(c)
SELECT COUNT(*) FROM test2
SELECT COUNT(*) FROM test INNER JOIN test2 ON test.b=test2.b
CREATE TABLE tbl (col0 INTEGER, col1 INTEGER)
INSERT INTO tbl VALUES (1, 0), (1, 1)
SELECT x.col1, y.col1 FROM tbl x JOIN tbl y ON x.col0 = y.col0 AND (x.col1 IS DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl x JOIN tbl y ON x.col0 = y.col0 AND x.col1 != y.col1 ORDER BY x.col1
CREATE TABLE tbl_s (col0 STRUCT(x INTEGER), col1 STRUCT(x INTEGER))
INSERT INTO tbl_s VALUES ({x: 1}, {x: 0}), ({x: 1}, {x: 1})
SELECT x.col1, y.col1 FROM tbl_s x JOIN tbl_s y ON x.col0 = y.col0 AND (x.col1 IS DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_s x JOIN tbl_s y ON x.col0 = y.col0 AND x.col1 != y.col1 ORDER BY x.col1
CREATE TABLE tbl_l (col0 INTEGER[], col1 INTEGER[])
INSERT INTO tbl_l VALUES ([1], [0]), ([1], [1])
SELECT x.col1, y.col1 FROM tbl_l x JOIN tbl_l y ON x.col0 = y.col0 AND (x.col1 IS DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_l x JOIN tbl_l y ON x.col0 = y.col0 AND x.col1 != y.col1 ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl x JOIN tbl y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl x JOIN tbl y ON x.col0 = y.col0 AND x.col1 = y.col1 ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_s x JOIN tbl_s y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_s x JOIN tbl_s y ON x.col0 = y.col0 AND x.col1 = y.col1 ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_l x JOIN tbl_l y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_l x JOIN tbl_l y ON x.col0 = y.col0 AND x.col1 = y.col1 ORDER BY x.col1
WITH abc AS ( SELECT * FROM ( VALUES (1, 'x'), (1, 'x'), (1, '0'), (1, '0') ) AS tbl(col0, col1) ) SELECT x.col0 AS c1, x.col1 AS c2, y.col0 AS c3, y.col1 AS c4 FROM abc x JOIN abc y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY c1, c2, c3, c4
CREATE TABLE tbl_null (col0 INTEGER, col1 INTEGER)
INSERT INTO tbl_null VALUES (1, 0), (1, 1), (1, NULL), (NULL, 1), (0, NULL), (NULL, 0), (NULL, NULL)
SELECT x.col1, y.col1 FROM tbl_null x JOIN tbl_null y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY x.col1, y.col1
SELECT x.col1, y.col1 FROM tbl_null x JOIN tbl_null y ON x.col0 = y.col0 AND x.col1 = y.col1 ORDER BY x.col1
SELECT x.col1, y.col1 FROM tbl_null x JOIN tbl_null y ON x.col0 = y.col0 AND (x.col1 IS NOT DISTINCT FROM y.col1) ORDER BY x.col1
create table a AS SELECT i FROM range(1, 2001, 1) t1(i)
select count(*) from a, (SELECT 2000 AS j) b where i < j
select count(*) from a, (SELECT 2000 AS j) b where i <= j
select count(*) from a, (SELECT 1 AS j) b where i > j
select count(*) from a, (SELECT 1 AS j) b where i >= j
SELECT test.b, test2.b FROM test, test2 WHERE test.b<test2.b
SELECT test.b, test2.b FROM test, test2 WHERE test.b <= test2.b ORDER BY 1,2
SELECT test.a, test.b, test2.b, test2.c FROM test, test2 WHERE test.a>test2.c AND test.b <= test2.b
INSERT INTO test VALUES (11, NULL), (NULL, 1)
INSERT INTO test2 VALUES (1, NULL), (NULL, 10)
PRAGMA debug_force_external=true
CREATE TABLE issue4419 (x INT, y VARCHAR)
INSERT INTO issue4419 VALUES (1, 'sssssssssssssssssueufuheuooefef')
INSERT INTO issue4419 VALUES (2, 'sssssssssssssssssueufuheuooefesffff')
INSERT INTO issue4419 VALUES (2, 'sssssssssssssssssueufuheuooefesffffsssssssieiffih')
SELECT * FROM issue4419 t1 INNER JOIN issue4419 t2 ON t1.x < t2.x
SELECT test.b, test2.b FROM test, test2 WHERE test.b <> test2.b ORDER BY test.b, test2.b
SELECT test.b, test2.b FROM test, test2 WHERE test.b <> test2.b AND test.b <> 1 AND test2.b <> 2 ORDER BY test.b, test2.b
INSERT INTO test VALUES (NULL, NULL)
INSERT INTO test2 VALUES (NULL, NULL)
create table a (i integer)
create table b (j integer)
insert into b values ('31904'),('31904'),('31904'),('31904'),('35709'),('31904'),('31904'),('35709'),('31904'),('31904'),('31904'),('31904')
select count(*) from a,b where i <> j
CREATE TABLE test (b INTEGER)
INSERT INTO test VALUES (1), (2)
CREATE TABLE test2 AS SELECT * FROM repeat(1, 10*1024) t1(b)
SELECT COUNT(*) FROM test INNER JOIN test2 ON test.b<>test2.b
CREATE TABLE t2 (b INTEGER, c INTEGER)
INSERT INTO t2 VALUES (2, 3)
CREATE TABLE t3 (c INTEGER, d INTEGER)
INSERT INTO t3 VALUES (3, 4)
SELECT * FROM t1 JOIN t2 USING (b) JOIN t3 USING (c) ORDER BY 1, 2, 3, 4
DROP TABLE t1
DROP TABLE t2
DROP TABLE t3
CREATE TABLE t1 (a INTEGER, b INTEGER, c INTEGER)
INSERT INTO t1 VALUES (1, 2, 2)
CREATE TABLE t2 (b INTEGER, c INTEGER, d INTEGER, e INTEGER)
INSERT INTO t2 VALUES (2, 2, 3, 4)
INSERT INTO t1 VALUES (1,2,3)
CREATE TABLE t2 (a INTEGER, b INTEGER, c INTEGER)
INSERT INTO t2 VALUES (1,2,3), (2,2,4), (1,3,4)
SELECT * FROM t1 JOIN t2 USING(a) JOIN t2 t2b USING (a) ORDER BY 1, 2, 3, 4, 5, 6, 7
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(a) ORDER BY t2.b
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(b) ORDER BY t2.c
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(a,b)
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(a,b,c)
SELECT a+1 FROM t1 JOIN t2 USING(a) ORDER BY a
SELECT * FROM t1 JOIN t2 USING(a,b)
select * from (select NULL::varchar as b) sq1, (select 'asdf' as b) sq2 where sq1.b = sq2.b
select * from (select 42 as a, NULL::varchar as b) sq1, (select 42 as a, 'asdf' as b) sq2 where sq1.b <> sq2.b
select * from (select 42 as a, NULL::varchar as b) sq1, (select 42 as a, 'asdf' as b) sq2 where sq1.a=sq2.a and sq1.b <> sq2.b
select * from (select 42 as a, 'asdf' as b) sq2, (select 42 as a, NULL::varchar as b) sq1 where sq1.b <> sq2.b
select * from (select 42 as a, 'asdf' as b) sq2, (select 42 as a, NULL::varchar as b) sq1 where sq1.a=sq2.a and sq1.b <> sq2.b
CREATE TABLE bigtable AS SELECT a::INTEGER a FROM generate_series(0, 9999, 1) tbl(a), generate_series(0, 9, 1) tbl2(b)
SELECT COUNT(*) FROM bigtable FULL OUTER JOIN smalltable USING (a)
SELECT COUNT(*) FROM bigtable RIGHT OUTER JOIN smalltable USING (a)
CREATE TABLE integers(i INTEGER, j INTEGER)
INSERT INTO integers VALUES (1, 1), (3, 3)
CREATE TABLE integers2(k INTEGER, l INTEGER)
INSERT INTO integers2 VALUES (1, 10), (2, 20)
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k UNION ALL SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY i
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k UNION SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY i
SELECT DISTINCT * FROM ( SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k UNION ALL SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k) tbl ORDER BY i
CREATE VIEW v1 AS SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k UNION ALL SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k
SELECT * FROM v1 FULL OUTER JOIN v1 v2 USING (i, j) ORDER BY 1, 2, 3, 4, 5, 6
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY i
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k AND integers.j > integers2.l ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN (SELECT k, l::VARCHAR AS l FROM integers2) integers2 ON integers.i=integers2.k ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN (SELECT * FROM integers2 WHERE 1=0) integers2 ON integers.i=integers2.k ORDER BY 1, 2, 3, 4
INSERT INTO integers VALUES (1, 1)
INSERT INTO integers2 VALUES (1, 10)
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i+integers2.k+9<>integers.j+integers2.l ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN (SELECT * FROM integers2 WHERE 1=0) integers2 ON integers.i+integers2.k+9<>integers.j+integers2.l ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i<>integers2.k ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN (SELECT * FROM integers2 WHERE 1=0) integers2 ON integers.i<>integers2.k ORDER BY 1, 2, 3, 4
CREATE TABLE test (x INT, y INT)
INSERT INTO test VALUES (1, 1), (2, 2), (3, 3)
SELECT * FROM (SELECT a2.x FROM (SELECT x FROM test WHERE x > 3) AS a1 FULL OUTER JOIN (SELECT x FROM test WHERE x = 1) AS a2 ON a1.x = a2.x) AS a3 FULL OUTER JOIN (SELECT 1 AS x) AS a4 ON a3.x = a4.x
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i<integers2.k ORDER BY 1, 2, 3, 4
SELECT i, j, k, l FROM integers FULL OUTER JOIN (SELECT * FROM integers2 WHERE 1=0) integers2 ON integers.i<integers2.k ORDER BY 1, 2, 3, 4
create table test_str(k varchar)
create table test_str_del(pk varchar)
create table test_int(k bigint)
create table test_int_del(pk bigint)
insert into test_str values('abc'), ('def')
insert into test_int values(1), (2)
select l.* from test_str l anti join test_str_del r on l.k = r.pk
select l.* from test_int l anti join test_int_del r on l.k = r.pk
insert into test_int VALUES (NULL)
select l.* from test_int l anti join test_int_del r on l.k is not distinct from r.pk
insert into test_int_del VALUES (NULL)
CREATE TABLE left_table (a INTEGER, b INTEGER, c INTEGER)
INSERT INTO left_table VALUES(42, 1, 1), (43, 1, 1)
CREATE TABLE right_table (a INTEGER, b INTEGER)
INSERT INTO right_table VALUES(42, 1)
SELECT * FROM left_table ANTI JOIN right_table ON left_table.a = right_table.a
SELECT * FROM left_table ANTI JOIN right_table ON left_table.a = right_table.a WHERE a > 5
SELECT * FROM left_table ANTI JOIN right_table ON ([left_table.a, left_table.b] = [right_table.a, right_table.b])
SELECT * FROM left_table ANTI JOIN (SELECT a as foo from right_table where b = 1) buzz ON left_table.a = buzz.foo
INSERT INTO left_table VALUES (43, 1, 5), (43, 1, 5), (43, 1, 5), (43, 1, 5)
SELECT * FROM left_table ANTI JOIN right_table ON (left_table.a = right_table.a)
CREATE TABLE other (a INTEGER, b INTEGER)
INSERT INTO other VALUES (42, 1), (43, 1)
WITH cte1 AS MATERIALIZED ( SELECT 'col1' AS col1, UNNEST( [TIMESTAMPTZ '2025-01-01 00:00:11+00', TIMESTAMPTZ '2025-01-01 00:00:41+00'] ) AS col2 ), cte2 AS ( SELECT 'col1' AS col1, TIMESTAMPTZ '2025-01-01 00:00:40+00' AS col2, 'col3' AS col3, ) SELECT * FROM cte1 ANTI JOIN cte2 ON cte1.col1 = cte2.col1 AND cte1.col2 > cte2.col2
WITH cte1 AS MATERIALIZED ( SELECT 'col1' AS col1, UNNEST( [TIMESTAMPTZ '2025-01-01 00:00:11+00', TIMESTAMPTZ '2025-01-01 00:00:41+00'] ) AS col2 ), cte2 AS ( SELECT 'col1' AS col1, TIMESTAMPTZ '2025-01-01 00:00:40+00' AS col2, 'col3' AS col3, ) SELECT * FROM cte1 SEMI JOIN cte2 ON cte1.col1 = cte2.col1 AND cte1.col2 > cte2.col2
create table t1 as select * from values (1, 2), (2, 4), (3, 8), (6, 25), (1, 25) t(a, b)
create table t2 as select * from values (4), (5) t(b)
select * from t1 semi join t2 on t1.a < t2.b and t1.b > t2.b order by all
select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
Explain select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
select * from t1 semi join t2 on t1.a < t2.b or t1.b < t2.b order by all
select * from t1 semi join t2 on (t1.a < t2.b and t1.b < t2.b) or (t1.a < t2.b and t1.b = 4) order by all
select * from t1 semi join t2 on (t1.a < t2.b or t1.b < t2.b) and (t1.a = 1 or t1.b = 4) order by all
CREATE TABLE flattened ("start" varchar, "end" varchar)
insert into flattened values ('2023-03-15T00:00:00Z', '2023-03-20T00:00:00Z')
create table input_table as select * from VALUES ('1', '2023-03-14T00:00:00Z', 2), ('2', '2023-03-15T00:00:00Z', 4), ('3', '2023-03-16T00:00:00Z', 7), ('4', '2023-03-17T00:00:00Z', 3), ('5', '2023-03-18T00:00:00Z', 2), ('6', '2023-03-19T23:59:59Z', 4), ('7', '2023-03-20T00:00:00Z', 7), ('8', '2023-03-21T00:00:00Z', 3) t(user_id, timestamp, value)
SELECT * FROM input_table ANTI JOIN flattened ON input_table."timestamp" >= flattened.start AND input_table."timestamp" < flattened.end
INSERT INTO left_table VALUES (42, 1, 1), (43, 1, 1), (42, 1, 1), (41, 1, 1), (41, 2, 2), (41, 7, 7)
INSERT INTO right_table select 41, range as b from range(375)
EXPLAIN ANALYZE SELECT * FROM left_table ANTI JOIN right_table ON left_table.a = right_table.a
explain analyze SELECT * FROM left_table ANTI JOIN right_table ON left_table.a = right_table.a WHERE a > 5
explain analyze SELECT * FROM left_table ANTI JOIN right_table ON ([left_table.a, left_table.b] = [right_table.a, right_table.b])
explain analyze SELECT * FROM left_table ANTI JOIN (SELECT a as foo from right_table where b > 5) buzz ON left_table.a = buzz.foo
EXPLAIN ANALYZE SELECT * FROM left_table ANTI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table ON left_table.a = joined_right_table.a
DELETE FROM left_table where c=5
EXPLAIN ANALYZE SELECT * FROM left_table ANTI JOIN right_table USING (a)
explain analyze SELECT * FROM left_table NATURAL ANTI JOIN right_table
EXPLAIN ANALYZE SELECT * FROM left_table NATURAL ANTI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table
EXPLAIN ANALYZE SELECT * FROM left_table ANTI JOIN right_table ON (left_table.a <> right_table.a) ORDER BY a, c
INSERT INTO left_table VALUES (41, 1, 1), (42, 1, 1), (42, 1, 1), (43, 1, 1), (45, 2, 2), (46, 7, 7)
EXPLAIN ANALYZE SELECT * FROM left_table SEMI JOIN right_table ON left_table.a = right_table.a
explain analyze SELECT * FROM left_table SEMI JOIN right_table ON left_table.a = right_table.a WHERE a > 5
explain analyze SELECT * FROM left_table SEMI JOIN right_table ON ([left_table.a, left_table.b] = [right_table.a, right_table.b])
explain analyze SELECT * FROM left_table SEMI JOIN (SELECT a as foo from right_table where b > 1) buzz ON left_table.a = buzz.foo
EXPLAIN ANALYZE SELECT * FROM left_table SEMI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table ON left_table.a = joined_right_table.a
EXPLAIN ANALYZE SELECT * FROM left_table SEMI JOIN right_table USING (a)
explain analyze SELECT * FROM left_table NATURAL SEMI JOIN right_table
EXPLAIN ANALYZE SELECT * FROM left_table NATURAL SEMI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table
EXPLAIN ANALYZE SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a <> right_table.a) ORDER BY a, c
EXPLAIN ANALYZE SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a > right_table.a)
explain analyze SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a + right_table.a = 85 OR left_table.a + right_table.b = 84) order by left_table.a, left_table.c
SELECT * FROM left_table SEMI JOIN right_table ON left_table.a = right_table.a
SELECT * FROM left_table SEMI JOIN right_table ON left_table.a = right_table.a WHERE a > 5
SELECT * FROM left_table SEMI JOIN right_table ON ([left_table.a, left_table.b] = [right_table.a, right_table.b])
SELECT * FROM left_table SEMI JOIN (SELECT a as foo from right_table where b = 1) buzz ON left_table.a = buzz.foo
INSERT INTO left_table VALUES (42, 1, 5), (42, 1, 5), (42, 1, 5), (42, 1, 5)
SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a = right_table.a)
SELECT * FROM left_table SEMI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table ON left_table.a = joined_right_table.a
SELECT * FROM left_table SEMI JOIN right_table USING (a)
SELECT * FROM left_table NATURAL SEMI JOIN right_table
SELECT * FROM left_table NATURAL SEMI JOIN (select right_table.a FROM right_table JOIN other ON (other.a = right_table.a)) joined_right_table
SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a <> right_table.a) ORDER BY a, c
SELECT * FROM left_table SEMI JOIN right_table ON (left_table.a > right_table.a)
SELECT * FROM left_table l SEMI JOIN right_table r ON l.id = r.id AND l.val > 1 AND r.category = 1
SELECT * FROM left_table l SEMI JOIN right_table r ON l.id = r.id AND r.category = 1
SELECT * FROM left_table l SEMI JOIN right_table r ON l.id = r.id AND l.val > 1
SELECT * FROM left_table l SEMI JOIN empty_table e ON l.id = e.id
SELECT * FROM left_table l ANTI JOIN right_table r ON l.id = r.id AND l.val > 1 AND r.category = 1
SELECT * FROM left_table l ANTI JOIN right_table r ON l.id = r.id AND r.category = 1
SELECT * FROM left_table l ANTI JOIN right_table r ON l.id = r.id AND l.val = 1
SELECT * FROM left_table l ANTI JOIN empty_table e ON l.id = e.id
SELECT * FROM left_table l SEMI JOIN right_table r ON l.id = r.id AND l.amount + r.budget > 1100
SELECT * FROM left_table l ANTI JOIN right_table r ON l.id = r.id AND l.amount + r.budget > 1100
pragma enable_verification
CREATE TABLE t0(c0 VARCHAR)
CREATE TABLE t1(c1 VARCHAR)
INSERT INTO t1(c1) VALUES (NULL)
INSERT INTO t0(c0) VALUES (1)
select * FROM t1 WHERE NOT EXISTS (SELECT 1 FROM t0 WHERE null)
select * FROM t1 WHERE EXISTS (SELECT 1 FROM t0 WHERE ((t0.c0) != (t1.c1)))
select * FROM t1 WHERE NOT EXISTS (SELECT 1 FROM t0 WHERE ((t0.c0)!=(t1.c1)))
create table lineitem (l_orderkey int, l_suppkey int, l_partkey int)
insert into lineitem values (1,1,42),(1,2,43),(3,3,44),(4,5,45),(5,5,46),(6,5,47)
select * from lineitem l1 where exists ( select * from lineitem l2 where l2.l_orderkey = l1.l_orderkey and l2.l_suppkey <> l1.l_suppkey )
select c0, EXISTS (select * from t1 where c1 != c0) from t0
drop table if exists t1
drop table if exists t2
create table t1 (id string)
create table t2 (id string)
insert into t1 values (NULL)
insert into t2 values (1), (1)
select * from t1 left join t2 on t1.id = t2.id
select * from t1 left join t2 on t1.id > t2.id
select * from t1 left join t2 on t1.id <> t2.id
insert into t2 values (NULL), (NULL)
insert into t1 (id) values (1), (1), (NULL)
insert into t2 (id) values (1), (1), (1), (1), (1), (1)
set explain_output='optimized_only'
CREATE OR REPLACE TABLE big_table AS SELECT i.range AS col1, CAST(random() * 1000 AS INTEGER) AS col2 FROM range(100) i
CREATE OR REPLACE TABLE single_col_table AS SELECT i.range AS col1 FROM range(50) i
explain SELECT * FROM big_table c LEFT OUTER JOIN single_col_table hd ON hd.col1=c.col1 AND ( FALSE )
CREATE TABLE integers1 AS SELECT * FROM (VALUES (1), (2), (3)) tbl(i)
CREATE TABLE integers2 AS SELECT * FROM (VALUES (1, '1'), (2, '2'), (3, '3')) tbl(i, s)
CREATE TABLE integers3 AS SELECT * FROM (VALUES (1, '4'), (2, '5'), (3, '6')) tbl(i, s)
SELECT i1.i AS i1_i, i2.s, i3.i AS i3_i FROM integers1 i1 LEFT OUTER JOIN (integers2 i2 LEFT OUTER JOIN integers3 i3 ON i2.i = i3.i) on false
CREATE TABLE foo (ts TIMESTAMP)
CREATE TABLE bar (ts TIMESTAMP)
INSERT INTO foo VALUES ('2023-01-01 00:00:00')
INSERT INTO foo VALUES ('2023-01-01 00:00:01')
SELECT foo.ts foo, bar.ts bar FROM foo LEFT JOIN bar ON foo.ts = bar.ts
SELECT foo.ts foo, bar.ts bar FROM foo LEFT JOIN bar ON foo.ts < bar.ts
SELECT foo.ts foo, bar.ts bar FROM foo LEFT JOIN bar ON foo.ts > bar.ts
CREATE TABLE a(a1 VARCHAR)
CREATE TABLE b( b1 VARCHAR, b2 TIMESTAMP, b3 TIMESTAMP, b4 VARCHAR, b5 VARCHAR, b6 VARCHAR, b7 TIMESTAMP, b8 TIMESTAMP, b9 VARCHAR, b10 VARCHAR, b11 VARCHAR, b12 VARCHAR, b13 VARCHAR, b14 VARCHAR, )
INSERT INTO b VALUES (NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)
CREATE TABLE c( c1 VARCHAR, )
CREATE TABLE d( d1 VARCHAR, d2 VARCHAR, )
SELECT * FROM a LEFT JOIN b ON b.b14 = a.a1 LEFT JOIN c ON b.b13 = c.c1 LEFT JOIN d ON b.b12 = d.d1 WHERE d.d2 IN ('')
select * from range(1) tbl(i) left join range(2) tbl2(j) on (i=j) where j+random()<0
INSERT INTO left_table VALUES (1, 1, 50), (2, 1, 75), (3, 2, 60), (4, 2, 90), (5, 99, 100)
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND l.val > 1 AND r.category = 1
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND r.category = 1
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND l.val > 1
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND true
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND false
SELECT * FROM left_table l LEFT JOIN right_table r ON l.id = r.id AND l.amount + r.budget > 1100
WITH t AS ( SELECT 1 AS r, [{n:1}, {n:2}] AS s UNION SELECT 2 AS r, [{n:3}, {n:4}] AS s ) SELECT r, s1.s.n FROM t LEFT JOIN UNNEST(s) AS s1(s) ON TRUE ORDER BY 1, 2
WITH t AS ( SELECT 1 AS r, ARRAY[1, 2, 3] AS a UNION SELECT 2 AS r, ARRAY[4] AS a UNION SELECT 4 AS r, ARRAY[] AS a ) SELECT r, a.value FROM t LEFT JOIN UNNEST(a) AS a(value) ON TRUE ORDER BY 1, 2
WITH t AS ( SELECT 1 AS r, ARRAY[1, 2, 3] AS a UNION SELECT 2 AS r, ARRAY[4] AS a UNION SELECT 4 AS r, ARRAY[]::INTEGER[] AS a ) SELECT r, a.value FROM t LEFT JOIN UNNEST(a) AS a(value) ON TRUE AND a.value IS NULL ORDER BY 1, 2
WITH t AS ( SELECT 1 AS r, ARRAY[1, 2, 3] AS a UNION SELECT 2 AS r, ARRAY[4] AS a UNION SELECT 4 AS r, ARRAY[] AS a ) SELECT r, a.value FROM t LEFT JOIN UNNEST(a) AS a(value) ON (1 = 1) AND TRUE AND list_contains([2, 3], 2) ORDER BY 1, 2
INSERT INTO integers VALUES (1, 2), (2, 3), (3, 4)
SELECT * FROM integers LEFT OUTER JOIN integers2 ON TRUE AND integers.i=integers2.k AND TRUE ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON TRUE AND integers.i=integers2.k AND FALSE ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON TRUE ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY i
SELECT * FROM integers2 RIGHT OUTER JOIN integers ON integers.i=integers2.k ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON integers.i=integers2.k WHERE k IS NOT NULL ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON integers.i=integers2.k AND integers2.k IS NOT NULL ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON i=1 ORDER BY i, k
SELECT * FROM integers LEFT OUTER JOIN integers2 ON 1=1 ORDER BY i, k
SELECT * FROM integers LEFT OUTER JOIN (SELECT * FROM integers2 WHERE 1<>1) tbl2 ON 1=2 ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON 1=2 ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON NULL<>NULL ORDER BY i
SELECT * FROM integers LEFT OUTER JOIN integers2 ON l=20 ORDER BY i, k
SELECT * FROM integers LEFT OUTER JOIN integers2 ON l>0 ORDER BY i, k
SELECT * FROM integers LEFT OUTER JOIN integers2 ON i=1 OR l=20 ORDER BY i, k
CREATE TABLE t0(c0 DATE, PRIMARY KEY(c0))
CREATE TABLE t1(c0 VARCHAR DEFAULT(DATE '1969-12-10'), c1 DOUBLE DEFAULT(0.16338108651823613))
INSERT INTO t1(c1) VALUES (true)
INSERT INTO t1(c0) VALUES (TIMESTAMP '1969-12-13 07:02:08')
INSERT INTO t0(c0) VALUES (DATE '1970-01-01'), (TIMESTAMP '1969-12-13 17:49:43')
SELECT MAX('a') FROM t0 JOIN t1 ON ((t0.c0)<=(((NULL)-(t1.rowid))::DATE))
SELECT MAX('a') FROM t0 RIGHT JOIN t1 ON ((t0.c0)<=(((NULL)-(t1.rowid))::DATE))
INSERT INTO left_table VALUES (1, 1), (2, 1), (3, 2)
INSERT INTO right_table VALUES (1, 1), (2, 2), (3, 1), (4, 1)
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND l.val > 1 AND r.category = 1
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND r.category = 1
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND l.val > 1
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND true
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND false
SELECT * FROM left_table l RIGHT JOIN right_table r ON l.id = r.id AND l.val + r.category > 2
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON integers.i=integers2.k ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON integers.i=integers2.k WHERE k IS NOT NULL ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON integers.i=integers2.k AND integers2.k IS NOT NULL ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON i=1 ORDER BY i, k
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON 1=1 ORDER BY i, k
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON 1=2 ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON NULL<>NULL ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON l=20 ORDER BY i, k
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON l>0 ORDER BY i, k
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON i=1 OR l=20 ORDER BY i, k
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON i=4 OR l=17 ORDER BY i
SELECT integers.*, integers2.* FROM integers2 RIGHT OUTER JOIN integers ON i+l=21 ORDER BY i
PRAGMA wal_autocheckpoint='1TB'
PRAGMA disable_checkpoint_on_shutdown
CREATE TABLE t0(c0 INT, c1 INT)
CREATE TABLE t1(c0 INT)
INSERT INTO t1 VALUES (0)
CREATE UNIQUE INDEX t0i0 ON t0(c0, c1)
INSERT INTO t0 VALUES (1,0), (-1,NULL)
UPDATE t0 SET c1 = NULL WHERE c0 = 1
SELECT * FROM t1 POSITIONAL JOIN t0 WHERE (t1.c0 > t0.c1) IS NULL
CREATE TABLE two (a INTEGER, b INTEGER)
INSERT INTO two VALUES (11, 1), (12, 2)
CREATE TABLE three AS SELECT * FROM (VALUES (11, 1), (12, 2), (13, 3) ) tbl(a, b)
CREATE TABLE threek AS SELECT * FROM generate_series(0, 3001) tbl(id)
SELECT * FROM two t1 POSITIONAL JOIN two t2
SELECT * FROM threek t1 POSITIONAL JOIN threek t2 WHERE t1.id <> t2.id
SELECT * FROM two t1 POSITIONAL JOIN three t2
SELECT * FROM three t1 POSITIONAL JOIN two t2
SELECT COUNT(a), COUNT(id) FROM three POSITIONAL JOIN threek
SELECT COUNT(id), COUNT(a) FROM threek POSITIONAL JOIN three
SELECT * FROM (SELECT * FROM two WHERE a % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM two WHERE a % 2 = 1) t2
SELECT * FROM (SELECT * FROM threek WHERE id % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM threek WHERE id % 2 = 1) t2 WHERE t1.id + 1 <> t2.id
select lefttable.x, righttable.y from (select 1 as x) lefttable asof left join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x limit 0) lefttable asof left join (select 1 as x, 1 as y) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x) lefttable asof join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
CREATE TABLE events0 (begin DOUBLE, value INTEGER)
INSERT INTO events0 VALUES (1, 0), (3, 1), (6, 2), (8, 3)
create table prices("when" timestamp, symbol int, price int)
insert into prices values ('2020-01-01 00:00:00', 1, 42)
create table trades("when" timestamp, symbol int)
insert into trades values ('2020-01-01 00:00:03', 1)
SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol = p.symbol AND t.when >= p.when
EXPLAIN SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol IS NOT DISTINCT FROM p.symbol AND t.when >= p.when
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON 1 = 1 AND p.ts >= e.begin ORDER BY p.ts ASC
WITH samples AS ( SELECT col0 AS starts, col1 AS ends FROM (VALUES (5, 9), (10, 13), (14, 20), (21, 23) ) ) SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM samples AS s1 ASOF JOIN samples as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
PRAGMA asof_loop_join_threshold=0
SELECT p.ts, e.value FROM range(0,10) p(ts) JOIN ( SELECT value, begin, LEAD(begin, 1, 'infinity'::DOUBLE) OVER (ORDER BY begin ASC) AS end FROM events0 ) e ON p.ts >= e.begin AND p.ts < e.end ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) LEFT JOIN ( SELECT value, begin, LEAD(begin, 1, 'infinity'::DOUBLE) OVER (ORDER BY begin ASC) AS end FROM events0 ) e ON p.ts >= e.begin AND p.ts < e.end ORDER BY p.ts ASC NULLS FIRST
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF LEFT JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC NULLS FIRST
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF LEFT JOIN events0 e USING (begin) ORDER BY p.begin ASC NULLS FIRST
INSERT INTO events0 VALUES (10, 4)
SELECT p.ts, e.value FROM range(0,10) p(ts) RIGHT JOIN ( SELECT value, begin, LEAD(begin, 1, 'infinity'::DOUBLE) OVER (ORDER BY begin ASC) AS end FROM events0 ) e ON p.ts >= e.begin AND p.ts < e.end ORDER BY p.ts ASC NULLS LAST
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF RIGHT JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC NULLS LAST
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF RIGHT JOIN events0 e USING (begin) ORDER BY p.begin ASC NULLS LAST
CREATE TABLE events (key INTEGER, begin DOUBLE, value INTEGER)
CREATE TABLE left_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, price DECIMAL)
INSERT INTO left_table VALUES (1, '2024-01-01 10:00:00', 'A', 150.00), (2, '2024-01-01 10:05:00', 'A', 151.00), (3, '2024-01-01 10:10:00', 'B', 380.00)
CREATE TABLE right_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, bid DECIMAL, active BOOLEAN)
INSERT INTO right_table VALUES (1, '2024-01-01 09:59:00', 'A', 149.50, true), (2, '2024-01-01 10:04:00', 'A', 150.50, false), (3, '2024-01-01 10:09:00', 'B', 379.00, true)
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150 AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND false
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
CREATE TABLE events0 AS SELECT '2023-03-21 13:00:00'::TIMESTAMP + INTERVAL (range) HOUR AS begin, range AS value FROM range(0, 4)
INSERT INTO events0 VALUES (NULL, -10), ('infinity', 9), ('-infinity', -1)
CREATE TABLE probe0 AS SELECT * FROM range('2023-03-21 12:00:00'::TIMESTAMP, '2023-03-21 22:00:00'::TIMESTAMP, INTERVAL 1 HOUR) p(begin)
INSERT INTO probe0 VALUES (NULL), ('infinity')
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
CREATE TABLE events0 (begin INTEGER, value INTEGER)
INSERT INTO events0 VALUES (NULL, -1), (1, 0), (3, 1), (6, 2), (8, 3), (999999, 9)
CREATE TABLE probe0 AS SELECT range::INTEGER AS begin FROM range(0,10)
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin >= e.begin ORDER BY ALL
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e USING (begin) ORDER BY ALL
CREATE TABLE tt1 (i INTEGER, j VARCHAR)
INSERT INTO tt1 VALUES (2, 'A'), (4, 'B'), (5, 'A')
CREATE TABLE tt2 (i INTEGER, j VARCHAR, k VARCHAR)
INSERT INTO tt2 VALUES (1, 'A', 'I'), (3, 'B', 'II')
explain SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON tt1.j = tt2.j AND tt1.i >= tt2.i ORDER BY tt1.i
SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON (tt1.j = tt2.j OR tt1.j = tt2.j) AND tt1.i >= tt2.i ORDER BY tt1.i
create table l (id integer, date timestamp, item varchar)
insert into l values (0, '2025-01-01', 'A')
create table r (id integer, date timestamp, item varchar, valuei double)
insert into r values (0, '2025-01-01', 'A', 8.0), (0, '2025-01-01', 'B', 12.0)
explain select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
PRAGMA debug_asof_iejoin=False
PRAGMA asof_loop_join_threshold = 0
CREATE TABLE prices_int("when" TIMESTAMP, symbol INTEGER, price INTEGER)
INSERT INTO prices_int VALUES ('2020-01-01 00:00:00', 1, 42), ('2020-01-01 00:00:00', 2, 55),
CREATE TABLE trades_int("when" timestamp, symbol INTEGER)
INSERT INTO trades_int VALUES ('2020-01-01 00:00:03', 1), ('2020-01-01 00:00:03', 3),
SELECT t.*, p.price FROM trades_int t ASOF JOIN prices_int p ON t.symbol = p.symbol AND t.when >= p.when
CREATE TABLE prices_varchar("when" TIMESTAMP, symbol VARCHAR, price INTEGER)
INSERT INTO prices_varchar VALUES ('2020-01-01 00:00:00', 'APPL', 42), ('2020-01-01 00:00:00', 'MEL', 55),
CREATE TABLE trades_varchar("when" timestamp, symbol VARCHAR)
INSERT INTO trades_varchar VALUES ('2020-01-01 00:00:03', 'APPL'), ('2020-01-01 00:00:03', 'VCT'),
SELECT t.*, p.price FROM trades_varchar t ASOF JOIN prices_varchar p ON t.symbol = p.symbol AND t.when >= p.when
CREATE OR REPLACE TABLE right_pushdown(time INTEGER, value FLOAT)
INSERT INTO right_pushdown VALUES (0, 0), (1, NULL),
CREATE TABLE issue13899(seq_no INT, amount DECIMAL(10,2))
INSERT INTO issue13899 VALUES (1,1.00), (2,null), (3,null), (4,null), (5,2.00), (6,null), (7,null), (8,3.00), (9,null), (10,null), (11,5.00)
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF LEFT JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
CREATE OR REPLACE TABLE issue12215 AS SELECT col0 AS starts, col1 AS ends FROM (VALUES (5, 9), (10, 13), (14, 20), (21, 23) )
SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM issue12215 AS s1 ASOF JOIN issue12215 as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF LEFT JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
select a.seq_no, a.amount, b.amount from issue13899 as a asof join issue13899 as b on a.seq_no>=b.seq_no and b.amount is not null ORDER BY 1
WITH t1 AS ( FROM (VALUES (1,2),(2,4)) t1(id, value) ), t2 AS ( FROM (VALUES (1,3)) t2(id, value) ) FROM t1 ASOF LEFT JOIN t2 ON t1.id <= t2.id ORDER BY 1
CREATE TABLE events (begin DOUBLE, value INTEGER)
INSERT INTO events VALUES (1, 0), (3, 1), (6, 2), (8, 3)
SELECT begin, value IN ( SELECT e1.value FROM ( SELECT * FROM events e1 WHERE e1.value = events.value) e1 ASOF JOIN range(1, 10) tbl(begin) USING (begin) ) FROM events ORDER BY ALL
INSERT INTO events0 VALUES (NULL, -1), ('infinity', 9)
CREATE TABLE asof_nulls ( time TIMESTAMP, value FLOAT )
INSERT INTO asof_nulls (time, value) VALUES ('2025-07-15 00:00:00', 42)
INSERT INTO asof_nulls (time, value) VALUES ('2025-07-15 01:00:00', null)
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF SEMI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF ANTI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT time_series.time, asof_nulls.value FROM (VALUES ('2025-07-15 02:00:00'::TIMESTAMP)) as time_series(time) ASOF LEFT JOIN asof_nulls ON asof_nulls.time <= time_series.time
SELECT * FROM left_table l ASOF SEMI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
CREATE TABLE events0 (begin VARCHAR, value INTEGER)
INSERT INTO events0 VALUES (NULL, -1), (1, 0), (3, 1), (6, 2), (8, 3), ('infinity', 9)
CREATE TABLE probe0 AS SELECT range::VARCHAR AS begin FROM range(0,10)
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
CREATE TABLE test(x INT)
SET merge_join_threshold=0
SET nested_loop_join_threshold=0
SELECT * FROM test AS a, test AS b WHERE (a.x BETWEEN b.x AND b.x)
INSERT INTO test(x) VALUES (1), (2), (3), (NULL), (NULL), (NULL)
CREATE TABLE all_null AS SELECT * FROM test
UPDATE all_null SET x=(NULL)
EXPLAIN SELECT * FROM all_null AS a, all_null AS b WHERE (a.x BETWEEN b.x AND b.x)
SELECT * FROM all_null AS a, all_null AS b WHERE (a.x BETWEEN b.x AND b.x)
EXPLAIN SELECT * FROM test AS a, all_null AS b WHERE (a.x BETWEEN b.x AND b.x)
SELECT * FROM test AS a, all_null AS b WHERE (a.x BETWEEN b.x AND b.x)
EXPLAIN SELECT * FROM all_null AS a, test AS b WHERE (a.x BETWEEN b.x AND b.x)
create table calendar as SELECT start_ts, start_ts + interval '12 hours' as end_ts, date_part('year',start_ts)::bigint * 100 + date_part('week',start_ts)::bigint as yyyyww FROM generate_series(TIMESTAMP '2023-01-01 06:00:00', TIMESTAMP '2023-06-01 00:00:00', INTERVAL '12 hours') tbl(start_ts)
create table snapshot_data as select TIMESTAMP '2023-03-01 08:00:00' as snapshot_ts, 1 as snapshot_value from generate_series(1,1000) t(i)
create table cal_last_13 as( select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) )
explain select count(*) from snapshot_data data join cal_last_13 cal on data.snapshot_ts >= cal.start_ts and data.snapshot_ts <= cal.end_ts
select count(*) from snapshot_data data join cal_last_13 cal on data.snapshot_ts >= cal.start_ts and data.snapshot_ts <= cal.end_ts
create or replace table cal_last_13 as ( select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) union all select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) )
create or replace table cal_last_13 as ( select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) union all select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) union all select * from calendar where yyyyww in (SELECT yyyyww FROM calendar) )
explain select count(*) from snapshot_data data join cal_last_13 cal on data.snapshot_ts >= cal.start_ts and data.snapshot_ts <= cal.end_ts join cal_last_13 cal2 on data.snapshot_ts >= cal2.start_ts and data.snapshot_ts <= cal2.end_ts
select count(*) from snapshot_data data join cal_last_13 cal on data.snapshot_ts >= cal.start_ts and data.snapshot_ts <= cal.end_ts join cal_last_13 cal2 on data.snapshot_ts >= cal2.start_ts and data.snapshot_ts <= cal2.end_ts
CREATE TABLE bigtbl AS FROM range(1000) t(i)
CREATE TABLE smalltbl AS SELECT i AS low, i + 1 AS high FROM range(100) t(i)
PRAGMA explain_output = 'PHYSICAL_ONLY'
EXPLAIN SELECT COUNT(*) FROM bigtbl JOIN smalltbl ON (bigtbl.i BETWEEN low AND high)
SET merge_join_threshold=1000
SET nested_loop_join_threshold=1000
SET prefer_range_joins=true
EXPLAIN SELECT COUNT(*) FROM bigtbl JOIN smalltbl ON (bigtbl.i BETWEEN low AND high AND bigtbl.i IS NOT DISTINCT FROM high - low)
SELECT COUNT(*) FROM bigtbl JOIN smalltbl ON (bigtbl.i BETWEEN low AND high AND bigtbl.i IS NOT DISTINCT FROM high - low)
PRAGMA explain_output = PHYSICAL_ONLY
create table calendar as SELECT * FROM range(DATE '2022-01-01', DATE '2024-02-01', INTERVAL '1' MONTH)
create table scd2 as select range as range_start, case when date_part('year', range) < 2023 then range + interval 4 month - interval 1 day end as range_end, n from calendar cross join generate_series(1, 85) as n
create table scd2_non_null as select range as range_start, case when date_part('year', range) < 2023 then range + interval 4 month - interval 1 day else '2099-01-01' end as range_end, n from calendar cross join generate_series(1, 85) as n
explain select range, count(*) as n from scd2_non_null inner join calendar on range between range_start and ifnull(range_end,'2099-01-01') group by range order by range
select range, count(*) as n from scd2_non_null inner join calendar on range between range_start and ifnull(range_end,'2099-01-01') group by range order by range
explain select range, count(*) as n from scd2 inner join calendar on range <= ifnull(range_end,'2099-01-01') and range_start <= range group by range order by range
select range, count(*) as n from scd2 inner join calendar on range <= ifnull(range_end,'2099-01-01') and range_start <= range group by range order by range
explain select range, count(*) as n from scd2 inner join calendar on range between range_start and ifnull(range_end,'2099-01-01') group by range order by range
select range, count(*) as n from scd2 inner join calendar on range between range_start and ifnull(range_end,'2099-01-01') group by range order by range
set merge_join_threshold=0
set prefer_range_joins=True
create or replace table states as select i // 100 as k, '2024-01-01'::TIMESTAMP + INTERVAL (i // 1) seconds as b, b + INTERVAL 1 second as e, from range(100_000) as tbl(i)
with joined as ( select lhs.k l, rhs.k r from states lhs inner join states rhs on lhs.b < rhs.e and rhs.b < lhs.e and lhs.k = rhs.k ) select count(*) from joined
explain with joined as ( select lhs.k l, rhs.k r from states lhs inner join states rhs on lhs.b < rhs.e and rhs.b < lhs.e and lhs.k = rhs.k ) select count(*) from joined
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ) SELECT lhs.id, rhs.id FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ), sub AS ( SELECT lhs.id AS lid, rhs.id AS rid FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL ) SELECT MIN(lid), MAX(rid) FROM sub
WITH RECURSIVE t AS ( SELECT 1 AS x, 0 AS begin, 4 AS end UNION ALL SELECT lhs.x + 1 AS x, GREATEST(lhs.begin, rhs.begin) as begin, LEAST(lhs.end, rhs.end) AS end FROM t lhs, t rhs WHERE lhs.begin + 1 < rhs.end - 1 AND rhs.begin + 1 < lhs.end - 1 AND lhs.x < 3 ) SELECT COUNT(*) FROM t
CREATE TABLE issue3486 AS SELECT generate_series as ts from generate_series(timestamp '2020-01-01', timestamp '2021-01-01', interval 1 day)
create table test_big as select range i, range + 100_000 j, 'hello' k from range (20_000)
create table test_small as select range i, range + 100_000 j, 'hello' k from range (0,20_000,10)
select * from test_small t1 join test_small t2 on (t1.i = t2.j) join test_small t3 on (true) join test_big t4 on (t3.i < t4.i and t3.j > t4.j)
CREATE TABLE east AS SELECT * FROM (VALUES ('r1', 100, 140, 12, 2), ('r2', 101, 100, 12, 8), ('r3', 103, 90, 5, 4) ) east(rid, id, dur, rev, cores)
CREATE TABLE west AS SELECT * FROM (VALUES ('s1', 404, 100, 6, 4), ('s2', 498, 140, 11, 2), ('s3', 676, 80, 10, 1), ('s4', 742, 90, 5, 4) ) west(rid, t_id, time, cost, cores)
EXPLAIN SELECT s1.rid, s2.rid FROM west s1, west s2 WHERE s1.time > s2.time ORDER BY 1, 2
SELECT s1.rid, s2.rid FROM west s1, west s2 WHERE s1.time > s2.time ORDER BY 1, 2
EXPLAIN SELECT s1.rid, s2.rid FROM west s1, west s2 WHERE s1.time > s2.time AND s1.cost < s2.cost ORDER BY 1, 2
SELECT s1.rid, s2.rid FROM west s1, west s2 WHERE s1.time > s2.time AND s1.cost < s2.cost ORDER BY 1, 2
EXPLAIN SELECT east.rid, west.rid FROM east, west WHERE east.dur < west.time AND east.rev > west.cost ORDER BY 1, 2
SELECT east.rid, west.rid FROM east, west WHERE east.dur < west.time AND east.rev > west.cost ORDER BY 1, 2
CREATE TABLE weststr AS ( SELECT rid, time::VARCHAR AS time, cost::VARCHAR as cost FROM west )
EXPLAIN SELECT s1.rid, s2.rid FROM weststr s1, weststr s2 WHERE s1.time > s2.time AND s1.cost < s2.cost ORDER BY 1, 2
SELECT s1.rid, s2.rid FROM weststr s1, weststr s2 WHERE s1.time > s2.time AND s1.cost < s2.cost ORDER BY 1, 2
EXPLAIN SELECT COUNT(*) FROM ( SELECT r.id, s.id FROM events r, events s WHERE r.start <= s.end AND r.end >= s.start AND r.id <> s.id ) q2
SELECT COUNT(*) FROM ( SELECT r.id, s.id FROM events r, events s WHERE r.start <= s.end AND r.end >= s.start AND r.id <> s.id ) q2
create table tt (x int, y int, z int)
insert into tt select nullif(r % 3, 0), nullif (r % 5, 0), r from range(10) tbl(r)
EXPLAIN select * from tt t1 left join tt t2 on t1.x < t2.x and t1.y < t2.y order by t1.x nulls first, t1.y nulls first, t1.z, t2.x, t2.y, t2.z
select * from tt t1 left join tt t2 on t1.x < t2.x and t1.y < t2.y order by t1.x nulls first, t1.y nulls first, t1.z, t2.x, t2.y, t2.z
pragma disable_optimizer
create table tt2 (x int)
insert into tt2 select * from range(10)
explain select t1.x, t1.y from ( select (case when x < 100 then null else 99 end) x, (case when x < 100 then 99 else 99 end) y from tt2 ) t1 left join tt2 t2 on t1.x < t2.x and t1.y < t2.x order by t1.x nulls first, t1.y nulls first
select t1.x, t1.y from ( select (case when x < 100 then null else 99 end) x, (case when x < 100 then 99 else 99 end) y from tt2 ) t1 left join tt2 t2 on t1.x < t2.x and t1.y < t2.x order by t1.x nulls first, t1.y nulls first
EXPLAIN SELECT t1.x, t2.x FROM 'test/sql/join/iejoin/overlap.left.csv' t1, 'test/sql/join/iejoin/overlap.right.csv' t2 WHERE t1.x < t2.x AND t1.y > t2.y
SELECT t1.x, t2.x FROM 'test/sql/join/iejoin/overlap.left.csv' t1, 'test/sql/join/iejoin/overlap.right.csv' t2 WHERE t1.x < t2.x AND t1.y > t2.y
SELECT t1.x, t2.x FROM 'test/sql/join/iejoin/overlap.left.csv' t1, 'test/sql/join/iejoin/overlap.right.csv' t2 WHERE t1.y > t2.y AND t1.x < t2.x
EXPLAIN SELECT t1.x, t2.x FROM 'test/sql/join/iejoin/overlap.left.csv' t1, 'test/sql/join/iejoin/overlap.right.csv' t2 WHERE t1.y > t2.y AND t1.x < t2.x
INSERT INTO test VALUES (11, 1), (12, 2)
SELECT * FROM test t1, test t2 ORDER BY 1, 2, 3, 4
SELECT COUNT(*) FROM test t1, range(2000) t2
SELECT COUNT(*) FROM range(2000) t1, test t2
CREATE TABLE students(id INTEGER, name VARCHAR, major VARCHAR, year INTEGER)
CREATE TABLE exams(sid INTEGER, course VARCHAR, curriculum VARCHAR, grade INTEGER, year INTEGER)
INSERT INTO students VALUES (1, 'Mark', 'CS', 2017)
INSERT INTO students VALUES (2, 'Dirk', 'CS', 2017)
INSERT INTO exams VALUES (1, 'Database Systems', 'CS', 10, 2015)
INSERT INTO exams VALUES (1, 'Graphics', 'CS', 9, 2016)
INSERT INTO exams VALUES (2, 'Database Systems', 'CS', 7, 2015)
INSERT INTO exams VALUES (2, 'Graphics', 'CS', 7, 2016)
SELECT s.name, e.course, e.grade FROM students s, exams e WHERE s.id=e.sid AND e.grade=(SELECT MAX(e2.grade) FROM exams e2 WHERE s.id=e2.sid) ORDER BY name, course
SELECT s.name, e.course, e.grade FROM students s, exams e WHERE s.id=e.sid AND (s.major = 'CS' OR s.major = 'Games Eng') AND e.grade <= (SELECT AVG(e2.grade) - 1 FROM exams e2 WHERE s.id=e2.sid OR (e2.curriculum=s.major AND s.year>=e2.year)) ORDER BY name, course
SELECT name, major FROM students s WHERE EXISTS(SELECT * FROM exams e WHERE e.sid=s.id AND grade=10) OR s.name='Dirk' ORDER BY name
SELECT (SELECT c0 OFFSET 1) FROM (VALUES(1)) c0
CREATE TABLE t0(c0 INT)
CREATE TABLE t2(c0 INT)
SELECT * FROM t2, t1, ( SELECT t2.c0 AS col_1, t1.c0 AS col_2) as subQuery0 INNER JOIN t0 ON ((subQuery0.col_2)) CROSS JOIN (SELECT t0.c0 AS col_1)
INSERT INTO t2(c0) VALUES (2)
INSERT INTO t1(c0) VALUES (1)
SELECT * FROM t2, t0 LEFT JOIN Lateral(SELECT t0.c0 AS col_0, t2.c0 AS col_1) as subQuery1 ON ((subQuery1.col_1)<(t0.c0))
drop table t0
drop table t1
CREATE TABLE t0(c0 DATE)
CREATE TABLE t1(c0 DATETIME, c1 DOUBLE)
SELECT * FROM t0, t1 CROSS JOIN (SELECT t0.c0 AS col_0 WHERE t1.c1) as subQuery0
drop table t2
CREATE TABLE my_logs ( featherEventId UUID, "duckInfo.gooseEmail" VARCHAR, "duckInfo.gooseSubject" VARCHAR )
CREATE TYPE MY_ENUM AS ENUM ( 'EnumField1', 'EnumField2', 'EnumField3', 'EnumField4', 'EnumField5', 'EnumField6', 'EnumField7', 'EnumField8' )
CREATE OR REPLACE MACRO swan_MY_ENUM (sa) AS ( WITH sa_parts AS ( SELECT STRING_SPLIT(sa, '@') AS emailParts ) SELECT 'EnumField2'::MY_ENUM FROM sa_parts )
CREATE TABLE lists(l INTEGER[])
INSERT INTO lists VALUES (ARRAY[1]), (ARRAY[2]), (ARRAY[3]), (NULL)
SELECT l, l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l=lists.l) i1 JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s]) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l=lists.l) i1 LEFT JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s]) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l=lists.l) i1 RIGHT JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s]) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM generate_series(1, 2, 1) tbl(s) LEFT JOIN (SELECT * FROM lists i1 WHERE i1.l=lists.l) i1 ON i1.l=ARRAY[tbl.s]) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM generate_series(1, 2, 1) tbl(s) RIGHT JOIN (SELECT * FROM lists i1 WHERE i1.l=lists.l) i1 ON i1.l=ARRAY[tbl.s]) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l IS NOT DISTINCT FROM lists.l) i1 JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s] OR (i1.l IS NULL AND tbl.s IS NULL)) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l IS NOT DISTINCT FROM lists.l) i1 LEFT JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s] OR (i1.l IS NULL AND tbl.s IS NULL)) FROM lists ORDER BY l NULLS LAST
SELECT l IN (SELECT i1.l FROM (SELECT * FROM lists i1 WHERE i1.l IS NOT DISTINCT FROM lists.l) i1 RIGHT JOIN generate_series(1, 2, 1) tbl(s) ON i1.l=ARRAY[tbl.s] OR (i1.l IS NULL AND tbl.s IS NULL)) FROM lists ORDER BY l NULLS LAST
CREATE TABLE nested_lists(l INTEGER[][])
INSERT INTO nested_lists VALUES (ARRAY[ARRAY[0], ARRAY[1]]), (ARRAY[ARRAY[2], ARRAY[NULL, 3]]), (ARRAY[ARRAY[4, 5], ARRAY[6, 7], ARRAY[], ARRAY[8]]), (NULL), (ARRAY[NULL]::INT[][])
SELECT UNNEST(l) FROM nested_lists
SELECT l, (SELECT SUM(a) FROM (SELECT UNNEST(b) AS a FROM (SELECT UNNEST(l) AS b))) FROM nested_lists ORDER BY l
SELECT i, EXISTS(SELECT i FROM integers WHERE i1.i>2) FROM integers i1 ORDER BY i
SELECT i, EXISTS(SELECT i FROM integers WHERE i=i1.i) FROM integers i1 ORDER BY i
SELECT i, EXISTS(SELECT i FROM integers WHERE i IS NULL OR i>i1.i*10) FROM integers i1 ORDER BY i
SELECT i, EXISTS(SELECT i FROM integers WHERE i1.i>i OR i1.i IS NULL) FROM integers i1 ORDER BY i
SELECT i FROM integers i1 WHERE EXISTS(SELECT i FROM integers WHERE i=i1.i) ORDER BY i
SELECT EXISTS(SELECT i FROM integers WHERE i>MIN(i1.i)) FROM integers i1
SELECT i, SUM(i) FROM integers i1 GROUP BY i HAVING EXISTS(SELECT i FROM integers WHERE i>MIN(i1.i)) ORDER BY i
SELECT EXISTS(SELECT i+MIN(i1.i) FROM integers WHERE i=3) FROM integers i1
SELECT EXISTS(SELECT i+MIN(i1.i) FROM integers WHERE i=5) FROM integers i1
SELECT EXISTS(SELECT i FROM integers WHERE i=i1.i) AS g, COUNT(*) FROM integers i1 GROUP BY g ORDER BY g
SELECT SUM(CASE WHEN EXISTS(SELECT i FROM integers WHERE i=i1.i) THEN 1 ELSE 0 END) FROM integers i1
SELECT (SELECT COVAR_POP(i1.i, i2.i) FROM integers i2) FROM integers i1 ORDER BY 1
CREATE TABLE t0(c2 INT)
INSERT INTO t0(c2) VALUES (NULL), (1)
SELECT t0.c2 FROM t0 WHERE NOT EXISTS ( SELECT 1 FROM ( SELECT t0.c2 AS col0 FROM t0 ) AS subQuery WHERE ((t0.c2) IS DISTINCT FROM (subQuery.col0)) )
SELECT t0.c2 FROM t0 WHERE NOT EXISTS ( SELECT 1 FROM ( SELECT t0.c2 AS col0 FROM t0 ) AS subQuery WHERE NOT ((t0.c2) IS DISTINCT FROM (subQuery.col0)) )
DROP TABLE t0
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
SELECT ( EXISTS( ( SELECT DISTINCT outer_alltypes."BIGINT", outer_alltypes."INT" FROM all_types inner_alltypes_1 WHERE inner_alltypes_1."BIGINT" GROUP BY NULL ) UNION BY NAME ( SELECT inner2."FLOAT" from all_types inner2 ) ) IS DISTINCT FROM outer_alltypes."struct" ) FROM all_types outer_alltypes GROUP BY ALL
create or replace table t1(c1 int64)
create or replace table t2(c1 int64)
select c1, not exists (select 1 from t2 where t1.c1 <= t2.c1) from t1
select c1 from t1 where not exists (select 1 from t2 where t1.c1 <= t2.c1)
select c1 from t1 anti join t2 on (t1.c1 <= t2.c1)
SELECT EXISTS(SELECT 1)
SELECT EXISTS(SELECT 1) FROM integers
SELECT EXISTS(SELECT * FROM integers)
SELECT EXISTS(SELECT * FROM integers WHERE i IS NULL)
SELECT * FROM integers WHERE EXISTS(SELECT 1) ORDER BY i
SELECT * FROM integers WHERE EXISTS(SELECT * FROM integers) ORDER BY i
SELECT * FROM integers WHERE NOT EXISTS(SELECT * FROM integers) ORDER BY i
SELECT * FROM integers WHERE EXISTS(SELECT NULL) ORDER BY i
SELECT EXISTS(SELECT * FROM integers WHERE i>10)
SELECT EXISTS(SELECT * FROM integers), EXISTS(SELECT * FROM integers)
SELECT EXISTS(SELECT * FROM integers) AND EXISTS(SELECT * FROM integers)
SELECT EXISTS(SELECT EXISTS(SELECT * FROM integers))
SELECT * FROM integers WHERE 1 IN (SELECT 1) ORDER BY i
SELECT * FROM integers WHERE 1 IN (SELECT * FROM integers) ORDER BY i
SELECT * FROM integers WHERE 1 IN (SELECT NULL::INTEGER) ORDER BY i
SELECT 1 IN (SELECT NULL::INTEGER) FROM integers
CREATE TABLE t0 (c0 INT)
CREATE TABLE t1 (c0 INT)
INSERT INTO t0 VALUES (1)
INSERT INTO t1 VALUES (1)
SELECT 1 = ANY(SELECT 1 FROM t1 JOIN (SELECT count(*) GROUP BY t0.c0) AS x(x) ON TRUE) FROM t0
CREATE TABLE t0 (c0 TIME,c1 DOUBLE PRECISION)
INSERT INTO t1 VALUES (1),(10),(7),(9),(NULL),(1),(7),(7),(0),(8),(0),(9),(NULL),(5),(3),(8),(0)
SET scalar_subquery_error_on_multiple_rows=false
INSERT INTO integers VALUES (1), (2), (3)
SELECT 2 > ANY(SELECT * FROM integers)
SELECT 1 > ANY(SELECT * FROM integers)
SELECT 4 > ALL(SELECT * FROM integers)
SELECT 1 > ALL(SELECT * FROM integers)
SELECT NULL > ANY(SELECT * FROM integers)
SELECT NULL > ALL(SELECT * FROM integers)
INSERT INTO integers VALUES (NULL)
SELECT i=ANY(SELECT i FROM integers WHERE i=i1.i) FROM integers i1 ORDER BY i
SELECT i>ALL(SELECT (i+i1.i-1)/2 FROM integers WHERE i IS NOT NULL) FROM integers i1 ORDER BY i
SELECT i=ALL(SELECT i FROM integers WHERE i<>i1.i) FROM integers i1 ORDER BY i
SELECT i FROM integers i1 WHERE i=ANY(SELECT i FROM integers WHERE i=i1.i) ORDER BY i
SELECT i FROM integers i1 WHERE i<>ANY(SELECT i FROM integers WHERE i=i1.i) ORDER BY i
SELECT i FROM integers i1 WHERE i=ANY(SELECT i FROM integers WHERE i<>i1.i) ORDER BY i
SELECT i FROM integers i1 WHERE i>ANY(SELECT i FROM integers WHERE i<>i1.i) ORDER BY i
SELECT i FROM integers i1 WHERE i>ALL(SELECT (i+i1.i-1)/2 FROM integers WHERE i IS NOT NULL) ORDER BY i
SELECT i=ALL(SELECT i FROM integers WHERE i=i1.i) FROM integers i1 ORDER BY i
SELECT i<>ALL(SELECT i FROM integers WHERE i=i1.i) FROM integers i1 ORDER BY i
SELECT i<>ANY(SELECT i FROM integers WHERE i=i1.i) FROM integers i1 ORDER BY i
SELECT i=ANY(SELECT i FROM integers WHERE i<>i1.i) FROM integers i1 ORDER BY i
CREATE TABLE test_data(a INTEGER, b INTEGER)
INSERT INTO test_data VALUES (1, 0), (0, 2), (2, 1), (NULL, 1)
SELECT (0, 0) < ANY(SELECT 1, 0)
SELECT (0, 0) < ANY(SELECT a, b FROM test_data)
SELECT (0, 1) < ANY(SELECT a, b FROM test_data)
SELECT (0, NULL) < ANY(SELECT 1, NULL)
SELECT (0, NULL) < ANY(SELECT a, b FROM test_data WHERE a IS NOT NULL)
SELECT (2, 0) > ANY(SELECT 1, 0)
SELECT (2, 0) > ANY(SELECT a, b FROM test_data WHERE a IS NOT NULL)
SELECT (1, 1) > ANY(SELECT 0, 2)
SELECT (1, 0) <= ANY(SELECT 1, 0)
SELECT (0, 5) <= ANY(SELECT 1, 0)
SELECT 1 = ANY(SELECT 1)
SELECT 1 = ANY(SELECT NULL)
SELECT 1 = ANY(SELECT 2)
SELECT NULL = ANY(SELECT 2)
SELECT 1 = ALL(SELECT 1)
SELECT 1 = ALL(SELECT NULL)
SELECT 1 = ALL(SELECT 2)
SELECT NULL = ALL(SELECT 2)
SELECT 1 IN (SELECT 1)
SELECT NULL IN (SELECT 1)
SELECT 1 IN (SELECT NULL)
SELECT 1 IN (SELECT 2)
SELECT 4 IN (SELECT * FROM integers)
SELECT 1 IN (SELECT * FROM integers)
SELECT 1 IN (SELECT * FROM integers) FROM integers
SELECT * FROM integers WHERE (4 IN (SELECT * FROM integers)) IS NULL ORDER BY 1
SELECT * FROM integers WHERE (i IN (SELECT * FROM integers)) IS NULL ORDER BY 1
SELECT 1 AS one WHERE 1 IN (SELECT 1)
CREATE TABLE test (id INTEGER, b INTEGER)
INSERT INTO test VALUES (1, 22)
INSERT INTO test VALUES (2, 21)
INSERT INTO test VALUES (3, 23)
SELECT * FROM test WHERE b IN (SELECT b FROM test WHERE b * id < 30) ORDER BY id, b
SELECT * FROM test WHERE b NOT IN (SELECT b FROM test WHERE b * id < 30) ORDER BY id, b
SELECT i FROM integers WHERE i >= ALL(SELECT i FROM integers)
SELECT i, i >= ALL(SELECT i FROM integers) FROM integers ORDER BY i
SELECT i FROM integers WHERE i >= ALL(SELECT i FROM integers WHERE i IS NOT NULL)
SELECT i, i >= ALL(SELECT i FROM integers WHERE i IS NOT NULL) FROM integers ORDER BY i
SELECT i FROM integers WHERE i > ALL(SELECT MIN(i) FROM integers)
SELECT i FROM integers WHERE i < ALL(SELECT MAX(i) FROM integers) ORDER BY 1
SELECT i FROM integers WHERE i <= ALL(SELECT i FROM integers)
SELECT i FROM integers WHERE i <= ALL(SELECT i FROM integers WHERE i IS NOT NULL)
SELECT i FROM integers WHERE i = ALL(SELECT i FROM integers WHERE i=1)
SELECT i FROM integers WHERE i <> ALL(SELECT i FROM integers WHERE i=1)
SELECT i FROM integers WHERE i = ALL(SELECT i FROM integers WHERE i IS NOT NULL)
SELECT i FROM integers WHERE i <> ALL(SELECT i FROM integers WHERE i IS NOT NULL)
SELECT i FROM integers WHERE i <= ANY(SELECT i FROM integers)
SELECT i FROM integers WHERE i > ANY(SELECT i FROM integers) ORDER BY 1
SELECT i, i > ANY(SELECT i FROM integers) FROM integers ORDER BY i
SELECT i, i > ANY(SELECT i FROM integers WHERE i IS NOT NULL) FROM integers ORDER BY i
SELECT i, NULL > ANY(SELECT i FROM integers) FROM integers ORDER BY i
SELECT i, NULL > ANY(SELECT i FROM integers WHERE i IS NOT NULL) FROM integers ORDER BY i
SELECT i FROM integers WHERE i = ANY(SELECT i FROM integers) order by i
SELECT i, i = ANY(SELECT i FROM integers WHERE i>2) FROM integers ORDER BY i
SELECT i, i = ANY(SELECT i FROM integers WHERE i>2 OR i IS NULL) FROM integers ORDER BY i
SELECT i, i <> ANY(SELECT i FROM integers WHERE i>2) FROM integers ORDER BY i
SELECT i, i <> ANY(SELECT i FROM integers WHERE i>2 OR i IS NULL) FROM integers ORDER BY i
SELECT i, i = ANY(SELECT i1.i FROM integers i1, integers i2, integers i3, integers i4, integers i5, integers i6 WHERE i1.i IS NOT NULL) FROM integers ORDER BY i
create table a(i integer)
insert into a values (42)
select * from (select i as j from a group by j) sq1 where j = 42
select * from (select i as j from a group by i) sq1 where j = 42
select * from (select 42) sq1 union all select * from (select 43) sq2
CREATE TABLE test (i INTEGER, j INTEGER)
INSERT INTO test VALUES (3, 4), (4, 5), (5, 6)
SELECT * FROM (SELECT i, j AS d FROM test ORDER BY i) AS b
SELECT b.d FROM (SELECT i * 2 + j AS d FROM test) AS b
SELECT a.i,a.j,b.r,b.j FROM (SELECT i, j FROM test) AS a INNER JOIN (SELECT i+1 AS r,j FROM test) AS b ON a.i=b.r ORDER BY 1
SELECT * FROM (SELECT i, j FROM test) AS a, (SELECT i+1 AS r,j FROM test) AS b, test WHERE a.i=b.r AND test.j=a.i ORDER BY 1
select sum(x) from (select i as x from test group by i) sq
select sum(x) from (select i+1 as x from test group by x) sq
SELECT a FROM (SELECT 42 a)
SELECT * FROM (SELECT 42 a), (SELECT 43 b)
SELECT * FROM (VALUES (42, 43))
SELECT * FROM (SELECT 42 a), (SELECT 43 b), (SELECT 44 c), (SELECT 45 d)
SELECT * FROM (FROM (SELECT 42 a), (SELECT 43 b)) JOIN (SELECT 44 c) ON (true) JOIN (SELECT 45 d) ON (true)
SELECT * FROM (SELECT unnamed_subquery.a FROM (SELECT 42 a)), (SELECT unnamed_subquery.b FROM (SELECT 43 b))
SELECT unnamed_subquery.a, unnamed_subquery2.b FROM (SELECT 42 a), (SELECT 43 b)
CREATE TABLE tbl(i INTEGER, arr INT[])
INSERT INTO tbl VALUES (1, ARRAY[1, 3, 7]), (2, ARRAY[8, NULL]), (3, ARRAY[3, NULL, 4]), (NULL, ARRAY[]::INT[])
SELECT * FROM tbl JOIN LATERAL (SELECT UNNEST(tbl.arr)) t(b) ON (i=b) ORDER BY i
SELECT * FROM tbl JOIN LATERAL (SELECT UNNEST(tbl.arr)) t(b) ON (i<>b) ORDER BY i, b
SELECT * FROM tbl JOIN LATERAL (SELECT UNNEST(tbl.arr)) t(b) ON (i<b) ORDER BY i, b
SELECT * FROM tbl JOIN LATERAL (SELECT UNNEST(tbl.arr)) t(b) ON (i>=b) ORDER BY i, b
SELECT * FROM tbl JOIN LATERAL (SELECT UNNEST(ARRAY[tbl.i * tbl.i])) t(b) ON (i>=b) ORDER BY i, b
SELECT * FROM tbl JOIN LATERAL (SELECT x FROM generate_series(0,5,1) t(x) WHERE x>i) t(b) ON (i>=b) ORDER BY i, b
SELECT * FROM tbl JOIN LATERAL (SELECT x FROM generate_series(0,5,1) t(x) WHERE x<i) t(b) ON (i>=b) ORDER BY i, b
from v1
select * from (select date '1992-01-01' as date), v1
SELECT * FROM (SELECT 42 AS c1) AS ref, (SELECT a + b + 1 FROM (SELECT 1) t1(a), (SELECT (SELECT (SELECT ref.c1 + 1)) + 1) t2(b) )
SELECT NULL FROM (SELECT 42 AS c1) AS ref, LATERAL (SELECT NULL FROM (SELECT NULL) AS r2, (SELECT (SELECT (SELECT ref.c1))) AS r3) AS r4
SELECT 1 FROM (SELECT 1) t1(c1), (SELECT TRUE IN (TRUE, t1.c1::VARCHAR LIKE 'a' ESCAPE NULL))
SELECT (SELECT t1.c1::VARCHAR LIKE 'a' ESCAPE NULL) FROM (SELECT 1) t1(c1)
select x, a, b from (values (1), (2)) t2(x), lateral (select count(*), count(a) from (select 1, 2 where 1 = x) t(a, b) group by grouping sets ((), (b), (a, b))) t3(a, b) order by all
select x, a from (values (1), (2)) t2(x), lateral (select sum(a) from (select 1, 2 where 1 = x) t(a, b) group by grouping sets ((), (b), (a, b))) t3(a) order by all
select * from (values (1), (2)) t2(x), lateral (select sum(a) from (select 42 a) where x=1) order by all
select * from (values (1), (2)) t2(x) left join (select sum(a) from (select 42 a) where x=1) on (1=1) order by all
SELECT * FROM integers i1, LATERAL (SELECT SUM(i + i1.i) FROM integers) t(sum) ORDER BY i
SELECT * FROM integers, (SELECT [i + 1]) t(k), (SELECT UNNEST(k)) t2(l) ORDER BY i
SELECT * FROM integers, LATERAL (SELECT [i + 1]) t(k), LATERAL (SELECT UNNEST(k)) t2(l) ORDER BY i
SELECT * FROM integers CROSS JOIN LATERAL (SELECT [i + 1]) t(k) CROSS JOIN LATERAL (SELECT UNNEST(k)) t2(l) ORDER BY i
SELECT * FROM integers, (SELECT integers) ORDER BY i
CREATE TABLE tbl ( x INTEGER, gen_x AS (x + 5) )
INSERT INTO tbl VALUES (1), (2), (3), (NULL)
SELECT * FROM tbl, (SELECT gen_x + 10) ORDER BY x NULLS LAST
CREATE FUNCTION my_func(x) AS (x + x)
CREATE TABLE tbl ( x INTEGER )
SELECT * FROM tbl, (SELECT my_func(x)) ORDER BY x NULLS LAST
SELECT total_seats FROM ( SELECT list(distinct {'key': gen_random_uuid(), 'val': 1 }) as l FROM range(0, 1600) ) as m, ( select sum(a.val) as value FROM ( SELECT UNNEST(l) a ) x ) as l(total_seats)
SELECT * FROM integers LEFT JOIN LATERAL (SELECT integers.i + 1) t(b) ON (i=b) ORDER BY i
SELECT * FROM integers LEFT JOIN LATERAL (SELECT integers.i) t(b) ON (i=b) ORDER BY i
SELECT * FROM integers LEFT JOIN LATERAL (SELECT * FROM integers WHERE i<>integers.i) t(b) ON (i=b) ORDER BY i
SELECT * FROM integers INNER JOIN LATERAL (SELECT integers.i WHERE integers.i IN (1, 3)) t(b) ON (i=b) ORDER BY i
SELECT * FROM integers LEFT JOIN LATERAL (SELECT integers.i WHERE integers.i IN (1, 3)) t(b) ON (i=b) ORDER BY i
FROM (SELECT 42) t(x), (SELECT x, row_number() OVER () QUALIFY NULL)
FROM (SELECT 42) t(x), (SELECT x * 2 QUALIFY row_number() OVER () < 10)
SELECT * FROM integers, LATERAL (VALUES (integers.i + 1)) t(k) ORDER BY i
SELECT * FROM integers a, integers b JOIN LATERAL (VALUES (a.i)) ss(x) ON (true) ORDER BY a.i, b.i
select s1, s2, sm from generate_series(1, 3) s1(s1), lateral (select s2, sum(s1 + s2) sm from generate_series(1, 3) s2(s2) group by s2) ss order by 1, 2
create table agg_data_1k as select g*10 AS g from generate_series(0, 999) g(g)
select * from (values (100), (300), (500)) as r(a), lateral ( select (g/2)::int as c1, array_agg(g::int) as c2, count(*) as c3 from agg_data_1k where g < r.a group by g/2) as s order by 1, 2, 4, 3
CREATE TABLE INT2_TBL(f1 int2)
INSERT INTO INT2_TBL(f1) VALUES ('0 '), (' 1234 '), (' -1234'), ('32767'), ('-32767')
CREATE TABLE INT4_TBL(f1 int4)
INSERT INTO INT4_TBL(f1) VALUES (' 0 '), ('123456 '), (' -123456'), ('2147483647'), ('-2147483647')
CREATE TABLE INT8_TBL(q1 int8, q2 int8)
INSERT INTO INT8_TBL VALUES (' 123 ',' 456'), ('123 ','4567890123456789'), ('4567890123456789','123'), (+4567890123456789,'4567890123456789'), ('+4567890123456789','-4567890123456789')
CREATE TABLE TEXT_TBL (f1 text)
INSERT INTO TEXT_TBL VALUES ('doh!'), ('hi de ho neighbor')
CREATE TABLE tenk1 ( unique1 int4, unique2 int4, two int4, four int4, ten int4, twenty int4, hundred int4, thousand int4, twothousand int4, fivethous int4, tenthous int4, odd int4, even int4, stringu1 varchar, stringu2 varchar, string4 varchar )
select (select MIN(val) from unnest((select a)) t(val)) from (select ARRAY[1, 2, 3, NULL]) t(a)
select (select MIN(val) from unnest((select (select a))) t(val)) from (select ARRAY[1, 2, 3, NULL]) t(a)
select * from (select array[1, 2, 3] a), unnest((select (select (select a))))
select (select MIN(val) from unnest(a) t(val)) from (select ARRAY[1, 2, 3, NULL]) t(a)
select * from (select 42) t(a), (select t.a + 1)
select * from (select 42) t(a) cross join lateral (select t.a + 1)
select * from (select 42 union all select 84) t(a), (select t.a + 1) ORDER BY ALL
select * from (select [42, 43, 44]) t(a), (select unnest(t.a)) order by all
select * from (select [42, 43, 44]) t(a), (select unnest(t.a)) t2(b) where b=43
select * from (select [42, 43, 44] union all select [45, NULL, 46]) t(a), (select unnest(t.a)) t2(b) order by all
select sum(b) from (select [42, 43, 44] union all select [45, NULL, 46]) t(a), (select unnest(t.a)) t2(b)
select a, sum(b) from (select [42, 43, 44] union all select [45, NULL, 46]) t(a), (select unnest(t.a)) t2(b) group by a order by a
create table t (i int)
insert into t values (1),(2),(3),(4),(4)
select array(select distinct i from t order by i desc) as a, array(select distinct i from t order by i desc) as b, array(select distinct i from t order by i desc) as c
select array(select unnest(l) AS i order by i desc nulls last) as a from (values ([NULL, 1, 2, 3, 4]), ([5, 6, NULL, 7, 8]), ([]), ([10, 11, 12])) t(l)
select array(select unnest(l) AS i order by i desc nulls first) as a from (values ([NULL, 1, 2, 3, 4]), ([5, 6, NULL, 7, 8]), ([]), ([10, 11, 12])) t(l)
SELECT ARRAY(SELECT i FROM t ORDER BY rowid DESC)
SELECT ARRAY(SELECT i FROM t ORDER BY t.rowid)
SELECT ARRAY (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 ORDER by 1) AS new_array
select array(select distinct i from t order by t.i desc) as a
select array(select distinct i from t union all select distinct i from t order by t.i desc) as a
SELECT ARRAY (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 ORDER by 1 DESC) AS new_array
select array(select * from unnest(['a', 'b']) as _t(u) order by if(u='a',100, 1)) as out
SELECT i, ARRAY( SELECT 42 ) top FROM integers i1 ORDER BY i
SELECT i, ARRAY( SELECT i FROM integers WHERE i1.i=i ) top FROM integers i1 ORDER BY i
SELECT i, ARRAY( SELECT i FROM integers WHERE i>i1.i ORDER BY i ASC NULLS FIRST ) top FROM integers i1 ORDER BY i
SELECT i, ARRAY( SELECT i1.i FROM integers i1, integers i2, integers i3, integers i4 WHERE i1.i=integers.i LIMIT 3 ) top FROM integers ORDER BY i
SELECT i, ARRAY( SELECT i1.i FROM integers i1, integers i2, integers i3, integers i4 WHERE i1.i=integers.i LIMIT 3 OFFSET 3 ) top FROM integers ORDER BY i
SELECT i, ARRAY( SELECT i1.i FROM integers i1, integers i2, integers i3, integers i4 WHERE i1.i=integers.i LIMIT 3 OFFSET 62 ) top FROM integers ORDER BY i
SELECT 1 FROM (select 4) v1(vc0) WHERE (3) NOT IN (COALESCE((SELECT 1 WHERE FALSE), v1.vc0))
CALL dbgen(sf=0)
CREATE TABLE Product(DaysToManufacture int, StandardCost int)
INSERT INTO Product VALUES (0, 5.0885), (1, 223.88), (2, 359.1082), (4, 949.4105)
SET pivot_filter_threshold=0
SET pivot_filter_threshold TO DEFAULT
RESET SESSION pivot_filter_threshold
CREATE TABLE table1(x INTEGER, y INTEGER)
INSERT INTO table1 VALUES (NULL, 2), (1, NULL)
CREATE TABLE table2(i INTEGER)
INSERT INTO table2 VALUES (1), (2), (3)
SELECT (x, y) IN (SELECT i, i + 1 FROM table2) from table1
SELECT (SELECT max((SELECT subq_0.c0 AS c1))) FROM (SELECT NULL AS c0) AS subq_0
SELECT (SELECT max(42) OVER (PARTITION BY (SELECT subq_0.c0 AS c1)) AS c6) FROM (SELECT NULL AS c0) AS subq_0
SELECT (SELECT max((SELECT subq_0.c0 AS c1)) OVER () AS c6) FROM (SELECT NULL AS c0) AS subq_0
SELECT i, ( SELECT * FROM integers WHERE i>i1.i ORDER BY i ASC NULLS FIRST LIMIT 1 ) top FROM integers i1 ORDER BY i
SELECT i, ( SELECT * FROM integers WHERE i>i1.i ORDER BY i DESC NULLS FIRST LIMIT 1 ) top FROM integers i1 ORDER BY i
SELECT i, ARRAY( SELECT * FROM integers WHERE i>i1.i ORDER BY i ) top FROM integers i1 ORDER BY i
SELECT (1, 2) IN (SELECT i, i + 1 FROM integers)
SELECT (date '1992-01-02', 2) IN (SELECT date '1992-01-01' + interval (i) days, i + 1 FROM integers)
SELECT (1, 2) IN (SELECT (i, i + 1) FROM integers)
SELECT row(1) IN (SELECT i FROM integers)
SELECT ROW(1, 2) IN (SELECT i, i + 1 FROM integers)
SELECT ROW(1, 2) IN (SELECT i, i + 2 FROM integers)
SELECT ROW(1, 2) IN (SELECT i, i + 2 FROM integers WHERE i IS NOT NULL)
select 1 where (1,2) in (select 1,2)
select 1 where (1,2) not in (select 1,2)
SELECT i, (SELECT s1.i FROM (SELECT * FROM integers WHERE i=i1.i) s1) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT s1.i FROM (SELECT i FROM integers WHERE i=i1.i) s1 INNER JOIN (SELECT i FROM integers WHERE i=4-i1.i) s2 ON s1.i>s2.i) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT s1.i FROM integers s1, integers s2 WHERE s1.i=s2.i AND s1.i=4-i1.i) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT s1.i FROM integers s1 INNER JOIN integers s2 ON s1.i=s2.i AND s1.i=4-i1.i) AS j FROM integers i1 ORDER BY i
SELECT * FROM integers s1 INNER JOIN integers s2 ON (SELECT 2*SUM(i)*s1.i FROM integers)=(SELECT SUM(i)*s2.i FROM integers) ORDER BY s1.i
SELECT * FROM integers s1 INNER JOIN integers s2 ON (SELECT s1.i=s2.i) ORDER BY s1.i
SELECT * FROM integers s1 INNER JOIN integers s2 ON (SELECT s1.i=i FROM integers WHERE s2.i=i) ORDER BY s1.i
SELECT * FROM integers s1 LEFT OUTER JOIN integers s2 ON (SELECT 2*SUM(i)*s1.i FROM integers)=(SELECT SUM(i)*s2.i FROM integers) ORDER BY s1.i
SELECT * FROM integers s1 LEFT OUTER JOIN integers s2 ON s1.i=s2.i AND (SELECT CASE WHEN s2.i>2 THEN TRUE ELSE FALSE END) ORDER BY s1.i
SELECT i, (SELECT i FROM integers WHERE i=i1.i UNION SELECT i FROM integers WHERE i=i1.i) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT i FROM integers WHERE i IS NOT NULL EXCEPT SELECT i FROM integers WHERE i<>i1.i) AS j FROM integers i1 WHERE i IS NOT NULL ORDER BY i
SELECT i, (SELECT i FROM integers WHERE i=i1.i INTERSECT SELECT i FROM integers WHERE i=i1.i) AS j FROM integers i1 ORDER BY i
CREATE TABLE tbl(a TINYINT, b SMALLINT, c INTEGER, d BIGINT, e VARCHAR, f DATE, g TIMESTAMP)
INSERT INTO tbl VALUES (1, 2, 3, 4, '5', DATE '1992-01-01', TIMESTAMP '1992-01-01 00:00:00')
SELECT EXISTS(SELECT t1.b+t1.c) FROM tbl t1
SELECT t1.c+(SELECT t1.b FROM tbl t2 WHERE EXISTS(SELECT t1.b+t2.a)) FROM tbl t1
SELECT 1 FROM tbl t1 JOIN tbl t2 ON (t1.d=t2.d) WHERE EXISTS(SELECT t1.c FROM tbl t3 WHERE t1.d+t3.c<100 AND EXISTS(SELECT t2.f < DATE '2000-01-01'))
SELECT EXISTS(SELECT 1 WHERE (t1.c>100 OR 1) AND t1.d<100) FROM tbl t1
SELECT EXISTS(SELECT t1.c,t1.d WHERE t1.d<100) FROM tbl t1
SELECT * FROM tbl t1 LEFT JOIN tbl t2 ON (SELECT t2.a)<100
SELECT * FROM tbl t1 LEFT JOIN tbl t2 ON (SELECT t2.a)>100
SELECT i, (SELECT COUNT(*) FROM (SELECT i1.i FROM integers GROUP BY GROUPING SETS(i1.i)) tbl) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM (SELECT i1.i FROM integers GROUP BY GROUPING SETS((i1.i), (), (i1.i), (i1.i, i1.i))) tbl) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM (SELECT i1.i FROM integers GROUP BY ROLLUP (i1.i, i1.i, i1.i, i1.i)) tbl) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM (SELECT i1.i FROM integers GROUP BY CUBE (i1.i, i1.i, i1.i, i1.i)) tbl) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT MIN(i) FROM integers GROUP BY GROUPING SETS(i1.i, i) HAVING i1.i=i) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(x) FROM (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT i1.i) t(x)) FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(x) FROM (SELECT i1.i UNION ALL SELECT 2 UNION ALL SELECT 1) t(x)) FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(x) FROM (SELECT 2 UNION ALL SELECT i1.i UNION ALL SELECT 1) t(x)) FROM integers i1 ORDER BY i
SELECT COUNT(DISTINCT (SELECT concat(gen_random_uuid()::VARCHAR, r::VARCHAR)) ) as total_seats FROM (SELECT 1 FROM generate_series(1, 100, 1)) AS t(r)
SELECT i, (SELECT 42+i1.i) AS j FROM integers i1 ORDER BY i
SELECT i FROM integers i1 ORDER BY (SELECT 100-i1.i)
SET scalar_subquery_error_on_multiple_rows=true
SELECT i, (SELECT 42+i1.i FROM integers LIMIT 1) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT 42+i1.i FROM integers LIMIT 0) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT i FROM integers WHERE 1=0 AND i1.i=i) AS j FROM integers i1 ORDER BY i
SELECT i, EXISTS(SELECT i FROM integers WHERE 1=0 AND i1.i=i) AS j FROM integers i1 ORDER BY i
SELECT i, i=ANY(SELECT i FROM integers WHERE 1=0 AND i1.i=i) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT i+i1.i FROM integers ORDER BY ALL LIMIT 1 OFFSET 1) AS j FROM integers i1 ORDER BY i
select (select val + i from generate_series(1, 2, 1) t(i) offset 1) from (select 42 val) t
select i, (select i1.i + i + i from generate_series(1, 100, 1) t(i) ORDER BY i DESC OFFSET 99) from integers i1 order by i
SELECT i, (SELECT i+i1.i FROM integers ORDER BY i NULLS LAST LIMIT 1) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT 42+i1.i AS j) SELECT j FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i FROM integers i1 ORDER BY (WITH i2 AS (SELECT 100-i1.i as j) SELECT j FROM i2)
SELECT i, (WITH i2 AS (SELECT 42+i1.i AS j FROM integers) SELECT j FROM i2 LIMIT 1) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT 42+i1.i AS j FROM integers) SELECT j FROM i2 LIMIT 0) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT i FROM integers WHERE 1=0 AND i1.i=i) SELECT i FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, EXISTS(WITH i2 AS (SELECT i FROM integers WHERE 1=0 AND i1.i=i) SELECT i FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, i=ANY(WITH i2 AS (SELECT i FROM integers WHERE 1=0 AND i1.i=i) SELECT i FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT i+i1.i FROM integers ORDER BY ALL LIMIT 1 OFFSET 1) SELECT * FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT i+i1.i FROM integers ORDER BY 1 NULLS LAST LIMIT 1 OFFSET 1) SELECT * FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT 42 WHERE i1.i>2) SELECT * FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT 42 WHERE i1.i IS NULL) SELECT * FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT i+i1.i FROM integers WHERE i=1) SELECT * FROM i2) AS j FROM integers i1 ORDER BY i
CREATE TABLE test AS FROM VALUES (1, 22), (1, 21), (2, 22) v(id, b)
SELECT * FROM test WHERE b=(SELECT MIN(b) FROM test AS a WHERE a.id=test.id)
SELECT * FROM test WHERE b=(SELECT MIN(b) FROM test AS a WHERE a.id=test.id AND a.id < test.b)
SELECT i, (SELECT SUM((SELECT i + 1)) OVER ()) FROM integers ORDER BY i
SELECT i, (SELECT SUM((SELECT i + 1)) OVER () WHERE i>=2) FROM integers ORDER BY i
SELECT i, (SELECT SUM((SELECT SUM(i))) OVER ()) FROM integers GROUP BY i ORDER BY i
SELECT i, (SELECT SUM(win) FROM (SELECT SUM((SELECT i1.i + integers.i)) OVER () AS win FROM integers i1) t) FROM integers ORDER BY i
SELECT i, (SELECT i FROM integers i2 WHERE i=(SELECT SUM(i) FROM integers i2 WHERE i2.i>i1.i)) FROM integers i1 ORDER BY 1
SELECT i, (SELECT SUM(i) IS NULL FROM integers i2 WHERE i2.i>i1.i) FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i) FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(i) FROM integers i2 WHERE i2.i>i1.i OR i2.i IS NULL) FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i OR i2.i IS NULL) FROM integers i1 ORDER BY i
SELECT i, (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i OR (i1.i IS NULL AND i2.i IS NULL)) FROM integers i1 ORDER BY i
SELECT i FROM integers i1 WHERE (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i)=0 ORDER BY i
SELECT i, (SELECT i FROM integers i2 WHERE i-2=(SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i)) FROM integers i1 ORDER BY 1
SELECT i, (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i GROUP BY i1.i) FROM integers i1 ORDER BY i
SELECT i, (SELECT CASE WHEN (SELECT COUNT(*) FROM integers i2 WHERE i2.i>i1.i)=0 THEN 1 ELSE 0 END) FROM integers i1 ORDER BY i
CREATE TABLE integers(id INTEGER, i INTEGER)
INSERT INTO integers VALUES (1, 1), (2, 2), (3, 3), (4, NULL)
DELETE FROM integers i1 WHERE i>(SELECT MAX(i) FROM integers WHERE i1.i<>i)
SELECT id, i FROM integers ORDER BY id
DELETE FROM integers i1 WHERE i=(SELECT MAX(i) FROM integers)
CREATE TABLE tbl_ProductSales (ColID int, Product_Category varchar(64), Product_Name varchar(64), TotalSales int)
CREATE TABLE another_T (col1 INT, col2 INT, col3 INT, col4 INT, col5 INT, col6 INT, col7 INT, col8 INT)
INSERT INTO tbl_ProductSales VALUES (1,'Game','Mobo Game',200),(2,'Game','PKO Game',400),(3,'Fashion','Shirt',500),(4,'Fashion','Shorts',100)
INSERT INTO another_T VALUES (1,2,3,4,5,6,7,8), (11,22,33,44,55,66,77,88), (111,222,333,444,555,666,777,888), (1111,2222,3333,4444,5555,6666,7777,8888)
SELECT col1 IN (SELECT ColID FROM tbl_ProductSales) FROM another_T
SELECT col1 IN (SELECT ColID + col1 FROM tbl_ProductSales) FROM another_T
SELECT col1 IN (SELECT ColID + col1 FROM tbl_ProductSales) FROM another_T GROUP BY col1
SELECT col1 IN (SELECT ColID + another_T.col1 FROM tbl_ProductSales) FROM another_T GROUP BY col1
SELECT (col1 + 1) AS k, k IN (SELECT ColID + k FROM tbl_ProductSales) FROM another_T GROUP BY k ORDER BY 1
SELECT col5 = ALL (SELECT 1 FROM tbl_ProductSales HAVING MIN(col8) IS NULL) FROM another_T GROUP BY col1, col2, col5, col8
SELECT CASE WHEN 1 IN (SELECT MAX(col7) UNION ALL (SELECT MIN(ColID) FROM tbl_ProductSales INNER JOIN another_T t2 ON t2.col5 = t2.col1)) THEN 2 ELSE NULL END FROM another_T t1
SELECT CASE WHEN 1 IN (SELECT (SELECT MAX(col7))) THEN 2 ELSE NULL END FROM another_T t1
INSERT INTO test VALUES (1, 1), (2, 2)
SELECT (SELECT y FROM test t2 WHERE t1.x = 5) FROM test t1
SELECT (SELECT y FROM test t2 WHERE t1.x = 5) IS NULL FROM test t1
SELECT (SELECT y FROM test t2 WHERE t1.x = 5) IS NOT NULL FROM test t1
create table r as select * from values (1, 1, 'a', 'A'), (1, null, 'b', 'B'), (1, 2, 'c', 'C'), (2, null, 'd', 'D') t(ra, rb, x, y)
create table b as select * from values (1, 1, 1), (2, 1, 2), (3, 1, 3), (4, 1, null), (5, 2, 1), (6, 2, null), (7, 99, 99) t(id, ba, bb)
select (select {'__matches': count(*)} from r where ba = ra and bb = rb group by ra, rb) as ref1, from b
select id, ba, bb, coalesce((select ROW(min(x), min(y), count(*)) from r where ba = ra and bb = rb group by ra, rb), ROW(null, null, 0)) as ref1, coalesce((select ROW(min(x), min(y), count(*)) from r where (ba = ra or ra is null) group by ra order by ba = ra), ROW(null, null, 0)) as ref4 from b ORDER BY 1, 2, 3
CREATE TABLE t1(fuel_type VARCHAR, location_country VARCHAR)
INSERT INTO t1 VALUES('natural_gas', 'US')
CREATE TABLE t2(__input_row_id BIGINT, "__input.fuel" VARCHAR)
INSERT INTO t2 VALUES(1, 'natural_gas')
SELECT ( SELECT NULL FROM ( SELECT fuel_type, location_country FROM "t1" WHERE "fuel_type" IS NOT DISTINCT FROM "__input.fuel" LIMIT 1 ) t1) FROM t2 AS __p
CREATE MACRO array_rv(arr) AS ( SELECT CASE WHEN l IS NOT NULL THEN l ELSE arr END FROM ( SELECT array_agg(elm ORDER BY g DESC) as l FROM (SELECT generate_subscripts(arr, 1) AS g, arr[g] AS elm) ) )
CREATE MACRO array_rv_coal(arr) AS ( SELECT COALESCE(l,arr) FROM ( SELECT array_agg(elm ORDER BY g DESC) as l FROM (SELECT generate_subscripts(arr, 1) AS g, arr[g] AS elm) ) )
CREATE TABLE t AS ( SELECT [1, 2, 3] AS arr UNION ALL SELECT [4, 5] AS arr UNION ALL SELECT [] AS arr )
SELECT array_rv(arr) FROM t ORDER BY arr
SELECT array_rv_coal(arr) FROM t ORDER BY arr
CREATE TABLE test AS FROM VALUES (1, 22), (1, 21), (2, 22) v(id, test_value)
CREATE TABLE test2 AS FROM VALUES (1, 44), (2, 42) v(id, test2_value)
SELECT * FROM test, test2 WHERE test.id=test2.id AND test_value*test2_value=(SELECT MIN(test_value*test2_value) FROM test AS a, test2 WHERE a.id=test.id AND a.id=test2.id)
CREATE TABLE test2 (a INTEGER, c INTEGER, str2 VARCHAR)
INSERT INTO test2 VALUES (11, 1, 'a'), (12, 1, 'b'), (13, 4, 'b')
SELECT a, SUM(a), (SELECT SUM(a)+SUM(t1.b) FROM test) FROM test t1 GROUP BY a ORDER BY a
SELECT (SELECT test.a+test.b+SUM(test2.a) FROM test2 WHERE str=str2) FROM test ORDER BY 1
SELECT * FROM test WHERE EXISTS(SELECT * FROM test2 WHERE test.a=test2.a AND test.b<>test2.c) order by b
SELECT a, a>=ANY(SELECT test2.a+c-b FROM test2 WHERE c>=b AND str=str2) FROM test ORDER BY 1
SELECT str, str=ANY(SELECT str2 FROM test2) FROM test
SELECT str, str=ANY(SELECT str2 FROM test2 WHERE test.a<>test2.a) FROM test
SELECT 1+(SELECT 1)
SELECT 1=(SELECT 1)
SELECT 1<>(SELECT 1)
SELECT 1=(SELECT NULL)
SELECT NULL=(SELECT 1)
SELECT (SELECT 42)
SELECT (SELECT (SELECT 42))
SELECT * FROM (SELECT 42) v1(a)
SELECT * FROM (SELECT 42, 41 AS x) v1(a)
INSERT INTO test VALUES (11, 22)
INSERT INTO test VALUES (12, 21)
INSERT INTO test VALUES (13, 22)
SELECT 1+(WITH cte AS (SELECT 1) SELECT * FROM cte)
SELECT 1=(WITH cte AS (SELECT 1) SELECT * FROM cte)
SELECT 1<>(WITH cte AS (SELECT 1) SELECT * FROM cte)
SELECT 1=(WITH cte AS (SELECT NULL) SELECT * FROM cte)
SELECT (WITH cte AS (SELECT 42) SELECT * FROM cte)
SELECT (WITH cte1 AS (WITH cte2 AS (SELECT 42) SELECT * FROM cte2) SELECT * FROM cte1)
SELECT * FROM (WITH cte(x) AS (SELECT 42) SELECT x FROM cte) v1(a)
SELECT * FROM (WITH cte AS (SELECT 42, 41 AS x) SELECT * FROM cte) v1(a)
SELECT (WITH cte AS (SELECT a * 42 FROM test) SELECT * FROM cte) IN (462, 504, 546)
SELECT a*(WITH cte AS (SELECT 42) SELECT * FROM cte) FROM test
CREATE TABLE t1(a INTEGER, b INTEGER, c INTEGER, d INTEGER, e INTEGER)
INSERT INTO t1(e,c,b,d,a) VALUES(103,102,100,101,104)
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i=integers.i) i1 JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i=integers.i) i1 LEFT JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i=integers.i) i1 RIGHT JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM generate_series(1, 2, 1) tbl(i) LEFT JOIN (SELECT * FROM integers i1 WHERE i1.i=integers.i) i1 ON i1.i=tbl.i) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM generate_series(1, 2, 1) tbl(i) RIGHT JOIN (SELECT * FROM integers i1 WHERE i1.i=integers.i) i1 ON i1.i=tbl.i) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i IS NOT DISTINCT FROM integers.i) i1 JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i OR (i1.i IS NULL AND tbl.i IS NULL)) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i IS NOT DISTINCT FROM integers.i) i1 LEFT JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i OR (i1.i IS NULL AND tbl.i IS NULL)) FROM integers ORDER BY i NULLS LAST
SELECT i IN (SELECT i1.i FROM (SELECT * FROM integers i1 WHERE i1.i IS NOT DISTINCT FROM integers.i) i1 RIGHT JOIN generate_series(1, 2, 1) tbl(i) ON i1.i=tbl.i OR (i1.i IS NULL AND tbl.i IS NULL)) FROM integers ORDER BY i NULLS LAST
CREATE TABLE item(i_manufact INTEGER)
SELECT * FROM item i1 WHERE (SELECT count(*) AS item_cnt FROM item WHERE (i_manufact = i1.i_manufact AND i_manufact=3) OR (i_manufact = i1.i_manufact AND i_manufact=3)) > 0 ORDER BY 1 LIMIT 100
SELECT * FROM item i1 WHERE (SELECT count(*) AS item_cnt FROM item WHERE (i_manufact = i1.i_manufact AND i_manufact=3) OR (i_manufact = i1.i_manufact AND i_manufact=3)) ORDER BY 1 LIMIT 100
SELECT * FROM integers WHERE i=(SELECT 1)
SELECT * FROM integers WHERE i=(SELECT SUM(1))
SELECT * FROM integers WHERE i=(SELECT MIN(i) FROM integers)
SELECT *, (SELECT MAX(i) FROM integers) FROM integers ORDER BY i
SELECT (SELECT 42) AS k, MAX(i) FROM integers GROUP BY k
SELECT i, MAX((SELECT 42)) FROM integers GROUP BY i ORDER BY i
SELECT (SELECT * FROM integers WHERE i>10) FROM integers
SELECT * FROM integers WHERE i=(SELECT i FROM integers WHERE i IS NOT NULL ORDER BY i LIMIT 1)
SELECT * FROM integers WHERE EXISTS (SELECT 1, 2)
SELECT * FROM integers WHERE EXISTS (SELECT i, i + 2 FROM integers)
SELECT (SELECT * FROM integers WHERE i=1)
SELECT (SELECT i FROM integers WHERE i=1)
CREATE TABLE strings(v VARCHAR)
INSERT INTO strings VALUES ('hello'), ('world'), (NULL)
SELECT NULL IN (SELECT * FROM strings)
SELECT 'hello' IN (SELECT * FROM strings)
SELECT 'bla' IN (SELECT * FROM strings)
SELECT 'bla' IN (SELECT * FROM strings WHERE v IS NOT NULL)
SELECT * FROM strings WHERE EXISTS(SELECT NULL)
SELECT * FROM strings WHERE EXISTS(SELECT v FROM strings WHERE v='bla')
SELECT (SELECT v FROM strings WHERE v='hello') FROM strings
SELECT (SELECT v FROM strings WHERE v='bla') FROM strings
SELECT (SELECT UNNEST([1]))
SELECT (SELECT UNNEST([NULL]))
SELECT (SELECT UNNEST([]))
SELECT (SELECT UNNEST(i)) FROM (VALUES ([1])) tbl(i)
SELECT (SELECT UNNEST(i)) FROM (VALUES ([NULL])) tbl(i)
SELECT (SELECT UNNEST(i)) FROM (VALUES ([])) tbl(i)
SELECT (SELECT SUM(k) FROM (SELECT UNNEST(i)) tbl(k)) FROM (VALUES ([1, 2, 3])) tbl(i)
SELECT (SELECT SUM(k)+SUM(l) FROM (SELECT UNNEST(i), UNNEST(j) FROM (VALUES ([1, 2, 3])) tbl(j)) tbl(k, l)) FROM (VALUES ([1, 2, 3])) tbl(i)
SELECT 1=ANY(SELECT UNNEST(i)) FROM (VALUES ([1, 2, 3])) tbl(i)
SELECT 4=ANY(SELECT UNNEST(i)) FROM (VALUES ([1, 2, 3])) tbl(i)
SELECT NULL=ANY(SELECT UNNEST(i)) FROM (VALUES ([1, 2, 3])) tbl(i)
SELECT 4=ANY(SELECT UNNEST(i)) FROM (VALUES ([1, 2, 3, NULL])) tbl(i)
UPDATE integers i1 SET i=(SELECT MAX(i) FROM integers WHERE i1.i<>i)
UPDATE integers i1 SET i=(SELECT MAX(i) FROM integers) WHERE i=(SELECT MIN(i) FROM integers)
UPDATE integers i1 SET i=(SELECT MAX(id) FROM integers WHERE id<i1.id)
UPDATE integers i1 SET i=2 WHERE i<(SELECT MAX(id) FROM integers WHERE i1.id<id)
UPDATE integers i1 SET i=DEFAULT WHERE i=(SELECT MIN(i) FROM integers WHERE i1.id<id)
SELECT NULL IN (SELECT * FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT '3' IN (SELECT * FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT 'hello' IN (SELECT * FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT 'bla' IN (SELECT * FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT 'hello' IN (SELECT * FROM strings WHERE v=s1.v or v IS NULL) FROM strings s1 ORDER BY v
SELECT 'bla' IN (SELECT * FROM strings WHERE v=s1.v or v IS NULL) FROM strings s1 ORDER BY v
SELECT * FROM strings WHERE EXISTS(SELECT NULL, v) ORDER BY v
SELECT * FROM strings s1 WHERE EXISTS(SELECT v FROM strings WHERE v=s1.v OR v IS NULL) ORDER BY v
SELECT * FROM strings s1 WHERE EXISTS(SELECT v FROM strings WHERE v=s1.v) ORDER BY v
SELECT (SELECT v FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT (SELECT v FROM strings WHERE v=s1.v OR (v='hello' AND s1.v IS NULL)) FROM strings s1 ORDER BY v
SELECT i, (SELECT row_number() OVER (ORDER BY i) FROM integers WHERE i1.i=i) FROM integers i1 ORDER BY i
SELECT i1.i, (SELECT rank() OVER (ORDER BY i) FROM integers WHERE i1.i=i) FROM integers i1, integers i2 ORDER BY i1.i
SELECT i1.i, (SELECT row_number() OVER (ORDER BY i) FROM integers WHERE i1.i=i) FROM integers i1, integers i2 ORDER BY i1.i
SELECT i, (SELECT SUM(i) OVER (ORDER BY i) FROM integers WHERE i1.i=i) FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(s1.i) OVER (ORDER BY s1.i) FROM integers s1, integers s2 WHERE i1.i=s1.i LIMIT 1) FROM integers i1 ORDER BY i
create table quantiles as select range r, random() FROM range(100) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
CALL dbgen(sf=0.001)
PRAGMA verify_external
SELECT quantile_disc(0.1::decimal(4,1), [0.1, 0.5, 0.9])
SELECT PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY "l_extendedprice") FROM lineitem
INSERT INTO test VALUES (11, 22), (13, 22), (12, 21)
CREATE TABLE qt (a INTEGER, b CHAR(1), c INTEGER)
INSERT INTO qt VALUES (1, 'A', 1), (2, 'A', 2), (3, 'B', 1), (4, 'B', 2)
SELECT * from qt QUALIFY row_number() over (PARTITION BY b ORDER BY c) = 1 ORDER BY b
SELECT a, b, c, row_number() over (PARTITION BY b ORDER BY c) as row_num FROM qt QUALIFY row_num = 1 ORDER BY b
CREATE TABLE exam (student TEXT, subject TEXT, mark INTEGER)
INSERT INTO exam VALUES ('Lily', 'Maths', 65), ('Lily', 'Science', 80), ('Lily', 'english', 70), ('Isabella', 'Maths', 50), ('Isabella', 'Science', 70), ('Isabella', 'english', 90), ('Olivia', 'Maths', 55), ('Olivia', 'Science', 60), ('Olivia', 'english', 89)
SELECT * FROM exam QUALIFY rank() OVER (ORDER BY mark desc) = 4
SELECT * FROM exam QUALIFY rank() OVER (PARTITION BY student ORDER BY mark DESC) = 2 ORDER BY student
SELECT * FROM exam WINDOW w AS (ORDER BY mark) QUALIFY row_number() OVER w >= 1 AND (rank() OVER w) <= 2 ORDER BY student
SELECT * FROM exam QUALIFY first_value(mark) OVER (PARTITION BY student ORDER BY mark) >= 60 order by mark
SELECT * FROM exam QUALIFY last_value(mark) OVER (PARTITION BY student ORDER BY mark) >= 85 order by mark
CREATE MACRO plus1(x) AS (x + (SELECT COUNT(*) FROM (SELECT b, SUM(test.a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1)))
SELECT plus1(3)
SELECT plus1(5)
DROP MACRO plus1
set enable_view_dependencies=true
CREATE TABLE test.t (a INTEGER, b INTEGER)
INSERT INTO test.t VALUES (11, 22), (13, 22), (12, 21)
CREATE VIEW test.v AS SELECT * FROM test.t QUALIFY row_number() OVER (PARTITION BY b) = 1
SELECT b, SUM(a) FROM test.v GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1 ORDER BY ALL
DROP TABLE test.t CASCADE
SELECT b, sum(a) AS a FROM (VALUES (1, 0), (1, 1)) t(a, b) GROUP BY b HAVING a > 0 ORDER BY ALL
create table t1(a int)
insert into t1 values (42), (84)
select a+1 as a from t1 group by a having a=42
create table t2(a int)
insert into t2 values (42), (84), (42)
select a as b, sum(a) as a from t2 group by b having a=42
SELECT 1 AS one FROM ( values (1,2), (3,2) ) t(a, b) HAVING 1 < 2
SELECT 1 AS one FROM ( values (1,2), (3,2) ) t(a, b) HAVING false
select sum(a) FROM ( values (1,2), (3,2) ) t(a, b) HAVING true
SELECT b, SUM(a) AS sum FROM test GROUP BY b HAVING b=21 ORDER BY b
SELECT b, SUM(a) FROM test GROUP BY b HAVING SUM(a) < 20 ORDER BY b
SELECT b, SUM(a) AS sum FROM test GROUP BY b HAVING sum < 20 ORDER BY b
SELECT b, SUM(a) AS sum FROM test GROUP BY b HAVING SUM(a) < 20 ORDER BY b
SELECT b, SUM(a) AS sum FROM test GROUP BY b HAVING COUNT(*) = 1 ORDER BY b
SELECT b, SUM(a) FROM test GROUP BY b HAVING SUM(a)+10>28
SELECT b, SUM(a) FROM test GROUP BY b HAVING SUM(a)>(SELECT SUM(t.a)*0.5 FROM test t)
SELECT test.b, SUM(a) FROM test GROUP BY test.b HAVING SUM(a)=(SELECT SUM(a) FROM test t WHERE test.b=t.b) ORDER BY test.b
SELECT test.b, SUM(a) FROM test GROUP BY test.b HAVING SUM(a)*2=(SELECT SUM(a)+SUM(t.a) FROM test t WHERE test.b=t.b) ORDER BY test.b
SELECT test.b, SUM(a) FROM test GROUP BY test.b HAVING SUM(a)*2+2=(SELECT SUM(a)+SUM(t.a)+COUNT(t.a) FROM test t WHERE test.b=t.b) ORDER BY test.b
SELECT test.b, SUM(a) FROM test GROUP BY test.b ORDER BY (SELECT SUM(a) FROM test t WHERE test.b=t.b) DESC
SELECT 42 HAVING 42 > 20
SELECT 42 HAVING 42 > 80
SELECT SUM(42) HAVING AVG(42) > MIN(20)
SELECT SUM(42) HAVING SUM(42) > SUM(80)
SELECT SUM(42)+COUNT(*)+COUNT(1), 3 HAVING SUM(42)+MAX(20)+AVG(30) > SUM(120)-MIN(100)
SELECT SUM(42) HAVING (SELECT SUM(42)) > SUM(80)
SELECT SUM(a) FROM test WHERE a=13 HAVING SUM(a) > 11
SELECT SUM(a) FROM test WHERE a=13 HAVING SUM(a) > 20
SELECT SUM(a) FROM test HAVING SUM(a)>10
SELECT SUM(a) FROM test HAVING SUM(a)<10
SELECT SUM(a) FROM test HAVING COUNT(*)>1
SELECT SUM(a) FROM test HAVING COUNT(*)>10
CREATE TABLE integers AS SELECT i%5 as even_groups, log(1 + i*i)::int as skewed_groups FROM range(10000) t(i)
SELECT list_sort(approx_top_k(even_groups, 10)) FROM integers
SELECT approx_top_k(skewed_groups, 5) FROM integers
SELECT approx_top_k(concat('this is a long prefix', skewed_groups::VARCHAR), 5) FROM integers
SELECT approx_top_k([skewed_groups], 5) FROM integers
SELECT approx_top_k({'i': skewed_groups}, 5) FROM integers
CREATE TABLE t1 (val VARCHAR, arg INT)
INSERT INTO t1 VALUES ('a', 2), ('a', 1), ('b', 5), ('b', 4), ('a', 3), ('b', 6)
SELECT arg_max(val, arg, 3 ORDER BY val DESC) FROM t1
SELECT list(rs.val) FROM (SELECT val, arg, row_number() OVER (ORDER BY arg DESC) as rid FROM t1 ORDER BY val) as rs WHERE rid < 4
SELECT arg_max(arg, val, 2 ORDER BY arg) FROM t1 GROUP BY val
CREATE TABLE t2 AS SELECT i%5 as even_groups, i FROM range(10000) t(i)
SELECT arg_max(even_groups, i, 3) FROM t2
select min(l_orderkey, 3) from lineitem
select max(l_orderkey, 3) from lineitem
SELECT l_returnflag, max( CASE WHEN l_returnflag='R' THEN null ELSE l_orderkey END, CASE WHEN l_returnflag='N' THEN 5 ELSE 3 END) FROM lineitem GROUP BY ALL ORDER BY ALL
CREATE MACRO compute_top_k(table_name, group_col, val_col, k) AS TABLE SELECT rs.grp, array_agg(rs.val ORDER BY rid) FROM ( SELECT group_col AS grp, val_col AS val, row_number() OVER (PARTITION BY group_col ORDER BY val_col DESC) as rid FROM query_table(table_name::VARCHAR) ORDER BY group_col DESC ) as rs WHERE rid <= k GROUP BY ALL ORDER BY ALL
SET disabled_optimizers = 'top_n_window_elimination'
SELECT * FROM compute_top_k(lineitem, l_returnflag, l_orderkey, 3)
SELECT l_returnflag, max(l_orderkey, 3) FROM lineitem GROUP BY ALL ORDER BY ALL
CREATE MACRO compute_bottom_k(table_name, group_col, val_col, k) AS TABLE SELECT rs.grp, array_agg(rs.val ORDER BY rid) FROM ( SELECT group_col AS grp, val_col AS val, row_number() OVER (PARTITION BY group_col ORDER BY val_col ASC) as rid FROM query_table(table_name::VARCHAR) ORDER BY group_col ASC ) as rs WHERE rid <= k GROUP BY ALL ORDER BY ALL
SELECT * FROM compute_bottom_k(lineitem, l_returnflag, l_orderkey, 3)
SELECT l_returnflag, min(l_orderkey, 3) FROM lineitem GROUP BY ALL ORDER BY ALL
CREATE TABLE tbl AS SELECT * FROM VALUES (1, 5, 1), (1, NULL, 2), (1, 3, NULL), (2, NULL, NULL), (3, 1, NULL) t(grp, arg, val)
SELECT arg_max_nulls_last(arg, val) FROM tbl
SELECT arg_max_nulls_last(arg, val, 1) FROM tbl
SELECT arg_max_nulls_last(val, val, 4) FROM tbl
SELECT grp, arg_max_nulls_last(arg, val) FROM tbl GROUP BY grp ORDER BY grp
SELECT grp, arg_max_nulls_last(arg, val, 2) FROM tbl GROUP BY grp ORDER BY grp
SELECT arg_min_nulls_last(arg, val) FROM tbl
SELECT arg_min_nulls_last(arg, val, 1) FROM tbl
SELECT arg_min_nulls_last(val, val, 4) FROM tbl
SELECT grp, arg_min_nulls_last(arg, val) FROM tbl GROUP BY grp ORDER BY grp
SELECT grp, arg_min_nulls_last(arg, val, 2) FROM tbl GROUP BY grp ORDER BY grp
SELECT equi_width_bins(0, 10, 2, true)
SELECT equi_width_bins(1000000, 1000010, 2, true)
SELECT equi_width_bins(99, 101, 2, true)
SELECT equi_width_bins(9, 11, 2, true)
SELECT equi_width_bins(10, 11, 2, true)
SELECT equi_width_bins(0, 5, 10, true)
SELECT equi_width_bins(0, 10, 5, true)
SELECT equi_width_bins(-10, 0, 5, true)
SELECT equi_width_bins(-10, 10, 5, true)
SELECT equi_width_bins(0, 9, 5, true)
SELECT equi_width_bins(0, 1734, 10, true)
SELECT equi_width_bins(0, 1724, 10, true)
CREATE TABLE t1 (k VARCHAR, el VARCHAR)
CREATE VIEW t1_v AS (SELECT * FROM t1 LIMIT 0)
CREATE TABLE el_ids (el VARCHAR, idx INTEGER)
INSERT INTO el_ids VALUES ('el', 10)
SELECT k, bitstring_agg(idx) FROM t1_v JOIN el_ids USING (el) GROUP BY k
SELECT is_histogram_other_bin(x::BIGINT) FROM(VALUES(1), (NULL)) t(x)
CREATE TABLE obs(n BIGINT)
INSERT INTO obs VALUES (0), (5), (7), (12), (20), (23), (24), (25), (26), (28), (31), (34), (36), (41), (47)
SELECT histogram_exact(n, [10, 20, 30, 40, 50]) FROM obs
SELECT histogram_exact(n::double, [10, 20, 30, 40, 50]) FROM obs
SELECT histogram_exact((date '2000-01-01' + interval (n) days)::date, [date '2000-01-01' + interval (x) days for x in [10, 20, 30, 40, 50]]) FROM obs
SELECT histogram_exact(n::varchar, [10, 20, 30, 40, 50]) FROM obs
SELECT histogram_exact([n], [[x] for x in [10, 20, 30, 40, 50]]) FROM obs
SELECT case when is_histogram_other_bin(bin) then '(other values)' else bin::varchar end as bin, count FROM ( SELECT UNNEST(map_keys(hist)) AS bin, UNNEST(map_values(hist)) AS count FROM (SELECT histogram_exact(n, [10, 20, 30, 40, 50]) AS hist FROM obs) )
SELECT case when is_histogram_other_bin(bin) then '(other values)' else bin::varchar end as bin, count FROM ( SELECT UNNEST(map_keys(hist)) AS bin, UNNEST(map_values(hist)) AS count FROM (SELECT histogram(n, [10, 20, 30, 40]) AS hist FROM obs) )
SELECT histogram_exact(r, [0, 1, 2, 3]) FROM range(4) t(r)
SELECT is_histogram_other_bin(NULL)
SELECT is_histogram_other_bin([[1]])
create table integers(i int)
insert into integers values (42)
insert into integers values (84)
SELECT * FROM histogram_values(integers, i, bin_count := 2)
INSERT INTO integers FROM range(127)
SELECT * FROM histogram_values(integers, i, bin_count => 10, technique => 'equi-width')
SELECT bin, count FROM histogram(integers, i, bin_count := 10, technique := 'equi-width')
INSERT INTO integers VALUES (99999999)
SELECT COUNT(*), AVG(count) FROM histogram_values(integers, i, technique := 'equi-height')
SELECT * FROM histogram_values(integers, i%2, technique := 'sample')
SELECT * FROM histogram_values(integers, (i%2)::VARCHAR)
SELECT COUNT(*), AVG(count) FROM histogram_values(integers, i::VARCHAR, technique := 'equi-height')
CREATE MACRO compute_top_k(table_name, group_col, val_col, k) AS TABLE SELECT rs.grp, array_agg(rs.val) FROM ( SELECT group_col AS grp, val_col AS val, row_number() OVER (PARTITION BY group_col ORDER BY val_col DESC) as rid FROM query_table(table_name::VARCHAR) ORDER BY group_col DESC ) as rs WHERE rid <= k GROUP BY ALL ORDER BY ALL
create table all_types as from test_all_types()
CREATE OR REPLACE TABLE window_table AS SELECT * FROM compute_top_k(tbl, grp_col, val_col, 2) as rs(grp, res)
SET disabled_optimizers = ''
CREATE OR REPLACE TABLE agg_table AS SELECT grp_col as grp, max(val_col, 2) as res FROM tbl GROUP BY ALL ORDER BY ALL
SELECT * FROM (SELECT * FROM window_table ORDER BY rowid) EXCEPT SELECT * FROM (SELECT * FROM agg_table ORDER BY rowid)
WITH my_data as ( SELECT 'text1'::varchar(1000) as my_column union all SELECT 'text1'::varchar(1000) as my_column union all SELECT 'text1'::varchar(1000) as my_column ) SELECT string_agg(my_column,', ') as my_string_agg FROM my_data
WITH my_data as ( SELECT 1 as dummy, 'text1'::varchar(1000) as my_column union all SELECT 1 as dummy, 'text1'::varchar(1000) as my_column union all SELECT 1 as dummy, 'text1'::varchar(1000) as my_column ) SELECT string_agg(my_column,', ') as my_string_agg FROM my_data GROUP BY dummy
SELECT NULL as a, NULL as b, NULL as c, NULL as d, 1 as id UNION SELECT 'Кирилл' as a, 'Müller' as b, '我是谁' as c, 'ASCII' as d, 2 as id ORDER BY 1
CREATE TABLE test (a INTEGER, s VARCHAR)
INSERT INTO test VALUES (11, 'hello'), (12, 'world'), (11, NULL)
SELECT COUNT(*), COUNT(s) FROM test
SELECT a, COUNT(*), COUNT(s) FROM test GROUP BY a ORDER BY a
SELECT s, SUM(a) FROM test GROUP BY s ORDER BY s
INSERT INTO test VALUES (11, 'hello'), (12, 'world')
SELECT COUNT(*), COUNT(s), COUNT(DISTINCT s) FROM test
SELECT a, COUNT(*), COUNT(s), COUNT(DISTINCT s) FROM test GROUP BY a ORDER BY a
SELECT a, COUNT(*), COUNT(s), COUNT(DISTINCT s) FROM test WHERE s IS NOT NULL GROUP BY a ORDER BY a
SELECT MIN(s), MAX(s) FROM test_strings
CREATE TABLE strings(s STRING, g INTEGER)
INSERT INTO strings VALUES ('hello', 0), ('world', 1), (NULL, 0), ('r', 1)
SELECT COUNT(*), COUNT(s), MIN(s), MAX(s) FROM strings
SELECT COUNT(*), COUNT(s), MIN(s), MAX(s) FROM strings WHERE s IS NULL
SELECT STRING_AGG(s, ' '), STRING_AGG(s, ''), STRING_AGG('', ''), STRING_AGG('hello', ' ') FROM strings
SELECT g, COUNT(*), COUNT(s), MIN(s), MAX(s), STRING_AGG(s, ' ') FROM strings GROUP BY g ORDER BY g
SELECT g, COUNT(*), COUNT(s), MIN(s), MAX(s), STRING_AGG(DISTINCT g::VARCHAR ORDER BY g::VARCHAR DESC) FROM strings GROUP BY g ORDER BY g
SELECT g, COUNT(*), COUNT(s), MIN(s), MAX(s), STRING_AGG(DISTINCT s ORDER BY s ASC) FROM strings GROUP BY g ORDER BY g
SELECT g, COUNT(*), COUNT(s), MIN(s), MAX(s), STRING_AGG(s, ' ') FROM strings WHERE s IS NULL OR s <> 'hello' GROUP BY g ORDER BY g
CREATE TABLE booleans(b BOOLEAN, g INTEGER)
INSERT INTO booleans VALUES (false, 0), (true, 1), (NULL, 0), (false, 1)
SELECT COUNT(*), COUNT(b), MIN(b), MAX(b) FROM booleans
SELECT COUNT(), COUNT(1), COUNT(*), COUNT(NULL), COUNT('hello'), COUNT(DATE '1992-02-02')
SELECT SUM(1), SUM(NULL), SUM(33.3)
SELECT SUM(True)
SELECT MIN(1), MIN(NULL), MIN(33.3), MIN('hello'), MIN(True), MIN(DATE '1992-02-02'), MIN(TIMESTAMP '2008-01-01 00:00:01')
SELECT MIN(1, 2)
SELECT MAX(1), MAX(NULL), MAX(33.3), MAX('hello'), MAX(True), MAX(DATE '1992-02-02'), MAX(TIMESTAMP '2008-01-01 00:00:01')
SELECT MAX(1, 2)
SELECT FIRST(1), FIRST(NULL), FIRST(33.3), FIRST('hello'), FIRST(True), FIRST(DATE '1992-02-02'), FIRST(TIMESTAMP '2008-01-01 00:00:01')
SELECT LAST(1), LAST(NULL), LAST(33.3), LAST('hello'), LAST(True), LAST(DATE '1992-02-02'), LAST(TIMESTAMP '2008-01-01 00:00:01')
SELECT AVG(1), AVG(NULL), AVG(33.3)
SELECT AVG(DATE '1992-02-02')
SELECT STRING_AGG('hello')
CREATE TABLE tbl(i INTEGER)
INSERT INTO tbl VALUES (NULL), (2), (3)
SELECT ANY_VALUE(i) AS a FROM tbl
SELECT ANY_VALUE(i) FROM five
SELECT i % 3 AS g, ANY_VALUE(i) FROM five GROUP BY 1 ORDER BY 1
SELECT ANY_VALUE(i ORDER BY 5-i) FROM five
SELECT i % 3 AS g, ANY_VALUE(i ORDER BY 5-i) FROM five GROUP BY 1 ORDER BY 1
DROP TABLE five
SELECT i::INTEGER % 3 AS g, ANY_VALUE(i ORDER BY 5-i) FROM five GROUP BY 1 ORDER BY 1
CREATE TABLE five_dates AS SELECT 1 AS i, NULL::DATE AS d, NULL::TIMESTAMP AS dt, NULL::TIME AS t, NULL::INTERVAL AS s UNION ALL SELECT i::integer AS i, '2021-08-20'::DATE + i::INTEGER AS d, '2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR AS dt, '14:59:37'::TIME + INTERVAL (i) MINUTE AS t, INTERVAL (i) SECOND AS s FROM range(1, 6, 1) t1(i)
SELECT ANY_VALUE(d), ANY_VALUE(dt), ANY_VALUE(t), ANY_VALUE(s) FROM five_dates
SELECT i % 3 AS g, ANY_VALUE(d), ANY_VALUE(dt), ANY_VALUE(t), ANY_VALUE(s) FROM five_dates GROUP BY 1 ORDER BY 1
CREATE TABLE tbl(a INTEGER, b VARCHAR)
INSERT INTO tbl VALUES (1, NULL), (2, 'thisisalongstring'), (3, 'thisisalsoalongstring')
SELECT ANY_VALUE(b) FROM tbl
SELECT ANY_VALUE(b) FROM tbl WHERE a=2
SELECT ANY_VALUE(b) FROM tbl WHERE a=1
SELECT ANY_VALUE(b) FROM tbl WHERE a=1 GROUP BY a
SELECT ANY_VALUE(b) FROM tbl WHERE a=0
SELECT ANY_VALUE(b) FROM tbl WHERE a=0 GROUP BY b
SELECT a, ANY_VALUE(b) FROM tbl GROUP BY a ORDER BY a
SELECT ANY_VALUE(i) FROM (VALUES (NULL::INT32)) tbl(i)
create table quantile as select range r, random() from range(10000) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
SELECT return_type, count(*) AS defined FROM duckdb_functions() WHERE function_name = 'reservoir_quantile' GROUP BY ALL HAVING defined <> 2 ORDER BY ALL
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(r, 0.5) as approx_quantile ,quantile(r,0.5) as true_quantile FROM quantile) AS T
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(r, 1.0) as approx_quantile ,quantile(r, 1.0) as true_quantile FROM quantile) AS T
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(r, 0.0) as approx_quantile ,quantile(r, 0.0) as true_quantile from quantile) AS T
SELECT approx_quantile(NULL, 0.5) as approx_quantile ,quantile(NULL, 0.5) as true_quantile
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(42, 0.5) as approx_quantile ,quantile(42, 0.5) as true_quantile) AS T
SELECT approx_quantile(NULL, 0.5) as approx_quantile ,quantile(NULL, 0.5) as true_quantile FROM quantile
SELECT approx_quantile(1, 0.5) as approx_quantile ,quantile(1, 0.5) as true_quantile FROM quantile
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(r, 0.1) as approx_quantile ,quantile(r, 0.1) as true_quantile from quantile) AS T
SELECT CASE WHEN ( approx_quantile between (true_quantile - 100) and (true_quantile + 100) ) THEN TRUE ELSE FALSE END FROM (SELECT approx_quantile(r, 0.9) as approx_quantile ,quantile(r, 0.9) as true_quantile from quantile) AS T
SELECT approx_quantile('1:02:03.000000+05:30'::TIMETZ, 0.5)
select approx_count_distinct(1)
select approx_count_distinct(NULL)
select approx_count_distinct('hello')
select approx_count_distinct(10), approx_count_distinct('hello') from range(100)
select approx_count_distinct(i) from range (100) tbl(i) WHERE 1 == 0
CREATE TABLE IF NOT EXISTS dates (t date)
INSERT INTO dates VALUES ('2008-01-01'), (NULL), ('2007-01-01'), ('2008-02-01'), ('2008-01-02'), ('2008-01-01'), ('2008-01-01'), ('2008-01-01')
CREATE TABLE IF NOT EXISTS timestamp (t TIMESTAMP)
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01'), (NULL), ('2007-01-01 00:00:01'), ('2008-02-01 00:00:01'), ('2008-01-02 00:00:01'), ('2008-01-01 10:00:00'), ('2008-01-01 00:10:00'), ('2008-01-01 00:00:10')
CREATE TABLE IF NOT EXISTS names (t string)
INSERT INTO names VALUES ('Pedro'), (NULL), ('Pedro'), ('Pedro'), ('Mark'), ('Mark'),('Mark'),('Hannes-Muehleisen'),('Hannes-Muehleisen')
create table t as select range a, mod(range,10) b from range(2000)
select argmin(NULL,NULL)
select argmin(1,1)
select argmin(i,i) from range (100) tbl(i)
select argmin(i,i) from range (100) tbl(i) where 1 = 0
select argmax(NULL,NULL)
select argmax(1,1)
select argmax(i,i) from range (100) tbl(i)
select argmax(i,i) from range (100) tbl(i) where 1 = 0
create table args (a integer, b integer)
insert into args values (1,1), (2,2), (8,8), (10,10)
select argmin(a,b), argmax(a,b) from args
select argmin(a,b), argmax(a,b) from args group by a%2 ORDER BY argmin(a,b)
select arg_min_null(NULL,NULL)
select arg_min_null(1,1)
select arg_min_null(i,i) from range (100) tbl(i)
select arg_min_null(i,i) from range (100) tbl(i) where 1 = 0
select arg_max_null(NULL,NULL)
select arg_max_null(1,1)
select arg_max_null(i,i) from range (100) tbl(i)
select arg_max_null(i,i) from range (100) tbl(i) where 1 = 0
select arg_min_null(a,b), arg_max_null(a,b) from args
select arg_min_null(a,b), arg_max_null(a,b) from args group by a%2 ORDER BY arg_min_null(a,b)
insert into args values (NULL, 0), (NULL, 12)
CREATE TABLE blobs (b BYTEA, a BIGINT)
SELECT AVG(3), AVG(NULL)
SELECT AVG(3::SMALLINT), AVG(NULL::SMALLINT)
SELECT AVG(3::DOUBLE), AVG(NULL::DOUBLE)
CREATE SEQUENCE seq
SELECT AVG(nextval('seq'))
SELECT AVG(i), AVG(1), AVG(DISTINCT i), AVG(NULL) FROM integers
SELECT AVG(i) FROM integers WHERE i > 100
CREATE TABLE intervals(itvl INTERVAL)
INSERT INTO intervals VALUES ('1 day'), ('30 days'), ('30 days'), ('30 days'), ('30 days')
SELECT AVG(itvl), AVG(DISTINCT itvl) FROM intervals
CREATE TABLE interval_tbl (f1 interval)
INSERT INTO interval_tbl (f1) VALUES ('@ 1 minute'), ('@ 5 hour'), ('@ 10 day'), ('@ 34 year'), ('@ 3 months'), ('@ 14 seconds ago'), ('1 day 2 hours 3 minutes 4 seconds'), ('6 years'), ('5 months'), ('5 months 12 hours')
CREATE TABLE bigints(n HUGEINT)
INSERT INTO bigints (n) VALUES ('9007199254740992'::HUGEINT), (1::HUGEINT), (0::HUGEINT)
SELECT AVG(n)::DOUBLE - '3002399751580331'::DOUBLE FROM bigints
SELECT histogram(n, [10, 20, 30, 40, 50]) FROM obs
SELECT histogram(n, [10, 20, 30, 40]) FROM obs
SELECT histogram(n::double, [10, 20, 30, 40]) FROM obs
SELECT histogram(n, []) FROM obs
SELECT histogram(n, [10, 40, 50, 30, 20]) FROM obs
SELECT n%2=0 is_even, histogram(n, [10, 20, 30, 40, 50]) FROM obs GROUP BY is_even ORDER BY is_even
SELECT n%2=0 is_even, histogram(n, case when n%2=0 then [10, 20, 30, 40, 50] else [11, 21, 31, 41, 51] end) FROM obs GROUP BY is_even ORDER BY is_even
SELECT histogram(i, range(999, 10000, 1000)) FROM range(10000) t(i)
SELECT histogram(v, [-9223372036854775808, -9223372036854775807, 9223372036854775807]) FROM (VALUES (-9223372036854775808), (-9223372036854775807), (0), (9223372036854775807)) t(v)
SELECT histogram(v, ['-infinity'::double, -10, 0, 10, 'infinity']) FROM (VALUES (-1e308), (-0.5), (0), ('inf'), ('-inf'), (0.5)) t(v)
SELECT histogram(v, range(timestamp '2000-01-01', timestamp '2005-01-01', interval '1 year')) FROM (VALUES (timestamp '2000-01-01'), (timestamp '2003-01-01')) t(v)
SELECT histogram(v, ['a', 'b', 'c', 'z']) FROM (VALUES ('a'), ('aaaa'), ('b'), ('c'), ('d')) t(v)
SELECT BIT_AND(3), BIT_AND(NULL)
SELECT BIT_AND(nextval('seq'))
INSERT INTO integers VALUES (3), (7), (15), (31), (3), (15)
SELECT BIT_AND(i), BIT_AND(1), BIT_AND(DISTINCT i), BIT_AND(NULL) FROM integers
SELECT BIT_AND(i) FROM integers WHERE i > 100
CREATE TABLE bits(b BIT)
INSERT INTO bits VALUES ('1110101011'), ('0111010101'), ('0101011101'), ('1111111111'), ('0100010011'), ('1100110011')
SELECT BIT_AND(b) FROM bits
SELECT BIT_AND(b) FROM bits WHERE get_bit(b, 2) = 1
SELECT BIT_AND('010110'::BIT)
SELECT BIT_OR(3), BIT_OR(NULL)
SELECT BIT_OR(nextval('seq'))
SELECT BIT_OR(i), BIT_OR(1), BIT_OR(DISTINCT i), BIT_OR(NULL) FROM integers
SELECT BIT_OR(i) FROM integers WHERE i > 100
INSERT INTO bits VALUES ('1010101001'), ('0011010101'), ('0001011101'), ('1011111101'), ('0000010001'), ('1000110001')
SELECT BIT_OR(b) FROM bits
SELECT BIT_OR(b) FROM bits WHERE get_bit(b, 3) = 0
SELECT BIT_OR('111010'::BIT)
SELECT BIT_XOR(3), BIT_XOR(NULL)
SELECT BIT_XOR(nextval('seq'))
SELECT BIT_XOR(i), BIT_XOR(1), BIT_XOR(DISTINCT i), BIT_XOR(NULL) FROM integers
SELECT BIT_XOR(i) FROM integers WHERE i > 100
SELECT BIT_XOR(b) FROM bits
SELECT BIT_XOR(b) FROM bits WHERE get_bit(b, 3) = 1
SELECT BIT_XOR('101011'::BIT)
SELECT BIT_XOR('0010101010101010101101011'::BIT) from bits
CREATE TABLE tinyints(i TINYINT)
INSERT INTO tinyints VALUES(1), (8), (3), (12), (7), (1), (2), (8)
SELECT BITSTRING_AGG(i) FROM tinyints
SELECT bit_count(BITSTRING_AGG(i)) FROM tinyints WHERE i <= 7
CREATE TABLE smallints(i SMALLINT)
INSERT INTO smallints VALUES(1), (8), (-3), (12), (7), (1), (-1), (-9), (NULL), (-2), (8)
SELECT BITSTRING_AGG(i) FROM smallints
SELECT bit_count(BITSTRING_AGG(i)) FROM smallints WHERE i = 8
CREATE TABLE ints(i INTEGER)
INSERT INTO ints VALUES(10), (-5), (11), (NULL), (30), (11), (23), (17), (27), (15), (5), (14)
SELECT BITSTRING_AGG(i) FROM ints
SELECT bit_count(BITSTRING_AGG(i)) FROM ints WHERE i > 20 AND i < 28
select bool_or(NULL)
select bool_and(NULL)
SELECT bool_or(True) FROM range(100)
SELECT bool_and(True) FROM range(100)
SELECT bool_or(True) FROM range(100) tbl(i) WHERE 1=0
SELECT bool_and(True) FROM range(100) tbl(i) WHERE 1=0
create table t (d date)
insert into t values (DATE'2021-02-09'-1),(DATE'2021-02-09'+1),(NULL)
select bool_or(d > '2021-02-09') AS or_result, bool_and(d > '2021-02-09') AS and_result from t
select d,bool_or(d > '2021-02-09') AS or_result, bool_and(d > '2021-02-09') AS and_result from t group by d order by d
select bool_or(d > '2021-02-09') over (partition by d) from t order by d
select bool_and(d > '2021-02-09') over (partition by d) from t order by d
select corr(NULL,NULL)
select corr(1,1)
create table aggr(k int, v decimal(10,2), v2 decimal(10, 2))
insert into aggr values(1, 10, null),(2, 10, 11), (2, 20, 22), (2, 25, null), (2, 30, 35)
select k, corr(v, v2) from aggr group by k ORDER BY ALL
select corr(v, v2) from aggr
select corr(v, v2) over (partition by k) from aggr
SELECT COUNT(*), COUNT(1), COUNT(100), COUNT(NULL), COUNT(DISTINCT 1)
INSERT INTO integers VALUES (1), (2), (NULL)
SELECT COUNT(*), COUNT(1), COUNT(i), COUNT(COALESCE(i, 1)), COUNT(DISTINCT i), COUNT(DISTINCT 1) FROM integers
SELECT COUNT(1 ORDER BY 1)
CREATE TABLE int(i INT)
SELECT COUNT(i), COUNT(rowid) FROM int
SELECT rowid // 200 AS g, COUNT(i), COUNT(rowid) FROM int GROUP BY g
INSERT INTO integers VALUES (3, 4), (3, 4), (2, 4)
SELECT i, COUNT(*) FROM integers GROUP BY i ORDER BY i
SELECT i, COUNT() FROM integers GROUP BY i ORDER BY i
SELECT COVAR_POP(3,3), COVAR_POP(NULL,3), COVAR_POP(3,NULL), COVAR_POP(NULL,NULL)
SELECT COVAR_SAMP(3,3), COVAR_SAMP(NULL,3), COVAR_SAMP(3,NULL), COVAR_SAMP(NULL,NULL)
CREATE SEQUENCE seqx
CREATE SEQUENCE seqy
SELECT COVAR_POP(nextval('seqx'),nextval('seqy'))
CREATE TABLE integers(x INTEGER, y INTEGER)
INSERT INTO integers VALUES (10,NULL), (10,11), (20,22), (25,NULL), (30,35)
SELECT COVAR_POP(x,y), COVAR_POP(x,1), COVAR_POP(1,y), COVAR_POP(x,NULL), COVAR_POP(NULL,y) FROM integers
SELECT COVAR_SAMP(x,y), COVAR_SAMP(x,1), COVAR_SAMP(1,y), COVAR_SAMP(x,NULL), COVAR_SAMP(NULL,y) FROM integers
SELECT COVAR_POP(x,y), COVAR_SAMP(x,y) FROM integers WHERE x > 100
SELECT COVAR_POP(NULL, NULL), COVAR_SAMP(NULL, NULL) FROM integers
SELECT COUNT(*), COUNT(i), STDDEV_SAMP(i), SUM(i), SUM(DISTINCT i), FIRST(i), LAST(i), MAX(i), MIN(i) FROM integers WHERE i > 100
CREATE TABLE emptyaggr(i INTEGER)
SELECT COUNT(*) FROM emptyaggr
SELECT SUM(i), COUNT(i), COUNT(DISTINCT i), COUNT(*), AVG(i), COUNT(*)+1, COUNT(i)+1, MIN(i), MIN(i+1), MIN(i)+1 FROM emptyaggr
select entropy(NULL)
select entropy(1)
create table aggr(k int)
insert into aggr values (0),(1),(1),(1),(4),(0),(3),(3),(2),(2),(4),(4),(2),(4),(0),(0),(0),(1),(2),(3),(4),(2),(3),(3),(1)
select entropy(k) from aggr
SELECT entropy(2) FROM range(100)
select entropy(k) from aggr group by k%2 order by all
create table names (name string)
insert into names values ('pedro'), ('pedro'), ('pedro'),('hannes'),('hannes'),('mark'),(null)
select entropy(name) from names
create table array_names as select case when name is null then null else [name] end l from names
select entropy(l) from array_names
CREATE TABLE integers(i INTEGER, grp INTEGER)
INSERT INTO integers VALUES (1, NULL), (2, 3), (3, 2), (NULL, 1)
SELECT FIRST(i ORDER BY grp NULLS LAST) FROM integers
SELECT FIRST(i ORDER BY grp NULLS FIRST) FROM integers
SELECT ANY_VALUE(i ORDER BY grp NULLS FIRST) FROM integers
SELECT ANY_VALUE(i ORDER BY grp NULLS LAST) FROM integers
SELECT ARG_MIN(i, grp) FROM integers
SELECT FIRST(i ORDER BY grp DESC NULLS LAST) FROM integers
SELECT ANY_VALUE(i ORDER BY grp DESC NULLS FIRST) FROM integers
SELECT ANY_VALUE(i ORDER BY grp DESC NULLS LAST) FROM integers
SELECT ARG_MAX(i, grp) FROM integers
SELECT LAST(i ORDER BY grp NULLS FIRST) FROM integers
SELECT FIRST(b) FROM tbl WHERE a=2
SELECT ARBITRARY(b) FROM tbl WHERE a=2
SELECT FIRST(b) FROM tbl WHERE a=1
SELECT FIRST(b) FROM tbl WHERE a=1 GROUP BY a
SELECT FIRST(b) FROM tbl WHERE a=0
SELECT FIRST(b) FROM tbl WHERE a=0 GROUP BY b
SELECT a, FIRST(b) FROM tbl GROUP BY a ORDER BY a
SELECT FIRST(i) FROM (VALUES (NULL::INT32)) tbl(i)
CREATE TABLE integer(i INTEGER, j INTEGER)
INSERT INTO integer VALUES (3, 4), (3, 5), (3, 7)
SELECT j * 2 FROM integer GROUP BY j * 2 ORDER BY j * 2
SELECT integer.j * 2 FROM integer GROUP BY j * 2 ORDER BY j * 2
SELECT j * 2 FROM integer GROUP BY integer.j * 2 ORDER BY j * 2
SELECT j * 2 FROM integer GROUP BY j * 2 ORDER BY integer.j * 2
SELECT integer.j * 2 FROM integer GROUP BY j * 2 ORDER BY integer.j * 2
SELECT j * 2 FROM integer GROUP BY integer.j * 2 ORDER BY integer.j * 2
SELECT integer.j * 2 FROM integer GROUP BY integer.j * 2 ORDER BY j * 2
SELECT integer.j * 2 FROM integer GROUP BY integer.j * 2 ORDER BY integer.j * 2
SELECT j * 2 AS i FROM integer GROUP BY j * 2 ORDER BY i
select histogram(NULL)
SELECT histogram(i) FROM range(100) tbl(i) WHERE 1=0
select histogram(1)
SELECT histogram('、')
SELECT histogram(2) FROM range(100)
CREATE TABLE hist_data (g INTEGER, e INTEGER)
INSERT INTO hist_data VALUES (1, 1), (1, 2), (2, 3), (2, 4), (2, 5), (3, 6), (5, NULL)
SELECT histogram(g) from hist_data
SELECT histogram(e) from hist_data
select histogram(g) from hist_data group by g%2==0 ORDER BY g%2==0
select histogram(g) from hist_data where g < 3
insert into names values ('pedro'), ('pedro'), ('pedro'),('hannes'),('hannes'),('mark'),(null),('Hubert Blaine Wolfeschlegelsteinhausenbergerdorff Sr.')
create table tmp (c0 integer, c1 integer)
insert into tmp values (0, 0), (1, 1), (2, 0), (0, 1), (1, 0), (2, 1), (0, 0), (1, 1), (2, 0), (0, 1)
SELECT c0, histogram(c1) FROM tmp GROUP BY c0 ORDER BY ALL
CREATE TABLE doubles(n DOUBLE)
INSERT INTO doubles (n) VALUES ('9007199254740992'::DOUBLE), (1::DOUBLE), (1::DOUBLE), (0::DOUBLE)
SELECT FAVG(n) - '2251799813685248.5'::DOUBLE FROM doubles
SELECT FSUM(n)::BIGINT FROM doubles
SELECT sumKahan(n)::BIGINT FROM doubles
SELECT kahan_sum(n)::BIGINT FROM doubles
select kurtosis(NULL)
select kurtosis(1)
select kurtosis(i) from (values (0), (0), (0), (0), (0), (0)) tbl(i)
select kurtosis(10) from range (5)
select kurtosis(10) from range (5) where 1 == 0
create table aggr(k int, v int, v2 int)
insert into aggr values (1, 10, null), (2, 10, 11), (2, 10, 15), (2, 10, 18), (2, 20, 22), (2, 20, 25), (2, 25, null), (2, 30, 35), (2, 30, 40), (2, 30, 50), (2, 30, 51)
select kurtosis(k), kurtosis(v), kurtosis(v2) from aggr
select kurtosis_pop(k), kurtosis_pop(v), kurtosis_pop(v2) from aggr
with onetwo as (select range::float as v from range(1,3)) select kurtosis_pop(v) from onetwo
select kurtosis(v2) from aggr group by v ORDER BY ALL
select kurtosis(v2) over (partition by v) from aggr
SELECT LAST(i) FROM five
SELECT i % 3 AS g, LAST(i) FROM five GROUP BY 1 ORDER BY 1
SELECT LAST(i ORDER BY 5-i) FROM five
SELECT i % 3 AS g, LAST(i ORDER BY 5-i) FROM five GROUP BY 1 ORDER BY 1
SELECT i::INTEGER % 3 AS g, LAST(i ORDER BY 5-i) FROM five GROUP BY 1 ORDER BY 1
CREATE TABLE five_dates AS SELECT i::integer AS i, '2021-08-20'::DATE + i::INTEGER AS d, '2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR AS dt, '14:59:37'::TIME + INTERVAL (i) MINUTE AS t, INTERVAL (i) SECOND AS s FROM range(1, 6, 1) t1(i)
SELECT LAST(d), LAST(dt), LAST(t), LAST(s) FROM five_dates
SELECT i % 3 AS g, LAST(d), LAST(dt), LAST(t), LAST(s) FROM five_dates GROUP BY 1 ORDER BY 1
SELECT LAST(d ORDER BY 5-i), LAST(dt ORDER BY 5-i), LAST(t ORDER BY 5-i), LAST(s ORDER BY 5-i) FROM five_dates
SELECT i % 3 AS g, LAST(d ORDER BY 5-i), LAST(dt ORDER BY 5-i), LAST(t ORDER BY 5-i), LAST(s ORDER BY 5-i) FROM five_dates GROUP BY 1 ORDER BY 1
SELECT LAST(dt::TIMESTAMPTZ), LAST(t::TIMETZ) FROM five_dates
SELECT i % 3 AS g, LAST(dt::TIMESTAMPTZ), LAST(t::TIMETZ) FROM five_dates GROUP BY 1 ORDER BY 1
SELECT LAST(b) FROM tbl WHERE a=2
SELECT LAST(b) FROM tbl WHERE a=1
SELECT LAST(b) FROM tbl WHERE a=1 GROUP BY a
SELECT LAST(b) FROM tbl WHERE a=0
SELECT LAST(b) FROM tbl WHERE a=0 GROUP BY b
SELECT a, LAST(b) FROM tbl GROUP BY a ORDER BY a
SELECT LAST(i) FROM (VALUES (NULL::INT32)) tbl(i)
SELECT mad(NULL), mad(1)
SELECT mad(NULL), mad(1) FROM range(2000)
create table tinys as select range r, random() from range(100) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
create table numerics as select range r, random() from range(10000) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
SELECT mad(('2018-01-01'::DATE + INTERVAL (r) DAY)::DATE) FROM numerics
SELECT mad('2018-01-01'::TIMESTAMP + INTERVAL (r) HOUR) FROM numerics
SELECT mad('00:00:00'::TIME + INTERVAL (r) SECOND) FROM numerics
select mad(x) from (values ('127'::DECIMAL(3,0)), ('-128'::DECIMAL(3,0))) tbl(x)
select mad(x) from (values ('32767'::DECIMAL(5,0)), ('-32768'::DECIMAL(5,0))) tbl(x)
select mad(x) from (values ('2147483647'::DECIMAL(10,0)), ('-2147483648'::DECIMAL(10,0))) tbl(x)
select mad(x) from (values (-1e308), (1e308)) tbl(x)
select mad(x) from (values ('294247-01-10'::date), ('290309-12-22 (BC)'::date)) tbl(x)
SELECT median(NULL), median(1)
SELECT median(NULL), median(1) FROM range(2000)
SELECT median(r)::VARCHAR FROM quantile
SELECT median(r::float)::VARCHAR FROM quantile
SELECT median(r::double)::VARCHAR FROM quantile
SELECT median(r::tinyint)::VARCHAR FROM quantile where r < 100
SELECT median(r::smallint)::VARCHAR FROM quantile
SELECT median(r::integer)::VARCHAR FROM quantile
SELECT median(r::bigint)::VARCHAR FROM quantile
SELECT median(r::hugeint)::VARCHAR FROM quantile
SELECT median(r::decimal(10,2))::VARCHAR FROM quantile
SELECT median(case when r is null then null else [r] end)::VARCHAR FROM quantile
create table lists as select array[i] l from generate_series(0,5,1) tbl(i)
select min(l) from lists where l[1]>2
select min(l) from lists where l[0]>2
DESCRIBE SELECT max(l) from (select unnest( [{'a':1}::JSON, [2]::JSON ]) as l)
select mode(NULL)
select mode(1)
create table aggr(k int, v decimal(10,2))
insert into aggr (k, v) values (1, 10), (1, 10), (1, 20), (1, 21)
select mode(v) from aggr
SELECT mode(2) FROM range(100)
insert into aggr (k, v) values (2, 20),(2, 20), (2, 25), (2, 30)
SELECT CASE WHEN ( value = 10 or value = 20) THEN TRUE ELSE FALSE END FROM (select mode(v) as value from aggr) AS T
insert into aggr (k, v) values (3, null)
select k, mode(v) from aggr group by k order by k
select mode(name) from names
select k, v, mode(v) over (partition by k) from aggr order by k, v
CREATE TABLE t1(c0 BIGINT, c1 SMALLINT)
INSERT INTO t1 VALUES(NULL,NULL)
INSERT INTO t1 VALUES(-9121942514766415310,NULL)
INSERT INTO t1 VALUES(-9113483941634330359,NULL)
INSERT INTO t1 VALUES(-8718457747090493475,NULL)
INSERT INTO t1 VALUES(-7650527153348320600,NULL)
INSERT INTO t1 VALUES(-7511073704802549520,NULL)
INSERT INTO t1 VALUES(-7342137292157212364,NULL)
INSERT INTO t1 VALUES(-7003121677824953185,NULL)
INSERT INTO t1 VALUES(-6971852266038069200,NULL)
INSERT INTO t1 VALUES(-6873545755554765972,NULL)
INSERT INTO t1 VALUES(-6355311124878824053,NULL)
CREATE TABLE integers(grp INTEGER, i INTEGER)
INSERT INTO integers VALUES (1, 10), (2, 15), (1, 30), (2, 20)
SELECT FIRST(i ORDER BY i) FROM integers
SELECT FIRST(i ORDER BY i, i, i) FROM integers
SELECT FIRST(i ORDER BY i, i DESC, i) FROM integers
SELECT FIRST(i ORDER BY i DESC) FROM integers
SELECT FIRST(i ORDER BY i DESC, i ASC) FROM integers
SELECT FIRST(i ORDER BY i), FIRST(i ORDER BY i DESC) FROM integers
SELECT grp, FIRST(i ORDER BY i) FROM integers GROUP BY grp ORDER BY ALL
SELECT grp, FIRST(i ORDER BY grp, i, grp DESC, i DESC) FROM integers GROUP BY grp ORDER BY ALL
SELECT grp, FIRST(i ORDER BY i DESC) FROM integers GROUP BY grp ORDER BY ALL
CREATE TABLE user_causes ( user_id INT, cause VARCHAR, "date" DATE )
CREATE TABLE flights( "year" INTEGER, "month" INTEGER, "day" INTEGER, dep_time INTEGER, sched_dep_time INTEGER, dep_delay DOUBLE, arr_time INTEGER, sched_arr_time INTEGER, arr_delay DOUBLE, carrier VARCHAR, flight INTEGER, tailnum VARCHAR, origin VARCHAR, dest VARCHAR, air_time DOUBLE, distance DOUBLE, "hour" DOUBLE, "minute" DOUBLE, time_hour TIMESTAMP)
SELECT "dest", mode() WITHIN GROUP (ORDER BY "arr_delay") AS "median_delay" FROM "flights" GROUP BY "dest"
SELECT "dest", percentile_cont(0.5) WITHIN GROUP (ORDER BY "arr_delay") AS "median_delay" FROM "flights" GROUP BY "dest"
SELECT "dest", percentile_cont([0.25, 0.5, 0.75]) WITHIN GROUP (ORDER BY "arr_delay") AS "iqr_delay" FROM "flights" GROUP BY "dest"
SELECT "dest", percentile_disc(0.5) WITHIN GROUP (ORDER BY "arr_delay") AS "median_delay" FROM "flights" GROUP BY "dest"
SELECT "dest", percentile_disc([0.25, 0.5, 0.75]) WITHIN GROUP (ORDER BY "arr_delay") AS "iqr_delay" FROM "flights" GROUP BY "dest"
select percentile_disc(0.25) within group(order by i desc) from generate_series(0,100) tbl(i)
select percentile_disc([0.25, 0.5, 0.75]) within group(order by i desc) from generate_series(0,100) tbl(i)
select percentile_cont(0.25) within group(order by i desc) from generate_series(0,100) tbl(i)
select percentile_cont([0.25, 0.5, 0.75]) within group(order by i desc) from generate_series(0,100) tbl(i)
SELECT percentile_disc(.5) WITHIN GROUP (order by col desc) FROM VALUES (11000), (3100), (2900), (2800), (2600), (2500) AS tab(col)
SELECT percentile_disc([.25, .5, .75]) WITHIN GROUP (order by col desc) FROM VALUES (11000), (3100), (2900), (2800), (2600), (2500) AS tab(col)
PRAGMA perfect_ht_threshold=20
INSERT INTO timeseries VALUES (1996, 10), (1997, 12), (1996, 20), (2001, 30), (NULL, 1), (1996, NULL)
SELECT year, SUM(val), COUNT(val), COUNT(*) FROM timeseries GROUP BY year ORDER BY year
SELECT year, LIST(val), STRING_AGG(val::VARCHAR, ',') FROM timeseries GROUP BY year ORDER BY year
create table manycolumns as select i a, i b, i c, i d, i e from range(0,2) tbl(i)
select a, b, c, d, e FROM manycolumns GROUP BY 1, 2, 3, 4, 5
CREATE TABLE tinyints AS SELECT i::TINYINT::VARCHAR AS t FROM range(-127, 128) tbl(i)
SELECT COUNT(DISTINCT i), MIN(i), MAX(i), SUM(i) / COUNT(i) FROM (SELECT t::TINYINT t1 FROM tinyints GROUP BY t1) tbl(i)
CREATE TABLE smallints AS SELECT i::SMALLINT::VARCHAR AS t FROM range(-32767, 32768) tbl(i)
SELECT COUNT(DISTINCT i), MIN(i), MAX(i), SUM(i) / COUNT(i) FROM (SELECT t::SMALLINT t1 FROM smallints GROUP BY t1) tbl(i)
PRAGMA disable_verification
create table dates as select date '1992-01-01' + concat(i, ' months')::interval as d from range(100) tbl(i)
select product(NULL)
select product(1)
INSERT INTO integers VALUES (1), (2),(4), (NULL)
SELECT product(i) FROM integers
SELECT PRODUCT(2) FROM range(100)
SELECT PRODUCT(2) FROM range(100) tbl(i) WHERE i % 2 != 0
select product(i) from integers group by i%2 order by all
SELECT PRODUCT(i) FROM range(100) tbl(i) WHERE 1=0
select product(i) over (partition by i%2) from integers
create table quantile as select range r, random() from range(0,1000000,100) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
SELECT quantile_cont(r, 0.5) FROM quantile
SELECT quantile_cont(r::decimal(10,2), 0.5) FROM quantile
SELECT quantile_cont(r, 1.0) FROM quantile
SELECT quantile_cont(r, 0.0) FROM quantile
SELECT quantile_cont(NULL, 0.5) FROM quantile
SELECT quantile_cont(42, 0.5) FROM quantile
SELECT quantile_cont(NULL, 0.5)
SELECT quantile_cont(42, 0.5)
SELECT quantile_cont(r, 0.25), quantile_cont(r, 0.5), quantile_cont(r, 0.75) from quantile
SELECT mod(r,1000) as g, quantile_cont(r, 0.25) FROM quantile GROUP BY 1 ORDER BY 1
SELECT quantile_cont('2021-01-01'::TIMESTAMP + interval (r) second, 0.5) FROM quantile
create table quantiles as select range r, random() FROM range(0,1000000,100) union all values (NULL, 0.25), (NULL, 0.5), (NULL, 0.75) order by 2
SELECT quantile_cont('2021-01-01'::TIMESTAMP + interval (r/100) hour, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont('1990-01-01'::DATE + interval (r/100) day, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont('00:00:00'::TIME + interval (r/100) second, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(('2021-01-01'::TIMESTAMP + interval (r/100) hour)::TIMESTAMPTZ, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(r, [0.25, 0.5, 0.75]) FROM quantiles
SELECT mod(r,1000) as g, quantile_cont(r, [0.25, 0.5, 0.75]) FROM quantiles GROUP BY 1 ORDER BY 1
SELECT quantile_cont(1, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(r, [0.25, 0.5, 0.75]) FROM quantiles WHERE 1=0
SELECT quantile_cont(r, []) FROM quantiles
pragma threads=4
PRAGMA verify_parallelism
CREATE TABLE quantile as SELECT range r, random() AS q FROM range(10000) UNION ALL VALUES (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) ORDER BY 2
SELECT quantile_disc(r, 0.5) FROM quantile
SELECT quantile_disc(r::decimal(10,2), 0.5) FROM quantile
SELECT quantile_disc(case when r is null then null else [r] end, 0.5) FROM quantile
SELECT quantile_disc(case when r is null then null else {'i': r} end, 0.5) FROM quantile
SELECT quantile_disc(r, 1.0) FROM quantile
SELECT quantile_disc(r, 0.0) FROM quantile
SELECT quantile_disc(NULL, 0.5) FROM quantile
SELECT quantile_disc(42, 0.5) FROM quantile
SELECT quantile_disc(NULL, 0.5)
SELECT quantile_disc(42, 0.5)
SELECT quantile_disc(r, 0.1), quantile_disc(r, 0.5), quantile_disc(r, 0.9) from quantile
create table quantiles as select range r, random() FROM range(10000) union all values (NULL, 0.1), (NULL, 0.5), (NULL, 0.9) order by 2
SELECT quantile_disc(r, [0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc(case when r is null then null else [r] end, [0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc(case when r is null then null else {'i': r} end, [0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc(col, [-.25, -.5, -.75]) FROM VALUES (11000), (3100), (2900), (2800), (2600), (2500) AS tab(col)
SELECT quantile_disc(d::VARCHAR, [0.1, 0.5, 0.9]) FROM range(0,100) tbl(d)
SELECT mod(r,10) as g, quantile_disc(r, [0.1, 0.5, 0.9]) FROM quantiles GROUP BY 1 ORDER BY 1
SELECT quantile_disc(1, [0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc(r, [0.1, 0.5, 0.9]) FROM quantiles WHERE 1=0
SELECT quantile_disc(r, []) FROM quantiles
SELECT quantile_disc('2021-01-01'::TIMESTAMP + interval (r) hour, [0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc('1990-01-01'::DATE + interval (r) day, [0.1, 0.5, 0.9]) FROM quantiles
select regr_avgx(NULL,NULL)
select regr_avgx(1,1)
select regr_avgy(NULL,NULL)
select regr_avgy(1,1)
select regr_count(NULL,NULL)
select regr_count(1,1)
select regr_slope(NULL,NULL)
select regr_slope(1,1)
select regr_r2(NULL,NULL)
select regr_r2(1,1)
select regr_r2(1e230*i, 0) from range(5) tbl(i)
select regr_r2(0, i) from range(5) tbl(i)
SELECT COUNT(1), MIN(1), FIRST(1), LAST(1),MAX(1), SUM(1), STRING_AGG('hello', ',')
SELECT COUNT(NULL), MIN(NULL), FIRST(NULL), LAST(NULL), MAX(NULL), SUM(NULL), STRING_AGG(NULL, NULL)
SELECT FIRST(NULL)
SELECT LAST(NULL)
SELECT NULL as a, NULL as b, 1 as id UNION SELECT CAST('00:00:00' AS TIME) as a, CAST('12:34:56' AS TIME) as b, 2 as id ORDER BY 1
SELECT COUNT(1), MIN(1), FIRST(1), LAST(1), MAX(1), SUM(1), STRING_AGG('hello', ',') FROM integers
SELECT COUNT(NULL), MIN(NULL), FIRST(NULL), LAST(NULL), MAX(NULL), SUM(NULL), STRING_AGG(NULL, NULL) FROM integers
select sem(NULL)
select sem(1)
select k, sem(v),sem(v2) from aggr group by k ORDER BY ALL
select sem(v),sem(v2) from aggr
select k, sem(v) over (partition by k) from aggr order by all
SELECT count(*) as total_rows, count(*) FILTER (WHERE i <= 5) as lte_five, count(*) FILTER (WHERE i % 2 = 1) as odds FROM generate_series(1,11) tbl(i)
SELECT count(*) FILTER (WHERE i % 2 = 1) as odds, count(*) FILTER (WHERE i <= 5) as lte_five, count(*) as total_rows FROM generate_series(1,11) tbl(i)
SELECT count(*) FILTER (WHERE i <= 5) as lte_five, count(*) FILTER (WHERE i % 2 = 1) as odds, count(*) as total_rows FROM generate_series(1,11) tbl(i)
CREATE TABLE issue3105(gender VARCHAR, pay FLOAT)
INSERT INTO issue3105 VALUES ('male', 100), ('male', 200), ('male', 300), ('female', 150), ('female', 250)
SELECT SUM(pay) FILTER (WHERE gender = 'male'), SUM(pay) FILTER (WHERE gender = 'female'), SUM(pay) FROM issue3105
SELECT SUM(pay), SUM(pay) FILTER (WHERE gender = 'male'), SUM(pay) FILTER (WHERE gender = 'female') FROM issue3105
SELECT SUM(pay) FILTER (WHERE gender = 'male'), SUM(pay), SUM(pay) FILTER (WHERE gender = 'female') FROM issue3105
SELECT SUM(pay) FILTER (gender = 'male'), SUM(pay), SUM(pay) FILTER (gender = 'female') FROM issue3105
select skewness(NULL)
select skewness(1)
select skewness (10) from range (5)
select skewness (10) from range (5) where 1 == 0
select skewness(k), skewness(v), skewness(v2) from aggr
select skewness(v2) from aggr group by v ORDER BY ALL
select skewness(v2) over (partition by v) from aggr order by v
create table dummy as select range % 10 g, range d from range(100)
SELECT count(*), count(d), sum(d), avg(d)::integer, min(d), max(d) FROM dummy
SELECT finalize(count(*) EXPORT_STATE), finalize(count(d) EXPORT_STATE), finalize(sum(d) EXPORT_STATE), finalize(avg(d) EXPORT_STATE)::integer, finalize(min(d) EXPORT_STATE), finalize(max(d) EXPORT_STATE) FROM dummy
SELECT g, count(*), count(d), sum(d), avg(d)::integer, min(d), max(d) FROM dummy GROUP BY g ORDER BY g
SELECT g, finalize(count(*) EXPORT_STATE), finalize(count(d) EXPORT_STATE), finalize(sum(d) EXPORT_STATE), finalize(avg(d) EXPORT_STATE)::integer, finalize(min(d) EXPORT_STATE), finalize(max(d) EXPORT_STATE) FROM dummy GROUP BY g ORDER BY g
CREATE TABLE state AS SELECT g, count(*) EXPORT_STATE count_star_state, count(d) EXPORT_STATE count_state, sum(d) EXPORT_STATE sum_state, avg(d) EXPORT_STATE avg_state, min(d) EXPORT_STATE min_state, max(d) EXPORT_STATE max_state FROM dummy GROUP BY g ORDER BY g
SELECT g, finalize(count_star_state),finalize(count_state), finalize(sum_state), finalize(avg_state)::integer, finalize(min_state), finalize(max_state) FROM state ORDER BY g
SELECT sum(d)*2 FROM dummy
SELECT FINALIZE(COMBINE(SUM(d) EXPORT_STATE, SUM(d) EXPORT_STATE)) FROM dummy
SELECT g, sum(d)*2 combined_sum FROM dummy GROUP BY g ORDER BY g
select g, finalize(combine(sum(d) EXPORT_STATE, sum_state)) combined_sum from dummy join state using (g) group by g, sum_state ORDER BY g
CREATE TABLE state2 AS SELECT g, sum(d) EXPORT_STATE sum_state FROM dummy WHERE g < 5 GROUP BY g ORDER BY g
create table stddev_test(val integer, grp integer)
insert into stddev_test values (42, 1), (43, 1), (42, 2), (1000, 2), (NULL, 1), (NULL, 3)
SELECT stddev_samp(1)
SELECT var_samp(1)
select round(stddev_samp(val), 1) from stddev_test
select round(stddev_samp(val), 1) from stddev_test where val is not null
select grp, sum(val), round(stddev_samp(val), 1), min(val) from stddev_test group by grp order by grp
select grp, sum(val), round(stddev_samp(val), 1), min(val) from stddev_test where val is not null group by grp order by grp
select round(stddev_pop(val), 1) from stddev_test
select round(stddev_pop(val), 1) from stddev_test where val is not null
select grp, sum(val), round(stddev_pop(val), 1), min(val) from stddev_test group by grp order by grp
select grp, sum(val), round(stddev_pop(val), 1), min(val) from stddev_test where val is not null group by grp order by grp
SELECT STRING_AGG('a',',')
SELECT STRING_AGG('a',','), STRING_AGG(NULL,','), STRING_AGG('a', NULL), STRING_AGG(NULL,NULL)
CREATE TABLE strings(g INTEGER, x VARCHAR, y VARCHAR)
INSERT INTO strings VALUES (1,'a','/'), (1,'b','-'), (2,'i','/'), (2,NULL,'-'), (2,'j','+'), (3,'p','/'), (4,'x','/'), (4,'y','-'), (4,'z','+')
SELECT g, STRING_AGG(x,'|') FROM strings GROUP BY g ORDER BY g
SELECT STRING_AGG(x,',') FROM strings WHERE g > 100
SELECT GROUP_CONCAT('a', ',')
SELECT GROUP_CONCAT('a')
SELECT g, GROUP_CONCAT(x) FROM strings GROUP BY g ORDER BY g
SELECT STRING_AGG(x ORDER BY x ASC), STRING_AGG(x, '|' ORDER BY x ASC) FROM strings
SELECT STRING_AGG(x ORDER BY x DESC), STRING_AGG(x,'|' ORDER BY x DESC) FROM strings
SELECT g, STRING_AGG(x ORDER BY x ASC), STRING_AGG(x, '|' ORDER BY x ASC) FROM strings GROUP BY g ORDER BY 1
CREATE TABLE strings AS SELECT c::VARCHAR g, (c*10+e)::VARCHAR x FROM range(0, 100, 1) t1(c), range(0, 100, 1) t2(e)
SELECT COUNT(*) FROM (SELECT g, STRING_AGG(x,',') FROM strings GROUP BY g) t1
SELECT g, STRING_AGG(x ORDER BY x DESC) FROM strings GROUP BY g ORDER BY 1, 2
SELECT g, STRING_AGG(x,',' ORDER BY x DESC) FROM strings GROUP BY g ORDER BY 1, 2
INSERT INTO integers SELECT * FROM range(0, 1000, 1)
SELECT SUM(i) FROM integers
INSERT INTO integers SELECT * FROM range(0, -1000, -1)
SELECT SUM(1) FROM integers
SELECT SUM(-1) FROM integers
SELECT SUM(-1) FROM integers WHERE i=-1
SELECT SUM(-1) FROM integers WHERE i>10000
CREATE TABLE bigints(b BIGINT)
INSERT INTO bigints SELECT * FROM range(4611686018427387904, 4611686018427388904, 1)
SELECT SUM(b) FROM bigints
SELECT sum(n ORDER BY ABS(n))::BIGINT FROM doubles
SELECT weighted_avg(3, 3), weighted_avg(3, NULL), weighted_avg(NULL, 3), weighted_avg(NULL, NULL)
SELECT weighted_avg(3, 0), weighted_avg(3, 0.0), weighted_avg(0, 3), weighted_avg(0.0, 3)
SELECT wavg(3, 3)
CREATE TABLE students(name TEXT, grade INTEGER, etcs INTEGER)
INSERT INTO students VALUES ('Alice', 8, 6), ('Alice', 6, 2), ('Bob', 6, 3), ('Bob', 8, 3), ('Bob', 6, 6)
SELECT name, weighted_avg(grade, etcs) FROM students GROUP BY name ORDER BY name
INSERT INTO students VALUES ('Alice', 42, 0)
INSERT INTO students VALUES ('Alice', 42, NULL)
INSERT INTO students VALUES ('Alice', NULL, 42)
CREATE TABLE integers(g integer, i integer)
INSERT INTO integers values (0, 1), (0, 2), (1, 3), (1, NULL)
SELECT g, SUM(i) FROM integers GROUP BY ALL ORDER BY 1
SELECT SUM(i), g FROM integers GROUP BY ALL ORDER BY 2
SELECT g, SUM(i) FROM integers GROUP BY * ORDER BY 1
SELECT g, SUM(i) FROM integers GROUP BY 1 ORDER BY ALL
SELECT g, SUM(i) FROM integers GROUP BY 1 ORDER BY *
SELECT g, SUM(i), COUNT(*), COUNT(i), SUM(g) FROM integers GROUP BY ALL ORDER BY 1
SELECT i%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY 1
SELECT i%2, SUM(i), SUM(g) FROM integers GROUP BY 1 ORDER BY 1
SELECT i%2, SUM(i), SUM(g) FROM integers GROUP BY i ORDER BY 1 NULLS FIRST, 2
SELECT (g+i)%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY 1 NULLS FIRST
SELECT * FROM (SELECT 1) t0(c0) GROUP BY c0 HAVING c0>0
SELECT c0 FROM (SELECT 1) t0(c0) GROUP BY ALL HAVING c0>0
SELECT c0 FROM (SELECT 1, 1 UNION ALL SELECT 1, 2) t0(c0, c1) GROUP BY ALL ORDER BY c0
SELECT c0 FROM (SELECT 1, 1 UNION ALL SELECT 1, 2) t0(c0, c1) GROUP BY ALL HAVING c1>0 ORDER BY c0
SELECT SUM(i) FROM integers GROUP BY ALL
SELECT SUM(i) FROM integers GROUP BY ALL ORDER BY ALL
SELECT g, SUM(i) FROM integers GROUP BY ALL ORDER BY g
SELECT t_k0, COUNT(*) FROM t GROUP BY t_k0 ORDER BY 1
INSERT INTO t VALUES (-2147483648), (2147483647)
CREATE TABLE t(t_k0 BIGINT)
INSERT INTO t VALUES (-9223372036854775808), (9223372036854775807)
CREATE TABLE t(t_k0 HUGEINT)
INSERT INTO t VALUES (-170141183460469231731687303715884105728), (170141183460469231731687303715884105727)
CREATE TABLE t(t_k0 UTINYINT)
INSERT INTO t VALUES (0), (255)
CREATE TABLE t(t_k0 USMALLINT)
INSERT INTO t VALUES (0), (65535)
CREATE TABLE t(t_k0 UINTEGER)
INSERT INTO t VALUES (0), (4294967295)
SELECT SUM(a), COUNT(*), AVG(a) FROM test
SELECT COUNT(*) FROM test
SELECT SUM(a), COUNT(*) FROM test WHERE a = 11
SELECT SUM(a), SUM(b), SUM(a) + SUM (b) FROM test
SELECT SUM(a+2), SUM(a) + 2 * COUNT(*) FROM test
SELECT b, SUM(a), SUM(a+2), AVG(a) FROM test GROUP BY b ORDER BY b
SELECT b, SUM(a) FROM test GROUP BY b ORDER BY COUNT(a)
SELECT b, SUM(a) FROM test GROUP BY b ORDER BY COUNT(a) DESC
SELECT b, SUM(a), COUNT(*), SUM(a+2) FROM test GROUP BY b ORDER BY b
SELECT b % 2 AS f, SUM(a) FROM test GROUP BY f ORDER BY f
SELECT b, SUM(a), COUNT(*), SUM(a+2) FROM test WHERE a <= 12 GROUP BY b ORDER BY b
INSERT INTO test VALUES (12, 21), (12, 21), (12, 21)
SELECT i % 2 AS k, SUM(i) FROM integers WHERE i IS NOT NULL GROUP BY k HAVING k>0
SELECT i % 2 AS k, SUM(i) FROM integers WHERE i IS NOT NULL GROUP BY k HAVING i%2>0
SELECT i % 2 AS k, SUM(i) FROM integers WHERE i IS NOT NULL GROUP BY 1 HAVING i%2>0
SELECT i, i % 2 AS i, SUM(i) FROM integers GROUP BY i ORDER BY i, 3
SELECT i, i % 2 AS k, SUM(i) FROM integers GROUP BY i ORDER BY k, 3
SELECT i, i % 2 AS k, SUM(i) FROM integers GROUP BY i ORDER BY i
SELECT i, SUM(i) FROM integers GROUP BY i ORDER BY i
SELECT (10-i) AS k, SUM(i) FROM integers GROUP BY k ORDER BY FIRST(i)
CREATE TABLE tbl(i INT)
CREATE TABLE test (a VARCHAR, b INTEGER)
INSERT INTO test VALUES ('helloworld', 22), ('thisisalongstring', 22), ('helloworld', 21)
SELECT a, SUM(b) FROM test GROUP BY a ORDER BY a
CREATE TABLE integers(i INTEGER, j INTEGER, k INTEGER)
INSERT INTO integers VALUES (1, 1, 2), (1, 2, 2), (1, 1, 2), (2, 1, 2), (1, 2, 4), (1, 2, NULL)
SELECT i, j, SUM(k), COUNT(*), COUNT(k) FROM integers GROUP BY i, j ORDER BY 1, 2
SELECT k, SUM(v) FROM intlists GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM intlists GROUP BY k, c ORDER BY 2, 3
SELECT k, SUM(v) FROM strlists GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM strlists GROUP BY k, c ORDER BY 2, 3
SELECT k, SUM(v) FROM structs GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM structs GROUP BY k, c ORDER BY 2, 3
SELECT k, SUM(v) FROM struct_lint_lstr GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM struct_lint_lstr GROUP BY k, c ORDER BY 2, 3
SELECT k, SUM(v) FROM r2l3r4l5i4i2l3v GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM r2l3r4l5i4i2l3v GROUP BY k, c ORDER BY 2, 3
SELECT k, SUM(v) FROM longlists GROUP BY k ORDER BY 2
SELECT k, LEAST(v, 21) as c, SUM(v) FROM longlists GROUP BY k, c ORDER BY 2, 3
CREATE TABLE t0(c0 INTEGER)
SELECT LENGTH(NULL) FROM t0 GROUP BY NULL
SELECT c0, LENGTH(NULL) FROM t0 GROUP BY c0
SELECT NULL, LENGTH(NULL) FROM t0 GROUP BY NULL
INSERT INTO t0(c0) VALUES (2), (3)
SELECT c0, LENGTH(NULL) FROM t0 GROUP BY c0 ORDER BY c0
SELECT NULL FROM t0 GROUP BY NULL
SELECT UPPER(NULL) FROM t0 GROUP BY NULL
SELECT LOWER(NULL) FROM t0 GROUP BY NULL
SELECT LENGTH(NULL::VARCHAR) FROM t0 GROUP BY NULL::VARCHAR
SELECT LENGTH(NULL) + 0 FROM t0 GROUP BY NULL
INSERT INTO integers VALUES (3, 4), (NULL, 4), (2, 4)
SELECT i, SUM(j) FROM integers GROUP BY i ORDER BY i
INSERT INTO integers VALUES (2, 3), (4, 5), (2, NULL), (NULL, NULL)
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY j
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY i, j
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY i NULLS FIRST, j NULLS FIRST
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY i, j NULLS FIRST
CREATE TABLE distinct_on_test(key INTEGER, v1 VARCHAR, v2 INTEGER[], v3 INTEGER)
INSERT INTO distinct_on_test VALUES (1, 'hello', ARRAY[1], 42), (1, 'hello', ARRAY[1], 42), (1, 'hello', ARRAY[1], 43), (2, NULL, NULL, 0), (2, NULL, NULL, 1), (2, NULL, NULL, NULL), (3, 'thisisalongstring', NULL, 0), (3, 'thisisalongstringbutlonger', NULL, 1), (3, 'thisisalongstringbutevenlonger', ARRAY[1, 2, 3, 4, 5, 6, 7, 8, 9], 2)
SELECT DISTINCT ON (key) * FROM distinct_on_test ORDER BY key, v1, v2, v3
SELECT DISTINCT ON (key) * FROM distinct_on_test WHERE key <> 2 ORDER BY key, v1, v2, v3
SELECT DISTINCT ON (key) * FROM distinct_on_test ORDER BY key, v1 DESC NULLS FIRST, v2 DESC NULLS FIRST, v3 DESC NULLS FIRST
SELECT DISTINCT ON (key) * FROM distinct_on_test WHERE key <> 2 ORDER BY key, v1 DESC NULLS FIRST, v2 DESC NULLS FIRST, v3 DESC NULLS FIRST
INSERT INTO integers VALUES (2, 3, 5), (4, 5, 6), (2, 7, 6)
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY i, j DESC
SELECT DISTINCT ON (i) i, j FROM integers ORDER BY j DESC
SELECT i, j, (SELECT DISTINCT ON(i) j) AS k FROM integers ORDER BY i, j
SELECT i, j, (SELECT DISTINCT ON(i) j ORDER BY i, j DESC) AS k FROM integers ORDER BY i, j
SELECT i, j, (SELECT DISTINCT ON(i) j ORDER BY i, k) AS k FROM integers ORDER BY i, j
INSERT INTO integers VALUES (2, 3, 7), (4, 5, 11)
SELECT DISTINCT ON(i) i, j, k FROM integers ORDER BY i, j ASC, k ASC
SELECT DISTINCT ON(i) i, j, k FROM integers ORDER BY i, j ASC, k DESC
INSERT INTO integers VALUES (2, NULL, 27), (4, 88, NULL)
SELECT DISTINCT ON(i) i, j, k FROM integers ORDER BY i, j NULLS FIRST, k DESC NULLS LAST
SELECT DISTINCT ON(i) i, j, k FROM integers ORDER BY i, j NULLS FIRST, k NULLS FIRST
CREATE TABLE test ( col1 int, col2 int, col3 int )
INSERT INTO test VALUES (22, 6, 8), (28, 57, 45), (82, 44, 71)
SET threads = 4
SELECT * FROM ( SELECT DISTINCT col2 FROM test GROUP BY ROLLUP (col1, col2, col3) ) ORDER BY col2
CREATE TABLE T (t1 int, t2 int)
INSERT INTO t VALUES (1, 1), (1, 2)
SELECT DISTINCT t1 FROM T ORDER BY t1, t2
SELECT DISTINCT ON (1) t1, t2 FROM T ORDER BY t1, t2
SELECT DISTINCT t1 FROM T UNION SELECT DISTINCT t1 FROM T ORDER BY t1
SELECT DISTINCT t1 FROM T UNION ALL SELECT DISTINCT t1 FROM T ORDER BY t1
create table test (id int, provider int, record_key int, record_rank int, record_date int)
explain select record_key from ( select distinct on (id, provider) id, provider, record_key from test order by id, provider, record_rank desc, record_date )
explain select distinct on (id, provider) record_key from test order by id, provider, record_rank desc, record_date
create table foo (a int, b int)
insert into foo values (1, 1), (2, 1), (2, 2)
select * from (select distinct on (a) a, b from foo order by a, b desc) sub
select * from (select distinct on (a) a, b from foo order by a, b desc) sub where b <> 2
INSERT INTO test VALUES (11, 22), (13, 22), (11, 21), (11, 22)
SELECT DISTINCT a, b FROM test ORDER BY a, b
SELECT DISTINCT test.a, b FROM test ORDER BY a, b
SELECT DISTINCT a FROM test ORDER BY a
SELECT DISTINCT b FROM test ORDER BY b
SELECT DISTINCT a, SUM(B) FROM test GROUP BY a ORDER BY a
SELECT DISTINCT MAX(b) FROM test GROUP BY a
SELECT DISTINCT CASE WHEN a > 11 THEN 11 ELSE a END FROM test
CREATE TABLE issue3056 AS (SELECT * FROM (VALUES (['TGTA']), (['CGGT']), (['CCTC']), (['TCTA']), (['AGGG']), (NULL)) tbl(genes))
SELECT DISTINCT genes FROM issue3056
SELECT DISTINCT ON (i) i, j FROM integers WHERE i <> 2
SELECT DISTINCT ON (j) i, j FROM integers WHERE i <> 2
SELECT DISTINCT ON (j, i) i, j FROM integers WHERE i <> 2
SELECT DISTINCT ON (j + 1, i * 3) i, j FROM integers WHERE i <> 2
SELECT DISTINCT ON (1) i, j FROM integers ORDER BY i
SELECT DISTINCT ON (1) i, j FROM integers ORDER BY i LIMIT 1
SELECT DISTINCT ON (1) i, j FROM integers ORDER BY i LIMIT 1 OFFSET 1
SELECT DISTINCT ON (2) i, j FROM integers ORDER BY 2
SELECT DISTINCT ON (2) j, k FROM integers ORDER BY 2
SELECT DISTINCT ON (3) i, j, k FROM integers ORDER BY 2
SELECT DISTINCT ON (3) i, j, k FROM integers ORDER BY 3
SELECT DISTINCT ON (2) j, (SELECT i FROM integers WHERE i=2 LIMIT 1) FROM integers ORDER BY 2
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key')) *
CREATE TABLE grouped_table AS SELECT 1 id, 42 index1, 84 index2 UNION ALL SELECT 2, 42, 84 UNION ALL SELECT 3, 13, 14
SELECT DISTINCT ON (COLUMNS('index[0-9]')) * FROM grouped_table ORDER BY index1, index2, id
FROM (VALUES (1,1,2),(1,1,3),(2,1,4)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key'), v) key1, key2, v ORDER BY key1, v
FROM (VALUES (1,1,2),(1,1,3)) AS t(k1,k2,v) SELECT DISTINCT ON (COLUMNS('k')) * ORDER BY k1, k2
CREATE TABLE a AS SELECT * FROM (VALUES (1,1),(2,2)) AS ta(ak, av)
CREATE TABLE b AS SELECT * FROM (VALUES (1,9),(2,8)) AS tb(bk, bv)
SELECT DISTINCT ON (a.ak, b.bk) * FROM a JOIN b ON a.ak = b.bk ORDER BY a.ak, b.bk
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key'), key1) * ORDER BY key1, key2
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS(* EXCLUDE (v))) * ORDER BY key1, key2
SELECT DISTINCT ON (COLUMNS('[0-9]')) * FROM grouped_table ORDER BY index1, index2, id
FROM (VALUES (1,1,2),(1,1,3),(2,2,4)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key')) * ORDER BY key1, key2, v
SELECT DISTINCT i%2 FROM integers ORDER BY 1
SELECT DISTINCT i % 2 FROM integers WHERE i<3 ORDER BY i
SELECT DISTINCT ON (1) i % 2, i FROM integers WHERE i<3 ORDER BY i
SELECT DISTINCT integers.i FROM integers ORDER BY i DESC
SELECT DISTINCT i FROM integers ORDER BY integers.i DESC
SELECT DISTINCT integers.i FROM integers ORDER BY integers.i DESC
create table tbl as (select i%50 as i, i%100 as j from range(50000) tbl(i))
select count(distinct i) from tbl
select sum(distinct i), sum(i), sum(j) from tbl
select sum(i), sum(j), sum(distinct i) from tbl
select sum(i), sum(distinct i), sum(j) from tbl
select sum(distinct i), count(j), sum(distinct j) from tbl
select sum(j), sum(distinct i), count(j), sum(distinct j) from tbl
select sum(distinct i), count(j), sum(distinct j), sum(j) from tbl
select count(distinct i) FILTER (WHERE i >= 20) from tbl
select sum(distinct i), sum(i) FILTER (WHERE j < 20), sum(j) FILTER (WHERE i >= 20) from tbl
select sum(i), sum(j) FILTER (WHERE j == 0), sum(distinct i) FILTER (WHERE i == 0) from tbl
select sum(i) FILTER (WHERE j == 5), sum(distinct i), sum(j) FILTER (WHERE i == 5) from tbl
create table tbl as select i%50 as i from range(1000000) tbl(i)
select count(distinct i), min(distinct i), max(distinct i), sum(distinct i), product(distinct i) from tbl
create table students ( course VARCHAR, type VARCHAR, value BIGINT )
insert into students (course, type, value) values ('CS', 'Bachelor', 34), ('CS', 'Bachelor', 34), ('CS', 'PhD', 12), ('Math', 'Masters', 12), ('CS', NULL, 10), ('CS', NULL, 12), ('Math', NULL, 12), ('Math', NULL, NULL)
SELECT GROUPING(course), course, sum(distinct value), COUNT(*) FROM students GROUP BY course ORDER BY all
SELECT sum(distinct value), GROUPING_ID(course), course, COUNT(*) FROM students GROUP BY course ORDER BY all
SELECT GROUPING(course), GROUPING(type), course, type, sum(distinct value), COUNT(*), sum(distinct value), FROM students GROUP BY course, type ORDER BY all
SELECT GROUPING(course), GROUPING(type), avg(distinct value), course, type, COUNT(*), sum(distinct value), FROM students GROUP BY CUBE(course, type) ORDER BY all
SELECT sum(distinct value), GROUPING(course, type), course, type, COUNT(*), sum(distinct value), FROM students GROUP BY CUBE(course, type) ORDER BY all
SELECT GROUPING(course), GROUPING(type), sum(distinct value), GROUPING(course)+GROUPING(type), course, type, count(distinct value), COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY all
SELECT GROUPING(course, type, course, course, type, value, type, course), avg(distinct value), avg(value), avg(distinct value), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type, value) ORDER BY all
SELECT GROUPING(students.course), GROUPING(students.type), sum(distinct value), GROUPING(course)+GROUPING(type), course, avg(distinct value), type, COUNT(*) FROM students GROUP BY CUBE(course, type, value) ORDER BY all
SELECT GROUPING(course), GROUPING(type), avg(value), GROUPING(course)+GROUPING(type), avg(distinct value), course, type, COUNT(*) FROM students GROUP BY CUBE(students.course, students.type) ORDER BY all
SELECT GROUPING(course), GROUPING(value), course, sum(distinct value), COUNT(*) FROM students GROUP BY CUBE(course, value) HAVING GROUPING(course)=0 ORDER BY all
create table tbl as select i%50::BIGINT as i, i%5::BIGINT as j from range(1000000) tbl(i)
select count(distinct i), min(distinct i), max(distinct i), sum(distinct i), product(distinct i) from tbl group by j order by all
WITH evs AS ( SELECT * FROM (VALUES ('1','123','7'), ('1','456','7') ) AS t("id", "type", "value" ) ) SELECT "id" , COUNT(DISTINCT "value") FILTER (WHERE "type" = '456') AS type_456_count FROM evs GROUP BY "id"
insert into students (course, type, value) values ('CS', 'Bachelor', 20), ('CS', 'Bachelor', 10), ('CS', 'PhD', -20), ('Math', 'Masters', 10), ('CS', NULL, -15), ('CS', NULL, 10), ('Math', NULL, 15)
select course, type, count(*), sum(distinct value) from students group by course, type order by all
select course, type, count(*), sum(distinct value) from students group by (course, type) order by all
select course, count(*), sum(distinct value) from students group by (), course, () order by all
select count(*), course, type, sum(distinct value) from students group by grouping sets ((course), (type)) order by all
select sum(distinct value), count(*), course, avg(distinct value), type from students group by grouping sets (course), grouping sets(type) order by all
select sum(distinct value), count(*), count(distinct value), course, type from students group by course, grouping sets(type) order by all
select count(*), ARG_MIN(distinct value%5, value), course, sum(distinct value), type from students group by course, grouping sets(type, ()) order by all
select sum(distinct value), count(*), course, type from students group by grouping sets((course, type), (course)) order by all
select count(*), count(distinct value), count(value), course, sum(distinct value), type from students group by grouping sets (grouping sets(course), grouping sets(type)) order by all
select count(*), avg(distinct value) FILTER (where value < 5), avg(distinct value), course, avg(value), type from students group by grouping sets (grouping sets(course, ()), grouping sets(type)) order by all
select count(*), sum(distinct value), course, type from students group by grouping sets ((course), (), (type)) order by all
CREATE TABLE strings( g INTEGER, x VARCHAR, y VARCHAR )
SELECT g, STRING_AGG(DISTINCT y, ',' ORDER BY y DESC) FILTER (WHERE g < 4) FROM strings GROUP BY g ORDER BY 1
SELECT g, count(y), STRING_AGG(DISTINCT y, ',' ORDER BY y DESC) FILTER (WHERE g < 4), sum(1) FROM strings GROUP BY g ORDER BY 1
SET order_by_non_integer_literal=true
create table students (course VARCHAR, type VARCHAR, highest_grade INTEGER)
insert into students (course, type, highest_grade) values ('CS', 'Bachelor', 8), ('CS', 'Bachelor', 8), ('CS', 'PhD', 10), ('Math', 'Masters', NULL), ('CS', NULL, 7), ('CS', NULL, 7), ('Math', NULL, 8)
select course, count(*) from students group by cube (course) order by 1, 2
select course, type, count(*) from students group by cube (course, type) order by 1, 2, 3
select course, type, count(*) from students group by cube ((course, type)) order by 1, 2, 3
select course, type, count(*) from students group by cube (course, type, course) order by 1, 2, 3
select course, type, highest_grade, count(*) from students group by cube (course, type, highest_grade) order by 1, 2, 3, 4
select course, type, count(*) from students group by cube (course), cube (type) order by 1, 2, 3
select course as crs, type, count(*) from students group by cube (crs), (), type order by 1, 2, 3
select course as crs, type as tp, count(*) from students group by grouping sets (cube (crs)), (), tp order by 1, 2, 3
create table students (course VARCHAR, type VARCHAR)
insert into students (course, type) values ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters'), ('CS', NULL), ('CS', NULL), ('Math', NULL)
SELECT GROUPING(course), course, COUNT(*) FROM students GROUP BY course ORDER BY 1, 2, 3
SELECT GROUPING_ID(course), course, COUNT(*) FROM students GROUP BY course ORDER BY 1, 2, 3
SELECT GROUPING(course), GROUPING(type), course, type, COUNT(*) FROM students GROUP BY course, type ORDER BY 1, 2, 3, 4, 5
SELECT GROUPING(course), GROUPING(type), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY 1, 2, 3, 4, 5
SELECT GROUPING(course, type), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY 1, 2, 3, 4
SELECT GROUPING(course), GROUPING(type), GROUPING(course)+GROUPING(type), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY 1, 2, 3, 4, 5
SELECT GROUPING(course, type, course, course, type, type, course), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY 1, 2, 3, 4
SELECT GROUPING(students.course), GROUPING(students.type), GROUPING(course)+GROUPING(type), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) ORDER BY 1, 2, 3, 4, 5
SELECT GROUPING(course), GROUPING(type), GROUPING(course)+GROUPING(type), course, type, COUNT(*) FROM students GROUP BY CUBE(students.course, students.type) ORDER BY 1, 2, 3, 4, 5
SELECT GROUPING(course), GROUPING(type), course, type, COUNT(*) FROM students GROUP BY CUBE(course, type) HAVING GROUPING(course)=0 ORDER BY 1, 2, 3, 4, 5
select 1 from students group by ()
select count(*) from students group by ()
select course, type, count(*) from students group by course, type order by 1, 2, 3
select course, type, count(*) from students group by (course, type) order by 1, 2, 3
select course, count(*) from students group by (), course, () ORDER BY 1
select count(*), course, type from students group by grouping sets ((course), (type)) order by 1, 2, 3
select count(*), course, type from students group by grouping sets (course), grouping sets(type) order by 1, 2, 3
select count(*), course, type from students group by course, grouping sets(type) order by 1, 2, 3
select count(*), course, type from students group by course, grouping sets(type, ()) order by 1, 2, 3
select count(*), course, type from students group by grouping sets((course, type), (course)) order by 1, 2, 3
select count(*), course, type from students group by grouping sets (grouping sets(course), grouping sets(type)) order by 1, 2, 3
select count(*), course, type from students group by grouping sets (grouping sets(course, ()), grouping sets(type)) order by 1, 2, 3
SELECT course, COUNT(*) FROM students GROUP BY GROUPING SETS ((), (course)) HAVING course LIKE 'C%' ORDER BY 1, 2
SELECT course, COUNT(*) FROM students GROUP BY GROUPING SETS ((), (course)) HAVING course LIKE 'C%' OR course NOT LIKE 'C%' OR course IS NULL ORDER BY 1, 2
SELECT course, COUNT(*) FROM students GROUP BY GROUPING SETS ((), (course)) HAVING random()<1000 ORDER BY ALL
SELECT course, COUNT(*) FROM students GROUP BY GROUPING SETS ((), (course)) HAVING random()>1000
CREATE TABLE response( id BIGINT, response VARCHAR )
INSERT INTO response VALUES (1,'yes'), (1,'no'), (1,'yes'), (2,'no'), (2,'no')
CREATE TABLE user_pq( id BIGINT, "name" VARCHAR )
INSERT INTO user_pq VALUES (1,'alice'), (2,'bob')
SELECT id, response, COUNT(DISTINCT id) FROM user_pq JOIN response USING (id) GROUP BY CUBE (id, response) ORDER BY 1 NULLS LAST, 2 NULLS LAST, 3 NULLS LAST
select count(*) group by grouping sets ((), ())
select course, count(*) from students group by rollup (course) order by 1, 2
select course, type, count(*) from students group by rollup (course, type) order by 1, 2, 3
select course, type, count(*) from students group by rollup ((course, type)) order by 1, 2, 3
select course, type, count(*) from students group by rollup (course, type, course) order by 1, 2, 3
select course, type, count(*) from students group by grouping sets ((course, type), (course), ()) order by 1, 2, 3
select course, type, count(*) from students group by rollup (course), rollup (type) order by 1, 2, 3
select course as crs, type, count(*) from students group by rollup (crs), (), type order by 1, 2, 3
select course as crs, type as tp, count(*) from students group by grouping sets (rollup (crs)), (), tp order by 1, 2, 3
CREATE TABLE empsalary (depname varchar, empno bigint, salary int, enroll_date date)
INSERT INTO empsalary VALUES ('develop', 10, 5200, '2007-08-01'), ('sales', 1, 5000, '2006-10-01'), ('personnel', 5, 3500, '2007-12-10'), ('sales', 4, 4800, '2007-08-08'), ('personnel', 2, 3900, '2006-12-23'), ('develop', 7, 4200, '2008-01-01'), ('develop', 9, 4500, '2008-01-01'), ('sales', 3, 4800, '2007-08-01'), ('develop', 8, 6000, '2006-10-01'), ('develop', 11, 5200, '2007-08-15')
SELECT depname, empno, salary, sum(salary) OVER (PARTITION BY depname ORDER BY empno) FROM empsalary ORDER BY depname, empno
SELECT sum(salary) OVER (PARTITION BY depname ORDER BY salary) ss FROM empsalary ORDER BY depname, ss
SELECT row_number() OVER (PARTITION BY depname ORDER BY salary) rn FROM empsalary ORDER BY depname, rn
SELECT empno, first_value(empno) OVER (PARTITION BY depname ORDER BY empno) fv FROM empsalary ORDER BY 2 DESC, 1 ASC
SELECT depname, empno, last_value(empno) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, salary, dense_rank() OVER (PARTITION BY depname ORDER BY salary) FROM empsalary order by depname, salary
SELECT depname, salary, rank() OVER (PARTITION BY depname ORDER BY salary) FROM empsalary order by depname, salary
SELECT depname, min(salary) OVER (PARTITION BY depname ORDER BY salary, empno) m1, max(salary) OVER (PARTITION BY depname ORDER BY salary, empno) m2, AVG(salary) OVER (PARTITION BY depname ORDER BY salary, empno) m3 FROM empsalary ORDER BY depname, empno
SELECT depname, STDDEV_POP(salary) OVER (PARTITION BY depname ORDER BY salary, empno) s FROM empsalary ORDER BY depname, empno
SELECT depname, COVAR_POP(salary, empno) OVER (PARTITION BY depname ORDER BY salary, empno) c FROM empsalary ORDER BY depname, empno
CREATE TABLE tenk1 ( unique1 int4, unique2 int4, two int4, four int4, ten int4, twenty int4, hundred int4, thousand int4, twothousand int4, fivethous int4, tenthous int4, odd int4, even int4, stringu1 string, stringu2 string, string4 string )
SELECT sum(unique1) over (order by unique1 rows between 2 preceding and 2 following) su FROM tenk1 order by unique1
SELECT sum(unique1) over (order by unique1 rows between 2 preceding and 1 preceding) su FROM tenk1 order by unique1
SELECT sum(unique1) over (order by unique1 rows between 1 following and 3 following) su FROM tenk1 order by unique1
SELECT sum(unique1) over (order by unique1 rows between unbounded preceding and 1 following) su FROM tenk1 order by unique1
SELECT sum(unique1) over (order by unique1 rows between 5 following and 10 following) su FROM tenk1 order by unique1
create table issue1472 (permno real, date date, ret real)
insert into issue1472 values (10000.0, '1986-02-28'::date, -0.2571428716182709), (10000.0, '1986-03-31'::date, 0.36538460850715637), (10000.0, '1986-04-30'::date, -0.09859155118465424), (10000.0, '1986-05-30'::date, -0.22265625), (10000.0, '1986-06-30'::date, -0.005025125574320555)
select permno, sum(log(ret+1)) over (PARTITION BY permno ORDER BY date rows between 12 preceding and 2 preceding), ret from issue1472 ORDER BY permno, date
create table issue1697 as select mod(b, 100) as a, b from (select b from range(10000) tbl(b)) t
select avg(a) over ( order by b asc rows between mod(b * 1023, 11) preceding and 23 - mod(b * 1023, 11) following) from issue1697
call dbgen(sf=0.01)
SELECT l_orderkey, l_shipmode, l_linenumber, mode(l_linenumber ORDER BY l_linenumber DESC) over w AS l_mode, FROM lineitem WINDOW w AS (partition by l_shipmode) ORDER BY ALL LIMIT 10
SELECT i, (i * 29) % 11 AS outside, i // 2 AS inside, cume_dist(ORDER BY inside DESC) OVER w AS cd, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY outside ) ORDER BY inside DESC, i
SELECT i, i // 2 AS inside, cume_dist(ORDER BY i // 2) OVER w AS cd, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY i // 2 ROWS BETWEEN 3 PRECEDING AND 3 FOLLOWING ) ORDER BY 1
WITH t AS ( SELECT i, DENSE_RANK() OVER (ORDER BY i % 50) AS d FROM range(3000) tbl(i) ), w AS ( SELECT d, COUNT(*) as c FROM t GROUP BY ALL ) SELECT COUNT(*), MIN(d), MAX(d), MIN(c), MAX(c) FROM w
WITH t AS ( SELECT i, DENSE_RANK() OVER (PARTITION BY i // 3000 ORDER BY i % 50) AS d FROM range(9000) tbl(i) ), w AS ( SELECT d, COUNT(*) as c FROM t GROUP BY ALL ) SELECT COUNT(*), MIN(d), MAX(d), MIN(c), MAX(c) FROM w
CREATE TABLE issue9416(idx VARCHAR, source VARCHAR, project VARCHAR, specimen VARCHAR, sample_id VARCHAR)
CREATE TABLE t1 (id INTEGER, ch CHAR(1))
INSERT INTO t1 VALUES (1, 'A')
INSERT INTO t1 VALUES (2, 'B')
INSERT INTO t1 VALUES (NULL, 'B')
SELECT id, string_agg(id, ' ') OVER (PARTITION BY ch ORDER BY id ROWS BETWEEN 1 FOLLOWING AND 2 FOLLOWING) FROM t1 ORDER BY 1
SELECT id, bitstring_agg(id, 1, 3) OVER (PARTITION BY ch ORDER BY id ROWS BETWEEN 1 FOLLOWING AND 2 FOLLOWING) FROM t1 ORDER BY 1
select * from ( select lag(i, -1) over () as negative, lead(i, 1) over () as positive from generate_series(0, 10, 1) tbl(i) ) w where negative <> positive
SELECT depname, sum(sum(salary)) over (partition by depname order by salary) FROM empsalary group by depname, salary order by depname, salary
SELECT empno, sum(salary*2) OVER (PARTITION BY depname ORDER BY empno) FROM empsalary ORDER BY depname, empno
SELECT empno, 2*sum(salary) OVER (PARTITION BY depname ORDER BY empno) FROM empsalary ORDER BY depname, empno
SELECT depname, sum(salary)*100.0000/sum(sum(salary)) OVER (PARTITION BY depname ORDER BY salary) AS revenueratio FROM empsalary GROUP BY depname, salary ORDER BY depname, revenueratio
CREATE TABLE empty_unsorted(c0 VARCHAR)
SELECT * FROM empty_unsorted WHERE(NOT(false = ANY([]))) ORDER BY(~((SUM(true) OVER() - SUM(true) OVER())::INT)) ASC
with source as ( select i, i * 3 % 5 as permuted, if(permuted > 0, NULL, permuted) as missing from range(5) tbl(i) ) select i, permuted, fill(missing order by permuted) over (order by i) as filled from source qualify filled <> permuted
with source as ( select i, i * 5 % 11 as permuted, if(permuted < 6, NULL, permuted) as missing from range(11) tbl(i) ) select i, permuted, fill(missing order by permuted) over (partition by permuted // 5 order by i) as filled from source qualify filled is distinct from permuted order by i
with null_chunk as ( select 1 as p, s, NULL::DOUBLE as v, from range(2050) tbl(s) union all select 2 as p, s, s::DOUBLE as v, from range(16) tbl(s) ) select p, s, fill(v order by -s) over(partition by p order by s) as f, from null_chunk
with null_chunk as ( select 1 as p, s, NULL::DOUBLE as v, from range(8) tbl(s) union all select 2 as p, s, s::DOUBLE as v, from range(8) tbl(s) ) select p, s, fill(v order by s) over(partition by p order by s) as f, from null_chunk order by all
with source as ( select i, i * 5 % 11 as permuted, if(permuted = 2, NULL, permuted) as missing, if(permuted < 4, NULL, permuted) as unsorted, from range(11) tbl(i) ) select i, permuted, fill(missing order by unsorted) over (order by i) as filled from source qualify filled is distinct from permuted order by i
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 0, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls first) over (order by i) as filled from source qualify filled is distinct from permuted
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 4, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls last) over (order by i) as filled from source qualify filled is distinct from permuted
select i, permuted, fill(missing order by permuted asc nulls last) over (order by i) as filled from (values (0, 1, NULL), (1, NULL, 0) ) source(i, missing, permuted) order by i
select i, permuted, fill(missing order by permuted asc nulls first) over (order by i) as filled from (values (0, NULL, 2), (1, 0, NULL), (2, 1, 1), ) source(i, missing, permuted) order by i
CREATE TABLE issue2549 AS SELECT * FROM (VALUES (0, 1, 614), (1, 1, null), (2, 1, null), (3, 1, 639), (4, 1, 2027) ) tbl(id, user_id, order_id)
SELECT id, user_id, order_id, LAST_VALUE (order_id IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, FIRST_VALUE (order_id IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN 1 PRECEDING AND UNBOUNDED FOLLOWING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, NTH_VALUE (order_id, 2 IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LEAD(order_id, 1, -1 IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LAG(order_id, 1, -1 IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LAG(order_id, 0, -1 IGNORE NULLS) over ( PARTITION BY user_id ORDER BY id ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LAST_VALUE (order_id RESPECT NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, FIRST_VALUE (order_id RESPECT NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, NTH_VALUE (order_id, 2 RESPECT NULLS) over ( PARTITION BY user_id ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LEAD(order_id, 1, -1 RESPECT NULLS) over ( PARTITION BY user_id ORDER BY id ) AS last_order_id FROM issue2549 ORDER BY ALL
SELECT id, user_id, order_id, LAG(order_id, 1, -1 RESPECT NULLS) over ( PARTITION BY user_id ORDER BY id ) AS last_order_id FROM issue2549 ORDER BY ALL
select LIST(salary ORDER BY enroll_date, salary) OVER (PARTITION BY depname) FROM empsalary ORDER BY ALL DESC
SELECT sum(i) OVER (ORDER BY i GROUPS 1 PRECEDING) FROM generate_series(1,10) AS _(i) ORDER BY i
select c1, lead(c1, 2) over (order by c0 rows between 2 preceding and 4 preceding) as b from (values (1, 2), (2, 3), (3, 4), (4, 5) ) a(c0, c1)
create table win(id int, v int, t int, f float, s varchar)
insert into win values (1, 1, 2, 0.54, 'h'), (1, 1, 1, 0.21, 'e'), (1, 2, 3, 0.001, 'l'), (2, 10, 4, 0.04, 'l'), (2, 11, -1, 10.45, 'o'), (3, -1, 0, 13.32, ','), (3, 5, -2, 9.87, 'wor'), (3, null, 10, 6.56, 'ld')
select id, v, t, lag(v, 2, NULL) over (partition by id order by t asc) from win order by id, t
CREATE TABLE issue14398 (date DATE, "group" INT, count INT, status STRING)
CREATE TABLE issue17266(c1 INT, c2 SMALLINT, c3 BITSTRING)
INSERT INTO issue17266 VALUES (0, null, null), (1, 32767, '101'), (2, -32767, '101'), (3, 0, '000'), (4, null, null)
SELECT c1, c3, c2, LAG(c3, c2, BITSTRING'010101010') OVER (PARTITION BY c1 ORDER BY c3) FROM issue17266 ORDER BY c1
SELECT c1, c3, c2, LEAD(c3, c2, BITSTRING'010101010') OVER (PARTITION BY c1 ORDER BY c3) FROM issue17266 ORDER BY c1
SELECT i, (i * 29) % 11 AS outside, i // 2 AS inside, lead(i, 1, NULL ORDER BY inside DESC, i) OVER w, lag(i, 1, NULL ORDER BY inside DESC, i) OVER w, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY outside ) ORDER BY inside DESC, i
SELECT i, i // 2 AS inside, lead(i, 1, NULL ORDER BY i // 2, i) OVER w AS next, lag(i, 1, NULL ORDER BY i // 2, i) OVER w AS prev, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY i // 2 ROWS BETWEEN 3 PRECEDING AND 3 FOLLOWING ) ORDER BY i
INSERT INTO issue17266 VALUES (0, null, null), (1, 32767, '101'), (2, -32767, '101'), (3, 0, '000'), (4, 1, '010'), (5, 0, '110'), (6, null, null)
SELECT c1, c3, c2, LAG(c3, c2 ORDER BY c1, BITSTRING'010101010') OVER (PARTITION BY c1 ORDER BY c3) FROM issue17266 ORDER BY c1
SELECT c1, c3, c2, LEAD(c3, c2 ORDER BY c1, BITSTRING'010101010') OVER (PARTITION BY c1 ORDER BY c3) FROM issue17266 ORDER BY c1
CREATE TABLE list_extract_test(i INTEGER, g INTEGER)
INSERT INTO list_extract_test VALUES (1, 1), (2, 1), (3, 2), (NULL, 3), (42, 3)
CREATE VIEW list_window AS SELECT g, LIST(i) OVER (PARTITION BY g ORDER BY i ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) as l FROM list_extract_test
SELECT * FROM list_window ORDER BY g
SELECT FIRST(LIST_EXTRACT(l, 1)) FROM list_window GROUP BY g ORDER BY g
SELECT FIRST(LIST_EXTRACT(l, 2)) FROM list_window GROUP BY g ORDER BY g
SELECT FIRST(LIST_EXTRACT(l, 3)) FROM list_window GROUP BY g ORDER BY g
create table list_combine_test as select range%3 j, range::varchar AS s, case when range%3=0 then '-' else '|' end sep from range(1, 65)
select j, s, list(s) over (partition by j order by s) from list_combine_test order by j, s
create table mads as select range r from range(20) union all values (NULL), (NULL), (NULL)
SELECT r % 2 as p, r, r/3.0, mad(r/3.0) over (partition by r % 2 order by r) FROM mads ORDER BY 1, 2
SELECT r, r/3.0, mad(r/3.0) over (order by r rows between 1 preceding and 1 following) FROM mads ORDER BY 1, 2, 3
SELECT r, r/3.0, mad(r/3.0) over (order by r rows between 1 preceding and 3 following) FROM mads ORDER BY 1, 2, 3
SELECT r % 3 as p, r, n, mad(n) over (partition by r % 3 order by r) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM mads) nulls ORDER BY 1, 2
SELECT r, n, mad(n) over (order by r rows between 1 preceding and 1 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM mads) nulls ORDER BY 1
SELECT r, n, mad(n) over (order by r rows between 1 preceding and 3 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM mads) nulls ORDER BY 1
SELECT r, n, mad(n) over (order by r rows between unbounded preceding and unbounded following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM mads) nulls ORDER BY 1
CREATE TABLE coverage AS SELECT * FROM (VALUES (1), (2), (3), (1) ) tbl(r)
SELECT r, mad(r) OVER (ORDER BY r ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM coverage ORDER BY 1
create table modes as select range r from range(10) union all values (NULL), (NULL), (NULL)
SELECT r % 2, r, r//3, mode(r//3) over (partition by r % 2 order by r) FROM modes ORDER BY 1, 2
SELECT r, r//3, mode(r//3) over (order by r rows between 1 preceding and 1 following) FROM modes ORDER BY ALL
SELECT r, r//3, mode(r//3) over (order by r rows between 1 preceding and 3 following) FROM modes ORDER BY 1, 2
SELECT r, r // 3, n, mode(n) over (partition by r % 3 order by r) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM modes) nulls ORDER BY 1
SELECT r, n, mode(n) over (order by r rows between 1 preceding and 1 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM modes) nulls ORDER BY ALL
SELECT r, n, mode(n) over (order by r rows between 1 preceding and 3 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM modes) nulls ORDER BY 1
SELECT r, n, mode(n) over (order by r rows between unbounded preceding and unbounded following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM modes) nulls ORDER BY 1
WITH t(r) AS (VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9), (NULL), (NULL), (NULL)) SELECT r, r//3, mode(r//3) over (order by r rows between 1 preceding and 1 following) FROM t ORDER BY ALL
PRAGMA debug_window_mode=separate
CREATE TABLE filtering AS SELECT x ,round(x * 0.333,0) % 3 AS y ,round(x * 0.333,0) % 3 AS z FROM generate_series(0,10) tbl(x)
SELECT x ,y ,z ,avg(x) OVER (PARTITION BY y) AS plain_window ,avg(x) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,avg(x) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM filtering ORDER BY y, x
SELECT x ,y ,z ,count(*) OVER (PARTITION BY y) AS plain_window ,count(*) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,count(*) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM filtering ORDER BY y, x
SELECT x ,y ,z ,median(x) OVER (PARTITION BY y) AS plain_window ,median(x) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,median(x) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM filtering ORDER BY y, x
SELECT x, count(x) FILTER (WHERE x % 2 = 0) OVER (ORDER BY x ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) FROM generate_series(0,10) tbl(x)
CREATE TABLE figure1 AS SELECT * FROM VALUES (1, 'a'), (2, 'b'), (3, 'b'), (4, 'c'), (5, 'c'), (6, 'b'), (7, 'c'), (8, 'a') v(i, s)
SELECT i , s , COUNT(DISTINCT s) OVER( ORDER BY i ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) AS c FROM figure1 ORDER BY i
SELECT i , s , COUNT(DISTINCT s) OVER( ORDER BY i ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING EXCLUDE TIES) AS c FROM figure1 ORDER BY i
SELECT i // 10 AS p, i, ANY_VALUE(i ORDER BY i DESC) OVER( PARTITION BY i // 10 ORDER BY i ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING ) AS c FROM range(20) tbl(i) ORDER BY ALL
SELECT i // 10 AS p, i, LIST(i ORDER BY i DESC) OVER( PARTITION BY i // 10 ORDER BY i ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING ) AS c FROM range(20) tbl(i) ORDER BY ALL
SELECT i // 10 AS p, i, LIST(DISTINCT i // 2 ORDER BY i DESC) OVER( PARTITION BY i // 10 ORDER BY i ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING ) AS c FROM range(20) tbl(i) ORDER BY ALL
CREATE OR REPLACE TABLE issue10855(i INTEGER, v FLOAT)
INSERT INTO issue10855 VALUES (0, 1), (1, 2), (2, 3),
create table tenk1d(ten int4, four int4)
insert into tenk1d values (0,0), (1,1), (3,3), (2,2), (4,2), (9,1), (4,0), (7,3), (0,2), (2,0), (5,1), (1,3), (3,1), (6,0), (8,0), (9,3), (8,2), (6,2), (7,1), (5,3)
SELECT four, ten, sum(ten) over (partition by four order by ten) st, last_value(ten) over (partition by four order by ten) lt FROM tenk1d ORDER BY four, ten
SELECT four, ten, sum(ten) over (partition by four order by ten range between unbounded preceding and current row) st, last_value(ten) over (partition by four order by ten range between unbounded preceding and current row) lt FROM tenk1d order by four, ten
SELECT four, ten, sum(ten) over (partition by four order by ten range between unbounded preceding and unbounded following) st, last_value(ten) over (partition by four order by ten range between unbounded preceding and unbounded following) lt FROM tenk1d order by four, ten
SELECT four, ten//4 as two, sum(ten//4) over (partition by four order by ten//4 range between unbounded preceding and current row) st, last_value(ten//4) over (partition by four order by ten//4 range between unbounded preceding and current row) lt FROM tenk1d order by four, ten//4
SELECT four, ten//4 as two, sum(ten//4) OVER w st, last_value(ten//4) OVER w lt FROM tenk1d WINDOW w AS (partition by four order by ten//4 range between unbounded preceding and current row) order by four, ten//4
SELECT depname, empno, nth_value(empno, 2) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, empno, nth_value(empno, NULL) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, empno, nth_value(NULL, 2) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, empno, nth_value(empno, case empno % 3 when 1 then 2 else NULL end) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
CREATE VIEW empno_nulls AS SELECT depname, case empno % 2 when 1 then empno else NULL end as empno, salary, enroll_date FROM empsalary
SELECT depname, empno, nth_value(empno, 2) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empno_nulls ORDER BY 1, 2, 3
SELECT depname, empno, 1 + empno %3 as offset, nth_value(empno, 1 + empno %3) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, empno, empno %3 as offset, nth_value(empno, empno %3) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
SELECT depname, empno, nth_value(-1, 2) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary ORDER BY 1, 2
CREATE TABLE Scoreboard(TeamName VARCHAR, Player VARCHAR, Score INTEGER)
INSERT INTO Scoreboard VALUES ('Mongrels', 'Apu', 350)
INSERT INTO Scoreboard VALUES ('Mongrels', 'Ned', 666)
INSERT INTO Scoreboard VALUES ('Mongrels', 'Meg', 1030)
INSERT INTO Scoreboard VALUES ('Mongrels', 'Burns', 1270)
INSERT INTO Scoreboard VALUES ('Simpsons', 'Homer', 1)
INSERT INTO Scoreboard VALUES ('Simpsons', 'Lisa', 710)
INSERT INTO Scoreboard VALUES ('Simpsons', 'Marge', 990)
INSERT INTO Scoreboard VALUES ('Simpsons', 'Bart', 2010)
SELECT TeamName, Player, Score, NTILE(2) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(2) OVER (ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY Score
SELECT TeamName, Player, Score, NTILE(1000) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT rank() OVER (ORDER BY COLUMNS('^(.*)_score$') DESC) AS '\1_rank' FROM ( SELECT range AS math_score, 100-range as reading_score from range(65, 100, 5) )
create table quantiles as select range r from range(10) union all values (NULL), (NULL), (NULL)
SELECT r % 2, r, median(r) over (partition by r % 2 order by r) FROM quantiles ORDER BY 1, 2
SELECT r, median(r) over (order by r rows between 1 preceding and 1 following) FROM quantiles ORDER BY 1, 2
SELECT r, median(r) over (order by r rows between 1 preceding and 3 following) FROM quantiles ORDER BY 1, 2
SELECT r, quantile(r, 0.5) over (order by r rows between 1 preceding and 3 following) FROM quantiles ORDER BY 1, 2
SELECT r % 2, r, median(r::VARCHAR) over (partition by r % 2 order by r) FROM quantiles ORDER BY 1, 2
SELECT r, median(r::VARCHAR) over (order by r rows between 1 preceding and 1 following) FROM quantiles ORDER BY 1, 2
SELECT r, quantile(r::VARCHAR, 0.5) over (order by r rows between 1 preceding and 3 following) FROM quantiles ORDER BY 1, 2
SELECT r, median('prefix-' || r::VARCHAR || '-suffix') over (order by r rows between 1 preceding and 1 following) FROM quantiles ORDER BY 1, 2
SELECT r % 3, r, n, median(n) over (partition by r % 3 order by r) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM quantiles) nulls ORDER BY 1, 2
SELECT r, n, median(n) over (order by r rows between 1 preceding and 1 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM quantiles) nulls ORDER BY 1
SELECT r, n, median(n) over (order by r rows between 1 preceding and 3 following) FROM (SELECT r, CASE r % 2 WHEN 0 THEN r ELSE NULL END AS n FROM quantiles) nulls ORDER BY 1
CREATE TABLE rides ( id INTEGER, requested_date DATE, city VARCHAR, wait_time INTEGER )
SELECT "id", "requested_date", "city", "wait_time", min("wait_time") OVER win_3d FROM rides WINDOW win_3d AS ( PARTITION BY "city" ORDER BY requested_date ASC RANGE BETWEEN INTERVAL 3 DAYS PRECEDING AND INTERVAL 1 DAYS PRECEDING) ORDER BY "requested_date", "city", "id"
WITH t AS ( SELECT i, RANK() OVER (ORDER BY i % 50) AS d FROM range(3000) tbl(i) ), w AS ( SELECT d, COUNT(*) as c FROM t GROUP BY ALL ) SELECT COUNT(*), MIN(d), MAX(d), MIN(c), MAX(c) FROM w
WITH t AS ( SELECT i, RANK() OVER (PARTITION BY i // 3000 ORDER BY i % 50) AS d FROM range(9000) tbl(i) ), w AS ( SELECT d, COUNT(*) as c FROM t GROUP BY ALL ) SELECT COUNT(*), MIN(d), MAX(d), MIN(c), MAX(c) FROM w
SELECT *, RANK() OVER (ORDER BY x NULLS FIRST) rank_nulls_first, RANK() OVER (ORDER BY x NULLS LAST) rank_nulls_last, FROM VALUES (1), (1), (1), (NULL) as issue8315(x) ORDER BY x
SELECT i, (i * 29) % 11 AS outside, rank(ORDER BY (i // 2) DESC) OVER w, percent_rank(ORDER BY (i // 2) DESC) OVER w, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY (i * 29) % 11 ) ORDER BY 2
WITH ranked AS ( SELECT i, i // 100 AS p, i % 50 AS o, 100 - 2 * (i % 50) - 1 AS expected, rank(ORDER BY i % 50 DESC) OVER w AS actual, FROM range(100_000) tbl(i) WINDOW w AS ( PARTITION BY i // 100 ORDER BY i ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING ) ) SELECT * FROM ranked WHERE expected <> actual ORDER BY p, o DESC LIMIT 20
SELECT i, i // 2 AS outside, rank(ORDER BY i // 2) OVER w, percent_rank(ORDER BY i // 2) OVER w, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY i // 2 ROWS BETWEEN 3 PRECEDING AND 3 FOLLOWING ) ORDER BY 1
SELECT i, (i * 29) % 11 AS outside, row_number(ORDER BY (i // 2) DESC) OVER w, ntile(4 ORDER BY (i // 2) DESC) OVER w, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY (i * 29) % 11 ) ORDER BY 2
SELECT row_number() OVER ()
SELECT avg(42) OVER ()
create table partsupp as select uuid()::varchar as c5 from range(8000)
SELECT (ntile(5002) OVER (ROWS BETWEEN CURRENT ROW AND CURRENT ROW) >= 0), c5 FROM partsupp
EXPLAIN SELECT i, LAG(i, 1) OVER() AS i1 FROM range(10) tbl(i)
SELECT i, LAG(i, 1) OVER() AS i1 FROM range(10) tbl(i)
EXPLAIN SELECT i, LAG(i, -1) OVER() AS i1 FROM range(10) tbl(i)
SELECT i, LAG(i, -1) OVER() AS i1 FROM range(10) tbl(i)
EXPLAIN SELECT i, LEAD(i, -1) OVER() AS i1 FROM range(10) tbl(i)
SELECT i, LEAD(i, -1) OVER() AS i1 FROM range(10) tbl(i)
EXPLAIN SELECT i, LEAD(i, 1) OVER() AS i1 FROM range(10) tbl(i)
SELECT i, LEAD(i, 1) OVER() AS i1 FROM range(10) tbl(i)
EXPLAIN SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
EXPLAIN SELECT i, LAG(i, 1, 50) OVER() AS i1 FROM range(10) tbl(i)
SELECT i, LAG(i, 1, 50) OVER() AS i1 FROM range(10) tbl(i)
create table integers (i int, j int)
insert into integers values (2, 2), (2, 1), (1, 2), (1, NULL)
EXPLAIN SELECT i, COUNT(*) OVER() FROM integers
EXPLAIN SELECT i, SUM(i) OVER() FROM integers
EXPLAIN SELECT j, COUNT(j) FILTER(WHERE i = 2) OVER(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM integers
EXPLAIN SELECT j, COUNT(*) FILTER(WHERE i = 2) OVER(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM integers
EXPLAIN SELECT j, SUM(j) FILTER(WHERE i = 2) OVER(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM integers
explain select row_number() over (), i, j from integers
select row_number() over (), i, j from integers
explain select rank() over (), i, j from integers
select rank() over (), i, j from integers
explain select dense_rank() over (), i, j from integers
explain SELECT i, SUM(DISTINCT i % 3) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM range(10) tbl(i)
SELECT i, SUM(DISTINCT i % 3) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM range(10) tbl(i)
EXPLAIN SELECT LIST(DISTINCT col0) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS result FROM (VALUES ({'key': 'A'}), ({'key': 'B'}), ({'key': 'A'}))
SELECT LIST(DISTINCT col0) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS result FROM (VALUES ({'key': 'A'}), ({'key': 'B'}), ({'key': 'A'}))
explain SELECT i, SUM(DISTINCT i % 5) FILTER (i % 3 = 0) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM range(20) tbl(i)
SELECT i, SUM(DISTINCT i % 5) FILTER (i % 3 = 0) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM range(20) tbl(i)
BEGIN TRANSACTION
create table wintest( item integer, return_ratio numeric, currency_ratio numeric)
SELECT item, rank() OVER (ORDER BY return_ratio) AS return_rank, rank() OVER (ORDER BY currency_ratio) AS currency_rank FROM wintest order by item
ROLLBACK
SELECT i, (i * 29) % 11 AS outside, first_value(i ORDER BY i DESC) OVER w, last_value(i ORDER BY i DESC) OVER w, nth_value(i, 2 ORDER BY i DESC) OVER w, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY (i * 29) % 11 ROWS BETWEEN 3 PRECEDING AND 3 FOLLOWING ) ORDER BY 2
with IDS as ( select * as idx from generate_series(1,4) ),DATA as ( select *, (case when idx != 3 then idx * 1.0 else NULL end) as value from IDS ) SELECT last(value ORDER BY idx IGNORE NULLS) OVER (ORDER BY idx ROWS BETWEEN UNBOUNDED PRECEDING AND 0 FOLLOWING) FROM DATA
CREATE TABLE all_nulls (order_col int,value_col float,partition_col int)
INSERT INTO all_nulls VALUES (2,NULL,10)
INSERT INTO all_nulls VALUES (1,NULL,10)
SELECT first_value(value_col ORDER BY order_col IGNORE NULLS) over (PARTITION BY partition_col) FROM all_nulls
SELECT last_value(value_col ORDER BY order_col IGNORE NULLS) over (PARTITION BY partition_col) FROM all_nulls
SELECT nth_value(value_col, 1 ORDER BY order_col IGNORE NULLS) over (PARTITION BY partition_col) FROM all_nulls
WITH t(a,b) AS ( VALUES (0, 'a'), (0, 'b'), (1, 'c'), (2, 'd'), (2, 'e'), (2, 'f') ), framed AS ( SELECT a, b, nth_value(b, 1) OVER w AS b1, nth_value(b, 1 ORDER BY b) OVER w AS b1_ordered, FROM t WINDOW w AS (ORDER BY a RANGE BETWEEN CURRENT ROW AND CURRENT ROW) ) FROM framed where a = 1
SELECT list(random()) OVER (ORDER BY id), max(random()) OVER (ORDER BY id) FROM range(3) t(id)
SELECT last_value(i ORDER BY i DESC) OVER w AS crash FROM range(5_000) tbl(i) WINDOW w AS (ORDER BY i ASC)
SELECT rank(ORDER BY i DESC) OVER w AS crash FROM range(5_000) tbl(i) WINDOW w AS (ORDER BY i ASC)
SELECT cume_dist(ORDER BY i DESC) OVER w AS crash FROM range(5_000) tbl(i) WINDOW w AS (ORDER BY i ASC)
SELECT MIN(i) OVER (PARTITION BY i ORDER BY i) FROM integers
select i, lag(i) over named_window from (values (1), (2), (3)) as t (i) window named_window as (order by i)
with subquery as (select i, lag(i) over named_window from (values (1), (2), (3)) as t (i) window named_window as (order by i)) select * from subquery
select * from (select i, lag(i) over named_window from (values (1), (2), (3)) as t (i) window named_window as (order by i)) t1
CREATE VIEW v1 AS select i, lag(i) over named_window from (values (1), (2), (3)) as t (i) window named_window as (order by i)
select * from v1
SELECT * FROM (SELECT i, lag(i) OVER named_window FROM ( VALUES (1), (2), (3)) AS t (i) window named_window AS ( ORDER BY i)) t1, (SELECT i, lag(i) OVER named_window FROM ( VALUES (1), (2), (3)) AS t (i) window named_window AS ( ORDER BY i)) t2 ORDER BY 1, 2, 3, 4
CREATE TABLE a (id INT)
WITH cte_a AS ( SELECT * FROM a WINDOW my_window AS () ), cte_b AS ( SELECT * FROM a WINDOW my_window AS () ) SELECT * FROM cte_a CROSS JOIN cte_b
create table a as select range%2==0 j, range::integer AS i from range(1, 5, 1)
select j, i, sum(i) over () from a order by 1,2
select j, i, sum(i) over (partition by j) from a order by 1,2
select j, i, sum(i) over (partition by j order by i) from a order by 1,2
drop table a
create table a as select range%2 j, range%3==0 AS i from range(1, 5, 1)
select j, i, bool_and(i) over (), bool_or(i) over () from a order by 1,2
select j, i, bool_and(i) over (partition by j), bool_or(i) over (partition by j) from a order by 1,2
select j, i, bool_and(not i) over (partition by j order by i), bool_and(i) over (partition by j order by i), bool_or(i) over (partition by j order by i) from a order by 1,2
create table integers as select range i from range(0,16)
select max(base), max(referenced), sum(refined), sum(unrefined) from ( select row_number() over w AS base, row_number() over (w) as referenced, sum(i % 4) over (w rows between 1 preceding and 1 following) AS refined, sum(i % 4) over (rows between 1 preceding and 1 following) AS unrefined from integers WINDOW w AS (partition by i // 4 order by i % 4) ) q
select x, y, count(*) over (partition by y order by x), count(*) over (w order by x) from (values (1, 1), (2, 1), (3, 2), (4, 2)) as t (x, y) window w as (partition by y) order by x
SELECT sum(i) over cumulativeSum FROM integers WINDOW cumulativeSum AS ()
SELECT part, id, sum(val) OVER(PARTITION BY part ORDER BY id), lead(val) OVER(PARTITION BY part ORDER BY id) FROM (SELECT range AS id, range % 5 AS part, range AS val FROM range(13)) t ORDER BY ALL
SELECT part, id, list_sort(list(val) OVER(PARTITION BY part)) FROM (SELECT range AS id, range % 5 AS part, range AS val FROM range(13)) t ORDER BY ALL
SELECT part, min(const) AS lo, max(const) AS hi FROM ( SELECT part, sum(val) OVER(PARTITION BY part) AS const FROM ( SELECT part, val FROM ( (SELECT range as part, random() AS val FROM range(10)) r CROSS JOIN range(3000) ) p ) t ) w GROUP BY ALL HAVING lo <> hi ORDER BY ALL
CREATE TABLE issue7353 ( Season VARCHAR, Medal VARCHAR, Sex VARCHAR, Ct INT, Depth INT )
PRAGMA default_null_order='NULLS LAST'
SELECT *, max(Ct) FILTER (WHERE Depth=1) OVER (PARTITION BY Season) as value_depth1 from issue7353 order by all
SELECT i // 10 AS p, i, STRING_AGG(i, ',' ORDER BY i DESC) OVER(PARTITION BY p) AS c FROM range(20) tbl(i) ORDER BY ALL
pragma threads=2
with repro2 AS ( SELECT range // 59 AS id, random() AS value FROM range(1475) ), X AS ( SELECT list(value) OVER ( PARTITION BY id ORDER BY value ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING ) AS values FROM repro2 ) select count(*) from X where values[1] != list_aggregate(values, 'min')
WITH cte AS ( SELECT 1 AS ext UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 ) SELECT CASE WHEN ext % 2 = 0 THEN 'even' ELSE 'odd' END AS pred, TRUE AS eof, CAST(NULL AS BOOLEAN) AS converter, STRING_AGG(cte.ext, 'abc') OVER () AS str_agg FROM cte
CREATE TABLE eventlog AS SELECT ts, CHR((RANDOM() * 3 + 65)::INTEGER) AS activity_name, (RANDOM() * 100)::INTEGER AS case_id FROM generate_series('2023-01-01'::TIMESTAMP, '2023-02-01'::TIMESTAMP, INTERVAL 1 HOUR) tbl(ts)
EXPLAIN FROM cse
CREATE VIEW noncse AS SELECT quantile(x, 0.3) over() as q3, quantile(x, 0.7) over() as q7 FROM generate_series(1, 10) as tbl(x)
EXPLAIN FROM noncse
CREATE TABLE dbplyr_052 (x INTEGER, g DOUBLE, w int)
INSERT INTO dbplyr_052 VALUES (1,1, 42),(2,1, 42),(3,1, 42),(2,2, 42),(3,2, 42),(4,2, 42)
SELECT x, g FROM (SELECT x, g, SUM(x) OVER (PARTITION BY g ORDER BY x ROWS UNBOUNDED PRECEDING) AS zzz67 FROM (SELECT x, g FROM dbplyr_052 ORDER BY x) dbplyr_053) dbplyr_054 WHERE (zzz67 > 3.0)
SELECT x, g FROM (SELECT x, g, SUM(x) OVER (PARTITION BY g ORDER BY x ROWS UNBOUNDED PRECEDING) AS zzz67 FROM (SELECT x, g FROM dbplyr_052 ORDER BY w) dbplyr_053) dbplyr_054 WHERE (zzz67 > 3.0)
SELECT x, g FROM (SELECT x, g, SUM(x) OVER (PARTITION BY g ORDER BY x ROWS UNBOUNDED PRECEDING) AS zzz67 FROM (SELECT * FROM dbplyr_052 ORDER BY x) dbplyr_053) dbplyr_054 WHERE (zzz67 > 3.0)
SELECT COUNT(DISTINCT 42) OVER ()
WITH t AS ( SELECT col0 AS a, col1 AS b FROM (VALUES (1,2), (1,1), (1,2), (2,1), (2,1), (2,2), (2,3), (2,4) ) v) SELECT *, COUNT(b) OVER(PARTITION BY a), COUNT(DISTINCT b) OVER(PARTITION BY a) FROM t ORDER BY 1, 2
WITH uncascaded AS ( SELECT i, i % 29 AS v FROM range(1000) tbl(i) ) SELECT i , v , COUNT(DISTINCT v) OVER (ORDER BY i ROWS BETWEEN 25 PRECEDING AND 25 FOLLOWING) AS w FROM uncascaded ORDER BY i
WITH cascaded AS ( SELECT i, i % 29 AS v FROM range(10000) tbl(i) ) SELECT i , v , COUNT(DISTINCT v) OVER (ORDER BY i ROWS BETWEEN 25 PRECEDING AND 25 FOLLOWING) AS w FROM cascaded ORDER BY i
SELECT i , s , i // 2 AS o , COUNT(DISTINCT s) OVER( ORDER BY i // 2 ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING EXCLUDE TIES ) AS c FROM figure1 ORDER BY i
INSERT INTO figure1 VALUES (9, NULL), (NULL, 'b'), (NULL, NULL),
SELECT i , s , COUNT(DISTINCT s) OVER( ORDER BY i, s NULLS LAST ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) AS c FROM figure1 ORDER BY i, s NULLS LAST
CREATE TABLE nested AS SELECT i, s, {"m": i % 2, "s": s} AS n, [(i % 2)::VARCHAR, s] AS l, i * i AS r FROM figure1
SELECT i , n , COUNT(DISTINCT n) OVER( ORDER BY i, s NULLS LAST ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) AS c FROM nested ORDER BY i, s NULLS LAST
SELECT i , l , COUNT(DISTINCT l) OVER( ORDER BY i, s NULLS LAST ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) AS c FROM nested ORDER BY i, s NULLS LAST
SELECT r , s , COUNT(DISTINCT s) OVER( ORDER BY r RANGE BETWEEN 10 PRECEDING AND 10 FOLLOWING) AS c FROM nested ORDER BY i, s NULLS LAST
SELECT i , s , STRING_AGG(DISTINCT s, ', ') OVER( ORDER BY i, s NULLS LAST ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) AS c FROM nested ORDER BY i, s NULLS LAST
CREATE TABLE testing AS SELECT x ,round(x * 0.333,0) % 3 AS y ,round(x * 0.333,0) % 3 AS z FROM generate_series(0,10) tbl(x)
SELECT x ,y ,z ,avg(x) OVER (PARTITION BY y) AS plain_window ,avg(x) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,avg(x) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM testing ORDER BY y, x
SELECT x ,y ,z ,count(*) OVER (PARTITION BY y) AS plain_window ,count(*) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,count(*) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM testing ORDER BY y, x
SELECT x ,y ,z ,median(x) OVER (PARTITION BY y) AS plain_window ,median(x) FILTER (WHERE x = 1) OVER (PARTITION BY y) AS x_filtered_window ,median(x) FILTER (WHERE z = 0) OVER (PARTITION BY y) AS z_filtered_window FROM testing ORDER BY y, x
create table lineitem ( l_extendedprice decimal(15,2), l_partkey integer, l_orderkey integer )
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(), from lineitem order by l_partkey, l_orderkey, l_extendedprice
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(order by l_partkey), from lineitem order by l_partkey, l_orderkey, l_extendedprice
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(order by l_partkey, l_orderkey), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(order by l_partkey, l_orderkey desc), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(), sum(l_extendedprice) over(order by l_partkey), sum(l_extendedprice) over(order by l_partkey, l_orderkey), sum(l_extendedprice) over(order by l_partkey, l_orderkey desc), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(partition by l_partkey), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(partition by l_partkey order by l_orderkey), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(partition by l_partkey order by l_orderkey desc), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(partition by l_partkey), sum(l_extendedprice) over(partition by l_partkey order by l_orderkey), sum(l_extendedprice) over(partition by l_partkey order by l_orderkey desc), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
select l_extendedprice, l_partkey, l_orderkey, sum(l_extendedprice) over(), sum(l_extendedprice) over(order by l_partkey), sum(l_extendedprice) over(order by l_partkey, l_orderkey), sum(l_extendedprice) over(partition by l_partkey order by l_orderkey desc), from lineitem order by l_partkey, l_orderkey, l_extendedprice desc
create table a as select case when range%2==0 then interval '1 year' else interval '2 years' end j, range::integer AS i from range(1, 5, 1)
select *, array_agg(col) over(partition by id order by col collate nocase) as lead_col_nocase from ( select unnest(array[1, 1, 1, 1]) as id, unnest(array['A', 'a', 'b', 'B']) as col )
CREATE TABLE db_city (name VARCHAR, city VARCHAR COLLATE NOCASE)
INSERT INTO db_city VALUES ('DuckDB', 'Amsterdam'), ('MonetDB','amsterdam'), ('VectorWise', 'Amstërdam')
SELECT name, city, row_number() OVER (PARTITION BY city) AS row_id FROM db_city
SELECT name, city, row_number() OVER (PARTITION BY city COLLATE NOCASE) AS row_id FROM db_city
CREATE TABLE t86 ( c0 VARCHAR COLLATE NOCASE NOT NULL )
CREATE TABLE t0 ( c0 BOOLEAN UNIQUE NOT NULL, PRIMARY KEY (c0) )
INSERT INTO t0(c0) VALUES (true)
INSERT INTO t86(c0) VALUES (''), ('cOB4')
(SELECT t86.c0, t0.c0 FROM t0, t86) EXCEPT ALL ( SELECT i,i FROM range(0, 4) r(i) )
CREATE TABLE t3(a TEXT, b TEXT, c INTEGER)
SELECT row_number() OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE NO OTHERS )
SELECT nth_value(c, 14) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE NO OTHERS )
SELECT min(c) OVER win, max(c) OVER win, sum(c) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE NO OTHERS ) ORDER BY a, b, c
SELECT row_number() OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE CURRENT ROW )
SELECT nth_value(c, 14) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE CURRENT ROW )
SELECT min(c) OVER win, max(c) OVER win, sum(c) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE CURRENT ROW ) ORDER BY a, b, c
SELECT row_number() OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE GROUP )
SELECT nth_value(c, 14) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE GROUP )
SELECT min(c) OVER win, max(c) OVER win, sum(c) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE GROUP ) ORDER BY a, b, c
SELECT row_number() OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE TIES )
SELECT nth_value(c, 14) OVER win FROM t3 WINDOW win AS ( ORDER BY c, b, a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE TIES )
create table a as select range%3 j, range::varchar AS s, case when range%3=0 then '-' else '|' end sep from range(1, 7, 1)
select j, s, string_agg(s) over (partition by j order by s) from a order by j, s
select j, s, string_agg(s, '|') over (partition by j order by s) from a order by j, s
CREATE TABLE item(i_category VARCHAR, i_brand VARCHAR, i_price INTEGER)
INSERT INTO item VALUES ('toys', 'fisher-price', 100)
SELECT i_category, i_brand, avg(sum(i_price)) OVER (PARTITION BY i_category), rank() OVER (PARTITION BY i_category ORDER BY i_category, i_brand) rn FROM item GROUP BY i_category, i_brand
CREATE TABLE item(i_category VARCHAR, i_brand VARCHAR, i_item_sk INTEGER)
CREATE TABLE store(s_store_name VARCHAR, s_company_name VARCHAR, s_store_sk INTEGER)
CREATE TABLE date_dim(d_year INTEGER, d_moy INTEGER, d_date_sk INTEGER)
CREATE TABLE store_sales(ss_sales_price DECIMAL, ss_item_sk INTEGER, ss_sold_date_sk INTEGER, ss_store_sk INTEGER)
INSERT INTO item VALUES ('Music', 'exportischolar', 1)
INSERT INTO store VALUES ('ought', 'Unknown', 1)
INSERT INTO date_dim VALUES (1999, 1, 1)
INSERT INTO store_sales VALUES (2.8, 1, 1, 1)
CREATE TABLE tbl AS SELECT 42 AS i
SELECT SUM(i) OVER (ROWS BETWEEN (SELECT UNNEST([1])) PRECEDING AND 1 FOLLOWING) FROM tbl
SELECT lead(c0, (SELECT UNNEST([0])), (SELECT UNNEST([1]))) OVER (ROWS BETWEEN 2 PRECEDING AND 4 PRECEDING) FROM (VALUES (1, 2)) a(c0)
CREATE TABLE tenk1 (unique1 int4, unique2 int4, two int4, four int4, ten int4, twenty int4, hundred int4, thousand int4, twothousand int4, fivethous int4, tenthous int4, odd int4, even int4, stringu1 varchar, stringu2 varchar, string4 varchar)
SELECT COUNT(*) OVER () FROM tenk1
SELECT sum(four) OVER (PARTITION BY ten ORDER BY unique2) AS sum_1, ten, four FROM tenk1 WHERE unique2 < 10 order by ten, unique2
SELECT row_number() OVER (ORDER BY unique2) rn FROM tenk1 WHERE unique2 < 10 ORDER BY rn
SELECT rank() OVER (PARTITION BY four ORDER BY ten) AS rank_1, ten, four FROM tenk1 WHERE unique2 < 10 ORDER BY four, ten
SELECT dense_rank() OVER (PARTITION BY four ORDER BY ten) FROM tenk1 WHERE unique2 < 10 ORDER BY four, ten
SELECT first_value(ten) OVER (PARTITION BY four ORDER BY ten) FROM tenk1 WHERE unique2 < 10 order by four, ten
SELECT cast(percent_rank() OVER (PARTITION BY four ORDER BY ten)*10 as INTEGER) FROM tenk1 ORDER BY four, ten
SELECT cast(cume_dist() OVER (PARTITION BY four ORDER BY ten)*10 as integer) FROM tenk1 WHERE unique2 < 10 order by four, ten
SELECT ntile(2) OVER (ORDER BY ten, four) nn FROM tenk1 ORDER BY ten, four, nn
SELECT ntile(3) OVER (ORDER BY ten, four) nn FROM tenk1 ORDER BY ten, four, nn
SELECT ntile(4) OVER (ORDER BY ten, four) nn FROM tenk1 ORDER BY ten, four, nn
CREATE TABLE mtcars (mpg DECIMAL, cyl INTEGER, disp DECIMAL, hp INTEGER, drat DECIMAL, wt DECIMAL, qsec DECIMAL, vs INTEGER, am INTEGER, gear INTEGER, carb INTEGER)
INSERT INTO mtcars VALUES ('21.0', '6', '160.0', '110', '3.90', '2.620', '16.46', '0', '1', '4', '4')
INSERT INTO mtcars VALUES ('21.0', '6', '160.0', '110', '3.90', '2.875', '17.02', '0', '1', '4', '4')
INSERT INTO mtcars VALUES ('22.8', '4', '108.0', '93', '3.85', '2.320', '18.61', '1', '1', '4', '1')
INSERT INTO mtcars VALUES ('21.4', '6', '258.0', '110', '3.08', '3.215', '19.44', '1', '0', '3', '1')
INSERT INTO mtcars VALUES ('18.7', '8', '360.0', '175', '3.15', '3.440', '17.02', '0', '0', '3', '2')
INSERT INTO mtcars VALUES ('18.1', '6', '225.0', '105', '2.76', '3.460', '20.22', '1', '0', '3', '1')
INSERT INTO mtcars VALUES ('14.3', '8', '360.0', '245', '3.21', '3.570', '15.84', '0', '0', '3', '4')
INSERT INTO mtcars VALUES ('24.4', '4', '146.7', '62', '3.69', '3.190', '20.00', '1', '0', '4', '2')
INSERT INTO mtcars VALUES ('22.8', '4', '140.8', '95', '3.92', '3.150', '22.90', '1', '0', '4', '2')
INSERT INTO mtcars VALUES ('19.2', '6', '167.6', '123', '3.92', '3.440', '18.30', '1', '0', '4', '4')
INSERT INTO mtcars VALUES ('17.8', '6', '167.6', '123', '3.92', '3.440', '18.90', '1', '0', '4', '4')
create table t as with q(id,s) as (values(1,42)), a(s)as materialized(select 42) select id from q join a on q.s=a.s
select id from t
DESCRIBE select 42 AS a
with cte as (select 42 AS a) FROM (DESCRIBE TABLE cte)
SUMMARIZE select 42 AS a
with cte as (select 42 AS a) FROM (SUMMARIZE TABLE cte)
CREATE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 5
FROM nums
WITH cte1 AS (SELECT NULL AS y), cte1_filter AS (SELECT y FROM cte1 WHERE y < '2025-12-01'::TIMESTAMPTZ) SELECT * FROM cte1_filter
CREATE TABLE t1 ( t1c1 BIGINT, t1c2 BIGINT, PRIMARY KEY (t1c1, t1c2) )
CREATE TABLE t2 ( t2c1 BIGINT )
create schema s1
create table s1.tbl(a varchar)
insert into s1.tbl values ('hello')
with tbl as (select 'world' b) select * from s1.tbl, tbl
WITH RECURSIVE cte AS (SELECT 42) SELECT * FROM cte
CREATE TABLE table1 (id INTEGER, a INTEGER)
CREATE TABLE table2 (table1_id INTEGER)
with cte as (select * from read_parquet('does/not/exist/file.parquet')) select 42
SELECT query, (WITH t AS (SELECT query) SELECT x FROM (VALUES ('cat')) AS _(x) WHERE x IN (SELECT query)) AS broken FROM (VALUES ('cat'), ('dog'), ('duck')) AS queries(query)
CREATE TABLE p(loc int8)
INSERT INTO p VALUES (1)
WITH RECURSIVE t(y, arr) AS ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr[:loc] FROM t, p WHERE y < 10 ) SELECT * FROM t
WITH RECURSIVE t(y, arr) AS ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr FROM t, p WHERE y < 10 AND y = loc ) SELECT * FROM t
WITH RECURSIVE t(y, arr) AS ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr[:loc] FROM t, p WHERE y < 10 AND y = loc ) SELECT * FROM t
WITH RECURSIVE t(arr) AS ( SELECT array[1,2,3,4,5,6] UNION ALL SELECT arr[arr[1]+1:6] FROM t WHERE arr[1] < 6 ) SELECT * FROM t
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT t1.x + t2.x + t3.x AS x FROM t t1, t t2, t t3 WHERE t1.x < 100 ) SELECT * FROM t ORDER BY 1
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT (t1.x + t2.x + t3.x)::HUGEINT AS x FROM t t1, t t2, t t3 WHERE t1.x < 100 ) SELECT * FROM t ORDER BY 1
CREATE TABLE a AS SELECT * FROM range(100) t1(i)
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT SUM(x) AS x FROM t, a WHERE x < 1000000 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT SUM(x) AS x FROM t, a WHERE x < 1000000 AND t.x=a.i ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT SUM(x) FROM (SELECT SUM(x) FROM t) t1(x), a WHERE x < 1000 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT (SELECT x + 1 FROM t) AS x FROM t WHERE x < 5 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS ( SELECT 1 AS x UNION SELECT (SELECT t.x+t2.x FROM t t2 LIMIT 1) AS x FROM t WHERE x < 10 ) SELECT * FROM t ORDER BY 1 NULLS LAST
call dbgen(sf=0.001)
CREATE TYPE supplier_change AS struct( part BIGINT, old BIGINT, new BIGINT )
CREATE TYPE savings AS struct( savings numeric, supplier_changes supplier_change[] )
CREATE TABLE tag(id int, name string, subclassof int)
INSERT INTO tag VALUES (7, 'Music', 9), (8, 'Movies', 9), (9, 'Art', NULL)
with RECURSIVE parents_tab (id , value , parent ) as (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ), parents_tab2(id , value , parent ) as (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ) select * from parents_tab union all select id, value+2, parent from parents_tab2 ORDER BY id, value, parent
create view vparents as with RECURSIVE parents_tab (id , value , parent ) as (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ), parents_tab2(id , value , parent ) as (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ) select * from parents_tab union all select id, value+2, parent from parents_tab2
select * from vparents
WITH my_list(value) AS (VALUES (1), (2), (3)) SELECT * FROM my_list LIMIT 0 OFFSET 1
with cte1 as (Select i as j from a) select * from cte1
with cte1 as (Select i as j from a) select x from cte1 t1(x)
with cte1(xxx) as (Select i as j from a) select xxx from cte1
with cte1(xxx) as (Select i as j from a) select x from cte1 t1(x)
with cte1 as (Select i as j from a), cte2 as (select ref.j as k from cte1 as ref), cte3 as (select ref2.j+1 as i from cte1 as ref2) select * from cte2 , cte3
with cte1 as (select i as j from a), cte2 as (select ref.j as k from cte1 as ref), cte3 as (select ref2.j+1 as i from cte1 as ref2) select * from cte2 union all select * FROM cte3
with cte1 as (Select i as j from a) select * from cte1 cte11, cte1 cte12
with cte1 as (Select i as j from a) select * from cte1 where j = (select max(j) from cte1 as cte2)
with cte1(x, y) as (select 42 a, 84 b) select zzz, y from cte1 t1(zzz)
create view va AS (with cte as (Select i as j from a) select * from cte)
select * from va
with cte AS (SELECT * FROM va) SELECT * FROM cte
with cte1 as (with b as (Select i as j from a) Select j from b) select x from cte1 t1(x)
with cte1(xxx) as (with ncte(yyy) as (Select i as j from a) Select yyy from ncte) select xxx from cte1
with cte1 as (with b as (Select i as j from a) select j from b), cte2 as (with c as (select ref.j+1 as k from cte1 as ref) select k from c) select * from cte1 , cte2
with cte1 as (Select i as j from a) select * from (with cte2 as (select max(j) as j from cte1) select * from cte2) f
with cte1 as (Select i as j from a) select * from cte1 where j = (with cte2 as (select max(j) as j from cte1) select j from cte2)
with cte as (Select i as j from a) select * from cte where j = (with cte as (select max(j) as j from cte) select j from cte)
create table a (id integer)
insert into a values (1729)
create view va as (with v as (select * from a) select * from v)
with a as (select * from va) select * from a
create or replace table orders(ordered_at int)
create or replace table stg_orders(ordered_at int)
insert into orders values (1)
insert into stg_orders values (1)
with orders as ( select * from main.stg_orders where ordered_at >= (select max(ordered_at) from main.orders) ), some_more_logic as ( select * from orders ) select * from some_more_logic
SELECT COUNT(*), MIN(i), MAX(i), AVG(i) FROM union_view
WITH RECURSIVE t(x) AS ( SELECT 1 UNION ALL SELECT x+1 FROM t WHERE x < 4 ), u(x) AS ( SELECT * FROM t UNION ALL SELECT u.x * 2 + t.x FROM u, t WHERE u.x < 32 ) SELECT * FROM u ORDER BY x LIMIT 5
CREATE TABLE v(x INT)
INSERT INTO v VALUES (1),(2),(3)
WITH RECURSIVE t(x) AS ( SELECT 1 UNION ALL SELECT x + 1 FROM (SELECT t.x+1 FROM v AS _(p) FULL OUTER JOIN t ON t.x = p) AS _(x) WHERE x < 10 ) SELECT * FROM v AS _(p) RIGHT OUTER JOIN t ON t.x = p ORDER BY p, t NULLS LAST
WITH RECURSIVE parent(p,c) AS ( VALUES ('c1','c2'), ('c1','c3'), ('c3','c4'), ('c3','c5'), ('c4','c6'), ('c4','c7') ), ancestor(a,c) AS ( FROM parent UNION SELECT a1.x, a2.y FROM recurring.ancestor AS a1(x,z) NATURAL JOIN recurring.ancestor AS a2(z,y) ) FROM ancestor ORDER BY ALL
CREATE TABLE emp (empno INTEGER PRIMARY KEY, ename VARCHAR, job VARCHAR, mgr INTEGER, hiredate DATE, sal DOUBLE, comm DOUBLE, deptno INTEGER)
WITH RECURSIVE ctename AS ( SELECT empno, ename FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
WITH RECURSIVE ctename AS ( SELECT empno, ename, 0 AS level FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.level + 1 FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
WITH RECURSIVE ctename AS ( SELECT empno, ename, ename AS path FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.path || ' -> ' || emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
CREATE VIEW ctenames AS ( WITH RECURSIVE ctename AS ( SELECT empno, ename, ename AS path FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.path || ' -> ' || emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename )
SELECT * FROM ctenames
WITH RECURSIVE fib AS ( SELECT 1 AS n, 1::bigint AS "fibₙ", 1::bigint AS "fibₙ₊₁" UNION ALL SELECT n+1, "fibₙ₊₁", "fibₙ" + "fibₙ₊₁" FROM fib ) SELECT n, "fibₙ" FROM fib LIMIT 20
with recursive t as (select 1 as x union select x+1 from t where x < 3) select * from t order by x
with recursive t(x) as (select 1 union select x+1 from t where x < 3) select * from t order by x
with recursive t(x) as (select 1 union select x+1 from t where x < 3) select zz from t t1(zz) order by zz
with recursive t(x) as (select 1 union select zzz+1 from t t1(zzz) where zzz < 3) select zz from t t1(zz) order by zz
with recursive t as (select 1 as x union select x from t) select * from t
with recursive t as (select 1 as x union select x+1 from t as m where m.x < 3) select * from t order by x
with recursive t as (select 1 as x union select m.x+f.x from t as m, t as f where m.x < 3) select * from t order by x
with recursive t as (select 1 as x, 'hello' as y union select x+1, y || '-' || 'hello' from t where x < 3) select * from t order by x
with recursive t as (select 1 as x union select x+1 from t where x < 3) select min(a1.x) from t a1, t a2
with recursive t as (select 1 as x union select x+(SELECT 1) from t where x < 3) select * from t order by x
with recursive t as (select 1 as x union all select * from (select x from t where x < 5) tbl(i) join (select 1) tbl2(i) using (i)) select * from t limit 3
with recursive t as (select 1 as x union all select * from (select 1) tbl2(i) join (select x from t where x < 5) tbl(i) using (i)) select * from t limit 3
with recursive t as (select 1 as x union all select x+1 from t where x < 3) select * from t
with recursive t as (select 1 as x union all select x+1 from t as m where m.x < 3) select * from t
with recursive t as (select 1 as x union all select m.x+f.x from t as m, t as f where m.x < 3) select * from t
with recursive t as (select 1 as x, 'hello' as y union all select x+1, y || '-' || 'hello' from t where x < 3) select * from t
with recursive t as (select 1 as x union all select x+1 from t where x < 3) select min(a1.x) from t a1, t a2
with recursive t as (select 1 as x union all select x+(SELECT 1) from t where x < 3) select * from t
create table integers as with recursive t as (select 1 as x union all select x+1 from t where x < 3) select * from t
with recursive t as (select (select min(x) from integers) as x union all select x+1 from t where x < 3) select * from t
with recursive t as (select 1 as x union all select sum(x+1) AS x from t where x < 3 group by x) select * from t
with recursive t as (select 1 as x union all select sum(x+1) AS x from t where x < 3) select * from (select * from t limit 10) t1(x) order by x nulls last
WITH RECURSIVE t AS ( SELECT 1 AS i UNION ALL SELECT j FROM t, generate_series(0, 10, 1) series(j) WHERE j=i+1 ) SELECT * FROM t
CALL enable_logging(level='error')
WITH RECURSIVE cte(x,y) USING KEY (x) AS ( SELECT 1, 0 UNION SELECT x, y+1 FROM cte WHERE y < 10 ) TABLE cte
SELECT log_level, message[0:42] FROM duckdb_logs
CALL truncate_duckdb_logs()
CALL enable_logging(level='warning')
SET deprecated_using_key_syntax='UNION_AS_UNION_ALL'
create table batch ( entity text, start_ts timestamp, duration interval )
create table active_events ( entity text, start_ts timestamp, end_ts timestamp )
explain create table new_active_events as with new_events as materialized ( select * from batch ), combined_deduplicated_events as ( select entity, min(start_ts) as start_ts, max(end_ts) as end_ts from active_events group by entity ), all_events as ( select * from combined_deduplicated_events ) select * from new_events
WITH a(x) AS MATERIALIZED ( SELECT * FROM generate_series(1, 10) ), b(x) AS MATERIALIZED ( SELECT * FROM a WHERE x < 8 ) SELECT * FROM b WHERE x % 3 = 1 ORDER BY x
WITH t(x) AS MATERIALIZED (VALUES (42)) INSERT INTO a (SELECT * FROM t)
WITH t(x) AS MATERIALIZED (VALUES (42)) DELETE FROM a WHERE a.i IN (SELECT * FROM t)
WITH t(x) AS MATERIALIZED (VALUES (42)) UPDATE a SET i = 0 WHERE a.i IN (SELECT * FROM t)
FROM a
insert into a values (2)
WITH t(x) AS MATERIALIZED (SELECT 1), u(x) AS MATERIALIZED (SELECT 2 UNION ALL SELECT * FROM t) DELETE FROM a WHERE a.i IN (SELECT * FROM u)
WITH t(x) AS MATERIALIZED (SELECT 1), u(x) AS MATERIALIZED (SELECT 2 UNION ALL SELECT * FROM t) UPDATE a SET i = 99 WHERE a.i IN (SELECT * FROM u)
FROM a ORDER BY 1
WITH t(x) AS MATERIALIZED (SELECT 1), u(x) AS MATERIALIZED (SELECT 2 UNION ALL SELECT * FROM t) INSERT INTO a (SELECT * FROM u)
WITH t(x) AS MATERIALIZED (SELECT 1) DELETE FROM a WHERE i IN (WITH s(x) AS MATERIALIZED (SELECT x + 41 FROM t) SELECT * FROM t)
WITH t(x) AS MATERIALIZED (SELECT 1) DELETE FROM a WHERE i IN (WITH s(x) AS MATERIALIZED (SELECT x + 41 FROM t) SELECT * FROM s)
WITH RECURSIVE cte AS MATERIALIZED (SELECT 42) SELECT * FROM cte
call dsdgen(sf=0)
insert into a values (1), (2), (3), (NULL), (42), (84)
PREPARE v1 AS WITH t(x) AS MATERIALIZED (VALUES ($1)) DELETE FROM a WHERE i IN (FROM t)
EXECUTE v1(42)
PREPARE v2 AS WITH t(x) AS MATERIALIZED (VALUES ($1)) DELETE FROM a WHERE (i + $2) IN (FROM t)
EXECUTE v2(5, 2)
WITH RECURSIVE t(y, arr) AS MATERIALIZED ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr[:loc] FROM t, p WHERE y < 10 ) SELECT * FROM t
WITH RECURSIVE t(y, arr) AS MATERIALIZED ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr FROM t, p WHERE y < 10 AND y = loc ) SELECT * FROM t
WITH RECURSIVE t(y, arr) AS MATERIALIZED ( SELECT 1, array[1,2,3,4,5,6] UNION ALL SELECT y+1, arr[:loc] FROM t, p WHERE y < 10 AND y = loc ) SELECT * FROM t
WITH RECURSIVE t(arr) AS MATERIALIZED ( SELECT array[1,2,3,4,5,6] UNION ALL SELECT arr[arr[1]+1:6] FROM t WHERE arr[1] < 6 ) SELECT * FROM t
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT t1.x + t2.x + t3.x AS x FROM t t1, t t2, t t3 WHERE t1.x < 100 ) SELECT * FROM t ORDER BY 1
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT (t1.x + t2.x + t3.x)::HUGEINT AS x FROM t t1, t t2, t t3 WHERE t1.x < 100 ) SELECT * FROM t ORDER BY 1
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT SUM(x) AS x FROM t, a WHERE x < 1000000 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT SUM(x) AS x FROM t, a WHERE x < 1000000 AND t.x=a.i ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT SUM(x) FROM (SELECT SUM(x) FROM t) t1(x), a WHERE x < 1000 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT (SELECT x + 1 FROM t) AS x FROM t WHERE x < 5 ) SELECT * FROM t ORDER BY 1 NULLS LAST
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS x UNION SELECT (SELECT t.x+t2.x FROM t t2 LIMIT 1) AS x FROM t WHERE x < 10 ) SELECT * FROM t ORDER BY 1 NULLS LAST
create view vparents as with RECURSIVE parents_tab (id , value , parent ) as MATERIALIZED (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ), parents_tab2 (id , value , parent ) as MATERIALIZED (values (1, 1, 2), (2, 2, 4), (3, 1, 4), (4, 2, -1), (5, 1, 2), (6, 2, 7), (7, 1, -1) ) select * from parents_tab union all select id, value+2, parent from parents_tab2
select * from vparents ORDER BY id, value, parent
WITH my_list(value) AS MATERIALIZED (VALUES (1), (2), (3)) SELECT * FROM my_list LIMIT 0 OFFSET 1
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION ALL SELECT y + 1 FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT 1 UNION ALL SELECT y + _.x FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION ALL SELECT y + _.x FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION ALL SELECT t1.y + t2.y + _.x FROM t AS t1, t AS t2 WHERE t1.y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y, (WITH RECURSIVE t(z) AS MATERIALIZED ( SELECT x + y UNION ALL SELECT z + 1 FROM t WHERE z < 3 ) SELECT sum(z) FROM t) AS z FROM generate_series(1,2) AS _(x), generate_series(1,2) AS __(y) order by all
SELECT x, y, (WITH RECURSIVE t(z) AS MATERIALIZED ( SELECT x + y UNION ALL SELECT z + 1 FROM (WITH RECURSIVE g(a) AS MATERIALIZED ( SELECT t.z FROM t UNION ALL SELECT g.a + (x + y) / 2 FROM g WHERE g.a < 3) SELECT * FROM g) AS t(z) WHERE z < 5 ) SELECT sum(z) FROM t) AS z FROM generate_series(1,2) AS _(x), generate_series(1,2) AS __(y) order by all
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION SELECT y + 1 FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT 1 UNION SELECT y + _.x FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION SELECT y + _.x FROM t WHERE y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y FROM generate_series(1,4) AS _(x), LATERAL (WITH RECURSIVE t(y) AS MATERIALIZED ( SELECT _.x UNION SELECT t1.y + t2.y + _.x FROM t AS t1, t AS t2 WHERE t1.y < 3 ) SELECT * FROM t) AS t ORDER BY x, y
SELECT x, y, (WITH RECURSIVE t(z) AS MATERIALIZED ( SELECT x + y UNION SELECT z + 1 FROM t WHERE z < 3 ) SELECT sum(z) FROM t) AS z FROM generate_series(1,2) AS _(x), generate_series(1,2) AS __(y) order by all
SELECT x, y, (WITH RECURSIVE t(z) AS MATERIALIZED ( SELECT x + y UNION SELECT z + 1 FROM (WITH RECURSIVE g(a) AS MATERIALIZED ( SELECT t.z FROM t UNION SELECT g.a + (x + y) / 2 FROM g WHERE g.a < 3) SELECT * FROM g) AS t(z) WHERE z < 5 ) SELECT sum(z) FROM t) AS z FROM generate_series(1,2) AS _(x), generate_series(1,2) AS __(y) order by all
with cte1 as MATERIALIZED (Select i as j from a) select * from cte1
with cte1 as MATERIALIZED (with b as MATERIALIZED (Select i as j from a) Select j from b) select x from cte1 t1(x)
with cte1(xxx) as MATERIALIZED (with ncte(yyy) as MATERIALIZED (Select i as j from a) Select yyy from ncte) select xxx from cte1
with cte1 as MATERIALIZED (with b as MATERIALIZED (Select i as j from a) select j from b), cte2 as MATERIALIZED (with c as MATERIALIZED (select ref.j+1 as k from cte1 as ref) select k from c) select * from cte1 , cte2
with cte1 as MATERIALIZED (Select i as j from a) select * from (with cte2 as MATERIALIZED (select max(j) as j from cte1) select * from cte2) f
with cte1 as MATERIALIZED (Select i as j from a) select * from cte1 where j = (with cte2 as MATERIALIZED (select max(j) as j from cte1) select j from cte2)
with cte as materialized (Select i as j from a) select * from cte where j = (with cte as (select max(j) as j from cte) select j from cte)
with cte as MATERIALIZED (Select i as j from a) select * from cte where j = (with cte as MATERIALIZED (select max(j) as j from cte) select j from cte)
with cte1 as MATERIALIZED (Select i as j from a) select x from cte1 t1(x)
with cte1(xxx) as MATERIALIZED (Select i as j from a) select xxx from cte1
with cte1(xxx) as MATERIALIZED (Select i as j from a) select x from cte1 t1(x)
with cte1 as MATERIALIZED (Select i as j from a), cte2 as MATERIALIZED (select ref.j as k from cte1 as ref), cte3 as MATERIALIZED (select ref2.j+1 as i from cte1 as ref2) select * from cte2 , cte3
with cte1 as MATERIALIZED (select i as j from a), cte2 as MATERIALIZED (select ref.j as k from cte1 as ref), cte3 as MATERIALIZED (select ref2.j+1 as i from cte1 as ref2) select * from cte2 union all select * FROM cte3
with cte1 as MATERIALIZED (Select i as j from a) select * from cte1 cte11, cte1 cte12
with cte1 as MATERIALIZED (Select i as j from a) select * from cte1 where j = (select max(j) from cte1 as cte2)
with cte1(x, y) as MATERIALIZED (select 42 a, 84 b) select zzz, y from cte1 t1(zzz)
create view va AS (with cte as MATERIALIZED (Select i as j from a) select * from cte)
with cte AS MATERIALIZED (SELECT * FROM va) SELECT * FROM cte
create view vb AS (with cte1 as MATERIALIZED (Select i as j from a), cte2 as MATERIALIZED (select ref.j+1 as k from cte1 as ref) select * from cte2)
select * from vb
create view va as (with v as MATERIALIZED (select * from a) select * from v)
with a as MATERIALIZED (select * from va) select * from a
CREATE TABLE T0(C1 INT)
CREATE TABLE T1(C1 INT)
INSERT INTO T0(C1) VALUES (1)
INSERT INTO T1(C1) VALUES (1)
WITH CTE AS MATERIALIZED ( SELECT A1, * FROM T0 LEFT JOIN ( SELECT C1 AS A1 FROM T1 ) ON T0.C1 = A1 ) SELECT A1 FROM CTE
WITH t(x) AS MATERIALIZED (SELECT 1) SELECT * FROM t
WITH t(x) AS MATERIALIZED (SELECT t FROM generate_series(1,3) AS _(t)) SELECT t1.x,1 as y FROM t AS t1 ORDER BY x
WITH t(x) AS MATERIALIZED (SELECT t FROM generate_series(1,3) AS _(t)) SELECT t1.x, t1.x FROM t AS t1 ORDER BY x
WITH t(x) AS MATERIALIZED (SELECT t FROM generate_series(1,3) AS _(t)) SELECT t1.x, t2.x FROM t AS t1, t AS t2 ORDER BY t1.x, t2.x
WITH t(x) AS MATERIALIZED (SELECT 1), u(x) AS MATERIALIZED (SELECT 2) SELECT * FROM u FULL OUTER JOIN t ON TRUE
WITH t(x) AS MATERIALIZED (SELECT x FROM generate_series(1,10) AS _(x) limit 4) SELECT DISTINCT x FROM t order by x desc
WITH t(x) AS MATERIALIZED (SELECT x FROM generate_series(1,10) AS _(x) limit 4) SELECT DISTINCT x FROM t order by x desc LIMIT 2
WITH t(x) AS MATERIALIZED ( WITH u(x) AS MATERIALIZED ( SELECT 42 ) SELECT * FROM u ) SELECT * FROM t
WITH t(x) AS MATERIALIZED (SELECT 1), u(x) AS MATERIALIZED (SELECT x+1 FROM t) TABLE u UNION ALL TABLE t
WITH RECURSIVE t(x) AS MATERIALIZED ( SELECT 1 UNION ALL SELECT x+1 FROM t WHERE x < 4 ), u(x) AS MATERIALIZED ( SELECT * FROM t UNION ALL SELECT u.x * 2 + t.x FROM u, t WHERE u.x < 32 ) SELECT * FROM u ORDER BY x LIMIT 5
WITH RECURSIVE t(x) AS MATERIALIZED ( SELECT 1 UNION ALL SELECT x + 1 FROM (SELECT t.x+1 FROM v AS _(p) FULL OUTER JOIN t ON t.x = p) AS _(x) WHERE x < 10 ) SELECT * FROM v AS _(p) RIGHT OUTER JOIN t ON t.x = p ORDER BY p, x NULLS LAST
WITH RECURSIVE ctename AS MATERIALIZED ( SELECT empno, ename FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
WITH RECURSIVE ctename AS MATERIALIZED ( SELECT empno, ename, 0 AS level FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.level + 1 FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
WITH RECURSIVE ctename AS MATERIALIZED ( SELECT empno, ename, ename AS path FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.path || ' -> ' || emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename
CREATE VIEW ctenames AS ( WITH RECURSIVE ctename AS MATERIALIZED ( SELECT empno, ename, ename AS path FROM emp WHERE empno = 7566 UNION ALL SELECT emp.empno, emp.ename, ctename.path || ' -> ' || emp.ename FROM emp JOIN ctename ON emp.mgr = ctename.empno ) SELECT * FROM ctename )
WITH RECURSIVE fib AS MATERIALIZED ( SELECT 1 AS n, 1::bigint AS "fibₙ", 1::bigint AS "fibₙ₊₁" UNION ALL SELECT n+1, "fibₙ₊₁", "fibₙ" + "fibₙ₊₁" FROM fib WHERE n <= 20 ) SELECT n, "fibₙ" FROM fib LIMIT 20
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t as m where m.x < 3) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select m.x+f.x from t as m, t as f where m.x < 3) select * from t
with recursive t as MATERIALIZED (select 1 as x, 'hello' as y union all select x+1, y || '-' || 'hello' from t where x < 3) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3) select min(a1.x) from t a1, t a2
with recursive t as MATERIALIZED (select 1 as x union all select x+(SELECT 1) from t where x < 3) select * from t
create table integers as with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3) select * from t
with recursive t as MATERIALIZED (select (select min(x) from integers) as x union all select x+1 from t where x < 3) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select sum(x+1) AS x from t where x < 3 group by x) select * from t
WITH RECURSIVE t AS MATERIALIZED ( SELECT 1 AS i UNION ALL SELECT j FROM t, generate_series(0, 10, 1) series(j) WHERE j=i+1 ) SELECT * FROM t
with recursive t as MATERIALIZED (select 1 as x union select x+1 from t where x < 3) select * from t order by x
with recursive t(x) as MATERIALIZED (select 1 union select x+1 from t where x < 3) select * from t order by x
with recursive t(x) as MATERIALIZED (select 1 union select x+1 from t where x < 3) select zz from t t1(zz) order by zz
with recursive t(x) as MATERIALIZED (select 1 union select zzz+1 from t t1(zzz) where zzz < 3) select zz from t t1(zz) order by zz
with recursive t as MATERIALIZED (select 1 as x union select x from t) select * from t
with recursive t as MATERIALIZED (select 1 as x union select x+1 from t as m where m.x < 3) select * from t order by x
with recursive t as MATERIALIZED (select 1 as x union select m.x+f.x from t as m, t as f where m.x < 3) select * from t order by x
with recursive t as MATERIALIZED (select 1 as x, 'hello' as y union select x+1, y || '-' || 'hello' from t where x < 3) select * from t order by x
with recursive t as MATERIALIZED (select 1 as x union select x+1 from t where x < 3) select min(a1.x) from t a1, t a2
with recursive t as MATERIALIZED (select 1 as x union select x+(SELECT 1) from t where x < 3) select * from t order by x
with recursive t as MATERIALIZED (select 1 as x union select x+(SELECT 1+t.x) from t where x < 5) select * from t order by x
create table integers as with recursive t as MATERIALIZED (select 1 as x union select x+1 from t where x < 3) select * from t
CREATE TABLE test (a hugeint)
INSERT INTO test values ((-170141183460469231731687303715884105728)::hugeint), (-1111::hugeint), (-1::hugeint), (0::hugeint), (1::hugeint), (1111::hugeint)
SELECT * FROM test order by a
SELECT * FROM test order by a DESC
CREATE TABLE test(col1 INT, col2 INT2[][][][][][])
INSERT INTO test VALUES(1000000000, null), (1000000001, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (1, [[[[[[]]]]]])
SELECT col1, col2 FROM test ORDER BY col1 NULLS LAST, col2
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY ALL LIMIT 2
SELECT COUNT(*) FROM (SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k LIMIT 2) tbl
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT 5
CREATE TABLE integers AS SELECT 5 k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM integers)
CREATE TABLE strings AS SELECT '5'::VARCHAR k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM strings)
PREPARE v1 AS SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT ?::VARCHAR
EXECUTE v1(5)
PREPARE v1 AS SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT ?::VARCHAR %
EXECUTE v1('0.05')
CREATE TABLE doubles AS SELECT 0.05 d
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT d FROM doubles) %
SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7
SELECT COUNT(*) FROM (SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7) tbl
CREATE TABLE integers AS SELECT -1 k
SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers ORDER BY * DESC
SELECT * FROM integers ORDER BY * DESC NULLS LAST
SELECT * FROM integers UNION ALL SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers UNION SELECT * FROM integers ORDER BY ALL
create table t1 as from VALUES ('A', 1), ('B', 3), ('C', 12), ('A', 5), ('B', 8), ('C', 9), ('A', 10), ('B', 20), ('C', 3) t(a, b)
PRAGMA disabled_optimizers='compressed_materialization'
from t1 order by a
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22)
SELECT a FROM test LIMIT 1
SELECT a FROM test LIMIT 1.25
SELECT a FROM test LIMIT 2-1
CREATE TABLE test2 (a STRING)
INSERT INTO test2 VALUES ('Hello World')
PREPARE v1 AS SELECT * FROM test2 LIMIT 3
EXECUTE v1
INSERT INTO integers VALUES (1), (2), (3), (4), (5)
CREATE SEQUENCE seq START 3
PRAGMA disable_verify_fetch_row
SELECT * FROM integers LIMIT nextval('seq')
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte)
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte) OFFSET (SELECT * FROM cte)
PREPARE v1 AS SELECT 'Test' LIMIT ?
EXECUTE v1(1)
EXECUTE v1(0)
PREPARE v2 AS SELECT * FROM RANGE(1000000000) LIMIT ? OFFSET ?
EXECUTE v2(3, 0)
EXECUTE v2(3, 17)
PREPARE v3 AS SELECT * FROM RANGE(1000000000) LIMIT 2 OFFSET ?
EXECUTE v3(0)
EXECUTE v3(17)
PREPARE v4 AS SELECT * FROM RANGE(1000000000) LIMIT ? OFFSET 17
EXECUTE v4(3)
EXECUTE v4(6)
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22), (14, 32), (15, 52)
SELECT a FROM test LIMIT 20 %
SELECT a FROM test LIMIT 40 PERCENT
SELECT a FROM test LIMIT 35%
SELECT a FROM test LIMIT 79.9%
SELECT a FROM test LIMIT 80.1%
SELECT a FROM test LIMIT 100 PERCENT
SELECT a FROM test LIMIT (30-10) %
SELECT * FROM test LIMIT RANDOM() %
SELECT a FROM test LIMIT 50% OFFSET 2
SELECT * FROM range(10) LIMIT 50% OFFSET 2
SELECT * FROM range(10000) LIMIT 0.1% OFFSET 8000
INSERT INTO integers VALUES (1), (NULL)
SELECT * FROM integers ORDER BY i
SELECT * FROM integers ORDER BY i NULLS FIRST
SELECT * FROM integers ORDER BY i NULLS LAST
SELECT 10 AS j, i FROM integers ORDER BY j, i NULLS LAST
CREATE TABLE test(i INTEGER, j INTEGER)
INSERT INTO test VALUES (1, 1), (NULL, 1), (1, NULL)
SELECT * FROM test ORDER BY i NULLS FIRST, j NULLS LAST
SELECT * FROM test ORDER BY i NULLS FIRST, j NULLS FIRST
SELECT * FROM test ORDER BY i NULLS LAST, j NULLS FIRST
SELECT i, j, row_number() OVER (PARTITION BY i ORDER BY j NULLS FIRST) FROM test ORDER BY i NULLS FIRST, j NULLS FIRST
SELECT i, j, row_number() OVER (PARTITION BY i ORDER BY j NULLS LAST) FROM test ORDER BY i NULLS FIRST, j NULLS FIRST
select b from test where a = 12
SELECT b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY a
SELECT a, b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY b, a
SELECT a, b FROM test ORDER BY 2, 1
SELECT a, b FROM test ORDER BY b DESC, a
SELECT a, b FROM test ORDER BY b, a DESC
SELECT a, b FROM test ORDER BY b, a DESC LIMIT 1
SELECT a, b FROM test ORDER BY b, a DESC LIMIT 1 OFFSET 1
SELECT a, b FROM test ORDER BY b, a DESC OFFSET 1
SELECT a, b FROM test WHERE a < 13 ORDER BY b
SELECT a AS k, b FROM test UNION SELECT a, b AS k FROM test ORDER BY k
SELECT a AS k, b FROM test UNION SELECT a AS k, b FROM test ORDER BY k
SELECT a % 2, b FROM test UNION SELECT a % 2 AS k, b FROM test ORDER BY a % 2
CREATE TABLE test AS SELECT a FROM range(10000, 0, -1) t1(a)
SELECT * FROM test ORDER BY a
SELECT a FROM test ORDER BY a
PRAGMA default_order='DESCENDING'
PRAGMA default_order='ASC'
insert into test values (100), (25), (75), (50)
select * from test order by i
drop table test
insert into test values (10000), (2500), (7500), (5000)
insert into test values (1000000), (250000), (750000), (500000)
insert into test values (1000000000), (250000000), (750000000), (500000000)
create table test (i hugeint)
insert into test values (295147905179352825856), (73786976294838206464), (147573952589676412928), (36893488147419103232)
CREATE TABLE tbl_structs AS SELECT {'a': 2.0, 'b': 'hello', 'c': [1, 2]} AS s1, 1::BIGINT AS i, {'k': 1::TINYINT, 'j': 0::BOOL} AS s2
INSERT INTO tbl_structs VALUES ( {'a': 1.0, 'b': 'yay', 'c': [10, 20]}, 42, {'k': 2, 'j': 1})
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY id
SELECT s1, s1.a FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY i
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY 2 DESC
SELECT i, UNNEST(s1), UNNEST(s2) FROM tbl_structs ORDER BY 5 DESC
CREATE OR REPLACE TABLE t3(c VARCHAR)
INSERT INTO t3 VALUES ('19'), ('21'), ('22'), ('23')
CREATE OR REPLACE TABLE t2( a VARCHAR, b VARCHAR )
INSERT INTO t2 VALUES ('3', '8'), ('5', NULL), ('8', NULL), ('11', NULL)
CREATE OR REPLACE TABLE t1( a VARCHAR, c VARCHAR )
INSERT INTO t1 VALUES ('2', '22'), ('3', '21'), ('11', '19'), ('5', '23')
WITH cte1 AS ( SELECT struct_pack(f := json_extract('[]', '$[*]')) AS s0, struct_pack(ff := c) AS s1, t1.a AS p FROM t3 JOIN t1 USING (c) ), cte2 AS ( SELECT cte1.s1, cte1.s0, struct_pack(fff := cte1.p) AS s2 FROM cte1 JOIN t2 AS t22 ON cte1.p = t22.a LEFT JOIN t2 ON t22.b = t2.a ), cte3 AS ( SELECT cte2.* FROM cte2 ORDER BY s1, s2 ) SELECT * FROM cte3 LIMIT 1
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 360
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 10 offset 440
LOAD tpch
CALL dbgen(sf=0.01)
PRAGMA disable_optimizer
SELECT o.o_orderkey FROM ( SELECT l.o_orderkey FROM orders l LEFT JOIN lineitem li ON li.l_orderkey = l.o_orderkey WHERE l.o_orderkey = 1 LIMIT 1 ) AS filtered JOIN orders o ON TRUE WHERE o.o_orderkey = 1
CREATE TABLE tbl AS SELECT concat('thisisastring', i) s FROM range(1_000_000) t(i)
FROM tbl WHERE s LIKE '%string999999%' LIMIT 5
EXPLAIN FROM tbl WHERE s LIKE '%string999999%' LIMIT 5
SELECT * FROM (SELECT SUM(i) FROM range(100000000000) tbl(i)) LIMIT 0
PRAGMA explain_output='OPTIMIZED_ONLY'
EXPLAIN SELECT * FROM (SELECT SUM(i) FROM range(100000000000) tbl(i)) LIMIT 0
EXPLAIN SELECT * FROM (SELECT SUM(i) FROM range(100000000000) tbl(i)) WHERE 1=0
SET preserve_insertion_order=false
CREATE TABLE integers AS SELECT 1 AS i FROM range(1000000) t(i)
SELECT MIN(i), MAX(i), COUNT(*) FROM integers
SELECT * FROM integers LIMIT 5
SELECT * FROM integers LIMIT 5 OFFSET 500000
CREATE TABLE integers2 AS SELECT * FROM range(1000000) tbl(i)
SELECT MIN(i), MAX(i), COUNT(*) FROM integers2
SELECT * FROM integers2 WHERE i IN (337, 195723, 442578, 994375)
SELECT * FROM integers2 WHERE i IN (337, 195723, 442578, 994375) LIMIT 4
CREATE TABLE df (x VARCHAR, y BIGINT)
INSERT INTO df VALUES ('2021-01-01 12:00:00', 1)
select CAST(x as DATE) = '2021-01-01' a, IF(CAST(x as DATE) = '2021-01-01', y, 0) b, CASE WHEN CAST(x as DATE) = '2021-01-01' THEN y ELSE 0 END c, IF(CAST(x as DATE) = '2021-01-01', 1, 0) d from df
create table null_table (i "null")
select typeof(i) from null_table
insert into null_table values (null)
create table null_list (i "null"[])
insert into null_list values (null), ([null])
select i from null_list
create table null_struct (i struct(n "null"))
insert into null_struct values (null), ({n:null})
select i from null_struct
create table null_map (i map("null", "null"))
SELECT typeof(1)
SELECT 127::DECIMAL(3,0)::TINYINT, -127::DECIMAL(3,0)::TINYINT, -7::DECIMAL(9,1)::TINYINT, 27::DECIMAL(18,1)::TINYINT, 33::DECIMAL(38,1)::TINYINT
SELECT 127::DECIMAL(3,0)::SMALLINT, -32767::DECIMAL(5,0)::SMALLINT, -7::DECIMAL(9,1)::SMALLINT, 27::DECIMAL(18,1)::SMALLINT, 33::DECIMAL(38,1)::SMALLINT
SELECT 127::DECIMAL(3,0)::INTEGER, -2147483647::DECIMAL(10,0)::INTEGER, -7::DECIMAL(9,1)::INTEGER, 27::DECIMAL(18,1)::INTEGER, 33::DECIMAL(38,1)::INTEGER
SELECT 127::DECIMAL(3,0)::BIGINT, -9223372036854775807::DECIMAL(19,0)::BIGINT, -7::DECIMAL(9,1)::BIGINT, 27::DECIMAL(18,1)::BIGINT, 33::DECIMAL(38,1)::BIGINT
SELECT 127::DECIMAL(3,0)::HUGEINT, -17014118346046923173168730371588410572::DECIMAL(38,0)::HUGEINT, -7::DECIMAL(9,1)::HUGEINT, 27::DECIMAL(18,1)::HUGEINT, 33::DECIMAL(38,1)::HUGEINT
SELECT 127::DECIMAL(3,0)::FLOAT, -17014118346046923173168730371588410572::DECIMAL(38,0)::FLOAT, -7::DECIMAL(9,1)::FLOAT, 27::DECIMAL(18,1)::FLOAT, 33::DECIMAL(38,1)::FLOAT
SELECT 127::DECIMAL(3,0)::DOUBLE, -17014118346046923173168730371588410572::DECIMAL(38,0)::DOUBLE, -7::DECIMAL(9,1)::DOUBLE, 27::DECIMAL(18,1)::DOUBLE, 33::DECIMAL(38,1)::DOUBLE
SELECT 100::TINYINT::DECIMAL(18,3), 100::TINYINT::DECIMAL(3,0), (-100)::TINYINT::DECIMAL(3,0), 0::TINYINT::DECIMAL(3,3)
SELECT 100::TINYINT::DECIMAL(38,35), 100::TINYINT::DECIMAL(9,6)
SELECT 100::SMALLINT::DECIMAL(18,3), 100::SMALLINT::DECIMAL(3,0), (-100)::SMALLINT::DECIMAL(3,0), 0::SMALLINT::DECIMAL(3,3)
SELECT 100::SMALLINT::DECIMAL(38,35), 100::SMALLINT::DECIMAL(9,6)
SELECT 100::INTEGER::DECIMAL(18,3), 100::INTEGER::DECIMAL(3,0), (-100)::INTEGER::DECIMAL(3,0), 0::INTEGER::DECIMAL(3,3)
SELECT 100::INTEGER::DECIMAL(38,35), 100::INTEGER::DECIMAL(9,6), 2147483647::INTEGER::DECIMAL(10,0), (-2147483647)::INTEGER::DECIMAL(10,0)
SELECT 100::BIGINT::DECIMAL(18,3), 100::BIGINT::DECIMAL(3,0), (-100)::BIGINT::DECIMAL(3,0), 0::BIGINT::DECIMAL(3,3)
SELECT 100::BIGINT::DECIMAL(38,35), 100::BIGINT::DECIMAL(9,6), 9223372036854775807::BIGINT::DECIMAL(19,0), (-9223372036854775807)::BIGINT::DECIMAL(19,0)
SELECT 922337203685477580::BIGINT::DECIMAL(18,0), (-922337203685477580)::BIGINT::DECIMAL(18,0)
SELECT 100::HUGEINT::DECIMAL(18,3), 100::HUGEINT::DECIMAL(3,0), (-100)::HUGEINT::DECIMAL(3,0), 0::HUGEINT::DECIMAL(3,3)
SELECT 100::HUGEINT::DECIMAL(38,35), 100::HUGEINT::DECIMAL(9,6), 17014118346046923173168730371588410572::HUGEINT::DECIMAL(38,0), (-17014118346046923173168730371588410572)::HUGEINT::DECIMAL(38,0)
SELECT 100::FLOAT::DECIMAL(18,3), 100::FLOAT::DECIMAL(3,0), (-100)::FLOAT::DECIMAL(3,0), 0::FLOAT::DECIMAL(3,3)
SELECT typeof(FIRST('0.1'::DECIMAL(4,1)))
SELECT FIRST(NULL::DECIMAL), FIRST('0.1'::DECIMAL(4,1))::VARCHAR, FIRST('4938245.1'::DECIMAL(9,1))::VARCHAR, FIRST('45672564564938245.1'::DECIMAL(18,1))::VARCHAR, FIRST('4567645908450368043562342564564938245.1'::DECIMAL(38,1))::VARCHAR
SELECT MIN(NULL::DECIMAL), MIN('0.1'::DECIMAL(4,1))::VARCHAR, MIN('4938245.1'::DECIMAL(9,1))::VARCHAR, MIN('45672564564938245.1'::DECIMAL(18,1))::VARCHAR, MIN('4567645908450368043562342564564938245.1'::DECIMAL(38,1))::VARCHAR
SELECT MAX(NULL::DECIMAL), MAX('0.1'::DECIMAL(4,1))::VARCHAR, MAX('4938245.1'::DECIMAL(9,1))::VARCHAR, MAX('45672564564938245.1'::DECIMAL(18,1))::VARCHAR, MAX('4567645908450368043562342564564938245.1'::DECIMAL(38,1))::VARCHAR
SELECT SUM(NULL::DECIMAL), SUM('0.1'::DECIMAL(4,1))::VARCHAR, SUM('4938245.1'::DECIMAL(9,1))::VARCHAR, SUM('45672564564938245.1'::DECIMAL(18,1))::VARCHAR, SUM('4567645908450368043562342564564938245.1'::DECIMAL(38,1))::VARCHAR
CREATE TABLE decimals AS SELECT i::DECIMAL(4,1) AS d1, (i * i)::DECIMAL(9,1) AS d2, (i * i * i)::DECIMAL(18,1) AS d3, (i * i * i * i)::DECIMAL(38,1) AS d4 FROM range(1000) tbl(i)
SELECT SUM(d1)::VARCHAR, SUM(d2)::VARCHAR, SUM(d3)::VARCHAR, SUM(d4)::VARCHAR FROM decimals
INSERT INTO decimals VALUES ('0.1', '0.1', '0.1', '0.1'), ('0.2', '0.2', '0.2', '0.2')
SELECT -('0.1'::DECIMAL), -('-0.1'::DECIMAL)
SELECT +('0.1'::DECIMAL), +('-0.1'::DECIMAL)
SELECT '0.1'::DECIMAL + '0.1'::DECIMAL
SELECT '0.1'::DECIMAL + 1::INTEGER
SELECT '0.5'::DECIMAL(4,4) + '0.5'::DECIMAL(4,4)
SELECT '0.5'::DECIMAL(1,1) + '100.0'::DECIMAL(3,0)
SELECT ('0.5'::DECIMAL(1,1) + 10000)::VARCHAR, ('0.54321'::DECIMAL(5,5) + 10000)::VARCHAR, ('0.5432154321'::DECIMAL(10,10) + 10000)::VARCHAR, ('0.543215432154321'::DECIMAL(15,15) + 10000::DECIMAL(20,15))::VARCHAR, ('0.54321543215432154321'::DECIMAL(20,20) + 10000)::VARCHAR, ('0.5432154321543215432154321'::DECIMAL(25,25) + 10000)::VARCHAR
SELECT '0.5'::DECIMAL(1,1) + 1::TINYINT, '0.5'::DECIMAL(1,1) + 1::SMALLINT, '0.5'::DECIMAL(1,1) + 1::INTEGER, '0.5'::DECIMAL(1,1) + 1::BIGINT, '0.5'::DECIMAL(1,1) + 1::HUGEINT
SELECT '0.5'::DECIMAL(1,1) + -1::TINYINT, '0.5'::DECIMAL(1,1) + -1::SMALLINT, '0.5'::DECIMAL(1,1) + -1::INTEGER, '0.5'::DECIMAL(1,1) + -1::BIGINT, '0.5'::DECIMAL(1,1) + -1::HUGEINT
SELECT '0.5'::DECIMAL(1,1) - 1::TINYINT, '0.5'::DECIMAL(1,1) - 1::SMALLINT, '0.5'::DECIMAL(1,1) - 1::INTEGER, '0.5'::DECIMAL(1,1) - 1::BIGINT, '0.5'::DECIMAL(1,1) - 1::HUGEINT
SELECT '0.5'::DECIMAL(1,1) - -1::TINYINT, '0.5'::DECIMAL(1,1) - -1::SMALLINT, '0.5'::DECIMAL(1,1) - -1::INTEGER, '0.5'::DECIMAL(1,1) - -1::BIGINT, '0.5'::DECIMAL(1,1) - -1::HUGEINT
CREATE TABLE decimals(d DECIMAL(3, 2))
SELECT [1.33, 10.0]
SELECT [0.1, 1.33, 10.0, 9999999.999999999]
SELECT [99999999999999999999999999999999999.9, 9.99999999999999999999999999999999999]
CREATE TABLE foo (my_struct STRUCT(my_double DOUBLE)[])
INSERT INTO foo VALUES ([{'my_double': 1.33}, {'my_double': 10.0}])
SELECT 1.0::DECIMAL(4,3)::DECIMAL(2,1), 1.0::DECIMAL(4,3)::DECIMAL(9,1), 1.0::DECIMAL(4,3)::DECIMAL(18,1), 1.0::DECIMAL(4,3)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(9,8)::DECIMAL(2,1), 1.0::DECIMAL(9,8)::DECIMAL(9,1), 1.0::DECIMAL(9,8)::DECIMAL(18,1), 1.0::DECIMAL(9,8)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(18,17)::DECIMAL(2,1), 1.0::DECIMAL(18,17)::DECIMAL(9,1), 1.0::DECIMAL(18,17)::DECIMAL(18,1), 1.0::DECIMAL(18,17)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(38,37)::DECIMAL(2,1), 1.0::DECIMAL(38,37)::DECIMAL(9,1), 1.0::DECIMAL(38,37)::DECIMAL(18,1), 1.0::DECIMAL(38,37)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(3,1)::DECIMAL(18,2), 1.0::DECIMAL(3,1)::DECIMAL(38,2)
SELECT 1.0::DECIMAL(3,1)::DECIMAL(2,1), 1.0::DECIMAL(3,1)::DECIMAL(9,1), 1.0::DECIMAL(3,1)::DECIMAL(18,1), 1.0::DECIMAL(3,1)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(9,1)::DECIMAL(2,1), 1.0::DECIMAL(9,1)::DECIMAL(8,1), 1.0::DECIMAL(9,1)::DECIMAL(18,1), 1.0::DECIMAL(9,1)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(18,1)::DECIMAL(2,1), 1.0::DECIMAL(18,1)::DECIMAL(8,1), 1.0::DECIMAL(18,1)::DECIMAL(17,1), 1.0::DECIMAL(18,1)::DECIMAL(38,1)
SELECT 1.0::DECIMAL(38,1)::DECIMAL(2,1), 1.0::DECIMAL(38,1)::DECIMAL(8,1), 1.0::DECIMAL(38,1)::DECIMAL(17,1), 1.0::DECIMAL(38,1)::DECIMAL(37,1)
select cast(9.49 as decimal(1,0))
select cast(-9.01 as decimal(1,0))
SELECT '1e3'::DECIMAL, '1e-1'::DECIMAL, '.1e3'::DECIMAL, '0.1e3'::DECIMAL
SELECT '-1e3'::DECIMAL, '-0.1e3'::DECIMAL, '-.1e-1'::DECIMAL, '-0.1e-1'::DECIMAL
SELECT '0e1'::DECIMAL, '-0e1'::DECIMAL, '00000e1'::DECIMAL, '-00000e1'::DECIMAL
SELECT '1e-100'::DECIMAL
SELECT '1e-9999'::DECIMAL
SELECT '1E3'::DECIMAL(4,0)
SELECT '1e8'::DECIMAL(9,0)
SELECT '1e17'::DECIMAL(18,0)
SELECT '1e37'::DECIMAL(38,0)
CREATE TABLE decimals(d DECIMAL(18,1))
INSERT INTO decimals VALUES (99000000000000000.0)
SELECT d+1 FROM decimals
SELECT -1-d FROM decimals
SELECT 1*d FROM decimals
SELECT TRY_CAST(1000 AS DECIMAL(3,0))
SELECT TRY_CAST(100 AS DECIMAL(2,0))
SELECT TRY_CAST('100' AS DECIMAL(2,0))
SELECT TRY_CAST('100'::DOUBLE AS DECIMAL(2,0))
SELECT TRY_CAST(100::DECIMAL(3,0) AS DECIMAL(2,0))
SELECT TRY_CAST(10000::DECIMAL(5,0) AS DECIMAL(2,0))
SELECT TRY_CAST(1000000000::DECIMAL(10,0) AS DECIMAL(2,0))
SELECT TRY_CAST(1000000000::DECIMAL(20,0) AS DECIMAL(2,0))
SELECT TRY_CAST(1000000 AS DECIMAL(5,0))
SELECT TRY_CAST('100000' AS DECIMAL(5,0))
SELECT TRY_CAST('100000'::DOUBLE AS DECIMAL(5,0))
SELECT TRY_CAST(100000::DECIMAL(6,0) AS DECIMAL(5,0))
SELECT 42.1, -10239814.1, 1049185157.12345, 102398294123451814.12345, -49238409238403918140294812084.12490812490
SELECT typeof(42.1), typeof(-10239814.1), typeof(1049185157.12345), typeof(102398294123451814.12345), typeof(-49238409238403918140294812084.12490812490)
SELECT 42., 42e3, 4.23e1, 10e20, .34, - 2.3
SELECT typeof(42.), typeof(42e3), typeof(4.23e1), typeof(10e20), typeof(.34), typeof(-2.3), typeof(10e100)
SELECT typeof('0.1'::DECIMAL)
SELECT '0.1'::DECIMAL::VARCHAR, '922337203685478.758'::DECIMAL::VARCHAR
SELECT '-0.1'::DECIMAL::VARCHAR, '-922337203685478.758'::DECIMAL::VARCHAR
SELECT ' 7 '::DECIMAL::VARCHAR, '9.'::DECIMAL::VARCHAR, '.1'::DECIMAL::VARCHAR
SELECT '0.123456789'::DECIMAL::VARCHAR, '-0.123456789'::DECIMAL::VARCHAR
SELECT '0.1'::DECIMAL(3, 0)::VARCHAR
SELECT '123.4'::DECIMAL(9)::VARCHAR
SELECT '0.1'::DECIMAL(3, 3)::VARCHAR, '-0.1'::DECIMAL(3, 3)::VARCHAR
select '0.1'::decimal::decimal::decimal
select '123.4'::DECIMAL(4,1)::VARCHAR
select '2.001'::DECIMAL(4,3)::VARCHAR
select '123456.789'::DECIMAL(9,3)::VARCHAR
CREATE TABLE decimals(i DECIMAL(38,1))
CREATE TABLE decimals2(i DECIMAL(38,1))
INSERT INTO decimals VALUES (4642275147320176030871715840)
INSERT INTO decimals2 VALUES (4642275147320176030871715840)
select count(*) from decimals inner join decimals2 on (decimals.i = decimals2.i)
create table q (big decimal (38,10))
insert into q (big ) values (9999999999999999899999999999.9999999999)
insert into q (big ) values (-9999999999999999899999999999.9999999999)
SELECT * FROM q
CREATE TABLE from_values AS VALUES (1000000), (10.0000000005)
SELECT * FROM from_values
CREATE TABLE from_list AS SELECT [1000000, 10.0000000005]
SELECT * FROM from_list
select '+1e-1'::DECIMAL(38,3)
select '+1234.56789e-1'::DECIMAL(38,0)
select '+1234.56789e-1'::DECIMAL(38,5)
select +1234.56789e-1::DECIMAL(38,5)
INSERT INTO decimals VALUES ('0.1'), ('0.2')
SELECT * FROM decimals
SELECT * FROM decimals ORDER BY d DESC
SELECT * FROM decimals WHERE d='0.1'::DECIMAL(3,2)
SELECT * FROM decimals WHERE d>='0.1'::DECIMAL(3,2)
SELECT * FROM decimals WHERE d='0.1'::DECIMAL(9,5)
SELECT * FROM decimals WHERE d >= '0.1'::DECIMAL(9,5) ORDER BY 1
INSERT INTO decimals VALUES ('0.11'), ('0.21')
SELECT * FROM decimals WHERE d = '0.1'::DECIMAL(9,1)
SELECT * FROM decimals WHERE d > '0.1'::DECIMAL(9,1) ORDER BY 1
DELETE FROM decimals WHERE d <> d::DECIMAL(9,1)
SELECT ABS('-0.1'::DECIMAL), ABS('0.1'::DECIMAL), ABS(NULL::DECIMAL)
select '1.023450000001'::DECIMAL(5,4)
select '1.234499999'::DECIMAL(4,3)
select '1.23499999'::DECIMAL(4,3)
select '1.234499999'::DECIMAL(5,4)
select '-1.023450000001'::DECIMAL(5,4)
select '-1.234499999'::DECIMAL(4,3)
select '-1.23499999'::DECIMAL(4,3)
select '-1.234499999'::DECIMAL(5,4)
CREATE TABLE decs(i DEC(), j DEC)
INSERT INTO decs VALUES (0176030871715840, 2.2)
SELECT * FROM decs
SELECT 1.25::FLOAT::DEC, 1.25::FLOAT::DEC()
CREATE TABLE decimals(i DECIMAL(), j DECIMAL)
INSERT INTO decimals VALUES (0176030871715840, 2.2)
SELECT 1.25::FLOAT::DECIMAL, 1.25::FLOAT::DECIMAL()
CREATE TYPE my_int AS INT
CREATE TYPE my_int_list AS my_int[]
SELECT [42]::my_int_list
CREATE TYPE alias AS VARCHAR
DROP TYPE alias
DROP TYPE IF EXISTS alias
CREATE TYPE str_alias as VARCHAR
SELECT upper('hello'::str_alias)
CREATE TYPE MAPPOINT AS MAP(INTEGER,INTEGER)
CREATE TABLE a(b MAPPOINT)
SELECT * FROM a
INSERT INTO a VALUES (MAP([1], [2])), (MAP([1, 2, 3], [4, 5, 6]))
CREATE TYPE POINT AS STRUCT(i INTEGER, j INTEGER)
CREATE TABLE a(b POINT)
INSERT INTO a VALUES ({'i': 3, 'j': 4})
INSERT INTO a VALUES (NULL)
INSERT INTO a VALUES (ROW(2, 3))
INSERT INTO a VALUES (ROW(3, NULL)), (ROW(NULL, 4))
CREATE TYPE foobar AS ENUM( 'Foo', 'Bar' )
CREATE TYPE top_nest AS STRUCT( foobar FOOBAR )
CREATE TABLE failing ( top_nest TOP_NEST )
insert into failing VALUES ( {'foobar': 'Foo'} )
SELECT top_nest FROM failing
CREATE TABLE person ( name text, current_alias alias )
INSERT INTO person VALUES ('Moe', 'happy')
select * from person
INSERT INTO person VALUES ('Pedro', 'ok')
INSERT INTO person VALUES ('Mark', 'sad')
select * from person where current_alias = 'sad'
select * from person where current_alias > 'ok'
CREATE TABLE pets ( name text, current_alias alias )
INSERT INTO pets VALUES ('Anne', 'happy')
INSERT INTO pets VALUES ('Oogie Boogie', 'ok')
INSERT INTO pets VALUES ('Mr. Fluffles McFluffingstein', NULL)
select * from pets
CREATE SCHEMA my_schema
CREATE TYPE my_schema.my_type AS STRUCT ( a int, b int )
CREATE TABLE my_schema.tbl ( c0 my_schema.my_type )
CREATE TABLE main.tbl ( c0 my_schema.my_type )
select ( 2::bit & 2::bit ) = 2::bit as b
FROM ( SELECT ( 2::bit & 2::bit ) AS a, 2::bit AS b, (a = b) AS '(a = b)', ) SELECT a, b, a = b, "(a = b)"
SELECT ('0101011'::BIT)
SELECT ('0101011'::BITSTRING)
CREATE TABLE bits (b bit)
INSERT INTO bits VALUES('101011010'), ('111'), ('1010010101111111001101')
SELECT * FROM bits
INSERT INTO bits VALUES('0'), ('1'), ('0000000000000000000111')
SELECT * FROM bits WHERE b = '111'
SELECT ('0101011'::BIT(10))
SELECT NULL::BIT
DELETE FROM bits
INSERT INTO bits VALUES (NULL)
SELECT TRY_CAST('101' AS BIT)
INSERT INTO bits VALUES('101111011010'), ('110001100100'), ('101001000110')
SELECT '10101'::BIT & '10001'::BIT
SELECT '1000001101011111'::BIT & '1100101101000011'::BIT
SELECT '01011'::BIT & '11000'::BIT
SELECT b & '011100011011'::BIT FROM bits
SELECT '10001111'::BIT | '00011011'::BIT
SELECT '1011'::BIT | '0001'::BIT
SELECT '10000010011101011111'::BIT | '11001011010011100011'::BIT
SELECT b | '011100011011'::BIT FROM bits
SELECT xor('101'::BIT, '001'::BIT)
SELECT xor('10000010011101011111'::BIT, '11001011010111000011'::BIT)
SELECT xor(b, '011100011011'::BIT) FROM bits
select bitstring('1', 6) from range(100000) group by 1
INSERT INTO bits VALUES('101001111'), ('00111'), ('100101010110000000000001'), ('111111010100')
SELECT bit_length('1010111111101010011101011'::BIT)
SELECT bit_length('0'::BIT)
SELECT bit_length(b) FROM bits
SELECT octet_length('10101111111010100111010'::BIT)
SELECT octet_length('0'::BIT)
SELECT octet_length(b) FROM bits
SELECT get_bit('101010101010101010'::BIT, 6)
SELECT get_bit('110'::BIT, 2)
SELECT get_bit('1010000'::BIT, 0)
SELECT get_bit(b, 4) FROM bits
SELECT set_bit('0101010101010101010'::BIT, 2, 1)
CREATE TABLE hugeints(g INTEGER, h UHUGEINT)
INSERT INTO hugeints VALUES (1, 42), (2, 1267650600228229401496703205376), (2, 0), (1, '8')
SELECT MIN(h), MAX(h), SUM(h), FIRST(h), LAST(h) FROM hugeints
SELECT g, MIN(h), MAX(h), SUM(h), FIRST(h), LAST(h) FROM hugeints GROUP BY g ORDER BY 1
PRAGMA threads=1
SELECT FIRST(h), LAST(h) FROM hugeints
SELECT g, FIRST(h), LAST(h) FROM hugeints GROUP BY g ORDER BY 1
SELECT ~(-50::UHUGEINT), -(-(50::UHUGEINT))
SELECT -(0::UHUGEINT)
SELECT 42::UHUGEINT + 42::UHUGEINT
SELECT '100000000000000000000'::UHUGEINT + '100000000000000000000'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT - 10::UHUGEINT + 10::UHUGEINT
SELECT 100::UHUGEINT - 42::UHUGEINT, 3::UHUGEINT - 2::UHUGEINT
SELECT 100::UHUGEINT * 50::UHUGEINT
SELECT '1701411834604692317'::UHUGEINT * '2'::UHUGEINT, '100000000000000000000'::UHUGEINT * '1000000000000000000'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT * 1::UHUGEINT
SELECT 100::UHUGEINT // 20::UHUGEINT, 90::UHUGEINT // 20::UHUGEINT
SELECT 100::UHUGEINT // 0::UHUGEINT
SELECT '100000000000000000000000000000000000000'::UHUGEINT // '10000000000000'::UHUGEINT, '100000000000000000000000000000000000000'::UHUGEINT // '2'::UHUGEINT
SELECT 10000000000000000000::UHUGEINT + 100::TINYINT, 10000000000000000000::UHUGEINT + 100::SMALLINT, 10000000000000000000::UHUGEINT + 100::INTEGER, 10000000000000000000::UHUGEINT + 100::BIGINT
SELECT 100::UHUGEINT + 0.5
SELECT COS(100::UHUGEINT)
SELECT CONCAT('hello number ', 100::UHUGEINT)
SELECT '7'::UHUGEINT, '130'::UHUGEINT, '924829852'::UHUGEINT
SELECT '0'::UHUGEINT, '-0'::UHUGEINT
SELECT '10000000000000000000000000000'::UHUGEINT
SELECT '1267650600228229401496703205376'::UHUGEINT, '340282366920938463463374607431768211455'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT, '0'::UHUGEINT
SELECT 42::TINYINT::UHUGEINT, 42::SMALLINT::UHUGEINT, 42::INTEGER::UHUGEINT, 42::BIGINT::UHUGEINT, 42::FLOAT::UHUGEINT, 42::DOUBLE::UHUGEINT
SELECT 42::UHUGEINT::TINYINT, 42::UHUGEINT::SMALLINT, 42::UHUGEINT::INTEGER, 42::UHUGEINT::BIGINT, 42::UHUGEINT::FLOAT, 42::UHUGEINT::DOUBLE
SELECT 127::UHUGEINT::TINYINT
SELECT 32767::UHUGEINT::SMALLINT
SELECT 2147483647::UHUGEINT::INTEGER
SELECT 9223372036854775807::UHUGEINT::BIGINT
SELECT typeof(10), typeof(10000000000), typeof(170141183460469231731687303715884105727), typeof(170141183460469231731687303715884105728), typeof(170141183460469231731687303715884105728000)
select '170141183460469231731687303715884105700e0'::UHUGEINT
select '340282366920938463463374607431768211455e0'::UHUGEINT
select 5.4321e4::UHUGEINT
select (0.00000000000000000000002e+44)::UHUGEINT
select '3.4e38'::UHUGEINT
select abs(1::UHUGEINT), abs('1329227995784915872903807060280344576'::UHUGEINT), abs(0::UHUGEINT)
select sign(1::UHUGEINT), sign(0::UHUGEINT)
select round(1::UHUGEINT, 0), round('1329227995784915872903807060280344576'::UHUGEINT, 0), round(0::UHUGEINT, 0)
select floor(1::UHUGEINT), floor('1329227995784915872903807060280344576'::UHUGEINT), floor(0::UHUGEINT)
select ceil(1::UHUGEINT), ceil('1329227995784915872903807060280344576'::UHUGEINT), ceil(0::UHUGEINT)
select LEAST(1::UHUGEINT, '1329227995784915872903807060280344576'::UHUGEINT, 0::UHUGEINT)
select GREATEST(1::UHUGEINT, '1329227995784915872903807060280344576'::UHUGEINT, 0::UHUGEINT)
SELECT NULL::UHUGEINT
CREATE TABLE hugeints(id INTEGER, h UHUGEINT)
INSERT INTO hugeints VALUES (1, NULL), (1, 1), (1, 2)
SELECT id, FIRST(h), LAST(h) FROM hugeints WHERE h IS NULL GROUP BY id
SELECT h, SUM(id) FROM hugeints GROUP BY h ORDER BY 1
SELECT id, h1.h, h2.h FROM hugeints h1 JOIN hugeints h2 USING (id) WHERE h1.h IS NULL AND h2.h IS NULL
SELECT (SELECT h1.h) FROM hugeints h1 ORDER BY 1
SELECT h1.h NOT IN (SELECT h1.h+h2.h FROM hugeints h2) FROM hugeints h1 ORDER BY 1
CREATE TABLE uhugeints(h UHUGEINT)
INSERT INTO uhugeints VALUES (42::UHUGEINT), ('1267650600228229401496703205376'::UHUGEINT)
SELECT h::UINTEGER FROM uhugeints WHERE h < 100::UINTEGER
SELECT COUNT(*) FROM uhugeints WHERE h = 42::UHUGEINT
SELECT COUNT(*) FROM uhugeints WHERE h <> '1267650600228229401496703205376'::UHUGEINT
SELECT COUNT(*) FROM uhugeints WHERE h < '1267650600228229401496703205376'::UHUGEINT
SELECT COUNT(*) FROM uhugeints WHERE h <= '1267650600228229401496703205376'::UHUGEINT
SELECT COUNT(*) FROM uhugeints WHERE h > '1267650600228229401496703205375'::UHUGEINT
SELECT COUNT(*) FROM uhugeints WHERE h >= 42::UHUGEINT
SELECT * FROM uhugeints JOIN uhugeints2 USING (h)
SELECT * FROM uhugeints t1 JOIN uhugeints2 t2 ON t1.h <> t2.h
SELECT * FROM uhugeints t1 JOIN uhugeints2 t2 ON t1.h >= t2.h ORDER BY 1 LIMIT 2
select 340282366920938463463374607431768211455
select typeof(340282366920938463463374607431768211455)
select 340282366920938463463374607431768211456
select typeof(340282366920938463463374607431768211456)
SELECT 251658240::UHUGEINT * 251658240::UHUGEINT
SELECT 251658240::UHUGEINT * 1080863910568919040::UHUGEINT
SELECT 251658240::UHUGEINT * 4642275147320176030871715840::UHUGEINT
SELECT 1080863910568919040::UHUGEINT * 251658240::UHUGEINT
SELECT 1080863910568919040::UHUGEINT * 1080863910568919040::UHUGEINT
SELECT 4642275147320176030871715840::UHUGEINT * 251658240::UHUGEINT
SELECT 170141183460469231731687303715884105727::UHUGEINT * 2::UHUGEINT
SELECT 19807040628566084398385987583::UHUGEINT * 8589934592::UHUGEINT
SELECT 36893488147419103231::UHUGEINT * 4611686018427387904::UHUGEINT
SELECT 2::UHUGEINT * 170141183460469231731687303715884105727::UHUGEINT
SELECT 8589934592::UHUGEINT * 19807040628566084398385987583::UHUGEINT
SELECT 4611686018427387904::UHUGEINT * 36893488147419103231::UHUGEINT
SELECT TRY_CAST('340282366920938463463374607431768211456' AS UHUGEINT)
SELECT TRY_CAST('340282366920938463463374607431768211456'::DOUBLE AS UHUGEINT)
SELECT TRY_CAST('-1' AS UHUGEINT)
CREATE TABLE unsigned(a UTINYINT,b USMALLINT, c UINTEGER, d UBIGINT)
INSERT INTO unsigned VALUES (1,1,1,1), (2,2,2,2)
select * from unsigned
SELECT (20)::UTINYINT + (200)::USMALLINT
SELECT (20)::UBIGINT + (200)::UBIGINT
SELECT (200)::UTINYINT * (200)::USMALLINT
SELECT (200)::UBIGINT * (200)::UBIGINT
SELECT (200)::UTINYINT - (20)::USMALLINT
SELECT 100::UTINYINT // 20::UTINYINT, 90::UTINYINT // 20::UTINYINT
SELECT 100::UTINYINT // 20::UBIGINT, 90::UTINYINT // 20::UBIGINT
SELECT 100::UTINYINT // 0::UTINYINT
SELECT 100::UTINYINT % 20::UTINYINT, 90::UTINYINT % 20::UTINYINT
SELECT 200::UTINYINT + 0.5
SELECT COS(100::UTINYINT)
SELECT CONCAT('hello number ', 100::UTINYINT)
SELECT 100000000::INTEGER + 100::USMALLINT
SELECT 100::USMALLINT + 0.5
SELECT COS(100::USMALLINT)
SELECT CONCAT('hello number ', 100::USMALLINT)
SELECT 100000000::INTEGER + 100::UINTEGER
SELECT 100::UINTEGER + 0.5
SELECT COS(100::UINTEGER)
SELECT CONCAT('hello number ', 100::UINTEGER)
SELECT 100000000::INTEGER + 100::UBIGINT
SELECT '7'::UTINYINT, '130'::UTINYINT, '255'::UTINYINT
SELECT '7'::USMALLINT, '130'::USMALLINT, '65535'::USMALLINT
SELECT '7'::UINTEGER, '130'::UINTEGER, '4294967295'::UINTEGER
SELECT '7'::UBIGINT, '130'::UBIGINT, '18446744073709551615'::UBIGINT
SELECT '0'::UTINYINT, '-0'::UTINYINT
SELECT 42::TINYINT::UTINYINT, 42::SMALLINT::UTINYINT, 42::INTEGER::UTINYINT, 42::BIGINT::UTINYINT, 42::FLOAT::UTINYINT, 42::DOUBLE::UTINYINT
SELECT 42::TINYINT::USMALLINT, 42::SMALLINT::USMALLINT, 42::INTEGER::USMALLINT, 42::BIGINT::USMALLINT, 42::FLOAT::USMALLINT, 42::DOUBLE::USMALLINT
SELECT 42::TINYINT::UINTEGER, 42::SMALLINT::UINTEGER, 42::INTEGER::UINTEGER, 42::BIGINT::UINTEGER, 42::FLOAT::UINTEGER, 42::DOUBLE::UINTEGER
SELECT 42::TINYINT::UBIGINT, 42::SMALLINT::UBIGINT, 42::INTEGER::UBIGINT, 42::BIGINT::UBIGINT, 42::FLOAT::UBIGINT, 42::DOUBLE::UBIGINT
SELECT (9223372036854775807)::BIGINT::UBIGINT
SELECT (9223372036854775808)::HUGEINT::UBIGINT
SELECT (9223372036854775808)::UHUGEINT::UBIGINT
select []::uint16[]
select []::uint32[]
select []::uint64[]
SELECT alias('5 days'::INTERVAL DAY TO SECOND)
SELECT interval 2 days
SELECT interval (2) day
SELECT interval (1+1) days
SELECT interval '2' days
SELECT to_years(2), to_months(2), to_days(2), to_hours(2), to_minutes(2), to_seconds(2)
SELECT interval (i) day from range(1, 4) tbl(i)
SELECT interval (i + 1) day from range(1, 4) tbl(i)
SELECT interval 2 years, interval 2 year
SELECT interval 2 months, interval 2 month
SELECT interval 2 days, interval 2 day
SELECT interval 2 hours, interval 2 hour
SELECT interval 2 minutes, interval 2 minute
SELECT cast('00:00:' as interval)
SELECT cast(NULL as interval)
SELECT try_cast(' ' as interval)
SELECT try_cast('AAAA' as interval)
SELECT try_cast('00:00:' as interval)
SELECT try_cast('3 doopiedoos' as interval)
SELECT try_cast('3 years 2 doy' as interval)
SELECT try_cast(NULL as interval)
SELECT TRY_CAST('42 seconds' AS INTERVAL), TRY_CAST('42 ' AS INTERVAL), TRY_CAST('42' AS INTERVAL), TRY_CAST(' 42' AS INTERVAL)
SELECT TRY_CAST('42x' AS INTERVAL)
SELECT TRY_CAST('42.5' AS INTERVAL), TRY_CAST('42.5 ' AS INTERVAL), '42.5'::INTERVAL
SELECT INTERVAL '2 years'
SELECT INTERVAL '2 years'::VARCHAR
SELECT INTERVAL '2Y 1 M'
SELECT INTERVAL '2Y 1 month 1 M 3S 20mS 16uS'
SELECT INTERVAL '2Y 1 month 02:01:03.020016'
SELECT INTERVAL '2Y 1 month 1M 3S 20mS 16uS'::VARCHAR
SELECT INTERVAL '2 yr 1 mon 1 min 3 sec 20 msec 16 usec'
SELECT INTERVAL '2 yrs 1 mons 1 mins 3 secs 20 msecs 16 usecs'
SELECT INTERVAL '-2Y 4 days 5 Hours 1 MinUteS 3S 20mS 16uS'
SELECT INTERVAL '-2Y 4 days 5 Hours 1 MinUteS 3S 20mS 16uS'::VARCHAR
SELECT INTERVAL '-2yr 4 d 5 hr 1 min 3 second 20 msecond 16 usecond'
SELECT INTERVAL '-2yrs 4 d 5 hrs 1 mins 3 seconds 20 mseconds 16 useconds'::VARCHAR
SELECT DATE '1992-03-01' + INTERVAL '1' YEAR
SELECT DATE '1992-03-01' + INTERVAL '0' MONTH
SELECT DATE '1992-03-01' - INTERVAL '0' MONTH
SELECT DATE '1992-03-01' + INTERVAL '1' MONTH
SELECT DATE '1992-03-01' - INTERVAL '1' MONTH
SELECT DATE '1992-03-01' + INTERVAL '2' MONTH
SELECT DATE '1992-03-01' - INTERVAL '2' MONTH
SELECT DATE '1992-03-01' + INTERVAL '3' MONTH
SELECT DATE '1992-03-01' - INTERVAL '3' MONTH
SELECT DATE '1992-03-01' + INTERVAL '4' MONTH
SELECT DATE '1992-03-01' - INTERVAL '4' MONTH
SELECT DATE '1992-03-01' + INTERVAL '5' MONTH
WITH d(y) AS ( SELECT UNNEST(range( '2023-05-11 4:00:00'::TIMESTAMP, '2023-05-11 4:00:00'::TIMESTAMP + TO_DAYS(7), TO_HOURS(6) )) ) SELECT y, y - ('2023-05-11 4:00:00'::TIMESTAMP) AS x FROM d WHERE x BETWEEN TO_HOURS(-44) AND TO_HOURS(44)
WITH d(y) AS ( SELECT UNNEST(range( '2023-05-11 4:00:00'::TIMESTAMP, '2023-05-11 4:00:00'::TIMESTAMP + TO_DAYS(7), TO_HOURS(6) )) ) SELECT y, y - ('2023-05-11 4:00:00'::TIMESTAMP) AS x FROM d WHERE x >= TO_HOURS(-44) AND x <= TO_HOURS(44)
SELECT INTERVAL '30' DAY > INTERVAL '1' MONTH
SELECT INTERVAL '30' DAY = INTERVAL '1' MONTH
SELECT INTERVAL '30' DAY >= INTERVAL '1' MONTH
SELECT INTERVAL '31' DAY > INTERVAL '1' MONTH
SELECT INTERVAL '1' HOUR < INTERVAL '1' DAY
SELECT INTERVAL '30' HOUR <= INTERVAL '1' DAY
SELECT INTERVAL '1' HOUR = INTERVAL '1' HOUR
SELECT INTERVAL '1' YEAR = INTERVAL '12' MONTH
select interval '28 days 432000 seconds' = interval '1 month 3 days'
CREATE TABLE issue14384(i INTERVAL)
INSERT INTO issue14384(i) VALUES ('2 years 3 months'), ('-1734799452 DAYS'), ('2 DAYS'), ('13 days'), ('1 month'), ('3 days'),
SELECT i FROM issue14384 ORDER BY ALL
CREATE TABLE interval (t INTERVAL)
INSERT INTO interval VALUES (INTERVAL '20' DAY), (INTERVAL '1' YEAR), (INTERVAL '1' MONTH)
SELECT COUNT(DISTINCT t) FROM interval
UPDATE interval SET t=INTERVAL '1' MONTH WHERE t=INTERVAL '20' DAY
SELECT * FROM interval i1 JOIN interval i2 USING (t) ORDER BY 1
SELECT * FROM interval i1 JOIN interval i2 ON (i1.t <> i2.t) ORDER BY 1
SELECT * FROM interval i1 JOIN interval i2 ON (i1.t > i2.t) ORDER BY 1
SELECT t, row_number() OVER (PARTITION BY t ORDER BY t) FROM interval ORDER BY 1, 2
INSERT INTO timestamps VALUES ('1993-08-14 00:00:00'), ('1993-08-15 01:01:02'), ('1993-08-16 00:00:00')
SELECT * FROM timestamps WHERE ts >= date '1993-08-15'
DROP TABLE timestamps
select '1969-01-01'::date
select '2370-01-01'::date
select '5877642-06-25 (BC)'::date
select '290308-01-01 (BC)'::date::timestamp
select '5877642-06-25 (BC)'::date + 1
select '290309-12-22 (BC)'::date + interval (1) day
select '290309-12-22 (BC)'::date + interval (1) month
select '5881580-07-10'::date
select '294247-01-10'::date::timestamp
select '5881580-07-10'::date - 1
select '294247-01-10'::date - interval (1) day
select '294247-01-10'::date - interval (1) month
SELECT '1992-01-01'::DATE::VARCHAR == '1992-01-01'
SELECT '1992-09-20'::DATE::VARCHAR == '1992-09-20'
SELECT '1992-02-29'::DATE::VARCHAR == '1992-02-29'
SELECT '3600-02-29'::DATE::VARCHAR == '3600-02-29'
SELECT '0030-01-01'::DATE::VARCHAR == '0030-01-01'
SELECT '30000-01-01'::DATE::VARCHAR == '30000-01-01'
SELECT '1969-01-01'::DATE::VARCHAR == '1969-01-01'
SELECT '1970-01-01'::DATE::VARCHAR == '1970-01-01'
SELECT '2369-01-01'::DATE::VARCHAR == '2369-01-01'
SELECT '2370-01-01'::DATE::VARCHAR == '2370-01-01'
SELECT '2371-01-01'::DATE::VARCHAR == '2371-01-01'
SELECT '-1000-01-01'::DATE::VARCHAR == '1001-01-01 (BC)'
select try_cast('' as date)
select try_cast(' ' as date)
select try_cast('1111' as date)
select try_cast(' 1111 ' as date)
select try_cast('1111-' as date)
select try_cast('1111-11' as date)
select try_cast('1111-11-' as date)
select try_cast('1111-111-1' as date)
select try_cast('1111-11-111' as date)
select try_cast('1111-11-11' as date)
select try_cast('1111-11-11 (bc)' as date)
select try_cast('2001-02-29' as date)
CREATE TABLE dates(i DATE)
INSERT INTO dates VALUES ('-1993-08-14'), (NULL)
SELECT * FROM dates
SELECT year(i) FROM dates
SELECT cast(i AS VARCHAR) FROM dates
SELECT DATE '0000-01-01'
SELECT DATE '1992-01-01 (BC)'
SELECT DATE '-1992-01-01'
CREATE TABLE bc_dates AS SELECT date '0020-01-01' - interval (i) years AS d from range(0, 40) tbl(i)
SELECT d, d::VARCHAR FROM bc_dates ORDER BY 1
INSERT INTO dates VALUES ('1993-08-14'), (NULL)
SELECT i + 5 FROM dates
SELECT i - 5 FROM dates
SELECT (i + 5) - i FROM dates
SELECT '2021-03-01'::DATE, DATE '2021-03-01', DATE('2021-03-01')
CREATE TABLE timestamp(i TIMESTAMP)
INSERT INTO dates VALUES ('1993-08-14')
INSERT INTO timestamp VALUES ('1993-08-14 00:00:01')
select count(*) from dates inner join timestamp on (timestamp.i::DATE = dates.i)
INSERT INTO dates VALUES ('1992-02-29')
INSERT INTO dates VALUES ('2000-02-29')
INSERT INTO dates VALUES ('1900-1-1')
SELECT 'hello'::ENUM('world', 'hello')
CREATE TABLE test AS SELECT 'hello'::ENUM('world', 'hello') AS h
SELECT * FROM test
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TYPE mood_2 AS ENUM ('1', '2', '3')
CREATE TABLE m ( m mood )
CREATE TABLE m_2 ( m mood_2 )
insert into m_2 values ('1')
CREATE TYPE mood_2 AS ENUM ('very sad', 'very ok', 'very happy')
CREATE TABLE m ( m mood, m_2 mood_2 )
insert into m values ('sad', 'very sad')
select * from m where m = ''::VARCHAR
SELECT m='' FROM m
PREPARE v1 AS SELECT m=? FROM m
EXECUTE v1('')
SELECT * FROM m WHERE m=m_2
CALL DBGEN(sf=0.01)
create type orderkey_enum as enum (Select (l_orderkey/4)::VARCHAR from lineitem)
create table t2 (c1 orderkey_enum)
insert into t2 (select (l_orderkey/4)::VARCHAR from lineitem)
CREATE TYPE l_comment_enum as ENUM(select l_comment from lineitem)
CREATE TABLE lineitem2 (comment l_comment_enum)
create table a (a string, b mood)
insert into a values (NULL, 'happy')
insert into a values ('woof', NULL)
insert into a values (NULL, NULL)
insert into a values ('quack', 'ok')
insert into a values ('moo', 'sad')
select coalesce(a, b) from a
CREATE TYPE onomatopoeia AS ENUM ('woof', 'quack', 'moo')
create table b (a onomatopoeia, b mood)
insert into b values (NULL, 'happy')
insert into b values ('woof', NULL)
insert into b values (NULL, NULL)
CREATE TABLE person ( name text )
insert into person values ('Pedro'), ('Mark'), ('Hannes'), ('Pedro'), ('Pedro'), ('Mark')
CREATE TYPE name_enum AS ENUM ('Pedro', 'Mark')
DROP TYPE name_enum
CREATE TYPE name_enum AS ENUM ('Pedro', 'Mark', 'Hannes')
select typeof(name) from person limit 1
ALTER TABLE person ALTER name TYPE text
CREATE TYPE IF NOT EXISTS mood AS ENUM ('sad', 'ok', 'happy')
select 'happy'::mood
CREATE TYPE bla AS ENUM ()
DROP TYPE bla
CREATE TYPE mood_2 AS ENUM ('sad','Sad','SAD')
ALTER TYPE mood ADD VALUE 'depressive'
ALTER TYPE mood REMOVE VALUE 'depressive'
DROP TYPE mood
DROP TYPE mood_2
DROP TYPE IF EXISTS mood
select ['happy']::mood[]
select [NULL,'happy',NULL]::mood[]
CREATE TYPE E1 AS ENUM ('v1', 'v2')
CREATE TABLE t1 (v E1)
INSERT INTO t1 VALUES ('v1')
SELECT typeof(CASE WHEN 1 THEN v END) FROM t1
CREATE TABLE person ( name text, current_mood mood )
INSERT INTO person VALUES ('Pedro', 'ok'), ('Mark', 'sad'),('Moe', 'happy'), ('Diego', NULL)
select current_mood::varchar from person
CREATE TYPE years AS ENUM ('2001', '2006', '2012', '2018')
CREATE TABLE albums ( name text, year_release years )
INSERT INTO albums VALUES ('Tenacious D', '2001'), ('The Pick of Destiny', '2006'),('Rize of the Fenix', '2012'), ('Post-Apocalypto', '2018'), ('Something Random', NULL)
select name, year_release::INT from albums
select name from albums where year_release::INT > 2010
CREATE TYPE years_error AS ENUM ('2001', '2006', '2012', 'bla')
CREATE TABLE albums_error ( name text, year_release years_error )
INSERT INTO albums_error VALUES ('Tenacious D', '2001'), ('The Pick of Destiny', 'bla')
select name, year_release::INT from albums_error where year_release = '2001'
CREATE TYPE mood AS ENUM ('sad', 'quackity-quack', 'happy', 'ok')
insert into person values ('Pedro','happy'), ('Mark', NULL), ('Hannes', 'quackity-quack'), ('Tim', 'ok'), ('Diego', 'sad')
CREATE TABLE robots ( name text, current_mood mood )
insert into robots values ('Timmynator','sad'), ('Tars', 'ok'), ('Diggernaut', NULL)
select person.name, robots.name from person inner join robots on (person.current_mood = robots.current_mood)
CREATE TYPE pet_mood AS ENUM ( 'happy','beaming', 'quackity-quack')
CREATE TABLE pet ( name text, current_mood pet_mood )
insert into pet values ('Oogie','happy'), ('Wilbur', 'quackity-quack'), ('Chorizo', NULL), ('Vacilo', 'beaming')
select person.name, pet.name from person inner join pet on (person.current_mood > pet.current_mood) where person.name = 'Pedro'
select person.name, pet.name from person inner join pet on (person.current_mood = pet.current_mood)
select person_name,pet_name from (select person.name as person_name, pet.name as pet_name, person.current_mood as person_mood, pet.current_mood as pet_mood from person,pet) as t where person_mood = pet_mood
select person_name,pet_name from (select person.name as person_name, pet.name as pet_name, person.current_mood as person_mood, pet.current_mood as pet_mood from person,pet) as t where pet_mood = person_mood
drop table person
DROP TYPE mood CASCADE
ALTER TABLE person ALTER current_mood SET DATA TYPE VARCHAR
DROP TABLE person
ALTER TABLE person DROP COLUMN current_mood
ALTER TABLE person ADD COLUMN current_mood mood
COMMIT
CREATE TABLE person ( past_mood mood, current_mood mood )
ALTER TABLE person ALTER past_mood SET DATA TYPE VARCHAR
ALTER TABLE person RENAME COLUMN current_mood TO past_mood
CREATE TABLE person ( name text, current_mood mood, past_mood mood )
SELECT type_name, logical_type FROM duckdb_types() WHERE NOT internal
CREATE TYPE s1.mood AS ENUM ('sad', 'ok', 'happy')
select 'happy'::s1.mood
DROP TYPE s1.mood
CREATE SCHEMA foo
CREATE TYPE foo.bar AS ENUM ('a', 'b')
CREATE TABLE foo.baz ( bar_col foo.bar NOT NULL )
drop schema foo cascade
CREATE SCHEMA "foo"
CREATE TYPE "foo.bar" AS ENUM ('a', 'b')
CREATE TABLE foo.baz ( bar_col "foo.bar" NOT NULL )
drop type "foo.bar" cascade
drop schema "foo" cascade
SET storage_compatibility_version='v0.10.2'
CREATE TYPE mood AS ENUM ( 'sad', 'ok', 'happy' )
CREATE TABLE person ( id INTEGER, c STRUCT( name text, current_mood mood ) )
INSERT INTO person VALUES ( 1, ROW('Mark', 'happy') )
FROM person
ALTER TABLE person DROP COLUMN c
ALTER TABLE person ADD COLUMN c STRUCT( name text, current_mood mood )
ALTER TABLE person ADD COLUMN c INT
ALTER TABLE person ALTER c SET DATA TYPE STRUCT( name text, current_mood mood )
UPDATE person SET c=ROW('Mark', 'happy')
select * from person where current_mood = 'sad'
select * from person where current_mood > 'sad'
select * from person where current_mood < 'sad'
CREATE TABLE pets ( name text, current_mood mood )
select person.name, pets.name from person inner join pets on (person.current_mood = pets.current_mood)
DROP TABLE pets
CREATE TYPE intelligence AS ENUM ('dumb', 'smart', 'ehh')
INSERT INTO aliens VALUES ('Alf o Eteimoso', 'happy'), ('Dr Zoidberg', 'sad')
ALTER TABLE aliens ADD COLUMN iq_level intelligence
select * from aliens
INSERT INTO aliens VALUES ('The Borg', 'ok', 'ehh')
ALTER TABLE aliens ALTER current_mood SET DATA TYPE VARCHAR
CREATE TEMP TABLE tbl_temp (name varchar , cur_mood mood)
insert into tbl_temp values ('bla', 'sad'), ('bla_2', 'happy')
select count(*) from tbl_temp
create type enum_numstrings as enum ('1', '2', '3', '4')
create table t1 as select range as a from range(10)
create table t2 (a enum_numstrings)
insert into t2 values ('1'), ('2'), ('3')
select t1.a, count(*) as num_matches from t1, t2 where t1.a != t2.a group by t1.a order by t1.a
insert into t2 values ('1'), ('2')
select * from t1, t2 where t1.a = t2.a order by t1.a
delete from t2 where 1=1
insert into t2 values (NULL), ('1')
select * from t1, t2 order by t1.a, t2.a NULLS FIRST
insert into t2 values ('2'), ('3'), ('4')
select * from t1, t2 where t2.a NOT IN ('2', '3', '4') order by t1.a, t2.a NULLS FIRST
create table t1(id INT, g GEOMETRY)
insert into t1 values (1, 'POINT(0 1)'), (2, 'LINESTRING(0 0, 1 1, 2 2)'), (3, 'POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))'), (4, 'MULTIPOINT((1 1), (2 2), (3 3))'), (5, 'MULTIPOINT(1 1, 2 2, 3 3)'), (6, 'MULTILINESTRING((0 0, 1 1), (2 2, 3 3))'), (7, 'MULTIPOLYGON(((0 0, 4 0, 4 4, 0 4, 0 0)), ((5 5, 7 5, 7 7, 5 7, 5 5)))'), (8, 'GEOMETRYCOLLECTION(POINT(1 1), LINESTRING(0 0, 1 1))'), (9, NULL)
select id, g::VARCHAR from t1 order by id
create table t2(id INT, g GEOMETRY)
insert into t2 values (1, 'POINT EMPTY'), (2, 'LINESTRING EMPTY'), (3, 'POLYGON EMPTY'), (4, 'MULTIPOINT EMPTY'), (5, 'MULTILINESTRING EMPTY'), (6, 'MULTIPOLYGON EMPTY'), (7, 'GEOMETRYCOLLECTION EMPTY'),
select id, g::VARCHAR from t2 order by id
SELECT 'MULTIPOINT(EMPTY, 2 2, EMPTY)'::GEOMETRY::VARCHAR
create table t3(id INT, g GEOMETRY)
select id, g::VARCHAR from t3 order by id
SELECT 'POINT(1e20 1e-10)'::GEOMETRY::VARCHAR
SELECT 'POINT(5e30 3e-20)'::GEOMETRY::VARCHAR
SELECT 'POINT(4.56e20 1.23e-10)'::GEOMETRY::VARCHAR
USE geo
select tags.storage_version from duckdb_databases() where database_name = 'geo'
select * from t_all_types order by id
select stats(g) from t_all_types limit 1
EXPLAIN ANALYZE SELECT id from t_all_types where g = 'MULTIPOINT Z (1 2 3, 3 4 5, 5 6 7)'
SELECT id from t_all_types where g = 'MULTIPOINT Z (1 2 3, 3 4 5, 5 6 7)'
INSERT INTO t_all_types VALUES (29, 'POINT (2 3)')
select id, g from t_all_types where id = 29
CHECKPOINT
USE memory
DETACH geo
INSERT INTO t_all_types VALUES (30, 'POINT (4 4)')
select st_crs(NULL)
select st_crs('foobar')
select st_crs(st_setcrs('POINT(0 1)', 'OGC:CRS84'))
create table t1 (g GEOMETRY('OGC:CRS84'))
create table t2 (g GEOMETRY('OGC:CRS83'))
insert into t2 values ('POINT(0 1)')
insert into t2 select st_setcrs(g, '') from t1
select * from t2
select st_crs(g) from t2
create table t3 (srid VARCHAR, g GEOMETRY)
insert into t3 values ('EPSG:4326', 'POINT(0 1)')
set disabled_optimizers = 'filter_pushdown'
set disabled_optimizers = ''
select 'POINT(0 1)'::GEOMETRY && 'POINT(0 0)'::GEOMETRY
select 'POINT(0 1)'::GEOMETRY && 'POINT(0 1)'::GEOMETRY
select 'POINT Z(0 1 2)'::GEOMETRY && 'POINT Z(0 1 3)'::GEOMETRY
select 'POINT(0 1)'::GEOMETRY && 'LINESTRING Z(0 0 1, 0 2 2, 2 2 3)'::GEOMETRY
PRAGMA disable_profiling
SELECT * FROM operator_metrics WHERE name = 'PARQUET_SCAN' ORDER BY total
create table t1(g GEOMETRY)
INSERT INTO t1 VALUES ('POINT(1 2)'::GEOMETRY)
select * from t1
SET checkpoint_threshold='1TB'
create table geo_wal.t1(g GEOMETRY)
INSERT INTO geo_wal.t1 VALUES ('POINT(1 2)'::GEOMETRY)
select * from geo_wal.t1
DETACH geo_wal
SET geometry_minimum_shredding_size = 0
select distinct segment_type from pragma_storage_info('t1') order by all
select stats(g) from t1
SELECT ST_AsText(g) FROM t1
INSERT INTO t1 VALUES ('LINESTRING(0 0, 1 1)'::GEOMETRY)
checkpoint
set checkpoint_threshold='10gb'
CREATE TABLE t1(geom GEOMETRY)
INSERT INTO t1 SELECT 'POINT(1 2)'::GEOMETRY FROM range(3)
DELETE FROM t1 WHERE rowid = 0
CREATE TABLE t1(type VARCHAR, has_z BOOLEAN, has_m BOOLEAN, g GEOMETRY)
select segment_type from pragma_storage_info('t2') order by all
select stats(g) from t2
create table t1(g geometry, id integer)
insert into t1 select printf('POINT(%d %d)', i, i)::geometry, i from range(0,3000) as r(i)
create index idx_id on t1(id)
select g, id from t1 where id = 1500
pragma verify_fetch_row
select g, id from t1 where id < 10 order by id
SET geometry_minimum_shredding_size = 3
CREATE TABLE t1(g GEOMETRY)
INSERT INTO t1 VALUES ('POINT(0 0)'::GEOMETRY)
INSERT INTO t1 VALUES ('POINT(1 1)'::GEOMETRY)
INSERT INTO t1 VALUES ('POINT(2 2)'::GEOMETRY)
create table t1(g geometry[], id integer)
insert into t1 select [printf('POINT(%d %d)', i, i)::geometry], i from range(0,3000) as r(i)
select g, id from t1 order by id limit 1
select g, id from t1 where id > 200 and id < 2500 order by id desc limit 1
SET checkpoint_threshold='10gb'
USE db
SET geometry_minimum_shredding_size=30000
CREATE TABLE pts AS SELECT printf('POINT(%f %f)', a::DOUBLE, a::DOUBLE)::GEOMETRY AS g FROM range(10000) t(a)
SELECT count(*) FROM pts
SELECT count(*) FROM pts WHERE g && 'POLYGON((10 10, 10 50, 50 50, 50 10, 10 10))'::GEOMETRY
SELECT count(*) FROM pts WHERE g::VARCHAR = 'POINT (5 5)'
CREATE TABLE mixed AS SELECT printf('POINT(%f %f)', a::DOUBLE, a::DOUBLE)::GEOMETRY AS g FROM range(5000) t(a) UNION ALL SELECT printf('LINESTRING(%f %f, %f %f)', a::DOUBLE, a::DOUBLE, (a + 1)::DOUBLE, (a + 1)::DOUBLE)::GEOMETRY FROM range(5000) t(a)
SELECT count(*) FROM mixed
SELECT count(*) FROM mixed WHERE g && 'POLYGON((10 10, 10 50, 50 50, 50 10, 10 10))'::GEOMETRY
CREATE TABLE withnull AS SELECT CASE WHEN a % 3 = 0 THEN NULL WHEN a % 3 = 1 THEN 'POINT EMPTY'::GEOMETRY ELSE printf('POINT(%f %f)', a::DOUBLE, a::DOUBLE)::GEOMETRY END AS g FROM range(10000) t(a)
SELECT count(g) FROM withnull
insert into t1 values (0, 'POINT(0 0)'::GEOMETRY)
select stats(g) from t1 limit 1
insert into t1 values (1, 'POINT(-2 2)'::GEOMETRY)
insert into t1 values (3, 'POINT(2 -2)'::GEOMETRY)
insert into t1 values (4, 'LINESTRING Z (0 0 0, 1 1 1, 2 2 2)'::GEOMETRY)
insert into t1 values (5, 'POLYGON M ((0 0 2, 4 0 2, 4 4 2, 0 4 2, 0 0 2))'::GEOMETRY)
insert into t1 values (6, 'MULTILINESTRING ZM ((0 0 -10 10, 1 1 -10 10), (2 2 2 1, 3 3 3 1))'::GEOMETRY)
USE geoms
CREATE TABLE geoms AS SELECT printf('POLYGON((%f %f, %f %f, %f %f, %f %f, %f %f))', x, y, x, y + 0.005, x + 0.005, y + 0.005, x + 0.005, y, x, y )::GEOMETRY AS geom FROM ( SELECT ((i % 10)::DOUBLE * 0.01) AS x, (floor(i / 10)::DOUBLE * 0.01) AS y FROM range(2048) AS t(i) )
SELECT count(*) AS count FROM geoms WHERE geom && 'POLYGON ((0.02 0.02, 0.02 0.05, 0.05 0.05, 0.05 0.02, 0.02 0.02))'::GEOMETRY
DETACH geoms
set geometry_minimum_shredding_size=0
use geometry
create table t1 (g STRUCT(a GEOMETRY[]))
CREATE TABLE t (id INT, geo GEOMETRY)
INSERT INTO t VALUES (1, NULL), (2, 'POINT(1.0 2.0)')
UPDATE t SET geo = 'POINT(0 1)' WHERE id = 1
SELECT id, ST_AsText(geo) FROM t ORDER BY id
insert into t1 values (1, 'POINT(0 1)'), (2, 'LINESTRING(0 0, 1 1, 2 2)'), (3, 'POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))'), (4, 'MULTIPOINT((1 1), (2 2), (3 3))'), (5, 'MULTILINESTRING((0 0, 1 1), (2 2, 3 3))'), (6, 'MULTIPOLYGON(((0 0, 4 0, 4 4, 0 4, 0 0)), ((5 5, 7 5, 7 7, 5 7, 5 5)))'), (7, 'GEOMETRYCOLLECTION(POINT(1 1), LINESTRING(0 0, 1 1))'), (8, NULL)
select id, ST_AsText(ST_GeomFromWKB(ST_AsWKB(g))) from t1 order by id
create table t_all_types(id INT, g GEOMETRY)
select id, g::VARCHAR from t_all_types order by id
select id, ST_AsText(ST_GeomFromWKB(ST_AsWKB(g))) from t_all_types order by id
CREATE TABLE times(i TIME)
INSERT INTO times VALUES ('00:01:20'), ('20:08:10.998'), ('20:08:10.33'), ('20:08:10.001'), (NULL)
SELECT * FROM times
SELECT cast(i AS VARCHAR) FROM times
SELECT '11:'::TIME
SELECT '11:1'::TIME
SELECT '11:11'::TIME
SELECT '11:11:'::TIME
INSERT INTO times VALUES ('00:00:01')
select count(*) from times inner join timestamp on (timestamp.i::TIME = times.i)
SELECT '15:30:00.123456789'::TIME_NS
CREATE TABLE times(tns TIME_NS)
SELECT tns, DATE_PART('hour', tns), DATE_PART('minute', tns), DATE_PART('second', tns), FROM times
SELECT tns, DATE_PART('millisecond', tns), DATE_PART('microsecond', tns), nanosecond(tns), DATE_PART('epoch', tns), FROM times
SELECT tns, DATE_PART(['hour', 'minute', 'second'], tns) FROM times
SELECT tns, DATE_PART(['millisecond', 'microsecond', 'epoch'], tns) FROM times
SELECT tns, DATE_PART(['timezone', 'timezone_hour', 'timezone_minute'], tns) p FROM times WHERE p <> {'timezone': 0, 'timezone_hour': 0, 'timezone_minute': 0}
SELECT tns, tns::TIME t FROM times
SELECT '2025-05-20 15:30:00.123456789'::TIMESTAMP_NS::TIME_NS t
SELECT '1962-05-20 15:30:00.123456789'::TIMESTAMP_NS::TIME_NS t
select timetz '02:30:00'
SELECT '02:30:00+04'::TIMETZ
SELECT '02:30:00+04:30'::TIMETZ
SELECT '02:30:00+04:30:45'::TIMETZ
SELECT '2023-08-20 16:15:03.123456'::TIMETZ
SELECT '02:30:00+1200'::TIMETZ
SELECT '02:30:00-1200'::TIMETZ
SELECT '2023-08-20 16:15:03.123456'::TIMESTAMP::TIMETZ
SELECT '16:15:03.123456'::TIME::TIMETZ
SELECT '02:30:00+04'::TIMETZ::TIME
SELECT '2021-08-20'::TIME
CREATE TABLE timetzs (ttz TIMETZ)
INSERT INTO timetzs VALUES (NULL), ('00:00:00+1559'), ('00:00:00+1558'), ('02:30:00'), ('02:30:00+04'), ('02:30:00+04:30'), ('02:30:00+04:30:45'), ('16:15:03.123456'), ('02:30:00+1200'), ('02:30:00-1200'), ('24:00:00-1558'), ('24:00:00-1559'),
SET Calendar='gregorian'
SET TimeZone='Asia/Singapore'
CREATE OR REPLACE TABLE single(c0 TIME WITH TIME ZONE)
INSERT INTO single(c0) VALUES ('12:34:56')
SELECT c0, c0::TIME AS t, c0::TIME::TIMETZ AS tz, FROM single
SELECT (c0::TIME = '12:34:56') AS e, (c0::TIME <> '12:34:56') AS u, (c0::TIME IN ('12:34:56')) AS i, (c0::TIME NOT IN ('12:34:56')) AS n, FROM single
select time '23:59:59.999999'
select time '23:59:59.999999' + interval (1) microsecond
select time '23:59:59.999999' + interval (1) second
select time '23:59:59.999999' + interval (1) minute
select time '23:59:59.999999' + interval (1) hour
select time '23:59:59.999999' + interval (1) day
select time '23:59:59.999999' + interval (1) month
select time '23:59:59.999999' + interval (1) year
SELECT '14:42:04'::TIME::VARCHAR
SELECT '14:42:04.35'::TIME::VARCHAR
SELECT '14:42:04.999999'::TIME::VARCHAR
SELECT '14:42:04.999999999'::TIME::VARCHAR
SELECT '14:42:04.000000'::TIME::VARCHAR
SELECT '14:42:04.500'::TIME::VARCHAR
select try_cast('' as time)
select try_cast(' ' as time)
select try_cast('11' as time)
select try_cast('11:' as time)
select try_cast('11:11' as time)
select try_cast('11:11:' as time)
select try_cast('11:11:A' as time)
select try_cast('11:11:A1' as time)
select try_cast('11/11/11' as time)
select try_cast(' 11:11:11 ' as time)
select try_cast('24:00:00' as time)
select try_cast('24:00:01' as time)
create type u as struct (i int, j int)
select cast (null as u array)
select cast (null as main.u)
select cast (null as main.u[])
select cast (null as main.u ARRAY)
select cast (null as SETOF main.u ARRAY)
select cast (null as main.u ARRAY[1])
create or replace table i (j struct(i double, j double))
insert into i values ({'i': 1.0, 'j': 2.0})
select j::main.u from i
select cast (null as u array[1])
select cast (null as u [])
CREATE SCHEMA app
CREATE TYPE app.item AS STRUCT ( id uuid, code UINTEGER )
CREATE TYPE app.product as STRUCT ( id uuid, items app.item[] )
CREATE SCHEMA app2
SET SEARCH_PATH TO app2
CREATE TYPE app2.item AS STRUCT ( id uuid, code UINTEGER )
CREATE TYPE app2.product as STRUCT ( id uuid, items item[] )
CREATE TABLE t1(c0 INT, c1 INET)
INSERT INTO t1(c0, c1) VALUES (1, '192.168.1.1')
SELECT * FROM t1
SELECT ((NULL, t1.c0, NULL)<>(t1.c1)) FROM t1
SELECT * FROM t1 WHERE ((NULL, t1.c0, NULL)<>(t1.c1))
SELECT * FROM t1 WHERE ((NULL, t1.c0, NULL)<>(t1.c1)) UNION ALL SELECT * FROM t1 WHERE (NOT ((NULL, t1.c0, NULL)<>(t1.c1))) UNION ALL SELECT * FROM t1 WHERE ((((NULL, t1.c0, NULL)<>(t1.c1))) IS NULL)
SET variant_minimum_shredding_size = 0
CREATE OR REPLACE TABLE test_structs( id INT, s STRUCT( name STRUCT( v VARCHAR, id INT ), nested_struct STRUCT( a integer, b bool ) ) )
CREATE OR REPLACE TABLE test_structs( id INT, s VARIANT )
INSERT INTO test_structs VALUES (1, {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}), (2, NULL), (3, {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}), (4, {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}})
CREATE OR REPLACE TABLE test_structs_nested(id INT, base STRUCT(s STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool))))
CREATE OR REPLACE TABLE test_structs_nested(id INT, base VARIANT)
INSERT INTO test_structs_nested VALUES (1, {'s': {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}}), (2, NULL), (3, {'s': {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}}), (4, {'s': {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}}})
CREATE TABLE a(c ROW(i ROW(a INTEGER), j INTEGER))
INSERT INTO a VALUES ({ 'i': { 'a': 3 }, 'j': 4 })
SELECT ((c).i).a FROM a
INSERT INTO a VALUES (ROW(ROW(NULL), 1))
INSERT INTO a VALUES (ROW(ROW(1), NULL))
INSERT INTO a VALUES (ROW(NULL, 1))
CREATE TABLE b AS SELECT { 'a': { 'a': 1, 'b': 'hello' } } c
SELECT (c).a FROM b
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v2 INT), NULL, {'v2': NULL::INTEGER})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 INT, v2 INT, v3 INT), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::INTEGER})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': 'hello'})
SELECT remap_struct( { 'i': 1, 'j': { 'x': 42, 'z': 100 } }, NULL::ROW( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR ), { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) }, { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } )
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'y': 100}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT, z STRUCT(a INT, b INT))), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'y': 'y'})}, {'v2': {'z': NULL::STRUCT(a INT, b INT)}})
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'y': 100, 'z': 1000}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, z INT), v3 VARCHAR), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'z': 'z'})}, {'v3': NULL::VARCHAR})
SELECT remap_struct( {'i': 1}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT, z INT), v3 VARCHAR), {'v1': 'i'}, {'v2': {'x': NULL::INT, 'y': NULL::INT, 'z': NULL::INT}, 'v3': NULL::VARCHAR})
CREATE TABLE structs(struct_val STRUCT(i INT, j VARCHAR))
INSERT INTO structs VALUES ({'i': 42, 'j': 'hello world this is my string'}), (NULL), ({'i': 100, 'j': NULL}), ({'i': NULL, 'j': 'string string string'})
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': 'hello'}) FROM structs
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::VARCHAR}) FROM structs
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 INT, v2 INT), {'v1': 'j', 'v2': 'i'}, NULL)
SELECT remap_struct( [ { 'i': 1, 'j': { 'x': 42, 'z': 100 } } ], NULL::STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )[], { 'list': ROW( 'list', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'list': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
CREATE TABLE large_list(s STRUCT(i INTEGER)[])
INSERT INTO large_list (SELECT LIST(CASE WHEN i%2=0 THEN {'i': i} ELSE NULL END) FROM range(5000) t(i))
SELECT COUNT(*), COUNT(j), SUM(j) FROM ( SELECT UNNEST(remap_struct(s, NULL::ROW(j INTEGER)[], {'list': ROW('list', {'j': 'i'})}, NULL), recursive := True) FROM large_list )
SELECT remap_struct( MAP { 'my_key1' : { 'i': 10, 'j': { 'x': 42, 'z': 100 } }, 'my_key2' : { 'i': 20, 'j': { 'x': 21, 'z': 50 } } }, NULL::MAP(VARCHAR, STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )), { 'key': 'key', 'value': ROW( 'value', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'value': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(INT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(BIGINT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(INT[], VARCHAR[]), { 'key': ROW( 'key', { 'list': 'list' } ), 'value': 'value' }, NULL )
SELECT CASE WHEN 1=1 THEN {'i': 1} ELSE {'i': 2} END
SELECT CASE WHEN 1=0 THEN {'i': 1} ELSE {'i': 2} END
SELECT CASE WHEN 1=1 THEN NULL ELSE {'i': 2} END
SELECT CASE WHEN 1=0 THEN NULL ELSE {'i': NULL} END
SELECT i, CASE WHEN i%2=0 THEN {'i': 1} ELSE {'i': 2} END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': 'hello'} ELSE {'i': 'world'} END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': 'hello', 'j': {'a': 3, 'b': NULL}} ELSE {'i': 'world', 'j': {'a': 7, 'b': 22}} END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': [1,2,3]} ELSE {'i': [7,8]} END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': [1,2,3]} ELSE NULL END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': [1,2,3]} ELSE {'i': NULL} END FROM range(6) tbl(i)
SELECT i, CASE WHEN i%2=0 THEN {'i': [1::INT,2::INT,3::INT]} ELSE {'i': [0::UBIGINT]} END FROM range(6) tbl(i)
CREATE TABLE tbl AS SELECT ({'HELLO': 3}) col
SELECT col['HELLO'] FROM tbl
SELECT col['hello'] FROM tbl
SELECT col.hello FROM tbl
SELECT "COL"."HELLO" FROM tbl
SELECT {'i': 1, 'j': 2}::ROW(i BIGINT, j VARCHAR)
SELECT {'i': NULL, 'j': 'hello'}::ROW(i BIGINT, j VARCHAR)
SELECT {'i': NULL, 'j': NULL}::ROW(i BIGINT, j VARCHAR)
SELECT NULL::ROW(i BIGINT, j VARCHAR)
SELECT ({'i': NULL, 'j': NULL}::ROW(i BIGINT, j VARCHAR))['i']
SELECT ({'i': NULL, 'j': NULL})['i']
SELECT (NULL::ROW(i BIGINT, j VARCHAR))['i']
SELECT {'i': 1, 'j': {'a': 2, 'b': 3}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': {'a': NULL, 'b': 3}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': {'a': 2, 'b': NULL}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': NULL}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT ({'i': 1, 'j': NULL}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR)))['j']['a']
CREATE TABLE t1 (s1 STRUCT(a INT, b INT))
INSERT INTO t1 VALUES ({a: 42, b: 43})
CREATE TABLE t2 (s1 STRUCT(a INT, c INT))
INSERT INTO t2 VALUES ({a: 100, c: 101})
SELECT {'a': {'e1': 42, 'e2': 42}} AS c UNION ALL BY NAME SELECT {'a': {'e2': 'hello', 'e3': 'world'}, 'b': '100'} AS c
SELECT {'x': 1} < {'x': 2}
SELECT {'x': 1} < {'x': 1}
SELECT NULL < {'x': 1}
SELECT {'x': 1} < NULL
SELECT {'x': 1} <= {'x': 2}
SELECT {'x': 1} <= {'x': 1}
SELECT NULL <= {'x': 1}
SELECT {'x': 1} <= NULL
SELECT {'x': 1} = {'x': 2}
SELECT {'x': 1} = {'x': 1}
SELECT NULL = {'x': 1}
SELECT {'x': 1} = NULL
SELECT struct_concat({'a': 1}, {'b': NULL}, NULL::STRUCT(k INT), struct_pack( x := 'foobar'))
CREATE TABLE t1 AS SELECT {'i': i, 'j': i + i % 2} as s FROM generate_series(1, 15) AS t(i)
SELECT struct_concat({'a': 2, 'b': NULL}, s) FROM t1
SELECT struct_concat(s, {'a': 2, 'b': NULL}) FROM t1 WHERE s.i % 2 = 0
SELECT struct_concat(row('a'), row('b'))
PREPARE v1 AS SELECT struct_concat({'a': 1}, ?)
EXECUTE v1({'b': 42})
PREPARE v2 AS SELECT struct_concat({'a': ?}, {'b': 42})
EXECUTE v2(1)
SELECT struct_contains(ROW(1, 2), 2)
SELECT struct_contains(ROW(1, 2), 3)
SELECT struct_contains(ROW(1, 2), NULL)
SELECT struct_contains(ROW(1, NULL), 1)
SELECT struct_contains(ROW(1, NULL), NULL)
SELECT struct_contains(NULL, 1)
SELECT struct_contains(ROW(1), NULL)
SELECT struct_contains(ROW(NULL), NULL)
SELECT struct_contains(ROW('test', 'notest'), 'notest')
SELECT struct_contains(ROW('test', 'notest'), 'a')
SELECT struct_contains(ROW(1, 2, 3), TRUE)
SELECT struct_contains(ROW(1, 2, 3), 1.0)
CREATE VIEW v1 AS SELECT * FROM (VALUES (1, {'a': {'a1': 3, 'a2': 7}, 'b': [1, 2, 3]}), (2, NULL), (3, {'a': NULL, 'b': [4, 5, NULL]})) tbl (a, b)
SELECT * FROM v1 v, v1 w ORDER BY v.a, w.a
SELECT * FROM v1 v, v1 w WHERE v.a >= w.a ORDER BY v.a, w.a
SELECT * FROM v1 v, v1 w WHERE v.a <> w.a ORDER BY v.a, w.a
SELECT * FROM v1 v, v1 w WHERE v.a <> w.a OR v.a > w.a ORDER BY v.a, w.a
CREATE TABLE t1 (s STRUCT(v VARCHAR))
INSERT INTO t1 VALUES (ROW(NULL))
SELECT s FROM t1 ORDER BY ALL
CREATE TABLE foo (bar struct(pip int))
INSERT INTO foo VALUES (ROW(42))
SELECT bar FROM foo ORDER BY ALL
CREATE OR REPLACE TABLE T AS SELECT [{'a': 'A', 'b':'B'}] AS x, [{'b':'BB','a':'AA'}] AS y
SELECT x, y, ARRAY_CONCAT(x, y) FROM T
CREATE OR REPLACE TABLE T (s STRUCT(a INT, b INT))
SELECT s FROM T ORDER BY ALL
CREATE TABLE tbl (a STRUCT(a INT, b VARCHAR))
INSERT INTO tbl VALUES (ROW(5, 'hello'))
SELECT l IS NOT DISTINCT FROM r FROM struct_int
SELECT l IS DISTINCT FROM r FROM struct_int
SELECT {'x': 'duck'} IS NOT DISTINCT FROM {'x': 'goose'}
SELECT {'x': 'duck'} IS NOT DISTINCT FROM {'x': 'duck'}
SELECT {'x': 'duck'} IS NOT DISTINCT FROM NULL
SELECT NULL IS NOT DISTINCT FROM {'x': 'duck'}
SELECT {'x': 'duck'} IS DISTINCT FROM {'x': 'goose'}
SELECT {'x': 'duck'} IS DISTINCT FROM {'x': 'duck'}
SELECT {'x': 'duck'} IS DISTINCT FROM NULL
SELECT NULL IS DISTINCT FROM {'x': 'duck'}
CREATE VIEW struct_str AS SELECT * FROM (VALUES ({'x': 'duck'}, {'x': 'duck'}), ({'x': 'duck'}, {'x': 'goose'}), ({'x': 'goose'}, {'x': 'duck'}), (NULL, {'x': 'duck'}), ({'x': 'goose'}, NULL), (NULL, NULL) ) tbl(l, r)
SELECT l IS NOT DISTINCT FROM r FROM struct_str
create table integers(i integer)
INSERT INTO integers VALUES (1),(1),(3),(20),(20),(20)
select unnest(map_entries(histogram(i))) FROM integers
CREATE TABLE a(id INTEGER PRIMARY KEY, c ROW(i ROW(a INTEGER), j INTEGER))
INSERT INTO a VALUES (1, { 'i': { 'a': 3 }, 'j': 4 })
SELECT * FROM a WHERE id=1
INSERT INTO a VALUES (2, NULL)
SELECT * FROM a ORDER BY id
INSERT INTO a VALUES (3, ROW(ROW(NULL), 1))
INSERT INTO a VALUES (4, ROW(ROW(1), NULL))
INSERT INTO a VALUES (5, ROW(NULL, 1))
SELECT * FROM a WHERE id=2
SELECT * FROM a WHERE id=3
SELECT * FROM a WHERE id=4
SELECT * FROM a WHERE id=5
SELECT {'a': 42, 'b': 84}::STRUCT(b INT, a INT)
SELECT {'a': ['1', '2', '3'], 'b': 84}::STRUCT(b INT, a INT[])
SELECT {'a': ['1', '2', '3'], 'b': 84}::STRUCT(b INT, A INT[])
SELECT {'a': ['1', '2', '3'], 'b': 84}::STRUCT(b INT, c INT[])
SELECT ROW(42, 84)::STRUCT(a INT, b INT)
CREATE VIEW struct_int AS SELECT * FROM (VALUES ({'x': 1, 'y': 0}), ({'x': 1, 'y': 2}), ({'x': 1, 'y': NULL}), ({'x': NULL, 'y': 2}), ({'x': NULL, 'y': NULL}), ({'x': NULL, 'y': 0}), (NULL) ) tbl(i)
SELECT lhs.i, rhs.i, lhs.i < rhs.i, lhs.i <= rhs.i, lhs.i = rhs.i, lhs.i <> rhs.i, lhs.i > rhs.i, lhs.i >= rhs.i, lhs.i IS NOT DISTINCT FROM rhs.i, lhs.i IS DISTINCT FROM rhs.i FROM struct_int lhs, struct_int rhs
CREATE VIEW list_str AS SELECT * FROM (VALUES ({'x': 'duck', 'y': ''}), ({'x': 'duck', 'y': 'goose'}), ({'x': 'duck', 'y': NULL}), ({'x': NULL, 'y': 'goose'}), ({'x': NULL, 'y': NULL}), ({'x': NULL, 'y': '0'}), (NULL) ) tbl(i)
SELECT lhs.i, rhs.i, lhs.i < rhs.i, lhs.i <= rhs.i, lhs.i = rhs.i, lhs.i <> rhs.i, lhs.i > rhs.i, lhs.i >= rhs.i, lhs.i IS NOT DISTINCT FROM rhs.i, lhs.i IS DISTINCT FROM rhs.i FROM list_str lhs, list_str rhs
CREATE TABLE a(id INTEGER, b ROW(i INTEGER, j INTEGER))
INSERT INTO a VALUES (1, {i: 1, j: 2})
CREATE TABLE b(id INTEGER, j VARCHAR)
INSERT INTO b VALUES (1, 'hello')
SELECT * FROM a LEFT JOIN b ON a.id<>b.id
SELECT * FROM a RIGHT JOIN b ON a.id<>b.id
SELECT * FROM a LEFT JOIN b ON a.id>b.id
SELECT * FROM a RIGHT JOIN b ON a.id>b.id
SELECT (SELECT b FROM a)
SELECT a from (SELECT STRUCT_PACK(a := 42, b := 43) as a) as t
SELECT a from (SELECT STRUCT_PACK(a := NULL, b := 43) as a) as t
SELECT a from (SELECT STRUCT_PACK(a := NULL) as a) as t
SELECT a from (SELECT STRUCT_PACK(a := i, b := i) as a FROM range(10000) tbl(i)) as t
SELECT a from (SELECT STRUCT_PACK(a := LIST_VALUE(1,2,3), b := i) as a FROM range(10000) tbl(i)) as t
SELECT struct_position(ROW(7, 2, 5), 7)
SELECT struct_position(ROW(7, 2, 5), 2)
SELECT struct_position(ROW(7, 2, 5), 5)
SELECT struct_position(ROW(1, 2, 3), 1.0)
SELECT struct_position(ROW(1.0, 2.0, 3.0, 4.0), 1)
SELECT struct_position(ROW(1, 2, 3), 4.0)
SELECT struct_position(ROW(1.0, 2.0, 3.0), 4)
SELECT struct_position(ROW(7), 5)
SELECT struct_position(ROW(1, 2, 3, 4), 4)
SELECT struct_position(ROW(true, false), true)
SELECT struct_position(ROW(true, true), false)
SELECT struct_position(ROW('test', 'notest'), 'notest')
CREATE OR REPLACE TABLE test_structs(id INT, s STRUCT(a integer, b bool))
CREATE OR REPLACE TABLE test_structs(id INT, s VARIANT)
INSERT INTO test_structs VALUES ( 1, { 'a': 42, 'b': true } ), ( 2, NULL ), ( 3, { 'a': 84, 'b': NULL } ), ( 4, { 'a': NULL, 'b': false } )
UPDATE test_structs SET s={'a': 84, 'b': false} WHERE id=2
SELECT s['b'], s.a FROM test_structs WHERE id=2
CREATE TABLE test_structs( col VARCHAR, i STRUCT(a integer, b bool, c VARCHAR) )
INSERT INTO test_structs VALUES ('test', {'a': 1, 'b': true, 'c': 'test'}), ('test', {'a': 2, 'b': false, 'c': 'hello'}), ('hello', NULL), ('test', {'a': 3, 'b': true, 'c': 'this is a long string'}), ('test', {'a': NULL, 'b': NULL, 'c': NULL})
SELECT i.a FROM test_structs
SELECT i.a, i.c FROM test_structs
SELECT i.a, i.c FROM test_structs where col == 'test'
pragma explain_output = optimized_only
EXPLAIN SELECT i.a FROM test_structs
CREATE TABLE tbl( s1 STRUCT( i SMALLINT ), s2 STRUCT( f DATE ), id INT )
INSERT INTO tbl VALUES (ROW(1), ROW(DATE '2024-01-30'), 0)
WITH subq AS (FROM tbl) SELECT id, min(subq.s1.i), min(subq.s2.f) FROM subq GROUP BY id
SELECT STATS({'i': 3, 'j': 4})
SELECT STATS(NULL::ROW(i INTEGER))
CREATE TABLE integers AS SELECT 3 i, 4 j
SELECT STATS({'i': i, 'j': j}) FROM integers
CREATE TABLE structs AS SELECT {'i': 3, 'j': 4} s
SELECT STATS(s['i']) FROM structs
SELECT (SELECT tbl.a['i'] + tbl.b['j'] FROM (VALUES ({'i': 1, 'j': 2})) tbl(b)) FROM (VALUES ({'i': 1, 'j': 2})) tbl(a)
SELECT (SELECT tbl2.a['i'] + tbl.b['j'] FROM (VALUES ({'i': 1, 'j': 2})) tbl(b)) FROM (VALUES ({'i': 1, 'j': 2})) tbl2(a)
CREATE TABLE a(b ROW(i INTEGER, j INTEGER))
INSERT INTO a VALUES (STRUCT_PACK(i := 1, j:= 2))
SELECT * FROM a ORDER BY (b).i
SELECT * FROM a ORDER BY (b).i, (b).j
SELECT UNNEST ( ( '1,2,3,4,,6' , ( case when random() < 10 then 0 else 1 end ) ) ), 42 x, x
INSERT INTO a VALUES ({'i': 1, 'j': 2})
UPDATE a SET b={'i': 3, 'j': 4}
UPDATE a SET b=NULL
UPDATE a SET b={'i': NULL, 'j': 4}
UPDATE a SET b={'i': 3, 'j': NULL}
INSERT INTO a VALUES ({'i': 2, 'j': 3})
UPDATE a SET b={'i': NULL, 'j': NULL} WHERE (b).j>=3
select row(42, 'hello') union all select '(84, world)'
select a<>b from VALUES ((NULL, 1, NULL), (5, 6, 7)) t(a, b)
select a<>b is null from VALUES ((NULL, 1, NULL), (5, 6, 7)) t(a, b)
select 1 from values (struct_pack(k := NULL)) t(a) where 1 <> a.k
select [NULL, 6] <> [6, 5]
select 1 from VALUES ([NULL, 6], [5, 6]) t(a, b) where a<>b
select 1 from VALUES ([NULL, 1, NULL], [5, 6, 7]) t(a, b) where a=b
select 1 from VALUES ((NULL, 1, NULL), (5, 6, 7)) t(a, b) where a<>b
select 1 from VALUES ((NULL, 1, NULL), (5, 6, 7)) t(a, b) where a<>b is null
select a<>b is null from VALUES ((NULL, 1, NULL), (5, 6, 7)) t(a, b) where NULL
select 1 from VALUES ((NULL, 1, NULL), (5, 6, 7), (NULL, 2), (4, 5)) t(a, b, c, d) where a<>b and c<>d
select a<>b, c<>d from VALUES ((NULL, 1, NULL), (5, 6, 7), (NULL, 2), (4, 5)) t(a, b, c, d) where a<>b and c<>d
select [{ t:'abc', len:5 }, ('abc', 2)]
select [('abc', 2), { t:'abc', len:5 }]
SELECT unnest([{'a':{ 'aa': 42}, 'b':{'bb': 84}}], recursive := true, keep_parent_names := true)
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a':{ 'aa': 42}, 'b':{'bb': 84}}], recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a':{ 'aa': {'aaa': 42}}, 'b':{'bb': 84}}], recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'bb': {'bbb': 12}}}], recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'bb': {'bbb': 12}}}], recursive := true, keep_parent_names := false))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'bb': {'bbb': 12}}}], recursive := true, max_depth := 3, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'bb': {'bbb': 12}}}], recursive := true, max_depth := 2, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'bb': {'bbb': 12}}}], max_depth := 3))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest(row(row(42)), recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest(row(row(42)), recursive := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest(row(row(42))))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest(row(row(42), 41), keep_parent_names := true, recursive := true))
SELECT UNNEST({'a': 42, 'b': 88})
SELECT a, b FROM (SELECT UNNEST({'a': 42, 'b': 88}))
SELECT UNNEST({'a': 42, 'b': {'c': 88, 'd': 99}})
SELECT UNNEST({'a': 42, 'b': {'c': 88, 'd': 99}}, recursive := true)
SELECT UNNEST({'a': 42, 'b': {'c': {'x': 4}, 'd': 99}}, max_depth := 2)
SELECT a, c, d FROM (SELECT UNNEST({'a': 42, 'b': {'c': 88, 'd': 99}}, recursive := true))
SELECT a, "b.c", "b.d" FROM (SELECT UNNEST({'a': 42, 'b': {'c': 88, 'd': 99}}, recursive := true, keep_parent_names := true))
SELECT UNNEST([{'a': 42, 'b': 88}, {'a': NULL, 'b': 99}])
SELECT UNNEST([{'a': 42, 'b': 88}, {'a': NULL, 'b': 99}], recursive := true)
SELECT UNNEST([[{'a': 42, 'b': {'x': 99}}, {'a': NULL, 'b': {'x': NULL}}]], max_depth:=1)
SELECT UNNEST([[{'a': 42, 'b': {'x': 99}}, {'a': NULL, 'b': {'x': NULL}}]], max_depth:=2)
SELECT UNNEST([[{'a': 42, 'b': {'x': 99}}, {'a': NULL, 'b': {'x': NULL}}]], max_depth:=3)
CREATE TABLE tbl_structs AS SELECT {'a': 1, 'b': 2, 'c': 3} AS s
INSERT INTO tbl_structs VALUES ({'a': 2, 'b': 3, 'c': 1})
INSERT INTO tbl_structs VALUES ({'a': 3, 'b': 1, 'c': 2})
SELECT UNNEST(s) FROM tbl_structs UNION ALL SELECT s.a, s.b, s.c FROM tbl_structs ORDER BY s.a, s.b, s.c
CREATE OR REPLACE TABLE tbl_structs AS SELECT {'a': 1, 'b': 2, 'c': 3} AS s
INSERT INTO tbl_structs VALUES ({'a': 1, 'b': 3, 'c': 1})
INSERT INTO tbl_structs VALUES ({'a': 1, 'b': 1, 'c': 2})
select unnest(s) from tbl_structs order by all
CREATE OR REPLACE TABLE tbl_structs AS SELECT {'a': 'hello'} s
INSERT INTO tbl_structs VALUES ({'a': 'WORLD'})
SELECT UNNEST(s) FROM tbl_structs ORDER BY 1 COLLATE NOCASE
CREATE OR REPLACE TABLE tbl_structs AS SELECT {'a': 'hello', 'b': 1} s
SELECT UNNEST(a) FROM (VALUES ({'a': 42, 'b': 88})) t(a)
SELECT (SELECT t.x FROM (SELECT UNNEST(a)) t(x)) FROM (VALUES ({'a': 42, 'b': 88})) t(a)
CREATE TABLE bigints AS SELECT i::BIGINT i FROM (VALUES (-9223372036854775808), (0), (9223372036854775807)) tbl(i)
SELECT i::UBIGINT FROM bigints WHERE i>=0 ORDER BY i
SELECT TRY_CAST(i AS UTINYINT) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS UBIGINT) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM bigints ORDER BY i
SELECT TRY_CAST(i AS INTEGER) FROM bigints ORDER BY i
SELECT i::HUGEINT::BIGINT FROM bigints ORDER BY i
SELECT i::FLOAT FROM bigints ORDER BY i
SELECT i::DOUBLE FROM bigints ORDER BY i
CREATE TABLE booleans AS SELECT b::BOOLEAN b FROM (VALUES (NULL), (0), (1)) tbl(b)
CREATE TABLE hugeints AS SELECT i::HUGEINT i FROM (VALUES (-170141183460469231731687303715884105728), (0), (170141183460469231731687303715884105727)) tbl(i)
SELECT TRY_CAST(i AS UTINYINT) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS UBIGINT) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS INTEGER) FROM hugeints ORDER BY i
SELECT TRY_CAST(i AS BIGINT) FROM hugeints ORDER BY i
SELECT i::FLOAT FROM hugeints ORDER BY i
SELECT i::DOUBLE FROM hugeints ORDER BY i
SELECT i::BOOL FROM hugeints ORDER BY i
CREATE TABLE integers AS SELECT i::INTEGER i FROM (VALUES (-2147483648), (0), (2147483647)) tbl(i)
SELECT i::UINTEGER FROM integers WHERE i>=0 ORDER BY i
SELECT i::UBIGINT FROM integers WHERE i>=0 ORDER BY i
SELECT TRY_CAST(i AS UTINYINT)::INTEGER FROM integers ORDER BY i
SELECT TRY_CAST(i AS USMALLINT)::INTEGER FROM integers ORDER BY i
SELECT TRY_CAST(i AS UINTEGER)::INTEGER FROM integers ORDER BY i
SELECT TRY_CAST(i AS UBIGINT)::INTEGER FROM integers ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM integers ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM integers ORDER BY i
SELECT i::BIGINT::INTEGER FROM integers ORDER BY i
SELECT i::HUGEINT::INTEGER FROM integers ORDER BY i
SELECT i::FLOAT FROM integers ORDER BY i
CREATE TABLE smallints AS SELECT i::SMALLINT i FROM (VALUES (-32768), (0), (32767)) tbl(i)
SELECT i::USMALLINT FROM smallints WHERE i>=0 ORDER BY i
SELECT i::UINTEGER FROM smallints WHERE i>=0 ORDER BY i
SELECT i::UBIGINT FROM smallints WHERE i>=0 ORDER BY i
SELECT TRY_CAST(i AS UTINYINT)::SMALLINT FROM smallints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT)::SMALLINT FROM smallints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER)::SMALLINT FROM smallints ORDER BY i
SELECT TRY_CAST(i AS UBIGINT)::SMALLINT FROM smallints ORDER BY i
SELECT TRY_CAST(i AS TINYINT)::SMALLINT FROM smallints ORDER BY i
SELECT i::INTEGER::SMALLINT FROM smallints ORDER BY i
SELECT i::BIGINT::SMALLINT FROM smallints ORDER BY i
SELECT i::HUGEINT::SMALLINT FROM smallints ORDER BY i
CREATE TABLE numerics(i NUMERIC(), j NUMERIC)
INSERT INTO numerics VALUES (0176030871715840, 2.2)
SELECT * FROM numerics
SELECT 1.25::FLOAT::NUMERIC, 1.25::FLOAT::NUMERIC()
CREATE TABLE tinyints AS SELECT i::TINYINT i FROM (VALUES (-128), (0), (127)) tbl(i)
SELECT i::UTINYINT::TINYINT FROM tinyints WHERE i>=0 ORDER BY i
SELECT i::USMALLINT::TINYINT FROM tinyints WHERE i>=0 ORDER BY i
SELECT i::UINTEGER::TINYINT FROM tinyints WHERE i>=0 ORDER BY i
SELECT i::UBIGINT::TINYINT FROM tinyints WHERE i>=0 ORDER BY i
SELECT TRY_CAST(i AS UTINYINT) FROM tinyints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM tinyints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER) FROM tinyints ORDER BY i
SELECT TRY_CAST(i AS UBIGINT) FROM tinyints ORDER BY i
SELECT i::SMALLINT::TINYINT FROM tinyints ORDER BY i
SELECT i::INTEGER::TINYINT FROM tinyints ORDER BY i
SELECT i::BIGINT::TINYINT FROM tinyints ORDER BY i
SELECT typeof(1::UBIGINT + 1::TINYINT)
SELECT typeof(1::UBIGINT + 1)
SELECT typeof(1::UBIGINT + 10000)
CREATE TABLE ubigints AS SELECT i::UBIGINT i FROM (VALUES (0), (18446744073709551615)) tbl(i)
SELECT TRY_CAST(i AS UTINYINT) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS INTEGER) FROM ubigints ORDER BY i
SELECT TRY_CAST(i AS BIGINT) FROM ubigints ORDER BY i
SELECT i::HUGEINT FROM ubigints ORDER BY i
SELECT i::FLOAT FROM ubigints ORDER BY i
SELECT i::DOUBLE FROM ubigints ORDER BY i
SELECT i::BOOL FROM ubigints ORDER BY i
CREATE TABLE uhugeints AS SELECT i::UHUGEINT i FROM (VALUES (0::UHUGEINT), (1::UHUGEINT), ('340282366920938463463374607431768211455'::UHUGEINT)) tbl(i)
SELECT TRY_CAST(i AS TINYINT) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS INTEGER) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS BIGINT) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS UTINYINT) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS UINTEGER) FROM uhugeints ORDER BY i
SELECT TRY_CAST(i AS UBIGINT) FROM uhugeints ORDER BY i
SELECT i::FLOAT FROM uhugeints ORDER BY i
SELECT i::DOUBLE FROM uhugeints ORDER BY i
SELECT i::BOOL FROM uhugeints ORDER BY i
CREATE TABLE uintegers AS SELECT i::UINTEGER i FROM (VALUES (0), (4294967295)) tbl(i)
SELECT TRY_CAST(i AS UTINYINT) FROM uintegers ORDER BY i
SELECT TRY_CAST(i AS USMALLINT) FROM uintegers ORDER BY i
SELECT i::UBIGINT FROM uintegers ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM uintegers ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM uintegers ORDER BY i
SELECT TRY_CAST(i AS INTEGER) FROM uintegers ORDER BY i
SELECT i::BIGINT FROM uintegers ORDER BY i
SELECT i::HUGEINT FROM uintegers ORDER BY i
SELECT i::FLOAT FROM uintegers ORDER BY i
SELECT i::DOUBLE FROM uintegers ORDER BY i
SELECT i::BOOL FROM uintegers ORDER BY i
CREATE TABLE usmallints AS SELECT i::USMALLINT i FROM (VALUES (0), (65535)) tbl(i)
SELECT TRY_CAST(i AS UTINYINT) FROM usmallints ORDER BY i
SELECT i::UINTEGER FROM usmallints ORDER BY i
SELECT i::UBIGINT FROM usmallints ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM usmallints ORDER BY i
SELECT TRY_CAST(i AS SMALLINT) FROM usmallints ORDER BY i
SELECT i::INTEGER FROM usmallints ORDER BY i
SELECT i::BIGINT FROM usmallints ORDER BY i
SELECT i::HUGEINT::USMALLINT FROM usmallints ORDER BY i
SELECT i::FLOAT FROM usmallints ORDER BY i
SELECT i::DOUBLE FROM usmallints ORDER BY i
SELECT i::BOOL FROM usmallints ORDER BY i
CREATE TABLE utinyints AS SELECT i::UTINYINT i FROM (VALUES (0), (255)) tbl(i)
SELECT i::USMALLINT FROM utinyints ORDER BY i
SELECT i::UINTEGER FROM utinyints ORDER BY i
SELECT i::UBIGINT FROM utinyints ORDER BY i
SELECT TRY_CAST(i AS TINYINT) FROM utinyints ORDER BY i
SELECT i::SMALLINT FROM utinyints ORDER BY i
SELECT i::INTEGER FROM utinyints ORDER BY i
SELECT i::BIGINT FROM utinyints ORDER BY i
SELECT i::HUGEINT FROM utinyints ORDER BY i
SELECT i::FLOAT FROM utinyints ORDER BY i
SELECT i::DOUBLE FROM utinyints ORDER BY i
SELECT i::BOOL FROM utinyints ORDER BY i
CREATE TABLE tinyint_limits AS SELECT (-128)::TINYINT min, 127::TINYINT max
CREATE TABLE smallint_limits AS SELECT (-32768)::SMALLINT min, 32767::SMALLINT max
CREATE TABLE integer_limits AS SELECT (-2147483648)::INTEGER min, 2147483647::INTEGER max
CREATE TABLE bigint_limits AS SELECT (-9223372036854775808)::BIGINT min, 9223372036854775807::BIGINT max
CREATE TABLE utinyint_limits AS SELECT (0)::UTINYINT min, 255::UTINYINT max
CREATE TABLE usmallint_limits AS SELECT (0)::USMALLINT min, 65535::USMALLINT max
CREATE TABLE uinteger_limits AS SELECT (0)::UINTEGER min, 4294967295::UINTEGER max
CREATE TABLE ubigint_limits AS SELECT (0)::UBIGINT min, 18446744073709551615::UBIGINT max
CREATE TABLE hugeint_limits AS SELECT (-17014118346046923173168730371588410572)::HUGEINT min, 17014118346046923173168730371588410572::HUGEINT max
select typeof([100::USMALLINT, 10000::SMALLINT])
select typeof([100::USMALLINT, 127::TINYINT])
select typeof([100::USMALLINT, 127::USMALLINT])
select typeof([100::USMALLINT, 10.5::DECIMAL])
select typeof([100::UTINYINT, 10000::SMALLINT])
select typeof([100::UTINYINT, 127::TINYINT])
select typeof([100::UTINYINT, 127::UTINYINT])
select typeof([100::UTINYINT, 127::USMALLINT])
select typeof([100::UTINYINT, 10.5::DECIMAL])
SELECT MAP(['a', 'b', 'c'], [1, 2, NULL])::MAP(VARCHAR, VARCHAR)
SELECT MAP(['a', 'b', 'c'], [1, 2, NULL])::MAP(VARCHAR, BIGINT)
SELECT MAP([1, 2, 3], [1, 2, NULL])::MAP(VARCHAR, BIGINT)
SELECT MAP([[1, 2, 3], [0], [123]], [1.0, 2.1, 4.9])::MAP(VARCHAR[], TINYINT)
SELECT MAP([1, 2, 3], ['A', 'B', 'C'])::MAP(TINYINT, VARCHAR)
CREATE TABLE ints (i INT)
INSERT INTO ints VALUES (1), (2), (3)
SELECT MAP(['name'], [i]) FROM ints
SELECT MAP([i], ['name'] ) FROM ints
SELECT MAP([i, i+1], ['x', 'y']) FROM ints
SELECT MAP([i, i+1], ['x', 'y']) FROM ints WHERE i > 1
SELECT MAP(['x'], [m]) FROM (SELECT MAP([i], ['y']) m FROM ints WHERE i <> 1)
SELECT MAP(['key'], [range]) FROM range(5) WHERE range > 2
SELECT MAP(['🦆', '🦤', '🐓'], [i, i+1, i+2]) FROM ints
SELECT MAP([10, i, i+1, 9], [i, 3.14, 0.12, 8.0]) FROM ints
CREATE TABLE tbl (v VARCHAR[])
INSERT INTO tbl VALUES (ARRAY['test', 'string']), (ARRAY['foo', 'bar'])
CREATE TABLE test(id int, attr MAP(VARCHAR, UNION(i INT, s VARCHAR)))
INSERT INTO test VALUES (1, MAP{'key1': 'str'})
SELECT id, attr['key2'] FROM test
select map(NULL::INT[], [1,2,3])
select map(NULL, [1,2,3])
select map(NULL, NULL)
select map(NULL, [1,2,3]) IS NULL
select map([1,2,3], NULL)
select map([1,2,3], NULL::INT[])
SELECT * FROM ( VALUES (MAP(NULL, NULL)), (MAP(NULL::INT[], NULL::INT[])), (MAP([1,2,3], [1,2,3])) )
select MAP(a, b) FROM ( VALUES (NULL, ['b', 'c']), (NULL::INT[], NULL), (NULL::INT[], NULL::VARCHAR[]), (NULL::INT[], ['a', 'b', 'c']), (NULL, ['longer string than inlined', 'smol']), (NULL, NULL), ([1,2,3], NULL), ([1,2,3], ['z', 'y', 'x']), ([1,2,3], NULL::VARCHAR[]), ) t(a, b)
SELECT get_type(NULL)
SELECT get_type(1)
SELECT get_type('hello')
SELECT make_type('STRUCT', a := make_type('INTEGER'), b := make_type('VARCHAR'))
SELECT make_type('LIST', make_type('STRUCT', a := make_type('STRUCT', x := make_type('INTEGER')), b := make_type('VARCHAR')))
SELECT 0 AND 0, 0 AND 1, 1 AND 0, 1 AND 1, NULL AND 0, NULL AND 1, 0 AND NULL, 1 AND NULL, NULL AND NULL
SELECT 0 OR 0, 0 OR 1, 1 OR 0, 1 OR 1, NULL OR 0, NULL OR 1, 0 OR NULL, 1 OR NULL, NULL OR NULL
SELECT NOT(0), NOT(1), NOT(NULL)
SELECT NULL IS NULL, NULL IS NOT NULL, 42 IS NULL, 42 IS NOT NULL
SELECT NULL = NULL, NULL <> NULL, 42 = NULL, 42 <> NULL
INSERT INTO test VALUES (11, 22), (NULL, 21), (13, 22), (12, NULL), (16, NULL)
SELECT b, COUNT(a), SUM(a), MIN(a), MAX(a) FROM test GROUP BY b ORDER BY b
INSERT INTO test VALUES (11, 1), (NULL, 2), (13, 3)
SELECT a IS NULL, a IS NOT NULL, rowid IS NULL, (a = NULL) IS NULL FROM test ORDER BY b
SELECT a IS NULL, a IS NOT NULL, rowid IS NULL, (a = NULL) IS NULL FROM test WHERE b != 1 ORDER BY b
SELECT NULL
SELECT 3 + NULL
SELECT NULL + 3
SELECT NULL + NULL
SELECT 1 + (NULL + NULL)
SET ieee_floating_point_ops=false
SELECT 4 / 0
INSERT INTO test VALUES (11, 22), (NULL, 21), (13, 22)
SELECT a FROM test
SELECT cast(a AS BIGINT) FROM test
SELECT a / 0 FROM test
SELECT a / (a - a) FROM test
SELECT SUM(a), MIN(a), MAX(a) FROM test
SELECT COUNT(*), COUNT(a), COUNT(b) FROM test
INSERT INTO test VALUES (12, NULL), (16, NULL)
INSERT INTO test VALUES (NULL, NULL), (NULL, 22)
SELECT NULL::BIGINT::VARCHAR::INT[]::ROW(i INTEGER, k INTEGER)::DECIMAL(4,0)
INSERT INTO tbl VALUES (1), (-1), (0), ('nan'), ('inf')
SET ieee_floating_point_ops = false
SET ieee_floating_point_ops = true
INSERT INTO floats VALUES ('INF'), (1), ('-INF')
SELECT * FROM floats
SELECT f FROM floats WHERE f=1
SELECT f FROM floats WHERE f<>1 ORDER BY 1
SELECT f FROM floats WHERE f>1 ORDER BY 1
SELECT f FROM floats WHERE f>=1 ORDER BY ALL
SELECT f FROM floats WHERE f<1
SELECT f FROM floats WHERE f<=1 ORDER BY ALL
DROP TABLE floats
insert into floats values ('inf', 1), ('inf', 7), ('-inf', 3), ('nan', 7), ('nan', 19), ('-inf', 2)
SELECT f, SUM(i) FROM floats GROUP BY f ORDER BY f
select sum(f) from floats where 0 < f and f != 'nan'::DOUBLE
select sum(f) from floats where 0 > f
select sum(f) from floats
create table floats_doubles (f FLOAT, d DOUBLE)
insert into floats_doubles VALUES (2e38, 1e308), (2e38, 1e308), (-1e38, 0), (-1e38, 0)
insert into floats values ('inf'), ('-inf'), ('nan')
SELECT MIN(f), MAX(f) FROM floats
select f, abs(f), exp(f), pow(f, 2), sqrt(case when f < 0 then NULL else f end), cbrt(f), ln(case when f < 0 then NULL else f end), degrees(f), radians(f), gamma(f), lgamma(f), atan(f), atan2(f, 0) from floats
drop table floats
SELECT nextafter('inf'::float, '-inf'::float)
SELECT nextafter('-inf'::float, 'inf'::float)
SELECT nextafter('inf'::double, '-inf'::double)
SELECT nextafter('-inf'::double, 'inf'::double)
INSERT INTO floats VALUES ('NAN'), (1), ('infinity'), ('-infinity'), (-1), (NULL)
SELECT f FROM floats ORDER BY f
SELECT f FROM floats ORDER BY f DESC
SELECT f FROM floats ORDER BY f DESC NULLS LAST LIMIT 2
SELECT f FROM floats ORDER BY f NULLS LAST LIMIT 2
SELECT f FROM floats ORDER BY f DESC NULLS LAST LIMIT 4
SELECT f FROM floats ORDER BY f NULLS LAST LIMIT 4
SELECT COUNT(*) FROM floats WHERE f > 0
SELECT COUNT(*) FROM floats WHERE f < 0
INSERT INTO floats VALUES ('NAN'), (1)
SELECT f FROM floats WHERE f<>1
SELECT f FROM floats WHERE f > 0 ORDER BY ALL
SELECT f FROM floats WHERE f >= 1 ORDER BY ALL
SELECT f FROM floats WHERE f<=1
SELECT f, SUM(i) OVER (PARTITION BY f) FROM floats ORDER BY f
SELECT f, i, SUM(i) OVER (ORDER BY f, i) FROM floats ORDER BY f, i
SELECT f, i, SUM(i) OVER (PARTITION BY f ORDER BY f, i) FROM floats ORDER BY f, i
SELECT i, f, SUM(i) OVER (ORDER BY i, f) FROM floats ORDER BY i, f
SELECT ['nan'::double]
SELECT UNNEST(['nan'::double])
SELECT {'a': 'nan'::double}
SELECT ({'a': 'nan'::double}).a
SELECT case when 1=1 then [1] else [2] end
SELECT case when 1=0 then [1] else [2] end
SELECT case when i%2=0 then [i] else [-i] end from range(5) tbl(i)
CREATE TABLE a AS SELECT case when i%2=0 then null else [i] end i from range(10) tbl(i)
select * from a
select case when i=[1] then [3] else [4] end from a
SELECT [1] < [2]
SELECT [1] < [1]
SELECT NULL < [1]
SELECT [1] < NULL
SELECT [1] <= [2]
SELECT [1] <= [1]
SELECT NULL <= [1]
SELECT [1] <= NULL
SELECT [1] = [2]
SELECT [1] = [1]
SELECT NULL = [1]
SELECT [1] = NULL
CREATE table x1 (b INT[])
SELECT b || NULL from x1
SELECT NULL || NULL from x1
SELECT NULL || b || NULL from x1
SELECT b || NULL || b from x1
select concat([42])
select concat([42], [43], [], [44], [], [45])
select concat([42]::INT[1], [43]::INT[1], NULL::INT[1], [44]::INT[1], NULL::INT[1], [45]::INT[1])
select list_concat([42])
select list_concat([42], [43], [], [44], [], [45])
select list_concat([42]::INT[1], [43]::INT[1], NULL::INT[1], [44]::INT[1], NULL::INT[1], [45]::INT[1])
select list_concat([1]::INT[1], [2, 3]::INT[2])
CREATE VIEW v1 AS SELECT * FROM (VALUES (1, [1, 2, 3]), (2, NULL), (3, [NULL, 3, 4])) tbl (a, b)
SELECT * FROM v1 v, v1 w WHERE v.a <> w.a OR v.a>w.a ORDER BY v.a, w.a
CREATE VIEW v2 AS SELECT * FROM (VALUES (1, {'a': [1, 2, 3]}), (2, NULL), (3, {'a': [NULL, 3, 4]})) tbl (a, b)
SELECT * FROM v2 v, v2 w ORDER BY v.a, w.a
SELECT * FROM v2 v, v2 w WHERE v.a >= w.a ORDER BY v.a, w.a
SELECT * FROM v2 v, v2 w WHERE v.a <> w.a ORDER BY v.a, w.a
SELECT * FROM v2 v, v2 w WHERE v.a <> w.a OR v.a > w.a ORDER BY v.a, w.a
CREATE VIEW v3 AS SELECT * FROM (VALUES (1, [[1, 2], [3]]), (2, NULL), (3, [[NULL, 3], [4]])) tbl (a, b)
SELECT * FROM v3 v, v3 w ORDER BY v.a, w.a
SELECT * FROM v3 v, v3 w WHERE v.a >= w.a ORDER BY v.a, w.a
SELECT * FROM v3 v, v3 w WHERE v.a <> w.a ORDER BY v.a, w.a
SELECT * FROM v3 v, v3 w WHERE v.a <> w.a OR v.a > w.a ORDER BY v.a, w.a
SELECT [1] IS NOT DISTINCT FROM [2]
SELECT [1] IS NOT DISTINCT FROM [1]
SELECT NULL IS NOT DISTINCT FROM [1]
SELECT [1] IS NOT DISTINCT FROM NULL
SELECT [1] IS DISTINCT FROM [2]
SELECT [1] IS DISTINCT FROM [1]
SELECT NULL IS DISTINCT FROM [1]
SELECT [1] IS DISTINCT FROM NULL
CREATE VIEW list_int1 AS SELECT * FROM (VALUES ([1], [1]), ([1], [2]), ([2], [1]), (NULL, [1]), ([2], NULL), (NULL, NULL) ) tbl(l, r)
SELECT l IS NOT DISTINCT FROM r FROM list_int1
SELECT l IS DISTINCT FROM r FROM list_int1
SELECT [1] IS NOT DISTINCT FROM [1, 2]
select string_split(string_agg(NULL, ','), ',')[100] from range(10)
CREATE TABLE a AS SELECT [ {'a': 3, 'b': NULL}, NULL, {'a': NULL, 'b': 'hello'} ] l
SELECT l[1] FROM a
SELECT l[2] FROM a
SELECT l[3] FROM a
CREATE TABLE nested AS SELECT [ {'a': 3, 'b': {'x': 3, 'y': [1, 2, 3]}}, NULL, {'a': NULL, 'b': {'x': NULL, 'y': [4, 5]}}, {'a': 27, 'b': NULL}, {'a': NULL, 'b': {'x': 7, 'y': NULL}} ] l
SELECT * FROM nested
SELECT l[1] FROM nested
SELECT l[2] FROM nested
SELECT l[3] FROM nested
SELECT l[4] FROM nested
SELECT l[5] FROM nested
SELECT l[5]['b'] FROM nested
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT[])
INSERT INTO a VALUES (1, [1, 2, 3])
INSERT INTO a VALUES (2, NULL), (3, [NULL]), (4, [4, 5, NULL, 6])
DROP TABLE a
CREATE TABLE a(id INTEGER, c INT[])
INSERT INTO a VALUES (1, [1, 2, 3]), (2, NULL), (3, [NULL]), (4, [4, 5, NULL, 6])
CREATE INDEX a_index ON a(id)
INSERT INTO a VALUES (1, [4, 5, NULL]), (1, NULL), (1, [NULL]), (1, [7, 8, 9, 10, 11, 12, 13, 14, 15])
SELECT * FROM a WHERE id=1 ORDER BY c[1] NULLS FIRST
INSERT INTO a SELECT i id, NULL c FROM range(2, 2500, 1) tbl(i)
SELECT c FROM a WHERE id=1
INSERT INTO a SELECT i id, [-i, i, 33] c FROM range(-2, -2500, -1) tbl(i)
INSERT INTO a SELECT i id, [1, 2, 3, 4, 5, i, -33] c FROM range(2500, 5000, 1) tbl(i)
INSERT INTO a VALUES (2, [4, 5])
INSERT INTO a VALUES (3, NULL)
INSERT INTO a VALUES (4, [NULL])
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT[][])
INSERT INTO a VALUES (1, [[1, 2, 3], [4, 5]])
INSERT INTO a SELECT i id, [[-i], [i, 33]] c FROM range(-2, -2500, -1) tbl(i)
INSERT INTO a SELECT i id, [[1, 2], [3, 4], [5, i, -33]] c FROM range(2500, 5000, 1) tbl(i)
INSERT INTO a VALUES (2, [[4, 5]])
INSERT INTO a VALUES (5, [[NULL], [NULL]])
CREATE TABLE test (l INTEGER[])
INSERT INTO test VALUES ([1, 2, 3]), ([NULL]), (NULL), ([-2, NULL, 4, 2])
SELECT list_extract(l, 0) FROM test
SELECT list_extract(l, 1) FROM test
SELECT l[:] FROM test
SELECT l[0:0] FROM test
SELECT l[0:1] FROM test
SELECT l[1:0] FROM test
CREATE VIEW list_int AS SELECT * FROM (VALUES ([1]), ([1, 2]), ([1, NULL]), ([NULL, 2]), ([NULL, NULL]), ([NULL]), (NULL) ) tbl(i)
SELECT lhs.i, rhs.i, lhs.i < rhs.i, lhs.i <= rhs.i, lhs.i = rhs.i, lhs.i <> rhs.i, lhs.i > rhs.i, lhs.i >= rhs.i, lhs.i IS NOT DISTINCT FROM rhs.i, lhs.i IS DISTINCT FROM rhs.i FROM list_int lhs, list_int rhs ORDER BY 1, 2
CREATE VIEW list_str AS SELECT * FROM (VALUES (['duck']), (['duck', 'goose']), (['duck', NULL]), ([NULL, 'goose']), ([NULL, NULL]), ([NULL]), (NULL) ) tbl(i)
SELECT lhs.i, rhs.i, lhs.i < rhs.i, lhs.i <= rhs.i, lhs.i = rhs.i, lhs.i <> rhs.i, lhs.i > rhs.i, lhs.i >= rhs.i, lhs.i IS NOT DISTINCT FROM rhs.i, lhs.i IS DISTINCT FROM rhs.i FROM list_str lhs, list_str rhs ORDER BY 1, 2
CREATE VIEW list_int AS SELECT * FROM VALUES ( ([1]), ([NULL]) ) tbl(a, b)
SELECT tbl.a, tbl.b, tbl.a < tbl.b, tbl.a <= tbl.b, tbl.a = tbl.b, tbl.a <> tbl.b, tbl.a > tbl.b, tbl.a >= tbl.b, tbl.a IS NOT DISTINCT FROM tbl.b, tbl.a IS DISTINCT FROM tbl.b FROM list_int tbl
CREATE TABLE a AS SELECT [{'a': 3, 'b': 'hello'}, NULL, {'a': NULL, 'b': 'thisisalongstring'}] l
INSERT INTO a VALUES ([{'a': 17, 'b': 'world'}])
SELECT UNNEST(l) FROM a
CREATE TABLE b AS SELECT [ {'a': {'a1': [1, 2, 3], 'a2': 17}, 'b': 'hello'}, NULL, {'a': {'a1': [NULL, 4, 5], 'a2': NULL}, 'b': 'thisisalongstring'}, {'a': {'a1': NULL, 'a2': 22}, 'b': NULL}, {'a': NULL, 'b': 'aaaaaaaaaaaaaaaaaaaaaaaa'}] l
SELECT * FROM b
SELECT UNNEST(l) FROM b
SELECT UNNEST(l)['a']['a1'] FROM b
SELECT UNNEST(l)['a']['a2'] FROM b
INSERT INTO b VALUES (NULL), ([ {'a': {'a1': [6, 7, 8, 9], 'a2': 17}, 'b': 'world1'}, NULL, {'a': {'a1': [10, 11, 12], 'a2': 22}, 'b': 'world2'} ])
SELECT UNNEST(l)['a'] FROM b
SELECT STATS([3, 4])
SELECT [3, 4]
SELECT STATS(NULL::INT[])
SELECT NULL::INT[]
SELECT STATS(['hello', 'world'])
SELECT STATS([interval 1 year, interval 2 year])
SELECT ['hello', 'world']
SELECT [interval 1 year, interval 2 year]
CREATE TABLE integers(i integer)
insert into integers values (3), (4)
SELECT STATS([i]) FROM integers LIMIT 1
SELECT [i] FROM integers
CREATE TABLE a(b INTEGER[])
INSERT INTO a VALUES ([1, 2]), (NULL), ([3, 4, 5, 6]), ([NULL, 7])
CREATE TABLE b(b INTEGER[][])
INSERT INTO b VALUES ([[1, 2], [3, 4]]), (NULL), ([NULL, [7, 8, NULL], [2, 3]]), ([[NULL, 6], NULL, [1, 2, NULL]])
CREATE TABLE c(b VARCHAR[])
INSERT INTO c VALUES (['hello', 'world']), (NULL), (['fejwfoaejwfoijwafew', 'b', 'c']), ([NULL, 'XXXXXXXXXXXXXXXXXXXXXXXX'])
SELECT * FROM c
CREATE TABLE lists(i INT[])
INSERT INTO lists SELECT [i, NULL, i+1] FROM range(10000) tbl(i)
DELETE FROM lists WHERE i[1] <= 9995
SELECT * FROM lists
DROP TABLE lists
CREATE TABLE lists(i INT[][])
INSERT INTO lists SELECT [[i], NULL, [i+1, 4], [NULL, 1, 2]] FROM range(10000) tbl(i)
DELETE FROM lists WHERE i[1][1] <= 9995
SELECT concat_ws('.', list_reverse(string_split('1.2..3', '.')))
create table lists(id int, i int[])
insert into lists values (1, [1, 2, 3]), (2, [4, 5]), (3, [NULL])
select * from lists order by id
update lists set i=[5,6,7] from lists l2 where lists.id=1
CREATE TABLE a(id INTEGER, b INTEGER[])
INSERT INTO a VALUES (0, [1, 2]), (1, NULL), (2, [3, 4, 5, 6]), (3, [NULL, 7])
DELETE FROM a WHERE b[1]=1
UPDATE a SET b=[7, 8, 9] WHERE b IS NULL
UPDATE a SET b=NULL WHERE id>=2
UPDATE a SET b=[NULL] WHERE id=2
CREATE TABLE a(id INTEGER, b VARCHAR[])
INSERT INTO a VALUES (0, ['hello world', 'bananas']), (1, NULL), (2, ['3, 4, 5, 6', 'numbers']), (3, [NULL, 'not a number'])
DELETE FROM a WHERE b[1][1]='3'
UPDATE a SET b=['very very long string', '123', 'test 123 123'] WHERE b IS NULL
UPDATE a SET b=[NULL, 'hello again', NULL] WHERE id=1
select [100::UTINYINT, 10000::SMALLINT]
select [100::USMALLINT, 10000::INTEGER]
select [100::USMALLINT, 10000.5]
select [100::USMALLINT, 0.5::DOUBLE]
select [-100::TINYINT, 200::UTINYINT]
select [-100::SMALLINT, 50000::USMALLINT]
select [-100::INTEGER, 3000000000::UINTEGER]
select [-100::BIGINT, 9999999999999999999::UBIGINT]
CREATE TABLE a(id INTEGER, b INTEGER[][])
INSERT INTO a VALUES (0, [[1, 2], NULL, [3, NULL]]), (1, NULL), (2, [[4, 5, 6, 7], [NULL]])
SELECT id, b[1] FROM a ORDER BY id
SELECT id, b[1][1] FROM a ORDER BY id
SELECT id, b[0][0] FROM a ORDER BY id
SELECT id, b[0][1] FROM a ORDER BY id
SELECT id, b[1][0] FROM a ORDER BY id
SELECT id, b[1][4] FROM a ORDER BY id
SELECT * FROM a WHERE b[1][1]=1
SELECT * FROM a WHERE b[1][1]=1 OR b[1][2]=2
CREATE TABLE nested(id INTEGER, b INTEGER[][][])
INSERT INTO nested VALUES (0, [[[1, 2], [3, 4]], NULL, [NULL, [2, 5]]]), (1, NULL), (2, [[[6, 7, 8, 9], [10, 11], [12, 13]], NULL, [NULL, [10, 11], [12, 13]]])
SELECT id, b[0:1] FROM a ORDER BY id
SELECT id, b[0:2] FROM a ORDER BY id
SELECT id, b[1:1] FROM a ORDER BY id
SELECT id, b[1:2] FROM a ORDER BY id
SELECT id, b[0:0] FROM a ORDER BY id
SELECT id, b[:] FROM a ORDER BY id
SELECT id, list_extract(b[:], 0) FROM a ORDER BY id
UPDATE a SET b=[[7, 8, 9], [10, 11]] WHERE b IS NULL
UPDATE a SET b=NULL WHERE id>=1
UPDATE a SET b=[[NULL], NULL, [NULL]] WHERE id=1
SELECT UNNEST([[1, 2, 3]], recursive := true)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], recursive := true)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], RECURSIVE := true)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], max_depth := 1)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], max_depth := 2)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], max_depth := 3)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], max_depth := 4)
SELECT UNNEST([[[[[1, 2], [3, 4]], [[5]]], [[[]]]]], max_depth := 5)
SELECT UNNEST([[1, 2, 3], [4, 5]]) AS a, UNNEST([1, 2, 3]) AS b
SELECT UNNEST([[1, 2, 3], [4, 5]], recursive := true) AS a, UNNEST([1, 2, 3]) AS b ORDER BY a NULLS LAST
SELECT UNNEST(a), b FROM (SELECT UNNEST([[1, 2, 3], [4, 5]]) AS a, UNNEST([1, 2, 3]) AS b)
SELECT UNNEST([1, 2, 3], recursive := true, recursive := true)
SELECT SUM(a) FROM UNNEST(RANGE(1, 11)) t(a)
create or replace function rnv(a,b) as (select a + b * pi())
select rnv(0, 1) from unnest( range(0,2) )
SELECT id, UNNEST(i), UNNEST(j) FROM (VALUES (3, ['hello', NULL, 'world'], [NULL])) tbl(id, i, j)
SELECT id, UNNEST(i), UNNEST(j) FROM (VALUES (1, ['abcd', 'efgh'], ['123456789abcd']), (2, NULL, ['123456789efgh', '123456789klmnop']), (3, ['hello', NULL, 'world'], [NULL])) tbl(id, i, j)
SELECT id, UNNEST(i), UNNEST(j) FROM (VALUES (1, [1, 2], [10]), (2, NULL, [11, 12]), (3, [3, NULL, 4], [NULL])) tbl(id, i, j)
SELECT UNNEST(i) FROM (VALUES ([[1, 2, 3], [4, 5]]), (NULL), ([[6, 7], NULL, [8, 9, NULL]])) tbl(i)
SELECT UNNEST(i), UNNEST(j) FROM (VALUES ([[1, 2, 3], [4, 5]], [[10, 11], [12, 13]]), (NULL, [[14, 15], [NULL, 16], NULL, NULL]), ([[6, 7], NULL, [8, 9, NULL]], NULL)) tbl(i, j)
SELECT UNNEST(i) FROM (VALUES ([{'a': 10, 'b': 1}, {'a': 11, 'b': 2}]), (NULL), ([{'a': 12, 'b': 3}, NULL, {'a': NULL, 'b': NULL}])) tbl(i)
SELECT UNNEST(i) FROM (VALUES ([{'a': {'a1': 7, 'a2': NULL}, 'b': 1}, {'a': {'a1': 9, 'a2': 10}, 'b': 2}]), (NULL), ([{'a': {'a1': 11, 'a2': 12}, 'b': 3}, NULL, {'a': NULL, 'b': NULL}, {'a': {'a1': NULL, 'a2': NULL}, 'b': 3}])) tbl(i)
SELECT id, UNNEST(i), UNNEST(j) FROM (VALUES (1, [{'a': 1, 'b': NULL}, {'a': 2, 'b': 'hello'}], [[1, 2, 3], [4, 5]]), (2, NULL, [[11, 12], NULL]), (3, [{'a': 3, 'b': 'test the best unnest fest'}, NULL, {'a': 4, 'b': 'abcd'}], [NULL])) tbl(id, i, j)
SELECT id, UNNEST(i), UNNEST(j) FROM (VALUES (1, [{'a': [1, 2], 'b': NULL}, {'a': NULL, 'b': 'hello'}], [[1, 2, 3], [4, 5]]), (2, NULL, [[11, 12], NULL]), (3, [{'a': [NULL, 4, 5], 'b': 'test the best unnest fest'}, NULL, {'a': [6, 7, NULL, 9], 'b': 'abcd'}], [NULL])) tbl(id, i, j)
SELECT id, UNNEST(i) FROM (VALUES (1, [[1,2], [3, 4]]::INT[2][]), (2, [[5, NULL], [7, 8]]::INT[2][]), (3, NULL::INT[2][]), (4, [[9, 10], NULL, [11, 12]]::INT[2][]), (5, []::INT[2][])) tbl(id, i)
SELECT id, UNNEST(i) FROM (VALUES (1, {'a': [1,2]::INT[2], 'b': [3, 4]::INT[2]}), (2, {'a': [5, NULL]::INT[2], 'b': [7, 8]::INT[2]}), (3, {'a': NULL::INT[2], 'b': [9, 10]::INT[2]}), (4, {'a': [11, 12]::INT[2], 'b': NULL::INT[2]}), (5, {'a': NULL, 'b': [13, 14]::INT[2]})) tbl(id, i)
CREATE TABLE tbl1 (str VARCHAR, str_list VARCHAR[])
INSERT INTO tbl1 VALUES ('a', ['vibrant', 'plant', 'day'])
CREATE TABLE tbl2 (data struct (str VARCHAR, str_list VARCHAR[]))
INSERT INTO tbl2 VALUES (('a', ['sunny', 'vibrant', 'day']))
SELECT UNNEST(data) FROM tbl2
SELECT UNNEST(str_list) FROM tbl1
SELECT UNNEST(data) FROM tbl2 INTERSECT SELECT * FROM tbl1
CREATE TABLE test (id VARCHAR, b STRUCT("n" VARCHAR, "v" STRUCT("n" VARCHAR, "v" BIGINT)[])[])
SELECT DISTINCT * FROM (SELECT id, UNNEST(b, recursive := true) FROM test)
SELECT DISTINCT id, UNNEST(b, recursive := true) FROM test
CREATE TABLE people(id INTEGER, name VARCHAR, address VARCHAR[])
insert into people values (1, 'Zuckerberg', ARRAY['New York'])
insert into people values (2, 'Bezos', ARRAY['Washington', 'Space'])
insert into people values (3, 'Tim', NULL)
insert into people values (4, 'Elvis', ARRAY[NULL, NULL, NULL])
insert into people values (5, 'Mark', ARRAY[]::VARCHAR[])
SELECT UNNEST(NULL)
SELECT UNNEST(NULL::BOOLEAN[])
SELECT name, UNNEST(address) FROM people
SELECT name, UNNEST(address), UNNEST([1]) FROM people
WITH t AS ( SELECT 1 AS r, ARRAY[1, 2, 3] AS a UNION SELECT 2 AS r, ARRAY[4] AS a UNION SELECT 3 AS r, NULL AS a) SELECT r, a, UNNEST(a) AS n FROM t ORDER BY r, n
WITH t AS ( SELECT 1 AS r, ARRAY[1, 2, 3] AS a UNION SELECT 2 AS r, ARRAY[4] AS a UNION SELECT 3 AS r, NULL AS a) SELECT r, a.value FROM t, (SELECT UNNEST(a)) AS a(value) ORDER BY r, a.value
SELECT * FROM UNNEST(ARRAY[1, 2, 3])
SELECT * FROM UNNEST([1, 2, 3]::INT ARRAY)
SELECT i FROM UNNEST(ARRAY[1, 2, 3]) AS tbl(i)
SELECT i FROM UNNEST(ARRAY[NULL, 'hello', 'world', 'bleorkbaejkoreijgaiorjgare']) AS tbl(i)
SELECT i FROM UNNEST([[1, 2], [3, 4], NULL, [4, 5, 6, 7]]) AS tbl(i)
SELECT i FROM UNNEST([{'a': [1, 2, 3], 'b': [4, 5, 6]}, {'a': [4, 5], 'b': [7, 8, 9, 10]}]) AS tbl(i)
SELECT COUNT(*) FROM UNNEST((SELECT LIST(range) FROM range(4000))) AS tbl(i)
SELECT i FROM UNNEST(NULL::INT[]) AS tbl(i)
SELECT i FROM UNNEST([]::INT[]) AS tbl(i)
CREATE TABLE lists AS SELECT [1,2,3] l UNION ALL SELECT [4,5] UNION ALL SELECT [] UNION ALL SELECT [NULL] UNION ALL SELECT [7, 8]
SELECT u FROM lists, UNNEST(l) AS unnest(u) ORDER BY l, u
PREPARE v1 AS SELECT * FROM UNNEST(?::INT[])
select l from a order by 1
select unnest(l) from a order by 1
create table a as select list(interval (i) years) AS l from range(1, 6, 1) t1(i)
create table a as select list(i%2=0) AS l from range(1, 6, 1) t1(i)
select [100::VARIANT, 1.2]
select ['test', 'hello', 'world'][1::VARIANT::INTEGER]
select ['test', 'hello', 'world'][1::VARIANT]
select ['test', 'hello', 'world'][1::BIGINT::VARIANT]
select 'true'::VARIANT::BOOL
select {'a': 'true'}::VARIANT::STRUCT(a BOOLEAN)
select '2019/03/21'::TIMESTAMP::VARIANT::TIMESTAMP
select '0.123456789'::DECIMAL(10,9)::VARIANT::DECIMAL(10,9)
select '0.123456789'::DECIMAL(10,9)::VARIANT::DECIMAL(10,8)
select {'a': '[1, 2, 3, 4]'}::VARIANT::STRUCT(a INTEGER[])
select {'a': ['1', '2', '3', '4']}::VARIANT::STRUCT(a INTEGER[])
create table struct_cast_tbl(a STRUCT(a VARCHAR))
select '"test"'::JSON::VARIANT
select ['"test"'::JSON for _ in range(10)]::JSON::VARIANT
select '{"hello": [1,2,true, false, null], "test": [1, {"test": false}, ["blob", "this is a long string", 123]]}'::JSON::VARIANT
WITH src(j) AS ( VALUES ('{"n":123456789012345678901234567890}'::JSON) ) SELECT (j::VARIANT).n as value, variant_typeof(value) as variant_type FROM src
select '"test"'::JSON::VARIANT::JSON
select ['"test"' for _ in range(10)]::JSON::VARIANT::JSON
select '{"hello": [1,2,true, false, null], "test": [1, {"test": false}, ["blob", "this is a long string", 123]]}'::JSON::VARIANT::JSON
select '{"hello": [1,2,true, false, null], "test": [1, {"test": false}, ["blob", "this is a long string", 123]]}'::JSON::VARIANT::JSON from range(10)
create table tbl(var JSON)
insert into tbl select '{"test":123,"hello":[1,2,3],"world":{"test":true}}'
insert into tbl select '{"test":123,"hello":[4,5,6],"world":{"test":true}}'
insert into tbl select '{"test":123,"hello":[],"world":{"test":true}}'
select struct_pack(*COLUMNS(*))::VARIANT::JSON from test_all_types()
select {'var': COLUMNS(*)::VARIANT}::VARIANT from test_all_types()
select [COLUMNS(['tinyint', 'double'])::VARIANT, NULL, COLUMNS(['tinyint', 'double'])::VARIANT] from test_all_types()
select [COLUMNS(['tinyint', 'double'])::JSON, NULL, COLUMNS(['tinyint', 'double'])::JSON] from test_all_types()
create table foo.variant_lineitem as select variant_normalize(STRUCT_PACK(*COLUMNS(*))::VARIANT) from lineitem
select * from foo.variant_lineitem limit 10
select COLUMNS(*)::JSON from foo.variant_lineitem limit 10
select lineitem::json from lineitem limit 2
select lineitem::json::variant::JSON from lineitem limit 2
select try_cast('hello world'::variant as int) + 1
select try_cast('hello world'::variant as STRUCT(a varchar, b boolean))
select try_cast({'a': true, 'b': 'foo'}::variant as STRUCT(a varchar, b boolean))
select try_cast({'c': true, 'd': 'foo'}::variant as STRUCT(a varchar, b boolean))
select try_cast('hello world'::variant as INTEGER[])
with cte as ( from ( values ('test'::VARIANT), (null::VARIANT), (42::VARIANT) ) t(a) ) select try_cast(a as INT) from cte
with cte as ( from ( values ('test'::VARIANT), (null::VARIANT), (42::VARIANT) ) t(a) ) select try_cast(a as STRUCT(a varchar, b boolean)) from cte
with cte as ( from ( values ('test'::VARIANT), (null::VARIANT), (42::VARIANT) ) t(a) ) select try_cast(a as BOOLEAN[]) from cte
with cte as ( from ( values ('test'::VARIANT), (null::VARIANT), (42::VARIANT) ) t(a) ) select try_cast(a as UNION(a integer, b decimal)) from cte
select try_cast(json('["1","x"]')::VARIANT as INTEGER[])
select try_cast(json('["1","x"]')::VARIANT as INTEGER[2])
SELECT [1]::VARIANT IS NOT DISTINCT FROM [2]
SELECT [1]::VARIANT IS NOT DISTINCT FROM [1]
SELECT NULL IS NOT DISTINCT FROM [1]::VARIANT
SELECT [1] IS NOT DISTINCT FROM NULL::VARIANT
SELECT [1]::VARIANT IS DISTINCT FROM [2]
SELECT [1]::VARIANT IS DISTINCT FROM [1]
SELECT NULL::VARIANT IS DISTINCT FROM [1]
SELECT [1]::VARIANT IS DISTINCT FROM NULL
CREATE VIEW list_int1 AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [2]), ([2], [1]), (NULL, [1]), ([2], NULL), (NULL, NULL) ) tbl(l, r)
SELECT [1]::VARIANT IS NOT DISTINCT FROM [1, 2]
SELECT NULL::VARIANT IS NOT DISTINCT FROM [1]
SELECT [1]::VARIANT IS DISTINCT FROM [1, 2]
CREATE TABLE t AS SELECT i, MAP{'k': i::VARIANT} AS m FROM range(2) t(i)
SELECT m::VARIANT FROM t WHERE i = 1
CREATE TABLE big AS SELECT i, MAP{'k': i::VARIANT} AS m FROM range(3000) t(i)
SELECT i, m::VARIANT FROM big WHERE i IN (1, 2, 2050) ORDER BY i
begin transaction
create table union_tbl( col UNION( a JSON, b INTEGER, c TINYINT ) )
insert into union_tbl VALUES ( { tag: 0::UINT8, a: '{"a": "hello", "b": true}', b: null::INTEGER, c: null::TINYINT } )
insert into union_tbl VALUES ( { tag: 0::UINT8, a: '{"c": "world"}'::JSON, b: null::INTEGER, c: null::TINYINT } )
select * from union_tbl
rollback
create table union_tbl( col UNION( a BOOL, b INTEGER, c TINYINT ) )
create table struct_tbl( col STRUCT( tag UINT8, A BOOL, B INTEGER, C TINYINT ) )
INSERT INTO struct_tbl VALUES (ROW(0, True, NULL, NULL)), (ROW(1, NULL, 23423, NULL)), (ROW(0, True, NULL, NULL))
insert into union_tbl select * from struct_tbl
delete from struct_tbl
INSERT INTO struct_tbl VALUES (ROW(0, True, NULL, NULL)), (ROW(1, NULL, 23423, NULL)), (ROW(2, True, NULL, NULL))
insert into union_tbl VALUES( {tag: 0::UINT8, a: False, b: NULL::INTEGER, c: NULL::TINYINT} )
insert into struct_tbl VALUES (ROW(1::UINT8, NULL, 1, NULL)), (ROW(1::UINT8, NULL, 2, NULL)), (ROW(1::UINT8, NULL, 3, 0))
CREATE TABLE tbl1 (u UNION(num INT, str VARCHAR))
INSERT INTO tbl1 VALUES (1), ('bar'), (3), ('foo'), (2), ('baz')
SELECT FIRST(u), LAST(u) FROM tbl1
SELECT union_tag(u), max(u) FROM tbl1 GROUP BY union_tag(u)
SELECT union_tag(u), min(u) FROM tbl1 GROUP BY union_tag(u)
SELECT sum(u.num) FROM tbl1
SELECT LAST(u) FROM tbl1 GROUP BY union_tag(u) HAVING union_tag(u) = 'num'
SELECT max(u), min(u) FROM tbl1
CREATE TABLE tbl(a UNION(b INT, c INT))
INSERT INTO tbl VALUES (union_value(b := 1)), (union_value(c := 2)), (union_value(b := 3))
SELECT a.b FROM tbl
SELECT a.c FROM tbl
SELECT a FROM tbl
CREATE TABLE tbl2(a UNION(b STRUCT(foo VARCHAR), c STRUCT(foo VARCHAR)))
INSERT INTO tbl2 VALUES (union_value(b := {'foo': 'bar'})), (union_value(c := {'foo': 'baz'}))
SELECT a.b.foo FROM tbl2
SELECT a.c.foo FROM tbl2
SELECT a FROM tbl2
CREATE TABLE tbl3(a UNION(b INT, c STRUCT(b INT)))
INSERT INTO tbl3 VALUES (1), (union_value(b := 2)), (union_value(c := {'b': 3}))
create table tbl1(u UNION(i32 INT, str VARCHAR))
insert into tbl1 values (1) , ('two') , ('three')
SELECT * FROM tbl1
SELECT u::varchar FROM tbl1
create table tbl2(u UNION(str VARCHAR, i32 INT, f32 FLOAT))
insert into tbl2 values ('five'), (4), (6.0)
SELECT * FROM tbl2
SELECT u.i32, u.str, u.f32 FROM tbl2
SELECT * FROM tbl2 UNION ALL SELECT * FROM tbl1
SELECT * FROM tbl1 UNION ALL SELECT * FROM tbl2
SELECT u.i32, u.str, u.f32 FROM tbl2 UNION ALL SELECT u.i32, u.str, NULL FROM tbl1 ORDER BY ALL
INSERT INTO tbl2 SELECT * FROM tbl1
SELECT union_extract(1::UNION(a INT, b FLOAT), 'a')
CREATE TABLE tbl1 (u UNION(a INT, b FLOAT, c VARCHAR))
INSERT INTO tbl1 VALUES (1), ('text'), (2.0)
SELECT u.a FROM tbl1 WHERE u.a IS NOT NULL
SELECT u.a FROM tbl1
SELECT u.b FROM tbl1 WHERE u.b IS NOT NULL
SELECT u.b FROM tbl1
SELECT u.c FROM tbl1 WHERE u.c IS NOT NULL
SELECT u.c FROM tbl1
SELECT u.a, u.b, u.c FROM tbl1
SELECT union_extract(u, 'a') FROM tbl1 WHERE union_extract(u, 'a') IS NOT NULL
SELECT union_extract(u, 'a') FROM tbl1
CREATE TABLE tbl1(id INT, a UNION(b INT, c VARCHAR))
CREATE TABLE tbl2(id INT, d UNION(e INT, f VARCHAR))
INSERT INTO tbl1 VALUES (1, 1), (3, 'foo'), (2, 2), (4, 'bar')
INSERT INTO tbl2 VALUES (1, 'foo'), (2, 'bar'), (3, 1), (4, 2)
SELECT id, union_tag(a), a.b, a.c FROM tbl1 UNION SELECT id, union_tag(d), d.e, d.f FROM tbl2 ORDER BY ALL
SELECT id, union_tag(a) as tag, a.b as v1, a.c as v2 FROM tbl1 UNION SELECT id, union_tag(d) as tag, d.e as v1, d.f as v2 FROM tbl2 ORDER BY ALL
SELECT tbl1.a.c, tbl1.id, tbl2.id FROM tbl2 JOIN tbl1 ON tbl1.a.c = tbl2.d.f ORDER BY ALL
SELECT t1.id FROM tbl1 as t1 JOIN tbl1 as t2 ON t1.a = t2.a ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 INNER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 FULL OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 LEFT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 RIGHT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
INSERT INTO tbl1 VALUES (1), ('bar'), (2), ('foo'), (3), ('baz')
SELECT * FROM tbl1 LIMIT 1
SELECT * FROM tbl1 OFFSET 1
SELECT * FROM tbl1 OFFSET 1 LIMIT 1
SELECT * FROM tbl1 WHERE u.str IS NOT NULL OFFSET 1
SELECT * FROM tbl1 WHERE u.str IS NOT NULL LIMIT 1 OFFSET 1
INSERT INTO tbl1 VALUES (union_value(a256 := 1337)), (union_value(a1 := 42))
SELECT u.a256 FROM tbl1
SELECT u.a1 FROM tbl1
CREATE TABLE tbl1 (union_list UNION(str VARCHAR, num INT)[])
INSERT INTO tbl1 VALUES ([1::UNION(str VARCHAR, num INT), 'one']), (['two'::UNION(str VARCHAR, num INT), 2]), ([3::UNION(str VARCHAR, num INT), 'three', '3']), ([4]), (list_value('five')), ([6])
CREATE TABLE tbl2 (union_with_list UNION(list INT[], num INT))
INSERT INTO tbl2 VALUES ([1, 2, 3]), (4), ([5]), (6), (NULL), (7), (list_value(8, 9, 10))
SELECT * FROM tbl2 WHERE union_with_list = [5]
SELECT union_with_list.num FROM tbl2
SELECT union_list[1] FROM tbl2 JOIN tbl1 ON union_with_list.num = union_list[1].num
SELECT union_list[1] FROM tbl2 JOIN tbl1 ON union_with_list.num = union_list[1]
CREATE TABLE tbl3 (union_with_lists UNION(strs VARCHAR[], nums INT[]))
INSERT INTO tbl3 VALUES (union_value(strs:=['one', 'two'])), (union_value(nums:=[1, 2])), (union_value(strs:=['three', NULL])), (union_value(nums:=[3, 4])), (union_value(strs:=['five'])), (union_value(nums:=[5])), (union_value(strs:=['six'])), (union_value(nums:=[NULL, 6])), (union_value(strs:=NULL)), (union_value(strs:=[1]))
SELECT union_tag(union_with_lists), union_with_lists FROM tbl3
CREATE TABLE tbl (a UNION(a INT, b INT))
INSERT INTO tbl VALUES (union_value(b := 1)), (union_value(a := 4)), (union_value(a := 1)), (union_value(b := 2)), (union_value(a := 3)), (NULL)
SELECT union_tag(a), a FROM tbl ORDER BY a ASC
SELECT union_tag(a), a FROM tbl ORDER BY a DESC
CREATE TABLE tbl5 (a UNION(lft INT, u UNION(lft VARCHAR, rght INT)))
INSERT INTO tbl5 VALUES (union_value(lft := 1))
CREATE TABLE tbl2 (u UNION(lft INT, u UNION(lft VARCHAR, rght INT)))
INSERT INTO tbl2 VALUES (union_value(lft := 1))
INSERT INTO tbl2 VALUES (NULL)
INSERT INTO tbl2 VALUES (union_value(u := union_value(rght := 2)))
INSERT INTO tbl2 VALUES (union_value(u := union_value(lft := '3')))
INSERT INTO tbl2 VALUES (union_value(u := '4'))
CREATE TABLE tbl1 (union_struct UNION(str VARCHAR, obj STRUCT(k VARCHAR, v INT)))
INSERT INTO tbl1 VALUES ({'k': 'key1', 'v': 1}), ('not a struct'), (NULL), ({'k': NULL, 'v': 1}), ({'k': 'key2', 'v': NULL}), ('key2')
SELECT union_struct.obj.k FROM tbl1
SELECT union_struct.obj.v FROM tbl1
SELECT union_struct.str FROM tbl1
SELECT * FROM tbl1 as l JOIN tbl1 as r ON l.union_struct.str = r.union_struct.obj.k
CREATE TABLE tbl2 (struct_union STRUCT(str VARCHAR, alt UNION(k VARCHAR, v INT)))
INSERT INTO tbl2 VALUES ({'str': 'key1', 'alt': 1}), ({'str': 'key2', 'alt': 'key2'}), ({'str': NULL, 'alt': NULL}), ({'str': NULL, 'alt': union_value(v := NULL)}), ({'str': 'key3', 'alt': union_value(k := NULL)}), ({'str': 'key4', 'alt': 'key2'})
SELECT * FROM tbl1 JOIN tbl2 ON tbl1.union_struct.str = tbl2.struct_union.alt.k order by all
SELECT union_tag(1::UNION(a INT, b VARCHAR))
SELECT union_tag(u) FROM tbl1
SELECT u FROM tbl1
SELECT union_tag(u) FROM tbl1 WHERE u = (1::UNION(a INT, b FLOAT, c VARCHAR))
SELECT enum_first(union_tag(u)) FROM tbl1 LIMIT 1
SELECT enum_last(union_tag(u)) FROM tbl1 LIMIT 1
SELECT enum_range(union_tag(u)) FROM tbl1 LIMIT 1
SELECT union_tag('foo'::UNION(num INT, str VARCHAR))
PREPARE p1 as SELECT union_tag(u) FROM tbl1
EXECUTE p1
PREPARE p2 as SELECT union_tag(?)
EXECUTE p2('woo'::UNION(a INT, b VARCHAR))
CREATE TABLE tbl(u UNION(i INTEGER, f FLOAT))
INSERT INTO tbl VALUES (1::INTEGER)
SELECT * from tbl
INSERT INTO tbl VALUES (2.0::FLOAT)
SELECT * FROM tbl
SELECT u.i FROM tbl
SELECT union_tag(u) FROM tbl
INSERT INTO tbl SELECT i from range(10) tbl(i)
CREATE TABLE tbl2(k VARCHAR, u UNION(num INTEGER, str VARCHAR) DEFAULT 'not set')
INSERT INTO tbl2(k) VALUES ('a'), ('b'), ('c')
SELECT u FROM tbl2
CREATE TABLE tbl3(k VARCHAR, u UNION(numeric UNION(i INTEGER, f FLOAT), str VARCHAR) DEFAULT 13.37::FLOAT)
CREATE TABLE tbl (u UNION(a INT, b VARCHAR))
INSERT INTO tbl VALUES (1), (NULL), (NULL::VARCHAR), (NULL::INT)
DELETE FROM tbl
SELECT union_tag(u) as tag, u as val FROM tbl
CREATE TABLE tbl(u UNION(num INTEGER, str STRING))
INSERT INTO tbl VALUES (union_value(num := 1))
INSERT INTO tbl VALUES (union_value(num := 1)), (1), (union_value(str := 'hello')), (2), ('world')
INSERT INTO tbl SELECT union_value(num := 1)::UNION(num INTEGER, str STRING) UNION ALL SELECT union_value(str := 'hello')::UNION(num INTEGER, str STRING)
SELECT CASE WHEN union_tag(u) = 'num' THEN u ELSE NULL END AS num FROM tbl
CREATE TABLE tbl3 (u UNION(num INTEGER, str STRING))
INSERT INTO tbl3 VALUES (union_value(num := 1)), (union_value(num := NULL)), (union_value(str := '3')), (union_value(str := NULL)),
SELECT u from tbl3 where u = NULL
SELECT union_value(str := NULL) IS NULL
SELECT union_tag(union_value(str := NULL))
SELECT union_tag(u), u FROM tbl3
create table t as select concat('1', repeat('0', i))::bignum as a from range(0,100) tbl(i)
select sum(a) from t
select sum(a) from t where a < 10000000
select 1.7976931348623157E+308::double::bignum = '179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368'::bignum
select (-1.7976931348623157E+308)::double::bignum = '-179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368'::bignum
select '179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368'::bignum::double = '1.7976931348623157E+308'::double
select '-179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368'::bignum::double = '-1.7976931348623157E+308'::double
select 3.4028235E+38::float::bignum = '340282346638528859811704183484516925440'::bignum
select (-3.4028235E+38)::float::bignum = '-340282346638528859811704183484516925440'::bignum
CREATE TABLE bignum_comparisons(a bignum)
select a, a < '9223372036854775807'::bignum from bignum_comparisons
select a, a <= '9223372036854775807'::bignum from bignum_comparisons
select a, a = '9223372036854775807'::bignum from bignum_comparisons
select a, a > '9223372036854775807'::bignum from bignum_comparisons
select a, a >= '9223372036854775807'::bignum from bignum_comparisons
select a, a != '9223372036854775807'::bignum from bignum_comparisons
select a, a < '2147483647'::bignum from bignum_comparisons
select a, a <= '2147483647'::bignum from bignum_comparisons
select a, a = '2147483647'::bignum from bignum_comparisons
select a, a > '2147483647'::bignum from bignum_comparisons
select a, a >= '2147483647'::bignum from bignum_comparisons
select '100'::bignum::double
select '100000'::bignum::double
select '1000000000000000'::bignum::double
select '340282366920938463463374607431768211455'::bignum::double
select '-100'::bignum::double
select '-100000'::bignum::double
select '-1000000000000000'::bignum::double
select '-340282366920938463463374607431768211455'::bignum::double
select 179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368::bignum::double
Select 85070591730234614260976917445211069672::BIGNUM
Select (-85070591730234614260976917445211069672)::BIGNUM
create table t as select (10 * power(10,i))::hugeint as i from range (0,38) t(i)
select distinct i::varchar == i::bignum::varchar FROM t
select distinct (-i)::varchar == (-i)::bignum::varchar FROM t
select distinct i::uhugeint::varchar == i::uhugeint::bignum::varchar FROM t
create table t (a bignum)
insert into t values (1), (10), (1000) , (33999999014383402399481480781255147520::DOUBLE), (17976931348623157081452742373170435679807056752584499659891747)
insert into t values (-17976931348623157081452742373170435679807056752584499659891747680315726078002853876058)
drop table t
insert into t values (1000000000000000000000000000000000000000000000000000000000000000000000000), (100000000000000000000000000000000000000000000000000000000000000000000000), (10000000000000000000000000000000000000000000000000000000000000000000000), (1000000000000000000000000000000000000000000000000000000000000000000000), (100000000000000000000000000000000000000000000000000000000000000000000)
SELECT -(1::BIGNUM)
SELECT -(0::BIGNUM)
SELECT -(-0::BIGNUM)
SELECT -(NULL::BIGNUM)
SELECT -('99999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999'::BIGNUM)
SELECT 10::bignum - 7::bignum
create table T (a bignum, b bignum)
insert into T values (0,0), (NULL, 10), (10, NULL), (100, -10), (-10, 100), (888, 271)
SELECT a-b,a,b FROM T
SELECT ('99999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999'::BIGNUM) - ('1'::BIGNUM)
SELECT ('1'::BIGNUM) - (-'9999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999'::BIGNUM)
SELECT "-"(c1, c2) FROM test_vector_types(CAST(NULL AS BIGNUM), CAST(NULL AS BIGNUM)) AS test_vector_types(c1, c2)
CREATE TABLE numbers ( x bignum )
select sum(x) from (values ('1'::bignum), ('-2'::bignum)) t(x), range(100000) t(y)
INSERT INTO numbers (x) VALUES (9223372036854775808::BIGNUM), (1::BIGNUM)
select (-1)::BIGNUM + 9223372036854775807::BIGNUM
select 9223372036854775808::BIGNUM + 1::BIGNUM
select (-10)::BIGNUM + (-1)::BIGNUM
select (-999999999)::BIGNUM + (-1)::BIGNUM
select 9223372036854775808::BIGNUM + (-1)::BIGNUM
select (-1000)::BIGNUM + (-1000)::BIGNUM
SELECT sum(x)::bignum FROM numbers
DROP TABLE numbers
INSERT INTO numbers (x) VALUES (255::BIGNUM), (255::BIGNUM)
select 179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368::double::bignum
select (-179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368)::double::bignum
select 33999999014383402399481480781255147520::float::bignum
select 33999999014383402399481480781255147520::double::bignum
select 0::double::bignum
select (-0)::double::bignum
select 1::double::bignum
select 100000::double::bignum
select 100000.595::double::bignum
select (-1)::bignum
select 0::bignum
select 1::bignum
CREATE TABLE integers(a bignum)
insert into integers values (0), (1), (-1)
select * from integers where a >= 0::BIGNUM
select * from integers where a < 0::BIGNUM
insert into integers values (300), (-300), (-10)
select (300)::BIGNUM
select (-300)::BIGNUM
select * from integers where a >= (-10)::BIGNUM
select * from integers where a = 1::BIGNUM
select '340282366920938463463374607431768211455'::UHUGEINT::bignum
select distinct i::varchar::bignum = i::bignum from range(-1000, 1000) t(i)
select '2147483646'::bignum = 2147483646::bignum
select '340282366920938463463374607431768211455'::UHUGEINT::bignum = '340282366920938463463374607431768211455'::bignum
select '-2147483646'::bignum = (-2147483646)::bignum
select '100'::bignum = 100::bignum
select '256'::bignum = 256::bignum
select '256'::bignum
select '2147483646'::bignum
select '21474836460000000000958'::bignum
select '-21474836460000000000958'::bignum
select '-21474836460000000000958214748364600000000009582147483646000000000095821474836460000000000958'::bignum
select '-0'::BIGNUM
select '+0'::BIGNUM
select '+0'::VARINT
select '-0010'::BIGNUM
select '-0010.'::BIGNUM
select '-0010.5'::BIGNUM
select '-0010.4999'::BIGNUM
select '0010.5'::BIGNUM
select '0010.4999'::BIGNUM
select '-0010.2'::BIGNUM
select '-0010.9'::BIGNUM
select '0010.2'::BIGNUM
CREATE TABLE test (a VARCHAR)
INSERT INTO test VALUES ('aaaaaaaaaa')
INSERT INTO test SELECT a||a||a||a||a||a||a||a||a||a FROM test WHERE LENGTH(a)=(SELECT MAX(LENGTH(a)) FROM test)
SELECT LENGTH(a) FROM test ORDER BY 1
CREATE TABLE emojis(id INTEGER, s VARCHAR)
INSERT INTO emojis VALUES (1, '🦆'), (2, '🦆🍞🦆')
SELECT * FROM emojis ORDER BY id
SELECT substring(s, 1, 1), substring(s, 2, 1) FROM emojis ORDER BY id
SELECT substring('u🦆', -2, 1)
SELECT substring('A3🦤u🦆f', -3, 3)
SELECT substring('🦤🦆f', -3, 2)
SELECT length(s) FROM emojis ORDER BY id
SELECT [{'i':1,'j':[2,3]},NULL]
SELECT [{'i':1,'j':[2,3]},NULL, {'i':1,'j':[2,3]}]
SELECT * FROM (VALUES (MAP(LIST_VALUE(1,2),LIST_VALUE(3,4))), (NULL), (MAP(LIST_VALUE(1,2),LIST_VALUE(3,4))), (NULL)) as a
SELECT MAP(LIST_VALUE({'i':1,'j':2},{'i':3,'j':4}),LIST_VALUE({'i':1,'j':2},{'i':3,'j':4}))
SELECT * FROM UNNEST(ARRAY[6]) AS x UNION ALL SELECT 2 FROM generate_series(1, 1)
CREATE TABLE tbl1 (a INT[3])
INSERT INTO tbl1 VALUES ([1, 2, 3]), ([4, NULL, 6]), ([7, 8, 9]), (NULL), ([10, 11, 12])
SELECT FIRST(a ORDER BY ALL), LAST(a ORDER BY ALL) FROM tbl1
SELECT COUNT(*), max(a) FROM tbl1 GROUP BY list_sum(a::INT[]) % 2 == 0
SELECT COUNT(*), max(a) FROM tbl1 GROUP BY list_sum(a::INT[]) % 2 == 0 HAVING list_sum(a::INT[]) % 2 == 0 NOT NULL
SELECT MAX(a), MIN(a) FROM tbl1
SELECT array_value(1, 2, 3)::VARCHAR
SELECT array_value(1, 2, 3)::INT[]
SELECT list_extract(array_value(1, 2, 3), 2)
SELECT unnest(array_value(1, 2, 3)::INT[])
SELECT array_value('1.0', '2.0', '3.0')::DOUBLE[3]
SELECT [1,2,3]::INT[3]
SELECT ['1.0', '2.0', '3.0']::DOUBLE[3]
SELECT NULL::INT[3]
SELECT [[1, 2, 3], [4, 5, 6]]::INT[3][2]
SELECT (NULL::INT[])::INT[3]
SELECT c::INT[3] FROM (VALUES ([1,2,3]), ([4,NULL,6]), (NULL), ([7,8,9])) as t(c)
CREATE OR REPLACE TABLE t1 AS SELECT [1, 2, 3]::INT[3]
SELECT DISTINCT array_value(array_value(1, 2, 3), array_value(4,5,6))
SELECT DISTINCT array_value([1,2,3], [4,5,6])
SELECT DISTINCT [array_value(1,2,3), array_value(4,5,6)]
SELECT * FROM (VALUES (array_value(NULL, 'abc')), (array_value(NULL, 'defg')), (NULL)) ORDER BY 1 DESC
SELECT * FROM (VALUES (array_value(NULL, 'ghf', NULL)), (array_value(NULL, NULL, 'defg')), (NULL)) ORDER BY 1 DESC
SELECT * FROM (VALUES (array_value(NULL, NULL, 'ghf')), (array_value(NULL, 'defg', NULL)), (NULL)) ORDER BY 1 DESC
create table tbl(c8 UTINYINT)
INSERT INTO tbl VALUES (0), (255), (NULL)
SELECT CAST(TRY_CAST(c8 AS ENUM('DUCK_DUCK_ENUM', 'GOOSE')) AS VARCHAR[3]) FROM tbl
CREATE TABLE array_tbl(c50 INTEGER[2][])
INSERT INTO array_tbl VALUES('[[1, 2], [1, 2]]')
INSERT INTO array_tbl VALUES('[[3, 4], [3, 4]]')
SELECT c50 FROM array_tbl GROUP BY ALL USING SAMPLE 3
CREATE TABLE test(c2 BOOL, c48 STRUCT(a INTEGER[3], b VARCHAR[3]))
INSERT INTO test VALUES(false, '{''a'': [NULL, 2, 3], ''b'': [a, NULL, c]}')
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types()
SELECT subq_0.c2 AS c4 FROM (SELECT ref_1.fixed_nested_int_array AS c2 FROM main.all_types AS ref_0 LEFT JOIN main.all_types AS ref_1 ON ((ref_0."varchar" !~~* ref_1."varchar"))) AS subq_0 RIGHT JOIN main.all_types AS ref_2 ON ((subq_0.c2 = ref_2.fixed_nested_int_array))
SELECT TRY_CAST(c36 AS INTEGER[][3]) FROM all_types AS t51(c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15, c16, c17, c18, c19, c20, c21, c22, c23, c24, c25, c26, c27, c28, c29, c30, c31, c32, c33, c34, c35, c36, c37, c38, c39, c40, c41, c42, c43, c44, c45, c46, c47, c48, c49, c50) WHERE c6
CREATE TABLE t1(i INTEGER, a INTEGER[3])
INSERT INTO t1 VALUES (1, array_value(1,2,3)), (2, array_value(NULL,5,6)), (3, array_value(7,NULL,9)), (4, array_value(10,11,NULL))
SELECT DISTINCT * FROM t1 ORDER BY ALL
CREATE OR REPLACE TABLE t1(i INTEGER, a INTEGER[][3])
INSERT INTO t1 SELECT i, array_value(list_value(i, i + 1, i + 2), list_value(i + 3, i + 4, i + 5), list_value(i + 6,i + 7,i + 8)) FROM range(0,9*9,9) as r(i)
SELECT * FROM t1 JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 JOIN t2 ON t1.a = t2.a ORDER BY ALL
SELECT * FROM t1 FULL OUTER JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 as a JOIN t1 as b ON (a.col1 != b.col1) ORDER BY ALL
CREATE TABLE test_array (c1 int[3])
INSERT INTO test_array values (null), (array[1, 2, 3]), (array[4, 5, 6]), (array[7, 8, 9])
SELECT * FROM test_array JOIN test_array AS t2 ON t2.c1 = test_array.c1
INSERT INTO test_array values (null)
INSERT INTO test_array values (array[10, 11, 12])
SELECT * FROM test_array JOIN test_array AS t2 ON t2.c1 = test_array.c1 ORDER BY test_array.c1
CREATE TABLE t1 (a INT[4096])
INSERT INTO t1 VALUES (range(0, 4096))
SELECT list_sum(a::INT[]) == list_sum(range(0, 4096)) FROM t1
CREATE TABLE t2 AS SELECT array_value(a) AS i FROM range(0, 4096) r(a)
SELECT sum(i[1]) FROM t2
CREATE TABLE t3 AS SELECT array_value(a, a+1) AS i FROM range(0, 4096) r(a)
SELECT sum(i[1]) == 8386560 AND sum(i[2]) == 8390656 FROM t3
CREATE OR REPLACE TABLE t4 AS SELECT * FROM range(0, 4096) as r(a)
UPDATE t4 SET a = NULL WHERE a % 2 = 0
CREATE TABLE t5 AS SELECT array_value(a) AS i FROM t4
SELECT sum(i[1]) FROM t5
CREATE TABLE arrays (a INTEGER[3])
INSERT INTO arrays VALUES ([1, 2, 3]), ([1, 2, 4]), ([7, 8, 9]), ([-1, -2, -3]), (NULL), ([4, NULL, 2])
SELECT list(a) FROM arrays
SELECT list(a ORDER BY a[3] ASC) FROM arrays
SELECT list(a ORDER BY a[3] DESC) FROM arrays
pragma disable_verification
SELECT list(array_value({'foo': [10]}))
CREATE TABLE t1 (a INT, b INT, c INT)
INSERT INTO t1 VALUES (1,2,3), (4,5,6)
SELECT list(array_value(a,b,c) ORDER By b) FROM t1 GROUP by c
SELECT a[3] FROM arrays
SELECT DISTINCT a FROM arrays ORDER BY ALL
SELECT DISTINCT a FROM arrays WHERE a[1] > 0 ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS UNION SELECT a FROM ARRAYS ) ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS WHERE a[1] > 0 UNION SELECT a FROM ARRAYS WHERE a[1] > 0 ) ORDER BY ALL
SELECT first(DISTINCT a ORDER BY a) FROM arrays
SELECT a::VARCHAR FROM arrays ORDER BY ALL
SELECT TRY_CAST(a::INTEGER[] AS INTEGER[3]) FROM ARRAYS ORDER BY ALL
SELECT a[2:-1] FROM arrays
SELECT a[3:99] FROM arrays
DESCRIBE SELECT * FROM arrays
SELECT a.filter(lambda x: x > 0) FROM arrays
CREATE OR REPLACE TABLE arrays2 (a INTEGER[3])
SELECT * FROM arrays2
SELECT a::JSON FROM arrays
create table arrays(id int primary key, a int[3])
insert into arrays select i, [i, i + 1, i +2] from range(200000) t(i)
select * from arrays where id=150000
SELECT array_value(1, 2, 3)
SELECT array_value(i -1, i, i + 1) FROM range(1,4) as r(i)
CREATE TABLE t1 (c INT[2])
INSERT INTO t1 VALUES (array_value(1, 2))
INSERT INTO t1 VALUES (array_value(3, 4))
SELECT * FROM t1 ORDER BY c DESC
CREATE OR REPLACE TABLE t1 AS SELECT * FROM (VALUES (array_value(6, NULL)), (array_value(1, 2)), (array_value(NULL,NULL)), (array_value(NULL, 3)))
SELECT * FROM t1 ORDER BY 1 DESC
SELECT * FROM t1 ORDER BY 1 ASC
CREATE OR REPLACE TABLE t1 AS SELECT * FROM VALUES (array_value(1,2), 1), (array_value(3,4), 2) as t(a, i)
SELECT MAX(i), arg_max(a, i) FROM t1
SELECT STATS(array_value(1,2))
PREPARE v1 AS SELECT array_cross_product($1::float[3], $2::float[3])
pragma preserve_insertion_order=true
CREATE TABLE tbl1 AS SELECT array_value(a, a) FROM range(0,122881) AS r1(a)
SELECT * FROM tbl1 LIMIT 1 OFFSET 0
SELECT * FROM tbl1 LIMIT 1 OFFSET 122880 // 2
SELECT * FROM tbl1 LIMIT 1 OFFSET 122879
SELECT * FROM tbl1 LIMIT 1 OFFSET 122880
SUMMARIZE arrays
SELECT a FROM arrays ORDER BY a LIMIT 1
SELECT min(a) FROM arrays
SELECT max(a) FROM arrays
INSERT INTO arrays VALUES ([-7, -8, -9]), ([-8, -9, -10])
SELECT TRY_CAST(array_value(1,2) as INTEGER[3])
SELECT TRY_CAST(x as INT[2][2]) FROM (VALUES ([[1,2],[3,4]]), ([[5,6],[7,8]])) AS t(x)
SELECT TRY_CAST(x as INT[2][2]) FROM (VALUES ([[1,2],[3,4]]), ([[5,6],[7,8,9]])) AS t(x)
SELECT TRY_CAST(x as INT[2][2]) FROM (VALUES ([[1,2],[3,4]]), ([[5,6],[7,8],[9,10]])) AS t(x)
SELECT TRY_CAST('[1,2]' as INTEGER[3])
SELECT CAST('[NULL, [1], [NULL]]' as INTEGER[1][3])
SELECT TRY_CAST('[NULL, [1], [abc]]' as INTEGER[1][3])
SELECT TRY_CAST('[NULL, [1,NULL,3], [1,2,3]]' as INTEGER[3][3])
SELECT CAST('[NULL, [1,NULL,3], [1,2,3]]' as INTEGER[3][3])
SELECT TRY_CAST('[NULL, [1,NULL,3], [1,2]]' as INTEGER[3][3])
SELECT TRY_CAST(test_vector AS INT[2]) AS a FROM test_vector_types(NULL::INTEGER[])
CREATE TABLE t1 (i VARCHAR[3])
INSERT INTO t1 VALUES (array_value('1',NULL,'3')), (NULL), (array_value(NULL,'5','6'))
SELECT DISTINCT * FROM t1
CREATE TABLE t2(i VARCHAR[2][2])
INSERT INTO t2 VALUES (array_value(array_value('1', NULL), array_value(NULL, '2'))), (NULL), (array_value(array_value('3', NULL), array_value(NULL, '4')))
SELECT DISTINCT * FROM t2
CREATE TABLE t3(i VARCHAR[2][])
INSERT INTO t3 VALUES (array_value(list_value('1', NULL), list_value(NULL, '2'))), (NULL), (array_value(list_value('3', NULL), list_value(NULL, '4')))
SELECT DISTINCT * FROM t3
CREATE TABLE t4(i VARCHAR[][2])
INSERT INTO t4 VALUES (list_value(array_value('1', NULL), array_value(NULL, '2'))), (NULL), (list_value(array_value('3', NULL), array_value(NULL, '4')))
SELECT DISTINCT * FROM t4
SELECT UNNEST(array_value(1, 2, NULL, 4, 5))
SELECT UNNEST(array_value('this is', 'a test', NULL, 'of unnesting arrays'))
CREATE TABLE t1 AS SELECT array_value(i + 1, i + 2) j FROM range(0, 10, 2) as t(i)
SELECT j, UNNEST(j) FROM t1
CREATE TABLE doubles_table (doubles_dynamic DOUBLE[], doubles_fixed DOUBLE[2])
INSERT INTO doubles_table VALUES ([1.2, 2.3], [1.2, 2.3])
SELECT UNNEST(doubles_dynamic) FROM doubles_table
SELECT UNNEST(doubles_fixed) FROM doubles_table
select min(struct_pack(i := i, j := i + 2)), max(struct_pack(i := i, j := i + 2)), first(struct_pack(i := i, j := i + 2)) from range(10) tbl(i)
select min(struct_pack(i := -i, j := -i - 2)), max(struct_pack(i := i + 2, j := i + 4)), first(struct_pack(i := i, j := i + 2)) from range(10) tbl(i)
select string_agg(struct_pack(i := i, j := i + 2)::VARCHAR, ',') from range(10) tbl(i)
select min(i), max(i), from struct_int
set threads=1
select min(i), max(i), first(i) from struct_int
SELECT MIN(s), MAX(s) FROM structs
DROP TABLE structs
SELECT MIN(s)['i'], MAX(s)['i'] FROM structs
CREATE TABLE structs AS SELECT {'i': i%2} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT {'i': interval (i+1) year} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT {'i': i::varchar || 'thisisalongsuffix'} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT {'i': 1} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT {'i': NULL} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT NULL::ROW(i INTEGER) AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT {'i': NULL::ROW(i INTEGER)} AS s FROM range(11) tbl(i)
CREATE TABLE structs AS SELECT [NULL::ROW(i INTEGER)] AS s FROM range(11) tbl(i)
SELECT {'i': 1, 'j': 2}
SELECT {'i': NULL, 'j': 2}
SELECT {'i': [], 'j': 2}
SELECT {'i': [1, 2, 3], 'j': 2}
SELECT {i: r, j: 2} FROM range(3) tbl(r)
create table tbl (data struct(str varchar)[])
insert into tbl (data) values ([struct_pack(str := 'value')]), (null), (null), (null)
select data[1].str as str from tbl where str is not null
create table t1 ( id int, k integer[], v decimal[] )
create table t2 (id int, v_map struct(key integer[], val decimal[]), k integer[])
insert into t2 select id, {'key': k, 'val': v}, k from t1
SELECT * FROM t2 order by id
SELECT * FROM t2 where id>=4 order by id
SELECT (SELECT {'a': 3})
SELECT (SELECT {'a': 3})['a']
SELECT (SELECT CASE WHEN 1=0 THEN {'a': 3} ELSE NULL END)
CREATE TABLE struct_data (g INTEGER, e INTEGER)
INSERT INTO struct_data VALUES (1, 1), (1, 2), (2, 3), (2, 4), (2, 5), (3, 6), (5, NULL)
SELECT STRUCT_PACK(a := 42, b := 43)
SELECT e, STRUCT_PACK(e) FROM struct_data ORDER BY e LIMIT 2
SELECT STRUCT_PACK(a := 42, b := 43) as struct
select null::row(a integer)
select STRUCT_PACK(a := NULL, b := NULL) as struct
SELECT e, STRUCT_EXTRACT(STRUCT_PACK(xx := e, yy := g), 'xx') as ee FROM struct_data
SELECT e, (STRUCT_PACK(xx := e, yy := g)).xx as ee FROM struct_data
SELECT e, (a).xx as ee FROM (SELECT e, STRUCT_PACK(xx := e, yy := g) FROM struct_data) tbl(e, a)
SELECT e, STRUCT_EXTRACT(STRUCT_PACK(xx := e, yy := g), 'xx') as s FROM struct_data WHERE e > 4
SELECT e, STRUCT_EXTRACT(STRUCT_PACK(xx := e, yy := g), 'xx') as s FROM struct_data WHERE e IS NULL
select struct_keys({a: 1, b: 2, c: 3, d: 4, e: 5})
select struct_keys(NULL::STRUCT(a INT, b VARCHAR))
select struct_keys(NULL)
CREATE TABLE t_struct_constant(x INTEGER)
INSERT INTO t_struct_constant VALUES (1), (2), (3)
SELECT struct_keys({a: 1, b: 2}) FROM t_struct_constant
CREATE TABLE t_struct_flat(col STRUCT(a INT, b VARCHAR), idx INTEGER)
INSERT INTO t_struct_flat VALUES (ROW(1, 'x')::STRUCT(a INT, b VARCHAR), 0), (ROW(2, 'y')::STRUCT(a INT, b VARCHAR), 1), (ROW(3, 'z')::STRUCT(a INT, b VARCHAR), 2), (NULL::STRUCT(a INT, b VARCHAR), 3), (ROW(4, 'q')::STRUCT(a INT, b VARCHAR), 4)
SELECT struct_keys(col) FROM t_struct_flat
CREATE TABLE filtered_struct ( col STRUCT(a INT, b VARCHAR), idx INTEGER )
INSERT INTO filtered_struct VALUES (ROW(1, 'x')::STRUCT(a INT, b VARCHAR), 0), (ROW(2, 'y')::STRUCT(a INT, b VARCHAR), 1), (NULL::STRUCT(a INT, b VARCHAR), 3), (ROW(4, 'q')::STRUCT(a INT, b VARCHAR), 2)
SELECT struct_keys(col) FROM filtered_struct WHERE idx % 2 != 0 ORDER BY idx
select struct_values({a: 1, b: 'x', c: 3})
select struct_values(NULL::STRUCT(a INT, b VARCHAR))
select struct_values(NULL)
SELECT struct_values({a: 1, b: 'y'}) FROM range(3)
CREATE TABLE t_struct_values_flat(col STRUCT(a INT, b VARCHAR), idx INTEGER)
INSERT INTO t_struct_values_flat VALUES ((1, 'x'), 0), ((2, 'y'), 1), ((3, 'z'), 2), (NULL, 3), ((4, 'q'), 4)
SELECT struct_values(col) FROM t_struct_values_flat
CREATE TABLE filtered_struct_values ( col STRUCT(a INT, b VARCHAR), idx INTEGER )
INSERT INTO filtered_struct_values VALUES ((1, 'x'), 0), ((2, 'y'), 1), (NULL, 3), ((4, 'q'), 2)
SELECT struct_values(col) FROM filtered_struct_values WHERE idx % 2 != 0 ORDER BY idx
select struct_values((10, 'hello'))
select struct_values({a: NULL, b: 'not null'})
CREATE TABLE tbl (a INTEGER[], b TEXT[])
INSERT INTO tbl VALUES (ARRAY[7, 5, 7], ARRAY['a', 'b', 'c'])
CREATE TABLE t AS SELECT MAP(list_value(1, 2, 3), list_value(10, 9, 10)) AS m
CREATE TABLE null_keys_list (k INT[], v INT[])
INSERT INTO null_keys_list VALUES ([1], [2]), (NULL, [4])
SELECT MAP(k, v) FROM null_keys_list
CREATE TABLE null_values_list (k INT[], v INT[])
INSERT INTO null_values_list VALUES ([1], [2]), ([4], NULL)
SELECT MAP(k, v) FROM null_values_list
CREATE TABLE a(b MAP(INTEGER,INTEGER))
SELECT MAP(list_value(1, 2, 3), list_value(10, 9, 8))
SELECT MAP(list_value({'i':1,'j':2}, {'i':3,'j':4}), list_value({'i':1,'j':2}, {'i':3,'j':4}))
SELECT MAP(list_value(1, 2, 3), list_value(6, NULL, 4))
SELECT MAP(list_value(1, 2, 3, 4), list_value(10, 9, 8, 7))
SELECT MAP(list_value(), list_value())
SELECT MAP()
INSERT INTO tbl VALUES (ARRAY[5, 7], ARRAY['test', 'string']), (ARRAY[6, 3], ARRAY['foo', 'bar'])
INSERT INTO tbl VALUES (ARRAY[5, 7], ARRAY['also_test', 'also_string'])
SELECT MAP(list_value([1], [2], [3], [4]), list_value(10, 9, 8, 7))
CREATE TABLE i AS SELECT str_split('my yay', ' ') AS l, range AS i FROM range(3)
SELECT list_transform(l, lambda x: {'map1': MAP {x::VARCHAR:1::VARCHAR, 'b'::VARCHAR: x::VARCHAR}}) FROM i
SELECT list_transform(l, x -> {'map1': MAP {x::VARCHAR:1::VARCHAR, 'b'::VARCHAR: x::VARCHAR}}) FROM i
select cardinality(NULL)
select CARDINALITY(MAP(LIST_VALUE(1, 2, 3, 4),LIST_VALUE(10, 9, 8, 7)))
select CARDINALITY(MAP(LIST_VALUE(),LIST_VALUE()))
select CARDINALITY(MAP())
create table ints (a integer, b integer)
insert into ints values (1,1),(5,2),(6,3),(2,2),(7,3),(3,3),(4,4)
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints group by a) as lst_tbl) as T ORDER BY ALL
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints where b < 3 group by a) as lst_tbl) as T ORDER BY ALL
select cardinality(m) from (select MAP(list_value(1), list_value(2)) from range(5) tbl(i)) tbl(m)
select grp, m, case when grp>1 then cardinality(m) else 0 end from (select grp, MAP(lsta,lstb) as m from (SELECT a%4 as grp, list(a) as lsta, list(a) as lstb FROM range(7) tbl(a) group by grp) as lst_tbl) as T
select map_concat(map([3,4,5], ['a', 'b', 'c']), map([1], ['d']))
select map_concat(map([3,4,5], ['a', 'b', 'c']), map([], []))
select map_concat(map([], []), map([], []))
select map_concat(map([], []), map([3,4,5], ['a', 'b', 'c']))
select map_concat(map([], []), NULL)
select map_concat(NULL, NULL)
select map_concat(map([1], NULL), NULL)
select map_concat(map([1], [1]), NULL)
select map_concat( map([3,4,5], ['a', 'b', 'c']), map([3,4,5], ['a', 'b', 'c']), map([3,4,5], ['a', 'b', 'd']) )
create table tbl( x map(BIGINT, VARCHAR), y map(BIGINT, VARCHAR), z map(BIGINT, VARCHAR), )
insert into tbl values ( map([3,4,2], ['abc', 'over_twelve_characters', 'input']), map([3,1,5,2], ['cba', 'a', 'b', 'c']), map([3,7,6], ['1', NULL, '123']) ), ( map([42, 1, 0], ['tiny', 'small', 'bigger']), map([0, 42, 1], ['tiny', 'small', 'bigger']), map([1], ['this is a long string']) ), ( map([5, 1337, 0], ['long', 'longer', 'longest']), map([], []), NULL ), ( NULL, NULL, NULL )
select map_concat(x, y, z) from tbl
SELECT MAP {}
SELECT MAP { 'hello': 'world', 'HELLO': 'WORLD' }
SELECT MAP { 1: 'a', 2: 'b' }
SELECT MAP { i: 'a' || i } FROM range(5) t(i)
SELECT MAP { [i]: [i + 1] } FROM range(5) t(i)
SELECT map_contains_entry(map([1,2,3],[4,5,6]), 2, 5) AS res
SELECT map_contains_value(map([1,2,3],[4,5,6]), 10) AS res
SELECT map_contains(map([1,2,3],[4,5,6]), 1) AS res
SELECT map_contains(map([1,2,3],[4,5,6]), 6) AS res
SELECT map_contains_value(map([1,2,3],[4,5,6]), 4) AS res
SELECT map_contains_value(map([1,2,3],[4,5,6]), 2) AS res
SELECT map_contains_entry(map([],[]), 1, 2) AS res
SELECT map_contains(map([],[]), 1) AS res
SELECT map_contains_value(map([],[]), 1) AS res
CREATE TABLE test_maps(m MAP(INT, INT), k INT, v INT, res_val BOOLEAN, res_key BOOLEAN)
SELECT bool_and(map_contains(m, k) = res_key) = bool_and(map_contains_value(m, v) = res_val) FROM test_maps
SELECT 'my_key' IN map(['my_key'], ['my_value']) AS res
select map_entries(MAP())
SELECT map_entries(map_from_entries([('a', 5)]))
SELECT map_entries(map_from_entries([ ('a', 5), ('b', 6), ('x', 21), ('abc', 0) ]))
SELECT map_entries(map([5], [NULL]))
SELECT map_entries(map_from_entries( [ ('a', 5), ('b', 6), ('x', 21), ('abc', 0) ] ))
select MAP_ENTRIES(MAP([],[]))
select MAP_ENTRIES(MAP(NULL, NULL))
select MAP_ENTRIES(NULL)
select MAP_ENTRIES(NULL::MAP("NULL", "NULL"))
select MAP_ENTRIES(NULL::MAP(INT, BIGINT))
select map_keys(MAP([],[]))
select map_keys(MAP(['a'],[5]))
select map_keys(MAP(['a', 'b', 'c', 'd'], [5,1,8,3]))
select map_keys(NULL)
CREATE TABLE t1 (list STRUCT(a INT, b VARCHAR)[])
INSERT INTO t1 VALUES (ARRAY[(1, 'x'), (2, 'y'), (4, 's')])
SELECT map_keys(MAP_FROM_ENTRIES(list)) FROM t1
INSERT INTO t1 VALUES (ARRAY[(2, 'a'), (3,'b')])
INSERT INTO t1 VALUES (ARRAY[(6, 'h'), (7,'g')])
INSERT INTO t1 VALUES (NULL)
create table tbl ( maps MAP(integer, text)[] )
insert into tbl VALUES ( [ MAP([5,3,2],['a','c','b']), MAP([1], [NULL]), MAP([7,9,1,3,5,6], ['ab','c','d','ef','ba','he']) ] )
SELECT MAP( [ [1],[2],[3] ], [ 4,2,0 ] )
CREATE TABLE tbl(a INT[][], b VARCHAR[])
INSERT INTO tbl VALUES([[2],[3],[4]], ['a', 'b', 'c'])
INSERT INTO tbl VALUES([[5],[6],[7]], ['d', 'e', 'f'])
INSERT INTO tbl VALUES([[8],[9],[10]], ['g', 'h', 'i'])
SELECT MAP(a, b) from tbl
SELECT MAP( [ {'foo': True}, {'foo': False}, {'foo': NULL} ], [ 4,2,0 ] )
SELECT MAP( [ MAP([5],[4]), MAP([10],[2]), MAP([2,3],[3,2]), MAP([10],[3]), MAP([3,2], [2,3]) ], [ 0,1,2,3,4 ] )
select m[1] from (select MAP(LIST_VALUE(1, 2, 3, 4),LIST_VALUE(10, 9, 8, 7)) as m) as T
select m[0] from (select MAP(LIST_VALUE(1, 2, 3, 4,5),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE(1, 2, 3, 4,5),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[2] from (select MAP(LIST_VALUE(),LIST_VALUE()) as m) as T
select m[2] from (select MAP() as m) as T
select m[2::TINYINT] from (select MAP(LIST_VALUE(1, 2, 3, 4,5),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m['Spice Girls'] from (select MAP(LIST_VALUE('Jon Lajoie', 'Backstreet Boys', 'Tenacious D' ),LIST_VALUE(10,9,10)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE('Jon Lajoie', 'Backstreet Boys', 'Tenacious D' ),LIST_VALUE(10,9,10)) as m) as T
select m['Tenacious D'] from (select MAP(LIST_VALUE('Jon Lajoie', 'Backstreet Boys', 'Tenacious D'),LIST_VALUE(10,9,1)) as m) as T
select map_extract(m,1) from (select MAP(LIST_VALUE(1, 2, 3, 4),LIST_VALUE(10, 9, 8, 7)) as m) as T
select map_extract(m,3) from (select MAP(LIST_VALUE(1, 2, 3, 4),LIST_VALUE(10, 9, 8, 7)) as m) as T
select m[3] from (select MAP(LIST_VALUE(1, 2, 3, 4),LIST_VALUE(10, 9, 8, 7)) as m) as T
select m[[2,0]] from (select MAP(LIST_VALUE([0], [1], [2,0], [3]),LIST_VALUE(10, 9, 8, 7)) as m) as T
select m[[2,3]] from (select MAP(LIST_VALUE([0], [1], [2,0], [3], [5]),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE({a:3}, {a:4}, {a:5}, {a:6},{a:7}),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[[2::TINYINT,3::BIGINT]] from (select MAP(LIST_VALUE([1], [2,3], [3], [2],[3,2]),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[[10,11]] from (select MAP(lst,lst) as m from (SELECT LIST([i,i+1]) as lst FROM range(10000) tbl(i)) as lst_tbl) as T
select m[['Tenacious D', 'test']] from (select MAP(LIST_VALUE(['Jon Lajoie'], ['test', NULL], ['Tenacious D', 'test'], ['test', 'Tenacious D']),LIST_VALUE(5,10,9,11)) as m) as T
select m[['Jon Lajoie']] from (select MAP(LIST_VALUE(['Jon Lajoie'], ['Tenacious D', 'a', 'b', 'c']),LIST_VALUE(10,1)) as m) as T
create table ints (a integer[], b integer[])
insert into ints values ([1],[1]), ([2],[2]),([3],[3]),([4],[4])
select m from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints where a[1] < 4 and b[1] > 1) as lst_tbl) as T
select m[[2]] from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints where a[1] < 4 and b[1] > 1) as lst_tbl) as T
insert into ints values ([5],[1]), ([1],[2]),([2],[3]),([6],[4])
create table t2 (id int, v_map map(integer, decimal), k integer[])
insert into t2 select id, map(k,v), k from t1
select v_map[array_sort(k, 'DESC', 'NULLS LAST')[1]] from t2 limit 10
SELECT true as equal FROM test_vector_types(NULL::INT[]) t(c) WHERE c IS NOT NULL
SELECT map([c], [c])[c] IS NOT DISTINCT FROM c as equal FROM test_vector_types(NULL::INT[]) t(c) WHERE c IS NOT NULL
SELECT filtered, last_element, pos, true from ( SELECT list_distinct(c) as filtered, filtered[-1] as last_element, CASE WHEN last_element IS NULL THEN 0 ELSE list_position(filtered, last_element) END as pos, CASE WHEN last_element IS NULL THEN [] ELSE [list_position(filtered, last_element)] END as expected_result, FROM test_vector_types(NULL::INT[]) t(c) WHERE c IS NOT NULL )
select id, v_map[array_sort(k, 'DESC', 'NULLS LAST')[1]] from t2 where id > 3 limit 10
CREATE table tbl (key INT, val VARCHAR)
INSERT INTO tbl VALUES (1,'duck'), (2,'DB'), (3,'duckDB')
SELECT MAP([key], [val])[key] FROM tbl WHERE key <> '2'
select map_values(MAP([],[]))
select map_values(MAP(['a'],[5]))
select map_values(MAP(['a', 'b', 'c', 'd'], [5,1,8,3]))
select map_values(NULL)
select map_values(MAP(['a', 'b', 'c', 'd', 'e'], [NULL, 0, 1, NULL, 3]))
SELECT map_values(MAP_FROM_ENTRIES(list)) FROM t1
select list_apply(maps, lambda x: map_values(x)) from tbl
CREATE MACRO map_values_macro(x) AS (map_values(x))
select map_values_macro(map_from_entries(list)) from t1
select maps, list_apply(maps, lambda x: list_sort(map_values(x))) from tbl
select maps, list_apply(maps, lambda x: map(list_sort(map_keys(x)), list_sort(map_values(x)))) from tbl
create table filtered ( col map(integer, integer), idx integer )
create macro input() as table select * from test_vector_types(NULL::INTEGER[]) t(i) where [x for x in i if x IS NOT NULL] != [] offset 3
select true, true from input()
select map_keys(m) = input, map_values(m) = input from ( select map(input, input) m, input from input() t(input) ) m
create table tbl ( not_filtered bool, keys INTEGER[], vals VARCHAR[] )
insert into tbl select case when i >= 500 then true else false end as not_filtered, [x for x in range(length)] keys, ['a' || i + x for x in range(length)] vals from ( select 1 + (random() * 5)::BIGINT as length, i from range(1000) t(i) )
select vals[1] as val, keys[1] as key, map(keys, vals)[key] as first_map_entry, from tbl where not_filtered and first_map_entry != val
create table data as from ( values ([1], [3]), ([2], [9]), ([3], [15]), ([4], [21]), ) as t(l, r)
select l[1], r[1], map(l, r) from data where r[1] != 3
select l[1], r[1], map(l, r) from data where r[1] != 9
select l[1], r[1], map(l, r) from data where r[1] != 15
select * from test_vector_types(NULL::MAP(varchar, int)) limit 1
SELECT TYPEOF(MAP_KEYS(NULL::MAP(TEXT, BIGINT)))
SELECT TYPEOF(MAP_KEYS(NULL))
SELECT TYPEOF(MAP_VALUES(NULL::MAP(TEXT, BIGINT)))
SELECT TYPEOF(MAP_VALUES(NULL))
SELECT TYPEOF(MAP_ENTRIES(NULL::MAP(TEXT, BIGINT)))
SELECT TYPEOF(MAP_ENTRIES(NULL))
SELECT TYPEOF(MAP_EXTRACT(NULL::MAP(TEXT, BIGINT), 'a'))
SELECT TYPEOF((NULL::MAP(TEXT, BIGINT))['a'])
SELECT TYPEOF(MAP_EXTRACT(NULL, 'a'))
SELECT TYPEOF(MAP_EXTRACT_VALUE(NULL, 'a'))
SELECT MAP_FROM_ENTRIES(list) FROM t1
INSERT INTO t1 VALUES (ARRAY[(10, NULL), (7,'g')])
INSERT INTO t1 VALUES (ARRAY[NULL, NULL])
create table string_key as select MAP_FROM_ENTRIES(ARRAY[('a', 'x'), ('b', 'y')]) col
select * from string_key
select col['a'] from string_key
create table tinyint_key as select MAP_FROM_ENTRIES(ARRAY[(123::TINYINT, 'x'), (-123::TINYINT, 'y')]) col
select * from tinyint_key
select col[123] from tinyint_key
create table smallint_key as select MAP_FROM_ENTRIES(ARRAY[(123::SMALLINT, 'x'), (-123::SMALLINT, 'y')]) col
select * from smallint_key
select col[123] from smallint_key
create table integer_key as select MAP_FROM_ENTRIES(ARRAY[(123::INTEGER, 'x'), (-123::INTEGER, 'y')]) col
select * from integer_key
select col[123] from integer_key
create table t1 as select id, [{'key': 0, 'value': id}] as entry from range(1000) t(id)
create table t2 as select 0 id from range(5)
select t1.id, map_from_entries(entry) from t1 join t2 using (id)
SELECT MAP_FROM_ENTRIES(ARRAY[([1,2], 2), ([3,4], 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[({'a':5, 'b':7}, 2), ({'a':3, 'b':8}, 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[(MAP([5,3,4], ['a', 'b', 'c']), 2), (MAP([4,3,5], ['a', 'b', 'c']), 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[([{'a':5, 'b':7}, {'a':5, 'b':7}], 2), ([{'a':5, 'b':7}, {'a':5, 'b':8}], 4)])
SELECT MAP_FROM_ENTRIES(NULL)
SELECT input FROM tbl
SELECT MAP_FROM_ENTRIES(input) FROM tbl
INSERT INTO tbl VALUES ([(5,3), (6,4), (7,3)])
INSERT INTO tbl VALUES (NULL)
SELECT 1=ALL([1, 2, 3])
SELECT 1=ALL([1, 2, 3, NULL])
SELECT 1=ANY([1, 2, 3])
SELECT 4=ANY([1, 2, 3])
SELECT 4=ANY([1, 2, 3, NULL])
SELECT 4>ALL([1, 2, 3])
SELECT 4>ALL([1, 2, 3, NULL])
SELECT 1=ANY(NULL)
CREATE VIEW v1 AS SELECT LIST(i) l FROM RANGE(5) tbl(i)
SELECT 1=ANY(l) FROM v1
SELECT 6=ANY(l) FROM v1
SELECT NULL=ANY(l) FROM v1
SELECT ARRAY[1,2], ARRAY[NULL], ARRAY['hello', 'world'], ARRAY[]
SELECT ARRAY[ARRAY[1,2]], ARRAY[ARRAY[ARRAY[1, 2], ARRAY[2, 3]], ARRAY[ARRAY[5], ARRAY[3, 4]]]
SELECT ARRAY[1,2,3,4,5,6,7,8,9,10,NULL]
SELECT ARRAY[1, i] FROM range(3) tbl(i) ORDER BY i
SELECT ARRAY[i] FROM range(3) tbl(i) WHERE (ARRAY[i])[1] == 1
SELECT ARRAY[1]::BIGINT[]
SELECT ARRAY[1]::BIGINT ARRAY
SELECT ARRAY[[1, 2], [3, 4]]::BIGINT[][]
SELECT ARRAY[[1, 2], [3, 4]]::VARCHAR[]
SELECT UNNEST(ARRAY[[1, 2], [3, 4]]::VARCHAR[])
SELECT ARRAY[[1, 2], [3, 4]]::VARCHAR
SELECT ARRAY_AGG(NULL), ARRAY_AGG(42)
SELECT ARRAY_AGG(i) FROM range(0, 3) tbl(i)
SELECT ARRAY_AGG(i) FROM range(0, 0) tbl(i)
CREATE TABLE films(film_id INTEGER, title VARCHAR)
CREATE TABLE actors(actor_id INTEGER, first_name VARCHAR, last_name VARCHAR)
CREATE TABLE film_actor(film_id INTEGER, actor_id INTEGER)
INSERT INTO films VALUES (1, 'The Martian'), (2, 'Saving Private Ryan'), (3, 'Team America')
INSERT INTO actors VALUES (1, 'Matt', 'Damon'), (2, 'Jessica', 'Chastain'), (3, 'Tom', 'Hanks'), (4, 'Edward', 'Burns'), (5, 'Kim', 'Jong Un'), (6, 'Alec', 'Baldwin')
INSERT INTO film_actor VALUES (1, 1), (2, 1), (3, 1), (1, 2), (2, 3), (2, 4), (3, 5), (3, 6)
SELECT title, ARRAY_AGG ( CASE WHEN first_name='Matt' and title='Team America' THEN 'MATT DAAAMON' ELSE first_name || ' ' || last_name END order by actor_id) actors FROM films JOIN film_actor USING (film_id) JOIN actors USING (actor_id) GROUP BY title ORDER BY title
select film_id, ARRAY_AGG(actor_id order by actor_id) FROM film_actor GROUP BY film_id ORDER BY ALL
CREATE OR REPLACE TABLE t1 (i INT, l INT[])
INSERT INTO t1 VALUES (1, []), (2, []), (3, [1,2]), (4, []), (5, [1,2])
SELECT i, l, row_number() OVER (PARTITION BY l ORDER BY i) as rid FROM t1 ORDER BY l, i
INSERT INTO t1 VALUES (6, NULL)
SELECT i, l, row_number() OVER (PARTITION BY l ORDER BY i) as rid FROM t1 ORDER BY l NULLS FIRST, i
SELECT list_aggr(list(i), 'quantile', 0.5) FROM range(1, 11) tbl(i)
SELECT list_aggr(list(i), 'quantile', [0.25, 0.5, 0.75]) FROM range(1, 11) tbl(i)
SELECT list_aggr(list(i)::varchar[], 'string_agg', '|') FROM range(1, 4) tbl(i)
pragma force_compression='dictionary'
CREATE TABLE Hosts (ips varchar[])
SELECT min(list_string_agg(ips)) FROM Hosts
SELECT min(ips[1]) FROM Hosts
SELECT min([x[2:4] for x in ips if x[1]::int > 1]) FROM Hosts
select min(i::varchar), max(i::varchar) from range(10) tbl(i)
select min(list_value(i)), max(list_value(i)) from range(10) tbl(i)
select min(list_value(-i)), max(list_value(i+2)) from range(10) tbl(i)
select min(i), max(i) from list_int
select first([i]) from range(10) tbl(i)
select first([0]) from range(10) tbl(i)
select first(i) from range(10) tbl(i) WHERE i=-1
select first(NULL::INT[]) from range(10) tbl(i) WHERE i=-1
select i%3 a, first([i]) from range(10) tbl(i) group by a order by a
select i%3 a, unnest(first([i])) from range(10) tbl(i) group by a order by a
select string_agg(list_value(i)::varchar, ',') from range(10) tbl(i)
select i, i % 2, min(list_value(i)) over(partition by i % 2 order by i) from range(10) tbl(i) ORDER BY 1
SELECT (SELECT [1, 2])
SELECT UNNEST((SELECT [1, 2]))
SELECT (SELECT [[1, 2], [3, 4]])
SELECT (SELECT {'a': [1, 2, 3], 'b': 7})
SELECT (SELECT LIST_VALUE())
SELECT (SELECT CASE WHEN 1=0 THEN LIST_VALUE() ELSE NULL END)
SELECT LIST_EXTRACT(NULL, 1)
SELECT LIST_EXTRACT(LIST_VALUE(), 1)
SELECT LIST_EXTRACT(LIST_VALUE(NULL), 1)
SELECT LIST_EXTRACT(LIST_VALUE(NULL), -1)
SELECT LIST_EXTRACT(LIST_VALUE(42), NULL)
SELECT LIST_EXTRACT(LIST_VALUE(42), 1)
SELECT LIST_ELEMENT(LIST_VALUE(42), 1)
SELECT LIST_EXTRACT(LIST_VALUE(42, 43), 2)
SELECT LIST_EXTRACT(LIST_VALUE(42, 43, 44, 45), -1)
SELECT LIST_EXTRACT(LIST_VALUE(42, 43, 44, 45), -2)
SELECT LIST_EXTRACT(LIST_VALUE(42, 43, 44, 45), -4)
SELECT LIST_EXTRACT(LIST_VALUE(42, 43, 44, 45), -5)
CREATE TABLE struct_data(str STRUCT(val VARCHAR)[])
INSERT INTO struct_data VALUES (NULL)
SELECT list_resize(str, 1) FROM struct_data
SELECT list_reduce(str, lambda a, b: a) FROM struct_data
SELECT str[1] FROM struct_data
SELECT list_aggregate(str, 'count') FROM struct_data
CREATE TABLE nested_struct_data(str STRUCT(str_nested STRUCT(val VARCHAR))[])
INSERT INTO nested_struct_data VALUES ([NULL])
SELECT list_transform(str, lambda x: x) FROM nested_struct_data
SELECT list_filter(str, lambda x: x.str_nested IS NULL) FROM nested_struct_data
CREATE TABLE struct_data_two_lists(str STRUCT(val VARCHAR)[][])
INSERT INTO struct_data_two_lists VALUES (NULL)
SELECT a[1] FROM (VALUES (LIST_VALUE())) tbl(a)
SELECT a[1] FROM (VALUES (LIST_VALUE(NULL))) tbl(a)
SELECT a[-1] FROM (VALUES (LIST_VALUE(NULL))) tbl(a)
SELECT a[NULL] FROM (VALUES (LIST_VALUE(42))) tbl(a)
SELECT a[1] FROM (VALUES (LIST_VALUE(42))) tbl(a)
SELECT a[1+1-1] FROM (VALUES (LIST_VALUE(42))) tbl(a)
SELECT a[b] FROM (VALUES (LIST_VALUE(42), 1)) tbl(a, b)
SELECT (LIST_VALUE(42))[1]
SELECT LIST_VALUE(42)[1]
SELECT a[2:] FROM (VALUES (LIST_VALUE(42, 43, 44))) tbl(a)
SELECT a[1:] FROM (VALUES (LIST_VALUE(42, 43, 44))) tbl(a)
SELECT a[:1] FROM (VALUES (LIST_VALUE(42, 43, 44))) tbl(a)
CREATE TABLE listdata ( c0 char(1), c1 char(1), c2 char(1), c3 char(1), c4 char(1), off integer, length integer)
INSERT INTO listdata VALUES ('h', 'e', 'l', 'l', 'o', 1, 2), ('w', 'o', 'r', 'l', 'd', 2, 3), ('b', NULL, NULL, NULL, NULL, 0, 1), (NULL, NULL, NULL, NULL, NULL, 0, 2)
CREATE VIEW lists AS SELECT CASE WHEN c0 = 'b' THEN LIST_VALUE(c0) WHEN c0 IS NULL THEN NULL ELSE LIST_VALUE(c0, c1, c2, c3, c4) END AS s, off, length FROM listdata
SELECT s from lists
CREATE TABLE duckdata(c0 char(1), c1 char(1), c2 char(1))
INSERT INTO duckdata VALUES ('🦆', 'a', 'b'), ('a', 'b', 'c')
CREATE VIEW ducks AS SELECT LIST_VALUE(c0, c1, c2) AS d from duckdata
CREATE VIEW hello AS SELECT s AS hello FROM lists WHERE off = 1 AND length = 2
CREATE VIEW nulltable as SELECT s as n FROM lists WHERE off = 0 AND length = 2
SELECT d from ducks
SELECT d[0:0] FROM ducks
SELECT s[1:2] FROM lists
SELECT list_slice([1,2,3,4,5], 1, 3, -1)
SELECT list_slice([1,2,3,4,5], 1, 3, -2)
SELECT ([1,2,3])[1:-:-1]
SELECT ([1,2,3])[:3:-1]
SELECT ([1,2,3,4,5])[:-:-1]
SELECT ([1,2,3,4,5])[:-:-2]
CREATE TABLE tbl (a INT[], start int, stop int, step int)
INSERT INTO tbl VALUES ([1,2,3,4,5], 1, 3, -2)
INSERT INTO tbl VALUES ([1,4,5,6,7,8], 3, 1, -2)
INSERT INTO tbl VALUES ([1,2,3,4,5], -4, -1, -2)
INSERT INTO tbl VALUES ([1,2,3,4,5], 1, 3, -1)
SELECT a[start:stop:step] FROM tbl
SELECT ([1,2,3,4,5,6])[-9223372036854775808:5]
SELECT ([1,2,3,4,5,6])[-9223372036854775808:5:2]
SELECT ([1,2,3,4,5,6])[1:9223372036854775807]
SELECT ([1,2,3,4,5,6])[1:9223372036854775807:2]
SELECT ([1,2,3,4,5,6])[9223372036854775807:9223372036854775807]
SELECT ([1,2,3,4,5,6])[9223372036854775807:-9223372036854775808]
SELECT ([1,2,3,4,5,6])[9223372036854775807:-9223372036854775808:-1]
SELECT ([1,2,3,4,5,6])[-9223372036854775808 + 1:5]
CREATE TABLE tbl (a INT[], start bigint, stop bigint, step bigint)
INSERT INTO tbl VALUES ([1,2,3,4,5], -9223372036854775808, 9223372036854775807, -1)
INSERT INTO tbl VALUES ([1,2,3,4,5], -9223372036854775808 + 1, 9223372036854775807 - 1, -1)
SELECT a[-9223372036854775808:9223372036854775807:step] FROM tbl
SELECT ([1,2,3,4,5,6])[-10:-10]
SELECT ([])[1:3:2]
SELECT ([1,2,3,4,5,6])[5:3:2]
SELECT ([1,2,3,4,5,6])[5:3]
SELECT ([1,2,3,4,5])[1:-:2]
SELECT 'abcdefg'[1:3]
SELECT 'abcdefg'[:3]
SELECT list_slice([1,2,3,4,5], 1, 3, 1)
SELECT ([])[1:3]
SELECT ([1,2,3,4,5])[-1:3]
SELECT ([1,2,3,4,5])[1:-3]
SELECT ([1,2,3,4,5])[6:8]
SELECT [{'i': 1,'j': [2, 3]}, NULL, {'i': 1, 'j': [2, 3]}]
CREATE TABLE list_data (g INTEGER, e INTEGER)
INSERT INTO list_data VALUES (1, 1), (1, 2), (2, 3), (2, 4), (2, 5), (3, 6), (5, NULL)
SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)
SELECT UNNEST(l1) FROM (SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)) t1
SELECT * FROM (SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)) t1, (SELECT LIST(b) l2 FROM (VALUES (4), (5), (6), (7)) AS t2 (b)) t2
SELECT UNNEST(l1) u1, UNNEST(l2) u2 FROM (SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)) t1, (SELECT LIST(b) l2 FROM (VALUES (4), (5), (6), (7)) AS t2 (b)) t2
SELECT UNNEST(l1), l2 FROM (SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)) t1, (SELECT LIST(b) l2 FROM (VALUES (4), (5), (6), (7)) AS t2 (b)) t2
SELECT l1, UNNEST(l2) FROM (SELECT LIST(a) l1 FROM (VALUES (1), (2), (3)) AS t1 (a)) t1, (SELECT LIST(b) l2 FROM (VALUES (4), (5), (6), (7)) AS t2 (b)) t2
SELECT UNNEST(LIST(e)) ue, LIST(g) from list_data ORDER BY 1 NULLS LAST
SELECT g, LIST(e) from list_data GROUP BY g ORDER BY g
SELECT g, LIST(e) l1, LIST(e) l2 from list_data GROUP BY g ORDER BY g
SELECT LIST_VALUE('hello')
SELECT LIST_VALUE('hello')::VARCHAR
SELECT l::VARChAR FROM (VALUES (LIST_VALUE('hello', 'world')), (LIST_VALUE('a', 'b', 'c'))) t(l)
SELECT LIST_VALUE(1, 2, 3, '4') a, LIST_VALUE('a','b','c') b, LIST_VALUE(42, NULL) c, LIST_VALUE(NULL, NULL, NULL) d, LIST_VALUE() e
SELECT a FROM (VALUES (LIST_VALUE(1, 2, 3, 4)), (LIST_VALUE()), (LIST_VALUE(NULL::INTEGER)), (LIST_VALUE(42))) lv(a)
SELECT a FROM (VALUES (LIST_VALUE('hello', 'world')), (LIST_VALUE()), (LIST_VALUE(NULL::VARCHAR)), (LIST_VALUE('42'))) lv(a)
SELECT * FROM (VALUES ((LIST_VALUE()), (LIST_VALUE(NULL)), LIST_VALUE(1, 2))) lv(a)
SELECT * FROM (VALUES (LIST_VALUE(1, 2)), (LIST_VALUE()), (LIST_VALUE(NULL::INTEGER))) lv(a)
SELECT LIST_VALUE(1, 2, 3) UNION ALL SELECT LIST_VALUE(NULL::INTEGER) UNION ALL SELECT LIST_VALUE() UNION ALL SELECT NULL
SELECT NULL UNION ALL SELECT LIST_VALUE() UNION ALL SELECT LIST_VALUE(NULL::INTEGER) UNION ALL SELECT LIST_VALUE(1, 2, 3)
SELECT UNNEST(a) ua FROM (VALUES (LIST_VALUE(1, 2, 3, 4)), (LIST_VALUE()), (LIST_VALUE(NULL::INTEGER)), (LIST_VALUE(42))) lv(a)
SELECT UNNEST(a) ua FROM (VALUES (LIST_VALUE()), (LIST_VALUE(1, 2, 3, 4)), (LIST_VALUE(NULL)), (LIST_VALUE(42))) lv(a)
SELECT DATE '1992-01-01'::TIMESTAMP_MS
SELECT DATE '1992-01-01'::TIMESTAMP_S
SELECT DATE '1992-01-01'::TIMESTAMP_NS
select '2023-12-08 08:51:39.123456'::TIMESTAMP_MS::TIME
select '2023-12-08 08:51:39.123456'::TIMESTAMP_S::TIME
select '2023-12-08 08:51:39.123456'::TIMESTAMP_NS::TIME
select '2024-05-10 11:06:33.446'::TIMESTAMP_S
select '2024-05-10 11:06:33.846'::TIMESTAMP_S
select '2024-05-10 11:06:33.123446'::TIMESTAMP_MS
select '2024-05-10 11:06:33.123846'::TIMESTAMP_MS
CREATE TABLE issue11995 (t TIMESTAMP)
INSERT INTO issue11995 VALUES ('2024-05-10 11:06:33.446'), ('2024-05-10 11:06:33.846'), ('2024-05-10 11:06:33.123446'), ('2024-05-10 11:06:33.523846')
SELECT '1969-01-01 01:03:20.45432'::TIMESTAMP::VARCHAR
SELECT '-1000-01-01 01:03:20.45432'::TIMESTAMP::VARCHAR
SELECT '1000-01-01 (BC) 01:03:20.45432'::TIMESTAMP::VARCHAR
CREATE TABLE timestamp(t TIMESTAMP)
INSERT INTO timestamp VALUES ('1992-02-29 00:00:00')
INSERT INTO timestamp VALUES ('2000-02-29 00:00:00')
CREATE TABLE specials (ts TIMESTAMP, tstz TIMESTAMPTZ, dt DATE)
INSERT INTO specials VALUES ('infinity'::TIMESTAMP, 'infinity'::TIMESTAMPTZ, 'infinity'::DATE), ('-infinity'::TIMESTAMP, '-infinity'::TIMESTAMPTZ, '-infinity'::DATE), ('epoch'::TIMESTAMP, 'epoch'::TIMESTAMPTZ, 'epoch'::DATE),
SELECT * FROM specials
CREATE TABLE abbreviations (ts TIMESTAMP, tstz TIMESTAMPTZ, dt DATE)
INSERT INTO abbreviations VALUES ('inf'::TIMESTAMP, 'inf'::TIMESTAMPTZ, 'inf'::DATE), ('-inf'::TIMESTAMP, '-inf'::TIMESTAMPTZ, '-inf'::DATE),
SELECT * FROM abbreviations
SELECT lhs.ts, rhs.ts, lhs.ts < rhs.ts, lhs.ts <= rhs.ts, lhs.ts = rhs.ts, lhs.ts <> rhs.ts, lhs.ts >= rhs.ts, lhs.ts > rhs.ts, FROM specials lhs, specials rhs ORDER BY 1, 2
SELECT lhs.tstz, rhs.tstz, lhs.tstz < rhs.tstz, lhs.tstz <= rhs.tstz, lhs.tstz = rhs.tstz, lhs.tstz <> rhs.tstz, lhs.tstz >= rhs.tstz, lhs.tstz > rhs.tstz, FROM specials lhs, specials rhs ORDER BY 1, 2
SELECT lhs.dt, rhs.dt, lhs.dt < rhs.dt, lhs.dt <= rhs.dt, lhs.dt = rhs.dt, lhs.dt <> rhs.dt, lhs.dt >= rhs.dt, lhs.dt > rhs.dt, FROM specials lhs, specials rhs ORDER BY 1, 2
SELECT MIN(ts), MAX(ts), MIN(tstz), MAX(tstz), MIN(dt), MAX(dt) FROM specials
SELECT MEDIAN(ts), MEDIAN(tstz), MEDIAN(dt) FROM specials
SELECT MODE(ts), MODE(tstz), MODE(dt) FROM specials
SELECT timestamp '2017-07-23 13:10:11'
SELECT timestamp '2017-07-23T13:10:11', timestamp '2017-07-23T13:10:11Z'
SELECT timestamp ' 2017-07-23 13:10:11 '
SELECT t FROM timestamp ORDER BY t
SELECT MIN(t) FROM timestamp
SELECT MAX(t) FROM timestamp
SELECT AVG(t) FROM timestamp
SELECT t-t FROM timestamp
SELECT YEAR(TIMESTAMP '1992-01-01 01:01:01')
SELECT YEAR(TIMESTAMP '1992-01-01 01:01:01'::DATE)
SELECT (TIMESTAMP '1992-01-01 01:01:01')::DATE
SELECT (TIMESTAMP '1992-01-01 01:01:01')::TIME
CREATE TABLE timestamp1(i TIMESTAMP)
CREATE TABLE timestamp2(i TIMESTAMP)
INSERT INTO timestamp1 VALUES ('1993-08-14 00:00:01')
INSERT INTO timestamp2 VALUES ('1993-08-14 00:00:01')
select count(*) from timestamp2 inner join timestamp1 on (timestamp1.i = timestamp2.i)
CREATE TABLE timestamps(ts_SEC TIMESTAMP_S, ts_MS TIMESTAMP_MS, ts TIMESTAMP, ts_NS TIMESTAMP_NS)
INSERT INTO timestamps VALUES ('2000-01-01 01:12:23', '2000-01-01 01:12:23.123', '2000-01-01 01:12:23.123456', '2000-01-01 01:12:23.123457')
SELECT ts_SEC=ts_MS, ts_SEC=ts, ts_SEC=ts_NS, ts_MS=ts, ts_MS=ts_NS, ts=ts_NS, ts_MS=ts_SEC, ts=ts_SEC, ts_SEC=ts_NS, ts=ts_MS, ts_NS=ts_MS, ts_NS=ts, FROM timestamps
SELECT typeof([TIMESTAMP '2000-01-01 01:12:23.123456', TIMESTAMP_NS '2000-01-01 01:12:23.123456'])
SELECT typeof([TIMESTAMP_NS '2000-01-01 01:12:23.123456', TIMESTAMP '2000-01-01 01:12:23.123456'])
SELECT CAST('2001-04-20 14:42:11.123' AS TIMESTAMP) a, CAST('2001-04-20 14:42:11.0' AS TIMESTAMP) b
SELECT TIMESTAMP '2001-04-20 14:42:11.12300000000000000000'
SET TimeZone='UTC'
SET TimeZone='America/Los_Angeles'
SET TimeZone='Etc/GMT-6'
CREATE TABLE IF NOT EXISTS timestamp (sec TIMESTAMP_S, milli TIMESTAMP_MS,micro TIMESTAMP_US, nano TIMESTAMP_NS )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01','2008-01-01 00:00:01.594','2008-01-01 00:00:01.88926','2008-01-01 00:00:01.889268321' )
SELECT * from timestamp
SELECT YEAR(sec),YEAR(milli),YEAR(nano) from timestamp
SELECT nano::TIMESTAMP, milli::TIMESTAMP,sec::TIMESTAMP from timestamp
SELECT micro::TIMESTAMP_S, micro::TIMESTAMP_MS,micro::TIMESTAMP_NS from timestamp
INSERT INTO timestamp VALUES ('2008-01-01 00:00:51','2008-01-01 00:00:01.894','2008-01-01 00:00:01.99926','2008-01-01 00:00:01.999268321' )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:11','2008-01-01 00:00:01.794','2008-01-01 00:00:01.98926','2008-01-01 00:00:01.899268321' )
SELECT s::TIMESTAMP_NS FROM VALUES ('2024-06-04 10:17:10.987654321'), ('2024-06-04 10:17:10.98765432'), ('2024-06-04 10:17:10.9876543'), ('2024-06-04 10:17:10.9876543'), ('2024-06-04 10:17:10.987654'), ('2024-06-04 10:17:10.98765'), ('2024-06-04 10:17:10.9876'), ('2024-06-04 10:17:10.987'), ('2024-06-04 10:17:10.98'), ('2024-06-04 10:17:10.9'), ('2024-06-04 10:17:10') AS tbl(s)
select '1969-01-01T23:59:59.9999999'::timestamp_ns
SELECT '1970-01-01 00:00:00.000000123'::TIMESTAMP_NS
select sec::TIME from timestamp
select timestamptz '2021-11-15 02:30:00'
select '2021-11-15 02:30:00'::TIMESTAMP::TIMESTAMPTZ
SELECT '1880-05-15T12:00:00+00:50:20'::TIMESTAMPTZ
select timestamp '1970-01-01'
select '290309-12-22 (BC) 00:00:00'::timestamp
select '290309-12-22 (BC) 00:00:00'::timestamp + interval (1) day
select timestamp '294247-01-10 04:00:54.775806'
select epoch(timestamp '294247-01-10 04:00:54.775806'), epoch(timestamp '290309-12-22 (BC) 00:00:00')
select year(timestamp '294247-01-10 04:00:54.775806'), year(timestamp '290309-12-22 (BC) 00:00:00')
select decade(timestamp '294247-01-10 04:00:54.775806'), decade(timestamp '290309-12-22 (BC) 00:00:00')
select monthname(timestamp '294247-01-10 04:00:54.775806'), monthname(timestamp '290309-12-22 (BC) 00:00:00')
select age(timestamp '294247-01-10 04:00:54.775806', '290309-12-22 (BC) 00:00:00'::timestamp)
CREATE TABLE ts_precision( sec TIMESTAMP(0), msec TIMESTAMP(3), micros TIMESTAMP(6), nanos TIMESTAMP (9) )
INSERT INTO ts_precision VALUES ('2020-01-01 01:23:45.123456789', '2020-01-01 01:23:45.123456789', '2020-01-01 01:23:45.123456789', '2020-01-01 01:23:45.123456789')
SELECT sec::VARCHAR, msec::VARCHAR, micros::VARCHAR, nanos::VARCHAR FROM ts_precision
SELECT EXTRACT(microseconds FROM sec), EXTRACT(microseconds FROM msec), EXTRACT(microseconds FROM micros), EXTRACT(microseconds FROM nanos) FROM ts_precision
SELECT TIMESTAMP '2021-05-25 04:55:03.382494 UTC'
SELECT TIMESTAMP '2021-05-25 04:55:03.382494 utc'
SELECT TIMESTAMP '2021-05-25 04:55:03.382494 uTc'
set Calendar='gregorian'
SELECT TIMESTAMPTZ '2021-05-25 04:55:03.382494 EST'
set TimeZone='America/Phoenix'
SELECT DATE_DIFF( 'HOUR', TIMESTAMP '2010-07-07 10:20:00' AT TIME ZONE 'Asia/Bangkok', TIMESTAMP '2010-07-07 10:20:00+00') AS hours
select try_cast('' as timestamp)
select try_cast(' ' as timestamp)
select try_cast('1111' as timestamp)
select try_cast(' 1111 ' as timestamp)
select try_cast('1111-' as timestamp)
select try_cast('1111-11' as timestamp)
select try_cast('1111-11-' as timestamp)
select try_cast('1111-111-1' as timestamp)
select try_cast('1111-11-111' as timestamp)
select try_cast('1111-11-11 11' as timestamp)
select try_cast('1111-11-11 11:11' as timestamp)
select try_cast('1111-11-11 11:11:999' as timestamp)
set TimeZone='UTC'
SELECT '2025-01-01T08:00:00+08'::TIMESTAMP AS c
SELECT '2025-01-01T08:00:00+08'::TIMESTAMPTZ AS c
select timestamptz '2020-12-31 21:25:58.745232'
select timestamptz '2020-12-31 21:25:58.745232+00'
select timestamptz '2020-12-31 21:25:58.745232+0000'
select timestamptz '2020-12-31 21:25:58.745232+02'
select timestamptz '2020-12-31 21:25:58.745232-02'
select timestamptz '2020-12-31 21:25:58.745232+0215'
select timestamptz '2020-12-31 21:25:58.745232+02:15'
select timestamptz '2020-12-31 21:25:58.745232-0215'
select timestamptz '2020-12-31 21:25:58+02:15'
select try_cast(try_cast('00112233-4455-6677-8899-aabbccddeeff'::UUID AS BLOB) as uuid) as test
SELECT '00112233-4455-6677-8899-aabbccddeeff'::UUID::BLOB
SELECT '00112233-4455-6677-8899-aabbccddeeff'::UUID::BLOB::UUID
SELECT try_cast(try_cast('{00112233-4455-6677-8899-aabbccddeeff}'::UUID AS BLOB) as uuid) as test
SELECT try_cast(try_cast(NULL::UUID AS BLOB) as uuid) as test
SELECT try_cast(NULL::BLOB as uuid) as test
SELECT try_cast(''::BLOB as uuid) as test
CREATE TABLE blobs (b BYTEA)
SELECT * FROM blobs
DELETE FROM blobs
SELECT ''::BLOB
SELECT NULL::BLOB
CREATE TABLE blob_empty (b BYTEA)
INSERT INTO blob_empty VALUES(''), (''::BLOB)
INSERT INTO blob_empty VALUES(NULL), (NULL::BLOB)
SELECT * FROM blob_empty
SELECT 'a'::BYTEA::VARCHAR
SELECT 'a'::VARCHAR::BYTEA
INSERT INTO blobs VALUES ('a'::BYTEA)
SELECT b || 'ZZ'::BYTEA FROM blobs
SELECT COUNT(*) FROM blobs
SELECT OCTET_LENGTH(b) FROM blobs
SELECT b || '5A5A'::VARCHAR FROM blobs
INSERT INTO blobs VALUES ('FF'::BYTEA)
INSERT INTO blobs VALUES ('55AAFF55AAFF55AAFF01'::BYTEA)
CREATE TABLE b(b blob)
INSERT INTO b VALUES (NULL)
CREATE TABLE blobs (b BYTEA, g INTEGER)
SELECT COUNT(*), COUNT(b), MIN(b), MAX(b) FROM blobs
SELECT * FROM blobs ORDER BY b
SELECT b, SUM(g) FROM blobs GROUP BY b ORDER BY b
CREATE TABLE blobs2 (b BYTEA, g INTEGER)
SELECT L.b, SUM(L.g) FROM blobs as L JOIN blobs2 AS R ON L.b=R.b GROUP BY L.b ORDER BY L.b
SELECT R.b, SUM(R.g) FROM blobs as L, blobs2 AS R WHERE L.b=R.b GROUP BY R.b ORDER BY R.b
INSERT INTO blobs VALUES ('aaaaaaaaaa')
INSERT INTO blobs SELECT b||b||b||b||b||b||b||b||b||b FROM blobs WHERE OCTET_LENGTH(b)=(SELECT MAX(OCTET_LENGTH(b)) FROM blobs)
SELECT OCTET_LENGTH(b) FROM blobs ORDER BY 1
SELECT TRY_CAST('\\' AS BLOB)
SELECT TRY_CAST('\\b12' AS BLOB)
SELECT TRY_CAST('ü' AS BLOB)
SELECT 251658240::HUGEINT * 251658240::HUGEINT
SELECT 251658240::HUGEINT * 1080863910568919040::HUGEINT
SELECT 251658240::HUGEINT * 4642275147320176030871715840::HUGEINT
SELECT 1080863910568919040::HUGEINT * 251658240::HUGEINT
SELECT 1080863910568919040::HUGEINT * 1080863910568919040::HUGEINT
SELECT 4642275147320176030871715840::HUGEINT * 251658240::HUGEINT
SELECT 85070591730234615865843651857942052863::HUGEINT * 2::HUGEINT
SELECT 19807040628566084398385987583::HUGEINT * 8589934592::HUGEINT
SELECT 36893488147419103231::HUGEINT * 4611686018427387904::HUGEINT
SELECT 2::HUGEINT * 85070591730234615865843651857942052863::HUGEINT
SELECT 8589934592::HUGEINT * 19807040628566084398385987583::HUGEINT
SELECT 4611686018427387904::HUGEINT * 36893488147419103231::HUGEINT
SELECT TRY_CAST('170141183460469231731687303715884105728' AS HUGEINT)
SELECT TRY_CAST('170141183460469231731687303715884105728'::DOUBLE AS HUGEINT)
SELECT TRY_CAST('-170141183460469231731687303715884105729' AS HUGEINT)
SELECT TRY_CAST('-170141183460469231731687303715884105729'::DOUBLE AS HUGEINT)
CREATE TABLE hugeints(g INTEGER, h HUGEINT)
INSERT INTO hugeints VALUES (1, 42.0), (2, 1267650600228229401496703205376), (2, -439847238974238975238975), (1, '-12')
SELECT -(100::HUGEINT), -(-(50::HUGEINT))
SELECT -(0::HUGEINT)
SELECT -(100000000000000000000::HUGEINT), -(-(100000000000000000000::HUGEINT))
SELECT 42::HUGEINT + 42::HUGEINT
SELECT 42::HUGEINT + -42::HUGEINT, -42::HUGEINT + 100::HUGEINT, -42::HUGEINT+-42::HUGEINT
SELECT '100000000000000000000'::HUGEINT + '100000000000000000000'::HUGEINT
SELECT '100000000000000000000'::HUGEINT + '-1000000000000000000000'::HUGEINT
SELECT '5'::HUGEINT + '-10000000000000000002'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT - 10::HUGEINT + 10::HUGEINT
SELECT '-170141183460469231731687303715884105728'::HUGEINT + 10::HUGEINT - 10::HUGEINT
SELECT 100::HUGEINT - 42::HUGEINT, 3::HUGEINT - 5::HUGEINT
SELECT -100::HUGEINT - 42::HUGEINT, -3::HUGEINT - 5::HUGEINT, 12::HUGEINT-(-12::HUGEINT)
SELECT 10000000000000000000::HUGEINT + 100::TINYINT, 10000000000000000000::HUGEINT + 100::SMALLINT, 10000000000000000000::HUGEINT + 100::INTEGER, 10000000000000000000::HUGEINT + 100::BIGINT
SELECT 100::HUGEINT + 0.5
SELECT COS(100::HUGEINT)
SELECT CONCAT('hello number ', 100::HUGEINT)
SELECT 1::HUGEINT << 3
SELECT 27::HUGEINT << 0
select 1::HUGEINT << 50, 1::HUGEINT << 100
select (((((1::HUGEINT << 50) << 50) << 2) << 3) << 8), (1::HUGEINT)<<50<<20<<7<<18<<3<<6<<9
select 1::HUGEINT << 64
SELECT 8::HUGEINT >> 3
SELECT 27::HUGEINT >> 0
SELECT -27::HUGEINT >> 1
SELECT 27::HUGEINT >> -1
select (1::HUGEINT << 100) >> 50, (1::HUGEINT << 120)>>108
SELECT '1329227995784915872903807060280344576'::HUGEINT >> 200
select -11367237885269962203896920952509169001 >> 200
SELECT '7'::HUGEINT, '130'::HUGEINT, '924829852'::HUGEINT
SELECT '0'::HUGEINT, '-0'::HUGEINT
SELECT '-7'::HUGEINT, '-130'::HUGEINT, '-924829852'::HUGEINT
SELECT '10000000000000000000000000000'::HUGEINT
SELECT '1267650600228229401496703205376'::HUGEINT, '17014118346046923173168730371588410572'::HUGEINT
SELECT '-1267650600228229401496703205376'::HUGEINT, '-17014118346046923173168730371588410572'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT, '-170141183460469231731687303715884105728'::HUGEINT
SELECT 42::TINYINT::HUGEINT, 42::SMALLINT::HUGEINT, 42::INTEGER::HUGEINT, 42::BIGINT::HUGEINT, 42::FLOAT::HUGEINT, 42::DOUBLE::HUGEINT
SELECT (-42)::TINYINT::HUGEINT, (-42)::SMALLINT::HUGEINT, (-42)::INTEGER::HUGEINT, (-42)::BIGINT::HUGEINT, (-42)::FLOAT::HUGEINT, (-42)::DOUBLE::HUGEINT
SELECT 42::HUGEINT::TINYINT, 42::HUGEINT::SMALLINT, 42::HUGEINT::INTEGER, 42::HUGEINT::BIGINT, 42::HUGEINT::FLOAT, 42::HUGEINT::DOUBLE
SELECT (-42)::HUGEINT::TINYINT, (-42)::HUGEINT::SMALLINT, (-42)::HUGEINT::INTEGER, (-42)::HUGEINT::BIGINT, (-42)::HUGEINT::FLOAT, (-42)::HUGEINT::DOUBLE
SELECT 127::HUGEINT::TINYINT, -127::HUGEINT::TINYINT
select '170141183460469231731687303715884105700e0'::hugeint
select '170141183460469231731687303715884105727e0'::hugeint
select abs(1::HUGEINT), abs('-1329227995784915872903807060280344576'::HUGEINT), abs(0::HUGEINT)
select sign(1::HUGEINT), sign('-1329227995784915872903807060280344576'::HUGEINT), sign(0::HUGEINT)
select round(1::HUGEINT, 0), round('-1329227995784915872903807060280344576'::HUGEINT, 0), round(0::HUGEINT, 0)
select floor(1::HUGEINT), floor('-1329227995784915872903807060280344576'::HUGEINT), floor(0::HUGEINT)
select ceil(1::HUGEINT), ceil('-1329227995784915872903807060280344576'::HUGEINT), ceil(0::HUGEINT)
select LEAST(1::HUGEINT, '-1329227995784915872903807060280344576'::HUGEINT, 0::HUGEINT)
select GREATEST(1::HUGEINT, '-1329227995784915872903807060280344576'::HUGEINT, 0::HUGEINT)
SELECT NULL::HUGEINT
CREATE TABLE hugeints(id INTEGER, h HUGEINT)
CREATE TABLE hugeints(h HUGEINT)
INSERT INTO hugeints VALUES (42::HUGEINT), ('1267650600228229401496703205376'::HUGEINT)
SELECT h::INTEGER FROM hugeints WHERE h < 100
SELECT COUNT(*) FROM hugeints WHERE h = 42::HUGEINT
SELECT COUNT(*) FROM hugeints WHERE h <> '1267650600228229401496703205376'::HUGEINT
SELECT COUNT(*) FROM hugeints WHERE h < '1267650600228229401496703205376'::HUGEINT
SELECT COUNT(*) FROM hugeints WHERE h <= '1267650600228229401496703205376'::HUGEINT
SELECT COUNT(*) FROM hugeints WHERE h > '1267650600228229401496703205375'::HUGEINT
SELECT COUNT(*) FROM hugeints WHERE h >= 42::HUGEINT
SELECT * FROM hugeints JOIN hugeints2 USING (h)
SELECT * FROM hugeints t1 JOIN hugeints2 t2 ON t1.h <> t2.h
SELECT * FROM hugeints t1 JOIN hugeints2 t2 ON t1.h >= t2.h ORDER BY 1 LIMIT 2
