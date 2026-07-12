# file: test/sql/order/issue_11936.test
# setup
CREATE TABLE test(col1 INT, col2 INT2[][][][][][])
# query
INSERT INTO test VALUES(1000000000, null), (1000000001, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (1, [[[[[[]]]]]])
CREATE TABLE test(col1 INT, col2 INT2[][][][][][])
SELECT col1, col2 FROM test ORDER BY col1 NULLS LAST, col2
# file: test/sql/order/limit_full_outer_join.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE integers2(k INTEGER, l INTEGER)
# query
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY ALL LIMIT 2
CREATE TABLE integers(i INTEGER, j INTEGER)
INSERT INTO integers VALUES (1, 1), (3, 3)
CREATE TABLE integers2(k INTEGER, l INTEGER)
INSERT INTO integers2 VALUES (1, 10), (2, 20)
SELECT COUNT(*) FROM (SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k LIMIT 2) tbl
# file: test/sql/order/order_by_all.test
# setup
CREATE TABLE integers(g integer, i integer)
# query
SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers UNION ALL SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers UNION SELECT * FROM integers ORDER BY ALL
SET default_null_order='nulls_first'
CREATE TABLE integers(g integer, i integer)
INSERT INTO integers values (0, 1), (0, 2), (1, 3), (1, NULL)
SELECT * FROM integers ORDER BY * DESC
SELECT * FROM integers ORDER BY * DESC NULLS LAST
# file: test/sql/order/order_by_internal_5293.test
# setup
create table t1 as from VALUES ('A', 1), ('B', 3), ('C', 12), ('A', 5), ('B', 8), ('C', 9), ('A', 10), ('B', 20), ('C', 3) t(a, b)
# query
from t1 order by a
create table t1 as from VALUES ('A', 1), ('B', 3), ('C', 12), ('A', 5), ('B', 8), ('C', 9), ('A', 10), ('B', 20), ('C', 3) t(a, b)
PRAGMA disabled_optimizers='compressed_materialization'
# file: test/sql/order/test_limit.test
# setup
CREATE SEQUENCE seq START 3
CREATE SEQUENCE of_seq START 1
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE TABLE test2 (a STRING)
CREATE TABLE integers(i INTEGER)
CREATE OR REPLACE TABLE t AS SELECT range x FROM range(10)
create table t0(c0 int)
# query
SELECT * FROM t0 ORDER BY ALL OFFSET (SELECT DISTINCT 6.5 FROM (SELECT 1) t1(c0) UNION ALL SELECT 3)
CREATE TABLE test (a INTEGER, b INTEGER)
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22)
SELECT a FROM test LIMIT 1
SELECT a FROM test LIMIT 1.25
SELECT a FROM test LIMIT 2-1
# file: test/sql/order/test_order_unnest.test
# setup
CREATE TABLE tbl_structs AS SELECT {'a': 2.0, 'b': 'hello', 'c': [1, 2]} AS s1, 1::BIGINT AS i, {'k': 1::TINYINT, 'j': 0::BOOL} AS s2
# query
CREATE TABLE tbl_structs AS SELECT {'a': 2.0, 'b': 'hello', 'c': [1, 2]} AS s1, 1::BIGINT AS i, {'k': 1::TINYINT, 'j': 0::BOOL} AS s2
INSERT INTO tbl_structs VALUES ( {'a': 1.0, 'b': 'yay', 'c': [10, 20]}, 42, {'k': 2, 'j': 1})
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY id
SELECT s1, s1.a FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY i
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY 2 DESC
# file: test/sql/settings/allowed_directories.test
# setup
CREATE TABLE integers(i INT)
CREATE TABLE a1.integers(i INTEGER)
# query
FROM integers
RESET allowed_directories
SET enable_external_access=false
SET allowed_directories=[]
CREATE TABLE integers(i INT)
COPY (SELECT 42 i) TO 'permission_test.csv' (FORMAT csv)
# file: test/sql/setops/test_union_all_by_name.test
# setup
CREATE TABLE t1 (x INT, y INT)
CREATE TABLE t2 (y INT, z INT)
CREATE TABLE new_table AS (SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2)
# query
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2
SELECT t1.x FROM t1 UNION ALL BY NAME SELECT x FROM t1 ORDER BY t1.x
SELECT x FROM t1 UNION ALL BY NAME SELECT x FROM t1 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT x FROM t1) UNION ALL BY NAME SELECT 5 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT y FROM t1) UNION ALL BY NAME SELECT 5 ORDER BY y
SELECT x AS a FROM t1 UNION ALL BY NAME SELECT x AS b FROM t1 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT y FROM t1) UNION ALL BY NAME (SELECT z FROM t2 UNION ALL SELECT y FROM t2) ORDER BY y, z
(SELECT x FROM t1 UNION ALL SELECT y FROM t1) UNION ALL BY NAME (SELECT z FROM t2 UNION ALL SELECT y FROM t2) ORDER BY t1.y
SELECT 1 UNION ALL BY NAME SELECT * FROM range(2, 100) UNION ALL BY NAME SELECT 999 LIMIT 5
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 ORDER BY z DESC
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 ORDER BY y
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 ORDER BY 3
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 ORDER BY 4
(SELECT x FROM t1 ORDER BY y) UNION ALL BY NAME (SELECT y FROM t2 ORDER BY z) ORDER BY x DESC
SELECT 1 UNION ALL BY NAME SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2
SELECT 1, 2 FROM t1 UNION SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2
SELECT 1, 2 FROM t1 UNION (SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2)
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 INTERSECT SELECT 2, 2 FROM t1
SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2 EXCEPT SELECT 2, 2 FROM t1
(SELECT x, y FROM t1 UNION ALL BY NAME SELECT y, z FROM t2) EXCEPT SELECT NULL, 2, 2 FROM t1
SELECT x, x FROM t1 UNION ALL BY NAME SELECT y FROM t2
SELECT x, x as a FROM t1 UNION ALL BY NAME SELECT y FROM t2
SELECT x as a FROM t1 UNION ALL BY NAME SELECT x FROM t1
SELECT DISTINCT ON(x) x FROM (SELECT 1 as x UNION ALL BY NAME SELECT '1' as x)
SELECT DISTINCT ON(x) x FROM (SELECT 1 as x UNION ALL BY NAME SELECT 1.1 as x)
# file: test/sql/setops/test_union_by_name.test
# setup
CREATE TABLE t1 (x INT, y INT)
CREATE TABLE t2 (y INT, z INT)
# query
SELECT t1.x FROM t1 UNION BY NAME SELECT x FROM t1 ORDER BY t1.x
SELECT x FROM t1 UNION BY NAME SELECT x FROM t1 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT x FROM t1) UNION BY NAME SELECT 5 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT y FROM t1) UNION BY NAME SELECT 5 ORDER BY y
SELECT x AS a FROM t1 UNION BY NAME SELECT x AS b FROM t1 ORDER BY t1.x
(SELECT x FROM t1 UNION ALL SELECT y FROM t1) UNION BY NAME (SELECT z FROM t2 UNION ALL SELECT y FROM t2) ORDER BY y, z
SELECT 1 UNION BY NAME SELECT * FROM range(2, 100) UNION BY NAME SELECT 999 ORDER BY #2, #1 LIMIT 5
SELECT x, y FROM t1 UNION BY NAME SELECT y, z FROM t2 ORDER BY y
SELECT x, y FROM t1 UNION BY NAME SELECT y, z FROM t2 ORDER BY 3, 1
SELECT x, y FROM t1 UNION BY NAME SELECT y, z FROM t2 ORDER BY 4
(SELECT 1 UNION BY NAME SELECT x, y FROM t1) UNION BY NAME SELECT y, z FROM t2 ORDER BY ALL
SELECT x, y FROM t1 UNION BY NAME (SELECT y, z FROM t2 INTERSECT SELECT 2, 2 FROM t1 ORDER BY #1) ORDER BY #1
(SELECT x, y FROM t1 UNION BY NAME SELECT y, z FROM t2 ORDER BY #1) EXCEPT SELECT NULL, 2, 2 FROM t1 ORDER BY #1
SELECT x, x as a FROM t1 UNION BY NAME SELECT y FROM t2 ORDER BY #1, #3
SELECT x as a FROM t1 UNION BY NAME SELECT x FROM t1 ORDER BY #1, #2
select '0' as c union all by name select 0 as c
select {'a': '0'} as c union all by name select {'a': 0} as c
SELECT {'a': 'hello'} AS c UNION ALL BY NAME SELECT {'b': 'hello'} AS c
SELECT {'a': 'hello'} AS c UNION ALL BY NAME SELECT {'a': 'hello', 'b': 'world'} AS c
SELECT [{'a': 42}, {'b': 84}]
# file: test/sql/parallelism/intraquery/test_parallel_nested_aggregates.test
# setup
create table t as select range a, range%10 b from range(100000)
# query
select min([-a, 1, a]), max([-a, 1, a]) from t group by b%2
select min({'i': a}), max({'i': a}) from t group by b%2 order by all
select min({'i': a, 'j': a % 2}), max({'i': a, 'j': a % 2}) from t group by b%2
# file: test/sql/pivot/optional_pivots.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
PIVOT Cities USING SUM(Population)
PIVOT Cities USING SUM(Population) GROUP BY Country
PIVOT Cities GROUP BY Country
PIVOT Cities ON Year GROUP BY Country
PIVOT (SELECT Country, Year FROM Cities) ON Year
# file: test/sql/pivot/pivot_15141.test
# setup
create table p (col1 timestamp, col2 int)
# query
pivot p using sum (col2) group by col1 order by col1
# file: test/sql/pivot/pivot_6390.test
# setup
CREATE TABLE cpb_tbl AS WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) FROM CPB
# query
pivot cpb_tbl on nf using sum(jg)group by cpdh
WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf IN (2010, 2017, 2018, 2022) using sum(jg)group by cpdh
WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf using sum(jg)group by cpdh
WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) SELECT * FROM (pivot CPB on nf using sum(jg)group by cpdh)
WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) from CPB pivot (sum(jg) for nf in (2010, 2017, 2018, 2022) group by cpdh)
WITH CPB AS (SELECT 42) SELECT * FROM ( WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf using sum(jg) group by cpdh)
WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf IN (2010, 2017, 2018, 2022) using sum(jg)group by cpdh
WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf using sum(jg)group by cpdh
WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) SELECT * FROM (pivot CPB on nf using sum(jg)group by cpdh)
WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) from CPB pivot (sum(jg) for nf in (2010, 2017, 2018, 2022) group by cpdh)
WITH CPB AS (SELECT 42) SELECT * FROM ( WITH CPB(CPDH,NF,JG) AS MATERIALIZED ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) pivot CPB on nf using sum(jg) group by cpdh)
# file: test/sql/pivot/pivot_bigquery.test
# setup
CREATE OR REPLACE TABLE Produce AS SELECT 'Kale' as product, 51 as Q1, 23 as Q2, 45 as Q3, 3 as Q4 UNION ALL SELECT 'Apple', 77, 0, 25, 2
# query
SELECT * FROM Produce PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) total_sales, COUNT(*) num_records FOR quarter IN ('Q1', 'Q2')) ORDER BY ALL
SELECT * FROM Produce UNPIVOT(sales FOR quarter IN (Q1, Q2, Q3, Q4)) ORDER BY ALL
SELECT product, first_half_sales, second_half_sales, semesters FROM Produce UNPIVOT( (first_half_sales, second_half_sales) FOR semesters IN ((Q1, Q2) AS 'semester_1', (Q3, Q4) AS 'semester_2'))
# file: test/sql/pivot/pivot_case_insensitive.test
# query
FROM Cities PIVOT ( array_agg(id) FOR name IN ('test','Test') )
FROM Cities PIVOT ( array_agg(id), sum(id) FOR name IN ('test','Test') )
# file: test/sql/pivot/pivot_databricks.test
# setup
CREATE OR REPLACE TEMPORARY VIEW sales(location, year, q1, q2, q3, q4) AS VALUES ('Toronto' , 2020, 100 , 80 , 70, 150), ('San Francisco', 2020, NULL, 20 , 50, 60), ('Toronto' , 2021, 110 , 90 , 80, 170), ('San Francisco', 2021, 70 , 120, 85, 105)
CREATE OR REPLACE TEMPORARY VIEW oncall (year, week, area , name1 , email1 , phone1 , name2 , email2 , phone2) AS VALUES (2022, 1 , 'frontend', 'Freddy', 'fred@alwaysup.org' , 15551234567, 'Fanny' , 'fanny@lwaysup.org' , 15552345678), (2022, 1 , 'backend' , 'Boris' , 'boris@alwaysup.org', 15553456789, 'Boomer', 'boomer@lwaysup.org', 15554567890), (2022, 2 , 'frontend', 'Franky', 'frank@lwaysup.org' , 15555678901, 'Fin' , 'fin@alwaysup.org' , 15556789012), (2022, 2 , 'backend' , 'Bonny' , 'bonny@alwaysup.org', 15557890123, 'Bea' , 'bea@alwaysup.org' , 15558901234)
# query
SELECT year, region, q1, q2, q3, q4 FROM sales PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region, too_many_names) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east', 'west') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT * FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'east') AS q1_east_2))
SELECT year, q1, q2, q3, q4 FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_total, q1_avg, q2_total, q2_avg, q3_total, q3_avg, q4_total, q4_avg FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) AS total, avg(sales) AS avg FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT * FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales), avg(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT * FROM sales UNPIVOT INCLUDE NULLS (sales FOR quarter IN (q1 AS "Jan-Mar", q2 AS "Apr-Jun", q3 AS "Jul-Sep", q4 AS "Oct-Dec"))
SELECT * FROM oncall UNPIVOT ((name, email, phone) FOR precedence IN ((name1, email1, phone1) AS primary, (name2, email2, phone2) AS secondary))
# file: test/sql/pivot/pivot_empty.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
PIVOT Cities ON Country USING SUM(Population)
PIVOT Cities ON Country, Name USING SUM(Population)
PIVOT Cities ON Country IN ('xx') USING SUM(Population)
PIVOT Cities ON (Country, Name) IN ('xx') USING SUM(Population)
PIVOT Cities ON Country IN ('xx', 'yy') USING SUM(Population)
# file: test/sql/pivot/pivot_enum.test
# setup
CREATE TYPE unique_months AS ENUM (SELECT DISTINCT month FROM monthly_sales ORDER BY CASE month WHEN 'JAN' THEN 1 WHEN 'FEB' THEN 2 WHEN 'MAR' THEN 3 ELSE 4 END)
CREATE TYPE not_an_enum AS VARCHAR
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_months) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_monthsx) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN not_an_enum) AS p ORDER BY EMPID
# file: test/sql/pivot/pivot_errors.test
# setup
CREATE TABLE test(i INT, j VARCHAR)
# query
PIVOT test ON j IN ('a', 'b') USING SUM(test.i)
PIVOT test ON j IN ('a', 'b') USING get_current_timestamp()
PIVOT test ON j IN ('a', 'b') USING sum(41) over ()
PIVOT test ON j IN ('a', 'b') USING sum(sum(41) over ())
FROM tbl PIVOT (c FOR IN enum_val)
# file: test/sql/pivot/pivot_expressions.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
PIVOT Cities ON Country || '_' || Name USING SUM(Population) GROUP BY Year
PIVOT Cities ON (CASE WHEN Country='NL' THEN NULL ELSE Country END) USING SUM(Population) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING COALESCE(SUM(Population), 0) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population)::VARCHAR GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + 42 GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + COUNT(*) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + Population GROUP BY Year
PIVOT Cities ON min(Country) over () USING SUM(Population) GROUP BY Year
PIVOT Cities ON min(Country) USING SUM(Population) GROUP BY Year
PIVOT Cities ON NULL USING SUM(Population) GROUP BY Year
PIVOT Cities ON 'hello world' USING SUM(Population) GROUP BY Year
PIVOT Cities ON (SELECT COUNTRY) USING SUM(Population) GROUP BY Year
# file: test/sql/pivot/pivot_generated.test
# setup
CREATE TABLE Product(DaysToManufacture int, StandardCost int GENERATED ALWAYS AS (DaysToManufacture * 5))
# query
SELECT 'AverageCost' AS Cost_Sorted_By_Production_Days, "0", "1", "2", "3", "4" FROM ( SELECT DaysToManufacture, StandardCost FROM Product ) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable
# file: test/sql/pivot/pivot_in_boolean.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
pivot cities on (Country='NL') using avg(Population) group by name
pivot cities on (Country='NL') in (false, true) using avg(Population) group by name
# file: test/sql/pivot/pivot_in_subquery.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
PIVOT Cities ON Year IN (SELECT Year FROM Cities ORDER BY Year DESC) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT YEAR FROM (SELECT Year, SUM(POPULATION) AS popsum FROM Cities GROUP BY Year ORDER BY popsum DESC)) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT '2010' UNION ALL SELECT '2000' UNION ALL SELECT '2020') USING SUM(Population)
PIVOT Cities ON Year IN (SELECT xx FROM Cities) USING SUM(Population)
# file: test/sql/pivot/pivot_operator_expression.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
PIVOT monthly_sales ON MONTH USING COALESCE(SUM(AMOUNT), 0)
SELECT mode(column_type) FROM (DESCRIBE PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)::INTEGER)
# file: test/sql/pivot/pivot_prepare.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
PREPARE v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount + ?) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR')) AS p ORDER BY EMPID
PREPARE v2 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT + ?)
PREPARE v3 AS PIVOT (SELECT empid, amount + ? AS amount, month FROM monthly_sales) ON MONTH USING SUM(AMOUNT)
# file: test/sql/pivot/pivot_star.test
# setup
CREATE TABLE t(id INT, jan INT, feb INT)
CREATE VIEW v AS PIVOT t ON id IN (CASE WHEN true THEN 'a' END) USING (SUM(feb))
CREATE VIEW poison_view AS SELECT * FROM t UNPIVOT (val FOR col IN (*))
CREATE VIEW expr_view AS SELECT * FROM t UNPIVOT (val FOR col IN (1+2+id))
# query
CREATE VIEW poison_view AS SELECT * FROM t UNPIVOT (val FOR col IN (*))
CREATE VIEW expr_view AS SELECT * FROM t UNPIVOT (val FOR col IN (1+2+id))
CREATE VIEW v AS PIVOT t ON id IN (CASE WHEN true THEN 'a' END) USING (SUM(feb))
# file: test/sql/pivot/pivot_storage.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
CREATE VIEW v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
CREATE MACRO pivot_macro(val) as TABLE SELECT * FROM monthly_sales PIVOT(SUM(amount + val) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
# query
CREATE VIEW v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
CREATE MACRO pivot_macro(val) as TABLE SELECT * FROM monthly_sales PIVOT(SUM(amount + val) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
FROM v1
FROM pivot_macro(1)
# file: test/sql/pivot/pivot_struct_aggregate.test
# setup
create table donnees_csv as select {'year': i::varchar, 'month': i::varchar} AS donnee, i%5 as variable_id, i%10 id_niv from range(1000) t(i)
# query
create table donnees_csv as select {'year': i::varchar, 'month': i::varchar} AS donnee, i%5 as variable_id, i%10 id_niv from range(1000) t(i)
pivot donnees_csv on variable_id using first(donnee) group by id_niv order by all
# file: test/sql/pivot/pivot_subquery.test
# setup
CREATE OR REPLACE TABLE sales(empid INT, amount INT, d DATE)
# query
PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT) ORDER BY ALL
PIVOT (PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)) ON empid USING SUM(COALESCE("2000_1",0) + COALESCE("2000_2",0) + COALESCE("2000_3",0) + COALESCE("2001_1",0) + COALESCE("2001_2",0) + COALESCE("2001_3",0))
CREATE VIEW pivot_view AS PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)
CREATE MACRO xt2(a) as TABLE PIVOT sales ON d USING SUM(amount)
CREATE MACRO xt2(a) as (PIVOT sales ON d USING SUM(amount))
# file: test/sql/pivot/test_multi_pivot.test
# setup
CREATE OR REPLACE TABLE sales(empid INT, amount INT, month TEXT, year INT)
# query
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount + year) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') amount IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) empid IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) ) AS p ORDER BY EMPID
# file: test/sql/pivot/test_pivot.test
# setup
CREATE TABLE Product(DaysToManufacture int, StandardCost int)
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount+1) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(COUNT(*) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC') GROUP BY empid) AS p ORDER BY EMPID
SELECT empid, January, February, March, April FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN' AS January, 'FEB' AS February, 'MAR' AS March, 'APR' AS April)) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (NULL, 'JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (NULL, 'JAN', 'FEB', 'MAR', 'APR')) AS p UNPIVOT INCLUDE NULLS(amount FOR MONTH IN ("NULL", JAN, FEB, MAR, APR)) ORDER BY ALL
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (NULL, 'JAN', 'FEB', 'MAR', 'APR')) AS p UNPIVOT EXCLUDE NULLS(amount FOR MONTH IN ("NULL", JAN, FEB, MAR, APR)) ORDER BY ALL
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (NULL, 'JAN', 'FEB', 'MAR', 'APR')) AS p UNPIVOT EXCLUDE NULLS(amount FOR MONTH IN ("NULL", JAN, FEB, MAR, APR)) ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'JAN')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(COS(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + (SELECT 42)) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + row_number() over ()) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTHx IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ()) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (*)) AS p ORDER BY EMPID
FROM ( SELECT DaysToManufacture, StandardCost FROM Product ) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN ('zz') ) AS PivotTable
# file: test/sql/pivot/test_pivot_duplicate_aggregates.test
# query
PIVOT (FROM range(21)) ON range USING sum(range), sum(range)
SELECT COUNT(*) FROM (PIVOT (FROM range(21)) ON range USING sum(range), sum(range))
PIVOT (FROM range(20)) ON range USING sum(range), sum(range)
PIVOT (FROM range(10)) ON range USING sum(range), sum(range)
PIVOT (FROM range(10)) ON range USING sum(range), sum(range), sum(range)
SELECT COUNT(*) FROM (PIVOT (FROM range(10)) ON range USING sum(range), sum(range), sum(range))
PIVOT (FROM range(21)) ON range USING sum(range), max(range), sum(range)
SELECT COUNT(*) FROM (PIVOT (FROM range(21)) ON range USING sum(range), max(range), sum(range))
PIVOT (FROM range(21)) ON range USING avg(range), avg(range)
PIVOT (FROM range(21)) ON range USING count(range), count(range)
SELECT * FROM ( PIVOT (FROM range(3)) ON range USING sum(range), sum(range) )
SELECT "0_sum(""range"")", "0_sum(""range"")_1" FROM ( PIVOT (FROM range(21)) ON range USING sum(range), sum(range) )
SELECT "5_sum(""range"")", "5_sum(""range"")_1" FROM ( PIVOT (FROM range(21)) ON range USING sum(range), sum(range) )
SELECT "0_sum(""range"")", "0_max(""range"")", "0_sum(""range"")_1" FROM ( PIVOT (FROM range(21)) ON range USING sum(range), max(range), sum(range) )
SELECT "5_sum(""range"")", "5_max(""range"")", "5_sum(""range"")_1" FROM ( PIVOT (FROM range(21)) ON range USING sum(range), max(range), sum(range) )
PIVOT ( SELECT range, range::VARCHAR as str_range FROM range(21) ) ON range USING max(str_range), max(str_range)
# file: test/sql/pivot/test_unpivot.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, dept TEXT, Jan INT, Feb INT, Mar INT, April INT)
# query
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) ORDER BY empid
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan AS January, feb AS February, mar AS March, april)) ORDER BY empid
SELECT p.id, p.type, p.m, p.vals FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) AS p(id, type, m, vals)
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR (month, month2) IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT(sales_jan_feb FOR month IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT((a, b, c) FOR month IN ((jan, feb), (mar, april)))
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), mar))
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, dec)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (empid, dept, jan, feb, mar, april))
UNPIVOT (SELECT * FROM monthly_sales) ON jan, feb, mar april INTO NAME month VALUE sales
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ()) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ('')) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(SUM(sales) FOR month IN (empid, dept, jan, feb, mar, april))
# file: test/sql/pivot/test_unpivot_stmt.test
# setup
CREATE TABLE t1(id BIGINT, "Sales (05/19/2020)" BIGINT, "Sales (06/03/2020)" BIGINT, "Sales (10/23/2020)" BIGINT)
# query
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM t1 UNPIVOT (sales FOR date IN ("Sales (05/19/2020)", "Sales (06/03/2020)", "Sales (10/23/2020)")) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON "Sales (05/19/2020)", "Sales (06/03/2020)", "Sales (10/23/2020)" INTO NAME date VALUE sales) ORDER BY ALL
SELECT * FROM (UNPIVOT t1 ON "Sales (05/19/2020)" AS "2020-05-19", "Sales (06/03/2020)" AS "2020-06-03", "Sales (10/23/2020)" AS "2020-10-23" INTO NAME date VALUE sales) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM t1 UNPIVOT (Sales FOR Date IN (COLUMNS('Sales.*'))) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON COLUMNS('Sales.*') INTO NAME date VALUE sales) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON * EXCLUDE (id) INTO NAME date VALUE sales) ORDER BY ALL
# file: test/sql/pivot/top_level_pivot_syntax.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL
# query
PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT))
PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid
PIVOT monthly_sales ON MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') USING SUM(AMOUNT) GROUP BY empid
PIVOT monthly_sales ON MONTH IN ('1-JAN', '2-FEB', '3-MAR') USING SUM(AMOUNT) GROUP BY empid
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)) ORDER BY ALL
PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY status) ORDER BY ALL
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') GROUP BY status) AS p ORDER BY 1
WITH pivoted_sales AS (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) SELECT * FROM pivoted_sales ORDER BY empid DESC
WITH pivoted_sales AS MATERIALIZED (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) SELECT * FROM pivoted_sales ORDER BY empid DESC
CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)
CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL
# file: test/sql/pivot/unpivot_expression.test
# query
unpivot (select 42 as col1, 'woot' as col2) on col1::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on COLUMNS(*)::VARCHAR
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR AS c, col2
select * from (select 42 as col1, 'woot' as col2) UNPIVOT ("value" FOR "name" IN (col1::VARCHAR, col2))
unpivot (select 42 as col1, 'woot' as col2) on (col1 + (SELECT col1))::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on random(), col2
unpivot (select 42 as col1, 'woot' as col2) on col1 + col2
unpivot (select 42 as col1, 'woot' as col2) on t.col1::VARCHAR, col2
# file: test/sql/pivot/unpivot_internal_names.test
# setup
CREATE TABLE unpivot_names(unpivot_names VARCHAR, unpivot_list VARCHAR, unpivot_list_2 VARCHAR, col1 INT, col2 INT, col3 INT)
# query
UNPIVOT unpivot_names ON COLUMNS('col*')
# file: test/sql/pivot/unpivot_no_columns.test
# setup
create table integers(i integer)
# query
unpivot integers on columns(* exclude (i))
# file: test/sql/pivot/unpivot_non_aligned_columns.test
# setup
CREATE TABLE test(id BIGINT, metric_1 VARCHAR, value_x VARCHAR, metric_2 VARCHAR, value_q VARCHAR, metric_3 VARCHAR, value_j VARCHAR)
# query
UNPIVOT test ON (metric_1, value_x), metric_2, metric_3
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUE metric_value
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUES metric_value, metric_type
# file: test/sql/pivot/unpivot_types.test
# query
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select 42) on columns(*))
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select {n : 1 }) on columns(*))
# file: test/sql/pivot/unpivot_unnamed_subquery.test
# query
unpivot (select cast(columns(*) as varchar) from (select 42 as col1, 'woot' as col2)) on columns(*)
# file: test/sql/returning/no_crash_when_no_returning_columns.test
# setup
CREATE TABLE v0 ( c1 INT )
# query
INSERT INTO v0 VALUES (1), (2), (3), (4) RETURNING * EXCLUDE c1
DELETE from v0 WHERE c1 = 0 RETURNING * EXCLUDE c1
UPDATE v0 SET c1 = 0 WHERE true RETURNING * EXCLUDE c1
Select * from v0 order by all
INSERT INTO v0 BY POSITION ( SELECT TRUE ) OFFSET 1 ROWS RETURNING v0 . * EXCLUDE c1
# file: test/sql/returning/returning_delete.test
# setup
CREATE SEQUENCE seq
CREATE TABLE table4 (a4 INTEGER, b4 INTEGER, c4 INTEGER)
CREATE TABLE table5 (a5 INTEGER, b5 INTEGER, c5 INTEGER)
CREATE TABLE table2 (a VARCHAR DEFAULT 'hello world', b INT)
CREATE TABLE table3 (a INTEGER DEFAULT nextval('seq'), b INTEGER)
CREATE TABLE table1 (a INTEGER DEFAULT -1, b INTEGER DEFAULT -2, c INTEGER DEFAULT -3)
CREATE TABLE test_optimized (id INTEGER, name VARCHAR, value DOUBLE)
CREATE TABLE test_where_returning (id INTEGER, amount DOUBLE, customer VARCHAR)
CREATE TABLE test_rowid_supported (id INTEGER, name VARCHAR)
CREATE TABLE test_rowid_generated ( id INTEGER, val INTEGER, computed INTEGER GENERATED ALWAYS AS (val * 2) VIRTUAL )
CREATE TABLE test_rowid_multi_gen ( id INTEGER, a INTEGER, b INTEGER, sum_ab INTEGER GENERATED ALWAYS AS (a + b) VIRTUAL, prod_ab INTEGER GENERATED ALWAYS AS (a * b) VIRTUAL )
CREATE TABLE test_rowid_gen_middle ( a INTEGER, gen INTEGER GENERATED ALWAYS AS (a + c) VIRTUAL, c INTEGER )
CREATE TABLE test_generated_virtual ( id INTEGER, base_value INTEGER, computed_value INTEGER GENERATED ALWAYS AS (base_value * 2) VIRTUAL )
CREATE TABLE test_generated_middle ( a INTEGER, gen INTEGER GENERATED ALWAYS AS (a + c) VIRTUAL, c INTEGER )
CREATE TABLE test_multi_generated ( x INTEGER, y INTEGER, sum_xy INTEGER GENERATED ALWAYS AS (x + y) VIRTUAL, prod_xy INTEGER GENERATED ALWAYS AS (x * y) VIRTUAL )
CREATE TABLE test_generated_varchar ( id INTEGER, name VARCHAR, greeting VARCHAR GENERATED ALWAYS AS ('Hello, ' || name || '!') VIRTUAL )
CREATE TABLE merge_gen_target(id INT, x INT, gen AS (x * 2))
CREATE TABLE merge_source(id INT)
CREATE TABLE merge_multi_gen( id INT, a INT, b INT, sum_ab INT GENERATED ALWAYS AS (a + b) VIRTUAL, prod_ab INT GENERATED ALWAYS AS (a * b) VIRTUAL )
CREATE TABLE merge_delete_source(id INT)
CREATE TABLE merge_rowid_target (id INT, val INT)
CREATE TABLE merge_rowid_source (id INT)
CREATE TABLE indexed_returning (id INT PRIMARY KEY, val VARCHAR, amount INT)
CREATE INDEX idx_amount ON indexed_returning(amount)
# query
DELETE FROM table2 WHERE b=3 RETURNING {'a': a, 'b': b}
# file: test/sql/returning/returning_delete_list.test
# setup
CREATE TABLE all_types("varchar" VARCHAR, nested_int_array INTEGER[][])
# query
INSERT INTO all_types VALUES('goo'||chr(0) || 'se' ,[[], [42, 999, NULL, NULL, -42], NULL, [], [42, 999, NULL, NULL, -42]])
# file: test/sql/returning/returning_insert.test
# setup
CREATE SEQUENCE seq
CREATE TABLE table1 (a INTEGER DEFAULT -1, b INTEGER DEFAULT -2, c INTEGER DEFAULT -3)
CREATE TABLE table2 (a VARCHAR DEFAULT 'hello world', b INT)
CREATE TABLE table3 (a INTEGER DEFAULT nextval('seq'), b INTEGER)
# query
INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c')
INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c') + 42
INSERT INTO table1 VALUES (1, 2, 3) RETURNING {'a':a, 'b':b, 'c':c}
INSERT INTO table1 VALUES (1, 2, 3) RETURNING [1, 2] IN (SELECT [a, b] from table1)
INSERT INTO table2(a, b) VALUES ('duckdb', 97) RETURNING {'a': a, 'b': b}
# file: test/sql/returning/returning_update.test
# setup
CREATE SEQUENCE seq
CREATE TABLE table1 (a INTEGER DEFAULT -1, b INTEGER DEFAULT -2, c INTEGER DEFAULT -3)
CREATE TABLE table5 (a5 INTEGER, b5 INTEGER, c5 INTEGER)
CREATE TABLE table2 (a VARCHAR DEFAULT 'hello world', b INT)
CREATE TABLE table3 (a INTEGER DEFAULT nextval('seq'), b INTEGER)
CREATE TABLE table4 (a INTEGER, b INTEGER, c INTEGER)
CREATE INDEX b_index ON table4(b)
# query
UPDATE table2 SET a='Mr.Duck', b=99 WHERE b=100 RETURNING {'a': a, 'b': b}
# file: test/sql/types/test_null_type.test
# setup
create table null_list (i "null"[])
create table null_struct (i struct(n "null"))
create table null_map (i map("null", "null"))
# query
insert into null_list values (null), ([null])
insert into null_struct values (null), ({n:null})
insert into null_map values (null), (map([null], [null]))
# file: test/sql/types/test_quotes.test
# setup
CREATE TYPE "coalesce" AS ENUM('a', 'b', 'c')
CREATE TYPE "select" AS ENUM('x', 'y', 'z')
CREATE TABLE t_defaults ( a UTINYINT DEFAULT CAST(1 AS UTINYINT), b USMALLINT DEFAULT CAST(1 AS USMALLINT), c UINTEGER DEFAULT CAST(1 AS UINTEGER), d UBIGINT DEFAULT CAST(1 AS UBIGINT), e UHUGEINT DEFAULT CAST(1 AS UHUGEINT), f TINYINT DEFAULT CAST(1 AS TINYINT), g HUGEINT DEFAULT CAST(1 AS HUGEINT), h DOUBLE DEFAULT CAST(1.0 AS DOUBLE), i SMALLINT DEFAULT CAST(1 AS SMALLINT), j INTEGER DEFAULT CAST(1 AS INTEGER) )
CREATE TABLE t_kw_udt ( a VARCHAR DEFAULT CAST('a' AS "coalesce"), b VARCHAR DEFAULT CAST('x' AS "select") )
CREATE TABLE t_kw_udt2 ( a VARCHAR DEFAULT CAST('a' AS "coalesce"), b VARCHAR DEFAULT CAST('x' AS "select") )
CREATE VIEW v_utiny AS SELECT CAST(1 AS UTINYINT)
CREATE VIEW v_usmall AS SELECT CAST(1 AS USMALLINT)
CREATE VIEW v_uint AS SELECT CAST(1 AS UINTEGER)
CREATE VIEW v_ubig AS SELECT CAST(1 AS UBIGINT)
CREATE VIEW v_uhuge AS SELECT CAST(1 AS UHUGEINT)
CREATE VIEW v_tiny AS SELECT CAST(1 AS TINYINT)
CREATE VIEW v_huge AS SELECT CAST(1 AS HUGEINT)
CREATE VIEW v_dbl AS SELECT CAST(1.0 AS DOUBLE)
CREATE VIEW v_small AS SELECT CAST(1 AS SMALLINT)
CREATE VIEW v_int AS SELECT CAST(1 AS INTEGER)
CREATE VIEW v_struct AS SELECT CAST({'x': 1, 'y': 2} AS STRUCT(x UTINYINT, y USMALLINT)) AS s
CREATE VIEW v_list AS SELECT CAST([1, 2] AS UTINYINT[]) AS l
# query
CREATE VIEW v_struct AS SELECT CAST({'x': 1, 'y': 2} AS STRUCT(x UTINYINT, y USMALLINT)) AS s
CREATE VIEW v_list AS SELECT CAST([1, 2] AS UTINYINT[]) AS l
# file: test/sql/types/decimal/decimal_automatic_cast.test
# setup
CREATE TABLE foo (my_struct STRUCT(my_double DOUBLE)[])
# query
SELECT [1.33, 10.0]
SELECT [0.1, 1.33, 10.0, 9999999.999999999]
SELECT [99999999999999999999999999999999999.9, 9.99999999999999999999999999999999999]
INSERT INTO foo VALUES ([{'my_double': 1.33}, {'my_double': 10.0}])
# file: test/sql/types/decimal/test_decimal_4106.test
# setup
CREATE TABLE from_values AS VALUES (1000000), (10.0000000005)
CREATE TABLE from_list AS SELECT [1000000, 10.0000000005]
# query
CREATE TABLE from_list AS SELECT [1000000, 10.0000000005]
# file: test/sql/types/decimal/test_decimal_from_string.test
# query
select map { 1: decimal 'b' }
# file: test/sql/types/alias/nested_alias.test
# setup
CREATE TYPE my_int AS INT
CREATE TYPE my_int_list AS my_int[]
# query
SELECT [42]::my_int_list
# file: test/sql/types/alias/test_alias_map.test
# setup
CREATE TYPE MAPPOINT AS MAP(INTEGER,INTEGER)
CREATE TABLE a(b MAPPOINT)
# query
INSERT INTO a VALUES (MAP([1], [2])), (MAP([1, 2, 3], [4, 5, 6]))
# file: test/sql/types/alias/test_alias_struct.test
# setup
CREATE TYPE POINT AS STRUCT(i INTEGER, j INTEGER)
CREATE TABLE a(b POINT)
# query
INSERT INTO a VALUES ({'i': 3, 'j': 4})
INSERT INTO a VALUES (ROW('hello', [1, 2]))
# file: test/sql/types/alias/test_alias_struct_nested_alias.test
# setup
CREATE TYPE foobar AS ENUM( 'Foo', 'Bar' )
CREATE TYPE top_nest AS STRUCT( foobar FOOBAR )
CREATE TABLE failing ( top_nest TOP_NEST )
# query
insert into failing VALUES ( {'foobar': 'Foo'} )
# file: test/sql/types/alias/test_alias_table.test
# setup
CREATE TYPE alias AS VARCHAR
CREATE TYPE intelligence AS VARCHAR
CREATE TYPE car_brand AS VARCHAR
CREATE TABLE pets ( name text, current_alias alias )
CREATE TABLE aliens ( name text, current_alias alias )
CREATE TABLE person ( name text, current_alias alias, last_year_alias alias, car car_brand )
# query
select count(*), current_alias from person group by current_alias order by all
# file: test/sql/types/bit/bit_issue_11211.test
# query
FROM ( SELECT ( 2::bit & 2::bit ) AS a, 2::bit AS b, (a = b) AS '(a = b)', ) SELECT a, b, a = b, "(a = b)"
# file: test/sql/types/interval/test_interval_comparison.test
# setup
CREATE TABLE issue14384(i INTERVAL)
# query
SELECT i FROM issue14384 ORDER BY ALL
SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) ORDER BY ALL
SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) WHERE (NOT (issue14384.i != issue14384.i)) ORDER BY ALL
# file: test/sql/types/enum/test_enum.test
# setup
CREATE TYPE bla AS ENUM ()
CREATE TYPE mood_2 AS ENUM ('sad','Sad','SAD')
CREATE TEMPORARY TYPE mood AS ENUM ('sad', 'ok', 'happy')
# query
select ['happy']::mood[]
select [NULL,'happy',NULL]::mood[]
select ['happy','ok','ok']::mood[]
select ['bla']::mood[]
select [1]::mood[]
select [NULL]::mood[]
select {'a': 'happy'::mood}
select {'a': 'happy'::mood, 'b': 'ok'::mood}
select {'a': 'happy'::mood, 'b': 1, 'c': 'ok'::mood}
select {'a': 'happy'::mood, 'b': 'bla'::mood}
select {'a': 'bla'::mood}
select MAP([1,2,3,4],['happy','ok','ok','sad']::mood[])
select MAP([1,2,3,4],['bla','ok','ok','sad']::mood[])
# file: test/sql/types/enum/test_enum_schema.test
# setup
CREATE SCHEMA s1
CREATE SCHEMA foo
CREATE TYPE s1.mood AS ENUM ('sad', 'ok', 'happy')
CREATE TYPE foo.bar AS ENUM ('a', 'b', 'c')
CREATE TABLE foo.baz ( bar_col "foo".bar NOT NULL )
CREATE TABLE foo.test ( qualified_array foo.bar[] )
# query
INSERT INTO foo.test VALUES (['a', 'b'])
# file: test/sql/types/enum/test_enum_structs.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE person ( id INTEGER, c STRUCT( name text, current_mood mood ) )
# query
FROM person
# file: test/sql/types/enum/test_enum_table.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TYPE intelligence AS ENUM ('dumb', 'smart', 'ehh')
CREATE TYPE car_brand AS ENUM ('Tesla', 'VW', 'Seat', 'Fiets')
create type breed AS ENUM ('Maltese', 'Shi-tzu', 'Samoyed', 'Robot')
create type midenum as enum ('0','1','2','3','4','5','6','7','8','9','10','11','12','13','14','15','16','17','18','19','20','21','22','23','24','25','26','27','28','29','30','31','32','33','34','35','36','37','38','39','40','41','42','43','44','45','46','47','48','49','50','51','52','53','54','55','56','57','58','59','60','61','62','63','64','65','66','67','68','69','70','71','72','73','74','75','76','77','78','79','80','81','82','83','84','85','86','87','88','89','90','91','92','93','94','95','96','97','98','99','100','101','102','103','104','105','106','107','108','109','110','111','112','113','114','115','116','117','118','119','120','121','122','123','124','125','126','127','128','129','130','131','132','133','134','135','136','137','138','139','140','141','142','143','144','145','146','147','148','149','150','151','152','153','154','155','156','157','158','159','160','161','162','163','164','165','166','167','168','169','170','171','172','173','174','175','176','177','178','179','180','181','182','183','184','185','186','187','188','189','190','191','192','193','194','195','196','197','198','199','200','201','202','203','204','205','206','207','208','209','210','211','212','213','214','215','216','217','218','219','220','221','222','223','224','225','226','227','228','229','230','231','232','233','234','235','236','237','238','239','240','241','242','243','244','245','246','247','248','249','250','251','252','253','254','255')
create type midenum_2 as enum ('0','1','2','3','4','5','6','7','8','9','10','11','12','13','14','15','16','17','18','19','20','21','22','23','24','25','26','27','28','29','30','31','32','33','34','35','36','37','38','39','40','41','42','43','44','45','46','47','48','49','50','51','52','53','54','55','56','57','58','59','60','61','62','63','64','65','66','67','68','69','70','71','72','73','74','75','76','77','78','79','80','81','82','83','84','85','86','87','88','89','90','91','92','93','94','95','96','97','98','99','100','101','102','103','104','105','106','107','108','109','110','111','112','113','114','115','116','117','118','119','120','121','122','123','124','125','126','127','128','129','130','131','132','133','134','135','136','137','138','139','140','141','142','143','144','145','146','147','148','149','150','151','152','153','154','155','156','157','158','159','160','161','162','163','164','165','166','167','168','169','170','171','172','173','174','175','176','177','178','179','180','181','182','183','184','185','186','187','188','189','190','191','192','193','194','195','196','197','198','199','200','201','202','203','204','205','206','207','208','209','210','211','212','213','214','215','216','217','218','219','220','221','222','223','224','225','226','227','228','229','230','231','232','233','234','235','236','237','238','239','240','241','242','243','244','245','246','247','248','249','250','251','252','253','254','255')
CREATE TYPE large_enum AS ENUM ('Floccinaucinihilipilification', 'Antidisestablishmentarianism', 'Honorificabilitudinitatibus')
CREATE TABLE pets ( name text, current_mood mood )
CREATE TABLE aliens ( name text, current_mood mood )
CREATE TABLE person ( name text, current_mood mood, last_year_mood mood, car car_brand )
CREATE TABLE person_string ( name text, current_mood text )
CREATE TABLE midenum_t ( test midenum )
CREATE TABLE midenum_t2 ( test_2 midenum_2 )
CREATE TABLE large_enum_tbl ( big_word large_enum )
# query
select count(*), current_mood from person group by current_mood order by all
# file: test/sql/types/geo/geometry_crs.test
# setup
create table t1 (g GEOMETRY('OGC:CRS84'))
create table t2 (g GEOMETRY('OGC:CRS83'))
create table t3 (srid VARCHAR, g GEOMETRY)
# query
select 'POINT(0 1)'::GEOMETRY((['abc']))
# file: test/sql/types/geo/geometry_crs_wkt2.test
# query
select typeof('POINT(0 1)'::GEOMETRY('GEOGCRS["WGS 84",foo[]'))
# file: test/sql/types/geo/geometry_shred.test
# setup
create table t1(g GEOMETRY)
# query
select distinct segment_type from pragma_storage_info('t1') order by all
# file: test/sql/types/geo/geometry_shred_empty.test
# setup
CREATE TABLE t1(type VARCHAR, has_z BOOLEAN, has_m BOOLEAN, g GEOMETRY)
# query
select segment_type from pragma_storage_info('t2') order by all
# file: test/sql/types/time/test_time_ns.test
# setup
CREATE TABLE times(tns TIME_NS)
# query
SELECT tns, DATE_PART(['hour', 'minute', 'second'], tns) FROM times
SELECT tns, DATE_PART(['millisecond', 'microsecond', 'epoch'], tns) FROM times
SELECT tns, DATE_PART(['timezone', 'timezone_hour', 'timezone_minute'], tns) p FROM times WHERE p <> {'timezone': 0, 'timezone_hour': 0, 'timezone_minute': 0}
SELECT date_part(['julian'], '23:59:59.123456789'::TIME_NS)
# file: test/sql/types/time/test_time_tz.test
# setup
CREATE TABLE timetzs (ttz TIMETZ)
# query
SELECT * FROM timetzs ORDER BY ALL
SELECT lhs.ttz, rhs.ttz, lhs.ttz < rhs.ttz, lhs.ttz <= rhs.ttz, lhs.ttz = rhs.ttz, lhs.ttz >= rhs.ttz, lhs.ttz > rhs.ttz, lhs.ttz <> rhs.ttz, FROM timetzs lhs, timetzs rhs ORDER BY ALL
# file: test/sql/types/struct/create_qualified_type_array.test
# setup
CREATE SCHEMA schema2
create type u as struct (i int, j int)
create type u2 as struct(i int, j int)
create or replace table i (j struct(i double, j double))
# query
select cast (null as main.u ARRAY[1])
insert into i values ({'i': 1.0, 'j': 2.0})
select cast (null as u array[1])
# file: test/sql/types/struct/nested_struct_projection_pushdown.test
# setup
CREATE OR REPLACE TABLE test_structs( id INT, s VARIANT )
CREATE OR REPLACE TABLE test_structs_nested(id INT, base VARIANT)
# query
INSERT INTO test_structs VALUES (1, {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}), (2, NULL), (3, {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}), (4, {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}})
INSERT INTO test_structs_nested VALUES (1, {'s': {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}}), (2, NULL), (3, {'s': {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}}), (4, {'s': {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}}})
# file: test/sql/types/struct/nested_structs.test
# setup
CREATE TABLE a(c ROW(i ROW(a INTEGER), j INTEGER))
CREATE TABLE b AS SELECT { 'a': { 'a': 1, 'b': 'hello' } } c
# query
INSERT INTO a VALUES ({ 'i': { 'a': 3 }, 'j': 4 })
CREATE TABLE b AS SELECT { 'a': { 'a': 1, 'b': 'hello' } } c
# file: test/sql/types/struct/remap_struct.test
# setup
CREATE TABLE structs(struct_val STRUCT(i INT, j VARCHAR))
# query
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v2 INT), NULL, {'v2': NULL::INTEGER})
select remap_struct(42, NULL::ROW(v1 INT, v2 INT, v3 INT), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::INTEGER})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 INT, v2 INT, v3 INT), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::INTEGER})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': 'hello'})
SELECT remap_struct( { 'i': 1, 'j': { 'x': 42, 'z': 100 } }, NULL::ROW( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR ), { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) }, { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } )
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'y': 100}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT, z STRUCT(a INT, b INT))), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'y': 'y'})}, {'v2': {'z': NULL::STRUCT(a INT, b INT)}})
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'y': 100, 'z': 1000}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, z INT), v3 VARCHAR), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'z': 'z'})}, {'v3': NULL::VARCHAR})
SELECT remap_struct( {'i': 1}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT, z INT), v3 VARCHAR), {'v1': 'i'}, {'v2': {'x': NULL::INT, 'y': NULL::INT, 'z': NULL::INT}, 'v3': NULL::VARCHAR})
INSERT INTO structs VALUES ({'i': 42, 'j': 'hello world this is my string'}), (NULL), ({'i': 100, 'j': NULL}), ({'i': NULL, 'j': 'string string string'})
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': 'hello'}) FROM structs
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, {'v2': NULL::VARCHAR}) FROM structs
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 INT, v2 INT), {'v1': 'j', 'v2': 'i'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v2': 'i'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL, {'v2': 'i'}, NULL)
SELECT remap_struct(ROW(1, 2), NULL::ROW(v1 VARCHAR), {'v2': 'i'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v1': 'k'}, NULL)
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR), {'v1': 'i'}, {'v1': NULL::VARCHAR})
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 VARCHAR, v2 VARCHAR), {'v1': 'i'}, NULL)
SELECT remap_struct(struct_val, NULL::ROW(v1 VARCHAR, v2 VARCHAR, v3 VARCHAR), {'v1': 'j', 'v3': 'i'}, struct_val) FROM structs
SELECT remap_struct({'i': 1, 'j': 2}, NULL::ROW(v1 INT, v2 INT, v3 INT), {'v1': 'j', 'v3': 'i'}, {'v2': 'hello'})
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'z': 100}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT), v3 VARCHAR), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'y': 'z'})}, {'v2': {'y': NULL::INT}, 'v3': NULL::VARCHAR})
SELECT remap_struct( {'i': 1, 'j': {'x': 42, 'z': 100}}, NULL::ROW(v1 INT, v2 STRUCT(x INT, y INT), v3 VARCHAR), {'v1': 'i', 'v2': ROW('j', {'x': 'x', 'y': 'z'})}, {'v2': NULL, 'v3': NULL::VARCHAR})
SELECT remap_struct( [ { 'i': 1, 'j': 42 } ], NULL::STRUCT(k INT)[], {'list': 'list'}, { 'list': { 'k': NULL } } )
# file: test/sql/types/struct/remap_struct_in_list.test
# setup
CREATE TABLE large_list(s STRUCT(i INTEGER)[])
# query
SELECT remap_struct( [ { 'i': 1, 'j': { 'x': 42, 'z': 100 } } ], NULL::STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )[], { 'list': ROW( 'list', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'list': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
INSERT INTO large_list (SELECT LIST(CASE WHEN i%2=0 THEN {'i': i} ELSE NULL END) FROM range(5000) t(i))
SELECT COUNT(*), COUNT(j), SUM(j) FROM ( SELECT UNNEST(remap_struct(s, NULL::ROW(j INTEGER)[], {'list': ROW('list', {'j': 'i'})}, NULL), recursive := True) FROM large_list )
# file: test/sql/types/struct/remap_struct_in_map.test
# query
SELECT remap_struct( MAP { 'my_key1' : { 'i': 10, 'j': { 'x': 42, 'z': 100 } }, 'my_key2' : { 'i': 20, 'j': { 'x': 21, 'z': 50 } } }, NULL::MAP(VARCHAR, STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )), { 'key': 'key', 'value': ROW( 'value', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'value': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(INT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(BIGINT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::STRUCT("key" BIGINT[], "value" VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(BIGINT[], MAP(VARCHAR, VARCHAR)), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(INT[], VARCHAR[]), { 'key': ROW( 'key', { 'list': 'list' } ), 'value': 'value' }, NULL )
# file: test/sql/types/struct/remap_struct_size.test
# setup
CREATE TABLE src(col1 STRUCT(i INT, j INT)[])
CREATE TABLE t AS SELECT remap_struct(col1, NULL::STRUCT(k INTEGER)[], NULL, struct_pack(list := struct_pack(k := NULL::INTEGER))) AS col1 FROM src
CREATE TABLE t2 AS SELECT remap_struct(col1, NULL::STRUCT(k INTEGER)[], NULL, struct_pack(list := struct_pack(k := NULL::INTEGER))) AS col1 FROM src
# query
INSERT INTO src VALUES ([{'i': 1, 'j': 2}])
FROM t ORDER BY ALL
INSERT INTO src VALUES ([{'i': 3, 'j': 4}]), ([{'i': 5, 'j': 6}])
FROM t2 ORDER BY ALL
# file: test/sql/types/struct/struct_case.test
# query
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
SELECT i, CASE WHEN i%2=0 THEN {'i': [1,2,3]} ELSE {'i': ['hello']} END FROM range(6) tbl(i)
# file: test/sql/types/struct/struct_case_insensitivity.test
# setup
CREATE TABLE tbl AS SELECT ({'HELLO': 3}) col
# query
CREATE TABLE tbl AS SELECT ({'HELLO': 3}) col
SELECT col['HELLO'] FROM tbl
SELECT col['hello'] FROM tbl
SELECT ({'hello': 3, 'hello': 4}) col
SELECT ({'HELLO': 3, 'HELLO': 4}) col
SELECT ({'HELLO': 3, 'hello': 4}) col
SELECT col['HELL'] FROM tbl
# file: test/sql/types/struct/struct_cast.test
# setup
CREATE TABLE structs(s ROW(i INTEGER, j INTEGER))
CREATE TABLE nested_structs(s ROW(i INTEGER, j ROW(a INTEGER, b INTEGER)))
# query
SELECT {'i': 1, 'j': 2}::ROW(i BIGINT, j VARCHAR)
SELECT {'i': NULL, 'j': 'hello'}::ROW(i BIGINT, j VARCHAR)
SELECT {'i': NULL, 'j': NULL}::ROW(i BIGINT, j VARCHAR)
SELECT ({'i': NULL, 'j': NULL}::ROW(i BIGINT, j VARCHAR))['i']
SELECT ({'i': NULL, 'j': NULL})['i']
SELECT (NULL::ROW(i BIGINT, j VARCHAR))['i']
SELECT {'i': 1, 'j': {'a': 2, 'b': 3}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': {'a': NULL, 'b': 3}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': {'a': 2, 'b': NULL}}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT {'i': 1, 'j': NULL}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR))
SELECT ({'i': 1, 'j': NULL}::ROW(i BIGINT, j ROW(a BIGINT, b VARCHAR)))['j']['a']
INSERT INTO structs VALUES ({'i': 1, 'j': 2}), ({'i': NULL, 'j': 2}), ({'i': 1, 'j': NULL}), (NULL)
INSERT INTO nested_structs VALUES ({'i': 1, 'j': {'a': 2, 'b': 3}}), ({'i': 1, 'j': {'a': NULL, 'b': 3}}), ({'i': 1, 'j': {'a': 2, 'b': NULL}}), ({'i': 1, 'j': NULL}), (NULL)
SELECT col::STRUCT(duck INT) FROM VALUES ('{"duck": null}'), ('{"duck": nulL}'), ('{"duck": nuLl}'), ('{"duck": nuLL}'), ('{"duck": nUll}'), ('{"duck": nUlL}'), ('{"duck": nULl}'), ('{"duck": nULL}'), ('{"duck": Null}'), ('{"duck": NulL}'), ('{"duck": NuLl}'), ('{"duck": NuLL}'), ('{"duck": NUll}'), ('{"duck": NUlL}'), ('{"duck": NULl}'), ('{"duck": NULL}'), AS tab(col)
SELECT {'i': 42, 'j': 84}::STRUCT(i INT) AS result
SELECT {'i': 42}::STRUCT(i INT, j INT) AS result
SELECT {'a': 7, 'i': 42, 'j': 84, 'k': 42}::STRUCT(m INT, k INT, l INT) AS result
# file: test/sql/types/struct/struct_cast_superset.test
# setup
CREATE TABLE t1 (s1 STRUCT(a INT, b INT))
CREATE TABLE t2 (s1 STRUCT(a INT, c INT))
# query
INSERT INTO t1 VALUES ({a: 42, b: 43})
INSERT INTO t2 VALUES ({a: 100, c: 101})
SELECT {'a': {'e1': 42, 'e2': 42}} AS c UNION ALL BY NAME SELECT {'a': {'e2': 'hello', 'e3': 'world'}, 'b': '100'} AS c
# file: test/sql/types/struct/struct_comparison.test
# setup
CREATE VIEW struct_int AS SELECT * FROM (VALUES ({'x': 1}, {'x': 1}), ({'x': 1}, {'x': 2}), ({'x': 2}, {'x': 1}), (NULL, {'x': 1}), ({'x': 2}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str AS SELECT * FROM (VALUES ({'x': 'duck'}, {'x': 'duck'}), ({'x': 'duck'}, {'x': 'goose'}), ({'x': 'goose'}, {'x': 'duck'}), (NULL, {'x': 'duck'}), ({'x': 'goose'}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str_int AS SELECT * FROM (VALUES ({'x': 'duck', 'y': 1}, {'x': 'duck', 'y': 1}), ({'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}), ({'x': 'goose', 'y': 2}, {'x': 'duck', 'y': 1}), (NULL, {'x': 'duck', 'y': 1}), ({'x': 'goose', 'y': 2}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_nested AS SELECT * FROM (VALUES ({'x': 1, 'y': {'a': 'duck', 'b': 1.5}}, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), ({'x': 1, 'y': {'a': 'duck', 'b': 1.5}}, {'x': 2, 'y': {'a': 'goose', 'b': 2.5}}), ({'x': 2, 'y': {'a': 'goose', 'b': 2.5}}, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), (NULL, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), ({'x': 2, 'y': {'a': 'goose', 'b': 2.5}}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_in_struct AS SELECT * FROM (VALUES ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 2, 'y': ['goose']}), ({'x': 2, 'y': ['goose']}, {'x': 1, 'y': ['duck', 'somateria']}), (NULL, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 2, 'y': ['goose']}, NULL), (NULL, NULL) ) tbl(l, r)
# query
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
SELECT {'x': 1} <> {'x': 2}
SELECT {'x': 1} <> {'x': 1}
SELECT {'x': 1} <> NULL
SELECT NULL <>{'x': 1}
SELECT {'x': 1} >= {'x': 2}
SELECT {'x': 1} >= {'x': 1}
SELECT NULL >= {'x': 1}
SELECT {'x': 1} >= NULL
SELECT {'x': 1} > {'x': 2}
SELECT {'x': 1} > {'x': 1}
SELECT NULL > {'x': 1}
SELECT {'x': 1} > NULL
CREATE VIEW struct_int AS SELECT * FROM (VALUES ({'x': 1}, {'x': 1}), ({'x': 1}, {'x': 2}), ({'x': 2}, {'x': 1}), (NULL, {'x': 1}), ({'x': 2}, NULL), (NULL, NULL) ) tbl(l, r)
# file: test/sql/types/struct/struct_concat.test
# setup
CREATE TABLE t1 AS SELECT {'i': i, 'j': i + i % 2} as s FROM generate_series(1, 15) AS t(i)
# query
SELECT struct_concat({'a': 1}, {'b': NULL}, NULL::STRUCT(k INT), struct_pack( x := 'foobar'))
CREATE TABLE t1 AS SELECT {'i': i, 'j': i + i % 2} as s FROM generate_series(1, 15) AS t(i)
SELECT struct_concat({'a': 2, 'b': NULL}, s) FROM t1
SELECT struct_concat(s, {'a': 2, 'b': NULL}) FROM t1 WHERE s.i % 2 = 0
SELECT struct_concat({'a': 'first struct'}, {'a': 'second struct'})
SELECT struct_concat({'a': 'first struct'}, {'A': 'second struct'})
# file: test/sql/types/struct/struct_different_names.test
# setup
CREATE TABLE t1 (s STRUCT(v VARCHAR))
CREATE TABLE foo (bar struct(pip int))
CREATE OR REPLACE TABLE T (s STRUCT(a INT, b INT))
CREATE TABLE tbl (a STRUCT(a INT, b VARCHAR))
CREATE VIEW v1 AS SELECT ROW(42)
# query
SELECT s FROM t1 ORDER BY ALL
SELECT bar FROM foo ORDER BY ALL
SELECT s FROM T ORDER BY ALL
# file: test/sql/types/struct/unnest_struct_mix.test
# setup
CREATE OR REPLACE TABLE tbl_structs AS SELECT {'a': 'hello', 'b': 1} s
# query
select unnest(s) from tbl_structs order by all
# file: test/sql/types/map/map_const_and_col_combination.test
# setup
CREATE TABLE ints (i INT)
CREATE TABLE tbl (v VARCHAR[])
CREATE TABLE MAP_input (keys INT[], values INT[])
CREATE TABLE groups (category INT, score INT)
CREATE TABLE align_tbl (i INT[])
CREATE TABLE allconst (i INT)
# query
SELECT MAP(['category', 'min', 'max'], [category, MIN(score), MAX(score)]) FROM groups GROUP BY category ORDER BY ALL
# file: test/sql/types/float/infinity_test.test
# query
SELECT f FROM floats WHERE f>=1 ORDER BY ALL
SELECT f FROM floats WHERE f<=1 ORDER BY ALL
# file: test/sql/types/float/nan_test.test
# query
SELECT f FROM floats WHERE f > 0 ORDER BY ALL
SELECT f FROM floats WHERE f >= 1 ORDER BY ALL
# file: test/sql/types/list/unnest_having_qualify.test
# query
select row_number() over () qualify unnest([1,2,3])
# file: test/sql/types/list/unnest_table_function.test
# setup
CREATE TABLE lists AS SELECT [1,2,3] l UNION ALL SELECT [4,5] UNION ALL SELECT [] UNION ALL SELECT [NULL] UNION ALL SELECT [7, 8]
CREATE TABLE tbl AS SELECT * FROM (VALUES ('a', array[4, 5, 5], array[5, 7]), ('b', array[2, 3], array[1, 2, 3, 4]), ('c', array[2, 3], array[4]) ) t(k, a,b)
# query
SELECT k, a, b, list_sort(ARRAY( SELECT DISTINCT ax FROM UNNEST(a) ta(ax) WHERE ax = any(b) ORDER BY ALL )) ab_intersect FROM tbl
# file: test/sql/types/variant/test_all_types.test
# query
select struct_pack(*COLUMNS(*))::VARIANT::JSON from test_all_types()
select {'var': COLUMNS(*)::VARIANT}::VARIANT from test_all_types()
select [COLUMNS(['tinyint', 'double'])::VARIANT, NULL, COLUMNS(['tinyint', 'double'])::VARIANT] from test_all_types()
select [COLUMNS(['tinyint', 'double'])::JSON, NULL, COLUMNS(['tinyint', 'double'])::JSON] from test_all_types()
# file: test/sql/types/variant/tpch_test.test
# setup
create table foo.variant_lineitem as select variant_normalize(STRUCT_PACK(*COLUMNS(*))::VARIANT) from lineitem
# query
create table foo.variant_lineitem as select variant_normalize(STRUCT_PACK(*COLUMNS(*))::VARIANT) from lineitem
select COLUMNS(*)::JSON from foo.variant_lineitem limit 10
# file: test/sql/types/variant/variant_distinct.test
# setup
CREATE VIEW list_int1 AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [2]), ([2], [1]), (NULL, [1]), ([2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [1, 2]), ([1, 2], [1]), (NULL, [1]), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int_empty AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([], []), ([], [1, 2]), ([1, 2], []), (NULL, []), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES (['duck'], ['duck']), (['duck'], ['duck', 'goose']), (['duck', 'goose'], ['duck']), (NULL, ['duck']), (['duck', 'goose'], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_of_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], [{'x': 'duck', 'y': 1}]), (NULL, [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck'}, {'x': 'duck'}), ({'x': 'duck'}, {'x': 'goose'}), ({'x': 'goose'}, {'x': 'duck'}), (NULL, {'x': 'duck'}), ({'x': 'goose'}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck', 'y': 1}, {'x': 'duck', 'y': 1}), ({'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}), ({'x': 'goose', 'y': 2}, {'x': 'duck', 'y': 1}), (NULL, {'x': 'duck', 'y': 1}), ({'x': 'goose', 'y': 2}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_nested AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 1, 'y': {'a': 'duck', 'b': 1.5}}, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), ({'x': 1, 'y': {'a': 'duck', 'b': 1.5}}, {'x': 2, 'y': {'a': 'goose', 'b': 2.5}}), ({'x': 2, 'y': {'a': 'goose', 'b': 2.5}}, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), (NULL, {'x': 1, 'y': {'a': 'duck', 'b': 1.5}}), ({'x': 2, 'y': {'a': 'goose', 'b': 2.5}}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_in_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 2, 'y': ['goose']}), ({'x': 2, 'y': ['goose']}, {'x': 1, 'y': ['duck', 'somateria']}), (NULL, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 2, 'y': ['goose']}, NULL), (NULL, NULL) ) tbl(l, r)
# query
CREATE VIEW list_int1 AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [2]), ([2], [1]), (NULL, [1]), ([2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [1, 2]), ([1, 2], [1]), (NULL, [1]), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int_empty AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([], []), ([], [1, 2]), ([1, 2], []), (NULL, []), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES (['duck'], ['duck']), (['duck'], ['duck', 'goose']), (['duck', 'goose'], ['duck']), (NULL, ['duck']), (['duck', 'goose'], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_of_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], [{'x': 'duck', 'y': 1}]), (NULL, [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck'}, {'x': 'duck'}), ({'x': 'duck'}, {'x': 'goose'}), ({'x': 'goose'}, {'x': 'duck'}), (NULL, {'x': 'duck'}), ({'x': 'goose'}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck', 'y': 1}, {'x': 'duck', 'y': 1}), ({'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}), ({'x': 'goose', 'y': 2}, {'x': 'duck', 'y': 1}), (NULL, {'x': 'duck', 'y': 1}), ({'x': 'goose', 'y': 2}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_in_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 2, 'y': ['goose']}), ({'x': 2, 'y': ['goose']}, {'x': 1, 'y': ['duck', 'somateria']}), (NULL, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 2, 'y': ['goose']}, NULL), (NULL, NULL) ) tbl(l, r)
# file: test/sql/types/union/union_cast.test
# setup
create table tbl1(u UNION(i32 INT, str VARCHAR))
create table tbl2(u UNION(str VARCHAR, i32 INT, f32 FLOAT))
CREATE TABLE tbl3 (u UNION(i INT))
CREATE TABLE tbl4 (u UNION(i INT, b BLOB))
CREATE TABLE t3 (id integer, u union(s1 struct(f1 varchar, f2 int), s2 struct(b1 varchar)))
# query
SELECT u.i32, u.str, u.f32 FROM tbl2 UNION ALL SELECT u.i32, u.str, NULL FROM tbl1 ORDER BY ALL
SELECT u::UNION(i SMALLINT) FROM tbl3 ORDER BY ALL
SELECT u::UNION(i SMALLINT, b VARCHAR) FROM tbl4 ORDER BY ALL
SELECT union_tag(u), union_tag(u::UNION(i SMALLINT, b INT)), u::UNION(i SMALLINT, b INT) FROM tbl4 ORDER BY ALL
# file: test/sql/types/union/union_join.test
# setup
CREATE TABLE tbl1(id INT, a UNION(b INT, c VARCHAR))
CREATE TABLE tbl2(id INT, d UNION(e INT, f VARCHAR))
# query
SELECT id, union_tag(a), a.b, a.c FROM tbl1 UNION SELECT id, union_tag(d), d.e, d.f FROM tbl2 ORDER BY ALL
SELECT id, union_tag(a) as tag, a.b as v1, a.c as v2 FROM tbl1 UNION SELECT id, union_tag(d) as tag, d.e as v1, d.f as v2 FROM tbl2 ORDER BY ALL
SELECT tbl1.a.c, tbl1.id, tbl2.id FROM tbl2 JOIN tbl1 ON tbl1.a.c = tbl2.d.f ORDER BY ALL
SELECT t1.id FROM tbl1 as t1 JOIN tbl1 as t2 ON t1.a = t2.a ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 INNER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 FULL OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 LEFT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 RIGHT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
# file: test/sql/types/union/union_struct.test
# setup
CREATE TABLE tbl1 (union_struct UNION(str VARCHAR, obj STRUCT(k VARCHAR, v INT)))
CREATE TABLE tbl2 (struct_union STRUCT(str VARCHAR, alt UNION(k VARCHAR, v INT)))
# query
SELECT * FROM tbl1 JOIN tbl2 ON tbl1.union_struct.str = tbl2.struct_union.alt.k order by all
# file: test/sql/types/nested/array/array_aggregate.test
# setup
CREATE TABLE tbl1 (a INT[3])
# query
SELECT FIRST(a ORDER BY ALL), LAST(a ORDER BY ALL) FROM tbl1
# file: test/sql/types/nested/array/array_fuzzer_failures.test
# setup
create table tbl(c8 UTINYINT)
CREATE TABLE array_tbl(c50 INTEGER[2][])
CREATE TABLE test(c2 BOOL, c48 STRUCT(a INTEGER[3], b VARCHAR[3]))
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types()
# query
SELECT c50 FROM array_tbl GROUP BY ALL USING SAMPLE 3
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types()
# file: test/sql/types/nested/array/array_joins.test
# setup
CREATE OR REPLACE TABLE t1(i INTEGER, a INTEGER[][3])
# query
SELECT DISTINCT * FROM t1 ORDER BY ALL
SELECT * FROM t1 JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 JOIN t2 ON t1.a = t2.a ORDER BY ALL
SELECT * FROM t1 FULL OUTER JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 as a JOIN t1 as b ON (a.col1 != b.col1) ORDER BY ALL
# file: test/sql/types/nested/array/array_misc.test
# setup
CREATE TABLE arrays (a INTEGER[3])
# query
SELECT DISTINCT a FROM arrays ORDER BY ALL
SELECT DISTINCT a FROM arrays WHERE a[1] > 0 ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS UNION SELECT a FROM ARRAYS ) ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS WHERE a[1] > 0 UNION SELECT a FROM ARRAYS WHERE a[1] > 0 ) ORDER BY ALL
SELECT a::VARCHAR FROM arrays ORDER BY ALL
SELECT TRY_CAST(a::INTEGER[] AS INTEGER[3]) FROM ARRAYS ORDER BY ALL
# file: test/sql/types/nested/map/test_map_cardinality.test
# setup
create table ints (a integer, b integer)
# query
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints group by a) as lst_tbl) as T ORDER BY ALL
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints where b < 3 group by a) as lst_tbl) as T ORDER BY ALL
# file: test/sql/types/nested/map/test_map_subscript.test
# setup
create table ints (a integer, b integer)
# query
select m from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints group by b) as lst_tbl) as T ORDER BY ALL
select m[1] from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints group by b) as lst_tbl) as T ORDER BY ALL
select m[1] from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints where b <4 group by b) as lst_tbl) as T ORDER BY ALL
# file: test/sql/types/nested/list/array_agg.test
# setup
CREATE TABLE films(film_id INTEGER, title VARCHAR)
CREATE TABLE actors(actor_id INTEGER, first_name VARCHAR, last_name VARCHAR)
CREATE TABLE film_actor(film_id INTEGER, actor_id INTEGER)
# query
select film_id, ARRAY_AGG(actor_id order by actor_id) FROM film_actor GROUP BY film_id ORDER BY ALL
# file: test/sql/prepared/prepare_from_first.test
# query
from (select 'from' fromV) select 'sel' selectV,*
# file: test/sql/insert/insert_by_name.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE "My Table"("My Column 1" INT, "My Column 2" INT)
CREATE TABLE tbl ( price INTEGER, total_price AS ((price)::DATE) )
CREATE TABLE tbl2 (a INTEGER, b INTEGER PRIMARY KEY)
CREATE TABLE tbl3 (id INTEGER PRIMARY KEY)
# query
FROM "My Table"
FROM tbl2
# file: test/sql/show_select/describe_subquery.test
# setup
CREATE TABLE t AS SELECT 42 AS a
# query
FROM (SHOW databases) t
# file: test/sql/join/test_merge_join_predicate.test
# setup
create or replace table tleft as select i as k100, '2024-01-01'::TIMESTAMP + INTERVAL (i) seconds as b100, b100 + INTERVAL 1 second as e100, from range(10) as tbl(i)
# query
select l.k100 as lid, r.k100 as rid from tleft l inner join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l left join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
explain select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
# file: test/sql/join/inner/test_join_is_distinct.test
# setup
CREATE TABLE tbl (col0 INTEGER, col1 INTEGER)
CREATE TABLE tbl_s (col0 STRUCT(x INTEGER), col1 STRUCT(x INTEGER))
CREATE TABLE tbl_l (col0 INTEGER[], col1 INTEGER[])
CREATE TABLE tbl_null (col0 INTEGER, col1 INTEGER)
CREATE TABLE tbl_s_null (col0 STRUCT(x INTEGER), col1 STRUCT(x INTEGER))
CREATE TABLE tbl_l_null (col0 INTEGER[], col1 INTEGER[])
create or replace table tb1 as select range*2 as a, range*50 as b from range(2)
create or replace table tb2 as select range*4 as a, range*500 as b from range(2)
# query
SELECT lhs.a, rhs.a FROM tb1 AS lhs LEFT JOIN tb2 AS rhs ON (lhs.a IS DISTINCT FROM rhs.a) ORDER BY ALL
# file: test/sql/join/inner/test_join_is_not_distinct.test
# setup
CREATE TABLE tbl (col0 INTEGER, col1 INTEGER)
CREATE TABLE tbl_s (col0 STRUCT(x INTEGER), col1 STRUCT(x INTEGER))
CREATE TABLE tbl_l (col0 INTEGER[], col1 INTEGER[])
CREATE TABLE tbl_null (col0 INTEGER, col1 INTEGER)
CREATE TABLE tbl_s_null (col0 STRUCT(x INTEGER), col1 STRUCT(x INTEGER))
CREATE TABLE tbl_l_null (col0 INTEGER[], col1 INTEGER[])
create or replace table tb1 as select range*2 as a, range*50 as b from range(2)
create or replace table tb2 as select range*4 as a, range*500 as b from range(2)
# query
SELECT lhs.a, rhs.a FROM tb1 AS lhs LEFT JOIN tb2 AS rhs ON (lhs.a IS NOT DISTINCT FROM rhs.a) ORDER BY ALL
# file: test/sql/join/inner/test_prefix_range_filter_pushdown.test
# setup
CREATE OR REPLACE TABLE probe AS SELECT i::BIGINT AS k, (i % 7)::INTEGER AS g FROM range(0, 100000) t(i)
CREATE OR REPLACE TABLE build AS SELECT (i * 2)::BIGINT AS k, (i % 7)::INTEGER AS g FROM range(0, 30000) t(i)
CREATE OR REPLACE TABLE probe_with_null AS SELECT i::BIGINT AS k, (i % 7)::INTEGER AS g FROM range(0, 20480) t(i) UNION ALL SELECT NULL::BIGINT as k, (i % 7)::INTEGER AS g FROM range(20480, 24576) t2(i) UNION ALL SELECT i::BIGINT AS k, (i % 7)::INTEGER AS g FROM range(24576, 100000) t3(i)
CREATE OR REPLACE TABLE probe_timestamp AS SELECT make_timestamp(i * 1000) AS k, (i % 7)::INTEGER AS g FROM range(0, 100000) t(i)
CREATE OR REPLACE TABLE build_timestamp AS SELECT make_timestamp(i * 2000) AS k, (i % 7)::INTEGER AS g FROM range(0, 30000) t(i)
CREATE OR REPLACE TABLE probe_varchar AS SELECT lpad((i // 3)::VARCHAR, 8, '0') AS k, (i % 7)::INTEGER AS g FROM range(0, 100000) t(i)
CREATE OR REPLACE TABLE build_varchar AS SELECT lpad(i::VARCHAR, 8, '0') AS k, (i % 7)::INTEGER AS g FROM range(0, 30000) t(i)
CREATE OR REPLACE TABLE probe_hugeint AS SELECT i::HUGEINT AS k, (i % 7)::INTEGER AS g FROM range(0, 100000) t(i)
CREATE OR REPLACE TABLE build_hugeint AS SELECT (i * 2)::HUGEINT AS k, (i % 7)::INTEGER AS g FROM range(0, 30000) t(i)
CREATE OR REPLACE TABLE prf_probe_string ( id INTEGER, k VARCHAR, g INTEGER )
CREATE OR REPLACE TABLE prf_build_string ( k VARCHAR, g INTEGER, payload INTEGER )
CREATE OR REPLACE TABLE prf_probe_string_nul ( id INTEGER, k VARCHAR, g INTEGER )
CREATE OR REPLACE TABLE prf_build_string_nul ( k VARCHAR, g INTEGER, payload INTEGER )
CREATE OR REPLACE TABLE prf_probe_span ( id INTEGER, k BIGINT, g INTEGER )
CREATE OR REPLACE TABLE prf_build_span ( k BIGINT, g INTEGER, payload INTEGER )
CREATE OR REPLACE TABLE prf_row_group_bug.prf_probe_small ( k SMALLINT, g SMALLINT )
CREATE OR REPLACE TABLE prf_build_row_group_bug ( k SMALLINT, g SMALLINT )
# query
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_small p JOIN prf_build_small b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_string p JOIN prf_build_string b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, hex(p.k), p.g, b.payload FROM prf_probe_string_nul p JOIN prf_build_string_nul b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_span p JOIN prf_build_span b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT b.k, p.k FROM prf_row_group_bug.prf_probe_small p RIGHT JOIN prf_build_row_group_bug b USING (k) ORDER BY ALL
# file: test/sql/join/semianti/plan_blockwise_NL_join_with_mutliple_conditions.test
# setup
create table t1 as select * from values (1, 2), (2, 4), (3, 8), (6, 25), (1, 25) t(a, b)
create table t2 as select * from values (4), (5) t(b)
CREATE TABLE flattened ("start" varchar, "end" varchar)
create table input_table as select * from VALUES ('1', '2023-03-14T00:00:00Z', 2), ('2', '2023-03-15T00:00:00Z', 4), ('3', '2023-03-16T00:00:00Z', 7), ('4', '2023-03-17T00:00:00Z', 3), ('5', '2023-03-18T00:00:00Z', 2), ('6', '2023-03-19T23:59:59Z', 4), ('7', '2023-03-20T00:00:00Z', 7), ('8', '2023-03-21T00:00:00Z', 3) t(user_id, timestamp, value)
# query
select * from t1 semi join t2 on t1.a < t2.b and t1.b > t2.b order by all
select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
Explain select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
select * from t1 semi join t2 on t1.a < t2.b or t1.b < t2.b order by all
select * from t1 semi join t2 on (t1.a < t2.b and t1.b < t2.b) or (t1.a < t2.b and t1.b = 4) order by all
select * from t1 semi join t2 on (t1.a < t2.b or t1.b < t2.b) and (t1.a = 1 or t1.b = 4) order by all
# file: test/sql/join/positional/issue20086.test
# setup
CREATE TABLE t0(c0 INT, c1 INT)
CREATE TABLE t1(c0 INT)
CREATE UNIQUE INDEX t0i0 ON t0(c0, c1)
# query
SELECT * FROM t1 POSITIONAL JOIN t0 WHERE (t1.c0 > t0.c1) IS NULL
# file: test/sql/join/positional/test_positional_join.test
# setup
CREATE TABLE two (a INTEGER, b INTEGER)
CREATE TABLE three AS SELECT * FROM (VALUES (11, 1), (12, 2), (13, 3) ) tbl(a, b)
CREATE TABLE threek AS SELECT * FROM generate_series(0, 3001) tbl(id)
# query
SELECT * FROM two t1 POSITIONAL JOIN two t2
SELECT * FROM threek t1 POSITIONAL JOIN threek t2 WHERE t1.id <> t2.id
SELECT * FROM two t1 POSITIONAL JOIN three t2
SELECT * FROM three t1 POSITIONAL JOIN two t2
SELECT COUNT(a), COUNT(id) FROM three POSITIONAL JOIN threek
SELECT COUNT(id), COUNT(a) FROM threek POSITIONAL JOIN three
SELECT * FROM (SELECT * FROM two WHERE a % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM two WHERE a % 2 = 1) t2
SELECT * FROM (SELECT * FROM threek WHERE id % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM threek WHERE id % 2 = 1) t2 WHERE t1.id + 1 <> t2.id
SELECT * FROM (SELECT * FROM three WHERE a % 2 = 1) t1 POSITIONAL JOIN (SELECT * FROM two WHERE a % 2 = 0) t2
SELECT * FROM (SELECT * FROM two WHERE a % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM three WHERE a % 2 = 1) t2
SELECT COUNT(t1.id), COUNT(t2.id) FROM (SELECT * FROM threek WHERE id % 2 = 0) t1 POSITIONAL JOIN (SELECT * FROM threek WHERE id % 3 = 0) t2
SELECT COUNT(t1.id), COUNT(t2.id) FROM (SELECT * FROM threek WHERE id % 3 = 0) t2 POSITIONAL JOIN (SELECT * FROM threek WHERE id % 2 = 0) t1
# file: test/sql/join/asof/test_asof_empty_right.test
# query
select lefttable.x, righttable.y from (select 1 as x) lefttable asof left join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x limit 0) lefttable asof left join (select 1 as x, 1 as y) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x) lefttable asof join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
# file: test/sql/join/asof/test_asof_join.test
# setup
CREATE TABLE events0 (begin DOUBLE, value INTEGER)
create table prices("when" timestamp, symbol int, price int)
create table trades("when" timestamp, symbol int)
# query
SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol = p.symbol AND t.when >= p.when
EXPLAIN SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol IS NOT DISTINCT FROM p.symbol AND t.when >= p.when
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON 1 = 1 AND p.ts >= e.begin ORDER BY p.ts ASC
WITH samples AS ( SELECT col0 AS starts, col1 AS ends FROM (VALUES (5, 9), (10, 13), (14, 20), (21, 23) ) ) SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM samples AS s1 ASOF JOIN samples as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts <> e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts = e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts >= e.begin AND p.ts >= e.value ORDER BY p.ts ASC
# file: test/sql/join/asof/test_asof_join_doubles.test
# setup
CREATE TABLE events0 (begin DOUBLE, value INTEGER)
CREATE TABLE events (key INTEGER, begin DOUBLE, value INTEGER)
CREATE TABLE probes AS SELECT key, ts FROM range(1,3) k(key) CROSS JOIN range(0,10) t(ts)
# query
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF LEFT JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC NULLS FIRST
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF LEFT JOIN events0 e USING (begin) ORDER BY p.begin ASC NULLS FIRST
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF RIGHT JOIN events0 e ON p.ts >= e.begin ORDER BY p.ts ASC NULLS LAST
SELECT p.begin, e.value FROM range(0,10) p(begin) ASOF RIGHT JOIN events0 e USING (begin) ORDER BY p.begin ASC NULLS LAST
SELECT p.key, p.ts, e.value FROM probes p ASOF JOIN events e ON p.key = e.key AND p.ts >= e.begin ORDER BY 1, 2 ASC
SELECT p.key, p.begin, e.value FROM (SELECT key, ts AS begin FROM probes) p ASOF JOIN events e USING (key, begin) ORDER BY 1, 2 ASC
SELECT p.key, p.ts, e.value FROM probes p ASOF LEFT JOIN events e ON p.key = e.key AND p.ts >= e.begin ORDER BY 1, 2, 3 ASC NULLS FIRST
SELECT p.key, p.begin, e.value FROM (SELECT key, ts AS begin FROM probes) p ASOF LEFT JOIN events e USING (key, begin) ORDER BY 1, 2 ASC NULLS FIRST
SELECT p.key, p.ts, e.value FROM probes p ASOF RIGHT JOIN events e ON p.key = e.key AND p.ts >= e.begin ORDER BY 1 ASC NULLS FIRST, 2
SELECT p.key, p.begin, e.value FROM (SELECT key, ts AS begin FROM probes) p ASOF RIGHT JOIN events e USING (key, begin) ORDER BY 1 ASC NULLS FIRST, 2
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN (from events0 where value < 0) e ON p.ts >= e.begin ORDER BY p.ts ASC
# file: test/sql/join/asof/test_asof_join_filter_pushdown.test
# setup
CREATE TABLE left_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, price DECIMAL)
CREATE TABLE right_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, bid DECIMAL, active BOOLEAN)
# query
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150 AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND false
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
# file: test/sql/join/asof/test_asof_join_inequalities.test
# setup
CREATE TABLE events0 AS SELECT '2023-03-21 13:00:00'::TIMESTAMP + INTERVAL (range) HOUR AS begin, range AS value FROM range(0, 4)
CREATE TABLE probe0 AS SELECT * FROM range('2023-03-21 12:00:00'::TIMESTAMP, '2023-03-21 22:00:00'::TIMESTAMP, INTERVAL 1 HOUR) p(begin)
# query
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
# file: test/sql/join/asof/test_asof_join_integers.test
# setup
CREATE TABLE events0 (begin INTEGER, value INTEGER)
CREATE TABLE probe0 AS SELECT range::INTEGER AS begin FROM range(0,10)
# query
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin >= e.begin ORDER BY ALL
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e USING (begin) ORDER BY ALL
# file: test/sql/join/asof/test_asof_join_predicates.test
# setup
CREATE TABLE tt1 (i INTEGER, j VARCHAR)
CREATE TABLE tt2 (i INTEGER, j VARCHAR, k VARCHAR)
create table l (id integer, date timestamp, item varchar)
create table r (id integer, date timestamp, item varchar, valuei double)
create temp table tbl1 as select unnest(range(1000)) % 10 as x, '2022-01-01'::timestamp + to_days(unnest(range(1000))) as ts
create temp table tbl2 as select unnest(range(1000)) % 10 as x, '2022-01-01'::timestamp + to_hours(unnest(range(1000))) as ts
# query
explain SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON tt1.j = tt2.j AND tt1.i >= tt2.i ORDER BY tt1.i
SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON (tt1.j = tt2.j OR tt1.j = tt2.j) AND tt1.i >= tt2.i ORDER BY tt1.i
explain select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
explain from tbl1 asof join tbl2 on tbl1.x = tbl2.x and tbl1.ts >= tbl2.ts and (tbl1.ts - tbl2.ts) < interval '1' hours
from tbl1 asof join tbl2 on tbl1.x = tbl2.x and tbl1.ts >= tbl2.ts and (tbl1.ts - tbl2.ts) < interval '1' hours
# file: test/sql/join/asof/test_asof_join_prefix.test
# setup
CREATE TABLE prices_int("when" TIMESTAMP, symbol INTEGER, price INTEGER)
CREATE TABLE trades_int("when" timestamp, symbol INTEGER)
CREATE TABLE prices_varchar("when" TIMESTAMP, symbol VARCHAR, price INTEGER)
CREATE TABLE trades_varchar("when" timestamp, symbol VARCHAR)
CREATE TABLE prices_struct("when" TIMESTAMP, symbol STRUCT(ticker VARCHAR, exchange INTEGER), price INTEGER)
CREATE TABLE trades_struct("when" timestamp, symbol STRUCT(ticker VARCHAR, exchange INTEGER))
CREATE TABLE prices_list("when" TIMESTAMP, symbol INTEGER[], price INTEGER)
CREATE TABLE trades_list("when" timestamp, symbol INTEGER[])
CREATE TABLE prices_array("when" TIMESTAMP, symbol INTEGER[2], price INTEGER)
CREATE TABLE trades_array("when" timestamp, symbol INTEGER[2])
CREATE TABLE prices_nested("when" TIMESTAMP, symbol STRUCT(ticker VARCHAR[], exchange INTEGER), price INTEGER)
CREATE TABLE trades_nested("when" timestamp, symbol STRUCT(ticker VARCHAR[], exchange INTEGER))
CREATE TABLE prices_multiple("when" TIMESTAMP, symbol VARCHAR, exchange INTEGER, price INTEGER)
CREATE TABLE trades_multiple("when" timestamp, symbol VARCHAR, exchange INTEGER)
# query
SELECT t.*, p.price FROM trades_int t ASOF JOIN prices_int p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_varchar t ASOF JOIN prices_varchar p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_struct t ASOF JOIN prices_struct p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_list t ASOF JOIN prices_list p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_array t ASOF JOIN prices_array p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_nested t ASOF JOIN prices_nested p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_multiple t ASOF JOIN prices_multiple p ON t.symbol = p.symbol AND t.exchange = p.exchange AND t.when >= p.when
# file: test/sql/join/asof/test_asof_join_pushdown.test
# setup
CREATE OR REPLACE TABLE right_pushdown(time INTEGER, value FLOAT)
CREATE TABLE issue13899(seq_no INT, amount DECIMAL(10,2))
CREATE OR REPLACE TABLE issue12215 AS SELECT col0 AS starts, col1 AS ends FROM (VALUES (5, 9), (10, 13), (14, 20), (21, 23) )
# query
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF LEFT JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM issue12215 AS s1 ASOF JOIN issue12215 as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF LEFT JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
select a.seq_no, a.amount, b.amount from issue13899 as a asof join issue13899 as b on a.seq_no>=b.seq_no and b.amount is not null ORDER BY 1
WITH t1 AS ( FROM (VALUES (1,2),(2,4)) t1(id, value) ), t2 AS ( FROM (VALUES (1,3)) t2(id, value) ) FROM t1 ASOF LEFT JOIN t2 ON t1.id <= t2.id ORDER BY 1
WITH t1 AS ( FROM (VALUES (1,2),(2,4)) t1(id, value) ), t2 AS ( FROM (VALUES (1,3)) t2(id, value) ) FROM t1 ASOF LEFT JOIN t2 ON t1.id >= t2.id AND t1.id = 1 ORDER BY 1
WITH t1 AS ( FROM VALUES (1::INT, '2020-01-01 00:00:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') AS t1(a, b) ), t2 AS ( FROM VALUES (1::INT, '2020-01-01 00:01:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') t2(c, d) ) SELECT * FROM t1 ASOF JOIN t2 ON t1=b == t2.d AND t1.b >= t2.d - INTERVAL '1' SECOND
# file: test/sql/join/asof/test_asof_join_subquery.test
# setup
CREATE TABLE events (begin DOUBLE, value INTEGER)
# query
SELECT begin, value IN ( SELECT e1.value FROM ( SELECT * FROM events e1 WHERE e1.value = events.value) e1 ASOF JOIN range(1, 10) tbl(begin) USING (begin) ) FROM events ORDER BY ALL
# file: test/sql/join/asof/test_asof_join_timestamps.test
# setup
CREATE TABLE events0 AS SELECT '2023-03-21 13:00:00'::TIMESTAMP + INTERVAL (range) HOUR AS begin, range AS value FROM range(0, 4)
CREATE TABLE probe0 AS SELECT * FROM range('2023-03-21 12:00:00'::TIMESTAMP, '2023-03-21 22:00:00'::TIMESTAMP, INTERVAL 1 HOUR) p(begin)
CREATE TABLE asof_nulls ( time TIMESTAMP, value FLOAT )
# query
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF SEMI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF ANTI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT time_series.time, asof_nulls.value FROM (VALUES ('2025-07-15 02:00:00'::TIMESTAMP)) as time_series(time) ASOF LEFT JOIN asof_nulls ON asof_nulls.time <= time_series.time
# file: test/sql/join/asof/test_asof_join_types.test
# setup
CREATE TABLE left_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, price DECIMAL)
CREATE TABLE right_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, bid DECIMAL, active BOOLEAN)
# query
SELECT * FROM left_table l ASOF SEMI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF RIGHT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF FULL JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
# file: test/sql/join/asof/test_asof_semi_anti_mark.test
# setup
CREATE TABLE left_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, price DECIMAL)
CREATE TABLE right_table (id INTEGER, ts TIMESTAMP, symbol VARCHAR, bid DECIMAL, active BOOLEAN)
# query
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
# file: test/sql/join/iejoin/iejoin_projection_maps.test
# setup
CREATE TABLE df (id INTEGER, id2 INTEGER, id3 INTEGER, value_double DOUBLE, value as (value_double::DECIMAL(4,3)), one_min_value as ((1.0 - value_double)::DECIMAL(4,3)))
# query
SELECT id2, id3, id3_right, sum(value * value_right) as value FROM ( SELECT df.*, df2.id3 as id3_right, df2.value as value_right FROM df JOIN df as df2 ON (df.id = df2.id AND df.id2 = df2.id2 AND df.id3 > df2.id3 AND df.id3 < df2.id3 + 30) ) tbl GROUP BY ALL ORDER BY ALL
# file: test/sql/join/iejoin/test_ieantijoin.test
# setup
CREATE TABLE left_small( id INTEGER, start DATE, stop DATE, symbol VARCHAR, price DECIMAL )
CREATE TABLE right_small ( id INTEGER, start DATE, stop DATE, symbol VARCHAR, bid DECIMAL, active BOOLEAN )
CREATE OR REPLACE TABLE wide_ranges AS ( SELECT id, '2026-01-01'::TIMESTAMP + INTERVAL (id * 5 * 2048 // 7) SECONDS AS start, '2026-01-01'::TIMESTAMP + INTERVAL ((id + 1) * 5 * 2048 // 7) SECONDS AS stop, CHR(65 + (id % 26)::INTEGER) AS symbol, 149.5 + id * 5 * 2048 / 700.0 AS price, FROM range(8) tbl(id) )
CREATE OR REPLACE TABLE narrow_ranges AS ( SELECT id, '2026-01-01'::TIMESTAMP + INTERVAL (id + 15 * 2048 // 7) SECONDS AS start, start + INTERVAL 1 SECOND AS stop, CHR(65 + (id % 26)::INTEGER) AS symbol, 150.0 + id / 100.0 AS bid, (id % 2)::BOOLEAN AS active, FROM range(2048*5) tbl(id) )
# query
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND l.price + r.bid < 400 ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol AND l.price + r.bid < 375 ORDER BY id
# file: test/sql/join/iejoin/test_iejoin.test
# setup
CREATE TABLE issue3486 AS SELECT generate_series as ts from generate_series(timestamp '2020-01-01', timestamp '2021-01-01', interval 1 day)
create table test_big as select range i, range + 100_000 j, 'hello' k from range (20_000)
create table test_small as select range i, range + 100_000 j, 'hello' k from range (0,20_000,10)
# query
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ) SELECT lhs.id, rhs.id FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ), sub AS ( SELECT lhs.id AS lid, rhs.id AS rid FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL ) SELECT MIN(lid), MAX(rid) FROM sub
# file: test/sql/join/iejoin/test_iejoin_predicate.test
# setup
create or replace table tleft as select i as k100, '2024-01-01'::TIMESTAMP + INTERVAL (i) seconds as b100, b100 + INTERVAL 1 second as e100, from range(10) as tbl(i)
# query
explain select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and r.b100 < l.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and r.b100 < l.e100 and l.k100 + r.k100 < 10 order by all
# file: test/sql/join/iejoin/test_iesemijoin.test
# setup
CREATE TABLE left_small( id INTEGER, start DATE, stop DATE, symbol VARCHAR, price DECIMAL )
CREATE TABLE right_small ( id INTEGER, start DATE, stop DATE, symbol VARCHAR, bid DECIMAL, active BOOLEAN )
CREATE OR REPLACE TABLE wide_ranges AS ( SELECT id, '2026-01-01'::TIMESTAMP + INTERVAL (id * 5 * 2048 // 7) SECONDS AS start, '2026-01-01'::TIMESTAMP + INTERVAL ((id + 1) * 5 * 2048 // 7) SECONDS AS stop, CHR(65 + (id % 26)::INTEGER) AS symbol, 149.5 + id * 5 * 2048 / 700.0 AS price, FROM range(6) tbl(id) )
CREATE OR REPLACE TABLE narrow_ranges AS ( SELECT id, '2026-01-01'::TIMESTAMP + INTERVAL (id) SECONDS AS start, start + INTERVAL 1 SECOND AS stop, CHR(65 + (id % 26)::INTEGER) AS symbol, 150.0 + id / 100.0 AS bid, (id % 2)::BOOLEAN AS active, FROM range(2048*5) tbl(id) )
# query
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND l.price + r.bid < 400 ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol AND l.price + r.bid < 375 ORDER BY id
# file: test/sql/generated_columns/virtual/partition.test
# setup
CREATE TABLE unit ( price INTEGER, amount_sold INTEGER, name VARCHAR, total_profit AS (price * amount_sold) )
# query
SELECT total_profit, COUNT(total_profit), SUM(amount_sold), SUM(price) FROM unit GROUP BY total_profit ORDER BY ALL
# file: test/sql/generated_columns/virtual/update_index.test
# setup
CREATE TABLE tbl_comp ( a INT, gen AS (2 * a), b INT, c VARCHAR, PRIMARY KEY(c))
# query
FROM tbl_comp
# file: test/sql/update/string_update_transaction_local_7348.test
# setup
CREATE TABLE t1(a VARCHAR(256) PRIMARY KEY, b INTEGER)
# query
FROM t1
# file: test/sql/constraints/primarykey/test_pk_updel_multi_column.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR, PRIMARY KEY(a, b))
# query
SELECT * FROM test ORDER BY ALL
# file: test/sql/constraints/foreignkey/test_fk_self_referencing.test
# setup
CREATE TABLE employee( id INTEGER PRIMARY KEY, managerid INTEGER, name VARCHAR, FOREIGN KEY(managerid) REFERENCES employee(id))
# query
SELECT * FROM employee ORDER BY ALL
# file: test/sql/constraints/foreignkey/test_fk_temporary.test
# setup
CREATE SCHEMA s1
CREATE TEMPORARY TABLE album (artistid INTEGER, albumname TEXT, albumcover TEXT, UNIQUE (artistid, albumname))
CREATE TEMPORARY TABLE song (songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY (songartist, songalbum) REFERENCES album (artistid, albumname))
CREATE TABLE s1.pkt(i INTEGER PRIMARY KEY)
CREATE TABLE s1.fkt(j INTEGER, FOREIGN KEY (j) REFERENCES s1.pkt(i))
CREATE TABLE pkt(i INTEGER UNIQUE)
CREATE TABLE fkt(j INTEGER, FOREIGN KEY (j) REFERENCES pkt(i))
CREATE INDEX k_index ON pkt(k)
CREATE INDEX l_index ON fkt(l)
# query
SELECT * FROM album ORDER BY ALL
SELECT * FROM song ORDER BY ALL
# file: test/sql/limit/test_batch_limit_filters.test
# setup
CREATE TABLE tbl AS SELECT concat('thisisastring', i) s FROM range(1_000_000) t(i)
# query
FROM tbl WHERE s LIKE '%string999999%' LIMIT 5
# file: test/sql/optimizer/test_common_subplan_cte_binding_order.test
# setup
CREATE TABLE orders (id INT, amount INT, status VARCHAR, created_at TIMESTAMP)
CREATE TABLE line_items (id INT, order_id INT, sku VARCHAR, extracted_at TIMESTAMP)
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
CREATE VIEW order_lifecycle AS WITH sku_agg AS ( SELECT order_id, sum(CASE WHEN sku = 'WIDGET' THEN 1 ELSE 0 END) AS widget_count FROM line_items_deduped GROUP BY order_id ) SELECT o.amount, CASE WHEN COALESCE(s.widget_count, 0) > 0 THEN 'widget_order' ELSE 'other' END AS order_type, (o.status != 'refunded') AS is_net_order FROM orders_deduped o LEFT JOIN sku_agg s ON o.id = s.order_id
# query
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
# file: test/sql/optimizer/test_duplicate_groups_optimizer.test
# setup
create table t1(col1 int, col2 int)
create table t2(col3 int)
create table t3 (a int, b int, c int)
# query
select * from t3 group by cube(a, b, c) order by all
# file: test/sql/optimizer/test_rowid_pushdown.test
# setup
CREATE TABLE t1 AS SELECT i + 100 as x FROM range(250000) AS t(i)
# query
SELECT * FROM t1 where rowid IN (6, 9) ORDER BY ALL
SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
EXPLAIN SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
# file: test/sql/optimizer/test_window_self_join.test
# setup
CREATE TABLE services (date DATE, train_number INT)
CREATE TABLE items (id INT, category VARCHAR)
CREATE TABLE null_partition_test (id INT, category VARCHAR)
CREATE OR REPLACE TABLE foo ( emp_name VARCHAR, dept_name VARCHAR, base_salary FLOAT )
# query
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 1
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 1 ORDER BY ALL
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) > 1
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) > 1
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 2
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 2
EXPLAIN FROM services QUALIFY count(*) OVER() = 1
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date ORDER BY train_number) = 1
FROM items QUALIFY count(*) OVER(PARTITION BY category) = 1
EXPLAIN FROM items QUALIFY count(*) OVER(PARTITION BY category) = 1
FROM null_partition_test QUALIFY count(*) OVER(PARTITION BY category) = 1
EXPLAIN FROM null_partition_test QUALIFY count(*) OVER(PARTITION BY category) = 1
EXPLAIN FROM services QUALIFY max(train_number) OVER(PARTITION BY date, train_number) = 101
FROM services QUALIFY max(train_number) OVER(PARTITION BY date, train_number) = 101
EXPLAIN FROM services QUALIFY max(train_number) OVER(PARTITION BY date, train_number) = 100 AND count(*) OVER(PARTITION BY date, train_number) = 1
FROM services QUALIFY max(train_number) OVER(PARTITION BY date, train_number) = 100 AND count(*) OVER(PARTITION BY date, train_number) = 1
EXPLAIN FROM services WHERE date > '2024-06-30'::DATE QUALIFY max(train_number) OVER(PARTITION BY date, train_number) = 100 AND count(*) OVER(PARTITION BY date, train_number) = 1
select avg(avg(base_salary)) over ( partition by dept_name ) from foo group by dept_name order by all
explain select avg(avg(base_salary)) over ( partition by dept_name ) from foo group by dept_name order by all
# file: test/sql/optimizer/topn_window_set_elimination.test
# setup
CREATE or replace TABLE timeseries AS FROM ( VALUES (timestamp '2026-03-25 05:33:11.822+08', 10), (timestamp '2026-03-26 05:33:11.822+08', 15), (timestamp '2026-03-27 05:33:11.822+08', 12), (timestamp '2026-03-28 05:33:11.822+08', 18), (timestamp '2026-03-29 05:33:11.822+08', 14) ) AS t(date, value)
CREATE TABLE t2 (a VARCHAR, b BOOLEAN, c VARCHAR)
CREATE OR REPLACE MACRO nextValue(time_serie, ts_col, value_col, ts) AS TABLE ( (SELECT ts_col, value_col FROM query_table(time_serie) WHERE ts_col >= ts order by ts_col limit 1) union select ts AS ts_col, (select value_col from query_table(time_serie) order by ts_col desc limit 1) AS value_col WHERE NOT EXISTS (FROM query_table(time_serie) WHERE ts_col >= ts) )
# query
from range(1,5) as t(days), nextValue(timeseries, date, value, '2026-03-29 05:33:11.822+08'::timestamp - INTERVAL (days) DAY) limit 5
WITH cte AS ( SELECT a, b, c::TIMESTAMPTZ AS c FROM t2 ) SELECT * FROM cte QUALIFY ROW_NUMBER() OVER (PARTITION BY a ORDER BY c DESC) = 1
# file: test/sql/optimizer/plan/test_filter_pushdown.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE cohort ( person_id INTEGER, cohort_start_date DATE, cohort_end_date DATE )
CREATE TABLE obs ( person_id INTEGER, observation_period_start_date DATE )
CREATE TABLE t0(c0 VARCHAR(500))
# query
SELECT q01.* FROM ( SELECT LHS.*, observation_period_start_date FROM ( SELECT q01.* FROM ( SELECT person_id, cohort_start_date, COALESCE(cohort_end_date, cohort_start_date) AS cohort_end_date FROM cohort ) q01 WHERE (cohort_start_date <= cohort_end_date) ) LHS INNER JOIN obs ON (LHS.person_id = obs.person_id) ) q01 WHERE (cohort_end_date >= observation_period_start_date) ORDER BY ALL
# file: test/sql/collate/icu_collation_propagation.test
# setup
create table tbl (a varchar, b varchar)
# query
select concat(a collate de, a) from tbl order by all
select lower(a collate de) from tbl order by all
select upper(a collate de) from tbl order by all
select trim(b collate de, '<>') from tbl order by all
select ltrim(b collate de, '<>') from tbl order by all
select rtrim(b collate de, '<>') from tbl order by all
select repeat(a collate de, 10) from tbl order by all
select left(b collate de, 6) from tbl order by all
select right(b collate de, 6) from tbl order by all
select right(left(b collate de, 6), 1) from tbl order by all
select reverse(a collate de) from tbl order by all
select a from tbl where contains(b collate de, 'o') order by all
select a from tbl where starts_with(b collate de, '>>>>>o') order by all
select a from tbl where b collate de like '%>o<%' order by all
# file: test/sql/collate/test_collation_propagation.test
# setup
create table tbl (a varchar, b varchar)
# query
select a from tbl where contains(b collate nocase, 'O') order by all
select concat(a collate noaccent, a) from tbl order by all
select lower(a collate noaccent) from tbl order by all
select upper(a collate noaccent) from tbl order by all
select trim(b collate noaccent, '<>') from tbl order by all
select ltrim(b collate noaccent, '<>') from tbl order by all
select rtrim(b collate noaccent, '<>') from tbl order by all
select repeat(a collate noaccent, 10) from tbl order by all
select left(b collate noaccent, 6) from tbl order by all
select right(b collate noaccent, 6) from tbl order by all
select right(left(b collate noaccent, 6), 1) from tbl order by all
select reverse(a collate noaccent) from tbl order by all
select a from tbl where contains(b collate noaccent, 'o') order by all
select a from tbl where contains(b, 'ö' collate noaccent) order by all
select a from tbl where starts_with(b collate noaccent, '>>>>>o') order by all
# file: test/sql/collate/test_pragma_collations.test
# query
from pragma_collations() where collname like 'n%' order by all
# file: test/sql/binder/alias_qualification_having.test
# setup
CREATE TABLE alias (g INT)
# query
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT c1, STRING_AGG(c2, ',' order by c2) as c3 GROUP BY c1 HAVING len(c3) > 7
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT c1, STRING_AGG(c2, ',' order by c2) as c3 GROUP BY c1 HAVING c3.len() > 7
# file: test/sql/binder/alias_qualification_qualify.test
# setup
CREATE TABLE alias (v INT)
# query
SELECT a, row_number() OVER (ORDER BY a) AS rn FROM (VALUES (3),(1),(2)) t(a) QUALIFY alias.rn <= 2 ORDER BY a
SELECT a + 1 AS x, row_number() OVER (ORDER BY alias.x) AS rx FROM (VALUES (10),(20),(30)) t(a) QUALIFY alias.rx = 2 ORDER BY a
SELECT a AS "MiXeD", row_number() OVER (ORDER BY a) AS r FROM (VALUES (2),(1)) t(a) QUALIFY alias.r = 1 ORDER BY a
SELECT v AS x, row_number() OVER (ORDER BY v) r FROM alias QUALIFY alias.r <= 2 ORDER BY v
SELECT c1, row_number() over(partition BY c1 order by c2) as rk FROM VALUES ('a', 1), ('b', 2), ('b', 3), ('c', 4) AS t(c1, c2) qualify rk.add(1) > 2
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT list(c2) over(partition BY c1) AS "c3" QUALIFY "c3".len() = 1
# file: test/sql/binder/group_by_incremental_alias.test
# setup
create table my_functions as select 'my_name' as function_name
# query
select function_name as raw, replace(raw, '_', ' ') as prettier from my_functions group by all
# file: test/sql/binder/qualified_alias_method_call.test
# setup
CREATE TABLE test_alias(a INT, b VARCHAR)
# query
SELECT a, b, row_number() OVER (ORDER BY a)::VARCHAR AS rn FROM test_alias QUALIFY rn.length() > 0 ORDER BY a
SELECT a, b, row_number() OVER (ORDER BY a)::VARCHAR AS rn FROM test_alias QUALIFY alias.rn.length() > 0 ORDER BY a
# file: test/sql/binder/separate_schema_tables.test
# setup
CREATE SCHEMA IF NOT EXISTS s1
CREATE SCHEMA IF NOT EXISTS s2
CREATE SCHEMA IF NOT EXISTS s3
CREATE TABLE tbl(i INT)
CREATE TABLE s1.t AS SELECT 1 id, 's1.t' payload UNION ALL SELECT 10 id, 'AAA' payload
CREATE TABLE s2.t AS SELECT 1 id, 's2.t' payload2 UNION ALL SELECT 100 id, 'BBB' payload2
CREATE TABLE s3.t AS SELECT 1 id, 's3.t' payload3 UNION ALL SELECT 1000 id, 'CCC' payload3
# query
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t LEFT JOIN s2.t USING (id) LEFT JOIN s3.t USING (id) ORDER BY ALL
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t RIGHT JOIN s2.t USING (id) RIGHT JOIN s3.t USING (id) ORDER BY ALL
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t FULL OUTER JOIN s2.t USING (id) FULL OUTER JOIN s3.t USING (id) ORDER BY ALL
# file: test/sql/binder/table_alias_single_quotes.test
# setup
CREATE TABLE integers(i INTEGER)
# query
SELECT t.k FROM integers AS 't'('k') ORDER BY ALL
SELECT t.k FROM integers t('k') ORDER BY ALL
# file: test/sql/binder/test_alias_map_in_subquery.test
# setup
CREATE OR REPLACE TABLE tbl (example VARCHAR)
CREATE OR REPLACE TABLE testjson (example JSON)
CREATE OR REPLACE MACRO strip_null_value(jsonValue) AS ( WITH keys AS (SELECT UNNEST(json_keys(jsonValue)) AS k), nonNull AS ( SELECT keys.k, jsonValue->keys.k AS v FROM keys WHERE nullif(v, 'null') IS NOT NULL ) SELECT json_group_object(nonNull.k, nonNull.v) FROM nonNull )
# query
SELECT (WITH keys AS (SELECT unnest(json_keys(example)) AS k), nonNull AS ( SELECT keys.k, example->keys.k AS v FROM keys WHERE nullif(v, 'null') IS NOT NULL ) SELECT json_group_object(nonNull.k, nonNull.v) FROM nonNull ) FROM testjson
CREATE OR REPLACE MACRO strip_null_value(jsonValue) AS ( WITH keys AS (SELECT UNNEST(json_keys(jsonValue)) AS k), nonNull AS ( SELECT keys.k, jsonValue->keys.k AS v FROM keys WHERE nullif(v, 'null') IS NOT NULL ) SELECT json_group_object(nonNull.k, nonNull.v) FROM nonNull )
# file: test/sql/merge/merge_into.test
# setup
CREATE TABLE Stock(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
CREATE TABLE merge_distinct_target(tableticker VARCHAR NOT NULL, figi VARCHAR, cik VARCHAR, lastupdated DATE NOT NULL)
CREATE TABLE Sale(item_id int, volume int)
CREATE VIEW my_view AS SELECT 42 item_id
# query
FROM Stock ORDER BY item_id
FROM merge_distinct_target ORDER BY tableticker
# file: test/sql/merge/merge_into_by_source.test
# setup
CREATE TABLE Stock(item_id int, balance int)
# query
FROM Stock ORDER BY ALL
# file: test/sql/merge/merge_into_constraint.test
# setup
CREATE TABLE Stock(item_id int NOT NULL, balance int, CHECK (balance>0))
CREATE TABLE Items(item_id int NOT NULL, total_cost INTEGER, base_cost INTEGER, tax_cost INTEGER, CHECK (total_cost = base_cost + tax_cost))
# query
FROM Items
# file: test/sql/merge/merge_into_default.test
# setup
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
# query
FROM Stock
# file: test/sql/merge/merge_into_index.test
# setup
CREATE TABLE Accounts(id INTEGER, username VARCHAR PRIMARY KEY, favorite_numbers INT[])
# query
FROM Accounts WHERE username='user2'
# file: test/sql/merge/merge_into_join_as_filter.test
# setup
create table foo (bar integer)
create or replace table aaa (id int, status varchar, flag int, starttime datetime, endtime datetime)
# query
FROM foo
# file: test/sql/merge/merge_into_multiple_updates.test
# setup
CREATE TABLE Entry(type varchar, number int, text varchar, country VARCHAR, date DATE)
CREATE TABLE NewEntry(type varchar, number int, text varchar, country VARCHAR, date DATE)
# query
FROM Entry ORDER BY type
# file: test/sql/merge/merge_into_subquery.test
# setup
CREATE TABLE Totals(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
# query
FROM Totals ORDER BY ALL
# file: test/sql/merge/merge_into_subquery_condition.test
# setup
CREATE TABLE target(id INT PRIMARY KEY, val INT)
# query
FROM target ORDER BY id
# file: test/sql/subquery/exists/test_exists_union_by_name.test
# setup
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
# query
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
SELECT ( EXISTS( ( SELECT DISTINCT outer_alltypes."BIGINT", outer_alltypes."INT" FROM all_types inner_alltypes_1 WHERE inner_alltypes_1."BIGINT" GROUP BY NULL ) UNION BY NAME ( SELECT inner2."FLOAT" from all_types inner2 ) ) IS DISTINCT FROM outer_alltypes."struct" ) FROM all_types outer_alltypes GROUP BY ALL
# file: test/sql/subquery/lateral/lateral_binding_views.test
# query
from v1
# file: test/sql/subquery/lateral/lateral_qualify.test
# query
FROM (SELECT 42) t(x), (SELECT x, row_number() OVER () QUALIFY NULL)
FROM (SELECT 42) t(x), (SELECT x * 2 QUALIFY row_number() OVER () < 10)
# file: test/sql/aggregate/qualify/test_qualify.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE TABLE qt (a INTEGER, b CHAR(1), c INTEGER)
CREATE TABLE exam (student TEXT, subject TEXT, mark INTEGER)
CREATE TABLE power (plant TEXT, date DATE, mwh INTEGER)
CREATE TABLE tenk1 (unique1 int4, unique2 int4, two int4, four int4, ten int4, twenty int4, hundred int4, thousand int4, twothousand int4, fivethous int4, tenthous int4, odd int4, even int4, stringu1 varchar, stringu2 varchar, string4 varchar)
# query
SELECT * from qt QUALIFY row_number() over (PARTITION BY b ORDER BY c) = 1 ORDER BY b
SELECT a, b, c, row_number() over (PARTITION BY b ORDER BY c) as row_num FROM qt QUALIFY row_num = 1 ORDER BY b
SELECT * FROM exam QUALIFY rank() OVER (ORDER BY mark desc) = 4
SELECT * FROM exam QUALIFY rank() OVER (PARTITION BY student ORDER BY mark DESC) = 2 ORDER BY student
SELECT * FROM exam WINDOW w AS (ORDER BY mark) QUALIFY row_number() OVER w >= 1 AND (rank() OVER w) <= 2 ORDER BY student
SELECT * FROM exam QUALIFY first_value(mark) OVER (PARTITION BY student ORDER BY mark) >= 60 order by mark
SELECT * FROM exam QUALIFY last_value(mark) OVER (PARTITION BY student ORDER BY mark) >= 85 order by mark
SELECT * FROM power QUALIFY rank() OVER (PARTITION BY plant ORDER BY date DESC) = 2 ORDER BY plant
SELECT * FROM (SELECT plant, date, avg(mwh) OVER (PARTITION BY plant ORDER BY date ASC RANGE BETWEEN INTERVAL 3 DAYS PRECEDING AND INTERVAL 3 DAYS FOLLOWING) AS avgmwh FROM power ORDER BY plant, avgmwh DESC) QUALIFY row_number() OVER (PARTITION BY plant ORDER BY avgmwh DESC) = 1 ORDER BY plant
SELECT b, SUM(a) AS sum FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) >= 1 AND sum < 20 ORDER BY b
SELECT b, SUM(a) AS sum FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) > sum * 10
SELECT * FROM qt QUALIFY row_number() OVER (PARTITION BY b ORDER BY c) = (SELECT max(c) FROM qt) ORDER BY b
SELECT unique1 FROM tenk1 QUALIFY cast(cume_dist() OVER (PARTITION BY four ORDER BY ten)*10 as integer) = 5 order by four, ten
SELECT unique1 FROM tenk1 QUALIFY first_value(ten) OVER (PARTITION BY four ORDER BY ten) = 1 order by four, ten
SELECT unique1 FROM tenk1 qualify lead(ten * 2, 1, -1) OVER (PARTITION BY four ORDER BY ten) = -1 order by four, ten
SELECT * FROM ( SELECT b FROM test as t GROUP BY b QUALIFY rank() OVER (PARTITION BY t.b) = 1 ) QUALIFY row_number() OVER (PARTITION BY b) = 1 ORDER BY 1
SELECT * FROM test QUALIFY row_number() OVER (PARTITION BY test.b) = (SELECT max(a) FROM qt GROUP BY qt.b QUALIFY rank() OVER (PARTITION BY qt.b) = 1 order by qt.b limit 1)
SELECT * FROM exam WINDOW w AS (ORDER BY mark) QUALIFY row_number() OVER w = 1
SELECT * FROM exam QUALIFY row_number() OVER w = 1 WINDOW w AS (ORDER BY mark)
SELECT b, avg(a) AS avga FROM test GROUP BY b QUALIFY avga > 10
SELECT b FROM test QUALIFY avga() > 10
SELECT b, SUM(a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) > sum
# file: test/sql/aggregate/qualify/test_qualify_macro.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE MACRO plus1(x) AS (x + (SELECT COUNT(*) FROM (SELECT b, SUM(test.a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1)))
# query
CREATE MACRO plus1(x) AS (x + (SELECT COUNT(*) FROM (SELECT b, SUM(test.a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1)))
# file: test/sql/aggregate/qualify/test_qualify_view.test
# setup
CREATE SCHEMA test
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v AS SELECT * FROM test.t QUALIFY row_number() OVER (PARTITION BY b) = 1
# query
CREATE VIEW test.v AS SELECT * FROM test.t QUALIFY row_number() OVER (PARTITION BY b) = 1
SELECT b, SUM(a) FROM test.v GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1 ORDER BY ALL
# file: test/sql/aggregate/distinct/test_distinct_on_columns.test
# setup
CREATE TABLE grouped_table AS SELECT 1 id, 42 index1, 84 index2 UNION ALL SELECT 2, 42, 84 UNION ALL SELECT 3, 13, 14
CREATE TABLE a AS SELECT * FROM (VALUES (1,1),(2,2)) AS ta(ak, av)
CREATE TABLE b AS SELECT * FROM (VALUES (1,9),(2,8)) AS tb(bk, bv)
CREATE TABLE complex_table AS SELECT 1 a, 2 b, 3 c UNION ALL SELECT 1, 2, 4 UNION ALL SELECT 1, 3, 5
# query
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key')) *
SELECT DISTINCT ON (COLUMNS('index[0-9]')) * FROM grouped_table ORDER BY index1, index2, id
FROM (VALUES (1,1,2),(1,1,3),(2,1,4)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key'), v) key1, key2, v ORDER BY key1, v
FROM (VALUES (1,1,2),(1,1,3)) AS t(k1,k2,v) SELECT DISTINCT ON (COLUMNS('k')) * ORDER BY k1, k2
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key'), key1) * ORDER BY key1, key2
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS(* EXCLUDE (v))) * ORDER BY key1, key2
SELECT DISTINCT ON (COLUMNS('[0-9]')) * FROM grouped_table ORDER BY index1, index2, id
SELECT DISTINCT ON (COLUMNS('nonexistent')) * FROM grouped_table
FROM (VALUES (1,1,2),(1,1,3),(2,2,4)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key')) * ORDER BY key1, key2, v
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS('key1'), COLUMNS('key2')) * ORDER BY key1, key2
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS(* EXCLUDE (v) REPLACE (key1+1 AS key1))) * ORDER BY key1, key2
SELECT DISTINCT COLUMNS('key') FROM (VALUES(1,1,2),(1,1,3)) AS t(key1,key2,v)
FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) SELECT DISTINCT ON (COLUMNS(['key1', 'key2'])) * ORDER BY key1, key2
SELECT DISTINCT ON (COLUMNS('a'), COLUMNS('b')) * FROM complex_table ORDER BY a, b, c
SELECT DISTINCT ON (COLUMNS('key')) * FROM ( SELECT * FROM (VALUES (1,1,2),(1,1,3)) AS t(key1,key2,v) ) subq ORDER BY key1, key2
# file: test/sql/secrets/create_secret_filesystem_logging.test
# setup
CREATE PERSISTENT SECRET my_secret (TYPE HTTP)
# query
FROM duckdb_secrets()
# file: test/sql/secrets/secret_compatibility_http.test
# query
from duckdb_secrets()
# file: test/sql/catalog/comment_on.test
# setup
CREATE TYPE test_type AS int32
CREATE SEQUENCE test_sequence
CREATE TABLE test_table as SELECT 1 as test_table_column
CREATE VIEW test_view as SELECT 1 as test_view_column
CREATE MACRO test_macro(a, b) AS a + b
CREATE FUNCTION test_function(a, b) AS a + b
CREATE MACRO test_table_macro(a,b) as TABLE select a,b
CREATE INDEX test_index ON test_table using art(test_table_column)
# query
from test_table_macro(1,2)
# file: test/sql/catalog/test_querying_from_detached_catalog.test
# query
FROM db2.tbl
FROM db2.main.tbl
FROM db2.non_existent_table
# file: test/sql/catalog/function/attached_macro.test
# setup
CREATE TABLE tbl AS SELECT UNNEST([42, 43]) AS x
CREATE MACRO checksum_macro.checksum(table_name) AS TABLE SELECT bit_xor(md5_number(COLUMNS(*)::VARCHAR)) FROM query_table(table_name)
# query
CREATE MACRO checksum_macro.checksum(table_name) AS TABLE SELECT bit_xor(md5_number(COLUMNS(*)::VARCHAR)) FROM query_table(table_name)
# file: test/sql/catalog/function/query_function.test
# setup
CREATE TABLE tbl (a INT, b INT, c INT)
CREATE TABLE tbl_int AS SELECT 42
CREATE TABLE tbl_varchar AS SELECT 'duckdb'
CREATE TABLE tbl2_varchar AS SELECT '1?ch@racter$'
CREATE TABLE tbl_empty AS SELECT ''
CREATE TABLE tbl2 (a INT, b INT, c INT)
CREATE TABLE "(SELECT 17 + 25)"(i int)
# query
FROM query('SELECT 42 AS a')
FROM query('SELECT abs(-42)')
FROM query('SELECT 1, 2, 3')
FROM query('SELECT *, 1 + 2 FROM tbl')
FROM query_table('tbl_int')
FROM query_table(['tbl_int'])
FROM query_table(tbl)
FROM query_table([tbl, tbl2])
FROM query_table()
FROM query_table(NULL)
FROM query_table([])
FROM query_table([''])
FROM query_table('tbl_int', 'tbl_varchar', tbl2_varchar)
FROM query_table([tbl_int, tbl2])
FROM query_table(not_defined_tbl)
FROM query_table('(SELECT 17 + 25)')
FROM query_table("(SELECT 17 + 25)")
FROM query_table(SELECT 17 + 25)
FROM query_table("SELECT 4 + 2")
FROM query_table('SELECT 4 + 2')
FROM query_table(['tbl_int', 'tbl_varchar', 'tbl_empty', 'tbl2_varchar'], false)
from query_table([tbl_int, tbl_varchar, tbl_empty, tbl2_varchar], true)
FROM query_table(true)
FROM query_table(tbl2, true)
FROM query_table(['tbl_int', 'tbl_varchar', 'tbl_empty', '(select ''I am a subquery'')'], false)
# file: test/sql/catalog/function/test_drop_macro.test
# setup
create macro m() as table (select 42 i)
# query
from m()
# file: test/sql/catalog/function/test_macro_issue_14276.test
# setup
CREATE OR REPLACE MACRO extract_many(x, y) AS (SELECT struct_pack(*COLUMNS(lambda z: z in y)) FROM (SELECT unnest(x)))
# query
CREATE OR REPLACE MACRO extract_many(x, y) AS (SELECT struct_pack(*COLUMNS(z -> z in y)) FROM (SELECT unnest(x)))
CREATE OR REPLACE MACRO extract_many(x, y) AS (SELECT struct_pack(*COLUMNS(lambda z: z in y)) FROM (SELECT unnest(x)))
# file: test/sql/catalog/function/test_macro_type_overloads.test
# setup
create or replace type cool_string as varchar
create view v as select 42
create or replace macro m(i bigint := 42::integer) as table (select i as i)
# query
from m('ab')
from m(s := 'ab')
from m([42])
from m(s := [42])
from m(0::tinyint) union all from m(0::smallint) union all from m(0::integer) union all from m(0::bigint) union all from m(0::hugeint) union all from m(0::double)
from m(0::utinyint) union all from m(0::usmallint) union all from m(0::uinteger) union all from m(0::ubigint) union all from m(0::uhugeint) union all from m(0::float)
from m(0, 42)
from m(0, 42::float)
from m(0::float, 42)
from m('duck')
# file: test/sql/catalog/view/test_loosely_qualified_view_sql.test
# setup
create schema db1.s1
create schema db2.s1
create table db1.s1.t1 as select 1 col
create or replace view v1 as select (from s1.t1)
# query
from db1.v1
# file: test/sql/catalog/view/test_view_duplicate_columns.test
# setup
create or replace table basic_table_a as select 42 as column_a
create or replace table basic_table_b as select 37 as column_a
create or replace view duplicate_column_view as ( from basic_table_a t1 cross join basic_table_b t2 select t1.*, t2.* )
# query
from duplicate_column_view
# file: test/sql/upsert/upsert_default_values.test
# setup
create or replace table tbl ( a integer primary key DEFAULT 5, b integer )
# query
FROM tbl
# file: test/sql/pragma/test_metadata_info.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE db1.integers(i INTEGER, j INTEGER)
# query
FROM pragma_metadata_info()
FROM pragma_metadata_info('db1')
FROM pragma_metadata_info(NULL)
# file: test/sql/pragma/test_show_tables_temp_views.test
# setup
CREATE SCHEMA s1
CREATE TEMPORARY VIEW v1 AS SELECT 42 AS a
CREATE VIEW v2 AS SELECT 42 AS b
CREATE VIEW s1.v3 AS SELECT 42 AS c
# query
FROM v2
# file: test/sql/pragma/profiling/test_duckdb_profiling_settings_function.test
# query
SELECT * EXCLUDE(value, description) FROM duckdb_profiling_settings()
SELECT * EXCLUDE(description) FROM duckdb_profiling_settings()
# file: test/sql/function/autocomplete/create_table.test
# setup
CREATE SCHEMA abcdefgh
CREATE SCHEMA "SCHEMA"
# query
FROM sql_auto_complete(NULL)
# file: test/sql/function/autocomplete/select.test
# setup
CREATE TABLE my_table(my_column INTEGER)
CREATE TABLE MyTable(MyColumn Varchar)
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl QUALIFY row_number() OVER () ORD') LIMIT 1
# file: test/sql/function/autocomplete/sql_format.test
# query
SELECT duckdb_format_sql('SELECT list_transform([1, 2, 3], x -> x + 1)') = $$SELECT list_transform([1, 2, 3], x -> x + 1)$$
SELECT duckdb_format_sql('SELECT list_apply(list_filter([1,2,3,4,5], x -> x > 2), y -> y * 10)') = $$SELECT list_apply(list_filter([1, 2, 3, 4, 5], x -> x > 2), y -> y * 10)$$
SELECT duckdb_format_sql('SELECT [1, 2, 3].list_transform(x -> x + 1)') = $$SELECT [1, 2, 3].list_transform(x -> x + 1)$$
# file: test/sql/function/date/test_date_trunc.test
# setup
CREATE TABLE dates(d DATE, s VARCHAR)
CREATE TABLE timestamps(d TIMESTAMP, s VARCHAR)
# query
from values ('2024-01-15 15:30:00'::timestamp) as a(t) where date_trunc('day', t) = '2024-01-15 12:30:00'
# file: test/sql/function/list/flatten.test
# setup
CREATE TABLE nums AS SELECT range % 8 i, range j FROM range(16)
CREATE TABLE lists AS SELECT i % 4 i, list(j ORDER BY rowid) j FROM nums GROUP BY i
CREATE TABLE nested_lists AS SELECT i, list_sort(list(j ORDER BY rowid)) j FROM lists GROUP BY i ORDER BY i
# query
FROM nested_lists
# file: test/sql/function/list/list_distinct.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE integers (l integer[])
CREATE TABLE enums (e mood[])
CREATE TABLE wheretest (name VARCHAR, l INTEGER[])
CREATE TABLE all_types AS SELECT * FROM test_all_types()
# query
SELECT list_distinct([COLUMNS(*)]) FROM all_types
# file: test/sql/function/list/list_reverse.test
# setup
create or replace table tbl_big as select range(5000) as list
CREATE TABLE tbl (id INTEGER, list INTEGER[])
CREATE TABLE tbl2 (id INTEGER, list INTEGER[])
CREATE TABLE palindromes (s VARCHAR)
CREATE OR REPLACE TABLE integers AS SELECT LIST(i) AS i FROM range(1, 10, 1) t1(i)
CREATE OR REPLACE TABLE lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
# query
select list_reverse(list_reverse(columns(['int_array', 'varchar_array', 'nested_int_array', 'array_of_structs', 'timestamp_array', 'double_array', 'date_array', 'timestamptz_array']))) IS NOT DISTINCT FROM columns(['int_array', 'varchar_array', 'nested_int_array', 'array_of_structs', 'timestamp_array', 'double_array', 'date_array', 'timestamptz_array']) from test_all_types()
# file: test/sql/function/list/test_lambda_with_struct_aliases.test
# setup
CREATE TABLE addresses (i INT, b INT)
CREATE TABLE test (a VARCHAR[])
# query
SELECT COALESCE(*COLUMNS(lambda c: {'title': c}.title IN ('a', 'c'))) FROM (SELECT NULL, 2, 3) t(a, b, c)
# file: test/sql/function/list/lambdas/incorrect.test
# setup
CREATE TABLE incorrect_test (i INTEGER)
CREATE TABLE l_filter_test (l integer[])
CREATE TABLE tbl AS SELECT {'a': 10} AS s
CREATE TABLE nested_list(i INT[][], other INT[])
CREATE TABLE map_tbl(m MAP(INTEGER, INTEGER))
CREATE TABLE dummy_tbl (y INT)
CREATE OR REPLACE FUNCTION transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
# query
SELECT list_reduce([1], x -> x, 3)
SELECT list_reduce([True], x -> x, x -> x)
SELECT [split('01:08:22', ':'), x -> CAST (x AS INTEGER)]
select list_apply(i, x -> x * 3 + 2 / zz) from (values (list_value(1, 2, 3))) tbl(i)
select x -> x + 1 from (values (list_value(1, 2, 3))) tbl(i)
SELECT list_apply(i, a.x -> x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
select list_apply(i, x -> x + 1 AND y + 1) from (values (list_value(1, 2, 3))) tbl(i)
SELECT list_transform([1, 2], (x, y, z) -> x + y + z)
SELECT list_filter([1, 2], (x, y, z) -> x >= y AND y >= z)
SELECT cos(x -> x + 1)
SELECT cos([1], x -> x + 1)
CREATE TABLE lambda_check (i BIGINT[], CHECK (list_filter(i, x -> x % 2 = 0) == []))
CREATE TABLE lambda_check (i BIGINT[], CHECK (list_transform(i, x -> x % 2) == []))
CREATE TABLE lambda_check ( i BIGINT[], j BIGINT[], CHECK ((list_apply(i, x -> list_count(list_filter(j, y -> y%2=0)) + x)) == []))
CREATE TABLE unit2( price INTEGER[], total_price INTEGER GENERATED ALWAYS AS (list_transform(price, x -> x + 1)) VIRTUAL )
SELECT list_transform(UNNEST(s), x -> UNNEST(x)) FROM tbl
SELECT list_transform(i, x -> UNNEST(x)) FROM nested_list
SELECT list_transform(i, x -> UNNEST(other)) FROM nested_list
SELECT list_transform(map_entries(m), x -> UNNEST(range(x.value))) FROM map_tbl
CREATE OR REPLACE function transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), lambda i: lst[i][j] ) ) )
CREATE OR REPLACE FUNCTION transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
# file: test/sql/function/list/lambdas/lambdas_and_functions.test
# setup
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
# query
FROM demo(3, 0)
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
FROM demo(3, {'a': 2})
# file: test/sql/function/list/lambdas/lambdas_and_group_by.test
# setup
CREATE TABLE tbl (tag_product VARCHAR)
CREATE TABLE uniform_purchase_forecast AS SELECT 'gold' AS color, 10 AS forecast UNION ALL SELECT 'blue', 15 UNION ALL SELECT 'red', 300
# query
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(lambda x: x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, lambda x: x)
FROM test_all_types() GROUP BY ALL HAVING array_intersect(NULL, NULL)
# file: test/sql/function/list/lambdas/transform.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE transformed_lists (g integer, l integer[])
CREATE TABLE corr_test (n integer, l integer[], g integer)
create table test(a int, b int)
# query
select list_transform(bb, lambda x: [x, b]), bb, b from (select list(b) over wind as bb, first(b) over wind as b from test window wind as (order by a asc, b asc rows between 4 preceding and current row) qualify row_number() over wind >4)
# file: test/sql/function/list/lambdas/arrow/expression_iterator_cases_deprecated.test
# setup
CREATE TABLE my_window (l integer[], g integer, o integer)
CREATE MACRO list_contains_macro(x, y) AS (SELECT list_contains(x, y))
# query
SELECT list_transform([10], x -> sum(1) + x)
SELECT list_filter([10], x -> sum(1) > 0)
SELECT list_transform([NULL, DATE '1992-09-20', DATE '2021-09-20'], elem -> extract('year' FROM elem) BETWEEN 2000 AND 2022)
SELECT list_filter([NULL, DATE '1992-09-20', DATE '2021-09-20'], elem -> extract('year' FROM elem) BETWEEN 2000 AND 2022)
SELECT list_transform(['hello', 'duck', 'sunshine'], str -> CASE WHEN str LIKE '%e%' THEN 'e' ELSE 'other' END)
SELECT list_filter(['hello', 'duck', 'sunshine'], str -> (CASE WHEN str LIKE '%e%' THEN 'e' ELSE 'other' END) LIKE 'e')
SELECT list_transform([2.0::DOUBLE], x -> x::INTEGER)
SELECT list_filter([2], x -> x::DOUBLE == 2)
SELECT list_transform([2.4, NULL, -4.7], x -> x != 10.4)
SELECT list_filter([2.4, NULL, -4.7], x -> x != -4.7)
SELECT list_transform([True, False, NULL], x -> x AND true)
SELECT list_filter([True, False, NULL], x -> x AND true)
SELECT list_transform([TIMESTAMP '1992-03-22', TIMESTAMP '209-03-22', TIMESTAMP '1700-03-22'], x -> century(x))
SELECT list_filter([TIMESTAMP '1992-03-22', TIMESTAMP '209-03-22', TIMESTAMP '1700-03-22'], x -> century(x) > 16)
SELECT list_transform([2], x -> x + x)
SELECT list_filter([2], x -> x + x = 4)
SELECT list_transform([2], x -> (SELECT 1 - x) * x)
SELECT list_filter([2], x -> (SELECT 1 - x) * x > 2)
SELECT list_filter([[1, 2, 1], [1, 2, 3], [1, 1, 1]], x -> list_contains_macro(x, 3))
SELECT list_transform([1], x -> x = UNNEST([1]))
SELECT list_filter([1], x -> x = UNNEST([1]))
SELECT list(list_transform(l, e -> e + 1)) OVER (PARTITION BY g ORDER BY o) FROM my_window ORDER BY ALL
# file: test/sql/function/list/lambdas/arrow/filter_deprecated.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE empty_lists (l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE corr_test (n integer, l varchar[], g integer)
create table lambdas AS SELECT [5,6] AS col1, [4,8] AS col2
# query
SELECT [1] AS l, list_filter([1], l -> l > 1)
SELECT list_filter(NULL, x -> x > 1)
SELECT list_filter([True], x -> x)
SELECT list_filter(['duck', 'a', 'ö'], duck -> contains(concat(duck, 'DB'), 'duck'))
SELECT list_filter([1, 2, 3], x -> x % 2 = 0)
SELECT list_filter([], x -> x > 1)
SELECT list_filter([1, NULL, -2, NULL], x -> x % 2 != 0)
SELECT list_filter([5, -6, NULL, 7], x -> x > 0)
SELECT list_filter([5, NULL, 7, NULL], x -> x IS NOT NULL)
SELECT list_filter(l, x -> x + 1 <= 2) FROM lists
SELECT list_filter(l, x -> x <= n) FROM lists
SELECT list_filter(l, x -> x IS NOT NULL) FROM lists
SELECT list_filter(['x', 'abc', 'z'], x -> contains(x || '0', 'a'))
SELECT list_transform([[1, 3], [2, 3, 1], [2, 4, 2]], x -> list_filter(x, y -> y <= 2))
SELECT list_concat(list_filter([42, -42, 8, -5, 2], elem -> elem > 0)::varchar[], list_filter(['enjoy', 'life', 'to', 'the', 'fullest'], str -> str ILIKE '%e%'))
SELECT array_filter([1, NULL], arr_elem -> arr_elem < 4)
SELECT list_filter(l, x -> x > 0) FROM empty_lists
SELECT g, list_count(list_filter(l, x -> x % 2 = 0)) FROM large_lists ORDER BY g
SELECT n FROM corr_test WHERE list_count(list_filter(l, elem -> length(elem) >= n)) >= n
SELECT ct.n FROM corr_test ct WHERE list_count(ct.l) < (SELECT list_count(list_filter(list_concat(list(c.n)::varchar[], ct.l), a -> length(a) >= 1)) FROM corr_test c GROUP BY c.g) ORDER BY ct.n
SELECT (SELECT list_filter(l, elem -> length(elem) >= 1)) FROM corr_test
SELECT (SELECT list_filter(l, elem -> length(elem) >= n)) FROM corr_test
SELECT (SELECT (SELECT (SELECT list_filter(l, elem -> length(elem) >= 1)))) FROM corr_test
SELECT list_filter([1, 2, 3, 4, 5, 6, 7, 8, 9], x -> x > #1) FROM range(10)
SELECT list_apply(col1, x -> list_filter(col2, y -> y)) from lambdas
# file: test/sql/function/list/lambdas/arrow/lambda_scope_deprecated.test
# setup
CREATE TABLE t1 AS SELECT [1, 2, 3] AS x
CREATE TABLE t2 AS SELECT [[1], [2], [3]] AS x
CREATE TABLE l_test (l integer[])
CREATE TABLE l_filter_test (l integer[])
CREATE TABLE qualified_tbl (x INTEGER[])
CREATE TABLE tbl_qualified AS SELECT 42 AS x
# query
SELECT list_apply(['hello'], x -> x) FROM t1
SELECT list_transform([[1], [2], [3]], x -> x[1]) FROM t2
SELECT l, list_transform(l, l -> l + 1) FROM l_test
SELECT l, list_filter(l, l -> l > 1) FROM l_filter_test
SELECT list_apply(i, a.x -> a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
SELECT list_transform(qualified_tbl.x, x -> (qualified_tbl.x)[1] + 1 + x) FROM qualified_tbl
SELECT list_transform(qualified_tbl.x, qualified_tbl.x -> qualified_tbl.x + 1) FROM qualified_tbl
SELECT list_transform([1, 2], x -> list_transform([3, 4], x -> x))
SELECT list_has_all(list_transform(['a'], variable_has_all -> variable_has_all), ['b']) AS list_transform_result
SELECT list_has_any(['b'], list_transform(['a'], variable_has_any -> variable_has_any)) AS list_transform_result
SELECT x, list_transform([1], x -> x) FROM tbl_qualified
SELECT list_transform([1,2,3], sqrt(xxx.z) -> xxx.z + 1) AS l
SELECT list_reduce([1, 2, 3, 4], x *++++++++* y -> x - y) AS l
# file: test/sql/function/list/lambdas/arrow/lambdas_and_functions_deprecated.test
# setup
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> (z -> 'a')) AS row )
# query
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> 0 + z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> (z -> 'a')) AS row )
# file: test/sql/function/list/lambdas/arrow/lambdas_and_group_by_deprecated.test
# setup
CREATE TABLE tbl (tag_product VARCHAR)
CREATE TABLE uniform_purchase_forecast AS SELECT 'gold' AS color, 10 AS forecast UNION ALL SELECT 'blue', 15 UNION ALL SELECT 'red', 300
# query
SELECT tag_product, list_aggr(list_transform( string_split(tag_product, ' '), word -> lower(word)), 'string_agg', ',') AS tag_material, FROM tbl GROUP BY tag_product ORDER BY ALL
SELECT 1, list_transform([5, 4, 3], x -> x + 1) AS lst GROUP BY 1
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(x -> x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, x -> x)
SELECT x FROM (VALUES (42)) t(x) GROUP BY x HAVING list_filter(NULL, lambda_param -> lambda_param = 1)
# file: test/sql/function/list/lambdas/arrow/lambdas_and_macros_deprecated.test
# setup
create table test as select range i from range(3)
CREATE MACRO list_contains_macro(x, y) AS (list_contains(x, y))
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, x -> x + num))
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, a -> x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, (x, y) -> x + y + num))
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, (x, y) -> list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, x -> x + y + z))
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], x -> 0))
# query
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, x -> x + num))
SELECT list_filter([[1, 2], NULL, [3], [4, NULL]], f -> list_count(macro_with_lambda(f, 2)) > 1)
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, a -> x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, (x, y) -> x + y + num))
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, (x, y) -> list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, x -> x + y + z))
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], x -> 0))
# file: test/sql/function/list/lambdas/arrow/list_comprehension_deprecated.test
# setup
CREATE TABLE fruit_tbl AS SELECT ['apple', 'banana', 'cherry', 'kiwi', 'mango'] fruits
CREATE TABLE word_tbl AS SELECT ['goodbye', 'cruel', 'world'] words
# query
SELECT list_transform(list_filter([0, 1, 2, 3, 4, 5], x -> x % 2 = 0), y -> y * y)
SELECT list_filter(list_filter([2, 4, 3, 1, 20, 10, 3, 30], x -> x % 2 == 0), y -> y % 5 == 0)
SELECT list_filter(['apple', 'banana', 'cherry', 'kiwi', 'mango'], fruit -> contains(fruit, 'a'))
SELECT list_transform([[1, NULL, 2], [3, NULL]], a -> list_filter(a, x -> x IS NOT NULL))
# file: test/sql/function/list/lambdas/arrow/reduce_deprecated.test
# setup
CREATE TABLE t1 (a varchar[])
CREATE TABLE right_only (v varchar[], i int)
CREATE TABLE nested (n integer[][][])
CREATE table where_clause (a int[])
CREATE TABLE t_struct (s STRUCT(v VARCHAR, i INTEGER)[])
CREATE OR REPLACE TABLE df(s STRUCT(a INT, b INT)[])
# query
SELECT list_reduce([1, 2, 3], (x, y) -> x + y)
SELECT list_reduce([1, 2, 3], (x, y) -> x * y)
SELECT list_reduce([100, 10, 1], (x, y, i) -> x - y - i)
SELECT list_reduce([1, 2, 3], (x, y) -> y - x)
SELECT list_reduce([1, 2, 3], (x, y) -> x - y)
SELECT list_reduce([1, 2, 3], (x, y, i) -> x + y + i)
SELECT list_reduce([NULL], (x, y, i) -> x + y + i)
SELECT list_reduce(NULL, (x, y, i) -> x + y + i)
SELECT list_reduce(['Once', 'upon', 'a', 'time'], (x, y) -> x || ' ' || y)
SELECT list_reduce(['a', 'b', 'c', 'd'], (x, y, i) -> x || ' - ' || CAST(i AS VARCHAR) || ' - ' || y)
SELECT list_reduce([], (x, y, i) -> x + y + i)
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y)::VARCHAR || 'please work')
SELECT list_reduce([1, 2], (x) -> x)
SELECT list_reduce(a, (x, y) -> x + y) FROM t1
SELECT list_reduce(a, (x, y, i) -> x + y + i) FROM t1
SELECT list_reduce(a, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce(v, (x, y) -> y[i]) FROM right_only
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([4, 5, 6], (a, b) -> x + y + a + b))
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b))
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([4, 5, 6], (a, b, a_i) -> x + y + a + b + x_i + a_i))
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i))
SELECT list_reduce([[10, 20], [30, 40], [50, 60]], (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (n, o) -> n + o)))
SELECT list_reduce([[1,2,3], [4,5,6], [7,8,9]], (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (n, o) -> n + o)))
SELECT list_reduce([[10, 20], [30, 40], NULL, [NULL, 60], NULL], (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (n, o) -> n + o)))
SELECT list_reduce(['a', 'b', 'c', 'd'], (x, y) -> list_reduce(['1', '2', '3', '4'], (a, b) -> x || y || a || b))
# file: test/sql/function/list/lambdas/arrow/reduce_initial_deprecated.test
# setup
CREATE TABLE t1 (l varchar[], initial varchar)
CREATE TABLE right_only (v varchar[], i int)
CREATE TABLE nested (n integer[][][], initial integer[][])
CREATE TABLE t_struct (s STRUCT(v VARCHAR, i INTEGER)[], initial STRUCT(v VARCHAR, i INTEGER))
CREATE OR REPLACE TABLE df(s STRUCT(a INT, b INT)[], initial STRUCT(a INT, b INT))
CREATE table where_clause (a int[], initial integer)
# query
SELECT list_reduce([1, 2, 3], (x, y) -> x + y, 100)
SELECT list_reduce([1, 2, 3], (x, y) -> x * y, -1)
SELECT list_reduce([100, 10, 1], (x, y, i) -> x - y - i, 1000)
SELECT list_reduce([1, 2, 3], (x, y) -> y - x, -1)
SELECT list_reduce([1, 2, 3], (x, y) -> x - y, 10)
SELECT list_reduce([1, 2, 3], (x, y, i) -> x + y + i, -1)
SELECT list_reduce([1, 2, 3], (x, y) -> x + y, NULL)
SELECT list_reduce([NULL], (x, y, i) -> x + y + i, 100)
SELECT list_reduce(NULL, (x, y, i) -> x + y + i, 100)
SELECT list_reduce(['Once', 'upon', 'a', 'time'], (x, y) -> x || ' ' || y, '-->')
SELECT list_reduce([], (x, y) -> x + y, 100)
SELECT list_reduce(['a', 'b', 'c'], (x, y) -> x || y, NULL)
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y), 'i dare you to cast me')
SELECT list_reduce([1, 2], (x) -> x, 100)
SELECT list_reduce(l, (x, y) -> x + y, initial) FROM t1
SELECT list_reduce(l, (x, y, i) -> x + y + i, initial) FROM t1
SELECT list_reduce(l, (x, y) -> x + y) FROM t1
SELECT list_reduce(l, (x, y) -> x || ' ' || y, initial) FROM t1
SELECT list_reduce(l, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([4, 5, 6], (a, b) -> x + y + a + b, 100), 1000)
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b), 1000)
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([4, 5, 6], (a, b, a_i) -> x + y + a + b + x_i + a_i, 100), 1000)
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i), 1000)
SELECT list_reduce([[10, 20], [30, 40], [50, 60]], (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (n, o) -> n + o)), [100, 200])
SELECT list_reduce([[1,2,3], [4,5,6], [7,8,9]], (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (n, o) -> n + o)), [100])
# file: test/sql/function/list/lambdas/arrow/rhs_parameters_deprecated.test
# setup
CREATE TABLE lists (i integer, v varchar[])
create table no_overwrite as select [range, range + 1] l from range(3)
# query
SELECT list_apply([1,2], x -> list_apply([3,4], y -> {'x': x, 'y': y})) AS bug
select list_transform([1,2], x -> list_transform([3,4], y -> x + y))
select list_transform([1,2], x -> list_transform([3,4], y -> list_transform([5,6], z -> z + y + x)))
select list_transform([1,2,3,4], x -> list_filter([4,5,1,2,3,3,3,5,1,4], y -> y != x))
select list_transform([[2, 4, 6]], x -> list_transform(x, y -> list_sum([y] || x)))
SELECT list_apply(range(5), x -> {x:x, w:list_filter(range(5), y -> abs(y-x) < 2)})
SELECT list_apply(range(8), x -> list_aggr(list_apply(range(8), y -> list_element('▁▂▃▄▅▆▇█', 1+abs(y-x))), 'string_agg', ''))
SELECT list_transform(v, x -> list_transform(v, y -> x || y)) FROM lists
SELECT list_transform(v, x -> list_transform(v, y -> list_transform(v, z -> x || y || z))) FROM lists
SELECT list_transform(v, x -> [list_transform([':-)'], y -> x || y || '-#lambdaLove')] || list_filter(list_transform(['B-)'], k -> [k] || [x]), j -> list_contains(j, 'a') or list_contains(j, 'duck'))) FROM lists
# file: test/sql/function/list/lambdas/arrow/storage_deprecated.test
# setup
CREATE MACRO my_transform(list) AS list_transform(list, x -> x * x)
CREATE MACRO my_filter(list) AS list_filter(list, x -> x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, (x, y) -> x + y)
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, elem -> list_reduce(list_transform(elem, x -> x + 1), (x, y) -> x + y) > 42)
# query
CREATE MACRO my_transform(list) AS list_transform(list, x -> x * x)
CREATE MACRO my_filter(list) AS list_filter(list, x -> x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, (x, y) -> x + y)
# file: test/sql/function/list/lambdas/arrow/transform_deprecated.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE transformed_lists (g integer, l integer[])
CREATE TABLE corr_test (n integer, l integer[], g integer)
create table test(a int, b int)
# query
select list_transform(bb, x->[x,b]), bb, b from (select list(b) over wind as bb, first(b) over wind as b from test window wind as (order by a asc, b asc rows between 4 preceding and current row) qualify row_number() over wind >4)
# file: test/sql/function/variant/variant_extract_try_cast.test
# setup
create table tbl(col VARIANT)
# query
from tbl order by all
# file: test/sql/function/variant/variant_typeof.test
# setup
CREATE TABLE T (v VARIANT)
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
# query
select variant_typeof(struct_pack(*COLUMNS(*))::VARIANT) test from test_all_types()
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
# file: test/sql/function/generic/cast_to_type.test
# setup
create table tbl(i int, v varchar)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN cast_to_type(nullif(trim(s::VARCHAR), ''), s) ELSE s END
# query
SELECT try_trim_null(COLUMNS(*)) FROM tbl
# file: test/sql/function/string/hex.test
# query
SELECT to_hex(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_hex(to_hex(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
SELECT to_binary(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_binary(to_binary(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
# file: test/sql/function/timestamp/test_icu_makedate.test
# setup
CREATE TABLE timestamps(ts TIMESTAMPTZ)
CREATE TABLE timezones AS (SELECT mm, tz FROM (VALUES (1, 'America/New_York'), (2, 'America/Los_Angeles'), (3, 'Europe/Rome'), (4, 'Asia/Kathmandu'), (5, 'Canada/Newfoundland'), (7, 'Pacific/Auckland'), (8, 'Asia/Hong_Kong'), (12, 'US/Hawaii') ) tbl(mm, tz) )
CREATE TABLE timeparts AS ( SELECT ts, yeartz(ts) yyyy, month(ts) mm, day(ts) dd, hour(ts) hr, minute(ts) mn, microsecond(ts) / 1000000.0 as ss, tz FROM timestamps t LEFT JOIN timezones z ON (month(t.ts) = z.mm) ORDER BY ts )
CREATE MACRO yeartz(ts) AS year(ts::TIMESTAMPTZ) * (CASE WHEN ERA(ts::TIMESTAMPTZ) > 0 THEN 1 ELSE -1 END)
# query
WITH all_types AS ( select * exclude(small_enum, medium_enum, large_enum) from test_all_types() ) SELECT make_timestamptz( CAST(century(CAST(a."interval" AS INTERVAL)) AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(txid_current() AS BIGINT), 'UTC') FROM all_types a
# file: test/sql/parser/columns_aliases.test
# setup
CREATE TABLE integers AS SELECT 42 i, 84 j UNION ALL SELECT 13, 14
CREATE TABLE numerics AS SELECT 42 a42, 84 b84, 126 c126, 1000 d
CREATE TABLE tbl ( price INTEGER, amount_sold INTEGER, total_profit AS (price * amount_sold), )
create table a as select 42 as i, 80 as j
create table b as select 43 as i, 84 as k
create table c as select 44 as i, 84 as l
# query
SELECT i, j FROM (SELECT COLUMNS(*)::VARCHAR FROM integers)
SELECT min_i, min_j, max_i, max_j FROM (SELECT MIN(COLUMNS(*)) AS "min_\0", MAX(COLUMNS(*)) AS "max_\0" FROM integers)
SELECT min_a, min_b, min_c FROM (SELECT MIN(COLUMNS('([a-z])\d+')) AS "min_\1" FROM numerics)
SELECT min_, "min__1", "min__2" FROM (SELECT MIN(COLUMNS('([a-z])\d+')) AS "min_\2" FROM numerics)
SELECT "min_\a\", "min_\b\", "min_\c\" FROM (SELECT MIN(COLUMNS('([a-z])\d+')) AS "min_\\\1\\" FROM numerics)
SELECT "a42aa", "b84bb", "c126cc" FROM (SELECT MIN(COLUMNS('([a-z])(\d+)')) AS "\1\2\1\1" FROM numerics)
SELECT MIN(COLUMNS('([a-z])\d+')) AS "\" FROM numerics
SELECT MIN(COLUMNS('([a-z])\d+')) AS "\a" FROM numerics
SELECT MIN(COLUMNS(*)) AS "min_\1" FROM numerics
SELECT price, amount_sold, total_profit FROM (SELECT COLUMNS(*)::VARCHAR FROM tbl)
SELECT varchar_price, varchar_amount_sold, varchar_total_profit FROM (SELECT COLUMNS(*)::VARCHAR AS "varchar_\0" FROM tbl)
select i, j, k from (select columns(*)::VARCHAR from a full outer join b using (i)) order by 1
select i, j, k, l from (select columns(*)::VARCHAR from a full outer join b using (i) full outer join c using (i)) order by 1
# file: test/sql/parser/columns_issue9867.test
# setup
CREATE TABLE df1 AS SELECT UNNEST(['K0', 'K1', 'K2', 'K3', 'K4', 'K5']) AS key, UNNEST([11, 12, 13, 14, 15, 16]) AS A, UNNEST([21, 22, 23, 24, 25, 26]) AS B
CREATE TABLE df2 AS SELECT UNNEST(['K0', 'K2', 'K5']) AS key, UNNEST([2, 3, 5]) AS C
# query
select sin(columns(df1.* exclude (key))) from df1 join df2 using(key)
select sin(columns(dfxx.* exclude (key))) from df1 join df2 using(key)
# file: test/sql/parser/from_first.test
# setup
CREATE TABLE integers(i INTEGER)
# query
FROM integers SELECT i + 1
FROM integers LIMIT 2
FROM integers WHERE i IS NOT NULL
FROM integers ORDER BY i DESC NULLS FIRST
FROM integers SELECT DISTINCT i%2 WHERE i>0 ORDER BY ALL
FROM integers SELECT i%2 AS g, SUM(i) sum GROUP BY g HAVING sum IS NOT NULL ORDER BY ALL
FROM integers JOIN integers i2 USING (i)
FROM integers i1, integers i2 SELECT COUNT(*)
# file: test/sql/parser/join_alias.test
# query
from ( (values (1), (2)) as t1 (a) cross join (values (3), (4)) as t2 (b) ) as t(x, y, z)
# file: test/sql/parser/star_expression.test
# setup
CREATE TABLE integers AS SELECT 42 i, 84 j UNION ALL SELECT 13, 14
# query
SELECT * FROM integers WHERE COLUMNS(*) IS NULL ORDER BY ALL
SELECT * FROM integers GROUP BY COLUMNS(*)
SELECT * FROM integers GROUP BY i HAVING COLUMNS(*)>42
FROM read_csv(*, *)
# file: test/sql/parser/test_columns.test
# setup
CREATE TABLE integers AS SELECT 42 i, 84 j UNION ALL SELECT 13, 14
CREATE TABLE grouped_table AS SELECT 1 id, 42 index1, 84 index2 UNION ALL SELECT 2, 13, 14
# query
SELECT COLUMNS(*) FROM integers
SELECT MIN(COLUMNS(*)), MAX(COLUMNS(*)) FROM integers
SELECT MIN(COLUMNS(* EXCLUDE (j))), MAX(COLUMNS(* EXCLUDE (i))) FROM integers
SELECT MIN(COLUMNS(* REPLACE (i+j AS i))) FROM integers
SELECT COLUMNS(*) + 1 FROM integers
SELECT COLUMNS(*) + COLUMNS(*) FROM integers
SELECT COLUMNS('indxe.*') FROM grouped_table
SELECT id, MIN(COLUMNS('index[0-9]')) FROM grouped_table GROUP BY all ORDER BY ALL
SELECT id, MIN(COLUMNS('[0-9]')) FROM grouped_table GROUP BY all ORDER BY ALL
SELECT id, MIN(COLUMNS('xxx')) FROM grouped_table GROUP BY all
SELECT MIN(COLUMNS('xxx')) FROM grouped_table
SELECT MIN(COLUMNS('[asdadd')) FROM grouped_table
SELECT COLUMNS(*) + COLUMNS(* EXCLUDE(j)) FROM integers
SELECT (SELECT COLUMNS(*)) FROM integers
SELECT columns(['a', null]) FROM values (42) t(a)
SELECT * FROM grouped_table ORDER BY COLUMNS('index[0-9]')
SELECT * FROM grouped_table ORDER BY COLUMNS(*)
# file: test/sql/parser/test_columns_lists.test
# setup
CREATE TABLE integers AS SELECT 42 i, 84 j UNION ALL SELECT 13, 14
# query
SELECT COLUMNS([x for x in *]) FROM integers
SELECT COLUMNS([x for x in (*) if x <> 'i']) FROM integers
SELECT COLUMNS(lambda x: x <> 'i') FROM integers
SELECT COLUMNS([x for x in (*) if x SIMILAR TO 'i']) FROM integers
SELECT COLUMNS(['i', 'i']) FROM integers
SELECT COLUMNS(list_concat(['i'], ['i'])) FROM integers
SELECT COLUMNS([x for x in (* EXCLUDE (i))]) FROM integers
SELECT COLUMNS(['i']) + COLUMNS(['i']) FROM integers
SELECT COLUMNS([i, j]) FROM integers
SELECT COLUMNS([x for x in COLUMNS(*)]) FROM integers
SELECT COLUMNS(COLUMNS(*)) FROM integers
SELECT COLUMNS([x for x in (*) if x = 'k']) FROM integers
SELECT COLUMNS(['k']) FROM integers
SELECT COLUMNS([x for x in (*) if x LIKE 'i']) FROM integers i1 JOIN integers i2 USING (i)
SELECT COLUMNS([x for x in (*) if x LIKE 'i']) FROM integers i1 JOIN integers i2 ON (i1.i=i2.i)
SELECT COLUMNS([43]) FROM integers
SELECT COLUMNS([NULL]) FROM integers
SELECT COLUMNS([]::VARCHAR[]) FROM integers
SELECT COLUMNS(NULL::VARCHAR[]) FROM integers
SELECT COLUMNS(NULL::VARCHAR) FROM integers
SELECT COLUMNS(['i']) + COLUMNS(['j']) FROM integers
SELECT COLUMNS([x for x in (* REPLACE (i AS i))]) FROM integers
# file: test/sql/parser/test_columns_order.test
# setup
CREATE TABLE tbl(col1 INTEGER, col2 INTEGER, col3 INTEGER)
# query
SELECT * FROM tbl ORDER BY COLUMNS('col1|col3')
SELECT * FROM tbl ORDER BY COLUMNS('col2|col3')
SELECT * FROM tbl ORDER BY COLUMNS('col2|col3') DESC
SELECT * FROM tbl ORDER BY COLUMNS('col2') DESC, COLUMNS('col3') ASC
SELECT * FROM tbl ORDER BY COLUMNS(lambda x: x[-1] IN ('2', '3'))
FROM tbl UNION FROM tbl ORDER BY COLUMNS('col2|col3') DESC
SELECT * FROM tbl ORDER BY COLUMNS('xxxx')
# file: test/sql/parser/test_columns_prepared.test
# setup
create or replace table my_table as select 'test1' as column1, 1 as column2, 'quack' as column3 union all select 'test2' as column1, 2 as column2, 'quacks' as column3 union all select 'test3' as column1, 3 as column2, 'quacking' as column3
# query
prepare v1 as select COLUMNS(?) from my_table
# file: test/sql/parser/test_columns_unpacked.test
# setup
create table contains_test as select '123abc234' a, 4 b, 'abc' c
create table sales as from ( values (150, '2017/06/12'::DATE, 3), (125, '2017/08/29'::DATE, 2), (175, '2017/06/12'::DATE, 4), ) t(amount, date, priority)
create table data AS ( SELECT * FROM (VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Alice'), (4, 'Carol') ) AS t(id, name) )
# query
select COALESCE(*COLUMNS(*)) from (select NULL, 2, 3) t(a, b, c)
select column_name from (describe select COALESCE(*COLUMNS(*)) from (select NULL, 2, 3) t(a, b, c))
select column_name from (describe select COALESCE(*COLUMNS(*)) as a from (select NULL, 2, 3) t(a, b, c))
select contains(*COLUMNS('[a|c]')) from contains_test
select COLUMNS('[a|c]') from contains_test
select *COLUMNS('[a|c]') from contains_test
select first(amount ORDER BY *COLUMNS('date|priority') ASC) from sales
select COALESCE(*COLUMNS(lambda c: c in ('a', 'c'))) from (select NULL, 2, 3) t(a, b, c)
select 2 in (*COLUMNS(*)) from (select 1, 2, 3) t(a, b, c)
from (VALUES (1, 2, 3), (2, 3, 0), (0, 0, 1)) tbl(a, b, c) where 1 IN (*COLUMNS(*))
select struct_pack(*COLUMNS(*)) from data
SELECT COLUMNS(*COLUMNS(*)) FROM (VALUES ('test'))
SELECT *COLUMNS(COLUMNS(*)) FROM (VALUES ('test'))
select COLUMNS(*), struct_pack(COLUMNS(['id'])) from data
select struct_pack(struct_pack(*COLUMNS(['id']), struct_pack(*COLUMNS(['name'])))) from data
select struct_pack( b := struct_pack(*COLUMNS(['id'])), a := struct_pack(*COLUMNS(['id'])) ) from data
select struct_pack(*COLUMNS('id')) a, struct_pack(*COLUMNS('name')) from data
select CONCAT(*COLUMNS(*), *COLUMNS(*)) from data
select COLUMNS(lambda col: *COLUMNS('id')) from data
select *COLUMNS(lambda col: *COLUMNS(*)) from data
with integers as ( SELECT * FROM (VALUES (42, 31), (85, 76), ) as t(a, b) ) select *COLUMNS(*) + 42 from integers
with integers as ( SELECT * FROM (VALUES (42, 31), (85, 76), ) as t(a, b) ) select *COLUMNS('a') + 42 from integers
with integers as ( select * FROM (VALUES (21, 42), (1337, 7331) ) as t(a, b) ) select [(UNPACK(a + COLUMNS(['a', 'b'])))] from integers
select [ UNPACK([ UNPACK(COLUMNS(*)), a + b ]) ] from ( select 42 a, 21 b )
select [UNPACK(COLUMNS(*)::VARCHAR)] from ( select 21::INTEGER a, True::BOOL b, 0.1234::DOUBLE c )
# file: test/sql/parser/test_columns_where.test
# setup
CREATE TABLE tbl(col1 INTEGER, col2 INTEGER, col3 INTEGER)
# query
SELECT * FROM tbl WHERE COLUMNS(*) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 AND COLUMNS(*) IS NOT NULL ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 AND COLUMNS(['col1', 'col3']) < 10 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['nonexistent']) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(* EXCLUDE (col1, col2, col3)) >= 2 ORDER BY ALL
# file: test/sql/parser/values_list_error.test
# query
FROM (VALUES ('1', '2'), ('1'))
FROM (VALUES ('1'), ('1', '2'))
# file: test/sql/peg_parser/parser/lambda_functions.test
# query
CALL check_peg_parser($TEST_PEG_PARSER$SELECT COLUMNS(lambda x: x LIKE 'col%') FROM integers$TEST_PEG_PARSER$)
# file: test/sql/peg_parser/parser/select_star.test
# query
CALL check_peg_parser($TEST_PEG_PARSER$SELECT integers.* EXCLUDE ('i')$TEST_PEG_PARSER$)
CALL check_peg_parser($TEST_PEG_PARSER$SELECT * EXCLUDE (db1.s1.t.c) FROM db1.s1.t, db2.s1.t$TEST_PEG_PARSER$)
# file: test/sql/storage/types/list/empty_float_arrays.test
# setup
CREATE TABLE test_table ( id INTEGER, emb FLOAT[], emb_arr FLOAT[3] )
# query
FROM test_table
# file: test/sql/storage/types/variant/extension_types.test
# setup
CREATE TABLE tbl (col VARIANT)
# query
select COLUMNS(*)::INET from tbl
# file: test/sql/storage/types/variant/test_all_types_single_object.test
# setup
create table tbl ( col VARIANT )
create or replace table intermediate as from query($$select col."$$ || getvariable('col_name') || $$" extracted from tbl$$)
# query
from query($$select col."$$ || getvariable('col_name') || $$"::$$ || getvariable('col_type') || ' from tbl')
from query('select extracted::' || getvariable('col_type') || ' from intermediate')
# file: test/sql/storage/types/variant/test_all_types_variant.test
# setup
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
# query
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
# file: test/sql/storage/types/variant/variant_null_missing.test
# setup
create table shredded_values (col VARIANT)
create table nested_shredded_values (col VARIANT)
create table shredded_array (col VARIANT)
# query
FROM shredded_values
FROM nested_shredded_values
FROM shredded_array
# file: test/sql/storage/wal/wal_create_insert_drop.test
# setup
create or replace table bla as select 42
# query
from bla2
from bla
# file: test/sql/storage/wal/wal_replay_with_function.test
# setup
CREATE TABLE test (id INTEGER)
# query
from test
# file: test/sql/storage/bc/test_broken_view_v092.test
# query
FROM duckdb_columns()
# file: test/sql/storage/bc/test_view_v092.test
# query
FROM duckdb_views()
# file: test/sql/storage/compression/roaring/roaring_bool_first_is_null.test
# setup
CREATE TABLE test (a BOOL)
# query
FROM test
# file: test/sql/extensions/permissions_duckdb_extension.test
# query
FROM read_blob('build/*/repository/*/*/parquet.duckdb_extension')
# file: test/sql/pg_catalog/pg_constraint.test
# setup
create table a (id int , primary key (id))
create table b (id int , foreign_a int, foreign key (foreign_a) references a)
# query
SELECT * EXCLUDE (OID, CONRELID, connamespace) FROM pg_catalog.pg_constraint
# file: test/sql/transactions/statement-preprocessor/invalidation_policy_is_respected_by_preprocessor.test
# setup
CREATE OR REPLACE TABLE t AS SELECT range::INT AS id FROM range(10)
# query
from t
# file: test/sql/attach/attach_default_table.test
# setup
CREATE OR REPLACE TABLE ddb.my_table AS (SELECT 1337 as value)
create table ddb as select 42 as value
CREATE VIEW ddb as SELECT 1
# query
FROM ddb
from ddb
from ddb.my_table
from ddb.main.my_table
from memory.main.ddb
from my_table
from main.my_table
# file: test/sql/attach/attach_different_alias.test
# query
FROM alias1.tbl1
FROM alias2.tbl1
# file: test/sql/attach/attach_encryption_fallback_readonly.test
# setup
CREATE TABLE enc.test AS SELECT 1 as a
# query
FROM enc.test ORDER BY value
FROM enc.test
# file: test/sql/attach/attach_fsspec.test
# query
FROM dummy.tbl
# file: test/sql/attach/attach_issue_7660.test
# setup
create table tbl1 as select 1 as a
# query
FROM test.tbl1
FROM tbl1
# file: test/sql/attach/attach_multi_identifiers.test
# setup
CREATE SCHEMA db1.s1
CREATE SCHEMA db2.s1
CREATE TABLE db2.s1.t(c INT)
CREATE OR REPLACE TABLE db1.s1.t ( c INT, c_squared AS (c * c), )
# query
SELECT * EXCLUDE (db1.s1.t.c) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (DB1.S1.T.C) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (s1.t.c) FROM db1.s1.t, (SELECT 42) t
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (db1.s1.t.c AS new_col) FROM db1.s1.t, db2.s1.t)
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (DB1.S1.T.C AS new_col) FROM db1.s1.t, db2.s1.t)
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (s1.t.c AS new_col) FROM db1.s1.t, (SELECT 42) t)
# file: test/sql/attach/attach_read_only_transaction.test
# setup
CREATE TABLE db1.integers(i INTEGER)
# query
FROM db1.integers
# file: test/sql/attach/attach_show_table.test
# setup
CREATE SCHEMA db2.test_schema
CREATE TABLE db1.table_in_db1(i int)
CREATE TABLE db2.table_in_db2(i int)
CREATE TABLE db2.test_schema.table_in_db2_test_schema(i int)
# query
FROM table_in_db2
FROM table_in_db2_test_schema
# file: test/sql/window/test_fill_orderby.test
# query
with source as ( select i, i * 3 % 5 as permuted, if(permuted > 0, NULL, permuted) as missing from range(5) tbl(i) ) select i, permuted, fill(missing order by permuted) over (order by i) as filled from source qualify filled <> permuted
with source as ( select i, i * 5 % 11 as permuted, if(permuted < 6, NULL, permuted) as missing from range(11) tbl(i) ) select i, permuted, fill(missing order by permuted) over (partition by permuted // 5 order by i) as filled from source qualify filled is distinct from permuted order by i
with source as ( select i, i * 5 % 11 as permuted, if(permuted = 2, NULL, permuted) as missing, if(permuted < 4, NULL, permuted) as unsorted, from range(11) tbl(i) ) select i, permuted, fill(missing order by unsorted) over (order by i) as filled from source qualify filled is distinct from permuted order by i
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 0, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls first) over (order by i) as filled from source qualify filled is distinct from permuted
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 4, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls last) over (order by i) as filled from source qualify filled is distinct from permuted
# file: test/sql/window/test_streaming_lead_lag.test
# query
EXPLAIN SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
# file: test/sql/window/test_streaming_nthvalue.test
# query
select i, nth_value(i, 2049) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) n from range(2049) tbl(i) QUALIFY n IS NOT NULL
# file: test/sql/window/test_streaming_window.test
# setup
create table integers (i int, j int)
CREATE TABLE v1(id bigint)
CREATE TABLE v2(id bigint)
CREATE TABLE issue17621(i INT, j INT, k INT)
CREATE VIEW vertices_view AS SELECT * FROM v1 UNION ALL SELECT * FROM v2
# query
WITH alternate AS ( SELECT range r, IF(range % 2, range, NULL) s FROM range(3000) ) SELECT r, s, last(s IGNORE NULLS) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) l FROM alternate QUALIFY l <> r - ((r + 1) % 2)
WITH alternate AS ( SELECT range r, IF(range < 2100, NULL, range) s FROM range(3000) ) SELECT r, s, first(s IGNORE NULLS) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) f FROM alternate QUALIFY (f IS NOT NULL AND r < 2100) OR (f IS NULL AND r >= 2100)
PREPARE sw1 AS SELECT i, row_number() OVER() AS row_no FROM range(10, 20) tbl(i) QUALIFY row_no <= ?::BIGINT
# file: test/sql/copy/partitioned/partitioned_window.test
# setup
CREATE TABLE partitioned_tbl AS SELECT i%2 AS partition, i col1, i // 7 col2, (i%3)::VARCHAR col3 FROM range(10000) t(i)
CREATE TABLE partitioned_tbl2 AS SELECT i%2 AS partition1, i%3 AS partition2, i col1, i + 1 col2 FROM range(10000) t(i)
# query
SELECT partition, LAG(col1) OVER w AS prev FROM partitioned_tbl WINDOW w AS (PARTITION BY partition ORDER BY col1) QUALIFY (col1 - prev) <> 2
SELECT partition1, partition2, LAG(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition1, partition2 ORDER BY col1) QUALIFY (col1 - prev) <> 6
SELECT count(*) FROM ( SELECT partition1, partition2, lag(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition1 ORDER BY col1) QUALIFY col1 - prev <> 2 )
SELECT count(*) FROM ( SELECT partition1, partition2, lag(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition2 ORDER BY col1) QUALIFY col1 - prev <> 3 )
# file: test/sql/order/hugeint_order_by_extremes.test
# setup
CREATE TABLE test (a hugeint)
# query
CREATE TABLE test (a hugeint)
INSERT INTO test values ((-170141183460469231731687303715884105728)::hugeint), (-1111::hugeint), (-1::hugeint), (0::hugeint), (1::hugeint), (1111::hugeint)
SELECT * FROM test order by a
SELECT * FROM test order by a DESC
# file: test/sql/order/limit_parameter.test
# setup
CREATE TABLE integers AS SELECT 5 k
CREATE TABLE strings AS SELECT '5'::VARCHAR k
CREATE TABLE doubles AS SELECT 0.05 d
# query
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT 5
CREATE TABLE integers AS SELECT 5 k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM integers)
CREATE TABLE strings AS SELECT '5'::VARCHAR k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM strings)
# file: test/sql/order/limit_union.test
# query
SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7
SELECT COUNT(*) FROM (SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7) tbl
# file: test/sql/order/negative_offset.test
# setup
CREATE TABLE integers AS SELECT -1 k
# query
SELECT * FROM generate_series(0,10,1) LIMIT 3 OFFSET -1
SELECT * FROM generate_series(0,10,1) LIMIT -3
SELECT * FROM generate_series(0,10,1) LIMIT -1%
CREATE TABLE integers AS SELECT -1 k
SELECT * FROM generate_series(0,10,1) LIMIT (SELECT k FROM integers)
# file: test/sql/order/order_overflow.test
# query
SELECT 42 ORDER BY -9223372036854775808
# file: test/sql/order/test_limit_cte.test
# query
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte)
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte) OFFSET (SELECT * FROM cte)
# file: test/sql/order/test_limit_parameter.test
# query
SELECT 'Test' LIMIT ?
PREPARE v1 AS SELECT 'Test' LIMIT ?
EXECUTE v1(1)
EXECUTE v1(0)
PREPARE v2 AS SELECT * FROM RANGE(1000000000) LIMIT ? OFFSET ?
# file: test/sql/order/test_limit_percent.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE TABLE test2 (a STRING)
CREATE TABLE struct_data (g INTEGER, e INTEGER)
CREATE TABLE integers(i INTEGER)
CREATE VIEW v3 AS SELECT i % 5 g, LIST(CASE WHEN i=6 or i=8 then null else i end) l FROM RANGE(20) tbl(i) group by g
# query
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22), (14, 32), (15, 52)
SELECT a FROM test LIMIT 20 %
SELECT a FROM test LIMIT 40 PERCENT
SELECT a FROM test LIMIT 35%
SELECT a FROM test LIMIT 79.9%
# file: test/sql/order/test_nulls_first.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE test(i INTEGER, j INTEGER)
# query
CREATE TABLE integers(i INTEGER)
INSERT INTO integers VALUES (1), (NULL)
SELECT * FROM integers ORDER BY i
SELECT * FROM integers ORDER BY i NULLS FIRST
SELECT * FROM integers ORDER BY i NULLS LAST
# file: test/sql/order/test_order_by.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
select b from test where a = 12
SELECT b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY a
SELECT a, b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY b, a
# file: test/sql/order/test_order_by_exceptions.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
SELECT a FROM test ORDER BY 2
SELECT a FROM test ORDER BY 'hello', a
SET order_by_non_integer_literal=true
SELECT a AS k, b FROM test UNION SELECT a, b AS k FROM test ORDER BY k
SELECT a AS k, b FROM test UNION SELECT a AS k, b FROM test ORDER BY k
# file: test/sql/order/test_order_large.test
# setup
CREATE TABLE test AS SELECT a FROM range(10000, 0, -1) t1(a)
# query
PRAGMA verify_parallelism
CREATE TABLE test AS SELECT a FROM range(10000, 0, -1) t1(a)
SELECT * FROM test ORDER BY a
# file: test/sql/order/test_order_pragma.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
SELECT a FROM test ORDER BY a
PRAGMA default_order='DESCENDING'
PRAGMA default_order='ASC'
PRAGMA default_order())
PRAGMA default_order='UNKNOWN'
# file: test/sql/order/test_order_range_mapping.test
# setup
create table test (i hugeint)
# query
insert into test values (100), (25), (75), (50)
select * from test order by i
drop table test
insert into test values (10000), (2500), (7500), (5000)
insert into test values (1000000), (250000), (750000), (500000)
# file: test/sql/order/top_n_issue_21623.test
# setup
CREATE OR REPLACE TABLE t3(c VARCHAR)
CREATE OR REPLACE TABLE t2( a VARCHAR, b VARCHAR )
CREATE OR REPLACE TABLE t1( a VARCHAR, c VARCHAR )
# query
CREATE OR REPLACE TABLE t3(c VARCHAR)
INSERT INTO t3 VALUES ('19'), ('21'), ('22'), ('23')
CREATE OR REPLACE TABLE t2( a VARCHAR, b VARCHAR )
INSERT INTO t2 VALUES ('3', '8'), ('5', NULL), ('8', NULL), ('11', NULL)
CREATE OR REPLACE TABLE t1( a VARCHAR, c VARCHAR )
# file: test/sql/order/top_n_nulls.test
# query
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 360
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 10 offset 440
# file: test/sql/settings/access_mode.test
# query
SET access_mode='read_only'
# file: test/sql/settings/allowed_configs.test
# query
SET allowed_configs=['lock_configuration']
SET allowed_configs=['allowed_configs']
SET allowed_configs=['']
SET allowed_configs=['not_a_real_setting']
SET allowed_configs=['TimeZone']
# file: test/sql/settings/allowed_configs_extensions.test
# query
SET lock_configuration=true
SET TimeZone='America/New_York'
SET Calendar='japanese'
# file: test/sql/settings/allowed_paths.test
# query
RESET allowed_paths
SET allowed_paths=[]
# file: test/sql/settings/block_allocator_memory.test
# query
RESET block_allocator_memory
SET block_allocator_memory='100MiB'
SET memory_limit='200MiB'
SET block_allocator_memory='-3%'
SET block_allocator_memory='150%'
# file: test/sql/settings/connection_local_settings.test
# setup
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
# query
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
SET default_order = 'ASCENDING'
SET default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'DESCENDING'
SET SESSION default_null_order = 'NULLS FIRST'
# file: test/sql/settings/default_null_order_extended.test
# setup
CREATE TABLE integers(i integer)
# query
CREATE TABLE integers(i integer)
INSERT INTO integers VALUES (1), (2), (3), (NULL)
SELECT * FROM integers ORDER BY i DESC
SELECT FIRST(i ORDER BY i), LAST(i ORDER BY i) FROM integers
SELECT FIRST(i ORDER BY i DESC), LAST(i ORDER BY i DESC) FROM integers
# file: test/sql/settings/drop_set_schema.test
# setup
create schema my_schema
create schema schema1
create schema schema2
create schema db2.schema1
# query
create schema my_schema
select current_schema()
SET schema='my_schema'
drop schema my_schema
create schema schema1
# file: test/sql/settings/errors_as_json.test
# query
SET errors_as_json=true
SELECT * FROM nonexistent_table
SELECT cbl FROM (VALUES (42)) t(col)
SECT cbl FROM (VALUES (42)) t(col)
select corr('hello', 'world')
# file: test/sql/settings/integer_division_setting.test
# query
SELECT 1/2
SELECT 1//2
SET integer_division=true
SET integer_division=false
# file: test/sql/settings/lock_configuration_schema.test
# setup
create schema s1
create schema s2
# query
create schema s1
create schema s2
use s1
use s2
reset schema
# file: test/sql/settings/max_execution_time.test
# query
SELECT current_setting('max_execution_time')
SET max_execution_time=5000
RESET max_execution_time
SELECT name, value, input_type FROM duckdb_settings() WHERE name = 'max_execution_time'
SET max_execution_time=100
# file: test/sql/settings/operator_memory_limit.test
# setup
CREATE TABLE t1 AS SELECT * FROM range(1000000)
# query
SELECT current_setting('operator_memory_limit')
SET operator_memory_limit='256MB'
RESET operator_memory_limit
SET operator_memory_limit='128MB'
SET operator_memory_limit=NULL
# file: test/sql/settings/set_schema_temp_main.test
# query
CREATE SCHEMA temp.s1
CREATE SCHEMA system.s1
set schema = 'temp'
set schema = 'system'
# file: test/sql/settings/setting_alias.test
# query
SELECT current_setting('null_order'), (SELECT value FROM duckdb_settings() WHERE name='null_order')
SET null_order='NULLS_FIRST'
RESET null_order
# file: test/sql/settings/setting_collation.test
# setup
CREATE TABLE collate_test(s VARCHAR)
# query
PRAGMA default_collation='NOCASE'
CREATE TABLE collate_test(s VARCHAR)
INSERT INTO collate_test VALUES ('hEllO'), ('WöRlD'), ('wozld')
SELECT COUNT(*) FROM collate_test WHERE 'BlA'='bLa'
SELECT * FROM collate_test WHERE s='hello'
# file: test/sql/settings/setting_disabled_optimizer.test
# query
SET disabled_optimizers=''
SET disabled_optimizers TO 'expression_rewriter'
SET disabled_optimizers TO 'expression_rewriter,filter_pushdown,join_order'
SELECT current_setting('disabled_optimizers')
SET disabled_optimizers TO 'expression_rewriteX'
# file: test/sql/settings/setting_exhaustive.test
# query
SET debug_window_mode='unknown'
SELECT * FROM duckdb_settings()
SET default_order='unknown'
SET enable_external_access=true
SET enable_profiling='unknown'
# file: test/sql/settings/setting_null_order.test
# query
SELECT * FROM range(3) UNION ALL SELECT NULL ORDER BY 1
# file: test/sql/settings/setting_order.test
# query
SELECT * FROM range(3) ORDER BY 1
# file: test/sql/settings/setting_preserve_identifier_case.test
# setup
CREATE SCHEMA MYSCHEMA
CREATE TABLE MYSCHEMA.INTEGERS(I INTEGER)
# query
SELECT value FROM duckdb_settings() WHERE name='preserve_identifier_case'
CREATE SCHEMA MYSCHEMA
CREATE TABLE MYSCHEMA.INTEGERS(I INTEGER)
SELECT duckdb_tables.schema_name, duckdb_tables.table_name, column_name FROM duckdb_tables JOIN duckdb_columns USING (table_oid)
DROP SCHEMA MYSCHEMA CASCADE
# file: test/sql/settings/setting_profiling_mode.test
# query
SET profiling_mode='standard'
SET profiling_mode='detailed'
SET profiling_mode='all'
SET profiling_mode='unknown'
# file: test/sql/settings/settings_icu.test
# query
SET Calendar='gregorian'
SET TimeZone='pacific/honolulu'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'TimeZone'
SET TimeZone='Pacific/Honolooloo'
SET Calendar='Coptic'
# file: test/sql/settings/test_disabled_file_systems.test
# query
SELECT current_setting('disabled_filesystems')
RESET disabled_filesystems
SET disabled_filesystems=''
SET disabled_filesystems='LocalFileSystem'
SET disabled_filesystems='LocalFileSystem,LocalFileSystem'
# file: test/sql/settings/test_disabled_local_filesystem_metadata.test
# query
PRAGMA disable_verification
SELECT * FROM duckdb_secrets()
SELECT * FROM duckdb_extensions()
# file: test/sql/settings/test_disabled_local_filesystem_secrets.test
# query
CREATE PERSISTENT SECRET my_s (TYPE S3)
# file: test/sql/settings/test_external_access_secrets.test
# query
CREATE PERSISTENT SECRET my_secret (TYPE S3)
# file: test/sql/settings/test_lock_configuration.test
# query
SELECT current_setting('lock_configuration')
SET memory_limit='8GB'
RESET lock_configuration
SET lock_configuration=false
SET memory_limit='10GB'
# file: test/sql/settings/user_agent.test
# query
SET custom_user_agent='something else'
RESET custom_user_agent
SELECT current_setting('custom_user_agent')
SET duckdb_api='something else'
SELECT regexp_matches(user_agent, '^duckdb/.*(.*)') FROM pragma_user_agent()
# file: test/sql/settings/reset/reset_memory_limit.test
# setup
CREATE TABLE t1 AS SELECT * FROM range(1000000)
# query
PRAGMA temp_directory=''
SET memory_limit='2MB'
CREATE TABLE t1 AS SELECT * FROM range(1000000)
RESET memory_limit
