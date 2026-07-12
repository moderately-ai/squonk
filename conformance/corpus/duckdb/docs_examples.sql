SELECT * EXCLUDE (col1) FROM tbl
SELECT * EXCLUDE (col1, col2) FROM tbl
SELECT * REPLACE (col1 / 1000 AS col1) FROM tbl
SELECT * RENAME (col1 AS new_col1) FROM tbl
SELECT COLUMNS(c -> c LIKE '%num%') FROM tbl
SELECT COLUMNS('number\d+') FROM tbl
SELECT city, sum(amount) FROM sales GROUP BY ALL
SELECT city, amount FROM sales ORDER BY ALL
FROM tbl SELECT city, amount
FROM tbl
FROM range(10) t(id) SELECT id * 2 AS doubled
SELECT [1, 2, 3] AS lst
SELECT ['a', 'b', 'c']
SELECT {'x': 1, 'y': 2, 'z': 3} AS point
SELECT MAP {'a': 1, 'b': 2} AS m
SELECT list_transform([1, 2, 3], x -> x + 1)
SELECT list_filter([1, 2, 3, 4], x -> x % 2 = 0)
SELECT list_reduce([1, 2, 3], (x, y) -> x + y)
PIVOT sales ON year USING sum(amount)
PIVOT sales ON year USING sum(amount) GROUP BY city
UNPIVOT monthly_sales ON jan, feb, mar INTO NAME month VALUE sales
SELECT * FROM monthly_sales UNPIVOT (sales FOR month IN (jan, feb, mar))
SELECT *, row_number() OVER (PARTITION BY city ORDER BY amount) FROM sales QUALIFY row_number() OVER (PARTITION BY city ORDER BY amount) = 1
SELECT city, amount FROM sales QUALIFY row_number() OVER (PARTITION BY city ORDER BY amount DESC) = 1
SELECT * FROM trades ASOF JOIN prices ON trades.symbol = prices.symbol AND trades.ts >= prices.ts
SELECT * FROM trades ASOF LEFT JOIN prices USING (symbol, ts)
SELECT * FROM t1 POSITIONAL JOIN t2
SELECT city FROM q1 UNION BY NAME SELECT city FROM q2
SELECT city FROM q1 UNION ALL BY NAME SELECT amount FROM q2
SELECT a[1] FROM tbl
SELECT a[1:3] FROM tbl
