INSERT INTO test VALUES(1000000000, null), (1000000001, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (null, [[[[[[]]]]]]), (1, [[[[[[]]]]]])
SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k ORDER BY ALL LIMIT 2
SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers UNION ALL SELECT * FROM integers ORDER BY ALL
SELECT * FROM integers UNION SELECT * FROM integers ORDER BY ALL
from t1 order by a
SELECT * FROM t0 ORDER BY ALL OFFSET (SELECT DISTINCT 6.5 FROM (SELECT 1) t1(c0) UNION ALL SELECT 3)
CREATE TABLE tbl_structs AS SELECT {'a': 2.0, 'b': 'hello', 'c': [1, 2]} AS s1, 1::BIGINT AS i, {'k': 1::TINYINT, 'j': 0::BOOL} AS s2
INSERT INTO tbl_structs VALUES ( {'a': 1.0, 'b': 'yay', 'c': [10, 20]}, 42, {'k': 2, 'j': 1})
FROM integers
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
select min([-a, 1, a]), max([-a, 1, a]) from t group by b%2
select min({'i': a}), max({'i': a}) from t group by b%2 order by all
select min({'i': a, 'j': a % 2}), max({'i': a, 'j': a % 2}) from t group by b%2
PIVOT Cities USING SUM(Population)
PIVOT Cities USING SUM(Population) GROUP BY Country
PIVOT Cities GROUP BY Country
PIVOT Cities ON Year GROUP BY Country
PIVOT (SELECT Country, Year FROM Cities) ON Year
pivot p using sum (col2) group by col1 order by col1
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
SELECT * FROM Produce PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) total_sales, COUNT(*) num_records FOR quarter IN ('Q1', 'Q2')) ORDER BY ALL
SELECT * FROM Produce UNPIVOT(sales FOR quarter IN (Q1, Q2, Q3, Q4)) ORDER BY ALL
SELECT product, first_half_sales, second_half_sales, semesters FROM Produce UNPIVOT( (first_half_sales, second_half_sales) FOR semesters IN ((Q1, Q2) AS 'semester_1', (Q3, Q4) AS 'semester_2'))
FROM Cities PIVOT ( array_agg(id) FOR name IN ('test','Test') )
FROM Cities PIVOT ( array_agg(id), sum(id) FOR name IN ('test','Test') )
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
PIVOT Cities ON Country USING SUM(Population)
PIVOT Cities ON Country, Name USING SUM(Population)
PIVOT Cities ON Country IN ('xx') USING SUM(Population)
PIVOT Cities ON (Country, Name) IN ('xx') USING SUM(Population)
PIVOT Cities ON Country IN ('xx', 'yy') USING SUM(Population)
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_months) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_monthsx) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN not_an_enum) AS p ORDER BY EMPID
PIVOT test ON j IN ('a', 'b') USING SUM(test.i)
PIVOT test ON j IN ('a', 'b') USING get_current_timestamp()
PIVOT test ON j IN ('a', 'b') USING sum(41) over ()
PIVOT test ON j IN ('a', 'b') USING sum(sum(41) over ())
FROM tbl PIVOT (c FOR IN enum_val)
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
SELECT 'AverageCost' AS Cost_Sorted_By_Production_Days, "0", "1", "2", "3", "4" FROM ( SELECT DaysToManufacture, StandardCost FROM Product ) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable
pivot cities on (Country='NL') using avg(Population) group by name
pivot cities on (Country='NL') in (false, true) using avg(Population) group by name
PIVOT Cities ON Year IN (SELECT Year FROM Cities ORDER BY Year DESC) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT YEAR FROM (SELECT Year, SUM(POPULATION) AS popsum FROM Cities GROUP BY Year ORDER BY popsum DESC)) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT '2010' UNION ALL SELECT '2000' UNION ALL SELECT '2020') USING SUM(Population)
PIVOT Cities ON Year IN (SELECT xx FROM Cities) USING SUM(Population)
PIVOT monthly_sales ON MONTH USING COALESCE(SUM(AMOUNT), 0)
SELECT mode(column_type) FROM (DESCRIBE PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)::INTEGER)
PREPARE v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount + ?) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR')) AS p ORDER BY EMPID
PREPARE v2 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT + ?)
PREPARE v3 AS PIVOT (SELECT empid, amount + ? AS amount, month FROM monthly_sales) ON MONTH USING SUM(AMOUNT)
CREATE VIEW poison_view AS SELECT * FROM t UNPIVOT (val FOR col IN (*))
CREATE VIEW expr_view AS SELECT * FROM t UNPIVOT (val FOR col IN (1+2+id))
CREATE VIEW v AS PIVOT t ON id IN (CASE WHEN true THEN 'a' END) USING (SUM(feb))
CREATE VIEW v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
CREATE MACRO pivot_macro(val) as TABLE SELECT * FROM monthly_sales PIVOT(SUM(amount + val) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
FROM v1
FROM pivot_macro(1)
create table donnees_csv as select {'year': i::varchar, 'month': i::varchar} AS donnee, i%5 as variable_id, i%10 id_niv from range(1000) t(i)
pivot donnees_csv on variable_id using first(donnee) group by id_niv order by all
PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT) ORDER BY ALL
PIVOT (PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)) ON empid USING SUM(COALESCE("2000_1",0) + COALESCE("2000_2",0) + COALESCE("2000_3",0) + COALESCE("2001_1",0) + COALESCE("2001_2",0) + COALESCE("2001_3",0))
CREATE VIEW pivot_view AS PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)
CREATE MACRO xt2(a) as TABLE PIVOT sales ON d USING SUM(amount)
CREATE MACRO xt2(a) as (PIVOT sales ON d USING SUM(amount))
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount + year) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') amount IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) empid IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) ) AS p ORDER BY EMPID
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
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM t1 UNPIVOT (sales FOR date IN ("Sales (05/19/2020)", "Sales (06/03/2020)", "Sales (10/23/2020)")) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON "Sales (05/19/2020)", "Sales (06/03/2020)", "Sales (10/23/2020)" INTO NAME date VALUE sales) ORDER BY ALL
SELECT * FROM (UNPIVOT t1 ON "Sales (05/19/2020)" AS "2020-05-19", "Sales (06/03/2020)" AS "2020-06-03", "Sales (10/23/2020)" AS "2020-10-23" INTO NAME date VALUE sales) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM t1 UNPIVOT (Sales FOR Date IN (COLUMNS('Sales.*'))) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON COLUMNS('Sales.*') INTO NAME date VALUE sales) ORDER BY ALL
SELECT id, regexp_replace(date, 'Sales [(]([0-9]+)/([0-9]+)/([0-9]+)[)]', '\3-\1-\2')::DATE AS date, sales FROM (UNPIVOT t1 ON * EXCLUDE (id) INTO NAME date VALUE sales) ORDER BY ALL
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
unpivot (select 42 as col1, 'woot' as col2) on col1::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on COLUMNS(*)::VARCHAR
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR AS c, col2
select * from (select 42 as col1, 'woot' as col2) UNPIVOT ("value" FOR "name" IN (col1::VARCHAR, col2))
unpivot (select 42 as col1, 'woot' as col2) on (col1 + (SELECT col1))::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on random(), col2
unpivot (select 42 as col1, 'woot' as col2) on col1 + col2
unpivot (select 42 as col1, 'woot' as col2) on t.col1::VARCHAR, col2
UNPIVOT unpivot_names ON COLUMNS('col*')
unpivot integers on columns(* exclude (i))
UNPIVOT test ON (metric_1, value_x), metric_2, metric_3
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUE metric_value
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUES metric_value, metric_type
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select 42) on columns(*))
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select {n : 1 }) on columns(*))
unpivot (select cast(columns(*) as varchar) from (select 42 as col1, 'woot' as col2)) on columns(*)
INSERT INTO v0 VALUES (1), (2), (3), (4) RETURNING * EXCLUDE c1
DELETE from v0 WHERE c1 = 0 RETURNING * EXCLUDE c1
UPDATE v0 SET c1 = 0 WHERE true RETURNING * EXCLUDE c1
Select * from v0 order by all
INSERT INTO v0 BY POSITION ( SELECT TRUE ) OFFSET 1 ROWS RETURNING v0 . * EXCLUDE c1
DELETE FROM table2 WHERE b=3 RETURNING {'a': a, 'b': b}
INSERT INTO all_types VALUES('goo'||chr(0) || 'se' ,[[], [42, 999, NULL, NULL, -42], NULL, [], [42, 999, NULL, NULL, -42]])
INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c')
INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c') + 42
INSERT INTO table1 VALUES (1, 2, 3) RETURNING {'a':a, 'b':b, 'c':c}
INSERT INTO table1 VALUES (1, 2, 3) RETURNING [1, 2] IN (SELECT [a, b] from table1)
INSERT INTO table2(a, b) VALUES ('duckdb', 97) RETURNING {'a': a, 'b': b}
UPDATE table2 SET a='Mr.Duck', b=99 WHERE b=100 RETURNING {'a': a, 'b': b}
insert into null_list values (null), ([null])
insert into null_struct values (null), ({n:null})
insert into null_map values (null), (map([null], [null]))
CREATE VIEW v_struct AS SELECT CAST({'x': 1, 'y': 2} AS STRUCT(x UTINYINT, y USMALLINT)) AS s
CREATE VIEW v_list AS SELECT CAST([1, 2] AS UTINYINT[]) AS l
SELECT [1.33, 10.0]
SELECT [0.1, 1.33, 10.0, 9999999.999999999]
SELECT [99999999999999999999999999999999999.9, 9.99999999999999999999999999999999999]
INSERT INTO foo VALUES ([{'my_double': 1.33}, {'my_double': 10.0}])
CREATE TABLE from_list AS SELECT [1000000, 10.0000000005]
select map { 1: decimal 'b' }
SELECT [42]::my_int_list
INSERT INTO a VALUES (MAP([1], [2])), (MAP([1, 2, 3], [4, 5, 6]))
INSERT INTO a VALUES ({'i': 3, 'j': 4})
INSERT INTO a VALUES (ROW('hello', [1, 2]))
insert into failing VALUES ( {'foobar': 'Foo'} )
select count(*), current_alias from person group by current_alias order by all
FROM ( SELECT ( 2::bit & 2::bit ) AS a, 2::bit AS b, (a = b) AS '(a = b)', ) SELECT a, b, a = b, "(a = b)"
SELECT i FROM issue14384 ORDER BY ALL
SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) ORDER BY ALL
SELECT * FROM issue14384 INNER JOIN ( SELECT INTERVAL 1000 DAY AS col0 FROM issue14384) AS sub0 ON (issue14384.i < sub0.col0) WHERE (NOT (issue14384.i != issue14384.i)) ORDER BY ALL
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
INSERT INTO foo.test VALUES (['a', 'b'])
FROM person
select count(*), current_mood from person group by current_mood order by all
select 'POINT(0 1)'::GEOMETRY((['abc']))
select typeof('POINT(0 1)'::GEOMETRY('GEOGCRS["WGS 84",foo[]'))
select distinct segment_type from pragma_storage_info('t1') order by all
select segment_type from pragma_storage_info('t2') order by all
SELECT tns, DATE_PART(['hour', 'minute', 'second'], tns) FROM times
SELECT tns, DATE_PART(['millisecond', 'microsecond', 'epoch'], tns) FROM times
SELECT tns, DATE_PART(['timezone', 'timezone_hour', 'timezone_minute'], tns) p FROM times WHERE p <> {'timezone': 0, 'timezone_hour': 0, 'timezone_minute': 0}
SELECT date_part(['julian'], '23:59:59.123456789'::TIME_NS)
SELECT * FROM timetzs ORDER BY ALL
SELECT lhs.ttz, rhs.ttz, lhs.ttz < rhs.ttz, lhs.ttz <= rhs.ttz, lhs.ttz = rhs.ttz, lhs.ttz >= rhs.ttz, lhs.ttz > rhs.ttz, lhs.ttz <> rhs.ttz, FROM timetzs lhs, timetzs rhs ORDER BY ALL
select cast (null as main.u ARRAY[1])
insert into i values ({'i': 1.0, 'j': 2.0})
select cast (null as u array[1])
INSERT INTO test_structs VALUES (1, {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}), (2, NULL), (3, {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}), (4, {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}})
INSERT INTO test_structs_nested VALUES (1, {'s': {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}}), (2, NULL), (3, {'s': {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}}), (4, {'s': {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}}})
INSERT INTO a VALUES ({ 'i': { 'a': 3 }, 'j': 4 })
CREATE TABLE b AS SELECT { 'a': { 'a': 1, 'b': 'hello' } } c
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
SELECT remap_struct( [ { 'i': 1, 'j': { 'x': 42, 'z': 100 } } ], NULL::STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )[], { 'list': ROW( 'list', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'list': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
INSERT INTO large_list (SELECT LIST(CASE WHEN i%2=0 THEN {'i': i} ELSE NULL END) FROM range(5000) t(i))
SELECT COUNT(*), COUNT(j), SUM(j) FROM ( SELECT UNNEST(remap_struct(s, NULL::ROW(j INTEGER)[], {'list': ROW('list', {'j': 'i'})}, NULL), recursive := True) FROM large_list )
SELECT remap_struct( MAP { 'my_key1' : { 'i': 10, 'j': { 'x': 42, 'z': 100 } }, 'my_key2' : { 'i': 20, 'j': { 'x': 21, 'z': 50 } } }, NULL::MAP(VARCHAR, STRUCT( v1 INT, v2 STRUCT( x INT, y INT, z INT ), v3 VARCHAR )), { 'key': 'key', 'value': ROW( 'value', { 'v1': 'i', 'v2': ROW( 'j', { 'x': 'x', 'z': 'z' } ) } ) }, { 'value': { 'v2': { 'y': NULL::INT }, 'v3': NULL::VARCHAR } } )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(INT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::MAP(BIGINT[], VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : 'test', [6,4,5] : 'world' }, NULL::STRUCT("key" BIGINT[], "value" VARCHAR), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(BIGINT[], MAP(VARCHAR, VARCHAR)), { 'key': 'key', 'value': 'value' }, NULL )
SELECT remap_struct( MAP { [1,2,3] : ['test'], [6,4,5] : ['world'] }, NULL::MAP(INT[], VARCHAR[]), { 'key': ROW( 'key', { 'list': 'list' } ), 'value': 'value' }, NULL )
INSERT INTO src VALUES ([{'i': 1, 'j': 2}])
FROM t ORDER BY ALL
INSERT INTO src VALUES ([{'i': 3, 'j': 4}]), ([{'i': 5, 'j': 6}])
FROM t2 ORDER BY ALL
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
CREATE TABLE tbl AS SELECT ({'HELLO': 3}) col
SELECT col['HELLO'] FROM tbl
SELECT col['hello'] FROM tbl
SELECT ({'hello': 3, 'hello': 4}) col
SELECT ({'HELLO': 3, 'HELLO': 4}) col
SELECT ({'HELLO': 3, 'hello': 4}) col
SELECT col['HELL'] FROM tbl
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
INSERT INTO t1 VALUES ({a: 42, b: 43})
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
SELECT struct_concat({'a': 1}, {'b': NULL}, NULL::STRUCT(k INT), struct_pack( x := 'foobar'))
CREATE TABLE t1 AS SELECT {'i': i, 'j': i + i % 2} as s FROM generate_series(1, 15) AS t(i)
SELECT struct_concat({'a': 2, 'b': NULL}, s) FROM t1
SELECT struct_concat(s, {'a': 2, 'b': NULL}) FROM t1 WHERE s.i % 2 = 0
SELECT struct_concat({'a': 'first struct'}, {'a': 'second struct'})
SELECT struct_concat({'a': 'first struct'}, {'A': 'second struct'})
SELECT s FROM t1 ORDER BY ALL
SELECT bar FROM foo ORDER BY ALL
SELECT s FROM T ORDER BY ALL
select unnest(s) from tbl_structs order by all
SELECT MAP(['category', 'min', 'max'], [category, MIN(score), MAX(score)]) FROM groups GROUP BY category ORDER BY ALL
SELECT f FROM floats WHERE f>=1 ORDER BY ALL
SELECT f FROM floats WHERE f<=1 ORDER BY ALL
SELECT f FROM floats WHERE f > 0 ORDER BY ALL
SELECT f FROM floats WHERE f >= 1 ORDER BY ALL
select row_number() over () qualify unnest([1,2,3])
SELECT k, a, b, list_sort(ARRAY( SELECT DISTINCT ax FROM UNNEST(a) ta(ax) WHERE ax = any(b) ORDER BY ALL )) ab_intersect FROM tbl
select struct_pack(*COLUMNS(*))::VARIANT::JSON from test_all_types()
select {'var': COLUMNS(*)::VARIANT}::VARIANT from test_all_types()
select [COLUMNS(['tinyint', 'double'])::VARIANT, NULL, COLUMNS(['tinyint', 'double'])::VARIANT] from test_all_types()
select [COLUMNS(['tinyint', 'double'])::JSON, NULL, COLUMNS(['tinyint', 'double'])::JSON] from test_all_types()
create table foo.variant_lineitem as select variant_normalize(STRUCT_PACK(*COLUMNS(*))::VARIANT) from lineitem
select COLUMNS(*)::JSON from foo.variant_lineitem limit 10
CREATE VIEW list_int1 AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [2]), ([2], [1]), (NULL, [1]), ([2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([1], [1]), ([1], [1, 2]), ([1, 2], [1]), (NULL, [1]), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_int_empty AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([], []), ([], [1, 2]), ([1, 2], []), (NULL, []), ([1, 2], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES (['duck'], ['duck']), (['duck'], ['duck', 'goose']), (['duck', 'goose'], ['duck']), (NULL, ['duck']), (['duck', 'goose'], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_of_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}], [{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], [{'x': 'duck', 'y': 1}]), (NULL, [{'x': 'duck', 'y': 1}]), ([{'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}], NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck'}, {'x': 'duck'}), ({'x': 'duck'}, {'x': 'goose'}), ({'x': 'goose'}, {'x': 'duck'}), (NULL, {'x': 'duck'}), ({'x': 'goose'}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW struct_str_int AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 'duck', 'y': 1}, {'x': 'duck', 'y': 1}), ({'x': 'duck', 'y': 1}, {'x': 'goose', 'y': 2}), ({'x': 'goose', 'y': 2}, {'x': 'duck', 'y': 1}), (NULL, {'x': 'duck', 'y': 1}), ({'x': 'goose', 'y': 2}, NULL), (NULL, NULL) ) tbl(l, r)
CREATE VIEW list_in_struct AS SELECT COLUMNS(*)::VARIANT FROM (VALUES ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 1, 'y': ['duck', 'somateria']}, {'x': 2, 'y': ['goose']}), ({'x': 2, 'y': ['goose']}, {'x': 1, 'y': ['duck', 'somateria']}), (NULL, {'x': 1, 'y': ['duck', 'somateria']}), ({'x': 2, 'y': ['goose']}, NULL), (NULL, NULL) ) tbl(l, r)
SELECT u.i32, u.str, u.f32 FROM tbl2 UNION ALL SELECT u.i32, u.str, NULL FROM tbl1 ORDER BY ALL
SELECT u::UNION(i SMALLINT) FROM tbl3 ORDER BY ALL
SELECT u::UNION(i SMALLINT, b VARCHAR) FROM tbl4 ORDER BY ALL
SELECT union_tag(u), union_tag(u::UNION(i SMALLINT, b INT)), u::UNION(i SMALLINT, b INT) FROM tbl4 ORDER BY ALL
SELECT id, union_tag(a), a.b, a.c FROM tbl1 UNION SELECT id, union_tag(d), d.e, d.f FROM tbl2 ORDER BY ALL
SELECT id, union_tag(a) as tag, a.b as v1, a.c as v2 FROM tbl1 UNION SELECT id, union_tag(d) as tag, d.e as v1, d.f as v2 FROM tbl2 ORDER BY ALL
SELECT tbl1.a.c, tbl1.id, tbl2.id FROM tbl2 JOIN tbl1 ON tbl1.a.c = tbl2.d.f ORDER BY ALL
SELECT t1.id FROM tbl1 as t1 JOIN tbl1 as t2 ON t1.a = t2.a ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 INNER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 FULL OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 LEFT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT tbl1.a, tbl1.id, tbl2.id FROM tbl2 RIGHT OUTER JOIN tbl1 ON tbl1.a = tbl2.b ORDER BY ALL
SELECT * FROM tbl1 JOIN tbl2 ON tbl1.union_struct.str = tbl2.struct_union.alt.k order by all
SELECT FIRST(a ORDER BY ALL), LAST(a ORDER BY ALL) FROM tbl1
SELECT c50 FROM array_tbl GROUP BY ALL USING SAMPLE 3
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types()
SELECT DISTINCT * FROM t1 ORDER BY ALL
SELECT * FROM t1 JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 JOIN t2 ON t1.a = t2.a ORDER BY ALL
SELECT * FROM t1 FULL OUTER JOIN t2 USING (i) ORDER BY ALL
SELECT * FROM t1 as a JOIN t1 as b ON (a.col1 != b.col1) ORDER BY ALL
SELECT DISTINCT a FROM arrays ORDER BY ALL
SELECT DISTINCT a FROM arrays WHERE a[1] > 0 ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS UNION SELECT a FROM ARRAYS ) ORDER BY ALL
SELECT * FROM ( SELECT a FROM ARRAYS WHERE a[1] > 0 UNION SELECT a FROM ARRAYS WHERE a[1] > 0 ) ORDER BY ALL
SELECT a::VARCHAR FROM arrays ORDER BY ALL
SELECT TRY_CAST(a::INTEGER[] AS INTEGER[3]) FROM ARRAYS ORDER BY ALL
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints group by a) as lst_tbl) as T ORDER BY ALL
select a, cardinality(m) from (select a,MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb, a FROM ints where b < 3 group by a) as lst_tbl) as T ORDER BY ALL
select m from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints group by b) as lst_tbl) as T ORDER BY ALL
select m[1] from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints group by b) as lst_tbl) as T ORDER BY ALL
select m[1] from (select MAP(lsta,lstb) as m from (SELECT list(a) as lsta, list(b) as lstb FROM ints where b <4 group by b) as lst_tbl) as T ORDER BY ALL
select film_id, ARRAY_AGG(actor_id order by actor_id) FROM film_actor GROUP BY film_id ORDER BY ALL
from (select 'from' fromV) select 'sel' selectV,*
FROM "My Table"
FROM tbl2
FROM (SHOW databases) t
select l.k100 as lid, r.k100 as rid from tleft l inner join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l left join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
explain select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and l.k100 + r.k100 < 10 order by all
SELECT lhs.a, rhs.a FROM tb1 AS lhs LEFT JOIN tb2 AS rhs ON (lhs.a IS DISTINCT FROM rhs.a) ORDER BY ALL
SELECT lhs.a, rhs.a FROM tb1 AS lhs LEFT JOIN tb2 AS rhs ON (lhs.a IS NOT DISTINCT FROM rhs.a) ORDER BY ALL
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_small p JOIN prf_build_small b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_string p JOIN prf_build_string b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, hex(p.k), p.g, b.payload FROM prf_probe_string_nul p JOIN prf_build_string_nul b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT p.id, p.k, p.g, b.payload FROM prf_probe_span p JOIN prf_build_span b ON p.k = b.k AND p.g >= b.g ORDER BY ALL
SELECT b.k, p.k FROM prf_row_group_bug.prf_probe_small p RIGHT JOIN prf_build_row_group_bug b USING (k) ORDER BY ALL
select * from t1 semi join t2 on t1.a < t2.b and t1.b > t2.b order by all
select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
Explain select * from t1 anti join t2 on t1.a < t2.b and t1.b < t2.b order by all
select * from t1 semi join t2 on t1.a < t2.b or t1.b < t2.b order by all
select * from t1 semi join t2 on (t1.a < t2.b and t1.b < t2.b) or (t1.a < t2.b and t1.b = 4) order by all
select * from t1 semi join t2 on (t1.a < t2.b or t1.b < t2.b) and (t1.a = 1 or t1.b = 4) order by all
SELECT * FROM t1 POSITIONAL JOIN t0 WHERE (t1.c0 > t0.c1) IS NULL
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
select lefttable.x, righttable.y from (select 1 as x) lefttable asof left join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x limit 0) lefttable asof left join (select 1 as x, 1 as y) righttable on lefttable.x >= righttable.x
select lefttable.x, righttable.y from (select 1 as x) lefttable asof join (select 1 as x, 1 as y limit 0) righttable on lefttable.x >= righttable.x
SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol = p.symbol AND t.when >= p.when
EXPLAIN SELECT t.*, p.price FROM trades t ASOF JOIN prices p ON t.symbol IS NOT DISTINCT FROM p.symbol AND t.when >= p.when
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON 1 = 1 AND p.ts >= e.begin ORDER BY p.ts ASC
WITH samples AS ( SELECT col0 AS starts, col1 AS ends FROM (VALUES (5, 9), (10, 13), (14, 20), (21, 23) ) ) SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM samples AS s1 ASOF JOIN samples as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts <> e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts = e.begin ORDER BY p.ts ASC
SELECT p.ts, e.value FROM range(0,10) p(ts) ASOF JOIN events0 e ON p.ts >= e.begin AND p.ts >= e.value ORDER BY p.ts ASC
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
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150 AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND true
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND false
SELECT * FROM left_table l ASOF JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price > 150
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND r.active = true
SELECT * FROM left_table l ASOF LEFT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin > e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin <= e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin < e.begin ORDER BY ALL ASC
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN events0 e USING (begin) ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e ON p.begin >= e.begin ORDER BY ALL
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN events0 e USING (begin) ORDER BY ALL
explain SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON tt1.j = tt2.j AND tt1.i >= tt2.i ORDER BY tt1.i
SELECT tt1.i, tt2.k FROM tt1 ASOF JOIN tt2 ON (tt1.j = tt2.j OR tt1.j = tt2.j) AND tt1.i >= tt2.i ORDER BY tt1.i
explain select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
select l.id, l.date, l.item as litem, r.item as ritem, valuei from l asof left join r on l.id = r.id and l.date >= r.date and (l.item = r.item or l.item = '*')
explain from tbl1 asof join tbl2 on tbl1.x = tbl2.x and tbl1.ts >= tbl2.ts and (tbl1.ts - tbl2.ts) < interval '1' hours
from tbl1 asof join tbl2 on tbl1.x = tbl2.x and tbl1.ts >= tbl2.ts and (tbl1.ts - tbl2.ts) < interval '1' hours
SELECT t.*, p.price FROM trades_int t ASOF JOIN prices_int p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_varchar t ASOF JOIN prices_varchar p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_struct t ASOF JOIN prices_struct p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_list t ASOF JOIN prices_list p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_array t ASOF JOIN prices_array p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_nested t ASOF JOIN prices_nested p ON t.symbol = p.symbol AND t.when >= p.when
SELECT t.*, p.price FROM trades_multiple t ASOF JOIN prices_multiple p ON t.symbol = p.symbol AND t.exchange = p.exchange AND t.when >= p.when
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
SELECT d1.time, d2.time, d1.value, d2.value FROM right_pushdown d1 ASOF LEFT JOIN ( SELECT * FROM right_pushdown WHERE value is not NULL ) d2 ON d1.time >= d2.time ORDER BY ALL
SELECT s1.starts as s1_starts, s2.starts as s2_starts, FROM issue12215 AS s1 ASOF JOIN issue12215 as s2 ON s2.ends >= (s1.ends - 5) WHERE s1_starts <> s2_starts ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
WITH t as ( SELECT t1.col0 AS left_val, t2.col0 AS right_val, FROM (VALUES (0), (5), (10), (15)) AS t1 ASOF LEFT JOIN (VALUES (1), (6), (11), (16)) AS t2 ON t2.col0 > t1.col0 ) SELECT * FROM t WHERE right_val BETWEEN 3 AND 12 ORDER BY ALL
select a.seq_no, a.amount, b.amount from issue13899 as a asof join issue13899 as b on a.seq_no>=b.seq_no and b.amount is not null ORDER BY 1
WITH t1 AS ( FROM (VALUES (1,2),(2,4)) t1(id, value) ), t2 AS ( FROM (VALUES (1,3)) t2(id, value) ) FROM t1 ASOF LEFT JOIN t2 ON t1.id <= t2.id ORDER BY 1
WITH t1 AS ( FROM (VALUES (1,2),(2,4)) t1(id, value) ), t2 AS ( FROM (VALUES (1,3)) t2(id, value) ) FROM t1 ASOF LEFT JOIN t2 ON t1.id >= t2.id AND t1.id = 1 ORDER BY 1
WITH t1 AS ( FROM VALUES (1::INT, '2020-01-01 00:00:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') AS t1(a, b) ), t2 AS ( FROM VALUES (1::INT, '2020-01-01 00:01:00'::TIMESTAMP), (2, '2020-01-02 00:00:00') t2(c, d) ) SELECT * FROM t1 ASOF JOIN t2 ON t1=b == t2.d AND t1.b >= t2.d - INTERVAL '1' SECOND
SELECT begin, value IN ( SELECT e1.value FROM ( SELECT * FROM events e1 WHERE e1.value = events.value) e1 ASOF JOIN range(1, 10) tbl(begin) USING (begin) ) FROM events ORDER BY ALL
SELECT p.begin, e.value FROM probe0 p ASOF LEFT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin, e.value FROM probe0 p ASOF RIGHT JOIN (SELECT * FROM events0 WHERE log(value + 5) > 10) e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF SEMI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT p.begin FROM probe0 p ASOF ANTI JOIN events0 e ON p.begin >= e.begin ORDER BY p.begin ASC
SELECT time_series.time, asof_nulls.value FROM (VALUES ('2025-07-15 02:00:00'::TIMESTAMP)) as time_series(time) ASOF LEFT JOIN asof_nulls ON asof_nulls.time <= time_series.time
SELECT * FROM left_table l ASOF SEMI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF RIGHT JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF FULL JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300
SELECT * FROM left_table l ASOF ANTI JOIN right_table r ON l.symbol = r.symbol AND l.ts >= r.ts AND l.price + r.bid > 300 ORDER BY 1
SELECT id2, id3, id3_right, sum(value * value_right) as value FROM ( SELECT df.*, df2.id3 as id3_right, df2.value as value_right FROM df JOIN df as df2 ON (df.id = df2.id AND df.id2 = df2.id2 AND df.id3 > df2.id3 AND df.id3 < df2.id3 + 30) ) tbl GROUP BY ALL ORDER BY ALL
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND l.price + r.bid < 400 ORDER BY id
FROM wide_ranges l ANTI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol AND l.price + r.bid < 375 ORDER BY id
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ) SELECT lhs.id, rhs.id FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL
WITH test AS ( SELECT i AS id, i AS begin, i + 10 AS end, i % 2 AS p1, i % 3 AS p2 FROM range(0, 10) tbl(i) ), sub AS ( SELECT lhs.id AS lid, rhs.id AS rid FROM test lhs, test rhs WHERE lhs.begin < rhs.end AND rhs.begin < lhs.end AND lhs.p1 <> rhs.p1 AND lhs.p2 <> rhs.p2 ORDER BY ALL ) SELECT MIN(lid), MAX(rid) FROM sub
explain select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and r.b100 < l.e100 and l.k100 + r.k100 < 10 order by all
select l.k100 as lid, r.k100 as rid from tleft l full join tleft r on l.b100 < r.e100 and r.b100 < l.e100 and l.k100 + r.k100 < 10 order by all
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND l.price + r.bid < 400 ORDER BY id
FROM wide_ranges l SEMI JOIN narrow_ranges r ON l.start < r.stop AND r.start < l.stop AND r.symbol = l.symbol AND l.price + r.bid < 375 ORDER BY id
SELECT total_profit, COUNT(total_profit), SUM(amount_sold), SUM(price) FROM unit GROUP BY total_profit ORDER BY ALL
FROM tbl_comp
FROM t1
SELECT * FROM test ORDER BY ALL
SELECT * FROM employee ORDER BY ALL
SELECT * FROM album ORDER BY ALL
SELECT * FROM song ORDER BY ALL
FROM tbl WHERE s LIKE '%string999999%' LIMIT 5
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
select * from t3 group by cube(a, b, c) order by all
SELECT * FROM t1 where rowid IN (6, 9) ORDER BY ALL
SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
EXPLAIN SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
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
from range(1,5) as t(days), nextValue(timeseries, date, value, '2026-03-29 05:33:11.822+08'::timestamp - INTERVAL (days) DAY) limit 5
WITH cte AS ( SELECT a, b, c::TIMESTAMPTZ AS c FROM t2 ) SELECT * FROM cte QUALIFY ROW_NUMBER() OVER (PARTITION BY a ORDER BY c DESC) = 1
SELECT q01.* FROM ( SELECT LHS.*, observation_period_start_date FROM ( SELECT q01.* FROM ( SELECT person_id, cohort_start_date, COALESCE(cohort_end_date, cohort_start_date) AS cohort_end_date FROM cohort ) q01 WHERE (cohort_start_date <= cohort_end_date) ) LHS INNER JOIN obs ON (LHS.person_id = obs.person_id) ) q01 WHERE (cohort_end_date >= observation_period_start_date) ORDER BY ALL
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
from pragma_collations() where collname like 'n%' order by all
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT c1, STRING_AGG(c2, ',' order by c2) as c3 GROUP BY c1 HAVING len(c3) > 7
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT c1, STRING_AGG(c2, ',' order by c2) as c3 GROUP BY c1 HAVING c3.len() > 7
SELECT a, row_number() OVER (ORDER BY a) AS rn FROM (VALUES (3),(1),(2)) t(a) QUALIFY alias.rn <= 2 ORDER BY a
SELECT a + 1 AS x, row_number() OVER (ORDER BY alias.x) AS rx FROM (VALUES (10),(20),(30)) t(a) QUALIFY alias.rx = 2 ORDER BY a
SELECT a AS "MiXeD", row_number() OVER (ORDER BY a) AS r FROM (VALUES (2),(1)) t(a) QUALIFY alias.r = 1 ORDER BY a
SELECT v AS x, row_number() OVER (ORDER BY v) r FROM alias QUALIFY alias.r <= 2 ORDER BY v
SELECT c1, row_number() over(partition BY c1 order by c2) as rk FROM VALUES ('a', 1), ('b', 2), ('b', 3), ('c', 4) AS t(c1, c2) qualify rk.add(1) > 2
FROM VALUES ('CS', 'Bachelor'), ('CS', 'Bachelor'), ('CS', 'PhD'), ('Math', 'Masters') AS t(c1, c2) SELECT list(c2) over(partition BY c1) AS "c3" QUALIFY "c3".len() = 1
select function_name as raw, replace(raw, '_', ' ') as prettier from my_functions group by all
SELECT a, b, row_number() OVER (ORDER BY a)::VARCHAR AS rn FROM test_alias QUALIFY rn.length() > 0 ORDER BY a
SELECT a, b, row_number() OVER (ORDER BY a)::VARCHAR AS rn FROM test_alias QUALIFY alias.rn.length() > 0 ORDER BY a
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t LEFT JOIN s2.t USING (id) LEFT JOIN s3.t USING (id) ORDER BY ALL
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t RIGHT JOIN s2.t USING (id) RIGHT JOIN s3.t USING (id) ORDER BY ALL
SELECT id, s1.t.id, s2.t.id, s3.t.id, s1.t.payload, s2.t.payload2, s3.t.payload3 FROM s1.t FULL OUTER JOIN s2.t USING (id) FULL OUTER JOIN s3.t USING (id) ORDER BY ALL
SELECT t.k FROM integers AS 't'('k') ORDER BY ALL
SELECT t.k FROM integers t('k') ORDER BY ALL
SELECT (WITH keys AS (SELECT unnest(json_keys(example)) AS k), nonNull AS ( SELECT keys.k, example->keys.k AS v FROM keys WHERE nullif(v, 'null') IS NOT NULL ) SELECT json_group_object(nonNull.k, nonNull.v) FROM nonNull ) FROM testjson
CREATE OR REPLACE MACRO strip_null_value(jsonValue) AS ( WITH keys AS (SELECT UNNEST(json_keys(jsonValue)) AS k), nonNull AS ( SELECT keys.k, jsonValue->keys.k AS v FROM keys WHERE nullif(v, 'null') IS NOT NULL ) SELECT json_group_object(nonNull.k, nonNull.v) FROM nonNull )
FROM Stock ORDER BY item_id
FROM merge_distinct_target ORDER BY tableticker
FROM Stock ORDER BY ALL
FROM Items
FROM Stock
FROM Accounts WHERE username='user2'
FROM foo
FROM Entry ORDER BY type
FROM Totals ORDER BY ALL
FROM target ORDER BY id
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
SELECT ( EXISTS( ( SELECT DISTINCT outer_alltypes."BIGINT", outer_alltypes."INT" FROM all_types inner_alltypes_1 WHERE inner_alltypes_1."BIGINT" GROUP BY NULL ) UNION BY NAME ( SELECT inner2."FLOAT" from all_types inner2 ) ) IS DISTINCT FROM outer_alltypes."struct" ) FROM all_types outer_alltypes GROUP BY ALL
from v1
FROM (SELECT 42) t(x), (SELECT x, row_number() OVER () QUALIFY NULL)
FROM (SELECT 42) t(x), (SELECT x * 2 QUALIFY row_number() OVER () < 10)
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
CREATE MACRO plus1(x) AS (x + (SELECT COUNT(*) FROM (SELECT b, SUM(test.a) FROM test GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1)))
CREATE VIEW test.v AS SELECT * FROM test.t QUALIFY row_number() OVER (PARTITION BY b) = 1
SELECT b, SUM(a) FROM test.v GROUP BY b QUALIFY row_number() OVER (PARTITION BY b) = 1 ORDER BY ALL
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
FROM duckdb_secrets()
from duckdb_secrets()
from test_table_macro(1,2)
FROM db2.tbl
FROM db2.main.tbl
FROM db2.non_existent_table
CREATE MACRO checksum_macro.checksum(table_name) AS TABLE SELECT bit_xor(md5_number(COLUMNS(*)::VARCHAR)) FROM query_table(table_name)
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
from m()
CREATE OR REPLACE MACRO extract_many(x, y) AS (SELECT struct_pack(*COLUMNS(z -> z in y)) FROM (SELECT unnest(x)))
CREATE OR REPLACE MACRO extract_many(x, y) AS (SELECT struct_pack(*COLUMNS(lambda z: z in y)) FROM (SELECT unnest(x)))
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
from db1.v1
from duplicate_column_view
FROM tbl
FROM pragma_metadata_info()
FROM pragma_metadata_info('db1')
FROM pragma_metadata_info(NULL)
FROM v2
SELECT * EXCLUDE(value, description) FROM duckdb_profiling_settings()
SELECT * EXCLUDE(description) FROM duckdb_profiling_settings()
FROM sql_auto_complete(NULL)
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl QUALIFY row_number() OVER () ORD') LIMIT 1
SELECT duckdb_format_sql('SELECT list_transform([1, 2, 3], x -> x + 1)') = $$SELECT list_transform([1, 2, 3], x -> x + 1)$$
SELECT duckdb_format_sql('SELECT list_apply(list_filter([1,2,3,4,5], x -> x > 2), y -> y * 10)') = $$SELECT list_apply(list_filter([1, 2, 3, 4, 5], x -> x > 2), y -> y * 10)$$
SELECT duckdb_format_sql('SELECT [1, 2, 3].list_transform(x -> x + 1)') = $$SELECT [1, 2, 3].list_transform(x -> x + 1)$$
from values ('2024-01-15 15:30:00'::timestamp) as a(t) where date_trunc('day', t) = '2024-01-15 12:30:00'
FROM nested_lists
SELECT list_distinct([COLUMNS(*)]) FROM all_types
select list_reverse(list_reverse(columns(['int_array', 'varchar_array', 'nested_int_array', 'array_of_structs', 'timestamp_array', 'double_array', 'date_array', 'timestamptz_array']))) IS NOT DISTINCT FROM columns(['int_array', 'varchar_array', 'nested_int_array', 'array_of_structs', 'timestamp_array', 'double_array', 'date_array', 'timestamptz_array']) from test_all_types()
SELECT COALESCE(*COLUMNS(lambda c: {'title': c}.title IN ('a', 'c'))) FROM (SELECT NULL, 2, 3) t(a, b, c)
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
FROM demo(3, 0)
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
FROM demo(3, {'a': 2})
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(lambda x: x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, lambda x: x)
FROM test_all_types() GROUP BY ALL HAVING array_intersect(NULL, NULL)
select list_transform(bb, lambda x: [x, b]), bb, b from (select list(b) over wind as bb, first(b) over wind as b from test window wind as (order by a asc, b asc rows between 4 preceding and current row) qualify row_number() over wind >4)
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
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> 0 + z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> (z -> 'a')) AS row )
SELECT tag_product, list_aggr(list_transform( string_split(tag_product, ' '), word -> lower(word)), 'string_agg', ',') AS tag_material, FROM tbl GROUP BY tag_product ORDER BY ALL
SELECT 1, list_transform([5, 4, 3], x -> x + 1) AS lst GROUP BY 1
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(x -> x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, x -> x)
SELECT x FROM (VALUES (42)) t(x) GROUP BY x HAVING list_filter(NULL, lambda_param -> lambda_param = 1)
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, x -> x + num))
SELECT list_filter([[1, 2], NULL, [3], [4, NULL]], f -> list_count(macro_with_lambda(f, 2)) > 1)
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, a -> x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, (x, y) -> x + y + num))
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, (x, y) -> list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, x -> x + y + z))
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], x -> 0))
SELECT list_transform(list_filter([0, 1, 2, 3, 4, 5], x -> x % 2 = 0), y -> y * y)
SELECT list_filter(list_filter([2, 4, 3, 1, 20, 10, 3, 30], x -> x % 2 == 0), y -> y % 5 == 0)
SELECT list_filter(['apple', 'banana', 'cherry', 'kiwi', 'mango'], fruit -> contains(fruit, 'a'))
SELECT list_transform([[1, NULL, 2], [3, NULL]], a -> list_filter(a, x -> x IS NOT NULL))
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
CREATE MACRO my_transform(list) AS list_transform(list, x -> x * x)
CREATE MACRO my_filter(list) AS list_filter(list, x -> x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, (x, y) -> x + y)
select list_transform(bb, x->[x,b]), bb, b from (select list(b) over wind as bb, first(b) over wind as b from test window wind as (order by a asc, b asc rows between 4 preceding and current row) qualify row_number() over wind >4)
from tbl order by all
select variant_typeof(struct_pack(*COLUMNS(*))::VARIANT) test from test_all_types()
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
SELECT try_trim_null(COLUMNS(*)) FROM tbl
SELECT to_hex(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_hex(to_hex(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
SELECT to_binary(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_binary(to_binary(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
WITH all_types AS ( select * exclude(small_enum, medium_enum, large_enum) from test_all_types() ) SELECT make_timestamptz( CAST(century(CAST(a."interval" AS INTERVAL)) AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(txid_current() AS BIGINT), 'UTC') FROM all_types a
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
select sin(columns(df1.* exclude (key))) from df1 join df2 using(key)
select sin(columns(dfxx.* exclude (key))) from df1 join df2 using(key)
FROM integers SELECT i + 1
FROM integers LIMIT 2
FROM integers WHERE i IS NOT NULL
FROM integers ORDER BY i DESC NULLS FIRST
FROM integers SELECT DISTINCT i%2 WHERE i>0 ORDER BY ALL
FROM integers SELECT i%2 AS g, SUM(i) sum GROUP BY g HAVING sum IS NOT NULL ORDER BY ALL
FROM integers JOIN integers i2 USING (i)
FROM integers i1, integers i2 SELECT COUNT(*)
from ( (values (1), (2)) as t1 (a) cross join (values (3), (4)) as t2 (b) ) as t(x, y, z)
SELECT * FROM integers WHERE COLUMNS(*) IS NULL ORDER BY ALL
SELECT * FROM integers GROUP BY COLUMNS(*)
SELECT * FROM integers GROUP BY i HAVING COLUMNS(*)>42
FROM read_csv(*, *)
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
SELECT * FROM tbl ORDER BY COLUMNS('col1|col3')
SELECT * FROM tbl ORDER BY COLUMNS('col2|col3')
SELECT * FROM tbl ORDER BY COLUMNS('col2|col3') DESC
SELECT * FROM tbl ORDER BY COLUMNS('col2') DESC, COLUMNS('col3') ASC
SELECT * FROM tbl ORDER BY COLUMNS(lambda x: x[-1] IN ('2', '3'))
FROM tbl UNION FROM tbl ORDER BY COLUMNS('col2|col3') DESC
SELECT * FROM tbl ORDER BY COLUMNS('xxxx')
prepare v1 as select COLUMNS(?) from my_table
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
SELECT * FROM tbl WHERE COLUMNS(*) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 AND COLUMNS(*) IS NOT NULL ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['col1', 'col2']) >= 2 AND COLUMNS(['col1', 'col3']) < 10 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(['nonexistent']) >= 2 ORDER BY ALL
SELECT * FROM tbl WHERE COLUMNS(* EXCLUDE (col1, col2, col3)) >= 2 ORDER BY ALL
FROM (VALUES ('1', '2'), ('1'))
FROM (VALUES ('1'), ('1', '2'))
CALL check_peg_parser($TEST_PEG_PARSER$SELECT COLUMNS(lambda x: x LIKE 'col%') FROM integers$TEST_PEG_PARSER$)
CALL check_peg_parser($TEST_PEG_PARSER$SELECT integers.* EXCLUDE ('i')$TEST_PEG_PARSER$)
CALL check_peg_parser($TEST_PEG_PARSER$SELECT * EXCLUDE (db1.s1.t.c) FROM db1.s1.t, db2.s1.t$TEST_PEG_PARSER$)
FROM test_table
select COLUMNS(*)::INET from tbl
from query($$select col."$$ || getvariable('col_name') || $$"::$$ || getvariable('col_type') || ' from tbl')
from query('select extracted::' || getvariable('col_type') || ' from intermediate')
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
FROM shredded_values
FROM nested_shredded_values
FROM shredded_array
from bla2
from bla
from test
FROM duckdb_columns()
FROM duckdb_views()
FROM test
FROM read_blob('build/*/repository/*/*/parquet.duckdb_extension')
SELECT * EXCLUDE (OID, CONRELID, connamespace) FROM pg_catalog.pg_constraint
from t
FROM ddb
from ddb
from ddb.my_table
from ddb.main.my_table
from memory.main.ddb
from my_table
from main.my_table
FROM alias1.tbl1
FROM alias2.tbl1
FROM enc.test ORDER BY value
FROM enc.test
FROM dummy.tbl
FROM test.tbl1
FROM tbl1
SELECT * EXCLUDE (db1.s1.t.c) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (DB1.S1.T.C) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (s1.t.c) FROM db1.s1.t, (SELECT 42) t
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (db1.s1.t.c AS new_col) FROM db1.s1.t, db2.s1.t)
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (DB1.S1.T.C AS new_col) FROM db1.s1.t, db2.s1.t)
SELECT * EXCLUDE (new_col) FROM (SELECT * RENAME (s1.t.c AS new_col) FROM db1.s1.t, (SELECT 42) t)
FROM db1.integers
FROM table_in_db2
FROM table_in_db2_test_schema
with source as ( select i, i * 3 % 5 as permuted, if(permuted > 0, NULL, permuted) as missing from range(5) tbl(i) ) select i, permuted, fill(missing order by permuted) over (order by i) as filled from source qualify filled <> permuted
with source as ( select i, i * 5 % 11 as permuted, if(permuted < 6, NULL, permuted) as missing from range(11) tbl(i) ) select i, permuted, fill(missing order by permuted) over (partition by permuted // 5 order by i) as filled from source qualify filled is distinct from permuted order by i
with source as ( select i, i * 5 % 11 as permuted, if(permuted = 2, NULL, permuted) as missing, if(permuted < 4, NULL, permuted) as unsorted, from range(11) tbl(i) ) select i, permuted, fill(missing order by unsorted) over (order by i) as filled from source qualify filled is distinct from permuted order by i
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 0, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls first) over (order by i) as filled from source qualify filled is distinct from permuted
with source as ( select i, (i + 1) * 3 % 5 as permuted, if(permuted = 4, NULL, permuted) as missing from ( from range(5) tbl(i) union all select NULL::INTEGER as i ) t(i) ) select i, permuted, fill(missing order by permuted asc nulls last) over (order by i) as filled from source qualify filled is distinct from permuted
EXPLAIN SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
SELECT i, LAG(i, 1) OVER() AS i1 FROM range(3000) tbl(i) WHERE i % 2 = 0 QUALIFY i1 <> i - 2
select i, nth_value(i, 2049) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) n from range(2049) tbl(i) QUALIFY n IS NOT NULL
WITH alternate AS ( SELECT range r, IF(range % 2, range, NULL) s FROM range(3000) ) SELECT r, s, last(s IGNORE NULLS) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) l FROM alternate QUALIFY l <> r - ((r + 1) % 2)
WITH alternate AS ( SELECT range r, IF(range < 2100, NULL, range) s FROM range(3000) ) SELECT r, s, first(s IGNORE NULLS) over(ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) f FROM alternate QUALIFY (f IS NOT NULL AND r < 2100) OR (f IS NULL AND r >= 2100)
PREPARE sw1 AS SELECT i, row_number() OVER() AS row_no FROM range(10, 20) tbl(i) QUALIFY row_no <= ?::BIGINT
SELECT partition, LAG(col1) OVER w AS prev FROM partitioned_tbl WINDOW w AS (PARTITION BY partition ORDER BY col1) QUALIFY (col1 - prev) <> 2
SELECT partition1, partition2, LAG(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition1, partition2 ORDER BY col1) QUALIFY (col1 - prev) <> 6
SELECT count(*) FROM ( SELECT partition1, partition2, lag(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition1 ORDER BY col1) QUALIFY col1 - prev <> 2 )
SELECT count(*) FROM ( SELECT partition1, partition2, lag(col1) OVER w AS prev FROM partitioned_tbl2 WINDOW w AS (PARTITION BY partition2 ORDER BY col1) QUALIFY col1 - prev <> 3 )
CREATE TABLE test (a hugeint)
INSERT INTO test values ((-170141183460469231731687303715884105728)::hugeint), (-1111::hugeint), (-1::hugeint), (0::hugeint), (1::hugeint), (1111::hugeint)
SELECT * FROM test order by a
SELECT * FROM test order by a DESC
CREATE TABLE test(col1 INT, col2 INT2[][][][][][])
SELECT col1, col2 FROM test ORDER BY col1 NULLS LAST, col2
CREATE TABLE integers(i INTEGER, j INTEGER)
INSERT INTO integers VALUES (1, 1), (3, 3)
CREATE TABLE integers2(k INTEGER, l INTEGER)
INSERT INTO integers2 VALUES (1, 10), (2, 20)
SELECT COUNT(*) FROM (SELECT i, j, k, l FROM integers FULL OUTER JOIN integers2 ON integers.i=integers2.k LIMIT 2) tbl
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT 5
CREATE TABLE integers AS SELECT 5 k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM integers)
CREATE TABLE strings AS SELECT '5'::VARCHAR k
SELECT * FROM generate_series(0, 10000, 1) tbl(i) ORDER BY i DESC LIMIT (SELECT k FROM strings)
SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7
SELECT COUNT(*) FROM (SELECT * FROM range(5) UNION ALL SELECT * FROM range(5) LIMIT 7) tbl
SELECT * FROM generate_series(0,10,1) LIMIT 3 OFFSET -1
SELECT * FROM generate_series(0,10,1) LIMIT -3
SELECT * FROM generate_series(0,10,1) LIMIT -1%
CREATE TABLE integers AS SELECT -1 k
SELECT * FROM generate_series(0,10,1) LIMIT (SELECT k FROM integers)
SET default_null_order='nulls_first'
CREATE TABLE integers(g integer, i integer)
INSERT INTO integers values (0, 1), (0, 2), (1, 3), (1, NULL)
SELECT * FROM integers ORDER BY * DESC
SELECT * FROM integers ORDER BY * DESC NULLS LAST
create table t1 as from VALUES ('A', 1), ('B', 3), ('C', 12), ('A', 5), ('B', 8), ('C', 9), ('A', 10), ('B', 20), ('C', 3) t(a, b)
PRAGMA disabled_optimizers='compressed_materialization'
SELECT 42 ORDER BY -9223372036854775808
CREATE TABLE test (a INTEGER, b INTEGER)
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22)
SELECT a FROM test LIMIT 1
SELECT a FROM test LIMIT 1.25
SELECT a FROM test LIMIT 2-1
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte)
WITH cte AS (SELECT 3) SELECT * FROM range(10000000) LIMIT (SELECT * FROM cte) OFFSET (SELECT * FROM cte)
SELECT 'Test' LIMIT ?
PREPARE v1 AS SELECT 'Test' LIMIT ?
EXECUTE v1(1)
EXECUTE v1(0)
PREPARE v2 AS SELECT * FROM RANGE(1000000000) LIMIT ? OFFSET ?
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22), (14, 32), (15, 52)
SELECT a FROM test LIMIT 20 %
SELECT a FROM test LIMIT 40 PERCENT
SELECT a FROM test LIMIT 35%
SELECT a FROM test LIMIT 79.9%
CREATE TABLE integers(i INTEGER)
INSERT INTO integers VALUES (1), (NULL)
SELECT * FROM integers ORDER BY i
SELECT * FROM integers ORDER BY i NULLS FIRST
SELECT * FROM integers ORDER BY i NULLS LAST
select b from test where a = 12
SELECT b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY a
SELECT a, b FROM test ORDER BY a DESC
SELECT a, b FROM test ORDER BY b, a
SELECT a FROM test ORDER BY 2
SELECT a FROM test ORDER BY 'hello', a
SET order_by_non_integer_literal=true
SELECT a AS k, b FROM test UNION SELECT a, b AS k FROM test ORDER BY k
SELECT a AS k, b FROM test UNION SELECT a AS k, b FROM test ORDER BY k
PRAGMA verify_parallelism
CREATE TABLE test AS SELECT a FROM range(10000, 0, -1) t1(a)
SELECT * FROM test ORDER BY a
SELECT a FROM test ORDER BY a
PRAGMA default_order='DESCENDING'
PRAGMA default_order='ASC'
PRAGMA default_order())
PRAGMA default_order='UNKNOWN'
insert into test values (100), (25), (75), (50)
select * from test order by i
drop table test
insert into test values (10000), (2500), (7500), (5000)
insert into test values (1000000), (250000), (750000), (500000)
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY id
SELECT s1, s1.a FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), s1.a AS id FROM tbl_structs ORDER BY 1
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY i
SELECT UNNEST(s1), UNNEST(s2), i FROM tbl_structs ORDER BY 2 DESC
CREATE OR REPLACE TABLE t3(c VARCHAR)
INSERT INTO t3 VALUES ('19'), ('21'), ('22'), ('23')
CREATE OR REPLACE TABLE t2( a VARCHAR, b VARCHAR )
INSERT INTO t2 VALUES ('3', '8'), ('5', NULL), ('8', NULL), ('11', NULL)
CREATE OR REPLACE TABLE t1( a VARCHAR, c VARCHAR )
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 360
select o_orderkey, o_clerk, o_orderstatus, o_totalprice from orders_small order by o_orderkey NULLS FIRST, o_clerk NULLS FIRST, o_orderstatus NULLS FIRST, o_totalprice DESC NULLS LAST limit 10 offset 440
SET access_mode='read_only'
SET allowed_configs=['lock_configuration']
SET allowed_configs=['allowed_configs']
SET allowed_configs=['']
SET allowed_configs=['not_a_real_setting']
SET allowed_configs=['TimeZone']
SET lock_configuration=true
SET TimeZone='America/New_York'
SET Calendar='japanese'
RESET allowed_directories
SET enable_external_access=false
SET allowed_directories=[]
CREATE TABLE integers(i INT)
COPY (SELECT 42 i) TO 'permission_test.csv' (FORMAT csv)
RESET allowed_paths
SET allowed_paths=[]
RESET block_allocator_memory
SET block_allocator_memory='100MiB'
SET memory_limit='200MiB'
SET block_allocator_memory='-3%'
SET block_allocator_memory='150%'
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
SET default_order = 'ASCENDING'
SET default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'DESCENDING'
SET SESSION default_null_order = 'NULLS FIRST'
CREATE TABLE integers(i integer)
INSERT INTO integers VALUES (1), (2), (3), (NULL)
SELECT * FROM integers ORDER BY i DESC
SELECT FIRST(i ORDER BY i), LAST(i ORDER BY i) FROM integers
SELECT FIRST(i ORDER BY i DESC), LAST(i ORDER BY i DESC) FROM integers
create schema my_schema
select current_schema()
SET schema='my_schema'
drop schema my_schema
create schema schema1
SET errors_as_json=true
SELECT * FROM nonexistent_table
SELECT cbl FROM (VALUES (42)) t(col)
SECT cbl FROM (VALUES (42)) t(col)
select corr('hello', 'world')
SELECT 1/2
SELECT 1//2
SET integer_division=true
SET integer_division=false
create schema s1
create schema s2
use s1
use s2
reset schema
SELECT current_setting('max_execution_time')
SET max_execution_time=5000
RESET max_execution_time
SELECT name, value, input_type FROM duckdb_settings() WHERE name = 'max_execution_time'
SET max_execution_time=100
SELECT current_setting('operator_memory_limit')
SET operator_memory_limit='256MB'
RESET operator_memory_limit
SET operator_memory_limit='128MB'
SET operator_memory_limit=NULL
CREATE SCHEMA temp.s1
CREATE SCHEMA system.s1
set schema = 'temp'
set schema = 'system'
SELECT current_setting('null_order'), (SELECT value FROM duckdb_settings() WHERE name='null_order')
SET null_order='NULLS_FIRST'
RESET null_order
PRAGMA default_collation='NOCASE'
CREATE TABLE collate_test(s VARCHAR)
INSERT INTO collate_test VALUES ('hEllO'), ('WöRlD'), ('wozld')
SELECT COUNT(*) FROM collate_test WHERE 'BlA'='bLa'
SELECT * FROM collate_test WHERE s='hello'
SET disabled_optimizers=''
SET disabled_optimizers TO 'expression_rewriter'
SET disabled_optimizers TO 'expression_rewriter,filter_pushdown,join_order'
SELECT current_setting('disabled_optimizers')
SET disabled_optimizers TO 'expression_rewriteX'
SET debug_window_mode='unknown'
SELECT * FROM duckdb_settings()
SET default_order='unknown'
SET enable_external_access=true
SET enable_profiling='unknown'
SELECT * FROM range(3) UNION ALL SELECT NULL ORDER BY 1
SELECT * FROM range(3) ORDER BY 1
SELECT value FROM duckdb_settings() WHERE name='preserve_identifier_case'
CREATE SCHEMA MYSCHEMA
CREATE TABLE MYSCHEMA.INTEGERS(I INTEGER)
SELECT duckdb_tables.schema_name, duckdb_tables.table_name, column_name FROM duckdb_tables JOIN duckdb_columns USING (table_oid)
DROP SCHEMA MYSCHEMA CASCADE
SET profiling_mode='standard'
SET profiling_mode='detailed'
SET profiling_mode='all'
SET profiling_mode='unknown'
SET Calendar='gregorian'
SET TimeZone='pacific/honolulu'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'TimeZone'
SET TimeZone='Pacific/Honolooloo'
SET Calendar='Coptic'
SELECT current_setting('disabled_filesystems')
RESET disabled_filesystems
SET disabled_filesystems=''
SET disabled_filesystems='LocalFileSystem'
SET disabled_filesystems='LocalFileSystem,LocalFileSystem'
PRAGMA disable_verification
SELECT * FROM duckdb_secrets()
SELECT * FROM duckdb_extensions()
CREATE PERSISTENT SECRET my_s (TYPE S3)
CREATE PERSISTENT SECRET my_secret (TYPE S3)
SELECT current_setting('lock_configuration')
SET memory_limit='8GB'
RESET lock_configuration
SET lock_configuration=false
SET memory_limit='10GB'
SET custom_user_agent='something else'
RESET custom_user_agent
SELECT current_setting('custom_user_agent')
SET duckdb_api='something else'
SELECT regexp_matches(user_agent, '^duckdb/.*(.*)') FROM pragma_user_agent()
PRAGMA temp_directory=''
SET memory_limit='2MB'
CREATE TABLE t1 AS SELECT * FROM range(1000000)
RESET memory_limit
