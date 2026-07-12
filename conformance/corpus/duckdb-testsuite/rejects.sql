SELECT test.t.col FROM test.tbl t
SELECT test.tbl.col FROM test.tbl t
SELECT testX.tbl.col FROM test.tbl
SELECT test.tblX.col FROM test.tbl
SELECT test.tbl.colX FROM test.tbl
select (select #1) from range(1)
SELECT #2 FROM range(1)
SELECT #1
SELECT #0 FROM range(1)
SELECT #-1 FROM range(1)
SELECT s2.tbl.i FROM s1.tbl
SELECT a.tbl.i FROM range(10) tbl(i)
SELECT 'j': 42
SELECT a.i FROM b : a
SELECT a : 42 AS b
SELECT * INTO t2 FROM t WHERE t LIKE 'b%'
SELECT * FROM t FOR UPDATE
SELECT * FROM t FOR NO KEY UPDATE
SELECT * FROM t FOR SHARE
SELECT * FROM t KEY SHARE
select * from (values (1)) tbl(a) natural join (values (1), (2)) tbl2(b) order by 1, 2
select (select * from (select 42) tbl(a) natural join (select 42) tbl(a))
SELECT COUNT(t1.rowid) FROM t1, v0 RIGHT JOIN t0 ON t1.c1=t0.c1 AND v0.c0=t0.c0
select * from (values (1)) t1(i) join (values (1)) t2(i) on (t1.i=t2.i) natural join (values (1)) t3(i)
select * from (values (1)) t1(i) natural join ((values (1)) t2(i) join (values (1)) t3(i) on (t2.i=t3.i))
SELECT b FROM test, test2 WHERE test.b > test2.b
SELECT * FROM t1 JOIN t2 USING (c)
SELECT * FROM t1 JOIN t2 USING (a)
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(a+b)
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING("")
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(d)
SELECT t2.a, t2.b, t2.c FROM t1 JOIN t2 USING(t1.a)
SELECT * FROM t1 JOIN t2 USING(a) JOIN t2 t2b USING (b)
select * from (values (1)) tbl(i) join ((values (1)) tbl2(i) join (values (1)) tbl3(i) on tbl2.i=tbl3.i) using (i)
SELECT * FROM left_table ANTI JOIN right_table ON left_table.a = right_table.a WHERE right_table.a < 43
SELECT * FROM left_table SEMI JOIN right_table ON left_table.a = right_table.a WHERE right_table.a < 43
WITH t AS ( SELECT 1 AS r, [{n:1}, {n:2}] AS s UNION SELECT 2 AS r, [{n:3}, {n:4}] AS s ) SELECT r, s1.s.n FROM t LEFT JOIN UNNEST(s) AS s1(s) ON FALSE
SELECT MAX(agg0) FROM (SELECT MAX('a') AS agg0 FROM t0 RIGHT JOIN t1 ON ((t0.c0)<=(((NULL)-(t1.rowid)))) WHERE t1.c0 UNION ALL SELECT MAX('a') AS agg0 FROM t0 RIGHT JOIN t1 ON ((t0.c0)<=(((NULL)-(t1.rowid)))) WHERE (NOT t1.c0) UNION ALL SELECT MAX('a') AS agg0 FROM t0 RIGHT JOIN t1 ON ((t0.c0)<=(((NULL)-(t1.rowid)))) WHERE ((t1.c0) IS NULL)) as asdf
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts <> e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts = e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts >= e.begin AND p.ts >= e.value ORDER BY p.ts ASC
WITH t1 AS ( FROM VALUES (1::INT, '2020-01-01 00:00:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') AS t1(a, b) ), t2 AS ( FROM VALUES (1::INT, '2020-01-01 00:01:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') t2(c, d) ) SELECT * FROM t1 ASOF JOIN t2 ON t1=b == t2.d AND t1.b >= t2.d - INTERVAL '1' SECOND
SELECT * FROM left_table l ASOF RIGHT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF FULL JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT (FALSE) IN (TRUE, (SELECT TIME '13:35:07' FROM t1) BETWEEN t0.c0 AND t0.c0) FROM t0
SELECT 2 ^ ANY(SELECT * FROM integers)
SELECT 2 ^ ANY([1, 2, 3])
SELECT * FROM integers, (SELECT SUM(i)) t(sum)
SELECT * FROM integers, LATERAL (SELECT SUM(i)) t(sum)
SELECT * FROM integers, LATERAL (SELECT integers.*) t2(k) ORDER BY i
SELECT * FROM integers, LATERAL (SELECT *) t2(k) ORDER BY i
SELECT * FROM integers LEFT JOIN LATERAL (SELECT integers.i WHERE integers.i IN (1, 3)) t(b) ON (i+b<b) ORDER BY i
SELECT * FROM (SELECT * FROM integers WHERE i=2) t(i) FULL JOIN LATERAL (SELECT t.i WHERE t.i IN (1, 3)) t2(b) ON (i=b) ORDER BY i, b
SELECT * FROM (SELECT * FROM integers WHERE i=2) t(i) RIGHT JOIN LATERAL (SELECT t.i WHERE t.i IN (1, 3)) t2(b) ON (i=b) ORDER BY i, b
select 1 from tenk1 a, lateral (select max(a.unique1) from int4_tbl b) ss
update xx1 set x2 = f1 from (select * from int4_tbl where f1 = x1) ss
update xx1 set x2 = f1 from (select * from int4_tbl where f1 = xx1.x1) ss
update xx1 set x2 = f1 from lateral (select * from int4_tbl where f1 = x1) ss
delete from xx1 using (select * from int4_tbl where f1 = x1) ss
delete from xx1 using (select * from int4_tbl where f1 = xx1.x1) ss
delete from xx1 using lateral (select * from int4_tbl where f1 = x1) ss
select array(select distinct i from t order by x.i desc) as a
SELECT ARRAY (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 ORDER by -1) AS new_array
SELECT ARRAY (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 ORDER by 2) AS new_array
SELECT ARRAY (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 ORDER by 'hello world') AS new_array
select array(select 1,2)
SELECT (SELECT l_linestat FROM orders) FROM lineitem
SELECT (SELECT l_returnfla FROM orders) FROM lineitem
SELECT (SELECT o_totalp FROM orders) FROM lineitem
SELECT * FROM lineitem WHERE (SELECT SUM(l_orderkey) > 0)
SELECT * FROM lineitem WHERE (SELECT SUM(o_orderke) FROM orders)
SELECT * FROM lineitem WHERE (SELECT SUM(o_orderke) OVER () FROM orders)
SELECT * FROM lineitem GROUP BY (SELECT SUM(o_orderke) OVER () FROM orders)
SELECT * FROM lineitem LIMIT (SELECT SUM(o_orderke) FROM orders LIMIT 1)
SELECT DaysToManufacture, StandardCost, (SELECT ["0", "1", "2", "3", "4"] FROM (SELECT DaysToManufacture, StandardCost) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable ) FROM Product
SELECT DaysToManufacture, StandardCost, (SELECT cost FROM (SELECT DaysToManufacture, StandardCost) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable UNPIVOT ( cost FOR days IN (0, 1, 2, 3, 4) ) ) FROM Product
SELECT DaysToManufacture, StandardCost, (SELECT LIST(cost) FROM (SELECT DaysToManufacture, StandardCost) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable UNPIVOT INCLUDE NULLS ( cost FOR days IN (0, 1, 2, 3, 4) ) ) FROM Product
SELECT (1, 2) IN (SELECT (i, i + 1, i + 2) FROM integers)
SELECT ROW(1, 2) IN (SELECT i1.i, i1.i + 1) FROM integers i1
SELECT * FROM integers s1 LEFT OUTER JOIN integers s2 ON (SELECT CASE WHEN s1.i+s2.i>10 THEN TRUE ELSE FALSE END) ORDER BY s1.i
SELECT * FROM integers s1 LEFT OUTER JOIN integers s2 ON s1.i=s2.i AND (SELECT CASE WHEN s1.i>2 THEN TRUE ELSE FALSE END) ORDER BY s1.i
SELECT i, (SELECT SUM(s1.i) FROM integers s1 LEFT OUTER JOIN integers s2 ON s1.i=s2.i OR s1.i=i1.i-1) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(s1.i) FROM integers s1 FULL OUTER JOIN integers s2 ON s1.i=s2.i OR s1.i=i1.i-1) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT row_number() OVER (ORDER BY i)) FROM integers i1 ORDER BY i
SELECT i, (SELECT 42+i1.i FROM integers) AS j FROM integers i1 ORDER BY i
SELECT i, (WITH i2 AS (SELECT 42+i1.i AS j FROM integers) SELECT j FROM i2) AS j FROM integers i1 ORDER BY i
SELECT i, (SELECT SUM(i + 1) OVER ()) FROM integers ORDER BY i
SELECT (col1 + 1) IN (SELECT ColID + (col1 + 1) FROM tbl_ProductSales) FROM another_T GROUP BY (col1 + 1)
SELECT col1+1, col1+42 FROM another_T GROUP BY col1+1
SELECT (col1 + 1) IN (SELECT ColID + (col1 + 42) FROM tbl_ProductSales) FROM another_T GROUP BY (col1 + 1)
SELECT CASE WHEN NOT col1 NOT IN (SELECT (SELECT MAX(col7)) UNION (SELECT MIN(ColID) FROM tbl_ProductSales LEFT JOIN another_T t2 ON t2.col5 = t1.col1)) THEN 1 ELSE 2 END FROM another_T t1 GROUP BY col1 ORDER BY 1
SELECT EXISTS (SELECT RANK() OVER (PARTITION BY SUM(DISTINCT col5))) FROM another_T t1
SELECT (SELECT SUM(col2) OVER (PARTITION BY SUM(col2) ORDER BY MAX(col1 + ColID) ROWS UNBOUNDED PRECEDING) FROM tbl_ProductSales) FROM another_T t1 GROUP BY col1
SELECT * FROM (SELECT 42, 41 AS x) v1(a, b, c)
SELECT (SELECT a * 42 FROM test)
SELECT * FROM (WITH cte AS (SELECT 42, 41 AS x) SELECT * FROM cte) v1(a, b, c)
SELECT (WITH cte AS (SELECT a * 42 FROM test) SELECT * FROM cte)
SELECT * FROM integers WHERE i=(SELECT i FROM integers WHERE i IS NOT NULL ORDER BY i)
SELECT * FROM integers WHERE i=(SELECT 1, 2)
SELECT * FROM integers WHERE i=(SELECT i, i + 2 FROM integers)
SELECT (SELECT * FROM integers i1, integers i2)
SELECT 3 IN (SELECT * FROM strings WHERE v=s1.v) FROM strings s1 ORDER BY v
SELECT quantile_cont(NULL, CAST(NULL AS DOUBLE[]))
SELECT * FROM exam QUALIFY row_number() OVER w = 1 WINDOW w AS (ORDER BY mark)
SELECT b, avg(a) AS avga FROM test GROUP BY b QUALIFY avga > 10
SELECT b FROM test QUALIFY avga() > 10
SELECT b, SUM(a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) > sum
SELECT plus1(2)
DROP TABLE test.t
SELECT * FROM test.v
select a FROM ( values (1,2), (3,2) ) t(a, b) HAVING true
SELECT a FROM test WHERE a=13 HAVING a > 11
SELECT a FROM test WHERE a=13 HAVING SUM(a) > 11
select approx_top_k(i, 0) from range(5) t(i)
select approx_top_k(i, -1) from range(5) t(i)
select approx_top_k(i, 999999999999999) from range(5) t(i)
select approx_top_k(i, NULL) from range(5) t(i)
SELECT equi_width_bins(-0.0, -1.0, 5, true)
SELECT equi_width_bins(0.0, 'inf'::double, 5, true)
SELECT equi_width_bins(0.0, 'nan'::double, 5, true)
SELECT equi_width_bins(0.0, 1.0, -1, true)
SELECT equi_width_bins(0.0, 1.0, 99999999, true)
SELECT equi_width_bins('a'::VARCHAR, 'z'::VARCHAR, 2, true)
SELECT * FROM histogram_values(integers, k)
SELECT * FROM histogram_values(integers, (i%2)::VARCHAR, technique := 'equi-width')
SELECT SUM(s) FROM strings GROUP BY g ORDER BY g
SELECT AVG(s) FROM strings GROUP BY g ORDER BY g
SELECT AVG(b) FROM booleans GROUP BY g ORDER BY g
SELECT COUNT(1, 2)
SELECT SUM('hello')
SELECT SUM(DATE '1992-02-02')
SELECT SUM()
SELECT SUM(1, 2)
SELECT MIN()
SELECT MAX()
SELECT FIRST()
SELECT approx_quantile(r, -0.1) FROM quantile
SELECT approx_quantile(r, 1.1) FROM quantile
SELECT approx_quantile(r, NULL) FROM quantile
SELECT approx_quantile(r, r) FROM quantile
SELECT approx_quantile(r::string, 0.5) FROM quantile
SELECT approx_quantile(r) FROM quantile
SELECT approx_quantile(r, 0.1, 0.2) FROM quantile
SELECT approx_quantile(42, CAST(NULL AS INT[]))
select approx_count_distinct(*)
select argmin()
select argmin(*)
select argmax()
select argmax(*)
select arg_min_null()
select arg_min_null(*)
select arg_max_null()
select arg_max_null(*)
SELECT AVG()
SELECT AVG(1, 2, 3)
SELECT AVG(AVG(1))
SELECT histogram(n, [10, 20, NULL]) FROM obs
SELECT histogram(n, NULL::BIGINT[]) FROM obs
SELECT BIT_AND()
SELECT BIT_AND(1, 2, 3)
SELECT BIT_AND(BIT_AND(1))
SELECT BIT_OR()
SELECT BIT_OR(1, 2, 3)
SELECT BIT_OR(BIT_AND(1))
SELECT BIT_XOR()
SELECT BIT_XOR(1, 2, 3)
SELECT BIT_XOR(BIT_XOR(1))
SELECT BITSTRING_AGG(i, -10, 20) FROM ints
SELECT BITSTRING_AGG(i, 2, 15) FROM tinyints
SELECT BITSTRING_AGG()
SELECT BITSTRING_AGG(1, 3, 4, 8, 0)
select bool_or(0)
select bool_and(0)
select bool_or()
select bool_and()
select bool_or(*)
select bool_and(*)
select corr()
select corr(*)
SELECT corr(a,b) FROM (values (1e301, 0), (-1e301, 0)) tbl(a,b)
SELECT corr(b,a) FROM (values (1e301, 0), (-1e301, 0)) tbl(a,b)
SELECT COUNT(DISTINCT *) FROM integers
SELECT COVAR_POP()
SELECT COVAR_POP(1, 2, 3)
SELECT COVAR_POP(COVAR_POP(1))
SELECT COVAR_SAMP()
SELECT COVAR_SAMP(1, 2, 3)
SELECT COVAR_SAMP(COVAR_SAMP(1))
select entropy()
select entropy(*)
select histogram()
select histogram(*)
SELECT COUNT(1, 2, 3)
SELECT COUNT(COUNT(1))
SELECT STDDEV_SAMP()
SELECT STDDEV_SAMP(1, 2, 3)
SELECT STDDEV_SAMP(STDDEV_SAMP(1))
SELECT SUM(1, 2, 3)
SELECT SUM(SUM(1))
SELECT FIRST(1, 2, 3)
select kurtosis()
select kurtosis(*)
select kurtosis(i) from (values (2e304), (2e305), (2e306), (2e307)) tbl(i)
select mode()
select mode(*)
SELECT user_id, list(DISTINCT cause ORDER BY "date" DESC) FILTER(cause IS NOT NULL) AS causes FROM user_causes GROUP BY user_id
SELECT "dest", mode() WITHIN GROUP (ORDER BY "arr_delay", "arr_time") AS "median_delay" FROM "flights" GROUP BY "dest"
SELECT "dest", duck(0.5) WITHIN GROUP (ORDER BY "arr_delay") AS "duck_delay" FROM "flights" GROUP BY "dest"
select percentile_disc() within group(order by i) from generate_series(0,100) tbl(i)
select percentile_disc(0.25, 0.5) within group(order by i) from generate_series(0,100) tbl(i)
select percentile_cont() within group(order by i) from generate_series(0,100) tbl(i)
select percentile_cont(0.25, 0.5) within group(order by i) from generate_series(0,100) tbl(i)
SELECT percentile_disc(CAST('NaN' AS REAL)) WITHIN GROUP (ORDER BY 1)
SELECT percentile_disc([]) WITHIN GROUP (ORDER BY LAST)
select product()
select product(*)
SELECT quantile_cont(r, NULL) FROM quantile
SELECT quantile_cont(interval (r/100) second, 0.5) FROM quantile
SELECT quantile_cont(r, -1.1) FROM quantile
SELECT quantile_cont(r, 1.1) FROM quantile
SELECT quantile_cont(r, "string") FROM quantile
SELECT quantile_cont(r::string, 0.5) FROM quantile
SELECT quantile_cont(r) FROM quantile
SELECT quantile_cont(r, 0.1, 50) FROM quantile
SELECT quantile_cont(interval (r/100) second, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(r, [-0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(r, (0.25, 0.5, 1.1)) FROM quantiles
SELECT quantile_cont(r, [0.25, 0.5, NULL]) FROM quantiles
SELECT quantile_cont(r, ["0.25", "0.5", "0.75"]) FROM quantiles
SELECT quantile_cont(r::string, [0.25, 0.5, 0.75]) FROM quantiles
SELECT quantile_cont(r, [0.25, 0.5, 0.75], 50) FROM quantiles
SELECT quantile_disc(r, -1.1) FROM quantile
SELECT quantile_disc(r, 1.1) FROM quantile
SELECT quantile_disc(r, "string") FROM quantile
SELECT quantile_disc(r, NULL) FROM quantile
SELECT quantile_disc(r) FROM quantile
SELECT quantile_disc(r, 0.1, 50) FROM quantile
SELECT quantile_cont(r, q) FROM quantile
SELECT quantile_disc(r, [-0.1, 0.5, 0.9]) FROM quantiles
SELECT quantile_disc(r, (0.1, 0.5, 1.1)) FROM quantiles
SELECT quantile_disc(r, [0.1, 0.5, NULL]) FROM quantiles
SELECT quantile_disc(r, ["0.1", "0.5", "0.9"]) FROM quantiles
SELECT quantile_disc(r, [0.1, 0.5, 0.9], 50) FROM quantiles
select regr_avgx()
select regr_avgx(*)
select regr_avgy()
select regr_avgy(*)
select regr_count()
select regr_count(*)
select regr_slope()
select regr_slope(*)
select sem()
select sem(*)
select skewness()
select skewness(*)
select skewness(i) from (values (-2e307), (0), (2e307)) tbl(i)
SELECT list(d) EXPORT_STATE from dummy
SELECT string_agg(d, ',') EXPORT_STATE from dummy
SELECT string_agg(d) EXPORT_STATE from dummy
SELECT FINALIZE(COMBINE(SUM(d) EXPORT_STATE, AVG(d) EXPORT_STATE)) FROM dummy
SELECT combine(NULL, NULL)
SELECT combine(42, 42)
SELECT finalize(NULL)
SELECT finalize(42)
select stddev(a) from (values (1e301), (-1e301)) tbl(a)
select var_samp(a) from (values (1e301), (-1e301)) tbl(a)
select var_pop(a) from (values (1e301), (-1e301)) tbl(a)
SELECT STRING_AGG()
SELECT STRING_AGG('a', 'b', 'c')
SELECT STRING_AGG(STRING_AGG('a',','))
SELECT STRING_AGG(x,','), STRING_AGG(x,y) FROM strings
SELECT STRING_AGG(1, 2)
SELECT g, STRING_AGG(x, y ORDER BY x ASC) FROM strings GROUP BY g ORDER BY 1
SELECT g, STRING_AGG(x, y ORDER BY x DESC) FROM strings GROUP BY g ORDER BY 1
SELECT g, STRING_AGG(DISTINCT y, ',' ORDER BY x DESC) FILTER (WHERE g < 4) FROM strings GROUP BY g ORDER BY 1
SELECT SUM(b)::BIGINT FROM bigints
SELECT (sum(n) WITHIN GROUP(ORDER BY ABS(n)))::BIGINT FROM doubles
SELECT (g+i)%2 + SUM(i), SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY 1
SELECT SUM(i) FROM integers GROUP BY ALL ORDER BY g
SELECT SUM(SUM(41)), COUNT(*)
SELECT b % 2 AS f, COUNT(SUM(a)) FROM test GROUP BY f
SELECT i, SUM(j), j FROM integers GROUP BY i ORDER BY i
SELECT 1 AS k, SUM(i) FROM integers GROUP BY k+1 ORDER BY 2
SELECT i % 2 AS k, SUM(i) FROM integers WHERE i IS NOT NULL GROUP BY 42 HAVING i%2>0
SELECT i % 2 AS k, SUM(k) FROM integers GROUP BY k
SELECT (10-i) AS k, SUM(i) FROM integers GROUP BY k ORDER BY i
SELECT * FROM tbl GROUP BY DEFAULT
SELECT * FROM tbl GROUP BY SUM(41)
SELECT DISTINCT ON (2) i FROM integers
SELECT DISTINCT ON(i, 'literal') i FROM integers
PREPARE v1 AS select distinct on (?) 42
SELECT DISTINCT ON (COLUMNS('nonexistent')) * FROM grouped_table
SELECT g, STRING_AGG(DISTINCT y ORDER BY y, '_' ) FILTER (WHERE g < 4) FROM strings GROUP BY g ORDER BY 1
select course, count(*) from students group by cube () order by 1, 2
select course, count(*) from students group by cube (cube (course)) order by 1, 2
select course, count(*) from students group by cube (grouping_sets (course)) order by 1, 2
select course, type, count(*) from students group by cube (course, type, course, type, course, type, course, type, course, type, course, type, (course, type), (course, type), course, type) order by 1, 2, 3
select course, type, count(*) from students group by cube (course, type, course, type, course), cube(type, course, type, course), cube(type, course, type, (course, type), (course, type), course, type) order by 1, 2, 3
SELECT GROUPING()
SELECT GROUPING() FROM students
SELECT GROUPING(NULL) FROM students
SELECT GROUPING(course) FROM students
SELECT GROUPING(course) FROM students GROUP BY ()
SELECT GROUPING(type) FROM students GROUP BY course
SELECT GROUPING(course) FROM students WHERE GROUPING(course)=0 GROUP BY course
select course from students group by ()
select course, count(*) from students group by rollup () order by 1, 2
select course, count(*) from students group by rollup (rollup (course)) order by 1, 2
select course, count(*) from students group by rollup (grouping_sets (course)) order by 1, 2
select fill(i order by 10-i, i * i) over (order by i) from range(3) tbl(i)
select fill(i::VARCHAR order by i) over (order by i) from range(3) tbl(i)
select fill(i order by i::VARCHAR) over (order by i) from range(3) tbl(i)
SELECT depname, min(salary) OVER (PARTITION BY depname ORDER BY salary, empno) m1 FROM empsalary GROUP BY m1 ORDER BY depname, empno
select row_number() over (range between unbounded following and unbounded preceding)
select row_number() over (range between unbounded preceding and unbounded preceding)
SELECT i, lead(i ORDER BY i // 2, i) OVER w AS f, FROM range(10) tbl(i) WINDOW w AS ( ORDER BY i // 2 ROWS BETWEEN 3 PRECEDING AND 3 FOLLOWING EXCLUDE TIES ) ORDER BY i
SELECT i, v, sum(v) OVER (ORDER BY i RANGE BETWEEN 1 PRECEDING AND -1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i RANGE BETWEEN -1 FOLLOWING AND 1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i RANGE BETWEEN -1 PRECEDING AND 1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i RANGE BETWEEN 1 PRECEDING AND -1 PRECEDING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i DESC RANGE BETWEEN 1 PRECEDING AND -1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i DESC RANGE BETWEEN -1 FOLLOWING AND 1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i DESC RANGE BETWEEN -1 PRECEDING AND 1 FOLLOWING) FROM issue10855
SELECT i, v, sum(v) OVER (ORDER BY i DESC RANGE BETWEEN 1 PRECEDING AND -1 PRECEDING) FROM issue10855
SELECT depname, empno, nth_value(empno) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary
SELECT depname, empno, nth_value(empno, 2, 3) OVER ( PARTITION BY depname ORDER BY empno ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING ) fv FROM empsalary
SELECT TeamName, Player, Score, NTILE() OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(1,2) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(1,2,3) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(1,2,3,4) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(-1) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT TeamName, Player, Score, NTILE(0) OVER (PARTITION BY TeamName ORDER BY Score ASC) AS NTILE FROM ScoreBoard s ORDER BY TeamName, Score
SELECT i, j, ROW_NUMBER() OVER (ORDER BY ALL) AS rn FROM ( SELECT i ,j FROM generate_series(1, 5) s(i) CROSS JOIN generate_series(1, 2) t(j) ) t
SELECT concat() OVER ()
SELECT nonexistingfunction() OVER ()
SELECT avg(row_number() over ()) over ()
SELECT avg(42) over (partition by row_number() over ())
SELECT avg(42) over (order by row_number() over ())
SELECT MIN(a) OVER (PARTITION BY i ORDER BY i) FROM integers
SELECT MIN(i) OVER (PARTITION BY a ORDER BY i) FROM integers
SELECT MIN(i) OVER (PARTITION BY i ORDER BY a) FROM integers
SELECT MIN(i) OVER (PARTITION BY i, a ORDER BY i) FROM integers
SELECT MIN(i) OVER (PARTITION BY i ORDER BY i, a) FROM integers
WITH subquery AS (SELECT i, lag(i) OVER named_window FROM ( VALUES (1), (2), (3)) AS t (i)) SELECT * FROM subquery window named_window AS ( ORDER BY i)
select i, lag(i) over named_window from (values (1), (2), (3)) as t (i) window named_window as (order by i), named_window as (order by j)
select x, y, count(*) over (partition by y order by x), count(*) over (w order by x) from (values (1, 1), (2, 1), (3, 2), (4, 2)) as t (x, y) window w as (partition by y order by x desc) order by x
select x, y, count(*) over (partition by y order by x), count(*) over (w partition by y) from (values (1, 1), (2, 1), (3, 2), (4, 2)) as t (x, y) window w as (partition by x) order by x
select i, sum(i) over (w) as smoothed from integers window w AS (order by i rows between 1 preceding and 1 following) order by i
SELECT sum(1) over cumulativeSum FROM integers WINDOW cumulativeSum AS (), cumulativesum AS (order by i rows between 1 preceding and 1 following)
select j, s, string_agg(s, sep) over (partition by j order by s) from a order by j, s
SELECT SUM(i) OVER (ROWS BETWEEN UNNEST([1]) PRECEDING AND 1 FOLLOWING) FROM tbl
SELECT SUM(i) OVER (ROWS BETWEEN 1 PRECEDING AND UNNEST([1]) FOLLOWING) FROM tbl
SELECT lead(c0, UNNEST([1])) OVER (ROWS BETWEEN 2 PRECEDING AND 4 PRECEDING) FROM (VALUES (1, 2)) a(c0)
SELECT x, count(x) FILTER (WHERE x % 2 = UNNEST([2])) OVER (ORDER BY x ROWS BETWEEN 2 PRECEDING AND 2 FOLLOWING) FROM generate_series(0, 10) tbl(x)
SELECT lead(c0, 0, UNNEST([1])) OVER (ROWS BETWEEN 2 PRECEDING AND 4 PRECEDING) FROM (VALUES (1, 2)) a(c0)
SELECT lag(ten, four, 0, 0) OVER (PARTITION BY four ORDER BY ten) lt FROM tenk1 order by four, ten, lt
with cte as (select 42 AS a) (DESCRIBE TABLE cte)
(DESCRIBE TABLE cte) ORDER BY 1
WITH cte1 AS ( SELECT 42 AS cte1c1, [84] AS cte1c2 ), cte2 AS ( SELECT * FROM t2 s ) INSERT OR REPLACE INTO t1 SELECT * FROM cte2
with recursive t as (select 1 as x intersect select x+1 from t where x < 3) select * from t order by x
with recursive t as (select 1 as x except select x+1 from t where x < 3) select * from t order by x
INSERT INTO table2 WITH cte AS (INSERT INTO table1 SELECT 1, 2 RETURNING id) SELECT id FROM cte
WITH RECURSIVE tag_hierarchy(id, source, path, target) AS ( SELECT id, name, name AS path, NULL AS target FROM tag WHERE subclassof IS NULL UNION ALL SELECT tag.id, tag.name, tag_hierarchy.path || ' <- ' || tag.name, tag.name AS target FROM tag, tag_hierarchy WHERE tag.subclassof = tag_hierarchy.id ) SELECT source, path, target FROM tag_hierarchy
with cte1 as (select 42), cte1 as (select 42) select * FROM cte1
with cte3 as (select ref2.j as i from cte1 as ref2), cte1 as (Select i as j from a), cte2 as (select ref.j+1 as k from cte1 as ref) select * from cte2 union all select * FROM cte3
WITH t(x) AS (SELECT x) SELECT * FROM range(10) AS _(x), LATERAL (SELECT * FROM t)
WITH cte AS (SELECT x) SELECT b.x FROM (SELECT 1) _(x), LATERAL (SELECT * FROM cte) b(x)
with cte as (select * from cte) select * from cte
with recursive t as (select 1 as x union select sum(x+1) from t where x < 3 order by x) select * from t
with recursive t as (select 1 as x union select sum(x+1) from t where x < 3 LIMIT 1) select * from t
with recursive t as (select 1 as x union select sum(x+1) from t where x < 3 OFFSET 1) select * from t
with recursive t as (select 1 as x union select sum(x+1) from t where x < 3 LIMIT 1 OFFSET 1) select * from t
with recursive t as (select 1 as x union all select x+1 from t where x < 3 order by x) select * from t
with recursive t as (select 1 as x union all select x+1 from t where x < 3 LIMIT 1) select * from t
with recursive t as (select 1 as x union all select x+1 from t where x < 3 OFFSET 1) select * from t
with recursive t as (select 1 as x union all select x+1 from t where x < 3 LIMIT 1 OFFSET 1) select * from t
with recursive t as MATERIALIZED (select 1 as x intersect select x+1 from t where x < 3) select * from t order by x
with recursive t as MATERIALIZED (select 1 as x except select x+1 from t where x < 3) select * from t order by x
WITH RECURSIVE tag_hierarchy(id, source, path, target) AS materialized ( SELECT id, name, name AS path, NULL AS target FROM tag WHERE subclassof IS NULL UNION ALL SELECT tag.id, tag.name, tag_hierarchy.path || ' <- ' || tag.name, tag.name AS target FROM tag, tag_hierarchy WHERE tag.subclassof = tag_hierarchy.id ) SELECT source, path, target FROM tag_hierarchy
with cte1 as MATERIALIZED (select 42), cte1 as MATERIALIZED (select 42) select * FROM cte1
with cte as MATERIALIZED (select * from cte) select * from cte
with cte3 as MATERIALIZED (select ref2.j as i from cte1 as ref2), cte1 as MATERIALIZED (Select i as j from a), cte2 as MATERIALIZED (select ref.j+1 as k from cte1 as ref) select * from cte2 union all select * FROM cte3
WITH t0(x) AS MATERIALIZED ( SELECT x FROM t1 ), t1(x) AS MATERIALIZED ( SELECT 1 ) SELECT * FROM t0
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3 order by x) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3 LIMIT 1) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3 OFFSET 1) select * from t
with recursive t as MATERIALIZED (select 1 as x union all select x+1 from t where x < 3 LIMIT 1 OFFSET 1) select * from t
with recursive t as MATERIALIZED (select 1 as x union select sum(x+1) from t where x < 3 order by x) select * from t
with recursive t as MATERIALIZED (select 1 as x union select sum(x+1) from t where x < 3 LIMIT 1) select * from t
with recursive t as MATERIALIZED (select 1 as x union select sum(x+1) from t where x < 3 OFFSET 1) select * from t
with recursive t as MATERIALIZED (select 1 as x union select sum(x+1) from t where x < 3 LIMIT 1 OFFSET 1) select * from t
SELECT * FROM generate_series(0,10,1) LIMIT 3 OFFSET -1
SELECT * FROM generate_series(0,10,1) LIMIT -3
SELECT * FROM generate_series(0,10,1) LIMIT -1%
SELECT * FROM generate_series(0,10,1) LIMIT (SELECT k FROM integers)
SELECT * FROM generate_series(0,10,1) LIMIT 1 OFFSET (SELECT k FROM integers)
SELECT 42 ORDER BY -9223372036854775808
SELECT a FROM test LIMIT a
SELECT a FROM test LIMIT a+1
SELECT a FROM test LIMIT SUM(42)
SELECT a FROM test LIMIT row_number() OVER ()
select 1 limit date '1992-01-01'
SELECT * FROM integers as int LIMIT (SELECT -1)
SELECT * FROM integers as int LIMIT (SELECT 'ab')
SELECT * FROM t ORDER BY x LIMIT (SELECT -1)
SELECT 'Test' LIMIT ?
EXECUTE v7(NULL, 922337203685477580700)
SELECT a FROM test LIMIT a %
SELECT a FROM test LIMIT (a+1) %
SELECT a FROM test LIMIT (a+b*c) %
SELECT a FROM test LIMIT SUM(42) %
SELECT * FROM range(100) LIMIT -10 %
SELECT * FROM test LIMIT (SELECT 'ab') %
select 1 limit date '2021-11-25' %
select * from test limit "Hello World" %
PRAGMA default_null_order())
PRAGMA default_null_order='UNKNOWN'
PRAGMA default_null_order=UNKNOWN)
PRAGMA default_null_order=3)
SELECT a-10 AS k FROM test UNION SELECT a-10 AS l FROM test ORDER BY 1-k
SELECT a FROM test ORDER BY 'hello world', a
SELECT a FROM test ORDER BY 2
SELECT a FROM test ORDER BY 'hello', a
SELECT a % 2, b FROM test UNION SELECT b, a % 2 AS k ORDER BY a % 2
SELECT a % 2, b FROM test UNION SELECT a % 2 AS k, b FROM test ORDER BY 3
SELECT a % 2, b FROM test UNION SELECT a % 2 AS k, b FROM test ORDER BY -1
SELECT a % 2, b FROM test UNION SELECT a % 2 AS k FROM test ORDER BY -1
PRAGMA default_order())
PRAGMA default_order='UNKNOWN'
PRAGMA default_order=UNKNOWN)
PRAGMA default_order=3)
insert into null_map values (null), (map([null], [null]))
CREATE TYPE l AS X(0,0,0,0,0,0,0,0,0,0,0)
CREATE TYPE ll AS XX(1,2,3,4,5,6,7,8,9,10)
CREATE TYPE ll AS XX(1,2,3,4,5,6,7,8,9)
CREATE TYPE ll AS XX(a,2,3,4,5,6,7,8,9)
SELECT 128::DECIMAL(3,0)::TINYINT
SELECT -128::DECIMAL(9,0)::TINYINT
SELECT 128::DECIMAL(18,0)::TINYINT
SELECT 14751947891758972421513::DECIMAL(38,0)::TINYINT
SELECT -32768::DECIMAL(9,0)::SMALLINT
SELECT 32768::DECIMAL(18,0)::SMALLINT
SELECT 14751947891758972421513::DECIMAL(38,0)::SMALLINT
SELECT 2147483648::DECIMAL(18,0)::INTEGER
SELECT 100::TINYINT::DECIMAL(3,1)
SELECT 1::TINYINT::DECIMAL(3,3)
SELECT 100::TINYINT::DECIMAL(18,17)
SELECT 100::TINYINT::DECIMAL(9,7)
SELECT 100::TINYINT::DECIMAL(38,37)
SELECT 100::SMALLINT::DECIMAL(3,1)
SELECT 1::SMALLINT::DECIMAL(3,3)
SELECT 100::SMALLINT::DECIMAL(18,17)
SELECT ('0.54321543215432154321543215432154321'::DECIMAL(35,35) + 10000)::VARCHAR
SELECT '0.000000000000000000000000000001'::DECIMAL(38,30) * '0.000000000000000000000000000001'::DECIMAL(38,30)
SELECT 10.00::DECIMAL(4,2)::DECIMAL(4,3)
SELECT 10.00::DECIMAL(4,2)::DECIMAL(9,8)
SELECT 10.00::DECIMAL(4,2)::DECIMAL(18,17)
SELECT 10.00::DECIMAL(4,2)::DECIMAL(38,37)
SELECT 10.00::DECIMAL(4,2)::DECIMAL(2,1)
SELECT 10.00::DECIMAL(9,7)::DECIMAL(7,6)
SELECT 10.00::DECIMAL(18,16)::DECIMAL(16,15)
SELECT 10.00::DECIMAL(38,36)::DECIMAL(36,35)
select cast('9.99' as decimal(1,0))
select cast(9.99::float as decimal(1,0))
select cast(9.99::double as decimal(1,0))
select cast(9.99 as decimal(1,0))
select cast(9.5 as decimal(1,0))
select cast(-9.99 as decimal(1,0))
select cast(-9.5 as decimal(1,0))
select cast(-9.999999999 as decimal(1,0))
SELECT '1e4'::DECIMAL(4,0)
SELECT '1e9'::DECIMAL(9,0)
SELECT '1e18'::DECIMAL(18,0)
SELECT '1e38'::DECIMAL(38,0)
SELECT '1e100'::DECIMAL
SELECT '1e100e100'::DECIMAL
SELECT '1e100.2'::DECIMAL
SELECT '1e9999999999'::DECIMAL
select (99000000000000000.0::DECIMAL(18,1)+99000000000000000.0::DECIMAL(18,1))
select (99000000000000000.0::DECIMAL(18,1)+99000000000000000.0::DECIMAL(18,1))::VARCHAR::DECIMAL(18,1)
select (50000000000000000.0::DECIMAL(18,1)+50000000000000000.0::DECIMAL(18,1))
select (-99000000000000000.0::DECIMAL(18,1)-99000000000000000.0::DECIMAL(18,1))
select (-50000000000000000.0::DECIMAL(18,1)-50000000000000000.0::DECIMAL(18,1))
select (9900000000000000000000000000000000000.0::DECIMAL(38,1)+9900000000000000000000000000000000000.0::DECIMAL(38,1))
select (5000000000000000000000000000000000000.0::DECIMAL(38,1)+5000000000000000000000000000000000000.0::DECIMAL(38,1))
select '10000000000000000000000000000000000000.0'::DECIMAL(38,1)
SELECT d+1000000000000000.0 FROM decimals
SELECT -1000000000000000.0-d FROM decimals
SELECT 2*d FROM decimals
SELECT CAST(1000 AS DECIMAL(3,0))
SELECT CAST(100 AS DECIMAL(2,0))
SELECT CAST('100' AS DECIMAL(2,0))
SELECT CAST('100'::DOUBLE AS DECIMAL(2,0))
SELECT CAST(100::DECIMAL(3,0) AS DECIMAL(2,0))
SELECT CAST(10000::DECIMAL(5,0) AS DECIMAL(2,0))
SELECT CAST(1000000000::DECIMAL(10,0) AS DECIMAL(2,0))
SELECT CAST(1000000000::DECIMAL(20,0) AS DECIMAL(2,0))
SELECT '9223372036854788.758'::DECIMAL
SELECT '1'::DECIMAL(3, 3)::VARCHAR
SELECT '-1'::DECIMAL(3, 3)::VARCHAR
SELECT '0.1'::DECIMAL(3, 4)
SELECT '0.1'::DECIMAL('hello')
SELECT '0.1'::DECIMAL((-17))
SELECT '0.1'::DECIMAL(40)
SELECT '0.1'::DECIMAL(1, 2, 3)
SELECT ROUND(12::DECIMAL(3,0), i) FROM range(1) tbl(i)
CREATE TYPE t4 AS UNION ( v0 SETOF t4 )
CREATE TYPE t4 AS t4[]
CREATE TYPE t4 AS STRUCT(a t4)
CREATE TYPE alias AS INTEGER
CREATE TYPE alias as BLOBL
INSERT INTO a VALUES (ROW(1, 2, 3))
INSERT INTO a VALUES (ROW(1))
INSERT INTO a VALUES (ROW('hello', 1))
INSERT INTO a VALUES (ROW('hello', [1, 2]))
INSERT INTO a VALUES (ROW(1, ROW(1, 7)))
CREATE TABLE person ( name text, current_car car )
CREATE TABLE aliens ( name text, current_alias alias )
SELECT bitstring('', 0)
SELECT bitstring('5', 10)
SELECT bitstring('0101011')
INSERT INTO bits VALUES('101211010')
INSERT INTO bits VALUES('1A10')
SELECT ''::BIT
INSERT INTO bits VALUES ('')
SELECT '010110'::BIT & '11000'::BIT
SELECT '0110'::BIT | '11000'::BIT
SELECT xor('011010110'::BIT, '11000'::BIT)
SELECT '010101'::BIT << -2
INSERT INTO bits VALUES('0110108')
SELECT get_bit('10101'::BIT, 6)
SELECT get_bit('001'::BIT, -1)
SELECT set_bit('11111'::BIT, 2, 7)
SELECT set_bit('10101'::BIT, 6, 1)
SELECT set_bit('011'::BIT, -1, 0)
SELECT '340282366920938463463374607431768211455'::UHUGEINT + '340282366920938463463374607431768211455'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT + '10'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT - 10::UHUGEINT + 11::UHUGEINT
SELECT '0'::UHUGEINT - '1'::UHUGEINT
SELECT '340282366920938463463374607431768211455'::UHUGEINT * 2::UHUGEINT
SELECT '34028236692093846346'::UHUGEINT * '33746074317682114556'::UHUGEINT
SELECT 1::UHUGEINT + '340282366920938463463374607431768211455'::UHUGEINT
SELECT 0::UHUGEINT - 1::UHUGEINT
SELECT '-1267650600228229401496703205376'::UHUGEINT, '-17014118346046923173168730371588410572'::UHUGEINT
SELECT '340282366920938463463374607431768211456'::UHUGEINT
SELECT '-1'::UHUGEINT
SELECT (-42)::TINYINT::UHUGEINT, (-42)::SMALLINT::UHUGEINT, (-42)::INTEGER::UHUGEINT, (-42)::BIGINT::UHUGEINT, (-42)::FLOAT::UHUGEINT, (-42)::DOUBLE::UHUGEINT
SELECT 1000::UHUGEINT::TINYINT
SELECT 128::UHUGEINT::TINYINT
SELECT 100000::UHUGEINT::SMALLINT
SELECT 32768::UHUGEINT::SMALLINT
select '340282366920938463463374607431768211456e0'::UHUGEINT
select '3.4e39'::UHUGEINT
select '3.5e38'::UHUGEINT
SELECT 251658240::UHUGEINT * 19938419936773738093557105904205168640::UHUGEINT
SELECT 1080863910568919040::UHUGEINT * 4642275147320176030871715840::UHUGEINT
SELECT 1080863910568919040::UHUGEINT * 19938419936773738093557105904205168640::UHUGEINT
SELECT 4642275147320176030871715840::UHUGEINT * 1080863910568919040::UHUGEINT
SELECT 4642275147320176030871715840::UHUGEINT * 4642275147320176030871715840::UHUGEINT
SELECT 4642275147320176030871715840::UHUGEINT * 19938419936773738093557105904205168640::UHUGEINT
SELECT 19938419936773738093557105904205168640::UHUGEINT * 251658240::UHUGEINT
SELECT 19938419936773738093557105904205168640::UHUGEINT * 1080863910568919040::UHUGEINT
SELECT CAST('340282366920938463463374607431768211456' AS UHUGEINT)
SELECT CAST('340282366920938463463374607431768211456'::DOUBLE AS UHUGEINT)
SELECT CAST('-1' AS UHUGEINT)
SELECT (200)::UTINYINT + (200)::UTINYINT
SELECT (18446744073709551615)::UBIGINT + (18446744073709551615)::UBIGINT
SELECT (200)::UTINYINT * (200)::UTINYINT
SELECT (18446744073709551615)::UBIGINT * (3)::UBIGINT
SELECT (200)::UTINYINT - (201)::UTINYINT
SELECT (200)::UTINYINT - (201)::USMALLINT
SELECT '256'::UTINYINT
SELECT '65536'::USMALLINT
SELECT '4294967296'::UINTEGER
SELECT '18446744073709551616'::UBIGINT
SELECT (100::UTINYINT)::DECIMAL(2,0)
SELECT (100::USMALLINT)::DECIMAL(2,0)
SELECT (100::UINTEGER)::DECIMAL(2,0)
SELECT (100::UBIGINT)::DECIMAL(2,0)
SELECT '265'::UTINYINT
SELECT '-1'::UTINYINT
SELECT '-1'::USMALLINT
SELECT '-1'::UINTEGER
SELECT '-1'::UBIGINT
SELECT (9223372036854775807)::BIGINT::UTINYINT
SELECT (9223372036854775807)::BIGINT::USMALLINT
SELECT (9223372036854775807)::BIGINT::UINTEGER
SELECT interval '2 10' years to months
SELECT interval '2 10' days to hours
SELECT interval '12 15:06' days to minutes
SELECT interval '12 15:06:04.123' days to seconds
SELECT interval '12:30' hours to minutes
SELECT interval '15:06:04.123' hours to seconds
SELECT interval '12:30' minutes to seconds
SELECT interval '99999999999999' years
SELECT cast(' ' as interval)
SELECT cast('AAAA' as interval)
SELECT cast('3 doopiedoos' as interval)
SELECT cast('3 years 2 doy' as interval)
SELECT cast('3 yearweek' as interval)
SELECT interval '-2147483648 months' AS without_ago, interval '-2147483648 months ago' AS with_ago, interval '-2147483648 months' = interval '-2147483648 months ago' AS are_equal
select '9999999999:54:32.101234'::INTERVAL
select '-9999999999:54:32.101234'::INTERVAL
SELECT INTERVAL 'P2MT1H1M'
SELECT INTERVAL 'P00-02-00T01:00:01'
SELECT INTERVAL '2 month' * INTERVAL '1 month 3 days'
SELECT 2 / INTERVAL '1 year 2 days 2 seconds'
SELECT INTERVAL ''
SELECT INTERVAL '1000000' SECOND - DATE '1993-03-01'
select INTERVAL '2 HOUR' - '12:15:37.123456-08'::TIMETZ
select '5877642-06-24 (BC)'::date
select '5877680-06-23 (BC)'::date
select '99999999-06-23 (BC)'::date
select '290309-01-01 (BC)'::date::timestamp
select '5877642-06-23 (BC)'::date::timestamp
select '5877642-06-24 (BC)'::date - 1
select '5877642-06-24 (BC)'::date - 365
select '5877642-06-24 (BC)'::date - 2147483647
SELECT '1993-01-32'::DATE::VARCHAR
SELECT '1993-02-29'::DATE::VARCHAR
SELECT '1993-03-32'::DATE::VARCHAR
SELECT '1993-04-31'::DATE::VARCHAR
SELECT '1993-05-32'::DATE::VARCHAR
SELECT '1993-06-31'::DATE::VARCHAR
SELECT '1993-07-32'::DATE::VARCHAR
SELECT '1993-08-32'::DATE::VARCHAR
SELECT DATE '0000-01-01 (BC)'
SELECT DATE '-0030-01-01 (BC)'
SELECT i * 3 FROM dates
SELECT i / 3 FROM dates
SELECT i % 3 FROM dates
SELECT i + i FROM dates
SELECT ''::DATE
SELECT ' '::DATE
SELECT '1992'::DATE
SELECT '1992-'::DATE
INSERT INTO dates VALUES ('blabla')
INSERT INTO dates VALUES ('1993-20-14')
INSERT INTO dates VALUES ('1993-08-99')
INSERT INTO dates VALUES ('1993-02-29')
INSERT INTO dates VALUES ('1900-02-29')
INSERT INTO dates VALUES ('02-02-1992')
INSERT INTO dates VALUES ('1900a01a01')
INSERT INTO dates VALUES ('-100000000-01-01')
SELECT 'hello'::ENUM
SELECT 'hello'::ENUM(42)
SELECT 'hello'::ENUM('zzz', 42)
SELECT 'hello'::ENUM(foobar 42)
insert into m SELECT * FROM m_2
SELECT * FROM m WHERE m::mood_2=m_2
ALTER TABLE person ALTER name TYPE name_enum
ALTER TABLE person ALTER name TYPE bogus_name
select 'awesome-bro'::mood
select 0::mood
CREATE TYPE bla AS ENUM (1,2,3)
CREATE TYPE bla AS ENUM ('sad',NULL)
CREATE TYPE bla AS ENUM ('sad','sad')
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy', NULL)
select ['bla']::mood[]
select [1]::mood[]
select name, year_release::INT from albums_error
SELECT person_mood::pet_mood FROM person_pet_den
drop type mood
INSERT INTO person VALUES ('Moe', 'diego')
CREATE TABLE aliens ( name text, current_mood mood )
SELECT CAST ('bla' as mood)
insert into tbl_temp values ('bla', 'invalid')
Select * from t1, t2 where t1.a = t2.a
select count(*) from t1, t2 where t1.date = t2.b
select count(*) from t1, t2 where t1.time = t2.b
select count(*) from t1, t2 where t1.timestamp = t2.b
select count(*) from t1, t2 where t1.timestamp_s = t2.b
select count(*) from t1, t2 where t1.timestamp_ms = t2.b
select count(*) from t1, t2 where t1.timestamp_ns = t2.b
select count(*) from t1, t2 where t1.time_tz = t2.b
CREATE TYPE "integer" AS ENUM ('1', '2', '3')
SELECT 4::INTEGEE
DROP TYPE "INTEGER"
DROP TYPE "INTEGEE"
CREATE TYPE integer AS ENUM ('1', '2', '3')
select 'GEOMETRYCOLLECTION Z (POINT Z (1 1 2), LINESTRING (0 0, 1 1))'::GEOMETRY
INSERT INTO t_all_types VALUES (29, 'POINT (1 2)')
select st_crs(1)
insert into t1 select * from t2
select 'POINT(0 1)'::GEOMETRY((['abc']))
select 'POINT(0 1)'::GEOMETRY(4326)
select st_setcrs(g, srid) from t3
select typeof('POINT(0 1)'::GEOMETRY('GEOGCRS["WGS 84",foo[]'))
SELECT ''::TIME
SELECT ' '::TIME
SELECT '1'::TIME
SELECT '11'::TIME
SELECT '11:11:f'::TIME
SELECT date_part('julian', '23:59:59.123456789'::TIME_NS)
SELECT date_part(['julian'], '23:59:59.123456789'::TIME_NS)
SELECT '02:30:00>04'::TIMETZ
SELECT '02:30:00+4'::TIMETZ
SELECT '02:30:00+4xx'::TIMETZ
SELECT '02:30:00+2000'::TIMETZ
SELECT '02:30:00+20:xx'::TIMETZ
SELECT '02:30:00+20:45:xx'::TIMETZ
SELECT 'infinity'::TIMETZ
SELECT '50:42:04.500'::TIME::VARCHAR
SELECT '100:42:04.500'::TIME::VARCHAR
SELECT '14:70:04.500'::TIME::VARCHAR
SELECT '14:100:04.500'::TIME::VARCHAR
SELECT '14:42:70.500'::TIME::VARCHAR
SELECT '14-42-04'::TIME::VARCHAR
select cast (null as huh.what.u array)
select cast (null as what.u array)
select j::main.u[] from i
select cast (null as u2 [])
INSERT INTO a VALUES (1)
INSERT INTO a VALUES (ROW(1, 2))
INSERT INTO a VALUES (ROW(ROW(1, 2, 3), 1))
select remap_struct(42, NULL::ROW(v1 INT, v2 INT, v3 INT), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::INTEGER})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v2': 'i'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL, {'v2': 'i'}, NULL)
SELECT remap_struct(ROW(1, 2), NULL::ROW(v1 VARCHAR), {'v2': 'i'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v1': 'k'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v1': 'i'}, {'v1': NULL::VARCHAR})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR, v2 VARCHAR), {'v1': 'i'}, NULL)
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, struct_val) FROM structs
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::STRUCT("key" BIGINT[], "value" VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(BIGINT[], MAP(VARCHAR, VARCHAR)), { 'key': 'key', 'value': 'value' }, NULL )
SELECT i, CASE WHEN i%2=0 THEN {'i': [1,2,3]} ELSE {'i': ['hello']} END FROM range(6) tbl(i)
SELECT ({'hello': 3, 'hello': 4}) col
SELECT ({'HELLO': 3, 'HELLO': 4}) col
SELECT ({'HELLO': 3, 'hello': 4}) col
SELECT col['HELL'] FROM tbl
SELECT struct_concat()
SELECT struct_concat(NULL::STRUCT(k INT), 'not a struct')
SELECT struct_concat({'a': 'first struct'}, {'a': 'second struct'})
SELECT struct_concat({'a': 'first struct'}, {'A': 'second struct'})
SELECT struct_concat({'a': 1}, NULL)
SELECT struct_concat({'a': 'named struct'}, row(10))
SELECT struct_contains({'a': 1, 'b': 2}, 2)
CREATE TABLE wrong AS FROM (VALUES (ROW(3)))
INSERT INTO t1 VALUES ({c: 34})
INSERT INTO foo VALUES ({'ignoreme': 3})
INSERT INTO T VALUES ({l: 1, m: 2}), ({x: 3, y: 4})
CREATE TABLE tbl2 AS SELECT ROW(42, 'world') a
SELECT struct_position({'a': 1, 'b': 2}, 2)
SELECT UNNEST ( ( '1,2,3,4,,6' , ( 1 ) ) ) , x x
select row(42, 'hello') union all select '{'': 42,'': hello}'
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'a': 12, 'b': {'': {'': 12}}}], max_depth := 3))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{'': {'': 12}}], recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([{}], recursive := true, keep_parent_names := true))
SELECT column_name, column_type FROM (DESCRIBE SELECT unnest([]), recursive := true, keep_parent_names := true))
SELECT UNNEST({'a': 42, 'b': 88}) + 42
SELECT UNNEST(UNNEST([{'a': 42, 'b': 88}, {'a': NULL, 'b': 99}]))
SELECT * FROM tbl_structs ORDER BY UNNEST(s)
select unnest(s) from tbl_structs order by 2 collate nocase
SELECT (SELECT UNNEST(a).a) FROM (VALUES ({'a': 42, 'b': 88})) t(a)
UPDATE t0 SET (c0) = ROW()
SELECT a + b FROM test_vector_types(NULL::INT, NULL::INT) t(a, b)
SELECT i::UTINYINT FROM bigints
SELECT i::USMALLINT FROM bigints
SELECT i::UINTEGER FROM bigints
SELECT i::UBIGINT FROM bigints
SELECT i::UTINYINT FROM bigints WHERE i>=0 ORDER BY i
SELECT i::USMALLINT FROM bigints WHERE i>=0 ORDER BY i
SELECT i::UINTEGER FROM bigints WHERE i>=0 ORDER BY i
SELECT i::TINYINT FROM bigints ORDER BY i
SELECT i::UTINYINT FROM hugeints
SELECT i::USMALLINT FROM hugeints
SELECT i::UINTEGER FROM hugeints
SELECT i::UBIGINT FROM hugeints
SELECT i::UTINYINT FROM hugeints WHERE i>=0 ORDER BY i
SELECT i::USMALLINT FROM hugeints WHERE i>=0 ORDER BY i
SELECT i::UINTEGER FROM hugeints WHERE i>=0 ORDER BY i
SELECT i::UBIGINT FROM hugeints WHERE i>=0 ORDER BY i
SELECT i::UTINYINT FROM integers
SELECT i::USMALLINT FROM integers
SELECT i::UINTEGER FROM integers
SELECT i::UBIGINT FROM integers
SELECT i::UTINYINT FROM integers WHERE i>=0 ORDER BY i
SELECT i::USMALLINT FROM integers WHERE i>=0 ORDER BY i
SELECT i::TINYINT FROM integers ORDER BY i
SELECT i::SMALLINT FROM integers ORDER BY i
SELECT i::UTINYINT FROM smallints
SELECT i::USMALLINT FROM smallints
SELECT i::UINTEGER FROM smallints
SELECT i::UBIGINT FROM smallints
SELECT i::UTINYINT FROM smallints WHERE i>=0 ORDER BY i
SELECT i::TINYINT FROM smallints ORDER BY i
SELECT s::SMALLINT FROM strings
SELECT i::DECIMAL(3,0)::SMALLINT FROM smallints ORDER BY i
SELECT i::UTINYINT FROM tinyints
SELECT i::USMALLINT FROM tinyints
SELECT i::UINTEGER FROM tinyints
SELECT i::UBIGINT FROM tinyints
SELECT s::TINYINT FROM strings
SELECT i::DECIMAL(3,1)::TINYINT FROM tinyints ORDER BY i
SELECT i::DECIMAL(9,7)::TINYINT FROM tinyints ORDER BY i
SELECT i::DECIMAL(18,16)::TINYINT FROM tinyints ORDER BY i
SELECT i::UTINYINT FROM ubigints ORDER BY i
SELECT i::USMALLINT FROM ubigints ORDER BY i
SELECT i::UINTEGER FROM ubigints ORDER BY i
SELECT i::TINYINT FROM ubigints ORDER BY i
SELECT i::SMALLINT FROM ubigints ORDER BY i
SELECT i::INTEGER FROM ubigints ORDER BY i
SELECT i::BIGINT FROM ubigints ORDER BY i
SELECT s::UBIGINT FROM strings
SELECT i::TINYINT FROM uhugeints
SELECT i::SMALLINT FROM uhugeints
SELECT i::INTEGER FROM uhugeints
SELECT i::BIGINT FROM uhugeints
SELECT i::UTINYINT FROM uhugeints ORDER BY i
SELECT i::USMALLINT FROM uhugeints ORDER BY i
SELECT i::UINTEGER FROM uhugeints ORDER BY i
SELECT i::UBIGINT FROM uhugeints ORDER BY i
SELECT i::UTINYINT FROM uintegers ORDER BY i
SELECT i::USMALLINT FROM uintegers ORDER BY i
SELECT i::TINYINT FROM uintegers ORDER BY i
SELECT i::SMALLINT FROM uintegers ORDER BY i
SELECT i::INTEGER FROM uintegers ORDER BY i
SELECT s::UINTEGER FROM strings
SELECT i::DECIMAL(3,0)::UINTEGER FROM uintegers ORDER BY i
SELECT i::DECIMAL(9,0)::UINTEGER FROM uintegers ORDER BY i
SELECT i::UTINYINT FROM usmallints ORDER BY i
SELECT i::TINYINT FROM usmallints ORDER BY i
SELECT i::SMALLINT FROM usmallints ORDER BY i
SELECT s::USMALLINT FROM strings
SELECT i::DECIMAL(3,0)::USMALLINT FROM usmallints ORDER BY i
SELECT i::DECIMAL(9,5)::USMALLINT FROM usmallints ORDER BY i
SELECT i::DECIMAL(18,14)::USMALLINT FROM usmallints ORDER BY i
SELECT i::DECIMAL(38,34)::USMALLINT FROM usmallints ORDER BY i
SELECT i::TINYINT FROM utinyints ORDER BY i
SELECT s::UTINYINT FROM strings
SELECT i::DECIMAL(3,1)::UTINYINT FROM utinyints ORDER BY i
SELECT i::DECIMAL(9,7)::UTINYINT FROM utinyints ORDER BY i
SELECT i::DECIMAL(18,16)::UTINYINT FROM utinyints ORDER BY i
SELECT i::DECIMAL(38,36)::UTINYINT FROM utinyints ORDER BY i
select [(SELECT min from hugeint_limits), 1.00005]
select [(SELECT max from hugeint_limits), 1.00005]
SELECT MAP([1, 2, 'hi'::VARCHAR], [1.0, 2.1, 4.9])::MAP(VARCHAR, TINYINT)
SELECT MAP(['x', 'y'], [i] ) FROM ints
SELECT MAP(keys, values) FROM MAP_input
SELECT MAP(['x', 'y'], i) FROM align_tbl
SELECT MAP(['x', 'y', '1', '2', '3', '4'], i) FROM align_tbl
SELECT MAP(i, ['x', 'y']) FROM align_tbl
SELECT DISTINCT MAP { * : ? IN ( SELECT TRUE ) }
SELECT make_type('STRUCT', make_type('INTEGER'), b := make_type('VARCHAR'))
CREATE TABLE T (v TYPE)
select sum(f) from floats_doubles where f > 0
select sum(d) from floats_doubles where d > 0
INSERT INTO a VALUES (1, [4, 5])
INSERT INTO a VALUES (1, [[4, 5]])
SELECT REPEAT ( '[{"a":' , 100000 )::INT[]
SELECT UNNEST(UNNEST([[1, 2, 3]]))
SELECT UNNEST()
SELECT UNNEST([1, 2, 3], 'hello')
SELECT UNNEST([1, 2, 3], recursive := 'hello')
SELECT UNNEST([1, 2, 3], rec := true)
SELECT UNNEST([1, 2, 3], max_depth := 0)
select 42 having unnest([1,2,3])
select row_number() over () qualify unnest([1,2,3])
SELECT * FROM UNNEST((SELECT ARRAY[1,2,3] UNION ALL SELECT ARRAY[1,2,3]))
SELECT i FROM UNNEST(NULL) AS tbl(i)
SELECT i FROM UNNEST(1) AS tbl(i)
SELECT i FROM UNNEST([1, 2], [3, 4]) AS tbl(i)
select {'a': 'lalala'}::VARIANT::STRUCT(a BOOLEAN)
select {'b': 42, 'a': 'lalala', 'c': {'a': 'test'}}::VARIANT::STRUCT(a BOOLEAN)
select [ 42::UNION(a INTEGER, b BOOLEAN, c VARIANT)::VARIANT, {'a': 21, 'b': false}::VARIANT::UNION(a INTEGER, b BOOLEAN, c VARIANT)::VARIANT, {'hello': 'world'}, 'test'::VARIANT::UNION(a INTEGER, b BOOLEAN, c VARIANT)::VARIANT, ]::VARIANT
select v::int from (values ({'a': 42}::variant)) t(v)
select cast(json('["1","x"]')::VARIANT as INTEGER[2])
insert into union_tbl VALUES ({tag: '0', a: true, b: null, c: null})
insert into union_tbl VALUES ({tag: 0::UINT8, a: true, b: null::INTEGER, d: null::TINYINT})
insert into union_tbl VALUES ({tag: 0::UINT8, a: 1, b: null::INTEGER, c: null::TINYINT})
insert into union_tbl VALUES ({tag: 4::UINT8, a: true, b: null::INTEGER, c: null::TINYINT})
insert into union_tbl VALUES( {tag: 1::UINT8, a: NULL::BOOLEAN, b: 32412, c: 123::TINYINT} )
CREATE TABLE tbl(a UNION(b INT, b INT))
CREATE TABLE tbl(a UNION(b INT, B INT))
INSERT INTO tbl VALUES (1), (2), (3)
INSERT INTO tbl VALUES (union_value(b := 3)), (union_value(a := 4)), (union_value(b := 5))
INSERT INTO tbl2 VALUES ({'foo': 'bar'}), ({'foo': 'baz'})
INSERT INTO tbl2 VALUES (union_value(b := {'foo': 'bar'})), (union_value(c := {'foo': 'baz'})), (union_value(d := {'foo': 'qux'}))
SELECT u::int FROM tbl1
SELECT u::UNION(i SMALLINT, v VARCHAR) FROM tbl4
SELECT u::UNION(i SMALLINT, b INT) FROM tbl4
SELECT union_tag(1::INTEGER::UNION(lu UNION(f1 VARCHAR, t2 BIGINT), ru UNION(t2 BIGINT, f3 TINYINT)))
SELECT union_extract(1, 'b')
CREATE TABLE tbl1 (u UNION())
SELECT union_tag(1)
EXECUTE p2(1)
INSERT INTO tbl VALUES (union_value())
INSERT INTO tbl VALUES (union_value(num := 1, other := 2))
INSERT INTO tbl VALUES (union_value(key := 1))
select 1797693134862315708145274237317043567980705675258449965989174768031572607800285387605895586327668781715404589535143824642343213268894641827684675467035375169860499105765512820762454900903893289440758685084551339423045832369032229481658085593321233482747978262041447231687381771809192998812504040261841248583700::bignum::double
select '1797693134862315708145274237317043567980705675258449965989174768031572607800285387605895586327668781715404589535143824642343213268894641827684675467035375169860499105765512820762454900903893289440758685084551339423045832369032229481658085593321233482747978262041447231687381771809192998812504040261841248583700'::bignum::double
select bignum + '-1'::bignum from test_all_types(use_large_bignum = true) limit 1
select bignum + '1'::bignum from test_all_types(use_large_bignum = true) limit 1 offset 1
select sum(bignum) from bignum_underflow
select sum(bignum) from bignum_overflow
select 1797693134862315708145274237317043567980705675258449965989174768031572607800285387605895586327668781715404589535143824642343213268894641827684675467035375169860499105765512820762454900903893289440758685084551339423045832369032229481658085593321233482747978262041447231687381771809192998812504040261841248583700::bignum
select '-1e310'::double::bignum
select '+-0'::BIGNUM
select '-+0'::BIGNUM
select '-'::BIGNUM
select '00-0010'::BIGNUM
select ''::BIGNUM
select 'bla'::BIGNUM
select '1000bla'::BIGNUM
select '1000.bla'::BIGNUM
SELECT * FROM UNNEST(array_value(1, 2, 3))
SELECT array_value(1, 2, 3)::INT[2]
SELECT array_value(1, 2, 3)::INT[4]
SELECT [1, 2, 3]::BLOB[3]
SELECT [1,2,3]::INT[2]
SELECT [[1, 2, 3], [4, 5, 6, 7]]::INT[3][2]
SELECT (['2', 'abc', '3']::VARCHAR[3])::INT[]
SELECT ([1,2,3]::INT[3])::INT
CREATE TABLE t1(a INT, b INT[0])
CREATE TABLE t1(a INT, b INT[4294967299])
CREATE TABLE t1(a INT, b INT[2147483647])
SELECT array_value()
CREATE TABLE t1(a INT, b INT[-1])
CREATE TABLE t1(a INT, b INT['foobar'])
SELECT ([1,2,3]::INTEGER[3])::INTEGER[0]
SELECT ([1,2,3]::INTEGER[3])::INTEGER[-1]
SELECT CAST(array_value(1,2) as INTEGER[3])
SELECT CAST(x as INT[2][2]) FROM (VALUES ([[1,2],[3,4]]), ([[5,6],[7,8,9]])) AS t(x)
SELECT CAST(x as INT[2][2]) FROM (VALUES ([[1,2],[3,4]]), ([[5,6],[7,8],[9,10]])) AS t(x)
SELECT CAST('[1,2]' as INTEGER[3])
SELECT CAST('[NULL, [1,NULL,3], [1,2]]' as INTEGER[3][3])
SELECT CAST(test_vector AS INT[2]) AS a FROM test_vector_types(NULL::INTEGER[])
SELECT {'i': 3, 'i': 4}
SELECT {}
SELECT STRUCT_PACK() FROM struct_data
SELECT STRUCT_PACK(e+1) FROM struct_data
SELECT STRUCT_PACK(a := e, a := g) FROM struct_data
SELECT STRUCT_PACK(e, e) FROM struct_data
SELECT STRUCT_EXTRACT(e, 'e') FROM struct_data
SELECT STRUCT_EXTRACT(e) FROM struct_data
SELECT STRUCT_EXTRACT('e') FROM struct_data
SELECT STRUCT_EXTRACT() FROM struct_data
select struct_keys(ROW(1, 2))
select struct_keys(42)
select struct_keys(['a', 'b'])
select struct_values(42)
select struct_values(['a', 'b'])
SELECT MAP(list_value(NULL, NULL, NULL, NULL, NULL), list_value(10, 9, 10, 11, 13))
SELECT MAP(list_value(1, NULL, 3), list_value(6, 5, 4))
SELECT MAP(list_value(1, 2, 3, 4, 1), list_value(10, 9, 8, 7, 6))
SELECT MAP(NULL)
SELECT MAP(a, b) FROM tbl
SELECT MAP(list_value(10), list_value())
SELECT MAP(10, 12)
SELECT MAP(list_value(10), list_value(10), list_value(10))
select map_concat()
select map_concat(map([], []))
SELECT MAP { NULL: 'a' || i } FROM range(5) t(i)
SELECT MAP( [ [1,2],[2,1],[3,1],[4,2],[4,2,0],[1,2] ], [ NULL,NULL,NULL,NULL,NULL,NULL ] )
SELECT MAP( [ [1,2],[2,1],[3,1],[4,2],[4,2,0],NULL ], [ NULL,NULL,NULL,NULL,NULL,NULL ] )
SELECT MAP( [ {'foo': True}, {'foo': False}, {'foo': NULL}, {'foo': True} ], [ 'n', 'o', 'p', 'e' ] )
SELECT MAP( [ {'foo': 0}, {'foo': 1}, NULL, {'foo': 2}, {'foo': 3} ], [ 'e', 'r', 'r', 'o', 'r' ] )
SELECT MAP( [ MAP([5],[4]), MAP([10],[2]), MAP([2,3],[3,2]), MAP([10],[3]), MAP([3,2], [2,3]), MAP([5],[4]) ], [ 0,1,2,3,4,5 ] )
SELECT MAP( [ MAP([5],[4]), MAP([10],[2]), MAP([2,3],[3,2]), NULL, MAP([3,2], [2,3]) ], [ 0,1,2,3,4 ] )
select m[NULL] from (select MAP(LIST_VALUE(1, 2, 3, 4,5, NULL),LIST_VALUE(10, 9, 8, 7,11,42)) as m) as T
select m[2] from (select MAP(LIST_VALUE(1, 2, 3, 4,2),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m['Jon Lajoie'] from (select MAP(LIST_VALUE('Jon Lajoie', 'Backstreet Boys', 'Tenacious D','Jon Lajoie' ),LIST_VALUE(10,9,10,11)) as m) as T
select m[0] from (select MAP(LIST_VALUE('Jon Lajoie', 'Backstreet Boys', 'Tenacious D' ),LIST_VALUE(10,9,10)) as m) as T
select m[1] from (select MAP(LIST_VALUE(1, 1, 1, 4),LIST_VALUE(10, 9, 8, 7)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE('Jon Lajoie', NULL, 'Tenacious D',NULL,NULL ),LIST_VALUE(10,9,10,11,13)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE(NULL, NULL, NULL,NULL,NULL ),LIST_VALUE(10,9,10,11,13)) as m) as T
select m[NULL] from (select MAP(LIST_VALUE(NULL, NULL, NULL,NULL, NULL),LIST_VALUE(NULL, NULL, NULL,NULL,NULL )) as m) as T
select m[NULL] from (select MAP(LIST_VALUE([2], [NULL], [3,0], [NULL,NULL],[5,4], NULL),LIST_VALUE(10, 9, 8, 7,11,42)) as m) as T
select m[2] from (select MAP(LIST_VALUE([2,2], [2], [3,3], [4,4,4],[2]),LIST_VALUE(10, 9, 8, 7,11)) as m) as T
select m[[1]] from (select MAP(LIST_VALUE([1], [1], [1], [4]),LIST_VALUE(10, 9, 8, 7)) as m) as T
create table string_key_dup as select MAP_FROM_ENTRIES(ARRAY[('a', 'x'), ('a', 'y')]) col
create table tinyint_key_dup as select MAP_FROM_ENTRIES(ARRAY[(123, 'x'), (123, 'y')]) col
create table smallint_key_dup as select MAP_FROM_ENTRIES(ARRAY[(123, 'x'), (123, 'y')]) col
create table integer_key_dup as select MAP_FROM_ENTRIES(ARRAY[(123, 'x'), (123, 'y')]) col
create table bigint_key_dup as select MAP_FROM_ENTRIES(ARRAY[(123, 'x'), (123, 'y')]) col
create table hugeint_key as select MAP_FROM_ENTRIES(ARRAY[(123, 'x'), (123, 'y')]) col
create table boolean_key_dup as select MAP_FROM_ENTRIES(ARRAY[(True, 'x'), (True, 'y')]) col
create table date_key_dup as select MAP_FROM_ENTRIES(ARRAY[('1992-09-20'::DATE, 'x'), ('1992-09-20'::DATE, 'y')]) col
SELECT map_from_entries()
SELECT map_from_entries(ARRAY[(1,2), (3,4)], ARRAY[(5,6), (7,8)])
SELECT map_from_entries(5)
SELECT map_from_entries(ARRAY[5,4,3])
select MAP_FROM_ENTRIES(ARRAY[(1, 'x', 'extra'), (2, 'y', 'extra')])
SELECT MAP_FROM_ENTRIES(ARRAY[(1, 'x'), (2, 'y', 'extra')])
SELECT MAP_FROM_ENTRIES(ARRAY[(NULL, 2), ([3,4], 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[([1,2], 2), ([1,2], 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[({'a':5, 'b':7}, 2), ({'a':5, 'b':7}, 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[(MAP([5,3,4], ['a', 'b', 'c']), 2), (MAP([5,3,4], ['a', 'b', 'c']), 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[([{'a':5, 'b':7}, {'a':5, 'b':8}], 2), ([{'a':5, 'b':7}, {'a':5, 'b':8}], 4)])
SELECT MAP_FROM_ENTRIES(ARRAY[NULL, (1, 'x'), NULL, (2, 'y')])
SELECT MAP_FROM_ENTRIES(ARRAY[(1, 'x'), (NULL, 'z'), (2, 'y')])
SELECT ARRAY[i, 'hello'] FROM generate_series(0,2) tbl(i) WHERE (ARRAY[i])[1] == 1
SELECT ARRAY[ARRAY[1], ARRAY['hello']]
SELECT ARRAY[[1, 2], [3, 4]]::BIGINT[]
SELECT UNNEST(UNNEST(ARRAY[[1, 2], [3, 4]]::VARCHAR[][]))
SELECT list_aggr([0, 1, 2, 3], 'arg_min', i) FROM range(1, 4) tbl(i)
SELECT list_aggr(list(i), 'quantile') FROM range(10) tbl(i)
SELECT list_aggr(list(i), 'min', 1) FROM range(10) tbl(i)
SELECT list_aggr(list(i), 'quantile', 0.5, 0.3, 0.5) FROM range(10) tbl(i)
SELECT list_aggr(list(i), 'quantile', i) FROM range(10) tbl(i)
SELECT LIST_EXTRACT(42, 1)
SELECT list_extract('1', 9223372036854775807)
SELECT list_extract('1', -9223372036854775808)
SELECT (1)[1:2]
SELECT '12345'[1:3:2]
SELECT ([1,2,3,4,5])[1:3:0]
SELECT a[start:stop:step] from err
SELECT ([1,2,3,4,5])[1:[NULL]:2]
SELECT ([1,2,3,4,5])[[NULL]:3:2]
SELECT ([1,2,3,4,5])[1:'a':2]
SELECT ([1,2,3,4,5])['a':3:2]
SELECT ([1,2,3,4,5])[1:[]:2]
SELECT SUM(UNNEST(le)) FROM ( SELECT g, LIST(e) le from list_data GROUP BY g ORDER BY g) xx
SELECT LIST(LIST(42))
SELECT UNNEST(UNNEST(LIST(42))
SELECT LIST()
SELECT LIST() FROM list_data
SELECT LIST(e, g) FROM list_data
SELECT g, UNNEST(l+1) u FROM (SELECT g, LIST(e) l FROM list_data GROUP BY g) u1
SELECT g, UNNEST(g) u FROM (SELECT g, LIST(e) l FROM list_data GROUP BY g) u1
SELECT * FROM (VALUES (LIST_VALUE(1, 2)), (LIST_VALUE()), (LIST_VALUE('a'))) lv(a)
SELECT CAST(LIST_VALUE(42) AS INTEGER)
SELECT LIST_VALUE(42) + 4
INSERT INTO timestamp VALUES ('blabla')
INSERT INTO timestamp VALUES ('1993-20-14 00:00:00')
INSERT INTO timestamp VALUES ('1993-08-99 00:00:00')
INSERT INTO timestamp VALUES ('1993-02-29 00:00:00')
INSERT INTO timestamp VALUES ('1900-02-29 00:00:00')
INSERT INTO timestamp VALUES ('02-02-1992 00:00:00')
INSERT INTO timestamp VALUES ('1900-1-1 59:59:23')
INSERT INTO timestamp VALUES ('1900a01a01 00:00:00')
select ts::TIME, tstz::TIME, dt::TIME FROM specials
select 'infinity'::TIME
select subtract( cast('infinity' as timestamp), timestamp '1970-01-01')
select subtract( timestamp '1970-01-01', cast('-infinity' as timestamp))
SELECT 'e'::TIMESTAMP
SELECT 'e'::DATE
SELECT 'i'::TIMESTAMP
SELECT 'i'::DATE
SELECT timestamp ' 2017-07-23 13:10:11 AA'
SELECT timestamp 'AA2017-07-23 13:10:11'
SELECT timestamp '2017-07-23A13:10:11'
SELECT SUM(t) FROM timestamp
SELECT t+t FROM timestamp
SELECT t*t FROM timestamp
SELECT t/t FROM timestamp
SELECT t%t FROM timestamp
select '90000-01-19 03:14:07.999999'::TIMESTAMP_US::TIMESTAMP_NS
SELECT TIMESTAMP_NS '2262-04-11 23:47:16.854775808'
select '290309-12-21 (BC) 12:59:59.999999'::timestamp
select '290309-12-22 (BC) 00:00:00'::timestamp - interval (1) microsecond
select '290309-12-22 (BC) 00:00:00'::timestamp - interval (1) second
select '290309-12-22 (BC) 00:00:00'::timestamp - interval (1) day
select '290309-12-22 (BC) 00:00:00'::timestamp - interval (1) month
select '290309-12-22 (BC) 00:00:00'::timestamp - interval (1) year
select timestamp '294247-01-10 04:00:54.775807'
select timestamp '294247-01-10 04:00:54.775808'
CREATE TABLE ts_precision(sec TIMESTAMP(10))
CREATE TABLE ts_precision(sec TIMESTAMP(99999))
CREATE TABLE ts_precision(sec TIMESTAMP(1, 1))
SELECT TIMESTAMP_NS '2262-04-11 23:47:16.854775807'
SELECT TIMESTAMP '2021-05-25 04:55:03.382494 EST'
SELECT 'abc �'::BYTEA
select 'ü'::blob
SELECT 1::BYTEA
SELECT 1.0::BYTEA
SELECT 1::tinyint::BYTEA
SELECT 1::smallint::BYTEA
SELECT 1::integer::BYTEA
SELECT 1::bigint::BYTEA
SELECT 1::decimal::BYTEA
SELECT '\\'::BLOB
SELECT '\\b12'::BLOB
SELECT 'ü'::BLOB
SELECT 251658240::HUGEINT * 19938419936773738093557105904205168640::HUGEINT
SELECT 1080863910568919040::HUGEINT * 4642275147320176030871715840::HUGEINT
SELECT 1080863910568919040::HUGEINT * 19938419936773738093557105904205168640::HUGEINT
SELECT 4642275147320176030871715840::HUGEINT * 1080863910568919040::HUGEINT
SELECT 4642275147320176030871715840::HUGEINT * 4642275147320176030871715840::HUGEINT
SELECT 4642275147320176030871715840::HUGEINT * 19938419936773738093557105904205168640::HUGEINT
SELECT 19938419936773738093557105904205168640::HUGEINT * 251658240::HUGEINT
SELECT 19938419936773738093557105904205168640::HUGEINT * 1080863910568919040::HUGEINT
SELECT SUM(170141183460469231731687303715884105727) FROM range(10)
SELECT SUM(x) FROM (VALUES (170141183460469231731687303715884105727), (170141183460469231731687303715884105727)) t(x)
SELECT AVG(170141183460469231731687303715884105727) FROM range(10)
SELECT AVG(x) FROM (VALUES (170141183460469231731687303715884105727), (170141183460469231731687303715884105727)) t(x)
SELECT CAST('170141183460469231731687303715884105728' AS HUGEINT)
SELECT CAST('170141183460469231731687303715884105728'::DOUBLE AS HUGEINT)
SELECT CAST('-170141183460469231731687303715884105729' AS HUGEINT)
SELECT CAST('-170141183460469231731687303715884105729'::DOUBLE AS HUGEINT)
SELECT '170141183460469231731687303715884105727'::HUGEINT + '170141183460469231731687303715884105727'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT + '10'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT - 10::HUGEINT + 11::HUGEINT
SELECT '-170141183460469231731687303715884105728'::HUGEINT + 10::HUGEINT - 11::HUGEINT
SELECT '-170141183460469231731687303715884105728'::HUGEINT - '170141183460469231731687303715884105727'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT - '-170141183460469231731687303715884105728'::HUGEINT
SELECT '170141183460469231731687303715884105727'::HUGEINT * 2::HUGEINT
SELECT '1701411834604692317'::HUGEINT * '131687303715884105727'::HUGEINT
SELECT -27::HUGEINT << 1
select 1::HUGEINT << 200
SELECT '1329227995784915872903807060280344576'::HUGEINT << 50
SELECT 27::HUGEINT << -1
SELECT 100::HUGEINT << '1329227995784915872903807060280344576'::HUGEINT
select 1::hugeint << 1000
select 1 << 170141183460469231731687303715884105727::hugeint
SELECT '1701411834604692317316873037158841057200'::HUGEINT
SELECT '-1701411834604692317316873037158841057200'::HUGEINT
SELECT '170141183460469231731687303715884105728'::HUGEINT
SELECT '-170141183460469231731687303715884105729'::HUGEINT
SELECT 1000::HUGEINT::TINYINT
SELECT 128::HUGEINT::TINYINT
SELECT -128::HUGEINT::TINYINT
SELECT 100000::HUGEINT::SMALLINT
select '170141183460469231731687303715884105735e0'::hugeint
select '1.7e39'::hugeint
select '2e38'::hugeint
select '0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001e5'::hugeint
select '1.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001e5'::hugeint
select '1.1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001e5'::hugeint
