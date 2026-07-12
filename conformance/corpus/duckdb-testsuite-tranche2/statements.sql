PRAGMA enable_verification
CREATE TABLE integers(i INTEGER, j INTEGER)
INSERT INTO integers BY NAME SELECT 42 AS j
INSERT INTO integers BY NAME SELECT 84 AS i
INSERT INTO integers BY NAME SELECT 99 AS j, 9 AS i
INSERT INTO integers BY POSITION SELECT 1 AS j, 10 AS i
FROM integers
CREATE TABLE "My Table"("My Column 1" INT, "My Column 2" INT)
INSERT INTO "My Table" BY NAME SELECT 1 AS "My Column 2"
FROM "My Table"
CREATE TABLE tbl ( price INTEGER, total_price AS ((price)::DATE) )
INSERT INTO integers BY POSITION VALUES (42, 84)
CREATE TABLE integers AS SELECT i, i%2 as j FROM generate_series(0,999999,1) tbl(i)
CREATE TABLE integers2 AS SELECT * FROM integers GROUP BY GROUPING SETS ((), (i), (i, j), (j))
SELECT SUM(i), SUM(j), COUNT(*), COUNT(i), COUNT(j) FROM integers
SELECT SUM(i), SUM(j), COUNT(*), COUNT(i), COUNT(j) FROM integers2
DROP TABLE integers
DROP TABLE integers2
CREATE TABLE integers AS SELECT case when i%2=0 then null else i end AS i, i%2 as j FROM generate_series(0,999999,1) tbl(i)
CREATE TABLE integers(i INTEGER)
BEGIN TRANSACTION
INSERT INTO integers VALUES (0), (1), (2)
SELECT COUNT(*) FROM integers
ROLLBACK
INSERT INTO integers SELECT i FROM range(100) tbl(i)
INSERT INTO integers SELECT NULL FROM range(100) tbl(i)
SELECT COUNT(i), SUM(i), MIN(i), MAX(i), COUNT(*) FROM integers
COMMIT
SELECT SUM(CASE WHEN i IS NULL THEN 1 ELSE 0 END) FROM integers
INSERT INTO integers SELECT * FROM integers
INSERT INTO integers VALUES (3, 4), (4, 3)
INSERT INTO integers VALUES (DEFAULT, 4)
INSERT INTO integers (i) SELECT j FROM integers
SELECT * FROM integers
INSERT INTO integers VALUES (1), (2), (3), (4), (5)
CREATE TABLE i2 AS SELECT 1 AS i FROM integers WHERE i % 2 <> 0
SELECT * FROM i2 ORDER BY 1
UPDATE i2 SET i=NULL
CREATE TABLE IF NOT EXISTS presentations(presentation_date Date NOT NULL UNIQUE, author VARCHAR NOT NULL, title VARCHAR NOT NULL, bio VARCHAR, abstract VARCHAR, zoom_link VARCHAR)
CREATE TABLE strings(i STRING)
INSERT INTO strings VALUES ('�(')
SELECT * FROM strings WHERE i = '�('
CREATE TABLE a(i integer, j integer)
INSERT INTO a VALUES (1, 2)
INSERT INTO integers SELECT 42
INSERT INTO integers SELECT CAST(NULL AS VARCHAR)
SET default_null_order='nulls_first'
CREATE TABLE strings(a VARCHAR)
INSERT INTO integers VALUES (3), (4), (NULL)
INSERT INTO strings SELECT * FROM integers
SELECT * FROM strings
UPDATE strings SET a=13 WHERE a='3'
SELECT * FROM strings ORDER BY cast(a AS INTEGER)
SET immediate_transaction_mode=true
INSERT INTO integers SELECT * FROM range(0, 5)
INSERT INTO integers SELECT * FROM range(0, 17)
INSERT INTO integers SELECT * FROM range(0, 1007)
INSERT INTO integers SELECT * FROM range(0, 3020)
INSERT INTO integers SELECT * FROM range(0, 3)
CREATE TABLE test (id INTEGER, a INTEGER)
INSERT INTO test VALUES (1, 1), (2, 2), (3, 3), (4, NULL)
SELECT * FROM test ORDER BY id
UPDATE test SET a=CASE WHEN id=1 THEN 7 ELSE NULL END WHERE id <= 2
UPDATE test SET a=17 WHERE id > 2
UPDATE test SET a=CASE WHEN id=4 THEN 1 ELSE NULL END
UPDATE test SET a=2 WHERE id >= 2 AND id <= 3
UPDATE test SET a=NULL WHERE id >= 3
UPDATE test SET a=id WHERE id != 3
UPDATE test SET a=NULL WHERE id != 3
UPDATE test SET a=3 WHERE id != 2
UPDATE test SET a=7 WHERE id != 3
UPDATE test SET a=CASE WHEN a IS NULL THEN 1 ELSE NULL END
UPDATE test SET a=NULL
BEGIN
CREATE TABLE t1(a VARCHAR(256) PRIMARY KEY, b INTEGER)
INSERT INTO t1 VALUES(' 4-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ', 2 + 1)
INSERT INTO t1 VALUES(' 34-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ', 18)
INSERT INTO t1 SELECT b, b + 1 FROM t1 WHERE b < 5
FROM t1
UPDATE t1 SET a = CONCAT(a, 'x') WHERE b % 2 = 0
CREATE TABLE test (a VARCHAR)
INSERT INTO test VALUES ('abcdefghijklmnopqrstuvwxyz')
INSERT INTO test SELECT a||a||a||a||a||a||a||a||a||a FROM test
DELETE FROM test WHERE length(a) = (SELECT MIN(length(a)) FROM test)
SELECT LENGTH(a) FROM test
UPDATE test SET a='a'
CREATE TABLE integers(id INTEGER, val INTEGER)
INSERT INTO integers SELECT i, i FROM range(10000) t(i)
PRAGMA checkpoint_threshold='1GB'
UPDATE integers SET val=val+1000000 WHERE id=1
UPDATE integers SET val=val+1000000 WHERE id=2
UPDATE integers SET val=val+1000000 WHERE id=3
SELECT COUNT(*) FROM integers WHERE val>1000000
CREATE TABLE tbl (key INT, fruit VARCHAR, cost INT)
INSERT INTO tbl VALUES (1, 'apple', 2), (2, 'orange', 3)
UPDATE tbl SET (key, fruit, cost) = (1, 'pear', 2)
SELECT * FROM tbl
UPDATE tbl SET (key, fruit, cost) = (2, 'apple', 3)
UPDATE tbl SET (key, fruit, cost) = 3
UPDATE tbl SET (key, fruit, cost) = ADD(key, cost)
CREATE TABLE test (a INTEGER)
INSERT INTO test VALUES (1), (2), (3), (NULL)
SELECT * FROM test ORDER BY a
UPDATE test SET a=NULL WHERE a=2
UPDATE test SET a=NULL WHERE a=3
UPDATE test SET a=10 WHERE a IS NULL
INSERT INTO test VALUES ('hello'), ('world')
UPDATE test SET a='test' WHERE a='hello'
UPDATE test SET a='test2' WHERE a='world'
DROP TABLE IF EXISTS test_stress_update_issue_19688
CREATE TABLE test_stress_update_issue_19688 ( id INTEGER, val INTEGER )
INSERT INTO test_stress_update_issue_19688 SELECT range AS id, range * 1000 AS val FROM range(1000)
SELECT COUNT(*) FROM test_stress_update_issue_19688
SELECT COUNT(DISTINCT id) FROM test_stress_update_issue_19688
DELETE FROM test WHERE a='hello'
UPDATE test SET a='hello'
INSERT INTO test VALUES ('a'), ('b'), ('c'), (NULL)
INSERT INTO test SELECT * FROM test
SELECT DISTINCT a FROM test ORDER BY a
UPDATE test SET a='aa' WHERE a='a'
UPDATE test SET a=NULL where a='world'
UPDATE test SET a='test2' WHERE a='test'
INSERT INTO test VALUES ('test'), ('world')
UPDATE test SET a=NULL WHERE a='world'
UPDATE test SET a='world' WHERE a IS NULL
INSERT INTO test VALUES (3)
SELECT * FROM test
SELECT * FROM test WHERE a=3
UPDATE test SET a=1
SELECT * FROM test WHERE a=1
UPDATE test SET a=4
INSERT INTO test VALUES (1), (2), (3)
UPDATE test SET a=a+1
DELETE FROM test
DROP TABLE test
CREATE TABLE src (a INTEGER)
INSERT INTO src VALUES (2)
SELECT * FROM src
UPDATE test SET a=test.a+s.a FROM src s
UPDATE test SET a=test.a+t.a FROM test t
UPDATE test SET a=t.a+s.a FROM test t, src s
UPDATE test SET a=s.q FROM (SELECT a+1 as q FROM src) s
CREATE VIEW vt AS (SELECT 17 as v)
UPDATE test SET a=v FROM vt
UPDATE test SET a=s.a FROM src s WHERE s.a = 2
UPDATE test t SET a=1 FROM src s WHERE s.a = t.a
UPDATE test t SET a=9 FROM src s WHERE s.a=t.a
CREATE TABLE student(id INTEGER, name VARCHAR, PRIMARY KEY(id))
INSERT INTO student SELECT i, 'creator' FROM RANGE(260001) tbl(i)
SELECT name FROM student WHERE id = 122879
SELECT name FROM student WHERE id = 122881
SELECT name FROM student WHERE id = 245780
SELECT name FROM student WHERE id = 150881
UPDATE student SET name = 'updator0' WHERE id = 122879
UPDATE student SET name = 'updator1' WHERE id = 122881
UPDATE student SET name = 'updator2' WHERE id = 245780
UPDATE student SET name = 'updator3' WHERE id = 150881
insert into student select i, 'creator' from range(130001) tbl(i)
select id, name from student where id=122881
UPDATE test SET a=4 WHERE a=1
UPDATE test SET a=5 WHERE a=2
UPDATE test SET a=6 WHERE a=3
UPDATE test SET a=a-3
UPDATE test SET a=7 WHERE a=4
UPDATE test SET a=8 WHERE a=5
UPDATE test SET a=9 WHERE a=6
UPDATE test SET a=NULL WHERE a=1
SELECT COUNT(*) FROM test WHERE a IS NULL
UPDATE test SET a=99 WHERE a IS NULL
SELECT SUM(a) FROM test
INSERT INTO test VALUES (4), (5), (6)
DELETE FROM test WHERE a < 4
SELECT * FROM test WHERE a=4
SELECT * FROM test WHERE a=5
UPDATE test SET a=9 WHERE a=5
UPDATE test SET a=7 WHERE a=3
UPDATE test SET a=8 WHERE a=4
CREATE TABLE t1 (id VARCHAR, new_id VARCHAR, tags VARCHAR[], g GEOMETRY)
INSERT INTO t1 VALUES ('A', 'B', ARRAY['tag1', 'tag2'], 'POINT(1 2)')
UPDATE t1 SET new_id = 'C' WHERE id='A'
UPDATE t1 SET tags = ['tag3'] WHERE id='A'
UPDATE t1 SET g = 'POINT(3 4)' WHERE id='A'
CREATE TABLE t2 (id INT, val VARCHAR, tags VARCHAR[], g GEOMETRY)
INSERT INTO t2 VALUES (1, 'A', ARRAY['tag1', 'tag2'], 'POINT (5 6)')
UPDATE t2 SET val = t1.new_id FROM t1 WHERE t2.val=t1.id
UPDATE t2 SET tags = ['tag3'] FROM t1 WHERE t2.val=t1.new_id
UPDATE t2 SET g = 'POINT(0 0)' FROM t1 WHERE t2.val=t1.new_id
CREATE TABLE a (b int)
UPDATE a SET b = b + 10
SELECT * FROM a
CREATE TABLE t1 (c0 INT)
INSERT INTO t1(c0) VALUES (1),(2),(3)
UPDATE t1 SET c0 = DEFAULT
SET wal_autocheckpoint='1TB'
create table test (id bigint primary key, c1 text)
insert into test (id, c1) values (1, 'foo')
insert into test (id, c1) values (2, 'bar')
begin transaction
delete from test where id = 1
update test set c1='baz' where id=2
commit
CREATE TABLE tbl(mycol INTEGER)
CREATE TABLE t(table_id BIGINT, val BOOLEAN)
INSERT INTO t VALUES (1, NULL)
WITH new_values(tid, new_val) AS ( VALUES (1, NULL) ) UPDATE t SET val=new_val FROM new_values WHERE table_id=tid
CREATE TABLE t(i int, j int)
INSERT INTO t SELECT ii, NULL FROM range(1024) tbl(ii)
select COUNT(j), MIN(j), MAX(j) from t
UPDATE t SET j = 1
CREATE TABLE tbl(i INTEGER)
INSERT INTO tbl FROM range(1000) t(i)
DELETE FROM tbl WHERE i BETWEEN 200 AND 300
CREATE TABLE a AS SELECT * FROM range(1000000) t1(i)
SELECT COUNT(*) FROM a
DELETE FROM a WHERE i%2=0
CREATE TABLE aggr (k int[])
INSERT INTO aggr VALUES ([0, 1, 1, 1, 4, 0, 3, 3, 2, 2, 4, 4, 2, 4, 0, 0, 0, 1, 2, 3, 4, 2, 3, 3, 1])
INSERT INTO aggr VALUES ([]), ([NULL]), (NULL), ([0, 1, 1, 1, 4, NULL, 0, 3, 3, 2, NULL, 2, 4, 4, 2, 4, 0, 0, 0, 1, NULL, 2, 3, 4, 2, 3, 3, 1])
SELECT COUNT(k) FROM aggr
DELETE FROM aggr
CREATE TABLE a(i INTEGER)
INSERT INTO a VALUES (42)
DELETE FROM a
CREATE TABLE t (id INT PRIMARY KEY, s TEXT, j BIGINT)
CREATE INDEX idx ON t(j)
INSERT INTO t VALUES (1, 'a', 10), (2, 'b', 20), (3, 'c', 30)
SELECT * FROM t ORDER BY id
DELETE FROM t WHERE id = 2
DELETE FROM t WHERE j = 30
INSERT INTO t VALUES (4, 'd', 40), (5, 'e', 50)
DELETE FROM t WHERE j > 10
DELETE FROM t
SELECT COUNT(*) FROM t
DELETE FROM t WHERE j >= 20
INSERT INTO t VALUES (2, 'new_20', 20), (3, 'new_30', 30)
CREATE TABLE a AS SELECT * FROM range(0, 10000, 1) t1(i)
SELECT COUNT(*) FROM a WHERE i >= 2000 AND i < 5000
DELETE FROM a WHERE i >= 2000 AND i < 5000
pragma threads=2
pragma verify_parallelism
INSERT INTO a SELECT * FROM range(0, 1024, 1)
DELETE FROM a WHERE i=0
DELETE FROM a WHERE i=1
DELETE FROM a WHERE i=1022
DELETE FROM a WHERE i=1023
TRUNCATE TABLE a
TRUNCATE a
INSERT INTO a VALUES (1), (2), (3)
DELETE FROM a USING (values (1)) tbl(i) WHERE a.i=tbl.i
DELETE FROM a USING (values (1)) tbl(i)
DELETE FROM a USING (values (1)) tbl(i), (values (1), (2)) tbl2(i) WHERE a.i=tbl.i AND a.i=tbl2.i
DELETE FROM a USING (values (4)) tbl(i) WHERE a.i=tbl.i
DELETE FROM a USING a a2(i) WHERE a.i>a2.i
create table integers as select * from generate_series(0, 9, 1)
create table integers2 as select * from generate_series(0, 9, 1)
DELETE FROM integers USING integers2
BEGIN transaction
CREATE or replace TABLE integers AS FROM range(10)
create table integers_copy as select * from integers
DELETE FROM integers USING range(100) RETURNING *
select * from integers_copy
create or replace table t1 as select range%10 a from range(1000)
create or replace table t2 as select range b from range(10)
create or replace table t2_copy as select * from t2
delete from t2 using t1 where a=b returning *
CREATE TABLE Stock(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
INSERT INTO Buy values(10, 1000)
INSERT INTO Buy values(30, 300)
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON FALSE WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance)
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (item_id, initial_stocks.balance)
FROM Stock ORDER BY item_id
MERGE INTO Stock AS s USING Buy AS b ON s.item_id = b.item_id WHEN MATCHED THEN UPDATE SET balance = balance + b.volume WHEN NOT MATCHED THEN INSERT VALUES (b.item_id, b.volume)
CREATE TABLE Sale(item_id int, volume int)
INSERT INTO Sale VALUES (10, 2200)
INSERT INTO Sale VALUES (20, 1900)
MERGE INTO Stock USING Sale ON Stock.item_id = Sale.item_id WHEN MATCHED AND Sale.volume > balance THEN ERROR WHEN MATCHED AND Sale.volume = balance THEN DELETE WHEN MATCHED AND TRUE THEN UPDATE SET balance = balance - Sale.volume WHEN MATCHED THEN ERROR WHEN NOT MATCHED THEN ERROR
CREATE TABLE dest (id INTEGER, val INTEGER)
CREATE TABLE src (id INTEGER, val INTEGER)
INSERT INTO dest VALUES (1, 10)
INSERT INTO src VALUES (1, 100), (2, 200)
MERGE INTO dest AS DBT_INTERNAL_DEST USING src AS DBT_INTERNAL_SOURCE ON DBT_INTERNAL_SOURCE.id = DBT_INTERNAL_DEST.id WHEN MATCHED THEN UPDATE SET "id" = DBT_INTERNAL_SOURCE."id", "val" = DBT_INTERNAL_SOURCE."val" WHEN NOT MATCHED THEN INSERT ("id", "val") VALUES ("id", "val")
SELECT * FROM dest order by all
DELETE FROM dest
DELETE FROM src
INSERT INTO dest VALUES (1, 10), (2, 20)
INSERT INTO src VALUES (1, 100)
MERGE INTO dest AS DBT_INTERNAL_DEST USING src AS DBT_INTERNAL_SOURCE ON DBT_INTERNAL_SOURCE.id = DBT_INTERNAL_DEST.id WHEN MATCHED THEN UPDATE SET "id" = DBT_INTERNAL_SOURCE."id", "val" = DBT_INTERNAL_SOURCE."val" WHEN NOT MATCHED BY SOURCE THEN UPDATE SET val=val+1
INSERT INTO Stock VALUES (5, 10), (10, 20), (20, 30)
MERGE INTO Stock USING (VALUES (5, 20), (10, 30)) new_accounts(item_id, balance) USING (item_id) WHEN MATCHED THEN UPDATE WHEN NOT MATCHED BY TARGET THEN INSERT WHEN NOT MATCHED BY SOURCE THEN DELETE
FROM Stock ORDER BY ALL
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED BY SOURCE THEN DELETE
CREATE TABLE Stock(item_id int NOT NULL, balance int, CHECK (balance>0))
MERGE INTO Stock USING (VALUES (1, 10)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, new_accounts.balance)
CREATE TABLE Items(item_id int NOT NULL, total_cost INTEGER, base_cost INTEGER, tax_cost INTEGER, CHECK (total_cost = base_cost + tax_cost))
INSERT INTO Items VALUES (1, 10, 8, 2)
MERGE INTO Items USING (VALUES (1, 15)) new_prices(item_id, total_cost) USING (item_id) WHEN MATCHED THEN UPDATE SET total_cost = new_prices.total_cost, base_cost = new_prices.total_cost - 2
FROM Items
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, DEFAULT)
MERGE INTO Stock USING (VALUES (20)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT (item_id) VALUES (new_accounts.item_id)
MERGE INTO Stock USING (VALUES (30)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT DEFAULT VALUES
FROM Stock order by all
UPDATE Stock SET balance=100
MERGE INTO Stock USING (VALUES (10)) reset_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE SET balance=DEFAULT WHEN NOT MATCHED THEN ERROR
CREATE TABLE Buys(item_id int, volume int)
INSERT INTO Buys VALUES (42, 100)
MERGE INTO Stock USING Buys USING (item_id) WHEN NOT MATCHED AND true THEN INSERT WHEN NOT MATCHED AND error('this should not be executed') THEN INSERT WHEN NOT MATCHED THEN ERROR
SELECT COUNT(*) FROM Stock
FROM Stock
CREATE TABLE Accounts(id INTEGER, username VARCHAR PRIMARY KEY, favorite_numbers INT[])
INSERT INTO Accounts VALUES (1, 'user1', NULL)
MERGE INTO Accounts USING ( VALUES (1, 'user2', [1, 2, 3]) ) new_account(id) USING (id) WHEN MATCHED THEN UPDATE WHEN NOT MATCHED THEN INSERT
FROM Accounts WHERE username='user2'
MERGE INTO Stock USING (VALUES (5, 10)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT *
MERGE INTO Stock USING (VALUES (6, 12)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT
MERGE INTO Stock USING (VALUES (0, 7)) new_accounts(balance, item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
MERGE INTO Stock USING (VALUES (12)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
CREATE TABLE t AS SELECT range a FROM generate_series(0,9) t(range)
CREATE TABLE v0 (v1 INTEGER PRIMARY KEY)
create table foo (bar integer)
insert into foo values (1)
merge into foo as f using (select 2 as bar) b on f.bar is not null when matched then update when not matched then insert
FROM foo
create or replace table aaa (id int, status varchar, flag int, starttime datetime, endtime datetime)
merge into aaa using ( select 1 as id, 'xx' as status, 1 as flag, now() as starttime, null as endtime ) as upserts on (upserts.id = aaa.id and aaa.flag =1::int and aaa.status = upserts.status) when matched then update set endtime = upserts.starttime when not matched then insert by name
CREATE TABLE Entry(type varchar, number int, text varchar, country VARCHAR, date DATE)
INSERT INTO Entry VALUES ('number', 50, NULL, NULL, NULL), ('text', NULL, 'Hello', NULL, NULL), ('country', NULL, NULL, 'Netherlands', NULL), ('date', NULL, NULL, NULL, DATE '2000-01-01')
CREATE TABLE NewEntry(type varchar, number int, text varchar, country VARCHAR, date DATE)
INSERT INTO NewEntry VALUES ('number', 100, NULL, NULL, NULL), ('text', NULL, 'World', NULL, NULL), ('country', NULL, NULL, 'Germany', NULL), ('date', NULL, NULL, NULL, DATE '2010-01-01')
MERGE INTO Entry USING NewEntry ON Entry.type=NewEntry.type WHEN MATCHED AND Entry.type='number' THEN UPDATE SET number=NewEntry.number WHEN MATCHED AND Entry.type='text' THEN UPDATE SET text=NewEntry.text WHEN MATCHED AND Entry.type='country' THEN UPDATE SET country=NewEntry.country WHEN MATCHED AND Entry.type='date' THEN UPDATE SET date=NewEntry.date WHEN MATCHED THEN ERROR
FROM Entry ORDER BY type
CREATE TABLE my_timeseries (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
insert into my_timeseries VALUES ('2025-09-15', 43, 39)
CREATE TABLE my_timeseries_new (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
insert into my_timeseries_new VALUES ('2025-09-15', 43, 39)
MERGE INTO my_timeseries old USING my_timeseries_new new ON ( old.x = new.x AND ( old.ts != new.ts OR old.x = 1 ) ) WHEN MATCHED THEN UPDATE
MERGE INTO my_timeseries old USING my_timeseries_new new USING(ts) WHEN MATCHED AND ( old.x IS DISTINCT FROM new.y ) THEN UPDATE
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON FALSE WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) RETURNING merge_action, *
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id) WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) RETURNING *
MERGE INTO Stock AS s USING Buy AS b ON s.item_id = b.item_id WHEN MATCHED THEN UPDATE SET balance = balance + b.volume WHEN NOT MATCHED THEN INSERT VALUES (b.item_id, b.volume) RETURNING *, merge_action
MERGE INTO Stock USING Sale ON Stock.item_id = Sale.item_id WHEN MATCHED AND Sale.volume > balance THEN ERROR WHEN MATCHED AND Sale.volume = balance THEN DELETE WHEN MATCHED AND TRUE THEN UPDATE SET balance = balance - Sale.volume WHEN MATCHED THEN ERROR WHEN NOT MATCHED THEN ERROR RETURNING Stock.item_id, merge_action, Stock.balance
WITH deleted_stocks(item_id) AS (VALUES (30)) MERGE INTO Stock USING deleted_stocks ON Stock.item_id = deleted_stocks.item_id WHEN MATCHED THEN DELETE RETURNING *, merge_action
CREATE TABLE Totals(item_id int, balance int)
INSERT INTO Buy values(10, 1000), (30, 300), (20, 2000)
MERGE INTO Totals USING (VALUES (10), (30)) Updates(item_id) ON Totals.item_id = Updates.item_id WHEN MATCHED THEN UPDATE SET balance = (SELECT SUM(volume) FROM Buy WHERE item_id=Totals.item_id) WHEN NOT MATCHED THEN INSERT VALUES (Updates.item_id, (SELECT SUM(volume) FROM Buy WHERE item_id=Updates.item_id))
FROM Totals ORDER BY ALL
INSERT INTO Buy values(10, 2000)
MERGE INTO Totals USING (VALUES (10), (20)) Updates(item_id) ON Totals.item_id = Updates.item_id WHEN MATCHED THEN UPDATE SET balance = (SELECT SUM(volume) FROM Buy WHERE item_id=Totals.item_id) WHEN NOT MATCHED THEN INSERT VALUES (Updates.item_id, (SELECT SUM(volume) FROM Buy WHERE item_id=Updates.item_id))
CREATE TABLE Totals(item_id int, balance int, biggest_item BOOL)
MERGE INTO Totals USING Buy USING (item_id) WHEN NOT MATCHED AND Buy.volume = (SELECT MAX(Volume) FROM Buy) THEN INSERT VALUES (Buy.item_id, Buy.volume, true) WHEN NOT MATCHED THEN INSERT VALUES (Buy.item_id, Buy.volume, false)
SELECT * FROM Totals ORDER BY item_id
CREATE TABLE dummy_edge(id INTEGER, ref_id INTEGER, "value" VARCHAR, note VARCHAR)
CREATE TABLE dummy_user(user_id INTEGER, "name" VARCHAR, email VARCHAR, created_at DATE)
CREATE TABLE dummy_null(id INTEGER, "value" INTEGER, optional_text VARCHAR)
MERGE INTO main.dummy_edge as target_0 USING dummy_user as ref_0 ON target_0.note = ref_0.name WHEN NOT MATCHED AND EXISTS ( SELECT id FROM main.dummy_null WHERE true ) THEN DO NOTHING
CREATE TABLE target(id INT PRIMARY KEY, val INT)
INSERT INTO target VALUES (1, 10), (2, 20)
MERGE INTO target AS t USING (VALUES (1, 99)) AS s(id, val) ON t.id = s.id AND t.val > (SELECT 5) WHEN MATCHED THEN UPDATE SET val = s.val
FROM target ORDER BY id
CREATE TABLE people (id INTEGER, name VARCHAR, salary FLOAT)
INSERT INTO people VALUES (1, 'John', 92_000.0), (2, 'Anna', 100_000.0)
INSERT INTO Stock (item_id) VALUES (5), (10), (20)
MERGE INTO Stock USING (VALUES (5, 10)) new_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE
MERGE INTO Stock USING (VALUES (10, 30)) new_accounts(item_id, balance) USING (item_id) WHEN MATCHED THEN UPDATE SET *
MERGE INTO Stock USING (VALUES (100, 20)) new_accounts(balance, item_id) USING (item_id) WHEN MATCHED THEN UPDATE BY NAME
CREATE TABLE tbl1 AS SELECT 1
SELECT * FROM tbl1
CREATE TABLE tbl2 AS SELECT 2 AS f
SELECT * FROM tbl2
CREATE OR REPLACE TABLE tbl3 AS SELECT 3
SELECT * FROM tbl3
CREATE OR REPLACE TABLE tbl1 AS SELECT 4
CREATE OR REPLACE TABLE tbl1 AS SELECT 'hello' UNION ALL SELECT 'world'
CREATE OR REPLACE TABLE tbl1 AS SELECT 5 WHERE false
CREATE TABLE tbl4(col1, col2) AS SELECT 1, 'hello'
SELECT * FROM tbl4
CREATE OR REPLACE TABLE tbl4(col1, col2) AS SELECT 2, 'duck'
CREATE TABLE test (x INTEGER[])
INSERT INTO test SELECT CASE WHEN x <= 520 THEN [0, 0] ELSE [0] END FROM generate_series(1, 2048) s(x)
CREATE TABLE test2 AS SELECT x FROM test
pragma enable_verification
SET VARIABLE location_var='boop'
CREATE SCHEMA db0
USE db0
CREATE TABLE t0 (a BIGINT PRIMARY KEY, b INT, c INT)
CREATE INDEX t0_idx ON t0 (b)
CREATE UNIQUE INDEX t0_uidx ON t0 (c)
CREATE UNIQUE INDEX t0_uidx2 ON db0.t0 (c)
create table t1 as select 'c1' as c1
CREATE OR REPLACE TABLE integers(i INTEGER, j INTEGER)
CREATE VIEW integers2 AS SELECT 42
CREATE TABLE IF NOT EXISTS integers(i INTEGER)
INSERT INTO integers VALUES (1, 2)
SELECT * FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i
SELECT i, i FROM range(5) tbl(i)
SELECT * FROM (SELECT i, i FROM range(5) tbl(i)) tbl
SELECT * FROM (SELECT i, i, i, i FROM range(5) tbl(i)) tbl
CREATE TABLE t1 AS SELECT i, i FROM range(5) tbl(i)
SELECT * FROM t1
CREATE TABLE t2 AS SELECT i, i, i, i FROM range(5) tbl(i)
SELECT * FROM (SELECT * FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i) tbl
CREATE TABLE t3 AS SELECT tbl1.i, tbl2.i FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i
SELECT * FROM t3
CREATE TABLE t4 AS SELECT * FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i
SELECT * FROM t4
CREATE TABLE T (a INTEGER USING COMPRESSION RLE)
DROP TABLE T
CREATE TABLE T (a INTEGER NOT NULL USING COMPRESSION RLE)
CREATE TABLE T (a INTEGER USING COMPRESSION RLE, b VARCHAR )
CREATE TABLE T (a INTEGER USING COMPRESSION RLE, b INTEGER USING COMPRESSION BITPACKING, C INTEGER USING COMPRESSION UNCOMPRESSED)
INSERT INTO T VALUES (1,1,1), (1,1,1), (1,1,1), (2,2,2), (2,2,2), (3,3,3)
SELECT * FROM T
SELECT compression FROM pragma_storage_info('T') WHERE segment_type ILIKE 'INTEGER' LIMIT 3
ALTER TABLE T RENAME COLUMN a TO a_1
ALTER TABLE T RENAME COLUMN b TO b_1
ALTER TABLE T RENAME COLUMN c TO c_1
ALTER TABLE T RENAME TO T_1
create schema s1
create table T ( vis enum ('hide', 'visible')[] )
select column_type from (describe T)
attach ':memory:' as db2
create schema schema2
create schema db2.schema3
create type schema2.foo as VARCHAR
create type db2.schema3.bar as BOOL
create table B ( vis schema2.foo[] )
insert into b values (['foo', 'bar'])
from b
create table C ( vis db2.schema3.bar[] )
insert into C values ([true])
CREATE TABLE tbl1(i INTEGER)
set variable location='my/location/path'
ATTACH ':memory:' AS test_db
CREATE TABLE test_db.sample AS SELECT i FROM range(100) t(i)
SELECT COUNT(*) FROM test_db.sample
ALTER DATABASE test_db SET ALIAS TO renamed_db
SELECT COUNT(*) FROM renamed_db.sample
ALTER DATABASE IF EXISTS non_existent SET ALIAS TO something_else
ATTACH ':memory:' AS another_db
ALTER SEQUENCE IF EXISTS seq OWNED BY x
ALTER TABLE IF EXISTS t0 ADD COLUMN c0 INT
ALTER TABLE IF EXISTS t0 ADD COLUMN IF NOT EXISTS c0 int
CREATE TABLE t0 (c0 INT)
ALTER TABLE t0 ADD COLUMN IF NOT EXISTS c0 int
ALTER TABLE t0 ADD COLUMN c1 int
INSERT INTO t0 VALUES (42, 43)
ALTER TABLE t0 ADD COLUMN IF NOT EXISTS c2 int
INSERT INTO t0 VALUES (42, 43, 44)
ALTER TABLE IF EXISTS t1 DROP COLUMN IF EXISTS c3
ALTER TABLE IF EXISTS t0 DROP COLUMN if EXISTS c3
ALTER TABLE t0 DROP COLUMN IF EXISTS c3
CREATE TABLE test(i INTEGER, j INTEGER NOT NULL)
INSERT INTO test VALUES (1, 1), (2, 2)
ALTER TABLE test ALTER COLUMN j DROP NOT NULL
CREATE TABLE test2(i INTEGER, j INTEGER)
INSERT INTO test2 VALUES (1, 1), (2, 2)
ALTER TABLE test2 ALTER COLUMN j DROP NOT NULL
DROP TABLE IF EXISTS test
CREATE TABLE test(i AS (1), j INTEGER NOT NULL)
INSERT INTO test VALUES (1), (2)
ALTER TABLE test ALTER COLUMN i DROP NOT NULL
CREATE TEMPORARY TABLE temp_drop_not_null_test(x INTEGER NOT NULL)
SELECT table_name, database_name, temporary FROM duckdb_tables() WHERE table_name='temp_drop_not_null_test'
CREATE TABLE t(i INTEGER, j INTEGER)
INSERT INTO t SELECT i, i FROM RANGE(2048) tbl(i)
INSERT INTO t VALUES(9999, NULL)
SELECT i FROM t WHERE j IS NULL
DROP TABLE IF EXISTS t
INSERT INTO t values(8888, 8888)
SELECT * FROM t WHERE j = 8888
INSERT INTO T SELECT 1,1 FROM RANGE(2048)
INSERT INTO t VALUES(2, 2)
ALTER TABLE t ALTER COLUMN j DROP NOT NULL
INSERT INTO t values(3, NULL)
INSERT INTO t VALUES(4, NULL)
INSERT INTO t VALUES(7, 7)
SELECT i FROM t
SELECT count(*) from t
INSERT INTO t VALUES (1, 1), (2, 2)
SELECT * FROM t
INSERT INTO t SELECT 5,5 from range(65534)
SELECT COUNT(*) FROM t WHERE j IS NULL
INSERT INTO t VALUES (1, 1), (2, 2), (3, null)
INSERT INTO t SELECT 4,4 FROM RANGE(65536)
INSERT INTO t VALUES (5, null)
SELECT * FROM t WHERE j IS NULL
INSERT INTO t SELECT 1,1 FROM RANGE(65536)
INSERT INTO t VALUES (3, null)
CREATE TABLE t0(c0 AS (1), c1 INT)
ALTER TABLE t0 ALTER c1 SET NOT NULL
create schema public
set schema=public
create table a1 (c int)
alter table public.a1 rename to a2
alter table a2 rename to a3
create view v1 as select 42
alter view public.v1 rename to v2
alter view v2 rename to v3
INSERT INTO tbl VALUES (999), (100)
ALTER TABLE tbl RENAME TO tbl2
ALTER TABLE tbl2 RENAME TO tbl3
ALTER TABLE tbl3 RENAME TO tbl4
ALTER TABLE tbl4 RENAME TO tbl5
CREATE TABLE tbl2(i INTEGER)
CREATE TABLE tbl3(i INTEGER)
CREATE TABLE tbl4(i INTEGER)
CREATE TEMPORARY TABLE temp_tbl(i INTEGER)
INSERT INTO temp_tbl VALUES (42)
SELECT table_name, database_name, temporary FROM duckdb_tables() WHERE table_name='temp_tbl'
ALTER TABLE temp.temp_tbl RENAME TO temp_tbl_renamed
create table MY_TABLE (i integer)
insert into MY_TABLE values(42)
alter table MY_TABLE rename to my_table
select * from my_table
select * from MY_TABLE
CREATE TABLE entry(i INTEGER)
INSERT INTO entry VALUES (1)
SELECT * FROM entry
ALTER TABLE entry RENAME TO entry2
CREATE TABLE entry(j INTEGER)
INSERT INTO entry VALUES (2)
ALTER TABLE entry2 RENAME TO entry3
CREATE TABLE entry(k INTEGER)
ALTER TABLE entry3 RENAME TO entry4
CREATE TABLE t1(i INTEGER)
INSERT INTO t1 VALUES (1), (2), (3)
CREATE TABLE t2(i VARCHAR)
INSERT INTO t2 VALUES (4), (5), (6)
DROP TABLE t2
ALTER TABLE t1 RENAME TO t2
SELECT i FROM t2 ORDER BY i
SELECT i FROM t1 ORDER BY i
ALTER TABLE t2 RENAME TO t3
DROP TABLE t3
CREATE TABLE t2 (i integer)
INSERT INTO t2 VALUES (7), (8), (9)
CREATE TABLE tbl(i INTEGER PRIMARY KEY, j INTEGER CHECK(j < 10))
INSERT INTO tbl VALUES (999, 4), (1000, 5)
INSERT INTO tbl VALUES (9999, 0), (10000, 1)
ALTER TABLE tbl RENAME TO new_tbl
INSERT INTO new_tbl VALUES (66, 6), (55, 5)
INSERT INTO tbl1 VALUES (999), (100)
ALTER TABLE tbl1 RENAME TO tbl2
DROP TABLE tbl2
CREATE VIEW v1 AS SELECT * FROM tbl
SELECT * FROM v1
CREATE UNIQUE INDEX i1 ON t0 (c0)
CREATE TABLE t3 (c0 INT)
DROP TABLE t0
CREATE TABLE t1 (i INTEGER)
INSERT INTO t1 VALUES (1)
INSERT INTO t1 VALUES (2)
SELECT * FROM t2
CREATE TABLE test AS SELECT {'t': 42} t
ALTER TABLE test ALTER t TYPE ROW(t VARCHAR) USING {'t': concat('hello', (test.t.t + 42)::varchar)}
ALTER TABLE test ALTER t TYPE ROW(t VARCHAR) USING {'t': concat('hello', (t.t + 42)::varchar)}
CREATE TABLE test(i INTEGER, j INTEGER)
ALTER TABLE test ALTER i SET DATA TYPE VARCHAR
SELECT * FROM test ORDER BY ALL
SELECT * FROM test WHERE i = '1'
ALTER TABLE test ALTER i SET DATA TYPE INTEGER
SELECT * FROM test WHERE i = 1
PRAGMA disable_verification
SELECT stats(i) FROM test LIMIT 1
CREATE TABLE tbl (col STRUCT(i INT))
INSERT INTO tbl SELECT {'i': range} FROM range(5000)
ALTER TABLE tbl ALTER col TYPE USING struct_insert(col, a := 42, b := NULL::VARCHAR)
INSERT INTO tbl VALUES ({'i': 10000, 'a': NULL, 'b': 'hello'})
CREATE TABLE test(i INTEGER CHECK(i < 10), j INTEGER)
ALTER TABLE test ALTER j SET DATA TYPE VARCHAR
PREPARE v1 AS SELECT * FROM test
EXECUTE v1
ALTER TABLE test ALTER i TYPE VARCHAR USING i::VARCHAR
ALTER TABLE test ALTER i TYPE INTEGER USING i::INTEGER
PREPARE v2 AS SELECT i+$1 FROM test
EXECUTE v2(1)
ALTER TABLE test ALTER i TYPE BIGINT USING i+100
CREATE INDEX i_index ON test(i)
DROP INDEX i_index
INSERT INTO test VALUES (3, 3)
ALTER TABLE test ALTER i SET DATA TYPE BIGINT
ALTER TABLE test ALTER i TYPE INTEGER USING 2*(i+j)
CREATE TABLE test(i INTEGER NOT NULL, j INTEGER)
INSERT INTO test VALUES ('hello', 3)
UPDATE test SET i='hello'
ALTER TABLE test ALTER j TYPE VARCHAR
CREATE TABLE test(i INTEGER UNIQUE, j INTEGER)
CREATE TABLE test(i AS (1), j INTEGER)
ALTER TABLE test RENAME COLUMN i TO k
INSERT INTO test (i, j) VALUES (1, 2), (2, 3)
INSERT INTO test (k, j) VALUES (1, 2), (2, 3)
PREPARE v1 AS SELECT i, j FROM test
PREPARE v2 AS SELECT * FROM test
SELECT i, j FROM test
START TRANSACTION
SELECT k FROM test
CREATE TABLE test( i INTEGER, j INTEGER )
CREATE TABLE test(i INTEGER, j INTEGER, PRIMARY KEY(i, j))
INSERT INTO test (i, j) VALUES (1, 1), (2, 2)
INSERT INTO test (k, j) VALUES (3, 3), (4, 4)
CREATE TABLE data(id INTEGER, x INTEGER)
ALTER TABLE data ALTER COLUMN id DROP DEFAULT
INSERT INTO data VALUES (1, 0), (2, 1)
ALTER TABLE data ALTER COLUMN x DROP DEFAULT
ALTER TABLE test ALTER j SET DEFAULT 3
INSERT INTO test (i) VALUES (3)
ALTER TABLE test ALTER COLUMN j DROP DEFAULT
INSERT INTO test (i) VALUES (4)
CREATE SEQUENCE seq
ALTER TABLE test ALTER j SET DEFAULT nextval('seq')
INSERT INTO test (i) VALUES (5), (6)
CREATE TABLE constrainty(i INTEGER PRIMARY KEY, j INTEGER)
ALTER TABLE constrainty ALTER j SET DEFAULT 3
INSERT INTO constrainty (i) VALUES (2)
SELECT * FROM constrainty
CREATE TEMPORARY TABLE temp_default_test(x INTEGER DEFAULT 1)
ALTER TABLE test ADD COLUMN k INTEGER
ALTER TABLE test ADD COLUMN l INTEGER
ALTER TABLE test ADD COLUMN m INTEGER DEFAULT 3
ALTER TABLE test ADD COLUMN l INTEGER DEFAULT 3
SELECT i, j, l FROM test
ALTER TABLE test ADD COLUMN m INTEGER DEFAULT nextval('seq')
ALTER TABLE test ADD COLUMN n INTEGER DEFAULT currval('seq')
CREATE VIEW x(x) AS (SELECT 1)
ALTER TABLE test ADD COLUMN k INTEGER DEFAULT 2
CREATE INDEX i_index ON test(k)
INSERT INTO test VALUES (3, 3, 3)
SELECT * FROM test WHERE k=2
SELECT * FROM test WHERE k=3
SELECT * FROM test WHERE m=2
SELECT stats(m) FROM test LIMIT 1
CREATE SCHEMA test_schema
CREATE TYPE main_int AS int32
CREATE TYPE test_schema.test_int AS int32
CREATE TABLE test_schema.test_t1 (i INT)
CREATE TABLE main_t1 (i INT)
ALTER TABLE test_schema.test_t1 ADD COLUMN not_found main_int
ALTER TABLE test_schema.test_t1 ADD COLUMN l test_int
CREATE TABLE test(s STRUCT(s2 STRUCT(v1 INT, v2 INT)))
INSERT INTO test VALUES (ROW(ROW(1, 1))), (ROW(ROW(2, 2)))
ALTER TABLE test ADD COLUMN s.s2.k INTEGER
ALTER TABLE test ADD COLUMN IF NOT EXISTS s.s2.v1 VARCHAR
ALTER TABLE test ADD COLUMN s.i INTEGER DEFAULT 100
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER))
INSERT INTO test VALUES (ROW(1, 1)), (ROW(2, 2))
ALTER TABLE test ADD COLUMN s.k INTEGER
ALTER TABLE test ADD COLUMN s.l INTEGER DEFAULT 42
ALTER TABLE test ADD COLUMN s.m INTEGER DEFAULT 42
ALTER TABLE test ADD COLUMN IF NOT EXISTS s.i VARCHAR
CREATE TABLE test(s STRUCT(i INT, s2 STRUCT(v1 INT, v2 INT)))
INSERT INTO test VALUES (ROW(42, ROW(1, 1))), (ROW(84, ROW(2, 2)))
ALTER TABLE test DROP s.s2.v1
ALTER TABLE test DROP COLUMN IF EXISTS s.s2.v1
ALTER TABLE test DROP COLUMN s.s2
ALTER TABLE test DROP COLUMN s.i
ALTER TABLE test DROP COLUMN IF EXISTS s.v
ALTER TABLE test RENAME s.s2.v1 TO i
ALTER TABLE test RENAME COLUMN s.s2 TO x
ALTER TABLE test RENAME s.i TO v1
ALTER TABLE test RENAME s.j TO v2
CREATE TABLE test (i INTEGER, j INTEGER, d TEXT)
INSERT INTO test VALUES (3, 4, 'hello'), (44, 45, '56')
ALTER TABLE test ADD PRIMARY KEY (i, j)
INSERT INTO test VALUES (1, 1, 'foo'), (1, 2, 'bar')
CREATE TABLE test (i INTEGER, j INTEGER)
INSERT INTO test VALUES (1, 1)
ALTER TABLE test ADD PRIMARY KEY (j)
INSERT INTO test VALUES (1, 2)
CREATE TABLE reverse (i INTEGER, j INTEGER)
INSERT INTO reverse VALUES (1, 2)
ALTER TABLE reverse ADD PRIMARY KEY (j, i)
CREATE TABLE scan (i INTEGER, j INTEGER)
INSERT INTO scan SELECT range, range + 1 FROM range(30000)
ALTER TABLE scan ADD PRIMARY KEY (i)
SELECT * FROM scan WHERE i = 2
CREATE TEMPORARY TABLE temp_pk_test(x INTEGER)
ALTER TABLE test ALTER COLUMN j SET NOT NULL
ATTACH ':memory:' as memory
USE memory
DETACH test_add_pk_attach
USE test_add_pk_attach
CREATE TABLE uniq (i INTEGER UNIQUE, j INTEGER)
INSERT INTO uniq VALUES (1, 10), (2, 20), (3, 30)
ALTER TABLE uniq ADD PRIMARY KEY (i)
CREATE TABLE integers(i integer)
INSERT INTO integers SELECT * FROM range(50000)
SELECT i FROM integers WHERE i = 100
DELETE FROM integers WHERE i = 42
ALTER TABLE integers ADD PRIMARY KEY (i)
CREATE TABLE duplicates (i INTEGER, j INTEGER)
INSERT INTO duplicates VALUES (1, 10), (2, 20), (3, 30), (1, 100)
CREATE TABLE nulls (i INTEGER, j INTEGER)
INSERT INTO nulls VALUES (1, 10), (2, NULL), (3, 30), (4, 40)
DROP TABLE nulls
INSERT INTO nulls VALUES (5, 10), (NULL, 20), (7, 30), (8, 100)
CREATE TABLE nulls_compound (i INTEGER, j INTEGER, k VARCHAR)
INSERT INTO nulls_compound VALUES (1, 10, 'hello'), (2, 20, 'world'), (NULL, NULL, NULL), (3, 100, 'yay')
CREATE TABLE test (a INTEGER[], b INTEGER)
CREATE TABLE tbl (i INTEGER)
INSERT INTO tbl VALUES (1)
CREATE INDEX PRIMARY_tbl_i ON tbl(i)
CREATE TABLE test (i INTEGER)
INSERT INTO test VALUES (1)
INSERT INTO test VALUES (1, 1), (2, 1), (2, NULL)
CREATE TABLE other (i INTEGER, j INTEGER)
INSERT INTO other VALUES (1, 1), (2, 1)
ALTER TABLE other ADD PRIMARY KEY (j)
PRAGMA disable_checkpoint_on_shutdown
PRAGMA wal_autocheckpoint='1TB'
INSERT INTO test VALUES (1, 2), (3, 4)
CREATE TABLE test ( a INT NOT NULL, b INT GENERATED ALWAYS AS (a) VIRTUAL, c INT, )
INSERT INTO test VALUES (5, 4)
ALTER TABLE test ADD PRIMARY KEY (c)
CREATE TABLE other (i INTEGER PRIMARY KEY, j INTEGER)
WITH cte as ( select a::MAP(STRUCT(n INTEGER, m INTEGER), STRUCT(i INTEGER, j INTEGER)) a from VALUES (MAP {ROW(3,3): ROW(1, 1)}), (MAP {ROW(4,4): ROW(2, 2)}) t(a) ) SELECT remap_struct( a, NULL::MAP(STRUCT(n INTEGER, m INTEGER), STRUCT(i INTEGER, j INTEGER, k INTEGER)), { 'key': 'key', 'value': ( 'value', { 'i': 'i', 'j': 'j' } ) }, { 'value': { 'k': NULL::INTEGER } } ) from cte
CREATE TABLE test( s MAP( STRUCT( n INTEGER, m INTEGER ), STRUCT( i INTEGER, j INTEGER ) ) )
INSERT INTO test VALUES (MAP {ROW(3,3): ROW(1, 1)}), (MAP {ROW(4,4): ROW(2, 2)})
ALTER TABLE test ADD COLUMN s.key.k INTEGER
select * from test
ALTER TABLE test ADD COLUMN s.value.b VARCHAR
drop table test
CREATE TABLE test( s STRUCT( a MAP( STRUCT( n INTEGER, m INTEGER ), STRUCT( i INTEGER, j INTEGER ) ) ) )
INSERT INTO test VALUES (ROW(MAP {ROW(3,3): ROW(1, 1)})), (ROW(MAP {ROW(4,4): ROW(2, 2)}))
ALTER TABLE test ADD COLUMN s.a.key.k INTEGER
ALTER TABLE test ADD COLUMN s.a.value.b VARCHAR
ALTER TABLE test DROP COLUMN s.value.j
ALTER TABLE test DROP COLUMN s.key.n
ALTER TABLE test DROP COLUMN s.a.key.m
ALTER TABLE test DROP COLUMN s.a.value.j
ALTER TABLE test RENAME COLUMN s.value.j TO abc
ALTER TABLE test RENAME COLUMN s.key.n TO def
ALTER TABLE test RENAME COLUMN s.a.key.m TO abc
ALTER TABLE test RENAME COLUMN s.a.value.j TO def
ALTER VIEW vw RENAME TO vw2
SELECT * FROM vw2
CREATE VIEW vw AS SELECT i+1 AS i FROM tbl
CREATE VIEW vw AS SELECT * FROM tbl
CREATE VIEW vw2 AS SELECT 1729 AS i
CREATE VIEW vw1 AS SELECT * FROM tbl1
ALTER VIEW vw1 RENAME TO vw2
ALTER VIEW vw2 RENAME TO vw3
ALTER VIEW vw3 RENAME TO vw4
SELECT * FROM vw1
WITH cte AS ( SELECT a::STRUCT(i INTEGER, j INTEGER)[] a FROM VALUES ([ROW(1, 1)]), ([ROW(2, 2)]) t(a) ) SELECT remap_struct( a, NULL::STRUCT(i INTEGER, j INTEGER, k INTEGER)[], {'list': ('list', {'i': 'i', 'j': 'j'})}, {'list': {'k': NULL::INTEGER}} ) FROM cte
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER)[])
INSERT INTO test VALUES ([ROW(1, 1)]), ([ROW(2, 2)])
ALTER TABLE test ADD COLUMN s.element.k INTEGER
CREATE TABLE test( s STRUCT( a STRUCT(i INTEGER, j INTEGER)[] ) )
INSERT INTO test VALUES (ROW([ROW(1, 1)])), (ROW([ROW(2, 2)]))
ALTER TABLE test ADD COLUMN s.a.element.k INTEGER
ALTER TABLE test DROP COLUMN s.element.j
ALTER TABLE test DROP COLUMN s.a.element.i
CREATE TABLE test( s STRUCT( i INTEGER, j INTEGER )[] )
ALTER TABLE test RENAME COLUMN s.element.j TO k
CREATE TABLE test( s STRUCT( a STRUCT( i INTEGER, j INTEGER )[] ) )
ALTER TABLE test RENAME COLUMN s.a.element.i TO k
ALTER TABLE test DROP COLUMN j
CREATE TABLE test(i INTEGER, j INTEGER CHECK(j < 10))
CREATE TABLE test2(i INTEGER, j INTEGER CHECK(i+j < 10))
SELECT * FROM test2
ALTER TABLE test DROP COLUMN i
CREATE TABLE t1 (id INTEGER PRIMARY KEY, val INTEGER, extra INTEGER)
INSERT INTO t1 SELECT i, i * 10, i FROM range(1000) tbl(i)
DELETE FROM t1 WHERE id < 500
ALTER TABLE t1 DROP COLUMN extra
EXPLAIN ANALYZE SELECT val FROM t1 WHERE id = 5
SELECT val FROM t1 WHERE id = 499
EXPLAIN ANALYZE SELECT val FROM t1 WHERE id = 500
SELECT val FROM t1 WHERE id = 500
ALTER TABLE test DROP COLUMN IF EXISTS blabla
CREATE TABLE test2 (id INT PRIMARY KEY, name TEXT, surname TEXT, age INT, UNIQUE(surname, age))
CREATE TABLE test(i INTEGER, j INTEGER, k INTEGER NOT NULL)
INSERT INTO test VALUES (1, 1, 11), (2, 2, 12)
INSERT INTO test VALUES (3, 13)
INSERT INTO test SELECT i, i FROM range(100) tbl(i)
DELETE FROM test WHERE j%2=0
SELECT COUNT(j), SUM(j) FROM test
UPDATE test SET j=j+100
CREATE TABLE test(i INTEGER PRIMARY KEY, j INTEGER)
create table t(i int, j as (2), k int, m as (3), n int)
alter table t drop column n
alter table t drop column m
alter table t drop column k
alter table t drop column j
CREATE TABlE t1 (foo INT)
CREATE TABLE test_table (id INTEGER PRIMARY KEY)
INSERT INTO test_table VALUES (1)
SELECT id FROM test_table LIMIT 1
UPDATE test_table SET id = 1 WHERE id = 1
SELECT rowid FROM test_table LIMIT 1
CREATE TABLE t0(c0 BOOLEAN, c1 INT)
CREATE INDEX i0 ON t0(c1, c0)
INSERT INTO t0(c1) VALUES (0)
SELECT * FROM t0
CREATE TABLE duplicate_id (id UINT32, id2 INT64)
INSERT INTO duplicate_id SELECT range, range FROM range (0, 2048, 1)
INSERT INTO duplicate_id VALUES (2047, 2047)
DROP TABLE duplicate_id
CREATE TABLE int128_first (id INT128, id2 INT128)
INSERT INTO int128_first SELECT range, range FROM range(5000)
CREATE UNIQUE INDEX idx_1 ON int128_first(id, id2)
CREATE TABLE uint8_first (id UINT8, id2 UINT8)
INSERT INTO uint8_first SELECT range, range FROM range(128)
CREATE INDEX idx_2 ON uint8_first(id, id2)
CREATE TABLE uint64_first (id UINT64, id2 UINT32, id3 UINT64, id4 FLOAT)
INSERT INTO uint64_first SELECT range, range, range, 0.456 + range FROM range(5000)
CREATE TABLE numbers(i DOUBLE)
INSERT INTO numbers VALUES (CAST(0 AS DOUBLE))
INSERT INTO numbers VALUES (CAST(-0 AS DOUBLE))
CREATE INDEX i_index ON numbers(i)
SELECT COUNT(i) FROM numbers WHERE i = CAST(0 AS DOUBLE)
SELECT COUNT(i) FROM numbers WHERE i = CAST(-0 AS DOUBLE)
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR, l BIGINT)
CREATE INDEX i_index ON integers using art((j+l))
INSERT INTO integers VALUES (10, 1, 'hello', 4), (11, 2, 'world', 6)
SELECT * FROM integers WHERE j+l=5
SELECT * FROM integers WHERE k='hello'
UPDATE integers SET j=5, l=l WHERE j=1
UPDATE integers SET j=5 WHERE j=5
SELECT * FROM integers WHERE j+l=9
DELETE FROM integers WHERE j+l=8
DELETE FROM integers WHERE j+l=9
SELECT COUNT(*) FROM integers WHERE j+l>0
CREATE TABLE integers(i TINYINT, j SMALLINT, k INTEGER, l BIGINT)
CREATE INDEX i_index1 ON integers(i)
CREATE INDEX i_index2 ON integers(j)
CREATE INDEX i_index3 ON integers(k)
CREATE INDEX i_index4 ON integers(l)
SELECT i FROM integers WHERE i > 0
SELECT j FROM integers WHERE j < 0
SELECT k FROM integers WHERE k >= 0
SELECT l FROM integers WHERE l <= 0
INSERT INTO integers VALUES (1,1,1,1)
INSERT INTO integers VALUES (2,2,2,2)
INSERT INTO integers VALUES (3,3,3,3)
CREATE TABLE numbers(i REAL)
INSERT INTO numbers VALUES (CAST(0 AS REAL))
INSERT INTO numbers VALUES (CAST(-0 AS REAL))
SELECT COUNT(i) FROM numbers WHERE i = CAST(0 AS REAL)
SELECT COUNT(i) FROM numbers WHERE i = CAST(-0 AS REAL)
CREATE TABLE numbers(i REAL PRIMARY KEY, j INTEGER)
INSERT INTO numbers VALUES (3.45, 4), (2.2, 5)
SELECT * FROM numbers
INSERT INTO numbers VALUES (6, 6)
CREATE TABLE tbl ( u_2 UNION("string" VARCHAR, "bool" BOOLEAN), u_1 UNION("string" VARCHAR), i INTEGER, u_list UNION("int" INTEGER, "list" INTEGER[], "bool" BOOLEAN))
INSERT INTO tbl VALUES ('hello', 'world', 42, [1, 2, 3]), (NULL, NULL, NULL, NULL), (true, NULL, 44, 45), (false, 'wazzup', false, [1])
CREATE INDEX idx_i ON tbl (i)
DROP INDEX idx_i
SELECT * FROM tbl ORDER BY ALL
CREATE UNIQUE INDEX idx_u_2_1 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_u_2_2 ON tbl ((u_2.bool))
CREATE UNIQUE INDEX idx_u_1 ON tbl ((u_1.string))
CREATE UNIQUE INDEX idx_list_1 ON tbl ((u_list.int))
CREATE UNIQUE INDEX idx_list_3 ON tbl ((u_list.bool))
INSERT INTO tbl VALUES ('helloo', 'worldd', 43, [1, 2, 3, 4])
SELECT u_1.string FROM tbl WHERE u_2 = 'helloo'
CREATE TABLE strings(i varchar)
CREATE INDEX i_index ON strings(i)
SELECT COUNT(i) FROM strings WHERE i = 'test'
SELECT COUNT(i) FROM strings WHERE i = 'somesuperbigstring'
SELECT COUNT(i) FROM strings WHERE i = 'maybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstring'
SELECT COUNT(i) FROM strings WHERE i = 'maybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstring2'
SELECT COUNT(i) FROM strings WHERE i >= 'somesuperbigstring' and i <='somesuperbigstringz'
SELECT COUNT(i) FROM strings WHERE i = 'somesuperthisdoesnotexist'
DROP TABLE strings
SET immediate_transaction_mode = true
CREATE TABLE tbl_comp ( a INT, b VARCHAR UNIQUE, gen AS (2 * a), c INT, d VARCHAR, PRIMARY KEY (c, b))
CREATE UNIQUE INDEX unique_idx ON tbl_comp((d || 'hello'), (a + 42))
CREATE INDEX normal_idx ON tbl_comp(d, a, c)
CREATE UNIQUE INDEX lookup_idx ON tbl_comp(c)
INSERT INTO tbl_comp VALUES (1, 'hello', 1, 'hello')
INSERT INTO tbl_comp VALUES (2, 'hello', 1, 'world') ON CONFLICT (c, b) DO UPDATE SET a = excluded.a, d = excluded.d
SELECT a, b, gen, c, d FROM tbl_comp WHERE c = 1
INSERT INTO tbl_comp VALUES (3, 'hoi', 2, 'wereld')
SELECT a, b, gen, c, d FROM tbl_comp ORDER BY ALL
INSERT INTO tbl_comp VALUES (42, 'hoii', 22, 'welt')
DELETE FROM tbl_comp
CREATE TABLE test1 (id INT PRIMARY KEY, payload VARCHAR)
CREATE TABLE test2 (id INT PRIMARY KEY, payload VARCHAR)
INSERT INTO test1 VALUES (1, 'row 1')
INSERT INTO test2 VALUES (1, 'row 1 from test 2')
SELECT id, payload FROM test1
DELETE FROM test1 WHERE id = 1
INSERT INTO test1 SELECT * FROM test2
CREATE TABLE t_7182 (it INTEGER PRIMARY KEY, jt INTEGER)
CREATE TABLE u_7182 (iu INTEGER PRIMARY KEY, ju INTEGER REFERENCES t_7182 (it))
INSERT INTO t_7182 VALUES (1, 1)
INSERT INTO u_7182 VALUES (1, NULL)
UPDATE u_7182 SET ju = 1 WHERE iu = 1
SELECT iu, ju, rowid FROM u_7182 WHERE iu = 1
CREATE TABLE tunion_5807 (id INTEGER PRIMARY KEY, u UNION (i int))
INSERT INTO tunion_5807 SELECT 1, 41
UPDATE tunion_5807 SET u = 42 WHERE id = 1
SELECT id, u, rowid FROM tunion_5807 WHERE id = 1
CREATE TABLE IF NOT EXISTS workers_5771 ( id INTEGER PRIMARY KEY NOT NULL, worker VARCHAR(150) UNIQUE NOT NULL, phone VARCHAR(20) NOT NULL)
INSERT INTO workers_5771 VALUES (1, 'wagner', '123')
SET checkpoint_threshold = '10.0 GB'
CREATE TABLE tbl (id INT PRIMARY KEY)
DELETE FROM tbl WHERE id = 1
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT)
INSERT INTO a VALUES (1, 4)
INSERT INTO a SELECT i id, NULL c FROM range(-2, -250000, -1) tbl(i)
SELECT c FROM a WHERE id=1
INSERT INTO a SELECT i id, -i c FROM range(-2, -250000, -1) tbl(i)
CREATE TABLE tbl (i BIGINT PRIMARY KEY, l1 BIGINT[])
INSERT INTO tbl VALUES(1, [1, 2, 3]), (2, [42])
SELECT i, l1, rowid FROM tbl ORDER BY ALL
UPDATE tbl SET l1 = [1, 2, 4] WHERE i = 1
INSERT OR REPLACE INTO tbl VALUES (2, [43])
INSERT OR REPLACE INTO tbl VALUES (2, [44])
CREATE TABLE tbl(i INT PRIMARY KEY)
INSERT INTO tbl FROM range(100_000)
CREATE TABLE tbl2(i INT)
INSERT INTO tbl2 VALUES (42)
DELETE FROM tbl
DELETE FROM tbl2
ALTER TABLE tbl2 ADD COLUMN j INTEGER
FROM tbl WHERE i=50_000
DROP TABLE tbl
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl VALUES (1, ['first payload'])
INSERT INTO tbl VALUES (1, ['con1 payload'])
INSERT INTO tbl VALUES (1, ['con2 payload'])
SELECT id, payload, rowid FROM tbl WHERE id = 1
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_list VALUES (1, ['first payload'])
INSERT INTO tbl_list VALUES (5, ['old payload'])
DELETE FROM tbl_list
INSERT INTO tbl_list VALUES (1, ['con1 payload'])
SELECT id, payload, rowid FROM tbl_list WHERE id = 1
SELECT id, payload, rowid FROM tbl_list ORDER BY ALL
CREATE TABLE tbl_rollback (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_rollback VALUES (1, ['first payload'])
DELETE FROM tbl_rollback
INSERT INTO tbl_rollback VALUES (1, ['con1 payload'])
SELECT id, payload, rowid FROM tbl_rollback ORDER BY ALL
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR)
INSERT INTO tbl VALUES (1, 'first payload')
INSERT INTO tbl VALUES (5, 'old payload')
INSERT INTO tbl VALUES (1, 'con1 payload')
SELECT id, payload, rowid FROM tbl ORDER BY ALL
CREATE TABLE tbl(i INT PRIMARY KEY, v VARCHAR)
INSERT INTO tbl VALUES (1, 'row 1'), (2, 'row 2'), (3, 'row 3')
DELETE FROM tbl WHERE i=2
SELECT * FROM tbl WHERE i=2
INSERT INTO tbl VALUES (2, 'new row')
INSERT INTO tbl_list SELECT range, [range || ' payload'] FROM range(5)
UPDATE tbl_list SET id = id + 5 RETURNING id, payload
INSERT INTO tbl_list SELECT range + 10, [(range + 10) || ' payload'] FROM range(3000)
INSERT INTO tbl_list SELECT range, [range || ' payload'] FROM range(10)
DELETE FROM tbl_list USING range(100) t(i) RETURNING id, payload
UPDATE tbl SET payload = ['con1 payload'] WHERE id = 1
UPDATE tbl SET payload = ['con2 payload'] WHERE id = 1
INSERT INTO tbl_list VALUES (1, ['first payload']), (2, ['second payload'])
UPDATE tbl_list SET payload = ['con1 payload'] WHERE id = 1
UPDATE tbl_list SET id = 3 WHERE id = 2
INSERT INTO tbl_list VALUES (2, ['new payload'])
SELECT id, payload, rowid FROM tbl_list WHERE id = 2
SELECT id, payload, rowid FROM tbl_list WHERE id = 3
UPDATE tbl SET id = 3 WHERE id = 1
INSERT INTO tbl VALUES (1, 'new payload')
UPDATE tbl SET payload = 'second payload' WHERE id = 1
SELECT id, payload FROM tbl WHERE id = 1
SELECT id, payload FROM tbl WHERE id = 3
SELECT id, payload, rowid FROM tbl WHERE id = 3
UPDATE tbl_rollback SET payload = ['con1 payload'] WHERE id = 1
INSERT OR REPLACE INTO tbl VALUES (1, ['con1 payload'])
INSERT OR REPLACE INTO tbl VALUES (1, ['con2 payload'])
INSERT OR REPLACE INTO tbl_list VALUES (1, ['con1 payload'])
CREATE TABLE tbl_local (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_local VALUES (1, ['first payload'])
INSERT OR REPLACE INTO tbl_local VALUES (1, ['con1 payload'])
INSERT OR REPLACE INTO tbl_local VALUES (1, ['local payload'])
SELECT id, payload, rowid FROM tbl_local WHERE id = 1
INSERT OR REPLACE INTO tbl_local VALUES (1, ['val2 payload']), (1, ['val2 payload'])
INSERT OR REPLACE INTO tbl_rollback VALUES (1, ['con1 payload'])
INSERT INTO tbl_rollback VALUES (2, ['second payload'])
CREATE TABLE hero ( name VARCHAR NOT NULL, secret_name VARCHAR NOT NULL, age INTEGER, PRIMARY KEY (name))
CREATE INDEX ix_hero_age ON hero (age)
INSERT INTO hero (name, secret_name, age) VALUES ('Captain North America', 'Esteban Rogelios', 93), ('Rusty-Man', 'Tommy Sharp', 48), ('Tarantula', 'Natalia Roman-on', 32), ('Spider-Boy', 'Pedro Parqueador', 17), ('Captain North America', 'Esteban Rogelios', 93) ON CONFLICT (name) DO UPDATE SET secret_name = EXCLUDED.secret_name, age = EXCLUDED.age
CREATE TABLE kvp ( "key" VARCHAR PRIMARY KEY, "value" VARCHAR, expiration BIGINT, "cache" BOOLEAN)
CREATE INDEX kve_idx ON kvp (expiration)
INSERT OR REPLACE INTO kvp VALUES ('/key', 'value', 0, false)
SELECT key, value, expiration, cache FROM kvp
INSERT OR REPLACE INTO kvp VALUES ('/key', 'value', 10000000, false)
INSERT INTO kvp VALUES ('/key', 'value', 20000000, false) ON CONFLICT DO UPDATE SET value = excluded.value, expiration = excluded.expiration, cache = excluded.cache
CREATE TABLE duplicates (id UBIGINT)
INSERT INTO duplicates SELECT range + 500 FROM range(500)
INSERT INTO duplicates SELECT range FROM range(500)
INSERT INTO duplicates SELECT range + 1000 FROM range(500)
CREATE INDEX idx_duplicates ON duplicates(id)
SELECT id FROM duplicates WHERE id = 255
CREATE TABLE leaf_merge_1 (id UINT32, id2 INT64)
INSERT INTO leaf_merge_1 SELECT range, range FROM range (0, 2048, 1)
INSERT INTO leaf_merge_1 SELECT 2047, 2047 FROM range (10)
CREATE INDEX idx_merge_1 ON leaf_merge_1(id, id2)
CREATE TABLE leaf_merge_2 (id UINT32, id2 INT64)
INSERT INTO leaf_merge_2 SELECT range, range FROM range (0, 2048, 1)
CREATE INDEX i_index ON integers(i)
INSERT INTO integers VALUES (2)
DELETE FROM integers where rowid = 1
DELETE FROM integers where rowid = 2
DELETE FROM integers where rowid = 3
SELECT sum(i) FROM integers WHERE i <= 2
SELECT sum(i) FROM integers WHERE i > 4
DELETE FROM integers WHERE i = 0
SELECT sum(i) FROM integers WHERE i > 15
DELETE FROM integers WHERE i=16
INSERT INTO integers VALUES (16)
SELECT sum(i) FROM integers WHERE i > 1
CREATE TABLE n48_tbl(i varchar, k integer)
INSERT INTO n48_tbl SELECT 'a', range FROM range(10000)
INSERT INTO n48_tbl SELECT 'b', range FROM range(25)
INSERT INTO n48_tbl SELECT 'c', range FROM range(25)
CREATE INDEX n48_tbl_idx ON n48_tbl(i, k)
CREATE TABLE n48_free (id INTEGER)
INSERT INTO n48_free SELECT range % 100 FROM range(2048)
CREATE INDEX idx_n48_free ON n48_free(id)
CREATE TABLE db.t (id VARCHAR, ts TIMESTAMP, value INTEGER, PRIMARY KEY (id, ts))
INSERT OR IGNORE INTO db.t SELECT range || 'hello this is a long prefix', current_timestamp, range FROM range(1_000_000)
CHECKPOINT db
CREATE TABLE tbl (id INTEGER)
CREATE INDEX idx ON tbl(id)
INSERT INTO tbl VALUES (1), (2)
CREATE TABLE tbl_varchar (id VARCHAR)
CREATE INDEX idx_varchar ON tbl_varchar(id)
INSERT INTO tbl_varchar VALUES ('hello I am a prefix, and it is a beautiful sommer evening, and the plants are blossoming - 1'), ('hello I am a prefix, and it is a beautiful sommer evening, and the plants are blossoming - 2')
DELETE FROM tbl_varchar WHERE id = 'hello I am a prefix, and it is a beautiful sommer evening, and the plants are blossoming - 1'
DELETE FROM tbl_varchar
INSERT INTO tbl_varchar VALUES ('012345678901234'), ('012345678901235')
INSERT INTO tbl_varchar VALUES ('0123456789-0123456789-0123456789-0123456789')
INSERT INTO tbl_varchar VALUES ('0123456779-0123456789-0123456789-0123456789')
CREATE TABLE tbl1 (i INT)
INSERT INTO tbl1 SELECT range FROM range(50000)
DELETE FROM tbl1 WHERE i > 4
CREATE INDEX idx ON tbl1(i)
SELECT COUNT(i) FROM tbl1 WHERE i = 1
INSERT INTO t7 VALUES (42)
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR)
CREATE INDEX i_index ON integers using art(j)
INSERT INTO integers VALUES (10, 1, 'hello'), (11, 2, 'world')
SELECT i FROM integers WHERE i=10
SELECT * FROM integers WHERE i=10
SELECT j FROM integers WHERE j=1
SELECT * FROM integers WHERE j=1
SELECT k FROM integers WHERE k='hello'
SELECT i, k FROM integers WHERE k='hello'
CREATE INDEX i_index ON integers using art(i)
INSERT INTO integers VALUES (1, 2), (1, 3)
SELECT * FROM integers WHERE i = 1 AND j = 2
CREATE TABLE integers AS SELECT 42 AS i FROM range(2050)
INSERT INTO integers SELECT 42 + 1 + range FROM range(5000)
CREATE INDEX i_index ON integers USING ART(i)
SET index_scan_percentage = 1.0
SET index_scan_max_count = 0
EXPLAIN ANALYZE SELECT COUNT(i) FROM integers WHERE i = 42
SELECT COUNT(i) FROM integers WHERE i = 42
INSERT INTO integers SELECT i FROM RANGE(0, 1024, 1) t2(j), (VALUES (0), (1)) t1(i) ORDER BY j, i
SELECT COUNT(*) FROM integers WHERE i<1
SELECT COUNT(*) FROM integers WHERE i<=1
SELECT COUNT(*) FROM integers WHERE i=0
SELECT COUNT(*) FROM integers WHERE i=1
SELECT COUNT(*) FROM integers WHERE i>0
SELECT COUNT(*) FROM integers WHERE i>=0
INSERT INTO integers SELECT i FROM RANGE(0, 2048, 1) t2(j), (VALUES (0), (1)) t1(i) ORDER BY j, i
INSERT INTO integers SELECT * FROM range(-500, 500, 1)
SELECT sum(i) FROM integers WHERE i >= -500 AND i <= -498
SELECT sum(i) FROM integers WHERE i >= -10 AND i <= 5
SELECT sum(i) FROM integers WHERE i >= 10 AND i <= 15
CREATE TABLE varchars(v VARCHAR PRIMARY KEY)
INSERT INTO varchars VALUES ('hello'), ('hello' || chr(0)), ('hello' || chr(0) || chr(0)), ('hello' || chr(0) || chr(0) || chr(0))
SELECT * FROM varchars WHERE v = 'hello'
SELECT * FROM varchars WHERE v = 'hello' || chr(0)
SELECT * FROM varchars WHERE v = 'hello' || chr(0) || chr(0)
SELECT * FROM varchars WHERE v = 'hello' || chr(0) || chr(0) || chr(0)
CREATE TABLE blobs(b BLOB PRIMARY KEY)
SELECT * FROM blobs WHERE b = ''
INSERT INTO integers VALUES (1), (2), (4)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 2
SELECT i FROM integers WHERE i = 2
PREPARE v1 AS SELECT * FROM integers WHERE i = $1
EXPLAIN ANALYZE EXECUTE v1(2)
EXECUTE v1(2)
CREATE TABLE test (x VARCHAR PRIMARY KEY)
INSERT INTO test VALUES ('abc')
INSERT INTO test VALUES ('def')
SELECT * FROM test WHERE x > 'z'
INSERT INTO test VALUES ('abcd')
SELECT x FROM test WHERE x > 'abce'
INSERT INTO test VALUES ('abcd'), ('abde')
CREATE TABLE test (x USMALLINT PRIMARY KEY)
INSERT INTO test SELECT i FROM range(1, 20) tbl(i)
SELECT x FROM test WHERE x > 20
INSERT INTO test VALUES (256)
INSERT INTO test SELECT i FROM range(1, 135) tbl(i)
CREATE TABLE tab0(pk INTEGER PRIMARY KEY, col0 INTEGER, col1 FLOAT, col2 TEXT, col3 INTEGER, col4 FLOAT, col5 TEXT)
INSERT INTO tab0 VALUES(0,25,74.4,'vvcgn',47,57.68,'ymlye')
INSERT INTO tab0 VALUES(1,72,81.64,'zsnbm',42,74.55,'tzagd')
INSERT INTO tab0 VALUES(2,45,38.39,'dmsso',87,29.20,'ywydk')
INSERT INTO tab0 VALUES(3,81,97.79,'tdbjm',48,89.67,'hvaol')
INSERT INTO tab0 VALUES(4,17,18.5,'ddcya',66,87.1,'ndulx')
INSERT INTO tab0 VALUES(5,46,83.75,'khqpe',31,31.98,'hzpio')
INSERT INTO tab0 VALUES(6,85,8.45,'ugwie',30,22.61,'klsxt')
INSERT INTO tab0 VALUES(7,36,54.34,'pflrv',18,61.89,'vrltg')
INSERT INTO tab0 VALUES(8,47,41.84,'plpkl',76,65.31,'yzivj')
INSERT INTO tab0 VALUES(9,76,63.21,'uakya',80,80.58,'ocfgj')
CREATE TABLE tab1(pk INTEGER PRIMARY KEY, col0 INTEGER, col1 FLOAT, col2 TEXT, col3 INTEGER, col4 FLOAT, col5 TEXT)
CREATE TABLE t_1 (fIdx VARCHAR, sIdx UUID,)
CREATE TABLE t_3 (fIdx VARCHAR, sIdx UUID)
CREATE TABLE t_4 (sIdx UUID)
CREATE TABLE t_5 (sIdx UUID)
CREATE UNIQUE INDEX _pk_idx_t_5 ON t_5 (sIdx)
INSERT INTO t_4 (sIdx) VALUES ('1381e0ce-6b3e-43f5-9536-5e7af3a512a5'::UUID), ('6880cdba-09f5-3c4f-8eb8-391aefdd8052'::UUID), ('a3e876dd-5e50-3af7-9649-689fd938daeb'::UUID), ('e0abc0d3-63be-41d8-99ca-b1269ed153a8'::UUID)
WITH cte_5 AS ( SELECT sIdx FROM t_4 ANTI JOIN t_3 USING (sIdx) ), cte_6 AS MATERIALIZED ( SELECT COALESCE(cte_5.sIdx, t_1.sIdx) AS sIdx, COALESCE(t_1.fIdx, cte_5.sIdx::VARCHAR) AS fIdx, FROM cte_5 FULL JOIN t_1 USING (sIdx) ), cte_7 AS ( SELECT t_5.sIdx, FROM t_5 WHERE sIdx IN (SELECT sIdx FROM cte_6) ) SELECT fIdx, FROM cte_6 JOIN cte_7 USING (sIdx) ORDER BY fIdx
CREATE TABLE integers (i BIGINT)
CREATE INDEX idx_integers ON integers (i)
INSERT INTO integers (i) VALUES ('1'), ('-1'), ('1')
SELECT i FROM integers WHERE i <= 0
CREATE TABLE t0(c1 TIMESTAMP)
INSERT INTO t0(c1) VALUES ('2020-02-29 12:00:00'), ('1969-12-09 09:26:38'), ('2020-02-29 12:00:00')
CREATE INDEX i0 ON t0(c1)
SELECT c1 FROM t0 WHERE c1 <= '2007-07-07 07:07:07'
SET index_scan_max_count = 1
INSERT INTO integers SELECT 42 FROM range(1000)
INSERT INTO integers SELECT 43 FROM range(10000)
CREATE INDEX idx ON integers(i)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 42
SET index_scan_percentage = 0.000001
SET index_scan_max_count = 4000
INSERT INTO integers SELECT 4242 FROM range(4000)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 4242
CALL dbgen(sf=0.01)
CREATE TABLE random_orders AS ( (SELECT o_orderkey FROM orders OFFSET 100 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 + 100000 LIMIT 3))
CREATE TABLE orders_shuffled AS FROM orders ORDER BY random()
EXPLAIN ANALYZE SELECT o_orderkey FROM orders_shuffled WHERE o_orderkey IN ( SELECT UNNEST(LIST(o_orderkey)) FROM random_orders ) ORDER BY ALL
ALTER TABLE orders_shuffled ADD PRIMARY KEY (o_orderkey)
CREATE TABLE tbl AS SELECT range AS i FROM range(500000)
ALTER TABLE tbl ADD PRIMARY KEY(i)
EXPLAIN ANALYZE DELETE FROM tbl WHERE i IN (3, 50, 299, 123)
SELECT COUNT(*) FROM tbl
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101)
SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) ORDER BY i
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i != 42 AND i <= 100
SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i != 42 AND i <= 100 ORDER BY i
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i = 42 AND i <= 100
SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i = 42 AND i <= 100 ORDER BY i
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i < 101 AND i >= 42
SELECT i FROM tbl WHERE i IN (2, 42, 100, 42, 101) AND i < 101 AND i >= 42 ORDER BY i
create or replace table t as select id: uuid(), v: i from generate_series(1, 700000) s(i)
create unique index uid on t(id)
set variable u1 = uuid()
set variable u2 = uuid()
set variable u3 = uuid()
set variable u4 = uuid()
start transaction
insert into t select * replace (getvariable('u1') as id) from t using sample 1 rows
select * from t where id = getvariable('u1')
insert into t select * replace (getvariable('u2') as id) from t using sample 1 rows
select * from t where id = getvariable('u2')
insert into t select * replace (getvariable('u3') as id) from t using sample 1 rows
PRAGMA wal_autocheckpoint='400KB'
CREATE TABLE tbl AS SELECT range AS i FROM range(40000)
SELECT used_blocks FROM pragma_database_size()
CREATE INDEX idx ON tbl(i)
SELECT used_blocks > 0 FROM pragma_database_size()
CREATE TABLE tbl (i INTEGER PRIMARY KEY)
INSERT INTO tbl SELECT range FROM range(40000)
SET wal_autocheckpoint = '1TB'
CREATE UNIQUE INDEX idx_tbl_i ON tbl(i)
INSERT INTO tbl SELECT r FROM range(0, 251) t(r)
DELETE FROM tbl WHERE i BETWEEN 0 AND 250
INSERT INTO tbl SELECT r FROM range(251, 2049) t(r)
DELETE FROM tbl WHERE i BETWEEN 251 AND 2048
INSERT INTO tbl SELECT r FROM range(2049, 4096) t(r)
DELETE FROM tbl WHERE i BETWEEN 2049 AND 4095
INSERT INTO tbl VALUES (5000)
INSERT INTO tbl VALUES (6000)
DELETE FROM tbl WHERE i = 5000
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 5000
CREATE UNIQUE INDEX idx_i ON tbl (i)
SELECT i FROM tbl WHERE i = 12501
SELECT i FROM tbl WHERE i = 1
INSERT INTO tbl VALUES (60000)
INSERT INTO tbl VALUES (60001)
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 60000
SELECT i FROM tbl WHERE i = 60000
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 60001
SELECT i FROM tbl WHERE i = 60001
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 3000
SELECT i FROM tbl WHERE i = 3000
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 13000
SELECT i FROM tbl WHERE i = 13000
EXPLAIN ANALYZE SELECT i FROM tbl WHERE i = 23000
SELECT i FROM tbl WHERE i = 23000
CREATE TABLE tbl( col1 INTEGER, col2 INTEGER, idx_col INTEGER, gen_col INTEGER GENERATED ALWAYS AS (col1 + col2) VIRTUAL, col5 VARCHAR )
CREATE UNIQUE INDEX idx_tbl_idx_col ON tbl(idx_col)
INSERT INTO tbl (col1, col2, idx_col, col5) SELECT r, r * 2, r, 'val' || r::VARCHAR FROM range(0, 1001) t(r)
DELETE FROM tbl WHERE idx_col % 2 = 0
EXPLAIN ANALYZE SELECT idx_col FROM tbl WHERE idx_col = 0
SELECT idx_col FROM tbl WHERE idx_col = 0
EXPLAIN ANALYZE SELECT idx_col FROM tbl WHERE idx_col = 10
SELECT idx_col FROM tbl WHERE idx_col = 10
CREATE TABLE integers (i INTEGER PRIMARY KEY)
CREATE TABLE integers (i INTEGER)
INSERT INTO integers (SELECT range FROM range(512) UNION ALL SELECT 55)
SELECT total_blocks < 5 FROM pragma_database_size()
SELECT table_name, index_count FROM duckdb_tables() ORDER BY table_name
SELECT index_name, table_name FROM duckdb_indexes() ORDER BY index_name
SELECT table_name, constraint_type FROM duckdb_constraints() ORDER BY ALL
SELECT id, name FROM pk_tbl ORDER BY id
SELECT id FROM pk_tbl WHERE id = 2
SELECT i FROM idx_tbl WHERE i = 11
INSERT INTO idx_tbl SELECT range, range, range FROM range(300000)
SELECT used_blocks > 2621440 / get_block_size('test_art_import') FROM pragma_database_size()
CREATE INDEX ART_index ON idx_tbl(i)
SELECT i, j, k FROM idx_tbl WHERE i = 110 ORDER BY ALL
DROP INDEX idx_1
DROP INDEX idx_2
CREATE TABLE tracking("nflId" VARCHAR , "frameId" INTEGER, "gameId" INTEGER, "playId" INTEGER)
INSERT INTO tracking values ('a', 0,0,0)
CREATE INDEX nflid_idx ON tracking (nflid)
CREATE UNIQUE INDEX tracking_key_idx ON tracking (gameId, playId, frameId, nflId)
CREATE TABLE raw( "year" SMALLINT, "month" TINYINT, "day" TINYINT, "customer_ID" BIGINT )
INSERT INTO raw VALUES (1, 1, 1, 1)
CREATE UNIQUE INDEX customer_year_month_idx ON raw (customer_ID, year, month)
SET threads=1
SET memory_limit = '10MB'
CREATE TABLE tbl AS SELECT range AS id FROM range(200000)
FROM duckdb_memory()
CREATE TABLE tbl (i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
INSERT INTO tbl SELECT range, range FROM range (3000)
CREATE TABLE fk_tbl (i INTEGER, j INTEGER, FOREIGN KEY (i) REFERENCES tbl(i), FOREIGN KEY (j) REFERENCES tbl(j))
INSERT INTO fk_tbl SELECT range, range FROM range (3000)
CREATE INDEX idx_drop ON tbl(i)
SELECT index_name FROM duckdb_indexes()
CREATE TABLE integers(i integer,j integer)
INSERT INTO integers VALUES (1,1),(2,2),(3,3),(4,4),(5,5),
checkpoint
SELECT j FROM integers where i = 3
CREATE TABLE tbl_deser_scan(id INTEGER)
INSERT INTO tbl_deser_scan SELECT range FROM range(100000)
INSERT INTO tbl_deser_scan SELECT 424242 FROM range(5)
INSERT INTO tbl_deser_scan SELECT 424243 FROM range(5)
INSERT INTO tbl_deser_scan SELECT 1 FROM range(5)
CREATE INDEX idx_deser_scan ON tbl_deser_scan(id)
SELECT id FROM tbl_deser_scan WHERE id >= 424242
CREATE TABLE max_row_id AS SELECT max(rowid) AS id FROM tbl_deser_scan WHERE id = 424242
SET wal_autocheckpoint = '10GB'
CREATE TABLE history(id TEXT, type TEXT, PRIMARY KEY(id, type))
INSERT INTO history(id, type) VALUES ('5_create_aaaaaaaaaaa_mapping', 'sql')
INSERT INTO history(id, type) VALUES ('m0001_initialize', 'sql')
INSERT INTO history(id, type) VALUES ('m0005_create_aaaaaaaaaaa_mapping_table', 'sql')
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
INSERT INTO pk_integers VALUES (1)
CREATE TABLE pk_integers2(i INTEGER PRIMARY KEY)
INSERT INTO pk_integers2 VALUES (1)
SELECT i FROM pk_integers WHERE i = 1
CREATE TABLE minimal_tbl(i INTEGER)
CREATE UNIQUE INDEX idx_minimal ON minimal_tbl(i)
INSERT INTO minimal_tbl VALUES (42)
INSERT INTO minimal_tbl VALUES (43)
INSERT INTO minimal_tbl VALUES (44)
DELETE FROM minimal_tbl where i = 42
INSERT INTO test SELECT range + 42 FROM range(100)
CREATE TABLE alter_test (a INTEGER)
INSERT INTO alter_test SELECT range + 42 FROM range(100)
CREATE INDEX other_idx ON test(a)
INSERT INTO test VALUES (0), (1)
INSERT INTO alter_test VALUES (0), (1)
CREATE UNIQUE INDEX i_index ON test(a)
ALTER TABLE alter_test ADD PRIMARY KEY(a)
DROP TABLE alter_test
CREATE TABLE drop_test (a INTEGER)
INSERT INTO drop_test SELECT range + 42 FROM range(100)
INSERT INTO drop_test VALUES (0), (1)
CREATE INDEX drop_idx ON drop_test(a)
DROP INDEX drop_idx
DELETE FROM test WHERE a = 1
DELETE FROM alter_test WHERE a = 1
INSERT INTO alter_test VALUES (1)
CREATE INDEX drop_idx ON test(a)
CREATE TABLE tbl (u_2 UNION("string" VARCHAR, "bool" BOOLEAN))
INSERT INTO tbl VALUES ('helloo')
INSERT INTO tbl VALUES ('hellooo')
CREATE UNIQUE INDEX idx_1 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_2 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_3 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_4 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_5 ON tbl ((u_2.string))
CREATE TABLE source(i INTEGER)
INSERT INTO source VALUES (1), (2), (3), (4), (5), (6)
INSERT INTO integers SELECT * FROM source WHERE i % 2 = 0
SELECT * FROM integers WHERE i<3 ORDER BY 1
SELECT * FROM integers ORDER BY 1
SELECT * FROM integers WHERE i>3 ORDER BY 1
SELECT * FROM integers WHERE i<=3 ORDER BY 1
SELECT * FROM integers WHERE i>=3 ORDER BY 1
UPDATE integers SET i=3 WHERE i=4
DELETE FROM integers WHERE i>3
SELECT * FROM integers WHERE i > 0 ORDER BY 1
SELECT * FROM integers WHERE i < 3 ORDER BY 1
UPDATE integers SET i=10 WHERE i=1
SELECT * FROM integers WHERE i < 5
SELECT * FROM integers WHERE i > 0
INSERT INTO integers VALUES (1, 2), (2, 2)
UPDATE integers SET j=10 WHERE i=1
UPDATE integers SET j=10 WHERE rowid=0
DELETE FROM integers WHERE rowid=1
SELECT * FROM integers WHERE j>5
UPDATE integers SET i=100, k='update' WHERE j=1
UPDATE integers SET i=20, k='t1' WHERE j=1
UPDATE integers SET i=21, k='t2' WHERE j=2
SELECT * FROM integers WHERE j=2
SELECT * FROM integers ORDER BY j
CREATE OR REPLACE TABLE bar (col1 VARCHAR, col2 VARCHAR UNIQUE)
INSERT INTO bar (col1, col2) VALUES (NULL, 'one')
UPDATE bar AS original SET col1 = 'a'
SELECT col1 FROM bar WHERE col2 = 'one'
CREATE TABLE t1 (c1 DECIMAL(4, 3))
INSERT INTO t1(c1) VALUES (1), (-0.505)
CREATE INDEX i1 ON t1 (TRY_CAST(c1 AS USMALLINT))
INSERT INTO t1(c1) VALUES (2), (3)
CREATE TABLE t2 (c1 VARCHAR)
CREATE INDEX i2 ON t2 (c1)
INSERT INTO t2 VALUES ('\0')
CREATE INDEX i22 ON t2 (c1)
CREATE TABLE t3(c1 INT)
INSERT INTO t3 VALUES (0), (85491)
CREATE INDEX i3 ON t3 (c1, (TRY_CAST(c1 AS USMALLINT)))
CREATE TABLE t4 (c1 BOOLEAN)
CREATE TABLE t1 AS (SELECT 1 c1, 'a' c2)
CREATE INDEX i1 ON t1 (c1)
PRAGMA MEMORY_LIMIT='4MB'
INSERT INTO t1(c2) (SELECT DISTINCT 'b')
create or replace table test as select 9223372036854776 + range * 9223372036854776 i from range(100)
create index my_index on test(i)
explain analyze select i from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i)
select count(*) from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i)
select i from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i) order by all
create or replace table sample as select i from test using sample reservoir(10) repeatable (42)
explain analyze select i from test SEMI JOIN sample USING (i)
select count(*) from test SEMI JOIN sample USING (i)
select i from test SEMI JOIN sample USING (i) order by all
CREATE TABLE t0(c0 INT)
INSERT INTO t0(c0) VALUES (2)
UPDATE t0 SET c0=0
CREATE INDEX i1 ON t0(c0)
DELETE FROM t0
CREATE TABLE t0(c0 DOUBLE, c1 TIMESTAMP DEFAULT(TIMESTAMP '1970-01-04 12:58:32'))
INSERT INTO t0(c1, c0) VALUES (TIMESTAMP '1969-12-28 23:02:08', 1)
INSERT INTO t0(c0) VALUES (DEFAULT)
CREATE INDEX i2 ON t0(c1, c0)
CREATE TABLE path ( it INTEGER, x0 TEXT NOT NULL, x1 TEXT NOT NULL )
CREATE TABLE edge ( id INTEGER DEFAULT nextval('seq'), it INTEGER DEFAULT 0, x0 TEXT, x1 TEXT )
CREATE INDEX edge1_idx ON edge (x1)
INSERT INTO edge (x0, x1) VALUES ('n2880','n3966')
INSERT INTO path SELECT 1, y0, y1 FROM (SELECT DISTINCT edge0.x0 AS y0, edge0.x1 AS y1 FROM edge AS edge0 WHERE edge0.it = 0 AND true AND NOT EXISTS (SELECT * from path AS pre WHERE pre.x0 = edge0.x0 AND pre.x1 = edge0.x1))
SELECT 1, y0, y1 FROM (SELECT DISTINCT edge0.x0 AS y0, path1.x1 AS y1 FROM edge AS edge0,path AS path1 WHERE edge0.it = 0 AND edge0.x1 = path1.x0 AND NOT EXISTS (SELECT * from path AS pre WHERE pre.x0 = edge0.x0 AND pre.x1 = path1.x1))
CREATE TABLE key_value_pairs (key VARCHAR PRIMARY KEY, value VARCHAR)
INSERT INTO key_value_pairs SELECT concat('key_', i::VARCHAR), concat('value_', i::VARCHAR) FROM range(10000) t(i) WHERE random() < 0.5
CREATE TABLE keys_to_lookup (key VARCHAR PRIMARY KEY)
INSERT INTO keys_to_lookup SELECT concat('key_', i::VARCHAR) FROM range(100) t(i)
SELECT COUNT(*) FROM ( SELECT key, value FROM keys_to_lookup JOIN key_value_pairs USING(key) )
CREATE TABLE td(tz VARCHAR(30) NOT NULL)
CREATE UNIQUE INDEX sqlsim0 ON td(tz)
CREATE TABLE tab0(c2 DATE NOT NULL)
CREATE TABLE tab1(c2 DATE NOT NULL)
INSERT INTO td VALUES (date '2008-02-29')
INSERT INTO td VALUES('2006-12-25')
INSERT INTO tab0 VALUES('2006-12-25')
COMMIT TRANSACTION
INSERT INTO tab1 VALUES('2006-12-25')
SELECT tz FROM td ORDER BY tz
CREATE TABLE t14(c0 BIGINT)
INSERT INTO t14(c0) VALUES ((1)), ((1)), ((1))
CREATE INDEX i1 ON t14(c0 )
DELETE FROM t14 WHERE t14.rowid
create or replace table test as ( select cast(unnest(range(1000)) as varchar) as x, cast(unnest(range(2000,3000)) as varchar) as y, cast(unnest(range(3000,4000)) as varchar) as z )
create index test_x on test(x)
create view test_view as (select z, y, x from test)
explain analyze select * from test_view where x = '525'
select z, y, x from test_view where x = '525'
drop index test_x
create index test_upper_x on test(upper(x))
explain analyze select * from test_view where upper(x) = '526'
select z, y, x from test_view where upper(x) = '526'
drop index test_upper_x
CREATE TABLE v0 (v2 VARCHAR, v1 INT)
INSERT INTO v0 (v2 ,v1 ) VALUES ('358677 4 2 1', 7), ('a%', 1)
CREATE UNIQUE INDEX v3 ON v0 (v1, v1, v1, v1, v1, v2, v1, v2, v1, v2, v2, v1, v2, v2, v2, v2, v2, v2, v1, v1, v2, v2, v1, v1, v2, v1)
PRAGMA immediate_transaction_mode = True
CREATE TABLE tbl AS SELECT range AS i FROM range(100)
CREATE INDEX IF NOT EXISTS my_idx ON tbl(i)
SELECT COUNT(*) FROM duckdb_indexes
DROP INDEX my_idx
INSERT INTO integers SELECT * FROM range(10)
DELETE FROM integers WHERE i=2 OR i=7
SELECT * FROM integers WHERE i=1
SELECT * FROM integers WHERE i=2
DELETE FROM integers
INSERT INTO integers SELECT * FROM repeat(1, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(2, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(3, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(4, 1500) t1(i)
SELECT count(i) FROM integers WHERE i > 1 AND i < 3
SELECT count(i) FROM integers WHERE i >= 1 AND i < 3
SELECT count(i) FROM integers WHERE i > 1
SELECT count(i) FROM integers WHERE i < 4
SELECT count(i) FROM integers WHERE i < 5
INSERT INTO integers SELECT * FROM repeat(5, 1500) t1(i)
DELETE FROM integers WHERE i = 5
CREATE TABLE t0(c0 INTEGER)
CREATE UNIQUE INDEX i0 ON t0(c0)
INSERT INTO t0(c0) VALUES (1)
SELECT * FROM t0 WHERE t0.c0 = 1
CREATE TABLE merge_violation (id INT)
INSERT INTO merge_violation SELECT range FROM range(2048)
INSERT INTO merge_violation SELECT range + 10000 FROM range(2048)
INSERT INTO merge_violation VALUES (2047)
CREATE TABLE A (A1 INTEGER,A2 VARCHAR, A3 INTEGER)
INSERT INTO A VALUES (1, 1, 1)
INSERT INTO A VALUES (2, 2, 2)
CREATE TABLE B (B1 INTEGER,B2 INTEGER, B3 INTEGER)
INSERT INTO B VALUES (1, 1, 1)
INSERT INTO B VALUES (2, 2, 2)
CREATE TABLE C (C1 VARCHAR, C2 INTEGER, C3 INTEGER)
INSERT INTO C VALUES ('t1', 1, 1)
INSERT INTO C VALUES ('t2', 2, 2)
SELECT A2 FROM A WHERE A1=1
CREATE INDEX A_index ON A (A1)
CREATE INDEX B_index ON B (B1)
CREATE TABLE integers(i integer, j integer, k BOOLEAN)
create table lists(id int, l int[])
INSERT INTO integers SELECT * FROM range(1, 20001, 1)
UPDATE integers SET i=i+1
SELECT SUM(i) FROM integers
SELECT SUM(i) FROM integers WHERE i > 0
SELECT * FROM integers WHERE i < 3
SELECT * FROM integers WHERE i <= 1
SELECT * FROM integers WHERE i >= 1
SELECT * FROM integers WHERE i = 1
SELECT * FROM integers WHERE i < 1
SELECT * FROM integers WHERE i <= 0
SELECT * FROM integers WHERE i > 1
SELECT * FROM integers WHERE i >= 2
SELECT * FROM integers WHERE i = 2
CREATE TABLE integers(i INTEGER, j INTEGER CHECK(i + j < 5), k INTEGER)
INSERT INTO integers VALUES (1, 2, 4)
UPDATE integers SET k=7
UPDATE integers SET i=i, j=3
UPDATE integers SET j=2
CREATE TABLE integers(i INTEGER NOT NULL, j INTEGER NOT NULL)
UPDATE integers SET j=3
CREATE TABLE integers(i INTEGER NOT NULL)
INSERT INTO integers VALUES (3)
UPDATE integers SET i=4
CREATE TABLE integers_with_null(i INTEGER)
INSERT INTO integers_with_null VALUES (3), (4), (5), (NULL)
INSERT INTO integers (i) SELECT * FROM integers_with_null WHERE i IS NOT NULL
SELECT * FROM integers ORDER BY i
UPDATE integers SET i=4 WHERE i>4
CREATE TABLE integers(i INTEGER UNIQUE, j INTEGER)
INSERT INTO integers VALUES (3, 4), (2, 5)
INSERT INTO integers VALUES (NULL, 6), (NULL, 7)
SELECT * FROM integers ORDER BY i, j
UPDATE integers SET i=77 WHERE i IS NULL AND j=6
CREATE TEMPORARY TABLE integers(i INTEGER, j VARCHAR)
INSERT INTO integers VALUES (3, '4'), (2, '4')
CREATE TEMPORARY TABLE integers(i INTEGER, j INTEGER)
CREATE UNIQUE INDEX uidx ON integers (i,j)
INSERT INTO integers VALUES (NULL, 6), (NULL, 6), (NULL, 7)
UPDATE integers SET i=77 WHERE i IS NULL AND j=7
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
INSERT INTO integers VALUES (1, 1), (2, 2)
INSERT INTO integers VALUES (3, 3), (4, 4)
INSERT INTO integers VALUES (5, 5), (6, 6)
INSERT INTO integers VALUES (100, 100)
CREATE UNIQUE INDEX "uidx" ON "integers" ("j")
INSERT INTO integers VALUES (3, '4'), (2, '5')
INSERT INTO integers VALUES (6,NULL), (7,NULL)
UPDATE integers SET j='7777777777777777777777777777' WHERE j IS NULL AND i=6
CREATE UNIQUE INDEX uidx ON integers (i)
CREATE TABLE integers(i INTEGER, j BOOLEAN, PRIMARY KEY(i, j))
INSERT INTO integers VALUES (1, false), (1, true), (2, false)
INSERT INTO integers VALUES (2, true)
SELECT * FROM integers ORDER BY 1, 2
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b))
INSERT INTO numbers VALUES (1,1,1,1,1),(1,2,1,1,1),(2,1,2,1,1),(2,2,2,2,2)
INSERT INTO numbers VALUES (1,5,1,1,4)
UPDATE numbers SET c=1 WHERE c=2
UPDATE numbers SET b=3 WHERE b=2
CREATE TABLE integers(i INTEGER PRIMARY KEY)
INSERT INTO integers VALUES (1), (2), (3)
UPDATE integers SET i=4 WHERE i=2
UPDATE integers SET i=5 WHERE i=3
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b,c,d,e))
INSERT INTO numbers VALUES (1,1,1,1,1),(1,2,1,1,1),(1,1,2,1,1),(2,2,2,2,2)
INSERT INTO numbers VALUES (1,1,1,1,4)
CREATE TABLE integers(i INTEGER, j VARCHAR, PRIMARY KEY(i, j))
INSERT INTO integers VALUES (3, 'hello'), (3, 'world')
INSERT INTO integers VALUES (6, 'bla')
CREATE TABLE tst(a varchar, b varchar,PRIMARY KEY(a,b))
INSERT INTO tst VALUES ('hell', 'hello'), ('hello','hell'), ('hel','hell'), ('hell','hel')
INSERT INTO tst VALUES ('hel', 'hello')
UPDATE tst SET b='hell' WHERE b='hel'
CREATE TABLE numbers(i varchar PRIMARY KEY, j INTEGER)
INSERT INTO numbers VALUES ('1', 4), ('2', 5)
INSERT INTO numbers VALUES ('6', 6)
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER)
INSERT INTO test VALUES (11, 1), (12, 2), (13, 3)
UPDATE test SET b=2 WHERE b=3
DELETE FROM test WHERE a=11
INSERT INTO test VALUES (11, 1)
UPDATE test SET a=4 WHERE b=1
UPDATE integers SET i=33
INSERT INTO integers VALUES (33)
CREATE TABLE test (a INTEGER, b VARCHAR, PRIMARY KEY(a, b))
INSERT INTO test VALUES (11, 'hello'), (12, 'world'), (13, 'blablabla')
UPDATE test SET b = 'pandas'
UPDATE test SET a = a + 3
UPDATE test SET a = a - 3
DELETE FROM test WHERE a = 12
INSERT INTO test VALUES (12, 'pandas')
INSERT INTO test VALUES (12, 'other pandas')
UPDATE test SET a = 4 WHERE a = 42
UPDATE test SET a = 4 WHERE a = 12
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER)
INSERT INTO integers VALUES (6, 6)
INSERT INTO integers VALUES (7, 8)
INSERT INTO integers VALUES (7, 33)
CREATE TABLE tbl(t ROW(t INTEGER) CHECK(t.t=42))
INSERT INTO tbl VALUES ({'t': 42})
CREATE TABLE tbl(t ROW(t INTEGER) CHECK(tbl.t.t=42))
CREATE TABLE integers(i INTEGER CHECK(i < 5))
CREATE TABLE integers(i INTEGER CHECK(i + j < 10), j INTEGER)
INSERT INTO integers VALUES (3, 3)
CREATE TABLE integers4(i INTEGER CHECK(integers4.i < 10), j INTEGER)
CREATE TABLE v0 ( v3 INTEGER, v2 INTEGER, v4 INTEGER, v1 INTEGER, CHECK ( ( ( NOT (v2 = v1) = v3 - 0 ) > (18 = 10) ) ), CHECK ( v3 = v4 ) )
INSERT INTO v0 (v4) VALUES (2), (1)
UPDATE v0 SET v4 = 44
SELECT COUNT(*), MIN(v4) FROM v0
CREATE TABLE v1 ( a INTEGER, b INTEGER, c INTEGER, CHECK (a = b), CHECK (b = c) )
INSERT INTO v1 VALUES (1, 1, 1)
SELECT a, b, c FROM v1
CREATE TABLE v2 ( x INTEGER, y INTEGER, z INTEGER, CHECK (x + y = z) )
INSERT INTO v2 VALUES (1, 2, 3)
INSERT INTO v2 VALUES (2, 3, 5)
SELECT x, y, z FROM v2 ORDER BY x
CREATE TABLE v3 ( id INTEGER, payload INTEGER, flag BOOLEAN, CHECK (flag IS NOT NULL) )
CREATE TABLE B (b1 INTEGER, b2 INTEGER, PRIMARY KEY(b1, b2))
CREATE TABLE A (a1 VARCHAR(1), a2 VARCHAR(1), a3 VARCHAR(1), a4 VARCHAR(1), a5 INTEGER, a6 INTEGER, PRIMARY KEY(a1, a2), UNIQUE(a3, a4), FOREIGN KEY (a5, a6) REFERENCES B(b1, b2))
INSERT INTO B (b1, b2) VALUES (1, 2), (2, 3), (6, 7)
CREATE TABLE C ( c1 INTEGER, c2 INTEGER, c3 VARCHAR(1), c4 VARCHAR(1), PRIMARY KEY (c1, c2), UNIQUE (c3, c4) )
CREATE TABLE D ( d1 INTEGER, d2 INTEGER, d3 VARCHAR(1), d4 VARCHAR(1), payload INTEGER, FOREIGN KEY (d1, d2) REFERENCES C (c1, c2), FOREIGN KEY (d3, d4) REFERENCES C (c3, c4) )
INSERT INTO C VALUES (0, 1, 'a', 'b'), (1, 0, 'a', 'c'), (2, 2, 'd', 'e')
INSERT INTO D VALUES (0, 1, 'a', 'b', 10), (1, 0, 'a', 'c', 20), (2, 2, 'd', 'e', 30)
CREATE SCHEMA freddy
SET schema = freddy
CREATE TABLE zippy( id INTEGER PRIMARY KEY )
CREATE TABLE george( zippy_id INTEGER, FOREIGN KEY (zippy_id) REFERENCES zippy(id) )
INSERT INTO zippy VALUES (1)
INSERT INTO george VALUES (1)
SELECT constraint_text FROM duckdb_constraints() WHERE table_name = 'george' AND constraint_type = 'FOREIGN KEY'
SET schema = main
CREATE TABLE zippy_main( id INTEGER PRIMARY KEY )
CREATE TABLE george_main( zippy_id INTEGER, FOREIGN KEY (zippy_id) REFERENCES zippy_main(id) )
SELECT constraint_text FROM duckdb_constraints() WHERE table_name = 'george_main' AND constraint_type = 'FOREIGN KEY'
CREATE TABLE tf_1 ( a integer, b integer, c integer, PRIMARY KEY (a), UNIQUE (b), UNIQUE (c) )
CREATE TABLE tf_2 ( d integer, e integer, f integer, FOREIGN KEY (d) REFERENCES tf_1 (a), FOREIGN KEY (e) REFERENCES tf_1 (b), FOREIGN KEY (f) REFERENCES tf_1 (c) )
INSERT INTO tf_1 VALUES (1, 1, 1)
INSERT INTO tf_1 VALUES (2, NULL, NULL)
INSERT INTO tf_2 VALUES (2, NULL, NULL)
DELETE FROM tf_2 WHERE d=2
DELETE FROM tf_1 WHERE a=2
INSERT INTO tf_1 VALUES (2, 3, NULL)
INSERT INTO tf_2 VALUES (1, 3, 1)
DELETE FROM tf_2 WHERE d=2 OR e=3
INSERT INTO tf_1 VALUES (2, NULL, 4)
INSERT INTO tf_2 VALUES (1, 1, 4)
create table x (c1 integer, primary key (c1))
create table y (c1 integer, foreign key (c1) references x (c1))
select count(*) from duckdb_constraints() where constraint_type = 'NOT NULL'
create table a (a int not null, constraint pk_a primary key (A))
create table b (a int references a (a))
drop table b
drop table a
create table a (i int primary key)
create table b (i int references A (i))
create table c (i int primary key, j int references C (i))
create table b (i int references a)
insert into a values (1)
create table a (i int)
create table a (i int, j int, primary key(i,j))
CREATE TABLE agency ( agency_id TEXT PRIMARY KEY, agency_name TEXT UNIQUE NOT NULL )
INSERT INTO agency VALUES (1, 1)
DROP TABLE routes
CREATE TABLE agency ( agency_id TEXT, agency_name TEXT NOT NULL )
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (agency_id) REFERENCES routes )
INSERT INTO routes VALUES (1, NULL)
INSERT INTO routes VALUES (2, 1)
CREATE TABLE agency ( agency_id TEXT, agency_id_2 TEXT, agency_name TEXT NOT NULL, PRIMARY KEY (agency_id, agency_id_2) )
CREATE TABLE t1(id INTEGER PRIMARY KEY)
CREATE TABLE t2(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id))
CREATE TABLE t3(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE NO ACTION)
CREATE TABLE t4(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE NO ACTION)
CREATE TABLE t5(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE RESTRICT)
CREATE TABLE t6(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE RESTRICT)
CREATE TABLE departments ( department_id INTEGER PRIMARY KEY, department_name VARCHAR(100) NOT NULL )
CREATE TABLE employees ( employee_id INTEGER PRIMARY KEY, employee_name VARCHAR(100) NOT NULL, department_id INT REFERENCES departments(department_id) )
ALTER TABLE employees RENAME TO old_employees
CREATE TABLE t1(i1 INTEGER UNIQUE)
INSERT INTO t1 VALUES (1), (2), (3), (4)
CREATE TABLE t2(i2 INTEGER PRIMARY KEY, FOREIGN KEY (i2) REFERENCES t1(i1))
INSERT INTO t2 VALUES (1), (2), (3)
CREATE TABLE t3(i3 INTEGER UNIQUE, FOREIGN KEY (i3) REFERENCES t2(i2))
INSERT INTO t3 VALUES (1), (2)
CREATE TABLE t4(i4 INTEGER, FOREIGN KEY (i4) REFERENCES t3(i3))
INSERT INTO t4 VALUES (1)
INSERT INTO t2 VALUES (4)
INSERT INTO t3 VALUES (3)
INSERT INTO t4 VALUES (2)
DELETE FROM t2 WHERE i2=4
INSERT INTO pk_integers VALUES (1), (2), (3)
CREATE TABLE fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
INSERT INTO fk_integers VALUES (1)
INSERT INTO fk_integers VALUES (1), (2)
CREATE TABLE fk_integers_another(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
create type custom_type as integer
create table parent ( id custom_type primary key )
create table child ( parent custom_type references parent )
drop table child
create table child ( parent integer references parent )
create type another_custom_type as integer
create table child ( parent another_custom_type references parent )
CREATE SCHEMA s1
CREATE SCHEMA s2
CREATE TABLE s1.pk_integers(i INTEGER PRIMARY KEY)
INSERT INTO s1.pk_integers VALUES (1), (2), (3)
SET storage_compatibility_version = 'v0.10.3'
USE fk_db
CREATE TABLE tbl_pk (i INT PRIMARY KEY, payload STRUCT(v VARCHAR, i INTEGER[]))
INSERT INTO tbl_pk VALUES (1, {'v': 'hello', 'i': [42]}), (2, {'v': 'world', 'i': [43]})
CREATE TABLE tbl_fk (i INT REFERENCES tbl_pk(i))
INSERT INTO tbl_fk VALUES (1), (1), (1)
USE other_fk_db
CHECKPOINT fk_db
DETACH fk_db
DROP TABLE pk_integers
INSERT INTO fk_integers VALUES (3)
DELETE FROM fk_integers WHERE j=3
CREATE TABLE pkt1( i1 INTEGER PRIMARY KEY CHECK(i1 < 3), j1 INTEGER UNIQUE )
CREATE TABLE pkt2( i2 INTEGER PRIMARY KEY, j2 INTEGER UNIQUE CHECK (j2 > 1000) )
CREATE TABLE fkt1( k1 INTEGER, l1 INTEGER, FOREIGN KEY(k1) REFERENCES pkt1(i1), FOREIGN KEY(l1) REFERENCES pkt2(i2) )
CREATE TABLE fkt2( k2 INTEGER, l2 INTEGER, FOREIGN KEY(k2) REFERENCES pkt1(j1), FOREIGN KEY(l2) REFERENCES pkt2(j2) )
INSERT INTO pkt1 VALUES (1, 11), (2, 12)
INSERT INTO pkt2 VALUES (101, 1001), (102, 1002)
INSERT INTO fkt1 VALUES (1, 102), (2, 101)
INSERT INTO fkt2 VALUES (12, 1001), (11, 1002)
DELETE FROM fkt1 WHERE k1=1
DELETE FROM fkt2 WHERE k2=11
SELECT * FROM pkt1
SELECT * FROM pkt2
CREATE TABLE vdata AS SELECT * FROM (VALUES ('v2',)) v(id)
CREATE VIEW v AS SELECT * FROM vdata
CREATE TABLE primary_table (id INT PRIMARY KEY)
CREATE TABLE secondary_table (primary_id INT, FOREIGN KEY (primary_id) REFERENCES primary_table(id))
INSERT INTO primary_table VALUES (42)
SELECT id FROM primary_table LIMIT 1
DELETE FROM primary_table WHERE id = 42
INSERT INTO pk_integers VALUES (1), (2)
INSERT INTO fk_integers VALUES (2)
CREATE TABLE employee( id INTEGER PRIMARY KEY, managerid INTEGER, name VARCHAR, FOREIGN KEY(managerid) REFERENCES employee(id))
INSERT INTO employee VALUES (1, NULL, 'Smith'), (2, NULL, 'Jhon'), (3, NULL, 'Romeo')
INSERT INTO employee VALUES (4, 2, 'Mark')
DELETE FROM employee WHERE id = 4
SELECT * FROM employee ORDER BY ALL
UPDATE employee SET name = 'Juliet' WHERE id = 3
UPDATE employee SET id = 4 WHERE id = 3
UPDATE employee SET managerid = 2 WHERE id = 4
UPDATE employee SET id = 5, managerid = 2 WHERE id = 4
SELECT * FROM employee WHERE managerid = 2
ALTER TABLE employee RENAME COLUMN name TO name_new
ALTER TABLE employee ALTER COLUMN name_new SET DATA TYPE TEXT
CREATE TEMPORARY TABLE album (artistid INTEGER, albumname TEXT, albumcover TEXT, UNIQUE (artistid, albumname))
INSERT INTO album VALUES (1, 'A', 'A_cover'), (2, 'B', 'B_cover'), (3, 'C', 'C_cover'), (4, 'D', 'D_cover')
CREATE TEMPORARY TABLE song (songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY (songartist, songalbum) REFERENCES album (artistid, albumname))
INSERT INTO song VALUES (11, 1, 'A', 'A_song'), (12, 2, 'B', 'B_song'), (13, 3, 'C', 'C_song')
DELETE FROM album WHERE albumname = 'D'
SELECT * FROM album ORDER BY ALL
UPDATE song SET songartist = 1, songalbum = 'A' WHERE songname = 'B_song'
SELECT * FROM song ORDER BY ALL
UPDATE album SET artistid=5, albumname='D' WHERE albumcover='B_cover'
SELECT * FROM album
UPDATE album SET albumcover='C_cover_new' WHERE artistid=3
UPDATE song SET songname='C_song_new' WHERE songartist=3
CREATE TABLE pkt(i INTEGER PRIMARY KEY)
CREATE TABLE fkt(j INTEGER, FOREIGN KEY (j) REFERENCES pkt(i))
INSERT INTO pkt VALUES (1)
INSERT INTO fkt VALUES (1)
INSERT INTO pkt VALUES (2)
INSERT INTO fkt VALUES (1), (2)
DELETE FROM fkt WHERE j = 1
DELETE FROM fkt WHERE j = 2
INSERT INTO pkt VALUES (3)
DELETE FROM pkt WHERE i = 3
DROP TABLE fkt
CREATE TABLE a(i INTEGER PRIMARY KEY)
USE db1
CREATE TABLE IF NOT EXISTS t1 ( cache_key VARCHAR PRIMARY KEY, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, )
CREATE TABLE IF NOT EXISTS t2 ( cache_key VARCHAR NOT NULL, dose DOUBLE NOT NULL, PRIMARY KEY (cache_key, dose), FOREIGN KEY (cache_key) REFERENCES t1 (cache_key) )
ATTACH ':memory:' AS other
USE other
DETACH db1
CREATE TABLE album(artistid INTEGER, albumname TEXT, albumcover TEXT, UNIQUE (artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, albumname))
DELETE FROM album WHERE albumname='D'
UPDATE song SET songartist=1, songalbum='A' WHERE songname='B_song'
SELECT * FROM song
ALTER TABLE song RENAME COLUMN songname TO songname_new
ALTER TABLE song ALTER COLUMN songname_new SET DATA TYPE VARCHAR
ALTER TABLE song DROP COLUMN songname_new
DROP TABLE song
ALTER TABLE album ALTER COLUMN albumcover SET DATA TYPE VARCHAR
ALTER TABLE album DROP COLUMN albumcover
DROP TABLE album
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0,10000) tbl(i)
set threads=1
INSERT INTO bigdata SELECT bigdata.* FROM bigdata, range(9)
PRAGMA verify_parallelism
pragma threads=4
PRAGMA threads=4
CREATE TABLE test2 as SELECT i as a, (i*2) as b, power(i,2) as c from range(0,10) tbl(i)
CREATE TABLE test3 as SELECT i as a, (i*3) as b, power(i,3) as c from range(0,10) tbl(i)
CREATE TABLE test4 as SELECT i as a, (i*4) as b, power(i,4) as c from range(0,10) tbl(i)
CREATE TABLE test5 as SELECT i as a, (i*5) as b, power(i,5) as c from range(0,10) tbl(i)
CREATE TABLE testpto as SELECT i as a, (i*10) as b, (i*100) as c from range(0,10000) tbl(i)
PRAGMA threads=1
CREATE TABLE integers AS SELECT range i FROM range(200000)
SET preserve_insertion_order=false
SET preserve_insertion_order=true
SET threads=2
CREATE TABLE integers2 AS SELECT range i, range % 4 j FROM range(200000)
CREATE TABLE bools AS SELECT i::bool i FROM range(2) i(i)
CREATE TABLE multi_column_test AS SELECT range i, range%10 j, case when range%2=0 then null else range end k FROM range(2500)
CREATE TABLE floating_point_test AS SELECT case when i%10=0 then null else i/10.0 end as fp FROM range(2500) t(i)
CREATE TABLE floating_point_nan AS SELECT case when i%10=0 then 'nan'::double when i%4=0 then null else i/10.0 end as fp FROM range(2500) t(i)
CREATE TABLE fp_nan_only AS SELECT 'nan'::float as float_val
CREATE TABLE string_test AS SELECT concat('thisisalongstring_', range) s FROM range(2500)
CREATE TABLE empty_test AS FROM range(2500) LIMIT 0
CREATE TABLE decimal_test AS SELECT 25.3::DECIMAL(4,1) AS dec_i16, 123456.789::DECIMAL(9,3) AS dec_i32, 123456789123.456::DECIMAL(18,3) AS dec_i64, 12345678912345678912345678912345678.912::DECIMAL(38,3) AS dec_i128 UNION ALL SELECT 1.1::DECIMAL(4,1), 2.123::DECIMAL(9,3), 3.456::DECIMAL(18,3), 4.567::DECIMAL(38,3)
CREATE TABLE struct_test AS SELECT case when i%10=0 then null else {'x': i, 'y': case when i%2=0 then 100 + i else null end} end struct_val FROM range(2500) t(i)
CREATE TABLE list_test AS SELECT [i] l1, case when i%10=0 then null else [case when i%2=0 then 100 + i else null end] end l2 FROM range(2500) t(i)
CREATE TABLE medium_list_test AS SELECT [i, i, i] l1, case when i%10=0 then null else [case when i%2=0 then 100 + i else null end, null, case when i%2=0 then null else 100 + i end] end l2 FROM range(2500) t(i)
CREATE TABLE nested_struct_test AS SELECT {'s1': {'x': i}, 's2': {'s3': {'y': i}, 'l': [i]}} n FROM range(2500) t(i)
set threads=4
create table encrypted.fuu as select 42
DETACH encrypted
FROM encrypted.fuu
CREATE OR REPLACE TABLE unencrypted.tbl AS SELECT * FROM range(10) t(i)
CREATE OR REPLACE TABLE v_0_10_2.tbl AS SELECT * FROM range(10) t(i)
SELECT SUM(i) FROM unencrypted.tbl
SELECT SUM(i) FROM v_0_10_2.tbl
DETACH unencrypted
DETACH v_0_10_2
COPY FROM DATABASE unencrypted TO encrypted
COPY FROM DATABASE v_0_10_2 TO encrypted_v2
DETACH encrypted_v2
SELECT SUM(i) FROM encrypted.tbl
SELECT SUM(i) FROM encrypted_v2.tbl
COPY FROM DATABASE encrypted TO unencrypted_new
SET force_mbedtls_unsafe = 'true'
CREATE OR REPLACE TABLE encrypted.tbl AS SELECT * FROM range(10) t(i)
SET force_mbedtls_unsafe = 'false'
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='encrypted_storage_version'
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='encrypted_v0'
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='encrypted_v1'
USE unencrypted
COPY FROM DATABASE unencrypted to encrypted_storage_version
COPY FROM DATABASE unencrypted to encrypted_v0
COPY FROM DATABASE unencrypted to encrypted_v1
DETACH encrypted_storage_version
DETACH encrypted_v0
DETACH encrypted_v1
SELECT l_suppkey FROM encrypted_v0.lineitem limit 10
SELECT l_suppkey FROM encrypted_v1.lineitem limit 10
COPY FROM DATABASE unencrypted to encrypted
SELECT l_suppkey FROM encrypted.lineitem limit 10
SELECT Delimiter, Quote, Escape FROM sniff_csv("data/19578.csv")
SELECT Delimiter, Quote, Escape FROM sniff_csv("data/19578.csv", strict_mode=false)
SELECT * FROM read_csv(['data/csv/unionbyname_21248_*.csv'], union_by_name = true, ignore_errors = true, all_varchar = true)
SELECT rsID, chr, pos, refb, altb FROM t1
SELECT rsID, chr, pos, refb, altb FROM t2
SELECT rsID, chr, pos, refb, altb FROM t3
CREATE TABLE test (a INTEGER, b INTEGER, c VARCHAR(10))
COPY (SELECT * FROM range(5) t(i)) TO (getvariable('copy_target')) WITH (HEADER)
COPY tbl FROM (getvariable('copy_target'))
PREPARE v1 AS COPY (SELECT 'hello world' str) TO $1
CREATE TABLE sales ( salesid INTEGER NOT NULL PRIMARY KEY, listid INTEGER NOT NULL, sellerid INTEGER NOT NULL, buyerid INTEGER NOT NULL, eventid INTEGER NOT NULL, dateid SMALLINT NOT NULL, qtysold SMALLINT NOT NULL, pricepaid DECIMAL (8,2), commission DECIMAL (8,2), saletime TIMESTAMP)
SELECT commas, periods FROM decimal_separators
SELECT typeof(commas), typeof(periods) FROM decimal_separators limit 1
SELECT commas, periods FROM decimal_separators2
SELECT typeof(commas), typeof(periods) FROM decimal_separators2 limit 1
SELECT commas, periods FROM decimal_separators3
SELECT commas, periods FROM decimal_separators4
SELECT typeof(commas), typeof(periods) FROM decimal_separators4 limit 1
CREATE TABLE ubn1(a BIGINT)
CREATE TABLE ubn2(a INTEGER, b INTEGER)
CREATE TABLE ubn3(a INTEGER, c INTEGER)
INSERT INTO ubn1 VALUES (1), (2), (9223372036854775807)
INSERT INTO ubn2 VALUES (3,4), (5, 6)
INSERT INTO ubn3 VALUES (100,101), (102, 103)
CREATE TYPE bla AS ENUM ('Y', 'N')
CREATE TABLE date_test(d date)
SET enable_external_access=false
CREATE TABLE integers AS SELECT * FROM range(10)
SELECT * FROM '~/integers.csv'
CREATE TABLE integers_load(i INTEGER)
COPY integers_load FROM '~/integers.csv'
SELECT * FROM integers_load
SELECT COUNT(*) FROM '~/homedir_integers*.csv'
CREATE TABLE integers AS FROM range(1000000) t(i)
CREATE TABLE T1 (name VARCHAR)
CREATE TABLE data (a VARCHAR, b VARCHAR, c VARCHAR)
FROM data
CREATE TABLE tbl(i INT, j VARCHAR, k DATE)
INSERT INTO tbl VALUES (42, 'hello world', NULL), (NULL, NULL, DATE '1992-01-01'), (100, 'thisisalongstring', DATE '2000-01-01')
SELECT COUNT(*) FROM v1
SELECT i, j, k FROM v1 ORDER BY i NULLS LAST
SELECT j FROM v1 ORDER BY j NULLS LAST
SELECT filename.replace('\', '/').split('/')[-1] FROM v1 LIMIT 1
CREATE TABLE s1.tbl AS SELECT * FROM range(10) t(i)
SELECT SUM(i) FROM s1.tbl
DETACH s1
SELECT * FROM customer
SELECT COUNT(c_login) FROM customer_quoted_nulls
SELECT CODGEO FROM leading_zeros LIMIT 1
SELECT typeof(CODGEO) FROM leading_zeros LIMIT 1
SELECT * FROM leading_zeros2
SELECT typeof(comune), typeof(codice_regione), typeof(codice_provincia) FROM leading_zeros2 LIMIT 1
select '09001'::int
select '00009001'::int
CREATE TABLE test (a VARCHAR, b INTEGER, c INTEGER)
CREATE TABLE test2 (a VARCHAR, b INTEGER, c INTEGER, d INTEGER)
create or replace table t as (from values ('a' || chr(0) || 'b') t(i))
SELECT phone FROM phone_numbers
SELECT typeof(phone) FROM phone_numbers LIMIT 1
SET VARIABLE csv_files=(SELECT LIST(file ORDER BY file) FROM globbed_files)
SELECT [parse_path(x)[-2:] for x in getvariable('csv_files')]
SELECT * FROM read_csv(getvariable('csv_files')) ORDER BY 1
WITH RECURSIVE t(i, j) AS ( SELECT 1, 0 UNION ALL ( SELECT i + 1, j + a FROM t, r WHERE i <= part ) ) SELECT * FROM t ORDER BY i
WITH RECURSIVE t(i) AS ( SELECT 1, NULL::DATE UNION ALL ( SELECT i+1, d FROM t, r WHERE i<5 ) ) SELECT * FROM t ORDER BY i
select count(*) from t
drop table t
CREATE TABLE t1 AS select '2024/12/12' as a, '01:02:03' as b, '2020/01/01 01:02:03' as c from range(0,10000)
insert into t1 values ('1','1','1')
CREATE TABLE date_tests (a DATE)
FROM date_tests
DROP TABLE date_tests
CREATE TABLE stg_device_metadata_with_dates ( device_id VARCHAR, device_name VARCHAR, device_type VARCHAR, manufacturer VARCHAR, model_number VARCHAR, firmware_version VARCHAR, installation_date DATE, location_id VARCHAR, location_name VARCHAR, facility_zone VARCHAR, is_active BOOLEAN, expected_lifetime_months INT, maintenance_interval_days INT, last_maintenance_date DATE )
FROM stg_device_metadata_with_dates
SELECT COUNT(*) FROM bgzf
SELECT COUNT(*) FROM concat
CREATE TABLE test (foo INTEGER, bar VARCHAR(10), baz VARCHAR(10), bam VARCHAR(10))
SELECT COUNT(bam) FROM test WHERE bam = '!'
CREATE TABLE blobs (b BYTEA)
SELECT b FROM blobs
DELETE FROM blobs
CREATE TABLE lineitem(a INT NOT NULL, b INT NOT NULL, c INT NOT NULL)
SELECT COUNT(*) FROM lineitem
SELECT a, b, c FROM lineitem ORDER BY a
DROP TABLE lineitem
SELECT COUNT(a), SUM(a) FROM test
SELECT * FROM test ORDER BY 1 LIMIT 3
CREATE TABLE test2 (a INTEGER, b INTEGER, c VARCHAR(10))
SELECT * FROM test2 ORDER BY 1 LIMIT 3
CREATE TABLE test_too_few_rows (a INTEGER, b INTEGER, c VARCHAR, d INTEGER)
CREATE TABLE test3 (a INTEGER, b INTEGER)
SELECT * FROM test3 ORDER BY 1 LIMIT 3
CREATE TABLE test4 (a INTEGER, b INTEGER, c VARCHAR(10))
SELECT * FROM test4 ORDER BY 1 LIMIT 3
CREATE TABLE empty_table (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE unterminated (a VARCHAR)
CREATE TABLE vsize (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE test (a INTEGER, b VARCHAR DEFAULT('hello'), c INTEGER DEFAULT(3+4))
SELECT COUNT(a), COUNT(b), COUNT(c), MIN(LENGTH(b)), MAX(LENGTH(b)), SUM(a), SUM(c) FROM test
SELECT l_partkey FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
SELECT COUNT(*) FROM (FROM lineitem EXCEPT FROM lineitem_rt)
CREATE TABLE test_null_option (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10), col_e VARCHAR)
SELECT * FROM test_null_option ORDER BY 1 LIMIT 3
DELETE FROM test_null_option
CREATE TABLE test_null_option_2 (col_a INTEGER, col_b INTEGER, col_c VARCHAR(10), col_d VARCHAR(10))
CREATE TABLE venue ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats INTEGER )
CREATE TABLE venue_2 ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats VARCHAR )
SELECT COUNT(*) from venue_2
DROP TABLE venue_2
create table t (a json)
FROM t
CREATE TABLE no_newline (a INTEGER, b INTEGER, c VARCHAR(10))
SET TimeZone='UTC'
SET Calendar = 'gregorian'
SET TimeZone = 'America/Los_Angeles'
SELECT cast(d as string) FROM date_test
CREATE TABLE dates (d DATE)
SELECT * FROM dates
SELECT * FROM dates ORDER BY d
CREATE TABLE new_dates (d DATE)
SELECT * FROM new_dates ORDER BY 1
CREATE TABLE timestamps(t TIMESTAMP)
SELECT * FROM timestamps
CREATE TABLE new_timestamps (t TIMESTAMP)
SELECT * FROM new_timestamps ORDER BY 1
DELETE FROM new_timestamps
CREATE TABLE no_quote(a VARCHAR, b VARCHAR)
SELECT * FROM no_quote
CREATE TYPE mood AS ENUM ('happy', 'sad', 'angry')
select count(*) from T
CREATE TABLE long_escaped_value (a INTEGER, b INTEGER, c VARCHAR)
SELECT * FROM long_escaped_value
CREATE TABLE long_escaped_value_unicode (a INTEGER, b INTEGER, c VARCHAR)
SELECT * FROM long_escaped_value_unicode
create table integers(i int)
insert into integers values (42)
drop table integers
select * from integers
create table tbl(a VARCHAR NOT NULL)
insert into tbl values ('')
abort
create table tbl_2(a VARCHAR NOT NULL, b VARCHAR NOT NULL, c VARCHAR NOT NULL, d VARCHAR)
insert into tbl_2 values ('','','','')
select * from tbl_2
CREATE TABLE test (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
SELECT * FROM test ORDER BY 1
CREATE TABLE test2 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
CREATE TABLE test3 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
call dbgen(sf=0.1)
create table t1 as select 1 as a,1 as b from range(3)
create table t2 (b integer, a integer)
insert into t2 select NULL as b,NULL as a from range(30000)
insert into t2 values (3,4)
CREATE TABLE T AS SELECT 'bar,baz', UNION ALL SELECT ',baz' from range (0,100000)
SELECT * FROM greek_utf8 ORDER BY 1
DELETE FROM greek_utf8
SELECT * FROM greek_utf8
DESCRIBE T
SELECT * FROM integers AS too_little_columns
SELECT * FROM integers AS too_many_columns
CREATE TABLE nullable_type (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10))
SELECT * FROM nullable_type
SELECT * FROM integers limit 1
select count(*) from integers
CREATE TABLE movie_info (id integer NOT NULL PRIMARY KEY, movie_id integer NOT NULL, info_type_id integer NOT NULL, info text NOT NULL, note text)
SELECT * FROM movie_info
CREATE TABLE trigger_loop (a VARCHAR)
INSERT INTO trigger_loop VALUES ('"')
CREATE TABLE users ( id INTEGER NOT NULL, /*primary key*/ name VARCHAR(10) NOT NULL, email VARCHAR )
select * from users order by all
DROP TABLE users
select * from users
CREATE TABLE proj ( id INTEGER NOT NULL, /*primary key*/ )
select * from proj
DROP table proj
CREATE TABLE proj ( name VARCHAR(10) NOT NULL, id INTEGER NOT NULL, /*primary key*/ )
CREATE TABLE proj ( email VARCHAR, id INTEGER NOT NULL, /*primary key*/ )
CREATE TABLE proj ( email VARCHAR, id VARCHAR NOT NULL, /*primary key*/ )
CREATE TABLE proj ( email VARCHAR, id integer NOT NULL, /*primary key*/ )
CREATE TABLE ppl ( name VARCHAR )
select objectid, name from test ORDER BY objectid limit 10
SELECT l_partkey, l_comment FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
DELETE FROM lineitem
SELECT * FROM lineitem
CALL dbgen(sf=10)
set memory_limit='32gb'
CREATE TABLE test (a INTEGER, b VARCHAR, c INTEGER)
SELECT LENGTH(b) FROM test ORDER BY a
SELECT SUM(a), SUM(c) FROM test
SHOW t
SELECT county_id, county_desc, vtd_desc, name_prefx_cd FROM ncvoters
DELETE FROM ncvoters
SELECT * FROM ncvoters
CREATE TABLE nfcstrings (s STRING)
SELECT COUNT(*) FROM nfcstrings WHERE s COLLATE NFC = 'ü'
CREATE TABLE nfcstrings (source STRING, nfc STRING, nfd STRING)
SELECT COUNT(*) FROM nfcstrings
SELECT COUNT(*) FROM nfcstrings WHERE source COLLATE NFC=nfc
SELECT COUNT(*) FROM nfcstrings WHERE nfc COLLATE NFC=nfd
DROP TABLE nfcstrings
drop table if exists reject_errors
select * exclude(scan_id ) from reject_errors order by all limit 5
from np
select a from np
select b,d from np
set threads =1
create view T_2 as SELECT * EXCLUDE (SETTLEMENTDATE, XX, filename, I), CAST(SETTLEMENTDATE AS TIMESTAMP) AS SETTLEMENTDATE, split(filename, '/')[8] AS file, isoyear(CAST(SETTLEMENTDATE AS TIMESTAMP)) AS "YEAR" FROM T
select count(*) from T_2
select * from v limit 10
select count(*) from v where a is null
select count(*) from v where b is null
select count(*) from v where c is null
select count(*) from v where d is null
SELECT year, uniquecarrier, origin, origincityname, div5longestgtime FROM ontime
DELETE FROM ontime
CREATE TABLE test AS VALUES ('a', 'foo', 1), ('a', 'foo', 2), ('a', 'bar', 1), ('b', 'bar', 1)
CREATE TABLE T as select '1, "Oogie Boogie"' from range (100000)
insert into T values ('2, """sir"" Oogie Boogie"')
CREATE TABLE T_2 as select '1, "Oogie Boogie"' from range (5000)
insert into T_2 values ('2, "\"sir\" Oogie Boogie"')
CREATE TABLE test (a VARCHAR, b INTEGER)
SELECT SUM(b) FROM test
SELECT string_split_regex(a, '[\r\n]+') FROM test ORDER BY a
SELECT * FROM dates ORDER BY 1
SELECT l_partkey, RTRIM(l_comment) FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
SELECT * FROM people
SELECT * FROM people2
select * from T limit 1
CREATE TABLE T (name varchar, money double, city varchar)
FROM T
CREATE TABLE test (column0 timestamptz)
FROM test
SELECT * FROM web_page ORDER BY wp_web_page_sk LIMIT 3
DELETE FROM web_page
SELECT * FROM web_page
SELECT SUM(a), MIN(LENGTH(b)), MAX(LENGTH(b)), SUM(LENGTH(b)), SUM(c) FROM test
CREATE TABLE tbl(id int, ts timestamp)
SELECT TRY_CAST('2022/01/27 11:04:57 PM' AS TIMESTAMPTZ)
CREATE TABLE tbl_tz(id int, ts timestamptz)
SELECT * FROM tbl_tz
CREATE TABLE people(id INTEGER, name VARCHAR)
INSERT INTO people VALUES (1, 'Mark'), (2, 'Hannes')
create table t (a integer)
insert into t values (1),(2),(NULL)
CREATE TABLE test_zst(a INTEGER, b INTEGER, c INTEGER, d VARCHAR, e VARCHAR)
CREATE TABLE abac_tbl (a VARCHAR, b VARCHAR, c VARCHAR)
SELECT * FROM abac_tbl
DELETE FROM abac_tbl
DROP TABLE abac_tbl
CREATE TABLE abac_tbl (a VARCHAR)
CREATE TABLE abac_tbl (a VARCHAR, b VARCHAR)
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
PRAGMA enable_profiling
PRAGMA profiling_mode = detailed
SELECT min (i + i) FROM integers
CREATE TABLE exprtest (a INTEGER, b INTEGER)
INSERT INTO exprtest VALUES (42, 10), (43, 100), (NULL, 1), (45, -1)
SELECT min (a + a ) FROM exprtest
SELECT a FROM exprtest WHERE a BETWEEN 43 AND 44
SELECT CASE a WHEN 42 THEN 100 WHEN 43 THEN 200 ELSE 300 END FROM exprtest
SET threads=4
CREATE TABLE test (a INTEGER, b VARCHAR(10))
INSERT INTO test VALUES (1, 'hello'), (2, 'world '), (3, ' xx')
copy (select 42) to 'con:'
SELECT * EXCLUDE (scan_id) FROM reject_errors order by all
DROP TABLE reject_errors
DROP TABLE reject_scans
SELECT COUNT(*) FROM reject_errors
SELECT COUNT(*) FROM csv_rejects_table
DROP TABLE csv_rejects_table
SELECT * EXCLUDE (scan_id) FROM reject_errors
SELECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_scans ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_errors ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_errors ORDER BY column_name
SELECT * EXCLUDE (scan_id) FROM reject_scans order by all
drop table reject_scans
SELECT * EXCLUDE (scan_id) FROM rejects_errors_2 order by all
drop table reject_errors
SELECT * EXCLUDE (scan_id) FROM rejects_scan_2 order by all
SELECT * EXCLUDE (scan_id) FROM rejects_scan_3 order by all
SELECT * EXCLUDE (scan_id) FROM rejects_errors_3 order by all
create temporary table t (a integer)
SELECT regexp_replace(file_path, '\\', '/', 'g'), line, column_idx, column_name, error_type, csv_line,line_byte_position, byte_position FROM reject_scans inner join reject_errors on (reject_scans.scan_id = reject_errors.scan_id and reject_scans.file_id = reject_errors.file_id)
SELECT regexp_replace(file_path, '\\', '/', 'g'), line, column_idx, column_name, error_type, line_byte_position,byte_position FROM reject_scans inner join reject_errors on (reject_scans.scan_id = reject_errors.scan_id and reject_scans.file_id = reject_errors.file_id)
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY ALL
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY byte_position
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY byte_position, error_message
SELECT * from locations_header_trailing_comma
describe locations_header_trailing_comma
SELECT COUNT(*) FROM cranlogs
SELECT * FROM cranlogs LIMIT 5
(SELECT * FROM cranlogs EXCEPT SELECT * FROM cranlogs2) UNION ALL (SELECT * FROM cranlogs2 EXCEPT SELECT * FROM cranlogs)
CREATE TABLE ncvoters2 AS SELECT * FROM ncvoters LIMIT 0
(SELECT * FROM ncvoters EXCEPT SELECT * FROM ncvoters2) UNION ALL (SELECT * FROM ncvoters2 EXCEPT SELECT * FROM ncvoters)
SELECT COUNT(*) FROM greek_utf8
SELECT COUNT(*) FROM movie_info
(FROM movie_info EXCEPT FROM movie_info2) UNION ALL (FROM movie_info2 EXCEPT FROM movie_info)
CREATE TABLE lineitem2 AS SELECT * FROM lineitem LIMIT 0
(SELECT * FROM lineitem EXCEPT SELECT * FROM lineitem2) UNION ALL (SELECT * FROM lineitem2 EXCEPT SELECT * FROM lineitem)
CREATE TABLE ontime2 AS SELECT * FROM ontime LIMIT 0
(SELECT * FROM ontime EXCEPT SELECT * FROM ontime2) UNION ALL (SELECT * FROM ontime2 EXCEPT SELECT * FROM ontime)
SELECT COUNT(*) FROM web_page
SELECT * FROM web_page ORDER BY column00 LIMIT 3
(SELECT * FROM web_page EXCEPT SELECT * FROM web_page2) UNION ALL (SELECT * FROM web_page2 EXCEPT SELECT * FROM web_page)
SELECT * FROM test ORDER BY column0
SELECT a, b FROM test
describe v
describe select * from v
SELECT a, column1, c FROM test ORDER BY a
SELECT a, b, a_1 FROM test ORDER BY a
SELECT a, b, a_1, a_1_1 FROM test ORDER BY a
SELECT column0, column1, column2 FROM test ORDER BY column0
SELECT a, column01, column12 FROM test
SELECT a, a_8, a_9, column12 FROM test
SELECT a, a_8, a_9, column12, column11, column12_1 FROM test
SELECT column00, column01, column02, column03, column04, column05, column06, column07, column08, column09, column10, column11, column12 FROM test
SELECT number, text, date FROM test ORDER BY number
SELECT column0, column1 FROM test ORDER BY column0
SELECT id FROM test
CREATE TABLE my_varchars(a VARCHAR, b VARCHAR, c VARCHAR)
INSERT INTO my_varchars VALUES ('Hello', 'Beautiful', 'World')
FROM my_varchars
SELECT a, b, c FROM test ORDER BY a
SELECT A, B, C FROM test ORDER BY a
SELECT _select, _insert, _join FROM test ORDER BY _select
SELECT _0_a, _1_b, _9_c FROM test ORDER BY _0_a
SELECT allo, teost, _ FROM test ORDER BY allo
SELECT aax, hello_world, qty_m2 FROM test ORDER BY aax
SELECT typeof(TestInteger), typeof(TestDouble), typeof(TestDate), typeof(TestText) FROM test LIMIT 1
SELECT TestInteger, TestDouble, TestDate, TestText FROM test WHERE TestDouble is not NULL
CREATE TABLE test (TestInteger integer, TestDouble double, TestDate varchar, TestText varchar)
create table t ( a blob)
CREATE OR REPLACE TABLE timings(tool string, sf float, day string, batch_type string, q string, parameters string, time float)
create table t (a integer, b double, c varchar)
insert into t values (1,1.1,'bla')
SELECT linenr, mixed_string, mixed_double FROM test LIMIT 3
SELECT typeof(linenr), typeof(mixed_string), typeof(mixed_double) FROM test LIMIT 1
SELECT linenr, mixed_string, mixed_double FROM test WHERE linenr > 27000 LIMIT 3
SELECT count(*) FROM test
SELECT a, b, t, d, ts FROM test ORDER BY a
SELECT typeof(a), typeof(b), typeof(t), typeof(d), typeof(ts) FROM test LIMIT 1
SELECT a, b, t, tf, d, df FROM test ORDER BY a
SELECT typeof(a), typeof(b), typeof(t), typeof(tf), typeof(d), typeof(df) FROM test LIMIT 1
SELECT i FROM test ORDER BY i
SELECT typeof(i), typeof(b) FROM test LIMIT 1
select count(file) from glob('./data/csv/afl/20250226_csv_fuzz_error/*')
select count(file) from glob('./data/csv/afl/3977/*')
CREATE TABLE t1 AS select i, (i+1) as j from range(0,3000) tbl(i)
describe t
CREATE TABLE special_char(a INT, b STRING)
INSERT INTO special_char VALUES (0, E'\\'), (1, E'\t'), (2, E'\n'), (3, E'a\\a'), (4, E'b\tb'), (5, E'c\nc'), (6, E'\\d'), (7, E'\te'), (8, E'\nf'), (9, E'g\\'), (10, E'h\t'), (11, E'i\n'), (12, E'\\j'), (13, E'\tk'), (14, E'\nl'), (15, E'\\\\'), (16, E'\t\t'), (17, E'\n\n'), (18, E'\\\t\n')
CREATE TABLE human_eval_jsonl AS SELECT REPLACE(COLUMNS(*), ' ', E'\t') FROM read_ndjson_auto( 'https://raw.githubusercontent.com/openai/human-eval/refs/heads/master/data/HumanEval.jsonl.gz')
DELETE FROM human_eval_jsonl WHERE split_part(task_id, '/', 2)::int >= 10
CREATE TABLE human_eval_csv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
CREATE TABLE human_eval_tsv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
TRUNCATE human_eval_csv
TRUNCATE human_eval_tsv
INSERT INTO human_eval_csv SELECT replace(COLUMNS(*), E'\r\n', E'\n') FROM read_csv('data/csv/unquoted_escape/human_eval.csv', quote = '', escape = '\', sep = ',', header = false, strict_mode = false)
INSERT INTO human_eval_tsv SELECT replace(COLUMNS(*), E'\r\n', E'\n') FROM read_csv('data/csv/unquoted_escape/human_eval.tsv', quote = '', escape = '\', sep = '\t', header = false, strict_mode = false)
CREATE TABLE dates(d DATE)
select count(*) from glob('/rewoiarwiouw3rajkawrasdf790273489*.csv') limit 10
select count(*) from glob('~/rewoiarwiouw3rajkawrasdf790273489*.py') limit 10
SELECT COUNT(*) FROM glob('*/*.csv')
SELECT COUNT(*) FROM glob('*.csv')
SELECT COUNT(*) FROM glob('csv/glob/*/*.csv')
set file_search_path=''
CREATE TABLE sensor_data(ts TIMESTAMP, value INT)
INSERT INTO sensor_data VALUES (TIMESTAMP '2000-01-01 01:02:03', 42), (TIMESTAMP '2000-02-01 01:02:03', 100), (TIMESTAMP '2000-03-01 12:11:10', 1000)
DELETE FROM sensor_data
INSERT INTO sensor_data VALUES (TIMESTAMP '2000-01-01 02:02:03', 62), (TIMESTAMP '2000-03-01 13:11:10', 50)
CREATE TABLE weird_tbl(id INT DEFAULT nextval('seq'), key VARCHAR)
INSERT INTO weird_tbl (key) VALUES ('/'), ('\/\/'), ('==='), ('value with strings'), ('?:&'), ('🦆'), ('==='), ('===')
ALTER TABLE weird_tbl RENAME COLUMN key TO "=/ \\/"
CREATE TABLE tbl AS SELECT i//1000 AS partition, i FROM range(10000) t(i)
CREATE TABLE tbl2 AS SELECT (date '2000-01-01' + interval (i//2000) years)::DATE AS part1, i%2 AS part2, i FROM range(10000) t(i)
CREATE TABLE t AS SELECT 2000+i%10 AS year, 1+i%3 AS month, i%4 AS c, i%5 AS d FROM RANGE(0,20) tbl(i)
WITH RECURSIVE cte AS ( SELECT 0 AS count, 1999 AS selected_year UNION ALL SELECT COUNT(*) AS count, MAX(partitioned_tbl.year) FROM partitioned_tbl, (SELECT MAX(selected_year) AS next_year FROM cte) WHERE partitioned_tbl.year = (SELECT MAX(selected_year) + 1 FROM cte) HAVING COUNT(*)>0 ) SELECT SUM(count), MIN(selected_year), MAX(selected_year) FROM cte WHERE count>0
CREATE TABLE test as SELECT i%2 as part_col, (i+1)%5 as value_col, i as value2_col from range(0,10) tbl(i)
CREATE TABLE partitioned_tbl AS SELECT i%2 AS partition, i col1, i // 7 col2, (i%3)::VARCHAR col3 FROM range(10000) t(i)
DROP TABLE partitioned_tbl
SELECT partition, SUM(col1) FROM partitioned_tbl GROUP BY partition ORDER BY ALL
EXPLAIN SELECT partition, SUM(col1) FROM partitioned_tbl GROUP BY partition ORDER BY ALL
SELECT partition, COUNT(DISTINCT col2) FROM partitioned_tbl GROUP BY partition ORDER BY ALL
SELECT partition, SUM(col1) FROM partitioned_tbl GROUP BY GROUPING SETS ((), (partition)) ORDER BY ALL
SELECT partition, SUM(col1) FILTER (col2%7>2) FROM partitioned_tbl GROUP BY partition ORDER BY ALL
SELECT SUM(col1), partition FROM partitioned_tbl GROUP BY partition ORDER BY ALL
SELECT partition, SUM(col1) FROM partitioned_tbl WHERE col2 > 100 GROUP BY partition ORDER BY ALL
CREATE TABLE partitioned_tbl2 AS SELECT i%2 AS partition1, i%3 AS partition2, i col1, i + 1 col2 FROM range(10000) t(i)
DROP TABLE partitioned_tbl2
SELECT partition1, partition2, SUM(col1) FROM partitioned_tbl2 GROUP BY partition1, partition2 ORDER BY ALL
USE attached_parquet
SELECT * FROM file
SELECT * FROM attached_parquet
CREATE MACRO assert_bloom_filter_hit(file, col, val) AS TABLE SELECT COUNT(*) > 0 AND COUNT(*) < MAX(row_group_id+1) FROM parquet_bloom_probe(file, col, val) WHERE NOT bloom_filter_excludes
EXECUTE statement2
execute statement(42)
PRAGMA disable_optimizer
CREATE TABLE test AS SELECT 'thisisaverylongstringbutitrepeatsmanytimessoitshighlycompressible' || (range % 10) i FROM range(100000)
CREATE OR REPLACE TABLE test AS SELECT 'coolstring' || range i FROM range(100000)
SET parquet_metadata_cache = true
SELECT unnest(parquet_file_metadata, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/arrow/column_orders.parquet')
CREATE TABLE raw_data ( ts TIMESTAMP_S NOT NULL, hits INTEGER NOT NULL )
INSERT INTO raw_data SELECT *, (random() * 500)::INTEGER FROM RANGE(TIMESTAMP '2023-11-01', TIMESTAMP '2023-11-06', INTERVAL 1 MINUTE)
CREATE TABLE timeseries AS ( SELECT DATE_TRUNC('hour', ts) AS bucket, SUM(hits)::BIGINT AS total FROM raw_data GROUP BY bucket )
SELECT * FROM timeseries ORDER BY ALL LIMIT 5
CREATE TABLE integers AS SELECT * FROM range(6) tbl(i)
FROM 'data/parquet-testing/invalid_utf8_stats.parquet'
SELECT json_extract(TX_JSON[1], 'block_hash') FROM json_tbl
PRAGMA tpch(1)
PRAGMA tpch(6)
SELECT unnest(parquet_file_metadata, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/arrow/alltypes_dictionary.parquet')
SELECT unnest(parquet_metadata, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet')
SELECT unnest(parquet_schema, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet')
SELECT COUNT(*) > 0 FROM (SELECT unnest(parquet_metadata, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet'))
SELECT COUNT(*) > 0 FROM (SELECT unnest(parquet_schema, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet'))
SELECT * FROM parquet_full_metadata('data/parquet-testing/decimal/decimal_dc.parquet')
SELECT * FROM parquet_full_metadata('data/parquet-testing/decimal/int64_decimal.parquet')
SELECT * FROM parquet_full_metadata('data/parquet-testing/glob/*.parquet')
SELECT * FROM parquet_full_metadata(['data/parquet-testing/decimal/int64_decimal.parquet', 'data/parquet-testing/decimal/int64_decimal.parquet'])
SELECT name, type, duckdb_type FROM (SELECT unnest(parquet_schema, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet')) WHERE type IS NOT NULL
SELECT column_id, name FROM (SELECT unnest(parquet_schema, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/lineitem-top10000.gzip.parquet')) ORDER BY column_id
CREATE TABLE integers(i INT)
SET parquet_metadata_cache=true
create table some_bools (val boolean)
insert into some_bools values (TRUE)
select count(*) from some_bools where val = 1
select count(*) from some_bools where val = '1'::bool
CREATE VIEW v1 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x
CREATE VIEW v2 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x UNION ALL SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}])
SELECT * FROM v2
CREATE VIEW v3 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':2}]} AS x
SELECT * FROM v3
CREATE VIEW v4 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':[2]}]} AS x
SELECT * FROM v4
CREATE TABLE lists as SELECT i as id, [i] as list from range(0,10000) tbl(i)
CREATE TABLE test_5209 AS SELECT range FROM range(10000)
CREATE TABLE table1 ( name VARCHAR, )
INSERT INTO table1 VALUES ('Test value 1!')
INSERT INTO table1 VALUES ('Test value 2!')
CREATE TABLE table2 ( name VARCHAR, number INTEGER, )
INSERT INTO table2 VALUES ('Other test value', 1)
INSERT INTO table2 VALUES ('Other test value', 2)
set parquet_metadata_cache=true
SET binary_as_string=true
SET binary_as_string=false
PRAGMA binary_as_string=1
SET storage_compatibility_version='v1.1.0'
PRAGMA add_parquet_key('key128', '0123456789112345')
PRAGMA add_parquet_key('key192', '012345678911234501234567')
PRAGMA add_parquet_key('key256', '01234567891123450123456789112345')
CREATE OR REPLACE TABLE test (i INTEGER)
PRAGMA add_parquet_key('key256base64', 'MDEyMzQ1Njc4OTExMjM0NTAxMjM0NTY3ODkxMTIzNDU=')
CREATE TABLE tbl AS SELECT i, 'thisisalongstring'||(i%5000)::VARCHAR AS str FROM range(100000) t(i)
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str AND str >= 'this'
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str AND str >= 'thisisalongstring2000' AND str <= 'thisisalongstring4000'
CREATE TABLE test_csv AS SELECT 1 as id, 'test_csv_content' as filename
CREATE TABLE test AS SELECT 1 as id, 'test' as filename
CREATE TABLE test_copy (i INT, j VARCHAR, filename VARCHAR)
SELECT i, j, parse_path(filename)[-2:] FROM test_copy
CREATE TABLE test_table_large AS SELECT * FROM range(0,10000) tbl(i)
SELECT ORGUNITID FROM tbl LIMIT 10
SELECT COUNT(*) FROM tbl WHERE Namevalidfrom <= '2017-03-01' AND Namevalidto >= '2017-03-01' AND Parentnamevalidfrom <= '2017-03-01' AND Parentnamevalidto >= '2017-03-01' AND CustomerCode = 'CODE'
Create table t1 (a int, b int, c int)
create or replace table orders(m int,v int,j int)
insert into orders select i%12+1,i,j from range(360)t(i),range(1000)s(j)
create table test as select i%5 as a, i%2 as b from range(0,10) tbl(i)
create table test2 as select i%5 as a, i%2 as b, i as c from range(0,10) tbl(i)
create table test_null_write as select 1 as c, NULL::INT as a, NULL::INT as b
SET explain_output='optimized_only'
explain SELECT * FROM test ORDER BY j DESC LIMIT 2
SELECT * FROM test ORDER BY j DESC LIMIT 2
explain SELECT * FROM test ORDER BY j, i LIMIT 2
SELECT * FROM test ORDER BY j, i LIMIT 2
explain SELECT i FROM test ORDER BY i LIMIT 2
explain SELECT * FROM (SELECT i + random() AS i, j, k, l FROM test) ORDER BY i LIMIT 2
SELECT * FROM (SELECT -i i, -j j, -k k, -l l FROM test) ORDER BY -j DESC LIMIT 2
SELECT * FROM (SELECT 100 + i i, 1000 + j j, 10000 + k k, 100000 + l l FROM (SELECT -i i, -j j, -k k, -l l FROM test)) ORDER BY j DESC LIMIT 2
explain SELECT * FROM test LIMIT 2 OFFSET 2
SELECT * FROM test LIMIT 2 OFFSET 2
explain SELECT * FROM test USING SAMPLE 2 ROWS
SELECT column_id, name FROM parquet_schema('data/parquet-testing/lineitem-top10000.gzip.parquet') ORDER BY column_id
WITH per_file AS ( SELECT file_name, COUNT(*) AS rows_per_file FROM parquet_schema('data/parquet-testing/glob3/**/*.parquet') GROUP BY file_name ) SELECT SUM(rows_per_file) AS total_rows, MAX(rows_per_file) AS max_rows_per_filename, (SELECT COUNT(DISTINCT column_id) FROM parquet_schema('data/parquet-testing/glob3/**/*.parquet')) AS distinct_column_ids FROM per_file
PRAGMA explain_output = OPTIMIZED_ONLY
CREATE TABLE copy_test(a INT, b INT)
DROP TABLE copy_test
CREATE TABLE test_nested AS SELECT 1 as id, {'a': {'b': {'c': 123}}} as deep_nested, {'x': 1, 'y': 2} as simple_struct
CREATE TABLE test_lists AS SELECT [1, 2, 3] as simple_list, [{'x': 1}, {'x': 2}] as list_of_structs, [[1, 2], [3, 4]] as nested_list
CREATE TABLE test_maps AS SELECT MAP {'a': 1, 'b': 2} as simple_map, MAP {'nested': {'inner': 123}} as map_of_struct
CREATE TABLE test_nullable AS SELECT {'a': NULL, 'b': 2} as partial_null, NULL::STRUCT(x INT, y INT) as full_null
CREATE OR REPLACE TABLE ubn1(a BIGINT)
CREATE OR REPLACE TABLE ubn2(a INTEGER, b INTEGER)
CREATE OR REPLACE TABLE ubn3(a INTEGER, c INTEGER)
CREATE TABLE test AS SELECT 'test' AS user, '2025' AS year
PREPARE v1 AS SELECT * FROM parquet_scan($1) ORDER BY 1
WITH RECURSIVE t(it, accum) AS ( SELECT 1, 0 UNION ALL ( SELECT it + 1, accum + j FROM t, r WHERE it <= x ) ) SELECT * FROM t ORDER BY it, accum
SELECT COUNT(*) FROM userdata1
SELECT COUNT(registration_dttm), COUNT(id), COUNT(first_name), COUNT(last_name), COUNT(email), COUNT(gender), COUNT(ip_address), COUNT(cc), COUNT(country), COUNT(birthdate), COUNT(salary), COUNT(title), COUNT(comments) FROM userdata1
SELECT MIN(registration_dttm), MAX(registration_dttm) FROM userdata1
SELECT MIN(id), MAX(id) FROM userdata1
SELECT FIRST(id) OVER w, LAST(id) OVER w FROM userdata1 WINDOW w AS (ORDER BY id RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) LIMIT 1
SELECT MIN(first_name), MAX(first_name) FROM userdata1
SELECT FIRST(first_name) OVER w, LAST(first_name) OVER w FROM userdata1 WINDOW w AS (ORDER BY id RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) LIMIT 1
SELECT MIN(last_name), MAX(last_name) FROM userdata1
SELECT FIRST(last_name) OVER w, LAST(last_name) OVER w FROM userdata1 WINDOW w AS (ORDER BY id RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) LIMIT 1
SELECT MIN(email), MAX(email) FROM userdata1
SELECT FIRST(email) OVER w, LAST(email) OVER w FROM userdata1 WINDOW w AS (ORDER BY id RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) LIMIT 1
SELECT MIN(gender), MAX(gender) FROM userdata1
PRAGMA explain_output = PHYSICAL_ONLY
pragma disable_object_cache
create table t (ts TIMESTAMP_S)
insert into t select make_timestamp((1706961600 + (360 * i))::BIGINT * 1000000) from range(10000) range(i)
select * from t limit 3
SET timezone='UTC'
CREATE TABLE selected_values AS SELECT 2 x
PRAGMA add_parquet_key('arrow_key', '0123456789012345')
PRAGMA add_parquet_key('arrow_key_generated_files', '0123456789abcdef')
SELECT f, i FROM integer_file_first WHERE i='042'
SELECT f, i FROM bigint_file_first WHERE i='042' ORDER BY ALL
SELECT f, i FROM integer_file_first WHERE i>10 ORDER BY ALL
SELECT f, i FROM bigint_file_first WHERE i>'10' ORDER BY ALL
SELECT f, i FROM integer_file_first WHERE i IS NULL
SELECT f, i FROM string_file_first WHERE i='042'
SELECT f, i FROM string_file_first WHERE i>'10' ORDER BY ALL
SELECT struct_val.i FROM integer_file_first ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i='042'
SELECT struct_val.i FROM bigint_file_first WHERE struct_val.i='042' ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i>10 ORDER BY ALL
SELECT struct_val.i FROM bigint_file_first WHERE struct_val.i>'10' ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i IS NULL
CREATE TABLE all_types AS SELECT * EXCLUDE (bit, "union") REPLACE ( case when extract(month from interval) <> 0 then interval '1 month 1 day 12:13:34.123' else interval end AS interval ) FROM test_all_types()
SELECT * REPLACE ( hugeint::DOUBLE AS hugeint, uhugeint::DOUBLE AS uhugeint, time_tz::TIME::TIMETZ AS time_tz ) FROM all_types
CREATE TABLE bools(b BOOL)
INSERT INTO bools SELECT CASE WHEN i%2=0 THEN NULL ELSE i%7=0 OR i%3=0 END b FROM range(10000) tbl(i)
SELECT COUNT(*), COUNT(b), BOOL_AND(b), BOOL_OR(b), SUM(CASE WHEN b THEN 1 ELSE 0 END) true_count, SUM(CASE WHEN b THEN 0 ELSE 1 END) false_count FROM bools
CREATE TABLE integers AS FROM range(100) t(i)
INSERT INTO dates VALUES (DATE '1992-01-01'), (DATE '1900-01-01'), (NULL), (DATE '2020-09-27')
CREATE TABLE decimals( dec4 DECIMAL(4,1), dec9 DECIMAL(9,2), dec18 DECIMAL(18,3), dec38 DECIMAL(38,4) )
INSERT INTO decimals VALUES ( -999.9, -9999999.99, -999999999999999.999, -999999999999999999999999999999999.9999 ), ( NULL, NULL, NULL, NULL ), ( 42, 42, 42, 42 ), ( -42, -42, -42, -42 ), ( 0, 0, 0, 0 ), ( 999.9, 9999999.99, 999999999999999.999, 999999999999999999999999999999999.9999 )
SELECT * FROM decimals
DELETE FROM decimals WHERE dec4<-42 OR dec4>42
CREATE TYPE mood AS ENUM ('joy', 'ok', 'happy')
CREATE TABLE enums(m mood)
INSERT INTO enums VALUES ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('joy')
UPDATE enums SET m=NULL WHERE m='joy'
UPDATE enums SET m=NULL
CREATE TABLE t AS SELECT 'joy'::mood AS m FROM range(10) t(i)
set variable field_id_values={i:{__duckdb_field_id:42,key:43,value:{__duckdb_field_id:44,element:{__duckdb_field_id:45,j:46}}}}
SELECT * FROM '~/integers.parquet'
COPY integers_load FROM '~/integers.parquet'
SELECT COUNT(*) FROM '~/homedir_integers*.parquet'
CREATE TABLE hugeints(h HUGEINT)
INSERT INTO hugeints VALUES (-1180591620717411303424), (0), (NULL), (1180591620717411303424)
CREATE TABLE IF NOT EXISTS intervals (i interval)
INSERT INTO intervals VALUES (interval '1' day), (interval '00:00:01'), (NULL), (interval '0' month), (interval '1' month)
CREATE TABLE empty_lists(i INTEGER[])
INSERT INTO empty_lists SELECT [] FROM range(10) UNION ALL SELECT [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
CREATE TABLE empty_lists_varchar(i VARCHAR[])
INSERT INTO empty_lists_varchar SELECT [] FROM range(10) UNION ALL SELECT ['hello', 'world', 'this', 'is', 'a', 'varchar', 'list']
CREATE TABLE empty_list_nested(i INT[][])
INSERT INTO empty_list_nested SELECT [] FROM range(10) UNION ALL SELECT [[1, 2, 3], [4, 5], [6, 7, 8]]
set memory_limit='4gb'
CREATE TABLE values_TINYINT AS SELECT d::TINYINT d FROM (VALUES (-128), (42), (NULL), (127)) tbl (d)
CREATE TABLE values_SMALLINT AS SELECT d::SMALLINT d FROM (VALUES (-32768), (42), (NULL), (32767)) tbl (d)
CREATE TABLE values_INTEGER AS SELECT d::INTEGER d FROM (VALUES (-2147483648), (42), (NULL), (2147483647)) tbl (d)
CREATE TABLE values_BIGINT AS SELECT d::BIGINT d FROM (VALUES (-9223372036854775808), (42), (NULL), (9223372036854775807)) tbl (d)
CREATE TABLE strings(s VARCHAR)
INSERT INTO strings VALUES ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('surprise')
UPDATE strings SET s=NULL WHERE s='joy'
UPDATE strings SET s=NULL
DELETE FROM strings
INSERT INTO strings VALUES ('0'), ('1'), ('2'), ('3'), ('4'), ('5'), ('6'), ('7'), ('8'), ('9'), ('10'), ('11'), ('12'), ('13'), ('14'), ('15'), ('16'), ('17'), ('18'), ('19'), ('20'), ('21'), ('22'), ('23'), ('24'), ('25'), ('26'), ('27'), ('28'), ('29')
INSERT INTO strings VALUES ('0'), ('1'), ('2'), (NULL), ('4'), ('5'), ('6'), (NULL), ('8'), ('9'), ('10'), ('11'), ('12'), ('13'), ('14'), ('15'), ('16'), ('17'), ('18'), ('19'), ('20'), (NULL), ('22'), ('23'), ('24'), ('25'), (NULL), ('27'), ('28'), ('29')
INSERT INTO timestamps VALUES (TIMESTAMP '1992-01-01 12:03:27'), (TIMESTAMP '1900-01-01 03:08:47'), (NULL), (TIMESTAMP '2020-09-27 13:12:01')
CREATE OR REPLACE TABLE timestamps(d TIMESTAMP_NS)
INSERT INTO timestamps VALUES ('1992-01-01 12:03:27.123456789'), ('1900-01-01 03:08:47.987654321'), (NULL), ('2020-09-27 13:12:01')
CREATE TABLE hugeints(h UHUGEINT)
INSERT INTO hugeints VALUES (0), (1), (NULL), (1180591620717411303424)
CREATE TABLE values_UTINYINT AS SELECT d::UTINYINT d FROM (VALUES (0), (42), (NULL), (255)) tbl (d)
CREATE TABLE values_USMALLINT AS SELECT d::USMALLINT d FROM (VALUES (0), (42), (NULL), (65535)) tbl (d)
CREATE TABLE values_UINTEGER AS SELECT d::UINTEGER d FROM (VALUES (0), (42), (NULL), (4294967295)) tbl (d)
CREATE TABLE values_UBIGINT AS SELECT d::UBIGINT d FROM (VALUES (0), (42), (NULL), (18446744073709551615)) tbl (d)
CREATE TABLE IF NOT EXISTS uuid (u uuid)
CREATE TABLE uuid2 AS SELECT uuid '47183823-2574-4bfd-b411-99ed177d3e43' uuid_val union all select uuid '00112233-4455-6677-8899-aabbccddeeff'
CREATE TABLE t1(part_key INT, val INT)
INSERT INTO t1 SELECT i%2, i FROM range(10) t(i)
CREATE TABLE empty_tbl(i INT, j VARCHAR)
CREATE TABLE tbl AS FROM range(10000) t(i) UNION ALL SELECT 100000
CREATE TABLE empty(i INTEGER)
CREATE TABLE struct_of_lists AS SELECT * FROM (VALUES ({'a': [1, 2, 3], 'b': ['hello', 'world']}), ({'a': [4, NULL, 5], 'b': ['duckduck', 'goose']}), ({'a': NULL, 'b': ['longlonglonglonglonglong', NULL, NULL]}), (NULL), ({'a': [], 'b': []}), ({'a': [1, 2, 3], 'b': NULL}) ) tbl(i)
CREATE TABLE list_of_structs AS SELECT * FROM (VALUES ([{'a': 1, 'b': 100}, NULL, {'a': 2, 'b': 101}]), (NULL), ([]), ([{'a': NULL, 'b': 102}, {'a': 3, 'b': NULL}, NULL]) ) tbl(i)
CREATE TABLE list_of_struct_of_structs AS SELECT * FROM (VALUES ([{'a': {'x': 33}, 'b': {'y': 42, 'z': 99}}, NULL, {'a': {'x': NULL}, 'b': {'y': 43, 'z': 100}}]), (NULL), ([]), ([{'a': NULL, 'b': {'y': NULL, 'z': 101}}, {'a': {'x': 34}, 'b': {'y': 43, 'z': NULL}}]), ([{'a': NULL, 'b': NULL}]) ) tbl(i)
CREATE TABLE list_of_lists_simple AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5]]), ([[6, 7]]), ([[8, 9, 10], [11, 12]]) ) tbl(i)
CREATE TABLE list_of_lists AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5], [], [6, 7]]), ([[8, NULL, 10], NULL, []]), ([]), (NULL), ([[11, 12, 13, 14], [], NULL, [], [], [15], [NULL, NULL, NULL]]) ) tbl(i)
CREATE TABLE list_of_lists_of_lists_of_lists AS SELECT [LIST(i)] i FROM list_of_lists UNION ALL SELECT NULL UNION ALL SELECT [NULL] UNION ALL SELECT [[], NULL, [], []] UNION ALL SELECT [[[NULL, NULL, [NULL]], NULL, [[], [7, 8, 9], [NULL], NULL, []]], [], [NULL]]
CREATE TABLE list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([8, 9, 10, 11]) ) tbl(i)
CREATE TABLE null_empty_list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([NULL]), ([]), ([]), ([]), ([]), ([8, NULL, 10, 11]), (NULL) ) tbl(i)
CREATE TABLE int_maps(m MAP(INTEGER,INTEGER))
INSERT INTO int_maps VALUES (MAP([42, 84], [1, 2])), (MAP([101, 201, 301], [3, NULL, 5])), (MAP([55, 66, 77], [6, 7, NULL]))
CREATE TABLE string_map(m MAP(VARCHAR,VARCHAR))
INSERT INTO string_map VALUES (MAP(['key1', 'key2'], ['value1', 'value2'])), (MAP(['best band', 'best boyband', 'richest person'], ['Tenacious D', 'Backstreet Boys', 'Jon Lajoie'])), (MAP([], [])), (NULL), (MAP(['option'], [NULL]))
CREATE TABLE list_map(m MAP(INT[],INT[]))
INSERT INTO list_map VALUES (MAP([[1, 2, 3], [], [4, 5]], [[6, 7, 8], NULL, [NULL]])), (MAP([], [])), (MAP([[1]], [NULL])), (MAP([[10, 12, 14, 16, 18, 20], []], [[1], [2]]))
CREATE TABLE varchar(v VARCHAR)
INSERT INTO varchar VALUES (NULL), ('hello'), (NULL), ('world'), (NULL)
INSERT INTO varchar SELECT repeat('A', 100000) v
CREATE TABLE structs AS SELECT {'a': NULL, 'b': 'hello'} i UNION ALL SELECT NULL UNION ALL SELECT {'a': 84, 'b': 'world'}
CREATE TABLE struct AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': 33, 'b': 32}), ({'a': 42, 'b': 27}) ) tbl(i)
CREATE TABLE struct_nulls AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': NULL, 'b': 32}), (NULL), ({'a': 42, 'b': NULL}) ) tbl(i)
CREATE TABLE struct_nested AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': 27, 'y1': 44}}), ({'a': {'x': 9, 'x1': 26}, 'b': {'y': 1, 'y1': 999}}), ({'a': {'x': 17, 'x1': 23}, 'b': {'y': 3, 'y1': 9999}}) ) tbl(i)
CREATE TABLE struct_nested_null AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': NULL, 'y1': 44}}), ({'a': {'x': NULL, 'x1': 26}, 'b': {'y': 1, 'y1': NULL}}), ({'a': {'x': 17, 'x1': NULL}, 'b': {'y': 3, 'y1': 9999}}), (NULL), ({'a': NULL, 'b': NULL}) ) tbl(i)
CREATE TABLE single_struct AS SELECT * FROM (VALUES ({'a': 42}), ({'a': 33}), ({'a': 42}) ) tbl(i)
CREATE TABLE single_struct_null AS SELECT * FROM (VALUES ({'a': 42}), ({'a': NULL}), (NULL) ) tbl(i)
CREATE TABLE nested_single_struct AS SELECT * FROM (VALUES ({'a': {'b': 42}}), ({'a': {'b': NULL}}), ({'a': NULL}), (NULL) ) tbl(i)
CREATE TABLE vals(i INTEGER, v VARCHAR)
INSERT INTO vals VALUES (1, 'hello')
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(2,10000) t(i)
SELECT MIN(i), MAX(i), MIN(v), MAX(v) FROM vals
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(10001,100000) t(i)
PRAGMA temp_directory=''
PRAGMA memory_limit='2MB'
CREATE TABLE vals(i INTEGER)
INSERT INTO vals SELECT CASE WHEN i % 2 = 0 THEN NULL ELSE i END FROM range(200000) tbl(i)
SELECT MIN(i), MAX(i), COUNT(i), COUNT(*) FROM vals
INSERT INTO vals SELECT * FROM vals
CREATE TABLE vals(i TINYINT)
INSERT INTO vals SELECT (CASE WHEN i % 2 = 0 THEN NULL ELSE i % 100 END)::TINYINT i FROM range(200000) tbl(i)
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
INSERT INTO test VALUES (11, 22, 'hello'), (13, 22, 'world'), (12, 21, 'test'), (10, NULL, NULL)
INSERT INTO test VALUES (14, 10, 'con')
INSERT INTO test VALUES (15, 10, 'con2')
INSERT INTO test VALUES (14, 10, 'con2')
INSERT INTO test VALUES (15, NULL, NULL)
SELECT COUNT(*), COUNT(a), COUNT(b), SUM(a), SUM(b), SUM(LENGTH(c)) FROM test
SELECT * FROM test ORDER BY a, b, c
INSERT INTO test SELECT i, NULL, NULL FROM range(15, 10000) tbl(i)
INSERT INTO test VALUES (16, 24, 'blabla')
DELETE FROM test WHERE a=14
INSERT INTO test VALUES (14, 11, 'bla')
create table test as select range % 10 i, range % 30 j from range(100)
select stats(i), stats(j) from test limit 1
CREATE TABLE tbl (a STRUCT("id" VARCHAR), b STRUCT("id" VARCHAR))
INSERT INTO tbl SELECT {'id': LPAD(i::VARCHAR, 4, '0')}, {'id': 'abc'} FROM range(10000) t(i)
SELECT COUNT(*) FROM (SELECT * FROM tbl WHERE b.id='abc') t
INSERT INTO tbl SELECT {'id': LPAD((i + 10000)::VARCHAR, 4, '0')}, {'id': 'bcd'} FROM range(10000) t(i)
SELECT COUNT(*) FROM (SELECT * FROM tbl WHERE b.id='bcd') t
SELECT * FROM strings ORDER BY 1
CREATE TABLE table1 (column1 integer, column2 integer)
INSERT INTO table1(column1, column2) values(1, 1)
INSERT INTO table1(column1, column2) values(1, 2)
UPDATE table1 SET column2 = 3 FROM table1 s WHERE s.column1 = 1
SET wal_autocheckpoint='1GB'
CREATE TABLE tbl (n TEXT[])
INSERT INTO tbl (n) SELECT CASE WHEN i < 100 THEN ['a', 'b'] ELSE [] END l FROM range(1026) t(i)
FROM tbl
SELECT COUNT(*) FROM test
SELECT * FROM test ORDER BY 1, 2, 3
INSERT INTO vals SELECT (CASE WHEN i % 2 = 0 THEN NULL ELSE i % 100 END)::TINYINT i FROM range(10) tbl(i)
INSERT INTO test SELECT * FROM range(1000000)
UPDATE test SET i=i+1
SELECT MIN(i), MAX(i), COUNT(*) FROM test
UPDATE test SET i=i+1 WHERE i < 1000
UPDATE test SET i=i+1 WHERE i > 1000 AND i < 2000
UPDATE test SET i=i+1 WHERE i > 2000 AND i < 3000
UPDATE test SET i=i+1 WHERE i > 3000 AND i < 4000
CREATE TABLE null_byte AS SELECT concat('goo', chr(0), i) AS v FROM range(10000) tbl(i)
SELECT MIN(v), MAX(v) FROM null_byte
SELECT * FROM null_byte WHERE v=concat('goo', chr(0), 42)
CREATE INDEX i_index ON null_byte(v)
DROP TABLE null_byte
CREATE TABLE test (a INTEGER, b INTEGER)
INSERT INTO test VALUES (11, 22), (13, 22)
CREATE INDEX i_index ON test using art(a)
INSERT INTO test VALUES (11, 24)
SELECT a, b FROM test WHERE a=11 ORDER BY b
SELECT a, b FROM test WHERE a>11 ORDER BY b
DELETE FROM test WHERE a=11 AND b=24
DELETE FROM test WHERE a=11 AND b=22
UPDATE test SET b=22 WHERE a=11
INSERT INTO test VALUES (22, 23)
INSERT INTO test VALUES (12, 24)
SELECT * FROM test WHERE a=12
SET force_compression='uncompressed'
CREATE TABLE test (a VARCHAR, j BIGINT)
INSERT INTO test VALUES (repeat('a', 64), 1)
INSERT INTO test VALUES (11, 22), (13, 22), (12, 21), (NULL, NULL)
INSERT INTO test FROM test
SELECT SUM(a) + SUM(b) FROM test
INSERT INTO test VALUES ('a'), ('bb'), ('ccc'), ('dddd'), ('eeeee')
SELECT a, COUNT(*) FROM test GROUP BY a ORDER BY a
SELECT count(a) FROM test WHERE a='a'
UPDATE test SET a='aaa' WHERE a='a'
CREATE TABLE test (a INTEGER, b BIGINT)
CREATE TABLE a_interval AS SELECT interval (range) year i FROM range(1,1001)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval WHERE i = interval 1 year
CREATE TABLE a_bool AS SELECT range % 2 = 0 AS i FROM range(1000)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool WHERE NOT i
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_versions65'
set storage_compatibility_version='v0.10.2'
SELECT tags FROM duckdb_databases() WHERE database_name = 'regular_file'
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_version64'
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_versions66'
SELECT tags FROM duckdb_databases() WHERE database_name LIKE 'empty%' ORDER BY database_name
CREATE TABLE integers( g integer, i integer )
INSERT INTO integers values (0, 1), (0, 2), (1, 3), (1, NULL)
CREATE VIEW v1 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL
CREATE VIEW v2 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL DESC NULLS LAST
CREATE TABLE test (a INTEGER, b VARCHAR)
CREATE TABLE TBL (id INT NOT NULL, age INT NOT NULL, PRIMARY KEY ( id ))
INSERT INTO TBL VALUES (1, 1)
DELETE FROM TBL WHERE id = 1
SELECT * FROM TBL
SELECT * FROM TBL WHERE id=1
CREATE TABLE t2 (i INTEGER, uid VARCHAR)
INSERT INTO t2 SELECT i.range AS i, gen_random_uuid() AS uid FROM range(50000) AS i
CREATE UNIQUE INDEX iu ON t2(uid)
SELECT total_blocks < 6291456 / get_block_size('index_checkpoint') * 1.2 FROM pragma_database_size()
PRAGMA wal_autocheckpoint='10KB'
CREATE TABLE test(i INTEGER)
INSERT INTO test SELECT * FROM range(100000) tbl(i)
SET checkpoint_threshold='1TB'
CREATE TABLE db.my_tbl(i INTEGER PRIMARY KEY)
INSERT INTO db.my_tbl FROM range(200_000)
DETACH db
show tables
select current_user
create table anno as select 42
drop table if exists anno
INSERT INTO test VALUES (11), (12), (13), (14), (15), (NULL)
DELETE FROM test WHERE a=12
DELETE FROM test WHERE a=13
CREATE TABLE test2 (a INTEGER)
INSERT INTO test2 VALUES (13), (12), (11)
SELECT * FROM test2 ORDER BY a
INSERT INTO test VALUES (14, 23)
DROP TABLE test2
CREATE TABLE test (a INTEGER, b STRING)
INSERT INTO test VALUES (NULL, 'hello'), (13, 'abcdefgh'), (12, NULL)
SELECT a, b FROM test ORDER BY a
CREATE TABLE IF NOT EXISTS test (a INTEGER, b STRING)
CREATE TABLE test AS FROM range(250000) t(i)
DELETE FROM test WHERE i < 150000
TRUNCATE test
INSERT INTO test SELECT CASE WHEN i%2=0 THEN i ELSE NULL END, CASE WHEN i%2=0 THEN 'hello'||i::VARCHAR ELSE NULL END FROM range(10000) tbl(i)
SELECT COUNT(*), SUM(a), MIN(a), MAX(a), MIN(b), MAX(b), COUNT(a), COUNT(b) FROM test
INSERT INTO test VALUES (1,100),(2,200)
CREATE UNIQUE INDEX idx ON test (i)
CREATE TABLE IF NOT EXISTS unique_index_test AS SELECT i AS ordernumber, j AS quantity FROM test
CREATE UNIQUE INDEX unique_index_test_ordernumber_idx_unique ON unique_index_test (ordernumber)
SELECT a+1 FROM tbl
SELECT a+2 FROM tbl
CREATE TABLE enc.blobs (b BLOB)
DETACH enc
SELECT * FROM enc.blobs
CREATE TABLE attach_no_wal.integers(i INTEGER)
INSERT INTO attach_no_wal.integers FROM range(10000)
DETACH attach_no_wal
SELECT COUNT(*) FROM attach_no_wal.integers
CREATE TABLE enc.test (a INTEGER, b INTEGER)
INSERT INTO enc.test VALUES (11, 22), (13, 22), (12, 21)
ALTER TABLE enc.test ALTER b TYPE VARCHAR
SELECT * FROM enc.test ORDER BY 1
INSERT INTO enc.test VALUES (10, 'hello')
SET temp_file_encryption = false
SET memory_limit = '8MB'
CREATE TEMPORARY TABLE tbl AS FROM range(10_000_000)
USE enc
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(1000000) t(i)
CREATE TABLE bits (b BIT)
INSERT INTO bits VALUES('1'), ('010111'), ('111110010011'), (NULL), ('000000000000000000'), ('00100110010100100101001010010101010011110101000000000111100100110')
SELECT * FROM bits
CREATE TABLE blobs (b BLOB)
SELECT * FROM blobs
CREATE TABLE hugeints (h HUGEINT)
INSERT INTO hugeints VALUES (1043178439874412422424), (42), (NULL), (47289478944894789472897441242)
SELECT * FROM hugeints
SELECT * FROM hugeints WHERE h = 42
SELECT h FROM hugeints WHERE h < 10 ORDER BY 1
CREATE TABLE interval (t INTERVAL)
INSERT INTO interval VALUES (INTERVAL '1' DAY), (NULL), (INTERVAL '3 months 2 days 5 seconds')
SELECT * FROM interval
SELECT t FROM interval WHERE t = INTERVAL '1' DAY
SELECT t FROM interval WHERE t >= INTERVAL '1' DAY ORDER BY 1
SELECT t FROM interval WHERE t > INTERVAL '10' YEAR ORDER BY 1
CREATE TABLE timestamp (sec TIMESTAMP_S, milli TIMESTAMP_MS,micro TIMESTAMP_US, nano TIMESTAMP_NS )
INSERT INTO timestamp VALUES (NULL,NULL,NULL,NULL )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01','2008-01-01 00:00:01.594','2008-01-01 00:00:01.88926','2008-01-01 00:00:01.889268321' )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:51','2008-01-01 00:00:01.894','2008-01-01 00:00:01.99926','2008-01-01 00:00:01.999268321' )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:11','2008-01-01 00:00:01.794','2008-01-01 00:00:01.98926','2008-01-01 00:00:01.899268321' )
SELECT * FROM timestamp ORDER BY sec
SELECT * FROM timestamp WHERE micro=TIMESTAMP '2008-01-01 00:00:01.88926' ORDER BY micro
SELECT * FROM timestamp WHERE micro=TIMESTAMP '2020-01-01 00:00:01.88926' ORDER BY micro
CREATE TABLE uhugeints (h UHUGEINT)
INSERT INTO uhugeints VALUES (0), (42), (NULL), ('340282366920938463463374607431768211455'::UHUGEINT)
SELECT * FROM uhugeints
SELECT * FROM uhugeints WHERE h = 42
SELECT h FROM uhugeints WHERE h < 10 ORDER BY 1
CREATE TABLE unsigned (a utinyint, b usmallint, c uinteger, d ubigint)
INSERT INTO unsigned VALUES (1,1,1,1), (42,42,42,42), (NULL,NULL,NULL,NULL), (255,65535,4294967295,18446744073709551615)
SELECT * FROM unsigned
SELECT * FROM unsigned WHERE a = 42
SELECT a FROM unsigned WHERE a < 10 ORDER BY 1
SELECT * FROM unsigned WHERE b = 42
SELECT b FROM unsigned WHERE b < 10 ORDER BY 1
SELECT * FROM unsigned WHERE c = 42
SELECT c FROM unsigned WHERE c < 10 ORDER BY 1
SELECT * FROM unsigned WHERE d = 42
SELECT d FROM unsigned WHERE d < 10 ORDER BY 1
UPDATE unsigned SET a = 10, b = 9, c = 8, d = 7 WHERE a = 1
CREATE TABLE uuids (u uuid)
INSERT INTO uuids VALUES ('A0EEBC99-9C0B-4EF8-BB6D-6BB9BD380A11'), (NULL), ('47183823-2574-4bfd-b411-99ed177d3e43'), ('{10203040506070800102030405060708}')
SELECT * FROM uuids
SELECT * FROM uuids WHERE u = 'A0EEBC99-9C0B-4EF8-BB6D-6BB9BD380A11'
SELECT * FROM uuids WHERE u = 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'
SELECT u FROM uuids WHERE u > '10203040-5060-7080-0102-030405060708' ORDER BY 1
CREATE TABLE a(i ROW(a INT, b INT) DEFAULT ({'a': 7, 'b': 2}))
INSERT INTO a VALUES (DEFAULT)
CREATE TABLE a AS SELECT { 'r1': { 'a': 'hello', 'b': 3 }, 'r2': { 'a': 'world', 'b': 17, 'c': NULL } } c
SELECT c['r1']['a'] from a
UPDATE a SET c={ 'r1': { 'a': 'blabla', 'b': 3 }, 'r2': { 'a': 'world', 'b': 18, 'c': NULL } }
INSERT INTO a VALUES ( { 'r1': { 'a': NULL, 'b': 3 }, 'r2': { 'a': NULL, 'b': 17, 'c': NULL } })
INSERT INTO a VALUES ({ 'r1': NULL, 'r2': { 'a': NULL, 'b': 17, 'c': NULL } })
INSERT INTO a VALUES ({ 'r1': NULL, 'r2': NULL })
INSERT INTO a VALUES(NULL)
select column_path, stats from pragma_storage_info('a') where stats LIKE '%[Min: -2147483648, Max: -2147483648]%'
DROP TABLE a
CREATE TABLE structs(s STRUCT(id INT))
INSERT INTO structs SELECT {'id': CASE WHEN r%3=0 THEN NULL ELSE i END } FROM ( SELECT UNNEST(range(1200)) r, UNNEST(repeat([1], 1000)) i UNION ALL SELECT UNNEST(range(1000)) r, UNNEST(repeat([2], 1000)) i )
INSERT INTO structs SELECT CASE WHEN r%13=0 THEN NULL ELSE {'id': CASE WHEN r%7=0 THEN NULL ELSE i END } END FROM ( SELECT UNNEST(range(2000)) r, UNNEST(repeat([1], 2000)) i )
SELECT DISTINCT s.id FROM structs ORDER BY ALL
create table tbl (col STRUCT(a VARCHAR[]))
insert into tbl SELECT {'a': []} from range(122881)
CREATE TABLE a(b STRUCT(i INTEGER, j INTEGER))
INSERT INTO a VALUES ({'i': 1, 'j': 2}), (NULL), ({'i': NULL, 'j': 2}), (ROW(1, NULL))
SELECT COUNT(*) FROM a WHERE b IS NULL
DELETE FROM a WHERE (b).i=1
UPDATE a SET b={i: 7, j: 9} WHERE b IS NULL
CREATE TABLE a(b MAP(INTEGER,INTEGER))
INSERT INTO a VALUES (MAP([1], [2])), (MAP([1, 2, 3], [4, 5, 6]))
CREATE TABLE a(i INT[] DEFAULT ([1, 2, 3]))
CREATE TABLE test_table ( id INTEGER, emb FLOAT[], emb_arr FLOAT[3] )
INSERT INTO test_table (id) VALUES (42)
FROM test_table
DROP TABLE test_table
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT[])
INSERT INTO a VALUES (1, [1, 2, 3])
SELECT * FROM a WHERE id=1
INSERT INTO a VALUES (2, NULL)
INSERT INTO a VALUES (3, [NULL])
INSERT INTO a VALUES (4, [4, 5, NULL, 6])
SELECT * FROM a WHERE id=2
SELECT * FROM a WHERE id=3
SELECT * FROM a WHERE id=4
CREATE TABLE a(id INTEGER, c INT[])
CREATE INDEX a_index ON a(id)
INSERT INTO a VALUES (1, [4, 5, NULL])
CREATE TABLE a(b INTEGER[])
INSERT INTO a VALUES ([1, 2]), (NULL), ([3, 4, 5, 6]), ([NULL, 7])
DELETE FROM a WHERE b[1]=1
CREATE TABLE b(b INTEGER[][])
INSERT INTO b VALUES ([[1, 2], [3, 4]]), (NULL), ([NULL, [7, 8, NULL], [2, 3]]), ([[NULL, 6], NULL, [1, 2, NULL]])
SELECT * FROM b
DELETE FROM b WHERE b[1][1]=1
CREATE TABLE c(b VARCHAR[])
INSERT INTO c VALUES (['hello', 'world']), (NULL), (['fejwfoaejwfoijwafew', 'b', 'c']), ([NULL, 'XXXXXXXXXXXXXXXXXXXXXXXX'])
SELECT * FROM c
CREATE TABLE a(id INTEGER, b ROW(a INTEGER, b INTEGER)[])
INSERT INTO a VALUES (1, [{'a': 3, 'b': 7}, {'a': NULL, 'b': 7}, NULL]), (2, []), (3, NULL), (4, [NULL, {'a': 7, 'b': NULL}, {'a': 1, 'b': 1}])
SELECT * FROM a ORDER BY id
UPDATE a SET b=[] WHERE id=3
create table tbl (col VARIANT)
insert into tbl SELECT NULL from range(154840)
insert into tbl SELECT True from range(5000)
CREATE TABLE tbl (col VARIANT)
INSERT into tbl select '127.0.0.1'::INET
select * from tbl
select COLUMNS(*)::INET from tbl
CREATE TABLE tbl(i INT PRIMARY KEY, v VARIANT)
INSERT INTO tbl select i, {'a': i, 'b': i % 5} from range(100) t(i)
SELECT v FROM tbl WHERE i=42
USE db2
pragma verify_fetch_row
select v from tbl WHERE i < 10
SET variant_minimum_shredding_size = 0
CREATE TABLE variant_list( col STRUCT( f1 INTEGER, f2 VARIANT, f3 VARCHAR, f4 BOOL ) )
INSERT INTO variant_list SELECT { 'f1': i, 'f2': {'a': i::INTEGER, 'b': 'val' || i}, 'f3': 'test', 'f4': i % 2 == 0 } from range(1000) t(i)
select col.f2.a::INTEGER, col.f2.b::VARCHAR, col.f4 from variant_list limit 10
create table tbl ( col VARIANT )
insert into tbl select t::VARIANT var from test_all_types() t
from query($$select col."$$ || getvariable('col_name') || $$"::$$ || getvariable('col_type') || ' from tbl')
create or replace table intermediate as from query($$select col."$$ || getvariable('col_name') || $$" extracted from tbl$$)
from query('select extracted::' || getvariable('col_type') || ' from intermediate')
SET force_variant_shredding = getvariable('my_type')
create or replace table "tbl2" as select * from tbl
select * from "tbl2"
SET variant_minimum_shredding_size = -1
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
create table tbl (a VARIANT)
insert into tbl VALUES (42)
update tbl SET a = 21
set variant_minimum_shredding_size = 0
create or replace table test_structs( id int, s VARIANT )
insert into test_structs values (1, { 'name': { 'v': 'row 1', 'id': 1 }, 'nested_struct': { 'a': 42, 'b': true } }), (2, null), (3, { 'name': { 'v': 'row 3', 'id': 3 }, 'nested_struct': { 'a': 84, 'b': null } }), (4, { 'name': null, 'nested_struct': { 'A': null, 'b': false } })
SET force_variant_shredding = 'VARCHAR[]'
create table tbl(col VARIANT)
insert into tbl select ['hello', 'world'] from range(5) t(i)
select stats(col[1]) from tbl limit 1
insert into tbl select 42::INTEGER from range(5) t(i)
select stats(col) from tbl limit 1
select stats(col[1]) from tbl offset 5 limit 1
delete from tbl where not variant_typeof(col).starts_with('ARRAY')
insert into tbl select [42, 21, 1337] from range(5)
delete from tbl
insert into tbl select {'a': 42, 'b': false} from range(5)
select stats(col.a) from tbl limit 1
INSERT into tbl select {'a': i, 'b': i % 5} col, i from range(1000) t(i)
select b from tbl where col == {'b': 2, 'a': 2}
SET force_variant_shredding = 'STRUCT(a INT, b INT)'
create table shredded_values (col VARIANT)
insert into shredded_values values ({'a': 1, 'b': 100}), ({'a': 10, 'b': NULL}), ({'a': 100}), ({'a': NULL, 'b': NULL})
FROM shredded_values
SET force_variant_shredding = 'STRUCT(id INT, s STRUCT(a INT, b INT))'
create table nested_shredded_values (col VARIANT)
insert into nested_shredded_values values ({'id': 1, 's': {'a': 1, 'b': 100}}), ({'id': 2, 's': {'a': 10, 'b': NULL}}), ({'id': 3, 's': {'a': 100}}), ({'id': 4, 's': {'a': NULL, 'b': NULL}}), ({'id': 5, 's': NULL}), ({'id': 6})
FROM nested_shredded_values
SET force_variant_shredding = 'STRUCT(a INT, b INT)[]'
create table shredded_array (col VARIANT)
insert into shredded_array values ([{'a': 1, 'b': 100}]), ([{'a': 10, 'b': NULL}]), ([{'a': 100}]), ([{'a': NULL, 'b': NULL}]), ([]), ([NULL]), (NULL)
FROM shredded_array
CREATE TABLE messages AS SELECT ( CASE WHEN i % 100 = 0 THEN '{"action": "block", "extra_field": null}' ELSE '{"action": "build"}' END )::JSON AS msg_json FROM range(10000) t(i)
ALTER TABLE messages ADD COLUMN msg VARIANT
UPDATE messages SET msg = msg_json::VARIANT
select count(*) from messages where msg.action::VARCHAR = 'block'
CREATE TABLE t AS SELECT ('{"id":"' || md5(i::VARCHAR) || md5((i+9)::VARCHAR) || '","x":' || CASE WHEN i < 150000 THEN '"a string"' ELSE '[1,2,3]' END || '}')::JSON::VARIANT AS v FROM range(300000) tbl(i) ORDER BY i
SET variant_minimum_shredding_size=0
create table succeeds as SELECT '{"": 1, "x": {"y": "t"}}'::JSON::VARIANT
select * from succeeds
create table fails as SELECT '{"x": "hello", "": "world"}'::JSON::VARIANT AS j
select * from fails
create table bluesky (col VARIANT)
create table shredded_integer (col VARIANT)
insert into shredded_integer select (i % 100)::INTEGER from range(100) t(i)
SELECT COUNT(*) FROM pragma_storage_info('shredded_integer') WHERE column_path = '[0, 2, 2]'
SELECT SUM(TRY_CAST(col AS INT)), COUNT(*) FROM shredded_integer
insert into shredded_integer values ('hello world')
use variant
create table t3 (i INT)
create or replace table z(id integer)
insert into z from range(200_000)
set checkpoint_threshold='1TB'
set immediate_transaction_mode=true
begin
insert into z from range(200_000, 400_000)
select min(id), max(id) from z
CREATE OR REPLACE TABLE snap.snapshot(pk BIGINT PRIMARY KEY, val1 VARCHAR, val2 VARCHAR)
INSERT INTO snap.snapshot SELECT pk, uuid()::VARCHAR as val1, uuid()::VARCHAR as val2 FROM generate_series(1, 4096) t(pk)
CHECKPOINT snap
DETACH snap
CREATE OR REPLACE TABLE novelty_inserts AS SELECT pk, uuid()::VARCHAR as val1, uuid()::VARCHAR as val2 FROM novelty_deletes
DELETE FROM snap.snapshot WHERE pk IN (SELECT pk FROM novelty_deletes)
INSERT INTO snap.snapshot SELECT * FROM novelty_inserts
SET threads = 1
PRAGMA wal_autocheckpoint = '1TB'
PRAGMA debug_checkpoint_abort = 'before_header'
CREATE TABLE db.integers AS SELECT * FROM range(100) tbl(i)
INSERT INTO db.integers VALUES (42)
CREATE TABLE fail_detach.integers AS SELECT * FROM range(100) tbl(i)
ATTACH ':memory:' AS memory_compressed (COMPRESS)
CREATE TABLE memory_compressed.a(i INTEGER)
INSERT INTO memory_compressed.a FROM range(10000000)
PRAGMA force_checkpoint
FORCE CHECKPOINT memory_compressed
SELECT case when memory_usage_bytes < 1000000 then 'success' else error(concat('Expected less than ', 1000000, ' bytes, but got ', memory_usage_bytes)) end FROM duckdb_memory() WHERE tag='IN_MEMORY_TABLE'
attach ':memory:' as db2 (compress)
use db2
pragma force_compression='zstd'
create table tbl as select i // 5_000 as num, num::varchar || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
force checkpoint
select distinct compression = 'Uncompressed' from pragma_storage_info('tbl') where segment_type = 'VARCHAR'
CREATE TABLE read_duckdb_test.my_tbl AS SELECT 42 i
DETACH read_duckdb_test
SELECT COUNT(*) FROM duckdb_databases
CREATE TABLE read_duckdb_test2.other_tbl AS SELECT 100 i
DETACH read_duckdb_test2
CREATE TABLE read_duckdb_test.my_tbl2 AS SELECT 84 j
CREATE TABLE rd.tbl ( price INTEGER, amount_sold INTEGER, total_profit AS (price * amount_sold), non_generated INTEGER )
INSERT INTO rd.tbl VALUES (5,4, 100)
DETACH rd
CREATE TABLE read_duckdb_index.my_tbl(i INTEGER PRIMARY KEY)
INSERT INTO read_duckdb_index.my_tbl SELECT i + 1 FROM range(1000000) t(i)
DETACH read_duckdb_index
CREATE SCHEMA rd.s1
CREATE SCHEMA rd.s2
CREATE TABLE rd.s1.my_tbl AS SELECT 42 i
CREATE TABLE rd.s2.my_tbl AS SELECT 84 i
DETACH suggested
CREATE TABLE rd.my_tbl AS SELECT 42 i
CREATE TABLE rd.my_tbl AS SELECT 100 i, 84 col1
CREATE TABLE rd.my_tbl AS SELECT 200 i, 84 col2
SET force_compression='dictionary'
CREATE OR REPLACE TABLE 'everflow_daily' AS SELECT case when i%10=0 THEN uuid()::VARCHAR ELSE 'N/A' END sub4 FROM range(10000) t(i)
UPDATE everflow_daily SET sub4 = NULL WHERE sub4 = 'N/A'
select count(*) from everflow_daily where sub4 = 'N/A'
INSERT INTO test VALUES (11, 22), (NULL, 22), (12, 21)
UPDATE test SET b=b+1 WHERE a=11
UPDATE test SET b=NULL WHERE a=11
INSERT INTO test VALUES (11, 22), (13, 22), (12, 21)
INSERT INTO test SELECT r, r FROM range(2000) t(r)
INSERT INTO test SELECT r, r FROM range(2000,200000) t(r)
UPDATE test SET j=j+1
INSERT INTO test SELECT r, r FROM range(200000,400000) t(r)
select count(*) FROM test
CREATE TABLE pk_integers (i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers (j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
DELETE FROM fk_integers WHERE j = 3
SET memory_limit = '1925kB'
SET threads = 2
CREATE TABLE integers AS SELECT * FROM generate_series(0,599999) t(i)
DELETE FROM integers WHERE i%2=0
ALTER TABLE integers ADD COLUMN k INTEGER
SELECT COUNT(*), COUNT(i), COUNT(k) FROM integers
UPDATE integers SET k=i+1
SELECT COUNT(*), COUNT(i), COUNT(k), SUM(k) - SUM(i) FROM integers
DELETE FROM integers WHERE i%3=0
INSERT INTO test SELECT * FROM generate_series(0, 999)
DELETE FROM test WHERE i%2=0
SELECT COUNT(*), SUM(i), MIN(i), MAX(i) FROM test
INSERT INTO test SELECT * FROM generate_series(1000, 1099)
DELETE FROM test WHERE i%3=0
INSERT INTO test SELECT * FROM generate_series(1000, 1999)
SELECT COUNT(*) FROM test WHERE i%7=0
DELETE FROM test WHERE i%7=0
INSERT INTO test SELECT 1 FROM generate_series(0, 4000)
INSERT INTO test SELECT 2 FROM generate_series(0, 4000)
DELETE FROM test WHERE i=2
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22), (12, 21)
INSERT INTO test VALUES (11, 24), (12, 25)
CREATE TABLE integers AS FROM range(4) t(i)
CREATE TABLE integers2(i int)
INSERT INTO integers VALUES (42)
INSERT INTO integers VALUES (84)
SELECT COUNT(*), SUM(i) FROM integers
CREATE TABLE autocheckpoint_db.tbl(x INTEGER)
CREATE TABLE autocheckpoint_db.delete_tbl(x INTEGER)
INSERT INTO autocheckpoint_db.delete_tbl SELECT * FROM range(100)
CHECKPOINT autocheckpoint_db
SELECT wal_size FROM pragma_database_size() WHERE database_name = 'autocheckpoint_db'
SET wal_autocheckpoint_entries=100
SELECT wal_size != '0 bytes' FROM pragma_database_size() WHERE database_name = 'autocheckpoint_db'
INSERT INTO autocheckpoint_db.tbl VALUES (9999)
SELECT COUNT(*) FROM autocheckpoint_db.tbl
DETACH autocheckpoint_db
SET wal_autocheckpoint_entries=0
CREATE TABLE entry_count_db.tbl(x INTEGER)
CREATE TABLE test(a INTEGER CHECK (a<10), b INTEGER CHECK(CASE WHEN b < 10 THEN a < b ELSE a + b < 100 END))
INSERT INTO test VALUES (3, 7)
INSERT INTO test VALUES (9, 90)
INSERT INTO integers VALUES (1, 1), (2, 2), (3, 3)
CREATE UNIQUE INDEX i_index ON integers(i)
EXPLAIN ANALYZE SELECT i, j FROM integers WHERE i = 1
SELECT i, j FROM integers WHERE i = 1
CREATE UNIQUE INDEX i_index ON integers USING art((i + j))
SELECT i, j FROM integers WHERE i + j = 2
CREATE UNIQUE INDEX i_index ON integers USING art((j + i))
SELECT i, j FROM integers WHERE j + i = 2
CREATE UNIQUE INDEX i_index ON integers USING art((j + i), j, i)
SET checkpoint_threshold='999999GB'
create table bla as select 42
drop table bla
create table bla as select 84
SELECT * FROM bla
alter table bla rename to bla2
from bla2
create or replace table bla as select 84
create or replace table bla as select 42
from bla
CREATE SCHEMA test
CREATE TABLE test.test (a INTEGER, b INTEGER)
INSERT INTO test.test VALUES (11, 22), (13, 22)
DROP TABLE test.test
DROP SCHEMA test
CREATE TABLE tbl(a INTEGER, b VARCHAR, c DOUBLE, d TIMESTAMP)
CREATE INDEX idx_ab ON tbl(a, b)
CREATE INDEX idx_a ON tbl(a)
INSERT INTO tbl SELECT range, 'value_' || range, range * 1.5, '2023-01-01 10:00:00'::TIMESTAMP + INTERVAL (range) DAY FROM range(10)
DELETE FROM tbl WHERE a % 5 = 0
EXPLAIN ANALYZE SELECT a, b, c, d FROM tbl WHERE (a) = 1
SELECT a, b, c, d FROM tbl WHERE (a) = 1
EXPLAIN ANALYZE SELECT a, b, c, d FROM tbl WHERE (a) = 5
SELECT a, b, c, d FROM tbl WHERE (a) = 5
INSERT INTO tbl VALUES (5, 'value_5', 7.5, '2023-01-06 10:00:00')
EXPLAIN ANALYZE SELECT a, b, c, d FROM tbl WHERE (a) = 2
SELECT COUNT(*) FROM tbl where (a) = 2
CREATE TABLE tbl(a BIGINT, b INT AS (2*a), c VARCHAR, d DOUBLE, e as (d + 2), f TIMESTAMP)
CREATE INDEX idx_cd ON tbl(c,d)
CREATE INDEX idx_df ON tbl(d, f)
INSERT INTO tbl VALUES (1, 'foo', 10.5, '2023-01-01 10:00:00'), (2, 'bar', 20.5, '2023-02-01 11:00:00'), (3, 'baz', 30.5, '2023-03-01 12:00:00')
SELECT a, b, c, d, e, f FROM tbl ORDER BY a
DELETE FROM tbl WHERE a in (2)
INSERT INTO tbl VALUES (1, 'foo', 10.5, '2023-01-01 10:00:00')
SELECT b, e FROM tbl WHERE (c,d) = ('baz', 30.5)
CREATE TABLE tbl(a INTEGER)
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 10
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 0
SELECT * FROM tbl WHERE a = 0
SELECT * FROM tbl WHERE a = 10
SELECT * FROM tbl WHERE a = 70
SELECT * FROM tbl WHERE a = 140
SELECT * FROM tbl WHERE a = 30
SELECT * FROM tbl WHERE a = 90
INSERT INTO tbl VALUES (150)
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 150
SELECT * FROM tbl WHERE a = 150
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 25010
SELECT * FROM tbl WHERE a = 25010
SELECT * FROM tbl WHERE a = 24999
INSERT INTO tbl SELECT range FROM range(100)
INSERT INTO tbl SELECT range + 100 FROM range(50)
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 1
SELECT * FROM tbl WHERE a = 1
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 5
SELECT * FROM tbl WHERE a = 5
INSERT INTO tbl VALUES (5)
CREATE TABLE t (a INTEGER)
PREPARE p1 AS INSERT INTO t VALUES ($1)
EXECUTE p1(42)
EXECUTE p1(43)
DEALLOCATE p1
SELECT a FROM t
PREPARE p1 AS DELETE FROM t WHERE a=$1
PREPARE p1 AS UPDATE t SET a = $1
CREATE TABLE wal_promote.T AS (FROM range(10))
DETACH wal_promote
INSERT INTO wal_promote.T VALUES (42)
USE wal_replay
CREATE TABLE t AS SELECT range::INT AS id FROM range(10)
ALTER TABLE t ADD COLUMN d TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
CREATE TABLE original AS SELECT d FROM t
DETACH wal_replay
select COUNT(d) != 0 from t
select d from t
select d from original
ALTER TABLE t ADD COLUMN r DOUBLE DEFAULT RANDOM()
CREATE TABLE original AS SELECT id, r FROM t
select COUNT(r) != 0 from t
select r from t
select r from original
SELECT nextval('seq')
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval WHERE i=interval 1 year
CREATE TABLE a_bool AS SELECT range%2=0 i FROM range(1000)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool WHERE not i
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE person ( name text, current_mood mood )
INSERT INTO person VALUES ('Moe', 'happy')
select * from person
drop table person
drop TYPE mood
SELECT k FROM test ORDER BY k
INSERT INTO test(a, b) VALUES (1, 1)
ALTER TABLE test ALTER b TYPE VARCHAR
INSERT INTO test VALUES (10, 'hello')
DELETE FROM test WHERE b='hello'
CREATE TABLE test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
INSERT INTO test (b) VALUES (11)
SELECT * FROM test ORDER BY b
INSERT INTO test (b) VALUES (12)
INSERT INTO test (b) VALUES (13)
INSERT INTO test (b) VALUES (14)
INSERT INTO test (b) VALUES (15)
CREATE TABLE test (a INTEGER DEFAULT 1, b INTEGER)
INSERT INTO test (b) VALUES (12), (13)
INSERT INTO test (b) VALUES (14), (15)
CREATE TABLE test AS SELECT -i a, -i b FROM range(100000) tbl(i)
INSERT INTO test SELECT i+1 a, i+1 b FROM range(1000) tbl(i)
SELECT COUNT(*), SUM(a), SUM(b), MIN(a), MAX(a), MIN(b), MAX(b) FROM test WHERE a>0
SELECT COUNT(*) FROM test WHERE a>0
SELECT COUNT(*) FROM test WHERE a>0 AND a<>b
SELECT SUM(CASE WHEN b IS NULL THEN 1 ELSE 0 END) FROM test WHERE a>0 AND a%2=0
UPDATE test SET b=b+1 WHERE a>0 AND a%2=0
SELECT * FROM test WHERE a>0 ORDER BY 1,2
SELECT COUNT(*) FROM test WHERE a>0 AND a%2=0
SELECT COUNT(*) FROM test WHERE a IS NULL OR b IS NULL
UPDATE test SET b=NULL WHERE a>0 AND a%2=1
UPDATE test SET b=NULL WHERE a>0 AND a%2=0
SELECT COUNT(*) FROM test WHERE a>0 AND b IS NULL
SELECT COUNT(*), SUM(a), SUM(b), MIN(a), MAX(a), MIN(b), MAX(b), COUNT(b) FROM test WHERE a>0
ALTER TABLE test DROP COLUMN b
SELECT a FROM test ORDER BY a
ALTER TABLE test RENAME COLUMN a TO k
ALTER TABLE test RENAME TO new_name
SELECT a FROM new_name ORDER BY 1
CREATE VIEW vtest AS SELECT * FROM test
SELECT a FROM vtest ORDER BY a
ALTER VIEW vtest RENAME TO new_name
CREATE SEQUENCE seq_cycle INCREMENT 1 MAXVALUE 3 START 2 CYCLE
SELECT nextval('seq_cycle')
CREATE SEQUENCE seq2
DROP SEQUENCE seq2
SELECT nextval('seq'), nextval('seq')
DROP SEQUENCE seq
CREATE TABLE persistent (i INTEGER)
CREATE TEMPORARY TABLE temp.a (i INTEGER)
DELETE FROM temp.a
CREATE TEMPORARY SEQUENCE seq
CREATE TEMPORARY SEQUENCE seq2
CREATE TEMPORARY VIEW v1 AS SELECT 42
CREATE TEMPORARY VIEW v2 AS SELECT 42
DROP VIEW v2
INSERT INTO temp.a VALUES (43)
UPDATE temp.a SET i = 44
UPDATE a SET i = 45
ALTER TABLE a RENAME COLUMN i TO k
SELECT a, b FROM test WHERE a>0 OR a IS NULL ORDER BY a
CREATE TABLE test AS SELECT (-i)::VARCHAR a, (-i)::VARCHAR b FROM range(100000) tbl(i)
INSERT INTO test VALUES ('11', '22'), (NULL, '22'), ('12', '21')
UPDATE test SET b=(b::INT+1)::VARCHAR WHERE a='11'
SELECT a, b FROM test WHERE a::INTEGER>0 OR a IS NULL ORDER BY a
UPDATE test SET b=NULL WHERE a='11'
CREATE TABLE test (a VARCHAR, b VARCHAR)
CREATE TABLE timestamp (t TIMESTAMP)
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01'), (NULL), ('2007-01-01 00:00:01'), ('2008-02-01 00:00:01'), ('2008-01-02 00:00:01'), ('2008-01-01 10:00:00'), ('2008-01-01 00:10:00'), ('2008-01-01 00:00:10')
SELECT * FROM timestamp ORDER BY t
SELECT * FROM timestamp WHERE t=TIMESTAMP '2007-01-01 00:00:01' ORDER BY t
SELECT * FROM timestamp WHERE t=TIMESTAMP '2000-01-01 00:00:01' ORDER BY t
INSERT INTO uhugeints VALUES (1043178439874412422424), (42), (NULL), (47289478944894789472897441242)
set enable_view_dependencies=true
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v (b,c) AS SELECT * FROM test.t
PRAGMA table_info('test.v')
SELECT * FROM test.v
DROP TABLE test.t CASCADE
SELECT * FROM test.t
DROP TABLE test.t
drop table test.t cascade
CREATE VIEW test.v2 AS SELECT 42
DROP VIEW test.v2
CREATE VIEW test.v AS SELECT * FROM test.t
SELECT database_name, schema_name FROM duckdb_schemas WHERE NOT internal
FROM duckdb_columns()
SHOW TABLES
FROM duckdb_views()
PRAGMA memory_limit='1024KiB'
set max_temp_directory_size='0KiB'
select "size" from duckdb_temporary_files()
set max_temp_directory_size='256KiB'
set max_temp_directory_size='4MB'
set preserve_insertion_order=true
CREATE OR REPLACE TABLE t2 AS SELECT random() FROM range(200000)
SELECT CASE WHEN sum("size") > 1000000 THEN true ELSE CONCAT('Expected size 1000000, but got ', sum("size"))::UNION(msg VARCHAR, b BOOLEAN) END FROM duckdb_temporary_files()
select current_setting('max_temp_directory_size')
set max_temp_directory_size='2550KiB'
set max_temp_directory_size='15gb'
set temp_directory=''
PRAGMA memory_limit='3MB'
select current_setting('max_temp_directory_size') a where a == '0 bytes'
reset max_temp_directory_size
reset temp_directory
SELECT current_setting('temp_directory').split('/')[-1]
SET temp_directory=''
PRAGMA memory_limit='3MiB'
SELECT current_setting('max_temp_directory_size')
SET max_temp_directory_size='15GB'
SELECT current_setting('max_temp_directory_size') a WHERE a == '0 bytes'
RESET max_temp_directory_size
SELECT current_setting('max_temp_directory_size') a where a == '0 bytes'
PRAGMA max_temp_directory_size='-1'
select value from duckdb_settings() where name = 'temp_directory'
set temp_directory=null
SET memory_limit='2MB'
CREATE TEMPORARY TABLE t AS FROM range(1_000_000)
CREATE TABLE collate_test(s VARCHAR COLLATE NOACCENT)
INSERT INTO collate_test VALUES ('Mühleisen'), ('Hëllö')
SELECT * FROM collate_test WHERE s='Muhleisen'
SELECT * FROM collate_test WHERE s='mühleisen'
SELECT * FROM collate_test WHERE s='Hello'
CREATE MACRO plus1(a) AS a+1
SELECT plus1(2)
DROP MACRO plus1
CREATE MACRO plus2(a, b := 2) AS a + b
SELECT plus2(3)
SELECT plus2(4)
CREATE MACRO addition(a) AS a, (a,b) AS a + b
SELECT addition(2), addition(1, 2)
CREATE TABLE test_default (a BOOL DEFAULT nextval('seq') is not distinct from nextval('seq'), b INTEGER)
INSERT INTO test_default (b) VALUES (2), (4), (6)
select * from test_default
CREATE TABLE test(a INTEGER NOT NULL)
SELECT * FROM test WHERE b='hello'
SELECT * FROM persistent
CREATE TEMPORARY TABLE a (i INTEGER)
CREATE TABLE test_tbl (id INT, name string, height double)
INSERT INTO test_tbl values (1,'tom', 1.1), (2,'dick',1.2),(3,'harry', 1.2), (4,'mary',0.9), (5,'mungo', 0.8), (6,'midge', 0.5)
CREATE MACRO xt(a, _name) as TABLE SELECT * FROM test_tbl WHERE id<=a or name = _name
SELECT * FROM xt(10, '*') ORDER BY height limit 1
CREATE TEMPORARY MACRO my_seq(start , finish, stride:=3) as TABLE SELECT * FROM generate_series(start , finish , stride)
SELECT * FROM my_seq(0,6)
SELECT * FROM xt(100, 'joe')
DROP MACRO TABLE xt
CREATE MACRO my_range(rend) AS TABLE SELECT * FROM range(rend)
SELECT * from my_range(2)
CREATE TABLE tbl ( price INTEGER, gcol AS (price) )
SELECT gcol FROM tbl
CREATE TABLE tbl ( gcol_x AS (x), gcol_y AS (y), y INTEGER, x TEXT CHECK (y > 5) )
INSERT INTO tbl VALUES (6,'test')
CREATE TABLE tbl ( gcol_x AS (x), gcol_y AS (y), y INTEGER CHECK (y > 5), x TEXT )
CREATE TABLE tbl ( gcol_x AS (x), gcol_y AS (y), y INTEGER UNIQUE, x TEXT PRIMARY KEY )
CREATE TABLE tbl ( gcol_x AS (x), gcol_y AS (y), y INTEGER, x TEXT, PRIMARY KEY (y), UNIQUE (x) )
CREATE TABLE base ( price INTEGER PRIMARY KEY )
CREATE TABLE tbl ( gcol_nest AS (gcol), gcol AS (x), x INTEGER, FOREIGN KEY (x) REFERENCES base (price) )
INSERT INTO base VALUES (5)
DROP TABLE base
CREATE TABLE tbl ( gcol2 AS (gcol1), price INTEGER DEFAULT (5), gcol1 AS (price), )
INSERT INTO tbl VALUES (DEFAULT)
ALTER TABLE tbl DROP COLUMN gcol2
SELECT * FROM test WHERE a IS NULL
UPDATE test SET b=NULL WHERE a IS NULL
INSERT INTO test VALUES (12, NULL)
UPDATE test SET b='test123' WHERE a=12
INSERT INTO test SELECT a FROM range(0, 1000) tbl1(a), repeat(0, 100) tbl2(b)
UPDATE test SET a=2000 WHERE a=1
DELETE FROM test WHERE a=2 OR a=17
SELECT SUM(a), COUNT(a) FROM test
SELECT COUNT(a) FROM test WHERE a=0
SELECT COUNT(a) FROM test WHERE a=1
SELECT COUNT(a) FROM test WHERE a=2
SELECT COUNT(a) FROM test WHERE a=17
CREATE TABLE test(a INTEGER, b INTEGER)
INSERT INTO test VALUES (1, 3), (NULL, NULL)
UPDATE test SET b=4 WHERE a=1
UPDATE test SET a=4, b=4 WHERE a=1
UPDATE test SET b=5, a=6 WHERE a=4
DELETE FROM test WHERE a=2
UPDATE test SET b=7 WHERE a=3
CREATE TABLE t1(v VARCHAR DEFAULT CURRENT_SCHEMA())
INSERT INTO t1 VALUES (DEFAULT)
CREATE VIEW v1 AS SELECT current_schema()
attach ':memory:' as db1
use db1
CREATE TABLE db1.tbl (id INTEGER DEFAULT nextval('seq'), s VARCHAR)
ALTER TABLE db1.tbl ADD COLUMN m INTEGER DEFAULT nextval('seq')
CREATE TABLE IF NOT EXISTS a(id INT PRIMARY KEY)
INSERT INTO a(id) VALUES (1)
ALTER TABLE a ADD COLUMN c REAL
ALTER TABLE a ALTER COLUMN c SET DEFAULT 10
ALTER TABLE a RENAME c TO d
ALTER TABLE a RENAME TO b
ALTER TABLE b DROP d
INSERT INTO b(id) VALUES (2)
INSERT INTO test VALUES (repeat('a', 1000000))
SELECT LENGTH(SUBSTRING(a, 0, 1000000)) FROM test
UPDATE test SET a=concat(a, 'a')
select total_blocks from pragma_database_size()
set enable_external_file_cache=true
set prefetch_all_parquet_files=true
select current_setting('validate_external_file_cache')
set validate_external_file_cache='VALIDATE_REMOTE'
set validate_external_file_cache='NO_VALIDATION'
set validate_external_file_cache='VALIDATE_ALL'
CREATE TABLE test_rle (a INTEGER)
INSERT INTO test_rle SELECT 2147480000 FROM range(0, 10000) tbl(i)
INSERT INTO test_rle SELECT 2147480001 FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test_rle') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
CREATE TABLE test_constant (a INTEGER)
INSERT INTO test_constant SELECT 1 FROM range(0, 2000) tbl(i)
SELECT compression FROM pragma_storage_info('test_constant') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
CREATE TABLE test_dict (a VARCHAR)
INSERT INTO test_dict SELECT concat('foobar-', (i%2)::VARCHAR) FROM range(0, 2000) tbl(i)
SELECT compression FROM pragma_storage_info('test_dict') WHERE segment_type ILIKE 'VARCHAR' LIMIT 1
CREATE TABLE test_bp (a INTEGER)
INSERT INTO test_bp SELECT i FROM range(0, 2000) tbl(i)
SELECT compression FROM pragma_storage_info('test_bp') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
INSERT INTO test VALUES (11, 22), (11, 22), (12, 21), (NULL, NULL)
SELECT SUM(a), SUM(b) FROM test
CREATE OR REPLACE TABLE t( x VARCHAR USING COMPRESSION Dictionary )
set logging_level='info'
set variable dataset_size = 122880
PRAGMA force_compression='uncompressed'
CALL enable_logging(level='trace')
CREATE TABLE test_uncompressed AS SELECT case when i%25=0 then 1337 else null end FROM range(getvariable('dataset_size')) tbl(i)
set enable_logging=false
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_uncompressed') and message.contains('VALIDITY') and message.contains('UNCOMPRESSED')
PRAGMA force_compression='roaring'
set enable_logging=true
CREATE TABLE test_roaring AS select * from test_uncompressed
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_roaring') and message.contains('VALIDITY') and message.contains('ROARING')
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then 1337 else null end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then null else 1337 end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test (a BIGINT)
INSERT INTO test SELECT case when i%25=0 then 1337 else null end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' and compression != 'Roaring'
select count(*) from test WHERE a IS NOT NULL
select sum(a), min(a), max(a) from test
delete from test
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then 1337 else null end from intermediates
INSERT INTO test SELECT CASE WHEN i % 3 = 0 THEN 1337 ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS NOT NULL
SELECT SUM(a), MIN(a), MAX(a) FROM test
PRAGMA force_compression='BitPacking'
CREATE TABLE test_uncompressed AS SELECT case when i%25=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_uncompressed') and message.contains('BOOLEAN') and message.contains('BITPACKING')
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_roaring') and message.contains('BOOLEAN') and message.contains('ROARING')
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then false else true end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test (a BOOL)
INSERT INTO test SELECT case when i%25=0 then true else false end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' and compression != 'Roaring'
select count(*) from test WHERE a IS true
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then true else false end from intermediates
INSERT INTO test SELECT case when i%50=0 then false when i%25=0 then true else NULL end FROM range(0,10_000) tbl(i)
select count(*) from test WHERE a IS NULL
select count(*) from test WHERE a IS false
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 then false when i = 1500 or i = 2000 then true else null end from intermediates
INSERT INTO test SELECT CASE WHEN i % 3 = 0 THEN true ELSE false END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOL' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS true
SELECT COUNT(*) FROM test WHERE a IS false
INSERT INTO test SELECT CASE WHEN i % 6 = 0 THEN true WHEN i % 3 = 0 THEN false ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS null
CREATE TABLE test ( a BOOL )
pragma force_compression='roaring'
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' and compression != 'Constant'
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then false else true end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%3=0 then true else false end FROM range(0,10000) tbl(i)
INSERT INTO test VALUES (null), (true), (true), (true), (true), (true), (true), (true), (null), (null), (null), (null), (null), (null), (null), (null), (false), (false), (false), (false), (false), (false), (false), (false), (null), (true), (null), (false), (false), (false), (false), (false)
INSERT INTO test SELECT case when i%25=0 then false else true end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then false else true end from intermediates
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then true else false end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) INSERT INTO test SELECT case when (i >= 0 and i < 110) or (i >= 1500 and i < 1800) or (i >= 2000) then false else true end FROM intermediates
INSERT INTO test SELECT CASE WHEN i % 1000 < 100 THEN true WHEN i % 1000 < 200 THEN false ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOL' and compression != 'Roaring'
select count(*) from test WHERE a IS null
set checkpoint_threshold = '10mb'
INSERT INTO test SELECT case when i%25=0 then true else false end FROM range(0,1025) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' and compression != 'Uncompressed'
CREATE TABLE test ( a INT )
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then null else 1337 end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%3=0 then 1337 else null end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%25=0 then null else 1337 end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then null else 1337 end from intermediates
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then 1337 else null end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) INSERT INTO test SELECT case when (i >= 0 and i < 110) or (i >= 1500 and i < 1800) or (i >= 2000) then null else 1337 end FROM intermediates
INSERT INTO test SELECT case when i%25=0 then 1337 else null end FROM range(0,1025) tbl(i)
CREATE TABLE test AS SELECT concat('longprefix', i) FROM range(30000) t(i)
SELECT DISTINCT compression FROM pragma_storage_info('test') where segment_type = 'VARCHAR'
SET disabled_compression_methods='fsst'
SELECT BOOL_OR(compression ILIKE 'fsst%') FROM pragma_storage_info('test')
PRAGMA force_compression = 'fsst'
INSERT INTO test VALUES ('11', '22'), ('11', '22'), ('12', '21'), (NULL, NULL)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VARCHAR' LIMIT 1
pragma threads=1
CREATE TABLE TEST (col VARCHAR)
INSERT INTO TEST SELECT '' FROM range(0,100000) tbl(i)
pragma force_compression='fsst'
CREATE TABLE TEST2 as SELECT * FROM TEST
CREATE TABLE trigger5759 AS SELECT CASE WHEN RANDOM() > 0.95 THEN repeat('ab', 1500) ELSE 'c' END FROM range(0,1000)
PRAGMA force_compression = 'bitpacking'
SELECT compression FROM pragma_storage_info('test') where segment_type != 'VALIDITY' and compression != 'BitPacking'
CREATE TABLE test (c INT64)
INSERT INTO test SELECT i from range(0,130000) tbl(i)
SELECT avg(c) FROM test
PRAGMA force_compression='bitpacking'
create table aux as select range::INT x from range(-2_000_000_000, 2_000_000_000, 2_000_000)
create table tt as select (x + if (random() > 0.5, 1, -1)) x from aux
select compression from pragma_storage_info('tt') where segment_type != 'VALIDITY'
CREATE TABLE test (id VARCHAR, col INTEGER)
INSERT INTO test SELECT i::VARCHAR id, i b FROM range(10000) tbl(i)
INSERT INTO test SELECT i::VARCHAR id, 1337 FROM range(20000, 30000) tbl(i)
INSERT INTO test SELECT i::VARCHAR id, i b FROM range(30000,40000) tbl(i)
SELECT compression FROM pragma_storage_info('test') where segment_type = 'INTEGER' and compression != 'BitPacking'
SELECT SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE col=1337
SELECT MIN(id), MAX(id), SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE id='5000'
SELECT MIN(id), MAX(id), SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE id::INT64%1000=0
PRAGMA force_bitpacking_mode='constant'
CREATE TABLE test (id VARCHAR, a HUGEINT)
select a from test limit 5
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'HUGEINT'
SET force_compression = 'bitpacking'
SELECT current_setting('force_bitpacking_mode')
INSERT INTO test SELECT case when i%5=0 then null else 1337 end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%5=0 then null else i end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%5=0 then null else i//2 end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BIGINT' and compression != 'BitPacking'
CREATE TABLE test (id VARCHAR, a BIGINT)
INSERT INTO test SELECT i::VARCHAR, -i FROM range(0,10000) tbl(i)
INSERT INTO test SELECT i::VARCHAR, 13371337 FROM range(0,10000) tbl(i)
select a from test limit 5 offset 12000
select avg(a) from test
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BIGINT'
INSERT INTO test SELECT i::VARCHAR, -i::HUGEINT + -1234567891011121314151617180000::HUGEINT FROM range(0, 10000) tbl(i)
pragma force_compression='bitpacking'
CREATE OR REPLACE TABLE toy_table AS SELECT * FROM 'https://github.com/duckdb/duckdb-data/releases/download/v1.0/bp_bug.parquet'
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
INSERT INTO test_bp SELECT 1 FROM range(0, 10000) tbl(i)
INSERT INTO test_bp SELECT 2 FROM range(0, 10000) tbl(i)
SELECT segment_info FROM pragma_storage_info('test_bp') WHERE segment_type NOT IN ('VALIDITY')
PRAGMA force_bitpacking_mode = 'delta_for'
CREATE OR REPLACE TABLE test_bp (a INTEGER)
INSERT INTO test_bp SELECT 3*(i // 1000) + (i%10) FROM range(0, 10000) tbl(i)
CREATE TABLE test (a integer)
INSERT INTO test SELECT i FROM range(0,150000) tbl(i)
CREATE TABLE test_2 AS SELECT a FROM test
select sum(a) from test
select sum(a) from test_2
drop table test_2
CREATE TABLE test (id VARCHAR, a UHUGEINT)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'UHUGEINT'
INSERT INTO test_bp SELECT 1 FROM range(0, 1000) tbl(i)
INSERT INTO test_bp SELECT 2 FROM range(0, 1000) tbl(i)
CREATE TABLE test (s ROW(a INTEGER))
SELECT SUM(s['a']), MIN(s['a']), MAX(s['a']), COUNT(*) FROM test
SELECT compression FROM pragma_storage_info('alp') WHERE compression='ALP'
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE compression='ALP'
SELECT compression FROM pragma_storage_info('two_alp') WHERE compression='ALP'
select d, f from tbl1_uncompressed
select d, f from tbl1_alp
select d, f from tbl2_uncompressed
select d, f from tbl2_alp
select d, f from tbl3_uncompressed
select d, f from tbl3_alp
create or replace table list_doubles as select 5700 i, [5700.0] l UNION ALL select i, CASE WHEN i%128=0 THEN [i::DOUBLE] ELSE []::DOUBLE[] END as data from range(10000) tbl(i) union all select 5700, [i] FROM range(100) tbl(i)
SELECT * FROM list_doubles WHERE i=5700
PRAGMA force_compression='alp'
DROP TABLE all_types
SELECT compression FROM pragma_storage_info('random_double') WHERE segment_type == 'double' AND compression != 'Uncompressed'
create table random_alp_double as select * from random_double
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE segment_type == 'double' AND compression != 'ALP'
select * from random_double
select * from random_alp_double
create table random_double as select round(random(), 6)::DOUBLE as data from range(1024) tbl(i)
create table random_float as select round(random(), 6)::FLOAT as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_float') WHERE segment_type == 'float' AND compression != 'Uncompressed'
create table random_alp_float as select * from random_float
SELECT compression FROM pragma_storage_info('random_alp_float') WHERE segment_type == 'float' AND compression != 'ALP'
select * from random_float
select * from random_alp_float
create table random_double as select 0::DOUBLE as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('temperatures_double') WHERE compression='Patas'
PRAGMA force_compression = 'rle'
INSERT INTO test_rle SELECT i FROM range(0, 2000) tbl(i)
SELECT compression FROM pragma_storage_info('test_rle') WHERE segment_type ILIKE 'INTEGER'
CREATE TABLE test (a BOOLEAN)
INSERT INTO test select false from range(2048)
INSERT INTO test select true from range(2048)
SELECT COUNT(*) FROM test WHERE a=false
INSERT INTO test select 0 from range(4096)
INSERT INTO test select 1 from range(2048)
INSERT INTO test select 2 from range(2048)
INSERT INTO test select 3 from range(1024)
INSERT INTO test select 4 from range(1024)
INSERT INTO test select 5 from range(512)
INSERT INTO test select 6 from range(512)
INSERT INTO test select 7 from range(512)
INSERT INTO test select 8 from range(512)
select distinct on (types) vector_type(a) as types from test order by all
select distinct on (types) types from (select vector_type(a) from test limit 8192) tbl(types)
select distinct on (types) types from (select vector_type(a) from test offset 8192) tbl(types)
SELECT compression FROM pragma_storage_info('t') WHERE compression='RLE'
CREATE TABLE tbl AS SELECT i id, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
SELECT * FROM tbl WHERE id = 5040 AND rle_val=100
SELECT * FROM tbl WHERE id = 5040 AND substr(rle_val::VARCHAR, 1, 3)='100'
SELECT * FROM tbl WHERE id >= 5020 AND rle_val=100
SELECT * FROM tbl WHERE rle_val=100
INSERT INTO test SELECT i::VARCHAR id, 1 b FROM range(5000) tbl(i)
INSERT INTO test SELECT (5000 + i)::VARCHAR id, 2 b FROM range(5000) tbl(i)
SELECT SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE col=2
CREATE TABLE test(id INTEGER PRIMARY KEY, col INTEGER)
SELECT SUM(a), MIN(a), MAX(a), COUNT(*) FROM test
PRAGMA force_compression='RLE'
INSERT INTO integers SELECT NULL FROM range(65535)
INSERT INTO integers SELECT 1
INSERT INTO integers SELECT 2
INSERT INTO integers SELECT 3
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM integers
SELECT * FROM tbl WHERE id >= 75 and id <= 125 and id%4=0
SELECT * FROM tbl WHERE id >= 33380 and id <= 33410 and id%4=0
CREATE TABLE tbl2 AS SELECT i id, i%5 id_modulo, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
SELECT COUNT(*), SUM(rle_val), MIN(rle_val), MAX(rle_val), SUM(rle_val_null), COUNT(rle_val_null) FROM tbl2 WHERE id >= 1500 and id <= 2500 AND id_modulo=3
SELECT COUNT(*), SUM(rle_val), MIN(rle_val), MAX(rle_val), SUM(rle_val_null), COUNT(rle_val_null) FROM tbl2 WHERE id >= 1500 and id <= 19500 AND id_modulo<=2
pragma force_compression='rle'
create table tbl as SELECT * FROM ( VALUES (['first name', 'last name', 'username'], 60), (['first name'], 0), (['username'], 0), (['first name', 'last name', 'username'], 0), (['first name', 'last name', 'username'], 0), (['username'], 0), (['username'], 0) ) AS t(attributes, minutes_duration)
SELECT "minutes_duration" FROM tbl WHERE NOT list_sort(['first name']) = tbl."attributes" ORDER BY ALL
PRAGMA force_compression = 'dictionary'
CREATE TABLE test ( a INTEGER, b VARCHAR )
INSERT INTO test (a, b) SELECT x AS a, CASE x % 5 WHEN 0 THEN 'aaaa' WHEN 1 THEN 'bbbb' WHEN 2 THEN 'cccc' WHEN 3 THEN 'dddd' WHEN 4 THEN NULL END AS b FROM range(10_000) t(x)
select distinct b from test order by a % 5
INSERT INTO test_dict SELECT i::VARCHAR FROM range(0, 2000) tbl(i)
SET storage_compatibility_version='v1.2.0'
CREATE TABLE big_string ( a VARCHAR, id INT )
INSERT INTO big_string values (concat(range(0,500000)::VARCHAR), 5)
SELECT a[1], strlen(a) from big_string
create table tbl ( a varchar )
set variable my_string = ( select concat(range(0,1000)::VARCHAR) )
INSERT INTO tbl (a) SELECT CASE WHEN (i % 7) = 0 THEN NULL ELSE getvariable('my_string') || i END FROM range(5000) t(i)
select count(*) from tbl where a IS NULL
create table tbl as select i // 5_000 as num, num::VARCHAR || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
select str[0:1]::BIGINT from tbl where num = 1 limit 10
SET default_block_size = '16384'
PRAGMA force_compression = 'zstd'
INSERT INTO test VALUES ('11'), ('11'), ('12'), (NULL)
USE zstd
SET zstd_min_string_length = 1
SET force_compression = 'zstd'
CREATE TABLE zstd(big_list VARCHAR[])
INSERT INTO zstd select [ chr(x::INT) for x in generate_series( ord('a'), ord('z') ) ] FROM range(1_000_000)
CHECKPOINT zstd
SELECT avg(len(big_list)) FROM zstd
SET force_compression='zstd'
CREATE TABLE zstd_data AS SELECT concat('thisisalongstring', i) str FROM range(1000) t(i)
select count(*) from pragma_storage_info('zstd_data') where compression='ZSTD'
SELECT compression FROM pragma_storage_info('t') WHERE segment_type = 'DOUBLE' AND compression != 'ALPRD'
select d, f from tbl1_alprd
select d, f from tbl2_alprd
select d, f from tbl3_alprd
PRAGMA force_compression='alprd'
create table random_double as select round(cos(1 / (random() + 0.001)), 15)::DOUBLE * -1 as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE segment_type == 'double' AND compression != 'ALPRD'
create table random_double as select random()::DOUBLE as data from range(1024) tbl(i)
create table random_alprd_double as select * from random_double
SELECT compression FROM pragma_storage_info('random_alprd_double') WHERE segment_type == 'double' AND compression != 'ALPRD'
select * from random_alprd_double
create table random_float as select random()::FLOAT as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_alp_float') WHERE segment_type == 'float' AND compression != 'ALPRD'
pragma force_compression='uncompressed'
create table uncompressed_data as select i, repeat( (i % 200)::INTEGER::VARCHAR, 2047 // len((i % 200)::INTEGER::VARCHAR) ) a from range(20000) t(i)
select * from uncompressed_data order by i
pragma force_compression='dict_fsst'
create table compressed_data as select * from uncompressed_data
select count(distinct a) from compressed_data
select * from compressed_data order by i
select count(distinct a) from compressed_data where contains(a, '11')
select count(distinct a) from compressed_data where i%10=0
set checkpoint_threshold='10mb'
CREATE TABLE tbl AS SELECT { 'a': i, 'b': NULL::VARCHAR } col FROM range(5000) t(i) union all select { 'a': 10000, 'b': 'hello' }
set force_compression='dict_fsst'
SELECT segment_type, compression FROM pragma_storage_info('tbl') WHERE segment_type != 'BIGINT'
set force_compression='zstd'
CREATE OR REPLACE TABLE tbl AS SELECT { 'a': i, 'b': NULL::VARCHAR } col FROM range(5000) t(i) union all select { 'a': 10000, 'b': 'hello' } FROM range(2)
select segment_type, compression from pragma_storage_info('tbl') where segment_type IN ('VARCHAR', 'VALIDITY') order by all
SELECT col FROM tbl ORDER BY col.a DESC LIMIT 3
PRAGMA force_compression = 'dict_fsst'
INSERT INTO test (a, b) SELECT x AS a, CASE x % 5 WHEN 0 THEN 'aaaa' WHEN 1 THEN 'bbbb' WHEN 2 THEN 'cccc' WHEN 3 THEN 'this is not an inlined string' WHEN 4 THEN NULL END AS b FROM range(80) t(x)
SELECT DISTINCT b FROM test ORDER BY a % 5
SET storage_compatibility_version='latest'
SELECT COUNT("XXX XXX/XXX") FROM db.t WHERE "XXX XXX/XXX" IS NOT NULL
SELECT COUNT(*) FROM db.t WHERE "XXX XXX/XXX" IS NULL
SELECT * FROM db.t
pragma force_compression='DICT_FSST'
CREATE OR REPLACE TABLE t1(type VARCHAR, id VARCHAR, problem VARCHAR)
INSERT INTO t1(type,id,problem) select 'events', 'test', NULL from range(40)
SELECT COUNT(*) FROM t1 WHERE problem IS NULL
CREATE OR REPLACE TABLE t( compressed VARCHAR USING COMPRESSION 'DICT_FSST' )
INSERT INTO t VALUES ('Error3')
UPDATE t SET compressed = NULL
SELECT * FROM t AS e WHERE e.compressed IS NULL
USE db_v1
USE db_v13
CREATE TABLE normal_string (a VARCHAR)
SELECT list_aggr(str_split(a,''),'min'), list_aggr(str_split(a,''),'min'), strlen(a) from normal_string
CREATE TABLE big_string (a VARCHAR)
SELECT list_aggr(str_split(a,''),'min'), list_aggr(str_split(a,''),'min'), strlen(a) from big_string
SELECT lower(compression) FROM pragma_storage_info('big_string') WHERE segment_type ILIKE 'VARCHAR' LIMIT 1
DROP TABLE big_string
DROP TABLE normal_string
CREATE TABLE blob_empty (b BYTEA)
INSERT INTO blob_empty VALUES(''), (''::BLOB)
INSERT INTO blob_empty VALUES(NULL), (NULL::BLOB)
SELECT * FROM blob_empty
SELECT lower(compression)!='fsst' FROM pragma_storage_info('blob_empty') WHERE segment_type ILIKE 'BLOB' LIMIT 1
DROP TABLE blobs
DROP TABLE blob_empty
CREATE TABLE test_empty (a VARCHAR)
select * from test_empty
CREATE TABLE test_empty_large AS SELECT '' as a from range(0,10000) union all select 'A' union all select ''
select count(*), min(a[1]), max(a[1]) from test_empty_large limit 5
DROP TABLE test_empty
DROP TABLE test_empty_large
CREATE TABLE test (id INT, col VARCHAR)
INSERT INTO test SELECT i::INT id, concat('BLEEPBLOOP-', (i%10)::VARCHAR) col FROM range(10000) tbl(i)
SELECT MIN(col), MAX(col), COUNT(*) FROM test WHERE col >= 'BLEEPBLOOP-5'
SELECT MIN(id), MAX(id), MIN(col), MAX(col), COUNT(*) FROM test WHERE id='5000'
drop type if exists test_result
create type test_result as UNION( ok BOOL, err STRUCT( expected VARCHAR, actual VARCHAR ) )
CREATE TABLE test(id INTEGER PRIMARY KEY, col VARCHAR)
INSERT INTO test SELECT i id, i::VARCHAR b FROM range(10000) tbl(i)
SELECT MIN(id), MAX(id), SUM(col::INT), MIN(col::INT), MAX(col::INT), COUNT(*) FROM test WHERE id=5000
SET storage_compatibility_version='v1.0.0'
SET storage_compatibility_version='v1.3.0'
INSERT INTO test SELECT (i%500)::VARCHAR FROM range(0, 10000) tbl(i)
SELECT SUM(a::INT), MIN(a::INT), MAX(a::INT), COUNT(*) FROM test
INSERT INTO test SELECT CONCAT('A-',(i%5)::VARCHAR) FROM range(0,1025) tbl(i)
select * from test limit 5
select a[3] from test limit 5
CREATE TABLE test (s ROW(a VARCHAR))
SELECT SUM(s['a']::INT), MIN(s['a']::INT), MAX(s['a']::INT), COUNT(*) FROM test
INSERT INTO test SELECT (i%500)::VARCHAR FROM range(0,150000) tbl(i)
select sum(a::INT) from test
select sum(a::INT) from test_2
DROP TABLE test_2
PRAGMA force_compression='rle'
SELECT total_blocks * block_size < 10 * 262144 FROM pragma_database_size()
select count(*) from pragma_storage_info('integers') where block_id IS NULL
SELECT total_blocks FROM pragma_database_size()
SELECT total_blocks * block_size < 15 * 262144 FROM pragma_database_size()
INSERT INTO integers (i1) VALUES (NULL)
CREATE TABLE test_list_2 (a integer, b STRUCT(c VARCHAR[], d VARCHAR[], e INTEGER[]))
INSERT INTO test_list_2 SELECT 1, row(['a', 'b', 'c', 'd', 'e', 'f'], ['A', 'B'], [1, 5, 9]) FROM range(10)
DELETE FROM test WHERE a=0
DELETE FROM test WHERE a=1
CREATE TABLE test(val VARCHAR)
SELECT strlen(val) FROM test
SET checkpoint_threshold = '5 KB'
SELECT wal_size == '0 bytes' FROM pragma_database_size()
CREATE TABLE t1 (id INTEGER, c0 DOUBLE)
INSERT INTO t1 SELECT *, random() FROM range(200000)
SELECT wal_size != '0 bytes' FROM pragma_database_size()
INSERT INTO test SELECT case when i%3=0 then null else i end FROM range(1000000) t(i)
SELECT SUM(a), COUNT(a), COUNT(*) FROM test
CREATE TABLE integers AS SELECT 42 i
SELECT total_blocks < 10 FROM pragma_database_size()
CREATE TEMPORARY TABLE test (a INTEGER)
CREATE TEMPORARY TABLE test2 (a INTEGER)
INSERT INTO test2 SELECT * FROM range(1000000)
UPDATE test SET a=500000 WHERE a=0
CREATE TABLE bigtbl(i INT)
INSERT INTO bigtbl FROM range(1000000)
CREATE TABLE little_tbl(i INT)
INSERT INTO little_tbl VALUES (1)
SELECT COUNT(*), SUM(i) FROM bigtbl
SET force_column_metadata_reuse=true
CREATE TABLE tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4 FROM range(200000) t(i)
SELECT COUNT(*), SUM(c0) FROM tbl
CREATE TABLE other_tbl AS SELECT i AS x FROM range(1000) t(i)
INSERT INTO other_tbl SELECT i FROM range(1000, 2000) t(i)
SELECT COUNT(*), SUM(x) FROM other_tbl
SET experimental_metadata_reuse=true
CREATE TABLE ducklake_table(end_snapshot BIGINT)
CREATE TABLE ducklake_column(end_snapshot BIGINT)
INSERT INTO ducklake_table VALUES (1)
INSERT INTO ducklake_column VALUES (1)
UPDATE ducklake_table SET end_snapshot = 3
UPDATE ducklake_column SET end_snapshot = 3
CREATE TABLE my_table (a INTEGER, b INTEGER)
USE partial_reuse_carries_column_extras
SET debug_skip_checkpoint_on_commit=true
SET debug_verify_blocks=true
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15 FROM range(500000) t(i)
UPDATE wide_tbl SET c6 = c6 + 1, c7 = c7 + 1 WHERE c0 < 100
SELECT COUNT(*), SUM(c0) FROM wide_tbl
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15, i AS c16, i AS c17, i AS c18, i AS c19 FROM range(200000) t(i)
ALTER TABLE wide_tbl ADD COLUMN c20 INTEGER DEFAULT 42
SELECT COUNT(*), SUM(c0), SUM(c20) FROM wide_tbl
ALTER TABLE wide_tbl DROP COLUMN c10
UPDATE wide_tbl SET c0 = c0 + 1 WHERE c0 < 100
INSERT INTO bigtbl VALUES (NULL)
CREATE TABLE rollback.tbl AS SELECT range AS i FROM range(100)
SELECT * FROM block_size_16kb.tbl
SELECT * FROM vector_size_512.tbl
SELECT COUNT(*) > 0 FROM pragma_storage_info('T') WHERE segment_type ILIKE 'INTEGER' AND compression = 'RLE'
SELECT * FROM T_1
SELECT COUNT(*) > 0 FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' AND compression = 'RLE'
ALTER TABLE T_1 DROP COLUMN c_1
ALTER TABLE T_1 DROP COLUMN b_1
SELECT compression FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' LIMIT 2
ALTER TABLE T_1 ADD COLUMN b INTEGER DEFAULT 2
SELECT compression FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' LIMIT 3
CREATE TABLE smaller_block_size.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE larger_block_size.tbl AS SELECT range AS i FROM range(10000)
CHECKPOINT smaller_block_size
CHECKPOINT larger_block_size
SELECT COUNT(*) > 0 FROM pragma_storage_info('larger_block_size.tbl') WHERE compression = 'BitPacking'
CREATE TABLE no_bitpacking.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE has_bitpacking.tbl AS SELECT range AS i FROM range(10000)
CHECKPOINT has_bitpacking
CHECKPOINT no_bitpacking
SELECT COUNT(*) FROM pragma_storage_info('no_bitpacking.tbl') WHERE compression = 'BitPacking'
CREATE TABLE integers AS SELECT * FROM range(100000) tbl(i)
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers')
SELECT MEDIAN(count) FROM pragma_storage_info('integers')
SELECT * FROM integers_parquet LIMIT 5
SELECT * FROM integers_parquet LIMIT 5 OFFSET 73654
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers_parquet')
SELECT MEDIAN(count) FROM pragma_storage_info('integers_parquet')
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers_parquet_no_order')
SELECT MEDIAN(count) FROM pragma_storage_info('integers_parquet_no_order')
CREATE TABLE small.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE large.tbl AS SELECT range AS i FROM range(10000)
SELECT list_sum(LIST(t1.i) || LIST(t2.i)) FROM large.tbl AS t1 JOIN small.tbl AS t2 ON t1.i = t2.i
CREATE TABLE db1.all_types AS SELECT * FROM test_all_types()
SELECT * FROM test_all_types()
SELECT * FROM db1.all_types
SET catalog_error_max_schemas = 0
CREATE TABLE db1.integers(i INTEGER)
CHECKPOINT db1
VACUUM db1.integers
ATTACH DATABASE ':memory:' AS db1
CREATE TABLE db1.test(a INTEGER, b INTEGER, c VARCHAR(10))
ATTACH '' AS tmp
CREATE TABLE tmp.t1(id int)
CREATE INDEX idx ON tmp.t1(id)
CREATE TABLE test(a INTEGER)
CREATE INDEX index ON test(a)
CREATE TYPE db1.mood AS ENUM('ok', 'sad', 'happy')
CREATE TABLE db1.integers(i mood)
DETACH default_size
DETACH dbname
SET default_block_size = '262144'
ATTACH DATABASE ':memory:' AS new_database (BLOCK_SIZE 262144, ROW_GROUP_SIZE 2048)
SELECT options['block_size'] from duckdb_databases() where database_name = 'new_database'
SELECT options['row_group_size'] from duckdb_databases() where database_name = 'new_database'
SELECT database_name FROM pragma_database_size() WHERE database_name = 'db1'
ATTACH ':memory:' AS db2
SELECT database_name FROM pragma_database_size() WHERE database_name = 'db1' OR database_name = 'db2' ORDER BY ALL
ATTACH ':memory:' as "my""db"
CREATE TABLE "my""db".tbl(i int)
INSERT INTO "my""db".tbl VALUES (42)
USE "my""db"
SET search_path=current_setting('search_path')
USE attach_quoated_base
CREATE SCHEMA "my""db"."my""schema"
CREATE TABLE "my""db"."my""schema".tbl(i int)
INSERT INTO "my""db"."my""schema".tbl VALUES (84)
USE "my""db"."my""schema"
CREATE SCHEMA """"
USE """"
ATTACH ':memory:' AS MyDB
USE MyDB
SELECT current_database()
CREATE OR REPLACE TABLE ddb.my_table AS (SELECT 1337 as value)
from ddb
from ddb.my_table
from ddb.main.my_table
create table ddb as select 42 as value
from memory.main.ddb
SELECT t1.value, t2.value FROM memory.main.ddb as t1 JOIN ddb.main.my_table as t2 ON t1.value != t2.value
use ddb
from my_table
from main.my_table
use memory
DROP TABLE memory.main.ddb
ATTACH DATABASE ':memory:' AS new_database
CREATE TABLE pk_tbl (id INTEGER PRIMARY KEY, name VARCHAR UNIQUE)
CREATE TABLE fk_tbl (id INTEGER REFERENCES pk_tbl(id))
CREATE TABLE tbl_alter_column (id INT, other INT, nn_col INT NOT NULL, rm INT, rename_c INT, my_def INT, drop_def INT DEFAULT 10, new_null_col INT)
ALTER TABLE tbl_alter_column ADD COLUMN k INTEGER
ALTER TABLE tbl_alter_column ALTER other SET DATA TYPE VARCHAR USING concat(other, '_', 'yay')
ALTER TABLE tbl_alter_column ALTER COLUMN nn_col DROP NOT NULL
ALTER TABLE tbl_alter_column DROP rm
ALTER TABLE tbl_alter_column RENAME rename_c TO my_new_col
ALTER TABLE tbl_alter_column ALTER COLUMN my_def SET DEFAULT 10
ALTER TABLE tbl_alter_column ALTER COLUMN drop_def DROP DEFAULT
ALTER TABLE tbl_alter_column ALTER COLUMN new_null_col SET NOT NULL
CREATE TABLE hello(i INTEGER)
CREATE TABLE db1.test(a INTEGER)
CREATE SCHEMA db1.myschema
CREATE TABLE db1.myschema.blablabla(i INTEGER)
SET catalog_error_max_schemas=0
RESET catalog_error_max_schemas
SELECT * FROM myschema.blablabla
SELECT * FROM memory.hello
USE db1.myschema
SELECT * FROM db1.main.test
create table alias1.tbl1 as select 1 as a
FROM alias1.tbl1
DETACH alias1
FROM alias2.tbl1
SELECT database_name FROM duckdb_databases() WHERE database_name = 'first'
CREATE TABLE a1.test (a INTEGER PRIMARY KEY, b INTEGER)
CHECKPOINT a1
CREATE TABLE a2.test (a INTEGER PRIMARY KEY, b INTEGER)
CHECKPOINT a2
DETACH encrypted_aws
SELECT tags FROM duckdb_databases() WHERE database_name LIKE '%encrypted%' ORDER BY database_name
ATTACH 'data/attach_test/encrypted_ctr_key=abcde.db' as enc1 (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'CTR')
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc2 (ENCRYPTION_KEY 'abcde')
set autoinstall_known_extensions=false
set autoload_known_extensions=false
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'GCM', READ_ONLY)
FROM enc.test ORDER BY value
CREATE TABLE enc.test AS SELECT 1 as a
FROM enc.test
CREATE TYPE db1.mood AS ENUM ('sad', 'ok', 'happy')
SELECT enum_range(NULL::db1.mood) AS my_enum_range
SELECT enum_range(NULL::db1.main.mood) AS my_enum_range
DROP TYPE db1.mood
DROP TYPE IF EXISTS db1.main.mood
CREATE TABLE db1.person ( name text, current_mood mood )
INSERT INTO db1.person VALUES ('Moe', 'happy')
select * from db1.person
CREATE TYPE db2.mood AS ENUM ('ble','grr','kkcry')
CREATE TABLE db2.person ( name text, current_mood mood )
INSERT INTO db2.person VALUES ('Moe', 'kkcry')
select * from db2.person
ATTACH ':memory:' AS db1
ATTACH ':memory:' as other
INSERT INTO db1.integers VALUES (1), (2), (3), (NULL)
CREATE VIEW db1.integers_view AS SELECT * FROM integers
CREATE TABLE other.dont_export_me (i integer)
rollback
drop table db1.integers CASCADE
drop view integers_view
SELECT * FROM integers ORDER BY i NULLS LAST
SELECT * FROM integers_view order by i NULLS LAST
SET VARIABLE db_type='DUCKDB'
ATTACH ':memory:' AS db1 (TYPE getvariable('db_type'))
SET VARIABLE db_type='UNKNOWN_TYPE'
SELECT database_name FROM duckdb_databases() WHERE database_name = 'concurrent'
DETACH concurrent
DETACH con2_rollback_detach
DETACH con1
ATTACH DATABASE ':memory:' AS db2
INSERT INTO db1.song VALUES (11, 1, 'A', 'A_song'), (12, 2, 'B', 'B_song'), (13, 3, 'C', 'C_song')
CREATE TABLE dummy.tbl(i INTEGER)
DETACH dummy
FROM dummy.tbl
ATTACH ':memory:' AS hidden_db (HIDDEN true)
SELECT database_name FROM duckdb_databases() WHERE database_name = 'hidden_db'
SELECT database_name FROM duckdb_tables() WHERE database_name = 'hidden_db'
CREATE TABLE hidden_db.main.tbl AS SELECT 42 AS i
SELECT * FROM hidden_db.main.tbl
DETACH hidden_db
CREATE TABLE s1.integers AS FROM range(10) t(i)
SELECT SUM(i) FROM s1.integers
ATTACH '~/home_dir.db' AS s1
SELECT * FROM db.strings
SELECT * FROM db.strings ORDER BY 1
ATTACH IF NOT EXISTS ':memory:' AS db1
CREATE TABLE db1.tbl(i INTEGER)
USE attach_index_db
CREATE TABLE tbl_a ( a_id INTEGER PRIMARY KEY, value VARCHAR NOT NULL )
CREATE INDEX idx_tbl_a ON tbl_a (value)
INSERT INTO tbl_a VALUES (1, 'x')
INSERT INTO tbl_a VALUES (2, 'y')
SELECT * FROM tbl_a WHERE a_id = 2
USE other_attach_index
DETACH attach_index_db
SELECT * FROM attach_index_db.tbl_a WHERE a_id = 2
create table mytable (C1 VARCHAR(10))
insert into mytable values ('a')
create table TOMERGE.mytable (C1 VARCHAR(10))
insert into TOMERGE.mytable SELECT * FROM mytable
select * from TOMERGE.mytable
attach ':memory:' as test
use test
create schema schema1
create table schema1.table1 as select 1 as a
set schema='schema1'
select * from table1
create table tbl1 as select 1 as a
FROM test.tbl1
DETACH test
FROM tbl1
CREATE TABLE db1.tbl AS SELECT 42 AS x, 3 AS y
CREATE MACRO db1.two_x_plus_y(x, y) AS 2 * x + y
SELECT db1.two_x_plus_y(x, y) FROM db1.tbl
SELECT db1.main.two_x_plus_y(x, y) FROM db1.tbl
SELECT two_x_plus_y(x, y) FROM db1.tbl
ATTACH DATABASE ':memory:' AS database
CREATE TABLE database.integers(i INTEGER)
INSERT INTO database.integers SELECT * FROM range(10)
CREATE SCHEMA db1.s1
CREATE SCHEMA db2.s1
CREATE TABLE db1.s1.t(c INT)
CREATE TABLE db2.s1.t(c INT)
INSERT INTO db1.s1.t VALUES (42)
INSERT INTO db2.s1.t SELECT c * 2 FROM db1.s1.t
SELECT * FROM db1.s1.t, db2.s1.t
SELECT db1.t.c, db2.t.c FROM db1.s1.t, db2.s1.t
SELECT db1.s1.t.c, db2.s1.t.c FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (db1.s1.t.c) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (DB1.S1.T.C) FROM db1.s1.t, db2.s1.t
SELECT * EXCLUDE (s1.t.c) FROM db1.s1.t, (SELECT 42) t
CREATE SCHEMA database.schema
CREATE TABLE database.schema.table(col ROW(field INTEGER))
INSERT INTO database.schema.table VALUES ({'field': 42})
SELECT database.schema.table.col.field FROM database.schema.table
SELECT database.schema.table.col FROM database.schema.table
SELECT database.schema.table FROM database.schema.table
USE database
SELECT schema.table FROM database.schema.table
SELECT "table" FROM database.schema.table
USE database.schema
SELECT "table" FROM "table"
SELECT schema.table FROM "table"
SET force_compression='roaring'
CREATE TABLE db1.tbl AS SELECT CASE WHEN i%2=0 THEN NULL ELSE i END i FROM range(10000) t(i)
CREATE TABLE db1.str_tbl AS SELECT STRING_AGG('long_string_' || i, '-') FROM range(1000) t(i)
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.tbl') WHERE compression='Roaring'
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.str_tbl') WHERE compression='ZSTD'
CREATE TABLE db1.tbl2 AS FROM db1.tbl
CREATE TABLE db1.str_tbl2 AS FROM db1.str_tbl
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.tbl2') WHERE compression='Roaring'
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.str_tbl2') WHERE compression='ZSTD'
CREATE TABLE no_wal_writes.tbl AS SELECT range AS id, 0 AS v FROM range(5_000)
UPDATE no_wal_writes.tbl SET v = id
DETACH no_wal_writes
SELECT COUNT(*) FROM no_wal_writes.tbl WHERE v = 0
CHECKPOINT no_wal_writes
CREATE TABLE db2.all_types_new AS SELECT * FROM test_all_types()
DETACH db2
SELECT * FROM db1.all_types_new
SELECT * FROM db2.all_types
CREATE TABLE persistent_attach.integers(i INTEGER)
INSERT INTO persistent_attach.integers VALUES (42)
SELECT SUM(i) FROM persistent_attach.integers
DETACH persistent_attach
CREATE OR REPLACE TABLE persistent.T1 (A0 int)
insert into persistent.T1 values (5)
SELECT column_name from pragma_storage_info('persistent.T1')
CREATE TABLE db1.integers AS SELECT * FROM range(10) t(i)
SELECT SUM(i) FROM db1.integers
CREATE TABLE db2.integers AS SELECT * FROM db1.integers
SELECT SUM(i) FROM db2.integers
ATTACH ':memory:' AS db1 (READ_WRITE)
CREATE TABLE db1.test AS SELECT * FROM integers
INSERT INTO db1.integers VALUES (42)
BEGIN TRANSACTION READ ONLY
FROM db1.integers
CREATE TABLE wal_writes.tbl AS SELECT range AS id, 0 AS v FROM range(5_000)
DETACH wal_writes
INSERT INTO wal_writes.tbl VALUES (42, 42)
CHECKPOINT wal_writes
UPDATE wal_writes.tbl SET v = id
CREATE TABLE temp_db.integers(i INTEGER)
DETACH temp_db
CREATE TABLE system_db.integers(i INTEGER)
DETACH system_db
CREATE TABLE db1.tbl AS FROM range(10000) t(i)
INSERT INTO db1.tbl FROM range(10000)
CREATE TABLE test.data (key BIGINT PRIMARY KEY)
INSERT INTO test.data SELECT * FROM range(8190)
SELECT COUNT(DISTINCT row_group_id) FROM pragma_storage_info('test.data')
INSERT INTO test.data VALUES(8190), (8191)
SELECT row_group_id, SUM(count) FROM pragma_storage_info('test.data') where segment_type != 'VALIDITY' GROUP BY (row_group_id) ORDER BY row_group_id
CREATE SCHEMA new_database.s1
CREATE SEQUENCE db1.seq
CREATE TABLE db1.integers(i INTEGER DEFAULT nextval('db1.seq'))
SELECT nextval('db1.seq')
detach db1
set storage_compatibility_version='latest'
CREATE TABLE A (A1 INTEGER PRIMARY KEY,A2 VARCHAR, A3 INTEGER)
CREATE INDEX A_index ON A (A2)
CREATE TABLE B(B1 INTEGER REFERENCES A(A1))
USE db1_other
detach db2
CREATE TABLE new_database.tbl(b INTEGER)
CREATE TABLE new_database.s1.tbl(c INTEGER)
SHOW ALL TABLES
CREATE TABLE db1.table_in_db1(i int)
CREATE TABLE db2.table_in_db2(i int)
CREATE SCHEMA db2.test_schema
CREATE TABLE db2.test_schema.table_in_db2_test_schema(i int)
USE DB1
USE db2.test_schema
USE DB2.TEST_sChEmA
FROM table_in_db2
FROM table_in_db2_test_schema
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='version_1_2_0'
DETACH version_1_2_0
CREATE TABLE default_version.tbl(i VARCHAR)
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='default_version'
DETACH default_version
INSERT INTO default_version.tbl VALUES ('abcd'), ('efgh'), ('hello'), ('world'), (NULL)
CHECKPOINT default_version
FROM default_version.tbl
SET storage_compatibility_version = 'v1.2.0'
SELECT tags['storage_version'] FROM duckdb_databases() WHERE database_name='modified_default_setting'
CREATE TABLE test.tbl(i INTEGER PRIMARY KEY)
select constraint_catalog, table_catalog, table_name from information_schema.table_constraints limit 1
CREATE TABLE new_database.integers(i INTEGER)
PRAGMA table_info('new_database.integers')
CREATE SCHEMA new_database.new_schema
CREATE TABLE new_database.new_schema.integers(i INTEGER)
PRAGMA table_info('new_database.new_schema.integers')
USE new_database.new_schema
PRAGMA table_info('integers')
CREATE TABLE attach_transaction.integers(i INTEGER)
INSERT INTO attach_transaction.integers VALUES (42)
DETACH attach_transaction
INSERT INTO attach_transaction.integers VALUES (84)
SELECT * FROM attach_transaction.integers
SELECT * FROM attach_transaction.integers ORDER BY 1
attach ':memory:' as mem
use mem
USE view_search_path
CREATE TABLE my_tbl(i INTEGER)
INSERT INTO my_tbl VALUES (42)
CREATE VIEW my_view AS FROM my_tbl
FROM my_view
CREATE SCHEMA my_schema
USE my_schema
INSERT INTO my_tbl VALUES (84)
USE view_search_path_other
FROM view_search_path.my_view
FROM view_search_path.my_schema.my_view
DETACH view_search_path
CREATE TABLE t1 AS SELECT 42 i
CREATE TABLE t2(c1 INT)
ALTER TABLE t2 ALTER c1 SET DEFAULT 0
ATTACH DATABASE ':memory:' as db2
INSERT INTO db1.t2 DEFAULT VALUES
SELECT * FROM db1.t2
CREATE TABLE db1.test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
INSERT INTO db1.test (b) VALUES (1)
alter table db1.test RENAME TO blubb
INSERT INTO db1.blubb (b) VALUES (10)
SELECT * FROM db1.blubb
INSERT INTO db2.blubb (b) VALUES (100)
SELECT * FROM db2.blubb
ATTACH DATABASE ':memory:' AS varchar
DETACH varchar
INSERT INTO new_database.integers VALUES (42)
INSERT INTO new_database.main.integers VALUES (84)
SELECT * FROM new_database.integers ORDER BY i
SELECT * FROM new_database.main.integers ORDER BY i
SELECT * FROM new_database.integers ORDER BY new_database.integers.i
SELECT * FROM new_database.main.integers ORDER BY new_database.main.integers.i
CREATE SCHEMA new_db.my_schema
CREATE TABLE new_db.my_schema.my_table(col INTEGER)
INSERT INTO new_db.my_schema.my_table VALUES (42)
CREATE VIEW new_db.my_schema.my_view AS SELECT 84
CREATE SEQUENCE new_db.my_schema.my_sequence
CREATE MACRO new_db.my_schema.one() AS (SELECT 1)
CREATE MACRO new_db.my_schema.range(a) as TABLE SELECT * FROM range(a)
SELECT new_db.my_schema.one()
SELECT * FROM new_db.my_schema.range(3)
DETACH new_db
SELECT * FROM new_name.my_schema.my_table
SELECT * FROM new_name.my_schema.my_view
ATTACH 'https://raw.githubusercontent.com/duckdb/duckdb/main/data/attach_test/attach.db' AS db
ATTACH 'https://raw.githubusercontent.com/duckdb/duckdb/main/data/attach_test/attach.db' AS db2
SHOW DATABASES
SELECT name FROM pragma_database_list ORDER BY name
USE new_database
CREATE TABLE tbl AS SELECT 42 i
SELECT * FROM new_database.tbl
SHOW SCHEMAS
USE new_database.new_s2
DROP SCHEMA new_database.new_s2
DETACH memory
DESCRIBE SCHEMAS
CREATE TABLE my_table(first_column bigint)
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table DROP COLUMN fi') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table ALTER COLUMN fi') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table RENAME COLUMN fi') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('COP') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY tbl FRO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY tbl FROM ''file.csv'' HEAD') LIMIT 1
CREATE TABLE my_table(my_column INTEGER)
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY my_') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE F') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name(a) A') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name(a) AS a+1, (b) A') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name (a) AS TA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name (a) AS TABLE SEL') LIMIT 1
SELECT suggestion, suggestion_start, suggestion_type FROM sql_auto_complete('CREATE SCH') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA I') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA IF NO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA IF NOT EX') LIMIT 1
ATTACH ':memory:' AS attached_in_memory
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA attac') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQ') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQUENCE seq CYC') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQUENCE seq INC') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('cr') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE T') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE OR RE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create ta') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTEGER, j INTE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTEGER PRI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTEGER PRIMARY KE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTEGER UNIQ') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('create table tbl(i INTEGER UNIQUE NO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TY') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS ENU') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS TIME WITH TI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS ROW(ts TIMESTAMP WITH TIME ZON') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DRO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP VI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE IF EX') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE tbl CAS') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_') LIMIT 1
CREATE TABLE my_schema.table_in_schema(my_column INTEGER)
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_s') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_schema.t') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT NULL FR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT CAST(a AS INTE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT a::INTE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT col IS DIST') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT col IS DISTINCT FRO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT col COLL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT col BETW') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT CASE WH') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT sum(42) IS NOT NUL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT sum(disti') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT sum(a, b orde') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT sum(a) filt') LIMIT 1
CREATE SCHEMA my_catalog_entry
CREATE TABLE my_catalog_entry(i INT)
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM my_c') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INS') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT IN') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl VAL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl(c1, c2) VAL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl(c1, c2) SEL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT OR IG') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT OR REP') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA show_t') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA enable_che') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA disable_che') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA thre') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('select gam') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('select nexta') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('select bit_l') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SEL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('WI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl WH') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl AN') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl OR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl ORDER B') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl ORDER BY AL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl GR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl GROUP B') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl GROUP BY AL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM tbl GROUP BY ALL HAV') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET e_directory') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET timez') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET memory') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('set thr') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('set allowed_p') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DESCR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SHOW my_') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SHOW my_s') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DESCRIBE my_schema.t') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call histo') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call histogram_') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call duckdb_ty') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM duckdb_c') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call read_cs') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM read_csv_a') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call unnes') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CALL glo') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('from ran') LIMIT 1
CALL dbgen(sf=0)
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT l_ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT 1 + l_ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT min(l_ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT min(42, l_ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT ''test_string'' LIKE l_c') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT l_orderkey FROM lin') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT l_orderkey FROM lineitem, ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT l_orderkey FROM lineitem JOIN ord') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT (SELECT SUM(l_orderkey) FROM lineit') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT * FROM (FROM lineit') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO lin') LIMIT 1
CREATE VIEW v1 AS SELECT 42 my_column_name
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT my_col') LIMIT 1
CREATE VIEW v2(alias_name) AS SELECT 42 alias
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT alias') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (RANG') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (RANGE BETWE') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (RANGE BETWEEN UNBOU') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (RANGE BETWEEN UNBOUNDED PREC') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (RANGE BETWEEN CURRENT ROW AND 5 PREC') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT row_number() OVER (PART') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT lag(col1) OVER (PARTITION BY col1, col2 ORD') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT sum(42) OVER (PARTITION BY col1, col2 ORDER BY col3 ROW') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT lead(l_orderkey) OVER win FR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT lead(l_orderkey) OVER (win) FROM tbl WINDOW win AS (PART') LIMIT 1
SELECT MAP([MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2])], [1])
SELECT MAP([2], [{'key1': MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2])}])
SELECT [MAP([2], [{'key1': MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2]), 'key2': 2}])]
INSERT INTO arrays VALUES ([1, 2, 3]), ([4, 5, 6]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
INSERT INTO arrays VALUES ([1, 2, 3]), ([1, 2, 4]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
select flatten([['a'], ['b'], ['c']]::varchar[1][3])
SELECT length(array_value(1, 2, 3))
create table arrays(a int[3])
insert into arrays values ([1, 2, 3]), ([4, 5, 6])
select length(a) from arrays
select length(NULL::int[3]) from arrays
insert into arrays values (NULL)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 1)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 2)
SELECT list_distinct(array_value(1,1,2,3,3)) = list_distinct([1,1,2,3,3])
SELECT list_sort(array_value(3,2,1)) = list_sort([3,2,1])
SELECT list_slice(array_value(1,2,3,4,5), 1, 3) = list_slice([1,2,3,4,5], 1, 3)
SELECT list_transform(array_value(3,2,1), lambda x: x + 1) = list_transform([3,2,1], lambda x: x + 1)
SELECT list_filter(array_value(3,2,1), lambda x: x > 1) = list_filter([3,2,1], lambda x: x > 1)
SELECT list_concat(array_value(1,2,3), array_value(4,5,6))
SELECT list_concat(array_value(1,2,3), NULL), list_concat(NULL, array_value(4,5,6))
SELECT list_resize(array_value(1,2), 3)
SELECT list_resize(array_value(1,2), 1)
SELECT list_resize(array_value(1,2), 0)
SELECT list_position(array_value(1,2,3), 2)
SELECT list_position(array_value(1,2,3), 4)
CREATE TABLE intervals(i INTERVAL, s VARCHAR)
INSERT INTO intervals VALUES ('2 years', 'year'), ('16 months', 'quarter'), ('42 days', 'day'), ('2066343400 microseconds', 'minute')
SELECT date_part(NULL::VARCHAR, NULL::INTERVAL) FROM intervals
SELECT date_part(s, NULL::INTERVAL) FROM intervals
SELECT date_part(NULL, i) FROM intervals
SELECT date_part(s, INTERVAL '4 years 5 months 18 days 128 seconds') FROM intervals
SELECT date_part('seconds', i) FROM intervals
SELECT date_part('epoch', i) FROM intervals
SELECT date_part(s, i) FROM intervals
SELECT i, DATE_PART(['year', 'month', 'day'], i) AS parts FROM intervals ORDER BY 1
SELECT i, DATE_PART(['millennium', 'century', 'decade', 'quarter'], i) AS parts FROM intervals ORDER BY 1
SELECT i, DATE_PART(['hour', 'minute', 'second', 'epoch'], i) AS parts FROM intervals ORDER BY 1
CREATE TABLE intervals(i INTERVAL)
INSERT INTO intervals VALUES ('2 years'), ('16 months'), ('42 days'), ('2066343400 microseconds'), (NULL)
SELECT EXTRACT(year FROM i) FROM intervals
SELECT EXTRACT(month FROM i) FROM intervals
SELECT EXTRACT(day FROM i) FROM intervals
SELECT EXTRACT(decade FROM i) FROM intervals
SELECT EXTRACT(century FROM i) FROM intervals
SELECT EXTRACT(millennium FROM i) FROM intervals
SELECT EXTRACT(quarter FROM i) FROM intervals
SELECT EXTRACT(epoch FROM i) FROM intervals
SELECT EXTRACT(microsecond FROM i) FROM intervals
SELECT EXTRACT(millisecond FROM i) FROM intervals
CREATE TABLE INTERVAL_MULDIV_TBL (span interval)
INSERT INTO INTERVAL_MULDIV_TBL VALUES ('41 months 12 days 360:00'), ('-41 months -12 days 360:00'), ('-12 days'), ('9 months -27 days 12:34:56'), ('-3 years 482 days 76:54:32.189'), ('4 months'), ('14 months'), ('999 months 999 days'),
SELECT span * 0.3 AS product FROM INTERVAL_MULDIV_TBL
SELECT span * 8.2 AS product FROM INTERVAL_MULDIV_TBL
SELECT span / 10 AS quotient FROM INTERVAL_MULDIV_TBL
SELECT span / 100 AS quotient FROM INTERVAL_MULDIV_TBL
select (interval '1 days') * 0.5::DOUBLE
select 0.5::DOUBLE * (interval '1 days')
select 2::BIGINT * (interval '1 days')
select (interval '1 days') * 2::BIGINT
SELECT i FROM intervals
SELECT DATE_TRUNC('millennium', i) FROM intervals
SELECT DATE_TRUNC('century', i) FROM intervals
SELECT DATE_TRUNC('decade', i) FROM intervals
SELECT DATE_TRUNC('hour', i) FROM intervals
SELECT DATE_TRUNC('minute', i) FROM intervals
SELECT DATE_TRUNC('millisecond', i) FROM intervals
SELECT DATE_TRUNC('microsecond', i) FROM intervals
SELECT DATE_TRUNC(s, i) FROM intervals
SELECT DATE_TRUNC(NULL, i) FROM intervals
INSERT INTO dates VALUES (DATE '1992-01-01')
SELECT DATE_ADD(DATE '2008-12-25', INTERVAL 5 DAY) AS five_days_later
SELECT DATE_ADD(TIMESTAMP '2008-12-25 00:00:00', INTERVAL 5 DAY) AS five_days_later
SELECT datediff('week', DATE '-5877641-06-25', DATE '5881580-07-10')
SELECT datediff('day', DATE '-5877641-06-25', DATE '5881580-07-10')
SELECT datediff('day', DATE '-5877641-06-25', DATE '5881580-07-10') / 7
SELECT datediff('week', DATE '5881580-07-10', DATE '-5877641-06-25')
SELECT datediff('microsecond', DATE '2000-01-01', DATE '2000-01-02')
SELECT datediff('microsecond', DATE '2000-01-02', DATE '2000-01-01')
SELECT EXTRACT(year FROM d) FROM dates
SELECT EXTRACT(month FROM d) FROM dates
SELECT EXTRACT(day FROM d) FROM dates
SELECT EXTRACT(decade FROM d) FROM dates
SELECT EXTRACT(century FROM d) FROM dates
SELECT EXTRACT(millennium FROM d) FROM dates
SELECT EXTRACT(microseconds FROM d) FROM dates
SELECT EXTRACT(milliseconds FROM d) FROM dates
SELECT EXTRACT(second FROM d) FROM dates
SELECT EXTRACT(minute FROM d) FROM dates
SELECT EXTRACT(hour FROM d) FROM dates
SELECT EXTRACT(epoch FROM d) FROM dates
create table t1 (date timestamp)
insert into t1 values ('2016-12-16T00:00:00.000Z')
insert into t1 values ('2020-02-17T23:59:59.998Z')
insert into t1 values ('2020-02-17T23:59:59.999Z')
insert into t1 values ('2020-02-18T00:00:00.000Z')
select * from t1 WHERE (date_trunc('DAY', T1.date) < ('2020-02-17T23:59:59.999Z'::timestamp)) ORDER BY 1
CREATE table T1(A0 TIMESTAMP)
SELECT date_trunc('DAY', A0) FROM T1
CREATE TABLE events as FROM (VALUES (TIMESTAMP '1992-09-20 20:38:40', 'Event A'), (TIMESTAMP '1992-09-20 21:45:15', 'Event B'), (TIMESTAMP '1992-09-20 22:15:30', 'Event C')) t(event_time, event_name)
CREATE TABLE users as FROM (VALUES (1, TIMESTAMP '1992-09-20 20:00:00'), (2, TIMESTAMP '1992-09-20 22:05:00')) t(user_id, created_at)
SELECT u.user_id, date_trunc('minute', e.event_time) AS truncated_minute FROM users u LEFT JOIN events e ON u.user_id = 1 ORDER BY e.event_time ASC
CREATE TABLE dates(d DATE, s VARCHAR)
INSERT INTO dates VALUES ('1992-01-01', 'year'), ('1992-03-03', 'month'), ('1992-05-05', 'day'), ('2022-01-01', 'isoyear'), ('044-03-15 (BC)', 'millennium'), ('infinity', 'century'), ('-infinity', 'decade'), (NULL, 'weekday'),
CREATE TABLE specifiers (specifier VARCHAR)
SELECT date_part(NULL::VARCHAR, NULL::TIMESTAMP) FROM dates
SELECT date_part(s, NULL::TIMESTAMP) FROM dates
SELECT date_part(NULL, d) FROM dates
SELECT date_part(s, DATE '1992-01-01') FROM dates
SELECT date_part('year', d) FROM dates
SELECT date_part('isoyear', d) FROM dates
SELECT date_part(s, d) FROM dates
SELECT date_part('era', d) FROM dates
SELECT date_part('julian', d) FROM dates
CREATE TABLE timestamps(d TIMESTAMP, s VARCHAR)
INSERT INTO dates VALUES ('1992-12-02', 'year'), ('1993-03-03', 'month'), ('1994-05-05', 'day'), ('2022-01-01', 'isoyear')
SELECT date_trunc(NULL::VARCHAR, NULL::TIMESTAMP) FROM dates
SELECT date_trunc(s, NULL::TIMESTAMP) FROM dates
SELECT date_trunc(NULL, d) FROM dates
SELECT date_trunc(NULL::VARCHAR, NULL::TIMESTAMP) FROM timestamps LIMIT 3
SELECT date_trunc(s, NULL::TIMESTAMP) FROM timestamps LIMIT 3
SELECT date_trunc(NULL, d) FROM timestamps LIMIT 3
SELECT date_trunc('month', DATE '1992-02-02') FROM dates LIMIT 1
SELECT date_trunc(s, d) FROM dates
SELECT date_trunc('minute', TIMESTAMP '1992-02-02 04:03:02') FROM timestamps LIMIT 1
SELECT date_trunc(s, d) FROM timestamps
CREATE TABLE dates(i DATE)
INSERT INTO dates VALUES ('1993-08-14'), (NULL)
SELECT EXTRACT(year FROM i) FROM dates
SELECT EXTRACT(month FROM i) FROM dates
SELECT EXTRACT(quarter FROM i) FROM dates
SELECT EXTRACT(day FROM i) FROM dates
SELECT EXTRACT(decade FROM i) FROM dates
SELECT EXTRACT(century FROM i) FROM dates
SELECT EXTRACT(DOW FROM i) FROM dates
SELECT EXTRACT(DOY FROM i) FROM dates
SELECT EXTRACT(epoch FROM i) FROM dates
SELECT EXTRACT(ISODOW FROM i) FROM dates
SELECT EXTRACT(century FROM cast('2000-10-10' AS DATE))
SELECT EXTRACT(century FROM cast('2001-10-10' AS DATE))
SELECT EXTRACT(millennium FROM cast('2000-10-10' AS DATE))
SELECT EXTRACT(millennium FROM cast('2001-10-10' AS DATE))
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) + 0)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) - 0)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) + 1)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) - 1)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) + 2)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) - 2)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) + 3)
SELECT EXTRACT(dow FROM cast('1970-01-01' AS DATE) - 3)
select date '1992-01-01' + interval (i) days, month(date '1992-01-01' + interval (i) days) from range(0, 366) tbl(i)
select date '1993-01-01' + interval (i) days, month(date '1993-01-01' + interval (i) days) from range(0, 366) tbl(i)
CREATE TABLE dates AS SELECT date '1970-01-01' + concat(i, ' years')::interval AS d from range(0, 430) tbl(i)
CREATE TABLE dates2 AS SELECT date '1970-01-01' + concat(i * 6, ' months')::interval AS d from range(0, 200) tbl(i)
SELECT EXTRACT(year FROM d) FROM dates ORDER BY 1
SELECT EXTRACT(year FROM d) FROM dates2 ORDER BY 1
SELECT strftime(DATE '1992-01-01', '%Y')
SELECT strftime('%Y', DATE '1992-01-01')
SELECT strftime('%Y', TIMESTAMP '1992-01-01')
SELECT strftime(DATE '1992-01-01', '(%Y)')
SELECT strftime(DATE '1992-01-01', '%% %Y %%')
SELECT strftime(DATE '1992-01-01', '%%%%%% %Y %%%%%%')
SELECT strftime(DATE '1992-02-01', '%d/%m/%Y')
SELECT strftime(DATE '1992-02-01', '%Y %Y %Y %Y')
SELECT strftime(d, '%d/%m/%Y') FROM dates ORDER BY d
SELECT strftime(NULL::DATE, '%d/%m/%Y') FROM dates ORDER BY d
SELECT strftime(d, NULL) FROM dates ORDER BY d
SELECT strftime(NULL::TIMESTAMP, NULL) FROM range(3)
SELECT strftime(d, '%a') FROM dates ORDER BY d
SELECT strftime(d, '%A') FROM dates ORDER BY d
SELECT strftime(d, '%w') FROM dates ORDER BY d
SELECT strftime(d, '%u') FROM dates ORDER BY d
SELECT strftime(d, '%d') FROM dates ORDER BY d
SELECT strftime(d, '%-d') FROM dates ORDER BY d
SELECT strftime(d, '%b') FROM dates ORDER BY d
SELECT strftime(d, '%h') FROM dates ORDER BY d
SELECT strftime(d, '%B') FROM dates ORDER BY d
SELECT strftime(d, '%m') FROM dates ORDER BY d
SELECT strftime(d, '%-m') FROM dates ORDER BY d
SELECT strftime(d, '%y') FROM dates ORDER BY d
CREATE TABLE dates(w INTERVAL, d DATE, shift INTERVAL, origin DATE)
select d, time_bucket('3 days'::interval, d) from dates
select d, time_bucket('3 years'::interval, d) from dates
select d, time_bucket(null::interval, d) from dates
select w, d, time_bucket(w, d) from dates
select d, time_bucket('4 days'::interval, d, '6 hours'::interval) from dates
select d, time_bucket('2 weeks'::interval, d, '6 days'::interval) from dates
select d, time_bucket('3 months'::interval, d, '6 days'::interval) from dates
select d, time_bucket(null::interval, d, '6 days'::interval) from dates
select time_bucket('3 months'::interval, null::date, '6 days'::interval) from dates
select d, time_bucket('3 months'::interval, d, null::interval) from dates
select w, d, shift, time_bucket(w, d, shift) from dates
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy', 'anxious')
CREATE TABLE test (x mood)
INSERT INTO test VALUES ('ok'), ('sad'), ('anxious'), ('happy')
SELECT enum_code(x) FROM test
PREPARE p1 as SELECT enum_code(x) FROM test
EXECUTE p1
PREPARE p2 as SELECT enum_code(?)
EXECUTE p2('happy'::mood)
CREATE TYPE rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
SELECT enum_first(null::rainbow)
SELECT enum_last(null::rainbow)
CREATE TYPE currency AS ENUM ('usd', 'brl', 'eur')
SELECT enum_range(null::rainbow)
SELECT enum_range_boundary('orange'::rainbow, 'green'::rainbow)
SELECT enum_range_boundary('green'::rainbow, 'orange'::rainbow)
SELECT enum_range_boundary(NULL, 'green'::rainbow)
SELECT enum_range_boundary('orange'::rainbow, NULL)
select epoch(TIME '14:21:13')
select extract(epoch from TIME '14:21:13')
select extract(seconds from TIME '14:21:13')
CREATE TABLE times(d TIME, s VARCHAR)
INSERT INTO times VALUES ('00:01:20', 'hour'), ('20:08:10.998', 'minute'), ('20:08:10.33', 'second'), ('20:08:10.001', 'millisecond')
SELECT date_part(NULL::VARCHAR, NULL::TIME) FROM times
SELECT date_part(s, NULL::TIME) FROM times
SELECT date_part(NULL, d) FROM times
SELECT date_part(s, TIME '14:28:50.447') FROM times
SELECT date_part('hour', d) FROM times
SELECT date_part(s, d) FROM times
SELECT d, DATE_PART(['hour', 'minute', 'microsecond'], d) AS parts FROM times ORDER BY 1
SELECT d, DATE_PART(['epoch', 'second', 'timezone', 'timezone_hour', 'timezone_minute'], d) AS parts FROM times ORDER BY 1
SELECT d, epoch_ns(d) FROM times ORDER BY ALL
SELECT d, epoch_us(d) FROM times ORDER BY ALL
CREATE TABLE times(i TIME)
INSERT INTO times VALUES ('00:01:20'), ('20:08:10.998'), ('20:08:10.33'), ('20:08:10.001'), (NULL)
SELECT EXTRACT(second FROM i) FROM times
SELECT EXTRACT(minute FROM i) FROM times
SELECT EXTRACT(hour FROM i) FROM times
SELECT EXTRACT(milliseconds FROM i) FROM times
SELECT EXTRACT(microseconds FROM i) FROM times
SELECT EXTRACT(epoch FROM i) FROM times
SELECT stats(EXTRACT(second FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(minute FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(hour FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(milliseconds FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(microseconds FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(epoch FROM i)) FROM times LIMIT 1
SELECT abs('-0.0'::float), abs('-0.0'::double)
SELECT 10 % 2.4, -10 % 2.4
SELECT 10.0 % 2.4, -10.0 % 2.4
SELECT 12345678901111111 % 2.0
select 12345678901234567890 % 123
SELECT 10000000000000000000000000000000000001::DECIMAL(38,0) % 0.00000000000000000000000000000000004
SELECT typeof(10.0 % 2.0), typeof(10.0 % 2.0 % 2.0 % 2.0)
SELECT 10.0 % 0.0
create table t1 as select * from generate_series(1,50) as t(number)
select * from t1 using sample 5
CREATE TABLE test(a integer)
insert into test values (1), (2), (3), (NULL)
select add(a,a) from test
select subtract(a,a) from test
select multiply(a,a) from test
select divide(a,a) from test
CREATE TABLE bits(t tinyint, s smallint, i integer, b bigint, h hugeint)
INSERT INTO bits VALUES (NULL, NULL, NULL, NULL, NULL), (31, 1023, 11834119, 50827156903621017, 3141592653589793238462643383279528841), (-59, -517, -575693, -9876543210, -148873535527910577765226390751398592512)
select bit_count(t), bit_count(s), bit_count(i), bit_count(b), bit_count(h) from bits
select i, even(i + 0.4) from generate_series(-4,4) tbl(i)
select i, even(i + 0.9) from generate_series(-4,4) tbl(i)
SELECT even(19.4), even(-19.4)
SELECT even(8.9), even(-8.9)
SELECT even(45::DOUBLE), even(-35::DOUBLE)
SELECT even(NULL)
SELECT even(1.7976931348623155e+308)
SELECT even(-1.7976931348623155e+308)
SELECT factorial(0)
SELECT factorial(NULL)
SELECT factorial(2)
SELECT factorial(10)
SELECT 10!
SELECT factorial(20)
SELECT factorial(30)
SET ieee_floating_point_ops=false
CREATE TABLE rs(x DOUBLE, y INTEGER)
INSERT INTO rs VALUES (10, 3),(10,-3),(-10,3),(-10,-3),(0,1),(1,1),(NULL,10),(10,NULL),(NULL,NULL)
SELECT fmod(x, y) FROM rs
SELECT fdiv(x, y) FROM rs
SELECT fmod(42, 0)
SELECT fmod(0, 0)
SELECT fdiv(42, 0)
SELECT fdiv(-42, 0)
SELECT fdiv(0, 0)
SELECT fmod(12.3456789, 5)
SELECT fdiv(12.3456789, 5)
CREATE TABLE numbers(n DOUBLE)
INSERT INTO numbers VALUES (NULL),(-42.8),(-42.2),(0), (42.2), (42.8)
SELECT cast(CEIL(n::tinyint) as bigint) FROM numbers ORDER BY n
SELECT cast(CEIL(n::smallint) as bigint) FROM numbers ORDER BY n
SELECT cast(CEIL(n::integer) as bigint) FROM numbers ORDER BY n
SELECT cast(CEIL(n::bigint) as bigint) FROM numbers ORDER BY n
SELECT cast(CEIL(n::float) as bigint) FROM numbers ORDER BY n
SELECT cast(CEIL(n::double) as bigint) FROM numbers ORDER BY n
SELECT cast(CEILING(n::double) as bigint) FROM numbers ORDER BY n
SELECT cast(FLOOR(n::tinyint) as bigint) FROM numbers ORDER BY n
SELECT cast(FLOOR(n::smallint) as bigint) FROM numbers ORDER BY n
SELECT cast(FLOOR(n::integer) as bigint) FROM numbers ORDER BY n
SELECT gamma(NULL)
SELECT gamma(-1)
SELECT gamma(1)
SELECT gamma(-0.1)
SELECT gamma(2)
SELECT gamma(10)
SELECT gamma(2::tinyint)
SELECT gamma(2::hugeint)
SELECT lgamma(NULL)
SELECT lgamma(-1)
SELECT lgamma(-100)
SELECT lgamma(1)
SELECT a, b, gcd(a, b), gcd(a, -b), gcd(b, a), gcd(-b, a) FROM (VALUES (0::int8, 0::int8), (0::int8, 29893644334::int8), (288484263558::int8, 29893644334::int8), (-288484263558::int8, 29893644334::int8), ((-9223372036854775808)::int8, 1::int8), ((-9223372036854775808)::int8, 9223372036854775807::int8), ((-9223372036854775808)::int8, 4611686018427387904::int8)) AS v(a, b)
SELECT gcd(42, NULL)
select lcm(120,25)
SELECT a, b, lcm(a, b), lcm(a, -b), lcm(b, a), lcm(-b, a) FROM (VALUES (0::int8, 0::int8), (0::int8, 29893644334::int8), (29893644334::int8, 29893644334::int8), (288484263558::int8, 29893644334::int8), (-288484263558::int8, 29893644334::int8), ((-9223372036854775808)::int8, 0::int8)) AS v(a, b)
SELECT lcm(42, NULL)
CREATE TABLE numbers(x DOUBLE)
INSERT INTO numbers VALUES (NULL), (1), (2)
SELECT geomean(x) FROM numbers
SELECT geomean(x::integer) FROM numbers
SELECT geomean(i) FROM generate_series(1000, 2000) tbl(i)
SELECT SQRT(0)
SELECT POW(1e300,100), POW(-1e300,100), POW(-1.0, 0.5)
SELECT EXP(1e300), EXP(1e100)
SELECT DEGREES(1e308)
INSERT INTO floats VALUES (3), ('nan'), ('inf'), ('-inf'), (NULL)
SELECT f, isnan(f), isinf(f), isfinite(f) FROM floats ORDER BY f
DROP TABLE floats
CREATE TABLE modme(a DOUBLE, b INTEGER)
INSERT INTO modme VALUES (42.123456, 3)
select mod(a, 40) from modme
select mod(42, 0)
select mod(a, 2) from modme
select mod(b, 2.1) from modme
select nextafter(NULL,1)
select nextafter(1,NULL)
select nextafter(99, 1)
select nextafter(99.0::DOUBLE, 1.0::DOUBLE) < 99
select nextafter(99.0::DOUBLE, 99.0::DOUBLE) = 99
select nextafter(99.0::DOUBLE, 100.0::DOUBLE) > 99
select nextafter(nextafter(99.0::DOUBLE, 100.0::DOUBLE),0::DOUBLE) = 99
select nextafter(99.0::FLOAT, 1.0::FLOAT) < 99
select nextafter(99.0::FLOAT, 100.0::FLOAT) > 99
select nextafter(nextafter(99.0::FLOAT, 100.0::FLOAT),0::FLOAT) = 99
create table test (a FLOAT)
INSERT INTO test VALUES (10),(20),(30),(40)
select abs(-17.4)
select cbrt(27.0)
select ceil(-42.8)
select ceiling(-95.3)
select exp(1.0)
select floor(-42.8)
select ln(2.0)
select log(100.0)
select log10(100.0)
select log2(4.0)
select pi()
select sqrt(2.0)
CREATE TABLE powerme(a DOUBLE, b INTEGER)
INSERT INTO powerme VALUES (2.1, 3)
select pow(a, 0) from powerme
select pow(b, -2) from powerme
select pow(a, b) from powerme
select pow(b, a) from powerme
select power(b, a) from powerme
CREATE TABLE t1 AS SELECT [random() for a IN range(1)] FROM range(2)
CREATE TABLE t2 AS SELECT random() FROM range(2)
CREATE TABLE t3 AS SELECT [random()] FROM range(2)
CREATE TABLE t4 AS SELECT [random() + range * 0 for a IN range(1)] FROM range(2)
SELECT count(*) FROM t1 WHERE (SELECT min(#1) FROM t1 ) == (SELECT max(#1) FROM t1)
SELECT count(*) FROM t2 WHERE (SELECT min(#1) FROM t2 ) == (SELECT max(#1) FROM t2)
SELECT count(*) FROM t3 WHERE (SELECT min(#1) FROM t3 ) == (SELECT max(#1) FROM t3)
SELECT count(*) FROM t4 WHERE (SELECT min(#1) FROM t4 ) == (SELECT max(#1) FROM t4)
CREATE TEMPORARY TABLE t1 AS SELECT RANDOM() a
CREATE TEMPORARY TABLE t2 AS SELECT RANDOM() b
CREATE TEMPORARY TABLE t3 AS SELECT RANDOM() c
SELECT COUNT(*) FROM (SELECT a FROM t1 JOIN t2 ON (a=b) JOIN t3 ON (b=c)) s1
CREATE TABLE roundme(a DOUBLE, b INTEGER)
INSERT INTO roundme VALUES (42.123456, 3)
select round(42.12345::DOUBLE, 0)
select round(42.12345::DOUBLE)
select round(42.12345::DOUBLE, 2)
select round(42.12345::DOUBLE, 4), round(42.1235::DOUBLE, 1000)
select round(42::DOUBLE, 0)
select round(42::DOUBLE, -1), round(42::DOUBLE, -2), round(42::DOUBLE, -1000)
select round(a, 1) from roundme
select round(b, 1) from roundme
select round(a, b) from roundme
SELECT round(1.0, (-2147483648)::INT)
SELECT roundBankers(45, -1)
select i, round_even(i + 0.5, 0) from generate_series(-2,4) tbl(i)
select i, round_even(i + 0.55, 0) from generate_series(-2,4) tbl(i)
select i, roundBankers(i + 0.55, 0) from generate_series(-2,4) tbl(i)
SELECT roundBankers(45, -1), roundBankers(35, -1)
SELECT roundBankers(45.5, 0), roundBankers(44.5, 0)
SELECT roundBankers(45.55, 1), roundBankers(45.45, 1)
SELECT roundBankers(-45, -1), roundBankers(-35, -1)
SELECT roundBankers(-45.5, 0), roundBankers(-44.5, 0)
SELECT roundBankers(-45.55, 1), roundBankers(-45.45, 1)
SELECT roundBankers(45::DOUBLE, -1), roundBankers(35::DOUBLE, -1)
SELECT roundBankers(45.5::DOUBLE, 0), roundBankers(44.5::DOUBLE, 0)
CREATE TABLE zz AS SELECT CAST(i AS SMALLINT) AS id, CAST(i AS SMALLINT) AS si FROM generate_series(1, 1000) t(i)
SELECT ROUND(53) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
SELECT ROUND(53, si) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
SELECT ROUND(53, -si) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
select round(100::INTEGER, int) from test_all_types()
INSERT INTO floats VALUES (3), (1.0::float), (-0.0::float), ('inf'), ('-inf'), (NULL)
SELECT f, signbit(f), isinf(f), isfinite(f) FROM floats ORDER BY f
SELECT signbit(1.0 / 0.0)
INSERT INTO numbers VALUES (-42),(-1),(0), (1), (42), (NULL)
SELECT cast(SIN(n::tinyint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(SIN(n::smallint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(SIN(n::integer)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(SIN(n::bigint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(SIN(n::float)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(SIN(n::double)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(COS(n::tinyint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(COS(n::smallint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(COS(n::integer)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(COS(n::bigint)*1000 as bigint) FROM numbers ORDER BY n
SELECT cast(COS(n::float)*1000 as bigint) FROM numbers ORDER BY n
CREATE TABLE truncme(a DOUBLE, b INTEGER, c UINTEGER)
INSERT INTO truncme VALUES (42.123456, 3, 19), (-3.141592, -7, 5)
INSERT INTO truncme VALUES (42.123456, 37, 19), (-3.141592, -75, 5)
SELECT 1::TINYINT + 1::TINYINT
SELECT 1::TINYINT + 1::SMALLINT
SELECT 1::TINYINT + 1::INT
SELECT 1::TINYINT + 1::BIGINT
SELECT 1::TINYINT + 1::REAL
SELECT 1::TINYINT + 1::DOUBLE
SELECT 1::SMALLINT + 1::TINYINT
SELECT 1::SMALLINT + 1::SMALLINT
SELECT 1::SMALLINT + 1::INT
SELECT 1::SMALLINT + 1::BIGINT
SELECT 1::SMALLINT + 1::REAL
SELECT 1::SMALLINT + 1::DOUBLE
INSERT INTO test VALUES (2)
SELECT ++-++-+i FROM test
SELECT +i FROM test
SELECT -i FROM test
SELECT +++++++i FROM test
SELECT -+-+-+-+-i FROM test
CREATE TABLE minima (t TINYINT, s SMALLINT, i INTEGER, b BIGINT)
INSERT INTO minima VALUES (-128, -32768, -2147483648, -9223372036854775808)
INSERT INTO dates VALUES ('1992-02-02')
SELECT length([1,2,3])
SELECT length([])
SELECT len(NULL)
SELECT array_length(ARRAY[1, 2, 3], 1)
SELECT len([1]) FROM range(3)
CREATE TABLE lists AS SELECT * FROM (VALUES ([1, 2]), ([NULL]), (NULL), ([]), ([3, 4, 5, 6, 7])) tbl(l)
SELECT len(l) FROM lists
SELECT array_to_string([1,2,3], '')
SELECT array_to_string([1,2,3], '-')
SELECT array_to_string(NULL, '-')
SELECT array_to_string([1, 2, 3], NULL)
SELECT array_to_string([], '-')
SELECT array_to_string([i, i + 1], '-') FROM range(6) t(i) WHERE i<=2 OR i>4
SELECT array_to_string_comma_default([1,2,3])
SELECT array_to_string_comma_default([1,2,3], sep:=',')
SELECT array_to_string_comma_default([1,2,3], sep:='')
SELECT array_to_string_comma_default([1,2,3], sep:='-')
SELECT array_to_string_comma_default(NULL, sep:='-')
SELECT array_to_string_comma_default([1, 2, 3], sep:=NULL)
SELECT array_to_string_comma_default([], sep:='-')
SELECT array_to_string_comma_default([i, i + 1], sep:='-') FROM range(6) t(i) WHERE i<=2 OR i>4
SELECT flatten([[1, 2, 3, 4]])
SELECT flatten([[1, 2], [3, 4]])
SELECT flatten([[], []])
SELECT flatten([[1, 2], [], [3, 4]])
SELECT flatten([[1, 2], []])
SELECT flatten([[], [1, 2]])
SELECT flatten(NULL)
SELECT flatten([NULL])
SELECT flatten([[NULL]])
SELECT flatten([NULL, [1], [2, 3], NULL, [4, NULL], [NULL, NULL]])
SELECT flatten([[[1, 2], [3, 4]], [[5,6], [7, 8]]])
SELECT flatten(flatten(flatten([[[[1], [2]], [[3], [4]]], [[[5], [6]], [[7], [8]]]])))
SELECT range(3)
SELECT generate_series(3)
SELECT range(3) FROM range(3)
SELECT range(i) FROM range(3) tbl(i)
SELECT range(NULL) FROM range(3) tbl(i)
SELECT range(CASE WHEN i%2=0 THEN NULL ELSE i END) FROM range(6) tbl(i)
SELECT range(0)
SELECT range(-1)
SELECT range(NULL)
SELECT range(1, 3)
SELECT generate_series(1, 3)
SELECT range(1, 1)
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-07-01', interval '3' month)
SELECT range(timestamp '2020-01-01', timestamp '2020-07-01', interval '3' month)
SELECT generate_series(timestamp '2020-06-01', timestamp '2020-01-01', -interval '3' month)
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-01-01', interval '1' day)
SELECT range(timestamp '2020-01-01', timestamp '2020-01-01', interval '1' day)
SELECT generate_series(timestamp '2020-06-01', timestamp '2020-01-01', interval '3' month)
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-06-01', -interval '3' month)
SELECT generate_series(NULL, timestamp '2020-06-01', -interval '3' month)
SELECT generate_series(timestamp '2020-01-01', NULL, -interval '3' month)
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-06-01', NULL)
SELECT count(*) FROM ( SELECT unnest(generate_series(timestamp '2000-01-01', timestamp '2020-06-01', interval '1' day)) )
SELECT generate_subscripts([4,5,6], 1)
SELECT generate_subscripts([], 1)
SELECT generate_subscripts(NULL, 1)
SELECT generate_series(timestamptz '2020-01-01', timestamptz '2020-07-01', interval '3' month)
SELECT range(timestamptz '2020-01-01', timestamptz '2020-07-01', interval '3' month)
SELECT generate_series(timestamptz '2020-06-01', timestamptz '2020-01-01', -interval '3' month)
SELECT generate_series(timestamptz '2020-01-01', timestamptz '2020-01-01', interval '1' day)
SELECT range(timestamptz '2020-01-01', timestamptz '2020-01-01', interval '1' day)
SELECT generate_series(timestamptz '2020-06-01', timestamptz '2020-01-01', interval '3' month)
SELECT generate_series(timestamptz '2020-01-01', timestamptz '2020-06-01', -interval '3' month)
SELECT count(*) FROM ( SELECT unnest(generate_series(timestamptz '2000-01-01', timestamptz '2020-06-01', interval '1' day)) )
SELECT generate_series(start, stop, step) FROM (VALUES (timestamptz '2020-01-01', timestamptz '2020-07-01', interval '3' month), (timestamptz '2020-12-04', timestamptz '2020-09-01', interval '-1 month -1 day'), (timestamptz '2020-03-08', timestamptz '2020-03-09', interval '6' hour), (timestamptz '2020-11-02', timestamptz '2020-11-01', interval '-43200' second), ) AS _(start, stop, step)
SELECT range(start, stop, step) FROM (VALUES (timestamptz '2020-01-01', timestamptz '2020-07-01', interval '3' month), (timestamptz '2020-12-04', timestamptz '2020-09-01', interval '-1 month -1 day'), (timestamptz '2020-03-08', timestamptz '2020-03-09', interval '6' hour), (timestamptz '2020-11-02', timestamptz '2020-11-01', interval '-43200' second), ) AS _(start, stop, step)
SELECT list_concat([1, 2], [3, 4])
SELECT array_cat([1, 2], [3, 4])
SELECT list_concat(NULL, [3, 4])
SELECT list_concat([1, 2], NULL)
SELECT list_concat([], [])
SELECT list_concat([], [3, 4])
SELECT list_concat([1, 2], [])
SELECT list_concat([1, 2], [3, 4], [5, 6])
SELECT list_concat([1, 2], [3, 4], [])
SELECT list_concat([1, 2], [], [5, 6])
SELECT list_concat([], [3, 4], [5, 6])
SELECT list_concat([], [], [5, 6])
create table TEST2 (i int[], j int)
insert into TEST2 values ([2,1,3], 2), ([2,3,4], 5), ([1], NULL)
select list_contains(i, j) from TEST2
create table TEST (i int[])
insert into TEST values ([2,1,3]), ([2,3,4]), ([1])
SELECT i, list_contains(i,1) from TEST
SELECT i, list_contains(i,4.0) from TEST
DROP table TEST
create table STR_TEST (i string[])
insert into STR_TEST values (['a','b','c']), (['d','a','e']), (['b']), (['aaaaaaaaaaaaaaaaaaaaaaaa'])
SELECT i, list_contains(i,'a') from STR_TEST
SELECT i, list_contains(i,'aaaaaaaaaaaaaaaaaaaaaaaa') from STR_TEST
INSERT INTO lists VALUES ([1, 2, 3]), ([4, 5, 6]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
SELECT list_cosine_similarity(l, [1, 2, 3]) FROM lists
SELECT list_cosine_similarity([], [])
INSERT INTO lists VALUES ([1, 2, 3]), ([1, 2, 4]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
SELECT list_distance(l, [1, 2, 3]) FROM lists
SELECT list_distance([], [])
SELECT list_distinct(NULL)
SELECT list_distinct([NULL])
SELECT list_distinct([])
SELECT list_distinct([]) WHERE 1 = 0
SELECT UNNEST(list_distinct([1, 1, 2, 2, 2, 3])) AS l ORDER BY l
SELECT UNNEST(list_distinct([1, 1, NULL, 2, 2, 2, 3, NULL, NULL])) AS l ORDER BY l
SELECT UNNEST(list_distinct(list_distinct([1, 1, -5, 10, 10, 2]))) AS l ORDER BY l
CREATE TABLE integers (l integer[])
INSERT INTO integers VALUES ([1, 1, 1]), ([1, NULL, 1, NULL])
INSERT INTO integers VALUES ([NULL]), (NULL), ([])
SELECT list_distinct(l) FROM integers
SELECT UNNEST(array_distinct([1, 2, 2, NULL])) AS l ORDER BY l
select list_has_any([1,2,3], [2,3,4])
select list_has_all([1,2,3], [2,3,4])
CREATE TABLE list_data(l1 int[], l2 int[])
INSERT INTO list_data VALUES (NULL, NULL)
INSERT INTO list_data VALUES (NULL, [1,2,3])
INSERT INTO list_data VALUES ([1,2,3], NULL)
INSERT INTO list_data VALUES ([1,2,3], [2,3,NULL])
INSERT INTO list_data VALUES ([1,2,NULL], [2,3,NULL])
INSERT INTO list_data VALUES ([1,2,NULL], [NULL,3,4])
INSERT INTO list_data VALUES ([1,2,3], [1,2,3])
INSERT INTO list_data VALUES ([1,2,3], [1,2,NULL])
select list_has_any(l1, l2) from list_data
SELECT list_inner_product([], [])
SELECT list_inner_product(l, [1, 2, 3]) FROM lists
DROP TABLE list_data
create table list_of_list(l1 int[][], l2 int[][])
insert into list_of_list values (NULL, NULL)
insert into list_of_list values ([[1 , 2, 3], NULL, [3, 2, 1]], [[ 2, 3, 4], NULL, [1, 2, 3]])
drop table list_of_list
create table list_of_strings(l1 string[], l2 string[])
insert into list_of_strings values (NULL, NULL)
insert into list_of_strings values ([NULL, 'a', 'b', 'c'], [NULL, 'b', 'c', 'd'])
insert into list_of_strings values (['here is a very long long string that is def more than 12 bytes', 'and a shorty'], ['here is a very long long string that is def more than 12 bytes', 'here is a very long long string that is def more than 12 bytes', 'c', 'd'])
drop table list_of_strings
create table large_lists(l1 int[], l2 int[])
insert into large_lists values (range(1, 3000), range(2000, 3000))
select list_position(i, j) from TEST2
SELECT i, list_position(i,1) from TEST
SELECT i, list_position(i,4.0) from TEST
create table TEST(i int[], j int)
insert into TEST values ([2,1,3], 2), ([2,3,4], 5), ([1], NULL), ([1, NULL, 2], NULL)
SELECT i, j, list_position(i,j) from TEST
CREATE TABLE NULL_TABLE (n int[], i int)
INSERT INTO NULL_TABLE VALUES (NULL, 1), (NULL, 2), (NULL, 3)
SELECT list_contains(n, i) FROM NULL_TABLE
DROP TABLE NULL_TABLE
SELECT i, list_position(i,'a') from STR_TEST
SELECT i, list_position(i,'aaaaaaaaaaaaaaaaaaaaaaaa') from STR_TEST
SELECT list_position(['NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_position([NULL, 0, 'NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_contains([NULL, 0, 'NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_position([[[NULL, 42]]], [[NULL, 42]])
SELECT list_resize([1, 2, 4], 2)
create table tbl(a int[], b int)
insert into tbl values ([5,4,3], 3)
insert into tbl values ([1,2,3], 5)
insert into tbl values (NULL, 8)
insert into tbl values ([10,11,12], 2)
select list_resize(a, b) from tbl
SELECT list_resize([], 2)
create table string_tbl(a string[], b int)
insert into string_tbl values (['abc', 'def'], 3)
insert into string_tbl values (['d', 'ef', 'ghij'], 8)
insert into string_tbl values (['lmnopqrs'], 5)
prepare q1 as select list_resize(?, ?)
prepare q2 as select array_resize(?, ?)
prepare q3 as select list_resize(?, ?, ?)
prepare q4 as select array_resize(?, ?, ?)
SELECT list_reverse(NULL)
SELECT list_reverse([])
SELECT list_reverse([NULL])
SELECT list_reverse([1, 42, 2])
SELECT array_reverse([1, 42, 2])
SELECT list_reverse([1, 42, NULL, 2])
SELECT list_reverse(list_reverse([1, 3, 3, 42, 117, 69, NULL]))
SELECT list_reverse ([[1, 2 ,42], [3, 4]])
create or replace table tbl_big as select range(5000) as list
select list_sort((list), 'desc') == list_reverse(list) from tbl_big
CREATE TABLE tbl (id INTEGER, list INTEGER[])
INSERT INTO tbl VALUES (1, [NULL, 3, 117, 42, 1]), (2, NULL), (3, [1, 8, 9]), (4, NULL), (5, NULL), (6, [NULL])
select child, count(*) as cnt, list_sort( list( parent ) ) as source_parents from test1 group by 1 having cnt > 1
SELECT list_unique(NULL)
SELECT list_unique([NULL])
SELECT list_unique([])
SELECT list_unique([]) WHERE 1 = 0
SELECT list_unique([1, 1, 2, 2, 2, 3])
SELECT list_unique([1, 1, NULL, 2, 2, 2, 3, NULL, NULL])
SELECT list_unique([1, 1, -5, 10, 10, 2])
INSERT INTO integers VALUES ([1, 1, 2, 2, 2, 3]), ([1, NULL, 1, NULL])
SELECT list_unique(l) FROM integers
SELECT array_unique([1, 2, 2, NULL])
SELECT list_unique([True, True, False, NULL])
SELECT list_unique([NULL::BOOLEAN])
SELECT list_value([3, 2, 1]::INTEGER[3], [4, 5, 6]::INTEGER[3])
SELECT list_value(['a', 'b', 'c']::VARCHAR[3], ['d', 'e', 'f']::VARCHAR[3])
SELECT list_value([DATE '2022-01-01', DATE '2022-01-02']::DATE[2], [DATE '2022-01-03', DATE '2022-01-04']::DATE[2])
SELECT list_value([1, 2]::INTEGER[2], [1.5, 2.5]::DOUBLE[2])
SELECT list_value([1, 2]::INTEGER[2], [3, 4]::DOUBLE[2], [5, 6]::INTEGER[2])
SELECT list_value([1.5, 2.5]::DOUBLE[2], [3, 4]::INTEGER[2], [5, 6]::INTEGER[2])
SELECT list_value([1, 2]::INTEGER[2], [3, 4]::INTEGER[2], [5.6, 7.8]::DOUBLE[2])
SELECT list_value([1, 2, 3]::INTEGER[3], NULL::INTEGER[3], [4, 5, 6]::INTEGER[3])
SELECT list_value([1, NULL, 3]::INTEGER[3], [3, 2, 1]::INTEGER[3])
SELECT list_value(['a', 'b', 'c']::VARCHAR[3], ['d', 'e', 'f']::VARCHAR[3], NULL::VARCHAR[3])
SELECT list_value(['a', NULL, 'c']::VARCHAR[3], ['d', 'e', 'f']::VARCHAR[3])
SELECT list_value([ROW(1, 'a'), ROW(2, 'b')]::ROW(i INTEGER, s VARCHAR)[], [ROW(3, 'c'), ROW(4, 'd')]::ROW(i INTEGER, s VARCHAR)[])
SELECT LIST_VALUE([1, 7], [2], [3])
SELECT LIST_VALUE([1, 7], [2], [3], NULL)
SELECT LIST_VALUE([1, 7], [2], NULL, [3])
SELECT LIST_VALUE([1, 7], [NULL], [2], [3])
CREATE TABLE test_table (c1 INTEGER[], c2 INTEGER[], c3 INTEGER[])
INSERT INTO test_table VALUES ([1, 1], [2, 2], [3])
INSERT INTO test_table VALUES ([4], [5, 5, 5], [6, 6])
INSERT INTO test_table VALUES ([7, 7, 7, 7], [8], [9, 9, 9])
INSERT INTO test_table VALUES ([], [], [])
INSERT INTO test_table VALUES ([-1, -1, NULL], NULL, [-2, -2])
SELECT LIST_VALUE(c1, c2, c3) FROM test_table
SELECT LIST_VALUE([ROW(1, 1), ROW(2, 2)], [ROW(3, 3)])
SELECT list_value({'a': 1, 'b': 'a'}, {'a': 2, 'b': 'b'})
SELECT list_value(NULL, {'a': 1, 'b': 'a'}, {'a': 2, 'b': 'b'})
SELECT list_value(NULL, {'a': 1, 'b': 'a'}, {'a': 2, 'b': 'b'}, NULL, {'a': 3, 'b': 'c'})
SELECT list_value({'a': 1, 'b': 'a'}, {'a': 2, 'b': 'b'}, {'a': NULL, 'b': 'c'})
CREATE TABLE tbl (s struct(a int, b varchar))
INSERT INTO tbl VALUES ({'a': 1, 'b': 'hello'}), ({'a': 42, 'b': 'world'})
SELECT list_value(s, s, s) AS res FROM tbl
CREATE TABLE tbl (s1 struct(a int, b varchar), s2 struct(a int, b varchar), s3 struct(a int, b varchar))
INSERT INTO tbl VALUES ({'a': 1, 'b': 'a'}, {'a': 2, 'b': 'b'}, {'a': 3, 'b': 'c'})
INSERT INTO tbl VALUES ({'a': 4, 'b': 'd'}, NULL, {'a': 6, 'b': 'f'})
INSERT INTO tbl VALUES ({'a': 7, 'b': 'g'}, {'a': 8, 'b': NULL}, {'a': 9, 'b': 'i'})
INSERT INTO tbl VALUES (NULL, NULL, NULL)
CREATE TABLE integers (i int[])
INSERT INTO integers VALUES ([1,2,3]), ([4,5,6])
CREATE TABLE selections (j boolean[])
INSERT INTO selections VALUES ([true, false, true]), ([false, true, false])
SELECT list_where([0, 1, 2], [true, false, false])
SELECT list_where(i, [true, false, true]) FROM integers
SELECT list_where(i, j) FROM integers, selections order by all
SELECT list_where([1,2,3], [True, True, False, False])
SELECT list_where([1,2,3], [True, False])
CREATE TABLE lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE bools AS SELECT range % 4 a, list((range % 2):: bool) s FROM range(10000) GROUP BY range % 4
select g, list_where(l, s) l from lists, bools
CREATE TABLE bools (b bool)
INSERT INTO bools VALUES (false), (true)
CREATE TABLE integers2 (j int[])
INSERT INTO integers2 VALUES ([]), (NULL)
SELECT list_zip([1,2,3])
SELECT list_zip([1,2,3], [2,3,4], [3,4,5], [])
SELECT list_zip([1,2,3], [1,2,3])
SELECT list_zip([1,2,3], [1,2])
SELECT list_zip([1, 2, 3]::int[3], [1, 2]::int[2], true)
SELECT list_zip([1, 2, 3]::int[3], [1, 2, 3]::int[3])
SELECT list_zip([1, '2', 3]::int[], [1, 2, 3]::int[3])
SELECT list_zip([1,2], [1,2,3])
SELECT repeat([1], 10)
SELECT repeat([{'x': 1}], 5)
SELECT repeat([[1]], 10)
SELECT repeat([1, 2], 5)
SELECT repeat([[[], [], [NULL], NULL]], 3)
SELECT repeat(['hello', 'thisisalongstring'], 5)
SELECT repeat([], 10)
SELECT repeat([], -1)
SELECT repeat(NULL::INT[], 10)
SELECT repeat(repeat([1], 50), 50) = repeat([1], 2500)
SELECT REPEAT(c,2) FROM TEST_VECTOR_TYPES(CAST(NULL AS INT[])) AS t(c)
SELECT COALESCE(*COLUMNS(lambda c: {'title': c}.title IN ('a', 'c'))) FROM (SELECT NULL, 2, 3) t(a, b, c)
CREATE TABLE addresses (i INT, b INT)
INSERT INTO addresses VALUES (1, 10), (2, 20), (1, 52), (3, 7)
SELECT i, sum(b) FROM addresses GROUP BY i HAVING sum(b) >= list_sum(list_transform([20], lambda x: {'title': x}.title + 30))
SELECT list_transform([10], lambda x: sum(1) + {'title': x}.title)
CREATE TABLE test (a VARCHAR[])
INSERT INTO test VALUES (NULL), ([]), (['asdf']), (['qwer', 'CXZASDF'])
ALTER TABLE test ALTER COLUMN a TYPE STRUCT(title VARCHAR)[] USING (list_transform(a, lambda x: {'title': x}))
SELECT a FROM test ORDER BY ALL
SET lambda_syntax='DISABLE_SINGLE_ARROW'
SELECT list_transform([10], lambda x: sum(1) + x)
SELECT list_filter([10], lambda x: sum(1) > 0)
SELECT list_transform([NULL, DATE '1992-09-20', DATE '2021-09-20'], lambda elem: extract('year' FROM elem) BETWEEN 2000 AND 2022)
SELECT list_filter([NULL, DATE '1992-09-20', DATE '2021-09-20'], lambda elem: extract('year' FROM elem) BETWEEN 2000 AND 2022)
SELECT list_transform(['hello', 'duck', 'sunshine'], lambda str: CASE WHEN str LIKE '%e%' THEN 'e' ELSE 'other' END)
SELECT list_filter(['hello', 'duck', 'sunshine'], lambda str: (CASE WHEN str LIKE '%e%' THEN 'e' ELSE 'other' END) LIKE 'e')
SELECT list_transform([2.0::DOUBLE], lambda x: x::INTEGER)
SELECT list_filter([2], lambda x: x::DOUBLE == 2)
SELECT list_transform([2.4, NULL, -4.7], lambda x: x != 10.4)
SELECT list_filter([2.4, NULL, -4.7], lambda x: x != -4.7)
SELECT list_transform([True, False, NULL], lambda x: x AND true)
SELECT [1] AS l, list_filter([1], lambda l: l > 1)
SELECT list_filter(NULL, lambda x: x > 1)
SELECT list_filter([True], lambda x: x)
SELECT list_filter(['duck', 'a', 'ö'], lambda duck: contains(concat(duck, 'DB'), 'duck'))
SELECT list_filter([1, 2, 3], lambda x: x % 2 = 0)
SELECT list_filter([], lambda x: x > 1)
SELECT list_filter([1, NULL, -2, NULL], lambda x: x % 2 != 0)
SELECT list_filter([5, -6, NULL, 7], lambda x: x > 0)
SELECT list_filter([5, NULL, 7, NULL], lambda x: x IS NOT NULL)
CREATE TABLE lists (n integer, l integer[])
INSERT INTO lists VALUES (1, [1]), (2, [1, 2, 3]), (3, NULL), (4, [-1, NULL, 2])
SELECT list_filter(l, lambda x: x + 1 <= 2) FROM lists
CREATE TABLE incorrect_test (i INTEGER)
CREATE TABLE l_filter_test (l integer[])
CREATE TABLE tbl AS SELECT {'a': 10} AS s
CREATE TABLE nested_list(i INT[][], other INT[])
INSERT INTO nested_list VALUES ([[1, 2]], [3, 4])
CREATE TABLE map_tbl(m MAP(INTEGER, INTEGER))
CREATE TABLE dummy_tbl (y INT)
SET lambda_syntax='ENABLE_SINGLE_ARROW'
CREATE OR REPLACE FUNCTION transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
CREATE TABLE t1 AS SELECT [1, 2, 3] AS x
SELECT list_apply(['hello'], lambda x: x) FROM t1
CREATE TABLE t2 AS SELECT [[1], [2], [3]] AS x
SELECT list_transform([[1], [2], [3]], lambda x: x[1]) FROM t2
CREATE TABLE l_test (l integer[])
INSERT INTO l_test VALUES ([1, 2, 3])
SELECT l, list_transform(l, lambda l: l + 1) FROM l_test
INSERT INTO l_filter_test VALUES ([1, 2])
SELECT l, list_filter(l, lambda l: l > 1) FROM l_filter_test
CREATE TABLE qualified_tbl (x INTEGER[])
INSERT INTO qualified_tbl VALUES ([1, 2])
SELECT list_transform(qualified_tbl.x, lambda x: (qualified_tbl.x)[1] + 1 + x) FROM qualified_tbl
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: z) AS row )
FROM demo(3, 0)
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: 0 + z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
FROM demo(3, {'a': 2})
CREATE TABLE tbl (tag_product VARCHAR)
INSERT INTO tbl VALUES ('milk chickpeas apples'), ('chocolate pepper')
SELECT tag_product, list_aggr(list_transform( string_split(tag_product, ' '), lambda word: lower(word)), 'string_agg', ',') AS tag_material, FROM tbl GROUP BY tag_product ORDER BY ALL
SELECT 1, list_transform([5, 4, 3], lambda x: x + 1) AS lst GROUP BY 1
CREATE TABLE uniform_purchase_forecast AS SELECT 'gold' AS color, 10 AS forecast UNION ALL SELECT 'blue', 15 UNION ALL SELECT 'red', 300
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(lambda x: x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, lambda x: x)
FROM test_all_types() GROUP BY ALL HAVING array_intersect(NULL, NULL)
SELECT x FROM (VALUES (42)) t(x) GROUP BY x HAVING list_filter(NULL, lambda lambda_param: lambda_param = 1)
CREATE TABLE test AS SELECT range i FROM range(3)
CREATE MACRO list_contains_macro(x, y) AS (list_contains(x, y))
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, lambda x: x + num))
SELECT list_filter([[1, 2], NULL, [3], [4, NULL]], lambda f: list_count(macro_with_lambda(f, 2)) > 1)
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, lambda a: x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, lambda x, y: x + y + num))
SELECT reduce_macro([1, 2, 3, 4], 5)
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, lambda x, y: list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, lambda x: x + y + z))
SELECT scoping_macro([11, 22], 3, 4)
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], lambda x: 0))
SELECT foo(v) FROM (SELECT 0 AS v)
SELECT list_transform(list_filter([0, 1, 2, 3, 4, 5], lambda x: x % 2 = 0), lambda y: y * y)
SELECT [x * x for x in [0, 1, 2, 3, 4, 5] if x%2=0]
SELECT list_filter(list_filter([2, 4, 3, 1, 20, 10, 3, 30], lambda x: x % 2 == 0), lambda y: y % 5 == 0)
SELECT [x for x in [x for x in [2, 4, 3, 1, 20, 10, 3, 30] if x % 2 == 0] if x % 5 == 0]
SELECT list_filter(['apple', 'banana', 'cherry', 'kiwi', 'mango'], lambda fruit: contains(fruit, 'a'))
SELECT [fruit for fruit in ['apple', 'banana', 'cherry', 'kiwi', 'mango'] if contains(fruit, 'a')]
CREATE TABLE fruit_tbl AS SELECT ['apple', 'banana', 'cherry', 'kiwi', 'mango'] fruits
SELECT [fruit for fruit in fruits if contains(fruit, 'a')] FROM fruit_tbl
SELECT list_transform([[1, NULL, 2], [3, NULL]], lambda a: list_filter(a, lambda x: x IS NOT NULL))
SELECT [len(x) for x in ['goodbye', 'cruel', 'world']]
CREATE TABLE word_tbl AS SELECT ['goodbye', 'cruel', 'world'] words
SELECT [len(x) for x in words] FROM word_tbl
SELECT list_reduce([1, 2, 3], lambda x, y: x + y)
SELECT list_reduce([1, 2, 3], lambda x, y: x * y)
SELECT list_reduce([100, 10, 1], lambda x, y, i: x - y - i)
SELECT list_reduce([1, 2, 3], lambda x, y: y - x)
SELECT list_reduce([1, 2, 3], lambda x, y: x - y)
SELECT list_reduce([1, 2, 3], lambda x, y, i: x + y + i)
SELECT list_reduce([NULL], lambda x, y, i: x + y + i)
SELECT list_reduce(NULL, lambda x, y, i: x + y + i)
SELECT list_reduce(['Once', 'upon', 'a', 'time'], lambda x, y: x || ' ' || y)
SELECT list_reduce(['a', 'b', 'c', 'd'], lambda x, y, i: x || ' - ' || CAST(i AS VARCHAR) || ' - ' || y)
CREATE table t1(a int[])
INSERT INTO t1 VALUES ([1, 2, 3])
SELECT list_reduce([1, 2, 3], lambda x, y: x + y, 100)
SELECT list_reduce([1, 2, 3], lambda x, y: x * y, -1)
SELECT list_reduce([100, 10, 1], lambda x, y, i: x - y - i, 1000)
SELECT list_reduce([1, 2, 3], lambda x, y: y - x, -1)
SELECT list_reduce([1, 2, 3], lambda x, y: x - y, 10)
SELECT list_reduce([1, 2, 3], lambda x, y, i: x + y + i, -1)
SELECT list_reduce([1, 2, 3], lambda x, y: x + y, NULL)
SELECT list_reduce([NULL], lambda x, y, i: x + y + i, 100)
SELECT list_reduce(NULL, lambda x, y, i: x + y + i, 100)
SELECT list_reduce(['Once', 'upon', 'a', 'time'], lambda x, y: x || ' ' || y, '-->')
SELECT list_reduce([], lambda x, y: x + y, 100)
SELECT list_reduce(['a', 'b', 'c'], lambda x, y: x || y, NULL)
SELECT list_apply([1,2], lambda x: list_apply([3,4], lambda y: {'x': x, 'y': y})) AS bug
SELECT list_transform([1,2], lambda x: list_transform([3,4], lambda y: x + y))
SELECT list_transform([1,2], lambda x: list_transform([3,4], lambda y: list_transform([5,6], lambda z: z + y + x)))
SELECT list_transform([1,2,3,4], lambda x: list_filter([4,5,1,2,3,3,3,5,1,4], lambda y: y != x))
SELECT list_transform([[2, 4, 6]], lambda x: list_transform(x, lambda y: list_sum([y] || x)))
SELECT list_apply(range(5), lambda x: {x:x, w:list_filter(range(5), lambda y: abs(y-x) < 2)})
SELECT list_apply(range(8), lambda x: list_aggr(list_apply(range(8), lambda y: list_element('▁▂▃▄▅▆▇█', 1+abs(y-x))), 'string_agg', ''))
CREATE TABLE lists (i integer, v varchar[])
INSERT INTO lists VALUES (1, ['a', 'b', 'c']), (8, NULL), (3, ['duck', 'db', 'tests']), (NULL, NULL), (NULL, ['lambdas!'])
SELECT list_transform(v, lambda x: list_transform(v, lambda y: x || y)) FROM lists
SELECT list_transform(v, lambda x: list_transform(v, lambda y: list_transform(v, lambda z: x || y || z))) FROM lists
SELECT list_transform(v, lambda x: [list_transform([':-)'], lambda y: x || y || '-#lambdaLove')] || list_filter(list_transform(['B-)'], lambda k: [k] || [x]), lambda j: list_contains(j, 'a') or list_contains(j, 'duck'))) FROM lists
CREATE MACRO my_transform(list) AS list_transform(list, lambda x: x * x)
CREATE MACRO my_filter(list) AS list_filter(list, lambda x: x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, lambda x, y: x + y)
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, lambda elem: list_reduce( list_transform(elem, lambda x: x + 1), lambda x, y: x + y) > 42)
SELECT my_transform([1, 2, 3])
SELECT my_filter([41, 42, NULL, 43, 44])
SELECT my_reduce([1, 2, 3])
SELECT my_nested_lambdas([[40, NULL], [20, 21], [10, 10, 20]])
CREATE TABLE tmp AS SELECT range AS id FROM range(10)
CREATE TABLE cities AS SELECT * FROM (VALUES ('Amsterdam', [90, 10]), ('London', [89, 102])) cities (name, prices)
ALTER TABLE cities ALTER COLUMN prices SET DATA TYPE INTEGER[] USING list_filter(cities.prices, lambda price: price < 100)
SELECT name, prices AS cheap_options FROM cities
SELECT SUM(list_i[1]) FROM lambda_view
SELECT lambda_macro(1, 2)
SELECT [1] AS l, list_transform([1], lambda l: l + 1)
SELECT list_transform(NULL, lambda x: x + 1)
SELECT list_transform([1], lambda x: x)
SELECT list_transform(['duck', 'a', 'ö'], lambda duck: concat(duck, 'DB'))
SELECT list_transform([1, 2, 3], lambda x: 1)
SELECT list_transform([], lambda x: x + 1)
SELECT list_transform([1, 2, 3], lambda x: x + 1)
SELECT list_transform([1, NULL, -2, NULL], lambda x: x + 1)
SELECT list_transform(l, lambda x: x) FROM lists
SELECT list_transform(l, lambda x: x + n) FROM lists
SELECT list_transform(l, lambda x: x < 2) FROM lists
SELECT list_transform(['x', 'abc', 'z'], lambda x: x || '0')
SELECT list_transform([1, 2, 3], lambda x: list_transform([4, 5, 6], lambda y, y_i: x + y + y_i))
SELECT list_transform(['abc'], lambda x, i: x[i + 1])
SELECT list_filter([1, 2, 1], lambda x, y: x >= y)
SELECT list_transform([1, 2, 3], lambda x: list_transform([4, 5, 6], lambda y: list_transform([7, 8, 9], lambda z, i: x + y + z + i)))
SELECT list_transform([10, 20, 30], lambda x, i: x + i)
SELECT list_transform([1, 2, 3, 4, 5, 6], lambda x, i: x * i)
SELECT list_transform([6, 5, 4, 3, 2, 1], lambda x, i: x * i)
SELECT list_transform([1, NULL, 3, 4, 5, 6], lambda x, i: x + i)
SELECT list_transform(NULL, lambda x, i: x + i)
SELECT list_transform(['1', '2', '3', '4'], lambda x, i: (x || ' + ' || CAST(i AS string)))
SELECT list_transform([1,2,3,4,5], lambda x, i: (x * 10 / i))
CREATE TABLE tbl(a int[])
SELECT [x for x in c if x IS NOT NULL] FROM test_vector_types(NULL::INT[]) t(c)
SELECT [x for x in c if x IS NULL] FROM test_vector_types(NULL::INT[]) t(c)
SELECT list_reduce(c, lambda x, y: x + y) FROM test_vector_types(NULL::INT[]) t(c) WHERE len(c) > 0
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
SELECT list_apply(['hello'], x -> x) FROM t1
SELECT list_transform([[1], [2], [3]], x -> x[1]) FROM t2
SELECT l, list_transform(l, l -> l + 1) FROM l_test
SELECT l, list_filter(l, l -> l > 1) FROM l_filter_test
SELECT list_transform(qualified_tbl.x, x -> (qualified_tbl.x)[1] + 1 + x) FROM qualified_tbl
SELECT list_transform([1, 2], x -> list_transform([3, 4], x -> x))
SELECT list_has_all([variable_has_all FOR variable_has_all IN ['a']], ['b']) AS list_comp_result
SELECT list_has_all(list_transform(['a'], variable_has_all -> variable_has_all), ['b']) AS list_transform_result
SELECT list_has_any(['b'], list_transform(['a'], variable_has_any -> variable_has_any)) AS list_transform_result
SELECT list_intersect(list_intersect([1], [1]), [1])
SELECT list_intersect([1], list_intersect([1], [1]))
SELECT list_has_any(LIST_VALUE(list_has_any([1], [1])), [1])
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> 0 + z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), x -> (z -> 'a')) AS row )
SELECT tag_product, list_aggr(list_transform( string_split(tag_product, ' '), word -> lower(word)), 'string_agg', ',') AS tag_material, FROM tbl GROUP BY tag_product ORDER BY ALL
SELECT 1, list_transform([5, 4, 3], x -> x + 1) AS lst GROUP BY 1
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(x -> x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, x -> x)
SELECT x FROM (VALUES (42)) t(x) GROUP BY x HAVING list_filter(NULL, lambda_param -> lambda_param = 1)
create table test as select range i from range(3)
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
SELECT [len(x) for x in words] from word_tbl
with base as ( select [4,5,6] as l ) select [x for x,i in l if i != 2] as filtered from base
select [x+i for x, i in [10, 9, 8, 7, 6]]
with base as ( select [4,5,6] as l ) select [x + 5 for x in l] as filtered from base
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
INSERT INTO t1 VALUES ([666])
INSERT INTO t1 VALUES (NULL)
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
create table no_overwrite as select [range, range + 1] l from range(3)
select l, [[{'x+y': x + y, 'x': x, 'y': y, 'l': l} for y in [42, 43]] for x in l] from no_overwrite
CREATE MACRO my_transform(list) AS list_transform(list, x -> x * x)
CREATE MACRO my_filter(list) AS list_filter(list, x -> x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, (x, y) -> x + y)
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, elem -> list_reduce(list_transform(elem, x -> x + 1), (x, y) -> x + y) > 42)
ALTER TABLE cities ALTER COLUMN prices SET DATA TYPE INTEGER[] USING list_filter(cities.prices, price -> price < 100)
CREATE TABLE varchars(v VARCHAR)
INSERT INTO varchars VALUES ('>>%Test<<'), ('%FUNCTION%'), ('Chaining')
DELETE FROM varchars
INSERT INTO varchars VALUES ('Test Function Chaining Alias')
CREATE OR REPLACE FUNCTION deprecated_syntax.transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
DETACH deprecated_syntax
SELECT [1] AS l, list_transform([1], l -> l + 1)
SELECT list_transform(NULL, x -> x + 1)
SELECT list_transform([1], x -> x)
SELECT list_transform(['duck', 'a', 'ö'], duck -> concat(duck, 'DB'))
SELECT list_transform([1, 2, 3], x -> 1)
SELECT list_transform([], x -> x + 1)
SELECT list_transform([1, 2, 3], x -> x + 1)
SELECT list_transform([1, NULL, -2, NULL], x -> x + 1)
SELECT list_transform(l, x -> x) FROM lists
SELECT list_transform(l, x -> x + n) FROM lists
SELECT list_transform(l, x -> x < 2) FROM lists
SELECT list_transform(['x', 'abc', 'z'], x -> x || '0')
SELECT list_transform([1, 2, 3], x -> list_transform([4, 5, 6], (y, y_i) -> x + y + y_i))
SELECT list_transform(['abc'], (x, i) -> x[i + 1])
SELECT list_filter([1, 2, 1], (x, y) -> x >= y)
SELECT list_transform([1, 2, 3], x -> list_transform([4, 5, 6], y -> list_transform([7, 8, 9], (z, i) -> x + y + z + i)))
SELECT list_transform([10, 20, 30], (x, i) -> x + i)
SELECT list_transform([1, 2, 3, 4, 5, 6], (x, i) -> x * i)
SELECT list_transform([6, 5, 4, 3, 2, 1], (x, i) -> x * i)
SELECT list_transform([1, NULL, 3, 4, 5, 6], (x, i) -> x + i)
SELECT list_transform(NULL, (x, i) -> x + i)
SELECT list_transform(['1', '2', '3', '4'], (x, i) -> (x || ' + ' || CAST(i AS string)))
SELECT list_transform([1,2,3,4,5], (x, i) -> (x * 10 / i))
INSERT INTO tbl VALUES ([5, 4, 3]), ([1, 2, 3]), (NULL), ([NULL, 101, 12])
CALL enable_logging(level='error')
SELECT log_level, message[0:37] FROM duckdb_logs
CALL truncate_duckdb_logs()
CALL enable_logging(level='warning')
RESET lambda_syntax
SELECT (SELECT (JSON '{ "key" : "value" }')->k AS v FROM (SELECT 'key' AS k) keys)
SELECT list_aggr([NULL, 1, 2], 'any_value')
INSERT INTO five VALUES (NULL), ([NULL]), ([]), ([NULL, 1, 2])
SELECT list_any_value(i) FROM five
DROP TABLE five
CREATE TABLE five_dates AS SELECT LIST(NULLIF(i,0)::integer) AS i, LIST('2021-08-20'::DATE + NULLIF(i,0)::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (NULLIF(i,0)) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (NULLIF(i,0)) MINUTE) AS t, LIST(INTERVAL (NULLIF(i,0)) SECOND) AS s FROM range(0, 6, 1) t1(i)
SELECT list_any_value(d), list_any_value(dt), list_any_value(t), list_any_value(s) FROM five_dates
DROP TABLE five_dates
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (NULLIF(i,0)) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (NULLIF(i,0)) MINUTE)::TIMETZ) AS t, FROM range(0, 6, 1) t1(i)
SELECT list_any_value(dt), list_any_value(t) FROM five_dates_tz
DROP TABLE five_dates_tz
CREATE TABLE five_complex AS SELECT LIST(NULLIF(i,0)::integer) AS i, LIST(NULLIF(i,0)::VARCHAR) AS s, LIST([NULLIF(i,0)]) AS l, LIST({'a': NULLIF(i,0)}) AS r FROM range(0, 6, 1) t1(i)
SELECT list_any_value(s), list_any_value(l), list_any_value(r) FROM five_complex
CREATE TABLE list_ints (l INTEGER[])
INSERT INTO list_ints SELECT LIST(i) FROM range(100) tbl(i)
select list_approx_count_distinct([10]), list_approx_count_distinct(['hello']) from list_ints
select list_approx_count_distinct(l), list_approx_count_distinct(['hello']) from list_ints
select list_approx_count_distinct([]) from list_ints
INSERT INTO list_ints VALUES ([]), (NULL), ([NULL])
select list_approx_count_distinct(l) from list_ints
CREATE TABLE IF NOT EXISTS dates (t date[])
INSERT INTO dates VALUES (['2008-01-01', NULL, '2007-01-01', '2008-02-01', '2008-01-02', '2008-01-01', '2008-01-01', '2008-01-01'])
SELECT list_count(t), list_approx_count_distinct(t) from dates
CREATE TABLE IF NOT EXISTS timestamp (t TIMESTAMP[])
INSERT INTO timestamp VALUES (['2008-01-01 00:00:01', NULL, '2007-01-01 00:00:01', '2008-02-01 00:00:01', '2008-01-02 00:00:01', '2008-01-01 10:00:00', '2008-01-01 00:10:00', '2008-01-01 00:00:10'])
SELECT list_avg([nextval('seq')])
CREATE TABLE integers(i INTEGER[])
INSERT INTO integers VALUES ([1, 2, 3]), ([6, 3, 2, 5]), ([]), ([NULL]), (NULL), ([1, NULL, 2, 3])
SELECT list_avg(i) FROM integers
CREATE TABLE vals(i INTEGER[], j HUGEINT[])
INSERT INTO vals VALUES ([NULL, NULL], [NULL, NULL, NULL])
SELECT list_avg(i), list_avg(j) FROM vals
CREATE TABLE bigints(n HUGEINT[])
INSERT INTO bigints (n) VALUES (['9007199254740992'::HUGEINT, 1::HUGEINT, 0::HUGEINT])
SELECT list_avg(n)::DOUBLE - '3002399751580331'::DOUBLE FROM bigints
CREATE TABLE doubles(n DOUBLE[])
INSERT INTO doubles (n) VALUES (['9007199254740992'::DOUBLE, 1::DOUBLE, 1::DOUBLE, 0::DOUBLE])
CREATE TABLE bigints (i BIGINT[])
INSERT INTO bigints VALUES ([1, 2, 3])
SELECT list_sum(i) FROM bigints
SELECT list_avg(i) FROM bigints
DELETE FROM bigints
INSERT INTO bigints VALUES ([1, 2, 3, 9223372036854775806])
INSERT INTO bigints VALUES ([-1, -2, -3])
INSERT INTO bigints VALUES ([-1, -2, -3, -9223372036854775806])
CREATE TABLE decimals (i DECIMAL(18,1)[])
INSERT INTO decimals VALUES ([1, 2, 3])
SELECT list_sum(i) FROM decimals
SELECT list_avg(i) FROM decimals
SELECT list_bit_and([nextval('seq')])
INSERT INTO integers VALUES ([3, 7, 15, 31, 3, 15])
SELECT list_bit_and([]) FROM integers
INSERT INTO integers VALUES ([]), (NULL), ([NULL]), ([3, 7, NULL, 15, 31, 3, 15, NULL])
SELECT list_bit_and(i), list_bit_and([1, 1, 1, 1, 1, 1]), list_bit_and(NULL) FROM integers
SELECT list_bit_or([nextval('seq')])
SELECT list_bit_or([]) FROM integers
SELECT list_bit_or(i), list_bit_or([1, 1, 1, 1, 1, 1]), list_bit_or(NULL) FROM integers
SELECT list_bit_xor([nextval('seq')])
CREATE TABLE integers (i INTEGER[])
SELECT list_bit_xor([]) FROM integers
SELECT list_bit_xor(i), list_bit_xor([1, 1, 1, 1, 1, 1]), list_bit_xor(NULL) FROM integers
CREATE TABLE bools (l BOOLEAN[])
INSERT INTO bools SELECT LIST(True) FROM range(100) tbl(i)
INSERT INTO bools SELECT LIST(False) FROM range(100) tbl(i)
INSERT INTO bools VALUES ([True, False])
INSERT INTO bools VALUES ([]), ([NULL]), (NULL), ([NULL, True, False, NULL])
SELECT list_bool_or(l) FROM bools
SELECT list_bool_and(l) FROM bools
SELECT list_count([1, 2, 3])
SELECT list_count([1]) FROM range(3)
CREATE TABLE lists (l INTEGER[])
INSERT INTO lists VALUES ([1, 2]), ([NULL]), (NULL), ([]), ([3, 4, 5, 6, 7]), ([1, 2, NULL, 1, NULL])
SELECT list_count(l) FROM lists
select list_entropy([1])
create table aggr(k int[])
insert into aggr values ([0, 1, 1, 1, 4, 0, 3, 3, 2, 2, 4, 4, 2, 4, 0, 0, 0, 1, 2, 3, 4, 2, 3, 3, 1])
insert into aggr values ([]), ([NULL]), (NULL), ([0, 1, 1, 1, 4, NULL, 0, 3, 3, 2, NULL, 2, 4, 4, 2, 4, 0, 0, 0, 1, NULL, 2, 3, 4, 2, 3, 3, 1])
select list_entropy(k) from aggr
CREATE TABLE entr (l INTEGER[])
INSERT INTO entr SELECT LIST(2) FROM range(100) tbl(i)
SELECT list_entropy(l) FROM entr
create table aggr2 (k int[])
INSERT INTO aggr2 VALUES ([0, 4, 0, 2, 2, 4, 4, 2, 4, 0, 0, 0, 2, 4, 2])
INSERT INTO aggr2 VALUES ([1, 1, 1, 3, 3, 1, 3, 3, 3, 1])
select list_entropy(k) from aggr2
SELECT list_aggr([1, 2], 'arbitrary')
SELECT list_first(i) FROM five
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
SELECT list_first(d), list_first(dt), list_first(t), list_first(s) FROM five_dates
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
SELECT list_first(dt), list_first(t) FROM five_dates_tz
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
SELECT list_first(s), list_first(l), list_first(r) FROM five_complex
DROP TABLE five_complex
CREATE TABLE const AS SELECT LIST(2) AS i FROM range(200) t1(i)
SELECT list_histogram(i) FROM const
select list_histogram([1])
CREATE TABLE hist_data (g INTEGER[])
INSERT INTO hist_data VALUES ([1, 1, 2, 2, 2, 3, 5]), ([1, 2, 3, 4, 5, 6, NULL]), ([]), (NULL), ([NULL])
SELECT list_histogram(g) from hist_data
create table names (name string[])
insert into names values (['pedro', 'pedro', 'pedro', 'hannes', 'hannes', 'mark', NULL, 'Hubert Blaine Wolfeschlegelsteinhausenbergerdorff Sr.'])
select list_histogram(name) from names
SELECT list_histogram(['2021-08-20'::TIMESTAMP])
SELECT list_histogram(['2021-08-20'::TIMESTAMP_S])
SELECT list_histogram(['2021-08-20'::TIMESTAMP_MS])
WITH cte AS (FROM (VALUES (0.0), (9.9)) df(l_orderkey)) SELECT * FROM histogram_values(cte, l_orderkey)
CREATE TABLE hugeints(h HUGEINT[])
INSERT INTO hugeints VALUES ([NULL, 1, 2]), (NULL), ([]), ([NULL]), ([1, 2, 3])
SELECT list_first(h), list_last(h), list_sum(h) FROM hugeints
DELETE FROM hugeints
INSERT INTO hugeints VALUES ([42.0, 1267650600228229401496703205376, -439847238974238975238975, '-12'])
SELECT list_min(h), list_max(h), list_sum(h), list_first(h), list_last(h) FROM hugeints
select list_kurtosis([1])
select list_kurtosis([0, 0, 0, 0, 0, 0])
insert into aggr values ([1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]), ([10, 10, 10, 10, 20, 20, 25, 30, 30, 30, 30]), ([NULL, 11, 15, 18, 22, 25, NULL, 35, 40, 50, 51]), (NULL), ([]), ([NULL])
select list_kurtosis(k) from aggr
select list_kurtosis_pop(k) from aggr
INSERT INTO five VALUES (NULL), ([NULL]), ([]), ([1, 2, NULL])
SELECT list_last(i) FROM five
SELECT list_last(d), list_last(dt), list_last(t), list_last(s) FROM five_dates
SELECT list_last(dt), list_last(t) FROM five_dates_tz
SELECT list_last(s), list_last(l), list_last(r) FROM five_complex
SELECT list_mad([1])
CREATE TABLE const AS SELECT LIST(1) AS i FROM range(2000) t1(i)
SELECT list_mad(i) FROM const
SELECT list_mad(r) FROM tinys
drop table tinys
SELECT list_mad(r) FROM numerics
drop table numerics
create table date as select list(('2018-01-01'::DATE + INTERVAL (r) DAY)::DATE) as r from range(10000) tbl(r)
SELECT list_mad(r) FROM date
create table hour as select list('2018-01-01'::TIMESTAMP + INTERVAL (r) HOUR) as r from range(10000) tbl(r)
SELECT list_mad(r) FROM hour
create table second as select list('00:00:00'::TIME + INTERVAL (r) SECOND) as r from range(10000) tbl(r)
SELECT list_max(i) FROM five
SELECT list_max(d), list_max(dt), list_max(t), list_max(s) FROM five_dates
SELECT list_max(dt), list_max(t) FROM five_dates_tz
SELECT list_max(s), list_max(l), list_max(r) FROM five_complex
SELECT list_median(r) FROM quantile
DROP TABLE quantile
CREATE TABLE quantile AS SELECT LIST(r::tinyint) AS r FROM range(100) t1(r)
CREATE TABLE range AS SELECT LIST(1) AS i FROM range(2000) t1(i)
INSERT INTO range VALUES (NULL), ([]), ([NULL])
SELECT list_median(i) FROM range
SELECT list_min(i) FROM five
SELECT list_min(d), list_min(dt), list_min(t), list_min(s) FROM five_dates
SELECT list_min(dt), list_min(t) FROM five_dates_tz
SELECT list_min(s), list_min(l), list_min(r) FROM five_complex
CREATE TABLE structs AS SELECT {'i': i} s FROM range(1000) t(i)
SELECT MIN(s), MAX(s) FROM structs
INSERT INTO structs VALUES ({'i': 99999999})
INSERT INTO structs VALUES ({'i': -9223372036854775808}), ({'i': 9223372036854775807})
INSERT INTO structs VALUES ({'i': NULL}), (NULL)
CREATE TABLE varchar_structs AS SELECT {'i': concat('long_prefix_', i)} s FROM range(1000) t(i)
SELECT MIN(s), MAX(s) FROM varchar_structs
INSERT INTO varchar_structs VALUES ({'i': chr(0)}), ({'i': 'zzzzz' || chr(0)})
SELECT MIN(s), MAX(s) FROM blob_structs
CREATE TABLE multi_member_struct AS SELECT {'i': (1000-i)//5, 'j': i} s FROM range(1000) t(i)
SELECT MIN(s), MAX(s) FROM multi_member_struct
CREATE TABLE lists AS SELECT case when i<500 then [i, i + 1, i + 2] else [i, 0] end AS l FROM range(1000) t(i)
CREATE TABLE range AS SELECT LIST(2) AS i FROM range(100) t1(i)
SELECT list_mode(i) FROM range
insert into names values (['pedro', 'pedro', 'pedro', 'hannes', 'hannes', 'mark', NULL])
select list_mode(name) from names
create table dates (v date[])
insert into dates values (['2021-05-02', '2021-05-02', '2021-05-02', '2020-02-29', '2020-02-29', '2004-09-01', NULL])
select list_mode(v) from dates
create table times (v time[])
insert into times values (['12:11:49.5', '12:11:49.5', '12:11:49.5', '06:30:00', '06:30:00', '21:15:22', NULL])
select list_mode(v) from times
create table timestamps (v timestamp[])
insert into timestamps values (['2021-05-02 12:11:49.5', '2021-05-02 12:11:49.5', '2021-05-02 12:11:49.5', '2020-02-29 06:30:00', '2020-02-29 06:30:00', '2004-09-01 21:15:22', NULL])
SELECT list_min(list_concat([1, 2], [-1]))
SELECT list_min(list_aggr([1, 2], 'list'))
CREATE TABLE lists (l1 INTEGER[], l2 INTEGER[])
INSERT INTO lists VALUES ([1, 2, 3], [4]), ([NULL, 1, -4, NULL], [NULL]), (NULL, NULL), ([NULL], [-4]), ([], [])
SELECT list_last(list_concat(l1, l2)) FROM lists
SELECT list_concat(list(list_last(l1)), list(list_first(l2))) FROM lists
SELECT array_aggregate([1, 2], 'min')
SELECT array_aggr([1, 2], 'min')
SELECT list_aggregate([1, 2], 'min')
INSERT INTO integers VALUES ([1, 2, 4]), (NULL), ([]), ([NULL]), ([1, 2, NULL, 4, NULL])
SELECT list_product(i) FROM integers
CREATE TABLE prods AS SELECT LIST(2) AS i FROM range(100) t1(i)
SELECT list_product(i) FROM prods
drop table prods
CREATE TABLE prods AS SELECT LIST(2) AS i FROM range(100 // 2) t1(i)
select list_sem([1])
create table aggr(k int[], v decimal(10,2)[], v2 decimal(10, 2)[])
insert into aggr values ([1, 2, 2, 2, 2], [10, 10, 20, 25, 30], [NULL, 11, 22, NULL, 35])
select list_sem(k), list_sem(v), list_sem(v2) from aggr
create table sems (l int[])
insert into sems values ([1, 2, 2, 2, 2]), ([1, 2, NULL, 2, 2, NULL, 2]), ([]), ([NULL]), (NULL)
select list_sem(l) from sems
select list_skewness([1])
CREATE TABLE skew AS SELECT LIST(10) AS i FROM range(5) t1(i)
select list_skewness (i) from skew
select list_skewness ([1,2])
insert into aggr values ([1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2], [10, 10, 10, 10, 20, 20, 25, 30, 30, 30, 30], [NULL, 11, 15, 18, 22, 25, NULL, 35, 40, 50, 51]), ([], NULL, [NULL])
select list_skewness(k), list_skewness(v), list_skewness(v2) from aggr
create table aggr2(v2 decimal(10, 2)[])
insert into aggr2 values ([NULL, 11, 15, 18]), ([22, 25]), ([NULL]), ([35, 40, 50, 51])
select list_skewness(v2) from aggr2
SELECT list_string_agg(['a', ','])
SELECT list_string_agg(['a'])
CREATE TABLE str_aggs (str varchar[])
INSERT INTO str_aggs VALUES (['a', ',']), ([NULL, ',']), (['a', NULL]), ([NULL, NULL]), (NULL), ([]), ([NULL])
SELECT list_string_agg(str) from str_aggs
CREATE TABLE strings(g INTEGER[], x VARCHAR[], y VARCHAR[])
INSERT INTO strings VALUES ([1, 1, 2, 2, 2, 3, 4, 4, 4], ['a', 'b', 'i', NULL, 'j', 'p', 'x', 'y', 'z'], ['/', '-', '/', '-', '+', '/', '/', '-', '+'])
SELECT list_string_agg(x), list_string_agg(y), list_string_agg(g::varchar[]) FROM strings
SELECT list_string_agg(x) FROM strings WHERE g > [100]
SELECT list_string_agg([1, 2])
SELECT list_string_agg([1, 2]::varchar[])
SELECT list_aggr(['a'], 'group_concat')
SELECT list_sum([2, 2])
INSERT INTO integers SELECT LIST(i) FROM range(0, 1000, 1) tbl(i)
INSERT INTO integers SELECT LIST(i) FROM range(-999, 1000, 1) tbl(i)
INSERT INTO integers SELECT LIST(i) FROM range(0, -1000, -1) tbl(i)
INSERT INTO integers VALUES (NULL), ([NULL]), ([])
SELECT list_sum(i) FROM integers
SELECT list_aggr(n, 'fsum') FROM doubles
SELECT list_aggr(n, 'sumKahan') FROM doubles
SELECT list_aggr(n, 'kahan_sum') FROM doubles
CREATE TABLE bigints(i BIGINT[])
INSERT INTO bigints SELECT LIST(i) FROM range(4611686018427387904, 4611686018427388904, 1) tbl(i)
INSERT INTO integers SELECT CASE WHEN i%3=0 THEN NULL ELSE i END i FROM range(10000) t(i)
SELECT SUM(i > 500), SUM(i=1), SUM(i IS NULL) FROM integers
SELECT COUNTIF(i > 500), COUNT_IF(i=1), COUNTIF(i IS NULL) FROM integers
CREATE TABLE uhugeints(h UHUGEINT[])
INSERT INTO uhugeints VALUES ([NULL, 1, 2]), (NULL), ([]), ([NULL]), ([1, 2, 3])
SELECT list_first(h), list_last(h), list_sum(h) FROM uhugeints
DELETE FROM uhugeints
INSERT INTO uhugeints VALUES ([42.0, 1267650600228229401496703205376, 0, '1'])
SELECT list_min(h), list_max(h), list_sum(h), list_first(h), list_last(h) FROM uhugeints
create table stddev_test(val integer[])
insert into stddev_test values ([42, 43, 42, 1000, NULL, NULL]), ([1, 1, 2, 2, 1, 3]), ([]), ([NULL]), (NULL)
SELECT list_stddev_samp([1])
SELECT list_var_samp([1])
select round(list_stddev_samp(val), 1) from stddev_test
select list_sum(val), round(list_stddev_samp(val), 1), list_min(val) from stddev_test
select round(list_stddev_pop(val), 1) from stddev_test
select list_sum(val), round(list_stddev_pop(val), 1), list_min(val) from stddev_test
select round(list_var_samp(val), 1) from stddev_test
select round(list_aggr(val, 'variance'), 1) from stddev_test
select list_sum(val), round(list_var_samp(val), 1), list_min(val) from stddev_test
select round(list_var_pop(val), 1) from stddev_test
select variant_extract({'a': 1234}::VARIANT, 'a')::VARCHAR
CREATE MACRO struct_cast_data() AS TABLE ( SELECT {'a': [ { 'b': 'hello', 'c': NULL, 'a': '1970/03/15'::DATE }, { 'b': NULL, 'c': True, 'a': '2020/11/03'::DATE } ]}::VARIANT AS a UNION ALL SELECT {'a': [ { 'b': 'this is a long string', 'c': False, 'a': '1953/9/16'::DATE } ]}::VARIANT )
select variant_extract(a, 'a[1].c') from struct_cast_data()
select variant_extract(a, 'a').variant_extract(1::UINTEGER).variant_extract('c') from struct_cast_data()
select ('{"a": 42, "b": [true, "test", true]}'::JSON::VARIANT).b[2]
select ('{"a": 42, "b": [true, "test", true]}'::JSON::VARIANT)['b'][2]
select ('{"a": 42, "b": [true, "test", true]}'::JSON::VARIANT)['b']['1']
select ('{"a": 42, "b": [true, "test", true]}'::JSON::VARIANT)['b[2]']
SELECT [ x.a for x in [ {'a': 42, 'b': [1,2,3]}::VARIANT, NULL, 84 ] ]
SELECT variant_extract((SELECT list(i) FROM range(500) t(i))::VARIANT, 300::UINTEGER)::BIGINT
insert into tbl SELECT * FROM UNNEST([ {'almost_a_number': c, 'a_number': CAST(TRY_CAST(c AS INT) AS VARCHAR)} for c in ['12', '24', '25a6', '24c', '16'] ])
from tbl order by all
select col.almost_a_number from tbl order by all
select TRY_CAST(col.almost_a_number AS BIGINT) from tbl order by all
select col.a_number from tbl order by all
select col.a_number::BIGINT from tbl order by all
set explain_output='optimized_only'
EXPLAIN select TRY_CAST(col.almost_a_number AS BIGINT) from tbl order by all
create table list_variant as select { 'id': l_orderkey, 'my_list': [ l_orderkey, l_orderkey + 1, l_orderkey + 2 ], 'my_struct': { 'a': l_orderkey, 'b': l_orderkey + 30 } }::variant as list_variant from range(5) t(l_orderkey)
select list_variant.my_list from list_variant limit 1
select list_variant.my_list::BIGINT[] from list_variant limit 1
select list_variant.my_struct from list_variant limit 1
select list_variant.my_struct::STRUCT(b BIGINT, a BIGINT) from list_variant limit 1
select (list_variant.my_struct::STRUCT(b VARCHAR, a VARCHAR)).b[1] from list_variant limit 1
select variant_typeof({'a': 42}::VARIANT)
select variant_typeof(({'a': 42}::VARIANT).variant_extract('a'))
select variant_typeof(struct_pack(*COLUMNS(*))::VARIANT) test from test_all_types()
CREATE TABLE T (v VARIANT)
select variant_typeof(variant_extract(test, 'bool')) from all_types
select variant_typeof(variant_extract(test, 'struct')) from all_types
select variant_typeof(variant_extract(test, 'struct').a) from all_types
select variant_typeof(variant_extract(test, 'struct').variant_extract('a')) from all_types limit 2
select variant_typeof(variant_extract(test, 'dec_18_6')) from all_types
select variant_typeof(variant_extract(test, 'array_of_structs')) from all_types
with cte as ( select * from all_types limit 1 offset 1 ) select variant_typeof(variant_extract(test, 'array_of_structs')[1]), variant_typeof(variant_extract(test, 'array_of_structs')[2]), variant_typeof(variant_extract(test, 'array_of_structs')[3]) from cte
CREATE TABLE tbl AS SELECT * FROM range(10) tbl(i)
SELECT can_cast_implicitly(i, NULL::BIGINT) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::HUGEINT) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::INTEGER) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::VARCHAR) FROM tbl LIMIT 1
SELECT * FROM tbl WHERE CASE WHEN i%2=0 THEN 1 ELSE 0 END AND CASE WHEN i<5 THEN 1 ELSE 0 END
create table t (n text)
insert into t values ('1'),('0'),('')
select n, case when n <> '' and cast(substr(n, 1, 1) as int) <= 0 then '0' when n <> '' and cast(substr(n, 1, 1) as int) > 0 then '1' else '2'end as x from t ORDER BY n
CREATE TABLE tbl AS SELECT i, 'thisisalongstring' || i::VARCHAR s FROM range(10) tbl(i)
SELECT i, s, CASE WHEN i%2=0 THEN s ELSE s END FROM tbl
SELECT i, s, CASE WHEN i%2=0 THEN s ELSE s END FROM (SELECT i, s||'_suffix' FROM tbl) tbl(i, s)
SELECT cast_to_type(' 42', NULL::INT)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN cast_to_type(nullif(trim(s::VARCHAR), ''), s) ELSE s END
SELECT try_trim_null(42) as trim_int, try_trim_null(' col ') as trim_varchar, try_trim_null('') as trim_empty
create table tbl(i int, v varchar)
insert into tbl values (42, ' hello '), (100, ' ')
SELECT try_trim_null(COLUMNS(*)) FROM tbl
PREPARE v1 AS SELECT cast_to_type(' 42', ?)
EXECUTE v1(NULL::INT)
EXECUTE v1(NULL::VARCHAR)
SELECT constant_or_null(1, NULL), constant_or_null(1, 10)
SELECT constant_or_null(1, case when i%2=0 then null else i end) from range(5) tbl(i)
SELECT constant_or_null(1, case when i%2=0 then null else i end, case when i%2=1 then null else i end) from range(5) tbl(i)
SELECT * FROM (SELECT 4 AS x) WHERE IF(x % 2 = 0, true, ERROR(FORMAT('x must be even number but is {}', x)))
CREATE TABLE structs AS SELECT * FROM (VALUES ({'i': 5, 's': 'string'}), ({'i': -2, 's': NULL}), ({'i': NULL, 's': 'not null'}), ({'i': NULL, 's': NULL}), (NULL) ) tbl(s)
SELECT s, HASH(s) FROM structs
CREATE TABLE lists AS SELECT * FROM (VALUES ([1], ['TGTA']), ([1, 2], ['CGGT']), ([], ['CCTC']), ([1, 2, 3], ['TCTA']), ([1, 2, 3, 4, 5], ['AGGG']), (NULL, NULL) ) tbl(li, lg)
SELECT li, HASH(li) FROM lists
SELECT lg, HASH(lg) FROM lists
CREATE TABLE maps AS SELECT * FROM (VALUES (MAP([1], ['TGTA'])), (MAP([1, 2], ['CGGT', 'CCTC'])), (MAP([], [])), (MAP([1, 2, 3], ['TCTA', NULL, 'CGGT'])), (MAP([1, 2, 3, 4, 5], ['TGTA', 'CGGT', 'CCTC', 'TCTA', 'AGGG'])), (NULL) ) tbl(m)
SELECT m, HASH(m) FROM maps
CREATE TABLE map_as_list AS SELECT * FROM (VALUES ([{'key':1, 'value':'TGTA'}]), ([{'key':1, 'value':'CGGT'}, {'key':2, 'value':'CCTC'}]), ([]), ([{'key':1, 'value':'TCTA'}, {'key':2, 'value':NULL}, {'key':3, 'value':'CGGT'}]), ([{'key':1, 'value':'TGTA'}, {'key':2, 'value':'CGGT'}, {'key':3, 'value':'CCTC'}, {'key':4, 'value':'TCTA'}, {'key':5, 'value':'AGGG'}]), (NULL) ) tbl(m)
SELECT HASH(m) FROM maps
SELECT HASH(m) FROM map_as_list
CREATE TYPE resistor AS ENUM ( 'black', 'brown', 'red', 'orange', 'yellow', 'green', 'blue', 'violet', 'grey', 'white' )
CREATE TABLE enums (r resistor)
CREATE TYPE t AS ENUM ('z','y','x')
SELECT greatest('x'::t, 'z'::t), 'x'::t > 'z'::t
CREATE TABLE all_types AS FROM test_all_types()
SELECT replace_type(' 42', NULL::VARCHAR, NULL::INT)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN replace_type(nullif(trim(s::VARCHAR), ''), NULL::VARCHAR, s) ELSE s END
PREPARE v1 AS SELECT replace_type(' 42', NULL::VARCHAR, ?)
select replace_type({duck: 3.141592653589793::DOUBLE, goose: 2.718281828459045::DOUBLE}, NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type(map {'duck': 3.141592653589793::DOUBLE, 'goose': 2.718281828459045::DOUBLE}, NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type([3.141592653589793, 2.718281828459045]::DOUBLE[], NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type([3.141592653589793, 2.718281828459045]::DOUBLE[2], NULL::DOUBLE, NULL::DECIMAL(15,2))
SELECT * FROM repeat_row(1, 2, 'foo', num_rows=3)
SELECT approx_count >= 3 FROM duckdb_approx_database_count()
SELECT 10 BETWEEN 10 AND 20
SELECT 9 BETWEEN 10 AND 20
SELECT 10 BETWEEN NULL AND 20
SELECT 30 BETWEEN NULL AND 20
SELECT 10 BETWEEN 10 AND NULL
SELECT 9 BETWEEN 10 AND NULL
SELECT NULL BETWEEN 10 AND 20
SELECT NULL BETWEEN NULL AND 20
SELECT NULL BETWEEN 10 AND NULL
SELECT NULL BETWEEN NULL AND NULL
INSERT INTO integers VALUES (1), (2), (3), (NULL)
SELECT i BETWEEN 1 AND 2 FROM integers ORDER BY i
PREPARE v1 AS SELECT ? BETWEEN 1 AND 2
EXECUTE v1(1)
EXECUTE v1(3)
PREPARE v2 AS SELECT 1 WHERE ? BETWEEN now() - INTERVAL '1 minute' AND now() + INTERVAL '1 minute'
EXECUTE v2(now())
EXECUTE v2(now() - INTERVAL '10 minute')
SELECT (RANDOM() * 10)::INT BETWEEN 6 AND 5
SELECT (RANDOM() * 10)::INT NOT BETWEEN 6 AND 5
select true is true
select false is true
select null is true
select 42 is true
select 0 is true
select true is not true
select false is not true
select null is not true
select 42 is not true
select 0 is not true
select null is null
select 42 is null
SELECT count FROM duckdb_connection_count()
SELECT IF(true, 1, 10), IF(false, 1, 10), IF(NULL, 1, 10)
SELECT IF(true, 20, 2000), IF(false, 20, 2000), IF(NULL, 20, 2000)
SELECT IF(true, 20.5, 2000), IF(false, 20, 2000.5), IF(NULL, 20, 2000.5)
SELECT IF(true, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp), IF(false, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp), IF(NULL, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp)
SELECT IF(true, 'true', 'false'), IF(false, 'true', 'false'), IF(NULL, 'true', 'false')
SELECT IFNULL(NULL, NULL), IFNULL(NULL, 10), IFNULL(1, 10)
SELECT IFNULL(NULL, 2000), IFNULL(20.5, 2000)
SELECT IFNULL(NULL, '1996-11-05 10:11:56'::timestamp), IFNULL('2020-05-05'::date, '1996-11-05 10:11:56'::timestamp)
SELECT IFNULL(NULL, 'not NULL'), IFNULL('NULL', 'not NULL')
SELECT * FROM integers WHERE i IN (1, 2) ORDER BY i
SELECT * FROM integers WHERE i IN (1, 2, 3, 4, 5, 6, 7, 8) ORDER BY i
SELECT i, i IN (1, 2, 3, 4, 5, 6, 7, 8) FROM integers ORDER BY i
SELECT i, i NOT IN (1, 3, 4, 5, 6, 7, 8) FROM integers ORDER BY i
SELECT i, i IN (1, 2, NULL, 4, 5, 6, 7, 8) FROM integers ORDER BY i
SELECT i, i IN (i + 1) FROM integers ORDER BY i
SELECT i, i IN (i + 1, 42, i) FROM integers ORDER BY i
SELECT i, 1 IN (i - 1, i, i + 1) FROM integers ORDER BY i
SELECT i, 1 NOT IN (i - 1, i, i + 1) FROM integers ORDER BY i
SELECT i, i IN (11, 12, 13, 14, 15, 16, 17, 18, 1, i) FROM integers ORDER BY i
SELECT i, i NOT IN (11, 12, 13, 14, 15, 16, 17, 18, 1, i) FROM integers ORDER BY i
SELECT i, 1 IN (11, 12, 13, 14, 15, 16, 17, 18, 1, i) FROM integers ORDER BY i
SELECT LEAST(1)
SELECT LEAST('hello world')
SELECT LEAST(1, 3)
SELECT LEAST(1, 3, 0)
SELECT LEAST(1, 3, 0, 2, 7, 8, 10, 11, -100, 30)
SELECT LEAST(1, 3, 0, 2, 7, 8, 10, 11, -100, 30, NULL)
SELECT LEAST(NULL, 3, 0, 2, 7, 8, 10, 11, -100, 30, 1)
SELECT GREATEST(NULL, 1.0::FLOAT)
SELECT LEAST(1.0, 10.0)
SELECT LEAST('hello', 'world')
SELECT LEAST('hello', 'world', 'blabla', 'tree')
SELECT LEAST(DATE '1992-01-01', DATE '1994-02-02', DATE '1991-01-01')
SELECT NULLIF(NULLIF ('hello', 'world'), 'blabla')
CREATE TABLE test (a STRING)
INSERT INTO test VALUES ('hello'), ('world'), ('test')
CREATE TABLE test2 (a STRING, b STRING)
INSERT INTO test2 VALUES ('blabla', 'b'), ('blabla2', 'c'), ('blabla3', 'd')
SELECT NULLIF(NULLIF ((SELECT a FROM test LIMIT 1 offset 1), a), b) FROM test2
INSERT INTO test3 VALUES (11, 22), (13, 22), (12, 21)
SELECT NULLIF(CAST(a AS VARCHAR), '11') FROM test3
SELECT a, CASE WHEN a>11 THEN CAST(a AS VARCHAR) ELSE CAST(b AS VARCHAR) END FROM test3 ORDER BY 1
SELECT CURRENT_SETTING('default_null_order')
SET default_null_order = 'nulls_last'
SET default_null_order = concat('nulls', '_', 'last')
SELECT CURRENT_SETTING('DEFAULT_NULL_ORDER')
SELECT * FROM range(3) UNION ALL SELECT NULL ORDER BY 1
SELECT sleep_ms(10)
SELECT sleep_ms(0)
SELECT sleep_ms(NULL)
SELECT 1, sleep_ms(50), 2
SELECT sleep_ms(100)
SELECT 42 WHERE sleep_ms(10) IS NULL
SELECT sleep_ms(10) FROM range(3)
SELECT sleep_ms(10 + 20)
SELECT sleep_ms(-10)
CREATE TABLE test_sleep_table AS SELECT * FROM range(5) tbl(id)
SELECT id, sleep_ms(10) FROM test_sleep_table ORDER BY id
SELECT id, sleep_ms(20) FROM test_sleep_table WHERE id < 3 ORDER BY id
select 1=1
SELECT STATS(5)
SELECT STATS(7)
SELECT STATS('hello')
SELECT STATS('1234567ü')
SELECT STATS(5+2)
SELECT STATS(i) FROM integers LIMIT 1
SELECT STATS(i+2) FROM integers LIMIT 1
SELECT STATS(i-5) FROM integers LIMIT 1
SELECT STATS(i*2) FROM integers LIMIT 1
SELECT STATS(i*-1) FROM integers LIMIT 1
SELECT STATS(i+1) FROM integers LIMIT 1
create table a (i double, j double)
insert into a values (1, 10), (42, 420)
EXPLAIN SELECT * FROM summary((SELECT * FROM a))
SELECT * FROM summary((SELECT * FROM a))
SELECT format_bytes(0)
SELECT format_bytes(1)
SELECT format_bytes(1023)
SELECT format_bytes(1024)
SELECT pg_size_pretty(1024)
SELECT format_bytes(1024*1024-1)
SELECT format_bytes(1024*1024)
SELECT format_bytes(1024*1024 + 555555)
SELECT format_bytes(1024*1024*1024-1)
SELECT format_bytes(1e9::BIGINT)
SELECT format_bytes(pow(1024,3)::BIGINT)
SELECT format_bytes(pow(1024.0,4)::BIGINT)
SELECT to_hex('duckdb')
SELECT hex(unhex('abcd'))
SELECT from_hex('6475636B6462')
SELECT from_hex('5')
SELECT unhex(hex('duckdb'))
SELECT to_hex(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_hex(to_hex(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
SELECT to_binary('duckdb')
SELECT from_binary('011001000111010101100011011010110110010001100010')
SELECT to_binary(columns('^(.*int|varchar|bignum)$')) FROM test_all_types()
SELECT from_binary(to_binary(columns('^(.*int|varchar|bignum)$'))) FROM test_all_types()
CREATE TABLE t0 (c0 VARCHAR)
INSERT INTO t0 VALUES ('票'),('t'),('%'),('丑'),('多'), ('🦆')
SELECT count(*) FROM t0 WHERE t0.c0 LIKE '_'
SELECT count(*) FROM t0 WHERE t0.c0 ILIKE '_'
SELECT '🦆a🦆' LIKE '_a_'
SELECT '🦆a🦆' ILIKE '_A_'
SELECT 'BaB' ILIKE '_A_'
SELECT '🦆🦆' ILIKE '_'
SELECT '🦆🦆' ILIKE '__'
SELECT '🦆🦆' ILIKE '___'
select md5('hello'), md5(NULL)
select md5_number('hello'), md5_number_upper(NULL)
select md5_number_upper('hello'), md5_number_upper(NULL)
select md5_number_lower('hello'), md5_number_lower(NULL)
CREATE TABLE strings AS SELECT s::VARCHAR s FROM generate_series(0,10,1) t(s)
select md5(s), md5('1') from strings ORDER BY s
select md5(s), md5('1') from strings where s::INTEGER BETWEEN 1 AND 3 ORDER BY s
SELECT chr(0)
SELECT chr(0)::blob
select ascii(chr(0))
CREATE TABLE null_byte AS SELECT concat('goo', chr(0), 'se') AS v
SELECT * FROM null_byte
SELECT * FROM null_byte WHERE contains(v, chr(0))
SELECT instr(v, chr(0)) FROM null_byte
SELECT * FROM null_byte WHERE v LIKE concat('%', chr(0), '%')
SELECT * FROM null_byte WHERE regexp_matches(v, chr(0))
SELECT * FROM null_byte WHERE regexp_full_match(v, concat('goo', chr(0), 'se'))
SELECT {'a': v} FROM null_byte
SELECT [v] FROM null_byte
SELECT parse_formatted_bytes('0 B')
SELECT parse_formatted_bytes('1 byte')
SELECT parse_formatted_bytes('2 bytes')
SELECT parse_formatted_bytes('1 b')
SELECT parse_formatted_bytes('1 KB')
SELECT parse_formatted_bytes('1.5 KB')
SELECT parse_formatted_bytes('2 MB')
SELECT parse_formatted_bytes('1 GB')
SELECT parse_formatted_bytes('1 TB')
SELECT parse_formatted_bytes('1 KiB')
SELECT parse_formatted_bytes('1.5 KiB')
SELECT parse_formatted_bytes('2 MiB')
SELECT * FROM (VALUES (parse_path('path/to/file.csv', 'system')), (parse_path('path/to/file.csv\file2.csv', 'both_slash')), (parse_path('path/to/file.csv', 'forward_slash')), (parse_path('path\to\file.csv/file2.csv', 'backslash'))) tbl(i)
SELECT parse_path('path/to/file.csv\file2.csv')
SELECT parse_path('file.csv', 'both_slash')
SELECT parse_path('/path/to/file.csv', 'forward_slash')
select parse_path('\path\to\file', 'forward_slash')
SELECT parse_path('//path/to//file.csv', 'forward_slash')
SELECT parse_path('p@th\t0\wh@t3ve%\42/12 ch,ars.sth', 'both_slash')
SELECT parse_path('path/to/file.csv','@')
SELECT parse_path('path/to/file.csv', NULL)
SELECT parse_path(NULL, NULL)
SELECT parse_path(NULL, '')
SELECT parse_path('')
SELECT * FROM (VALUES (parse_path('path\to\file.csv/file2.csv', 'system')), (parse_path('path/to/file.csv\file2.csv', 'both_slash')), (parse_path('path/to/file.csv', 'forward_slash')), (parse_path('path\to\file.csv/file2.csv', 'backslash'))) tbl(i)
SELECT parse_path('home/user/documents/file.csv\file2.csv')
SELECT parse_path('//path/to///file.csv', 'forward_slash')
SELECT * FROM (VALUES (parse_dirname('path\to\file.csv/file2.csv', 'system')), (parse_dirname('path/to/file.csv\file2.csv', 'both_slash')), (parse_dirname('path/to/file.csv', 'forward_slash')), (parse_dirname('path\to\file.csv/file2.csv', 'backslash'))) tbl(i)
SELECT parse_dirname('path/to/file.csv\file2.csv')
SELECT parse_dirname('///path/to//file.csv', 'forward_slash')
select parse_dirname('file.csv')
SELECT parse_dirname('')
SELECT * FROM (VALUES (parse_dirpath('path\to\file.csv/file2.csv', 'system')), (parse_dirpath('path/to/file.csv\file2.csv', 'both_slash')), (parse_dirpath('path/to/file.csv', 'forward_slash')), (parse_dirpath('path\to\file.csv/file2.csv', 'backslash'))) tbl(i)
SELECT parse_dirpath('path/to/file.csv\file2.csv')
SELECT parse_dirpath('///path/to//file.csv', 'forward_slash')
select parse_dirpath('file.csv')
CREATE TABLE filenames (filename VARCHAR)
INSERT INTO filenames VALUES ('rundate_2023-01-01_pass_1'), ('rundate_2023-01-01_pass_2'), ('rundate_2023-01-01_pass_3'), ('rundate_2023-01-10_pass_1'), ('rundate_2023-01-10_pass_2'), ('rundate_2023-02-14_pass_1'), ('invalid'), (NULL)
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.pass::SMALLINT AS PASS, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(1000) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.pass::SMALLINT AS PASS, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_([a-z]+?)_(\d+)', ['rundate', 'opt', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.opt AS opt, groups.pass::SMALLINT AS pass, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_([0-9]+?)_(\d+)', ['rundate', 'opt', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.opt AS opt, groups.pass::SMALLINT AS pass, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
SELECT regexp_escape('https://duckdb.org')
SELECT regexp_escape('abc123ABC')
SELECT regexp_escape('a.b[c]*')
SELECT regexp_escape('a b c')
SELECT regexp_escape('\n')
SELECT regexp_escape('line1\nline2')
SELECT regexp_escape('@')
SELECT regexp_escape('path\to\wonderland')
SELECT regexp_escape('$()*+.?[\]^{|}-')
CREATE TABLE tbl (c VARCHAR(255))
INSERT INTO tbl SELECT 'a)*.?[\]b^{2.+_c' FROM generate_series(1, 500)
INSERT INTO tbl(c) SELECT '1?ch@racter$' FROM generate_series(1, 500)
SELECT regexp_extract('foobarbaz', 'b..')
SELECT regexp_extract('foobarbaz', 'B..')
SELECT regexp_extract('foobarbaz', 'B..', 0, 'i')
SELECT regexp_extract('foobarbaz', 'b..', 1)
SELECT regexp_extract('foobarbaz', '(b..)(b..)')
SELECT regexp_extract('foobarbaz', '(b..)(b..)', 1)
SELECT regexp_extract('foobarbaz', '(b..)(b..)', 2)
CREATE TABLE test (s VARCHAR, p VARCHAR, i INT)
INSERT INTO test VALUES ('foobarbaz', 'b..', 0), ('foobarbaz', 'b..', 1), ('foobarbaz', '(b..)(b..)', 0), ('foobarbaz', '(b..)(b..)', 1), ('foobarbaz', '(b..)(b..)', 2)
SELECT regexp_extract(s, p, 0) FROM test
SELECT regexp_extract(s, 'b..', 0) FROM test
SELECT regexp_extract('foobarbaz', NULL, 0)
SELECT regexp_extract_all('1a 2b 14m', '(\d+)', 1)
SELECT regexp_extract_all('1a 2b 14m', '(\d+)([a-z]+)', 2)
SELECT REGEXP_EXTRACT_ALL('test', '.')
SELECT regexp_extract_all('1a 2b 14m', '(\\d+)([a-z]+)', -1)
SELECT regexp_extract_all('1a 2b 14m', '\\d+')
SELECT regexp_extract_all('1a 2b 14m', '\\d+', 0)
SELECT regexp_extract_all('1a 2b 14m', '\\d+', 1)
SELECT regexp_extract_all('1a 2b 14m', '\\d+', 2)
SELECT regexp_extract_all('1a 2b 14m', '\\d+', -1)
SELECT regexp_extract_all('1a 2b 14m', '(\\d+)?', 1)
SELECT regexp_extract_all('a 2b 14m', '(\\d+)?', 1)
SELECT regexp_extract_all('1a 2b 14m', '(\\d+)([a-z]+)')
SELECT regexp_extract_all('Peter:33 Paul:14', '(\w+):(\d+)', ['name','num']) AS res
SELECT regexp_extract_all('a1 b2 c', '(\w)(\d)?', ['c','d']) AS res
SELECT regexp_extract_all('Aa aA', '(a)', ['lower'], 'i') AS res
SELECT regexp_extract_all(NULL, '(a)', ['g'])
SELECT regexp_extract_all('щцф', '(.)', ['ch']) AS res
SELECT regexp_extract_all('aa', '(a)(b)?', ['a','b']) AS res
SELECT regexp_extract_all('ab ac a', '(a)(b)?(c)?', ['a','b','c']) AS res
SELECT regexp_extract_all('x1 y2', '(\w)(\d)', ['digit','letter']) AS res
SELECT regexp_extract_all('aaaaa', '(a)', ['g']) AS res
SELECT regexp_extract_all('щц', '(щ)(ц)?', ['first','second']) AS res
SELECT regexp_extract_all('hi', '(h|i)?', ['ch']) AS res
SELECT regexp_extract_all(NULL, '(a)', ['g']) AS res
CREATE TABLE regex(s STRING)
INSERT INTO regex VALUES ('asdf'), ('xxxx'), ('aaaa')
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f')
SELECT s FROM regex WHERE NOT REGEXP_MATCHES(s, 'as(c|d|e)f')
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f') AND s = 'asdf'
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f') AND REGEXP_MATCHES(s, 'as[a-z]f')
SELECT regexp_replace('foobarbaz', 'b..', 'X')
SELECT regexp_replace('ana ana', 'ana', 'banana', 'g')
SELECT regexp_replace('ANA ana', 'ana', 'banana', 'gi')
SELECT regexp_replace('ana', 'ana', 'banana', 'c')
SELECT regexp_replace('ANA', 'ana', 'banana', 'i')
SELECT regexp_replace('as^/$df', '^/$', '', 'l')
SELECT regexp_replace('as^/$df', '^/$', '')
SELECT regexp_replace('hello world', '.*', 'x', 'sg')
SELECT COUNT(*) FROM (SELECT 'x x') t1(a) JOIN (SELECT regexp_replace('hello world', '.*', 'x', 'ng')) t2(a) USING (a)
CREATE TABLE test(v VARCHAR)
INSERT INTO test VALUES ('hello'), ('HELLO')
SELECT regexp_replace(v, 'h.*', 'world', 'i') FROM test ORDER BY v
CREATE TABLE t0 as FROM VALUES('asdf') t(c0)
SELECT regexp_matches(c0, NULL) from t0
SELECT regexp_matches(c0, '.*sd.*') from t0
SELECT regexp_matches(c0, '.*yu.*') from t0
SELECT regexp_matches(c0, '') from t0
SELECT regexp_matches(c0, 'sd') from t0
SELECT regexp_full_match(c0, 'sd') from t0
SELECT regexp_full_match(c0, '.sd.') from t0
SELECT regexp_matches(c0, '^sdf$') from t0
SELECT regexp_matches('', '.*yu.*')
SELECT regexp_matches('', '.*')
SELECT regexp_matches(c0, CAST(NULL AS STRING)) from t0
SELECT regexp_split_to_table('a b c', ' ')
SELECT regexp_split_to_table('axbyc', '[x|y]')
SELECT regexp_split_to_table('axbyc', '[x|y]'), 42
CREATE TABLE data(wsc INT, zipcode VARCHAR)
INSERT INTO data VALUES (32, '00' || chr(32) || '001'), (160, '00' || chr(160) || '001'), (0, '00🦆001')
SELECT sha1('hello'), sha1(NULL)
SELECT sha1('')
SELECT sha1(s), sha1('1') FROM strings ORDER BY s
SELECT sha1(s), sha1('1') FROM strings WHERE s::INTEGER BETWEEN 1 AND 3 ORDER BY s
SELECT sha1(''::blob)
SELECT sha256('hello'), sha256(NULL)
SELECT sha256('')
SELECT sha256(s), sha256('1') FROM strings ORDER BY s
SELECT sha256(s), sha256('1') FROM strings WHERE s::INTEGER BETWEEN 1 AND 3 ORDER BY s
SELECT strip_accents('hello'), strip_accents('héllo')
SELECT strip_accents('mühleisen'), strip_accents('hannes mühleisen')
CREATE TABLE collate_test(s VARCHAR, str VARCHAR)
INSERT INTO collate_test VALUES ('äää', 'aaa')
INSERT INTO collate_test VALUES ('hännës mühlëïsën', 'hannes muhleisen')
INSERT INTO collate_test VALUES ('olá', 'ola')
INSERT INTO collate_test VALUES ('ôâêóáëòõç', 'oaeoaeooc')
SELECT strip_accents(s)=strip_accents(str) FROM collate_test
CREATE TABLE strings(s VARCHAR, off INTEGER)
INSERT INTO strings VALUES ('hello', 1), ('world', 2), ('b', 1), (NULL, 2)
SELECT array_extract('🦆ab', 4), array_extract('abc', 4)
SELECT array_extract(s, 2) FROM strings
SELECT array_extract(s, 3) FROM strings
SELECT array_extract(s, off) FROM strings
SELECT array_extract('hello', off) FROM strings
SELECT array_extract(NULL::VARCHAR, off) FROM strings
SELECT array_extract('hello', NULL) FROM strings
SELECT array_extract(NULL::VARCHAR, NULL) FROM strings
SELECT array_extract(s, -1) FROM strings
SELECT array_extract(s, 1) FROM strings
SELECT ascii('x')
SELECT ASCII('a')
SELECT ASCII('ABC')
SELECT ASCII('Ω')
SELECT ASCII('ΩΩ')
SELECT ASCII('Ä')
SELECT ASCII('5')
SELECT ASCII(NULL)
SELECT CHR(97)
SELECT CHR(196)
SELECT CHR(937)
SELECT CHR(NULL)
select bar(x * x, 0, 100) from range(0, 11) t(x)
select bar(9, 10, 20)
select bar(120, -10, 100, 10)
select bar(40, 20, 0)
select bar(100, 200, 0)
select bar(-10, 20, 0)
select bar('nan'::double, 0, 10)
select bar('infinity'::double, 0, 10)
select bar('-infinity'::double, 0, 10)
select bar(null, 0, 10)
select bar(1, 'nan'::double, 10)
select bar(1, 'infinity'::double, 10)
select BIT_LENGTH(NULL), BIT_LENGTH(''), BIT_LENGTH('$'), BIT_LENGTH('¢'), BIT_LENGTH('€'), BIT_LENGTH('𐍈')
CREATE TABLE strings(a STRING, b STRING)
INSERT INTO strings VALUES ('', 'Zero'), ('$', NULL), ('¢','Two'), ('€', NULL), ('𐍈','Four')
select BIT_LENGTH(a) FROM strings
select BIT_LENGTH(b) FROM strings
select BIT_LENGTH(a) FROM strings WHERE b IS NOT NULL
select UPPER('áaaá'), UPPER('ö'), LOWER('S̈'), UPPER('ω')
SELECT UPPER('Αα Ββ Γγ Δδ Εε Ζζ Ηη Θθ Ιι Κκ Λλ Μμ Νν Ξξ Οο Ππ Ρρ Σσς Ττ Υυ Φφ Χχ Ψψ Ωω'), LOWER('Αα Ββ Γγ Δδ Εε Ζζ Ηη Θθ Ιι Κκ Λλ Μμ Νν Ξξ Οο Ππ Ρρ Σσς Ττ Υυ Φφ Χχ Ψψ Ωω')
select UPPER(''), UPPER('hello'), UPPER('MotörHead'), UPPER(NULL)
select LOWER(''), LOWER('hello'), LOWER('MotörHead'), LOWER(NULL)
select UCASE(''), UCASE('hello'), UCASE('MotörHead'), UCASE(NULL)
select LCASE(''), LCASE('hello'), LCASE('MotörHead'), LCASE(NULL)
INSERT INTO strings VALUES ('Hello', 'World'), ('HuLlD', NULL), ('MotörHead','RÄcks')
select UPPER(a), UCASE(a) FROM strings
select LOWER(a), LCASE(a) FROM strings
select LOWER(b), LCASE(b) FROM strings
select UPPER(a), LOWER(a), UCASE(a), LCASE(a) FROM strings WHERE b IS NOT NULL
SELECT length_grapheme('S̈a')
SELECT length_grapheme('🤦🏼‍♂️')
SELECT length_grapheme('🤦🏼‍♂️ L🤦🏼‍♂️R 🤦🏼‍♂️')
SELECT length('S̈a')
SELECT length('🤦🏼‍♂️')
SELECT length('🤦🏼‍♂️ L🤦🏼‍♂️R 🤦🏼‍♂️')
SELECT strlen('🤦🏼‍♂️')
SELECT strlen('S̈a')
SELECT REVERSE('S̈a︍')
SELECT REVERSE('Z͑ͫ̓ͪ̂ͫ̽͏̴̙̤̞͉͚̯̞̠͍A̴̵̜̰͔ͫ͗͢')
SELECT REVERSE('🤦🏼‍♂️')
SELECT REVERSE('🤦🏼‍♂️ L🤦🏼‍♂️R 🤦🏼‍♂️')
INSERT INTO strings VALUES ('hello'), ('world'), (NULL)
SELECT s || ' ' || s FROM strings ORDER BY s
SELECT s || ' ' || '🦆' FROM strings ORDER BY s
SELECT s || ' ' || '🦆' || NULL FROM strings ORDER BY s
SELECT CONCAT('hello')
SELECT CONCAT('hello', 33, 22)
SELECT CONCAT('hello', 33, NULL, 22, NULL)
SELECT CONCAT('hello', ' ', s) FROM strings ORDER BY s
select [1] || [2]
select [1] || NULL
select list_concat([1], NULL)
select array[1] || array[2]
select array[1] || array[NULL]
select list_concat(array[1], array[NULL])
select array[1] || cast(NULL as int array)
select 'hi' || NULL
select list_concat([1], [2], [3])
select [1] || [2] || [3]
select CONCAT(a, 'SUFFIX') FROM strings
select CONCAT('PREFIX', b) FROM strings
select CONCAT(a, b) FROM strings
select CONCAT(a, b, 'SUFFIX') FROM strings
select CONCAT(a, b, a) FROM strings
select CONCAT('1', '2', '3', '4', '5', '6', '7', '8', '9', '0')
select '1234567890' || '1234567890', '1234567890' || NULL
select CONCAT('1234567890', '1234567890'), CONCAT('1234567890', NULL)
select CONCAT_WS(',',a, 'SUFFIX') FROM strings
select CONCAT_WS('@','PREFIX', b) FROM strings
select CONCAT_WS('$',a, b) FROM strings
select CONCAT_WS(a, b, 'SUFFIX') FROM strings
select CONCAT_WS(a, b, b) FROM strings
select CONCAT_WS('@','1', '2', '3', '4', '5', '6', '7', '8', '9')
select CONCAT_WS(b, '[', ']') FROM strings ORDER BY a
select CONCAT_WS(',', a, 'SUFFIX') FROM strings WHERE a != 'Hello'
select CONCAT_WS(',', 'hello')
select CONCAT_WS(NULL, 'hello')
select CONCAT_WS(',', NULL)
select CONCAT_WS(NULL, b, 'SUFFIX') FROM strings
SELECT CONTAINS('hello world', 'h'), CONTAINS('hello world', 'he'), CONTAINS('hello world', 'hel'), CONTAINS('hello world', 'hell'), CONTAINS('hello world', 'hello'), CONTAINS('hello world', 'hello '), CONTAINS('hello world', 'hello w'), CONTAINS('hello world', 'hello wo'), CONTAINS('hello world', 'hello wor'), CONTAINS('hello world', 'hello worl')
SELECT CONTAINS('hello world', 'a'), CONTAINS('hello world', 'ha'), CONTAINS('hello world', 'hea'), CONTAINS('hello world', 'hela'), CONTAINS('hello world', 'hella'), CONTAINS('hello world', 'helloa'), CONTAINS('hello world', 'hello a'), CONTAINS('hello world', 'hello wa'), CONTAINS('hello world', 'hello woa'), CONTAINS('hello world', 'hello wora')
select contains('hello', ''), contains('', ''), contains(NULL, '')
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
INSERT INTO strings VALUES ('hello', 1, 2), ('world', 2, 3), ('b', 1, 1), (NULL, 2, 2)
SELECT contains(s,'h') FROM strings
SELECT contains(s,'e') FROM strings
SELECT contains(s,'d') FROM strings
SELECT contains(s,'he') FROM strings
SELECT contains(s,'ello') FROM strings
SELECT contains(s,'lo') FROM strings
SELECT contains(s,'he-man') FROM strings
INSERT INTO strings VALUES ('átomo')
INSERT INTO strings VALUES ('olá mundo')
INSERT INTO strings VALUES ('你好世界')
INSERT INTO strings VALUES ('two ñ three ₡ four 🦆 end')
SELECT contains(s,'á') FROM strings
SELECT contains(s,'olá mundo') FROM strings
SELECT contains(s,'你好世界') FROM strings
SELECT contains(s,'two ñ thr') FROM strings
SELECT contains(s,'ñ') FROM strings
SELECT contains(s,'₡ four 🦆 e') FROM strings
SELECT contains(s,'🦆 end') FROM strings
SELECT damerau_levenshtein('out', 'out')
SELECT damerau_levenshtein('three', 'there')
SELECT damerau_levenshtein('potion', 'option')
SELECT damerau_levenshtein('letter', 'lettre')
SELECT damerau_levenshtein('out', 'to')
SELECT damerau_levenshtein('to', 'out')
SELECT damerau_levenshtein('laos', 'also')
SELECT damerau_levenshtein('tomato', 'otamot')
SELECT damerau_levenshtein('abcdefg', 'bacedgf')
SELECT damerau_levenshtein('abcdefghi', 'bzacdefig')
SELECT damerau_levenshtein('bzacdefig', 'abcdefghi')
SELECT damerau_levenshtein('at', 'tarokk')
SELECT format('hello'), format(NULL)
SELECT format('{}', 'hello'), format('{}: {}', 'hello', 'world')
SELECT format('{}', NULL), format(NULL, 'hello', 'world')
SELECT format('{} {}', TRUE, FALSE)
SELECT format('{}', 33), format('{} + {} = {}', 3, 5, 3 + 5)
SELECT format('{} {} = {}', DATE '1992-01-01', TIME '12:01:00', TIMESTAMP '1992-01-01 12:01:00')
SELECT format('{}', 120381902481294715712::HUGEINT)
SELECT format('{}', 120381902481294715712::UHUGEINT)
SELECT format('{:.3f}', '1.234'::DECIMAL)
SELECT format('{:04d}', 33), format('{} {:02d}:{:02d}:{:02d} {}', 'time', 12, 3, 16, 'AM'), format('{:10d}', 1992)
SELECT format('{1} {1} {0} {0}', 1, 2)
select format('{:x}', 123456789)
select printf('%,d', 123456789)
select format('{:d}', 123456789)
select printf('%,d', 123456789123456789123456789::HUGEINT)
select printf('%.d', 123456789)
select printf('%.d', -123456789123456789123456789::HUGEINT)
select printf('%_d', 123456789)
select printf('%''d', 123456789)
select printf('%.0d', 123456789)
select format('{:,}', 123456789)
select format('{:_}', 123456789)
select format('{:''}', 123456789)
select format('{:,}', 123456789123456789123456789::UHUGEINT)
SELECT 'aaa' GLOB 'bbb'
SELECT 'aaa' GLOB 'aaa'
SELECT 'aaa' GLOB '*'
SELECT 'aaa' GLOB '*a'
SELECT 'aaa' GLOB '*b'
SELECT 'aaa' GLOB 'a*'
SELECT 'aaa' GLOB 'b*'
SELECT 'aaa' GLOB 'a?a'
SELECT 'aaa' GLOB 'a?'
SELECT 'aaa' GLOB '??*'
SELECT 'aaa' GLOB '????*'
SELECT 'ababac' GLOB '*abac'
SELECT 'aaa' ILIKE 'bbb'
SELECT 'aaa' ILIKE 'aAa'
SELECT 'aaa' ILIKE '%'
SELECT 'aaa' ILIKE '%A'
SELECT 'aaa' ILIKE '%b'
SELECT 'aaa' ILIKE 'A%'
SELECT 'aaa' ILIKE 'b%'
SELECT 'aaa' ILIKE 'A_a'
SELECT 'aaa' ILIKE 'a_'
SELECT 'aaa' ILIKE '__%'
SELECT 'aaa' ILIKE '____%'
SELECT 'ababac' ILIKE '%abac'
select 'a%c' ilike 'a$%C' escape '$'
select 'A%C' ilike 'a$%c' escape '$'
select 'a%c' ilike 'a$%C' escape '/'
select NULL ilike 'a$%C' escape '/'
select 'a%c' ilike NULL escape '$'
select 'a%c' ilike 'a$%C' escape NULL
CREATE TABLE tbl(str VARCHAR, pat VARCHAR)
INSERT INTO tbl VALUES ('a%c', 'a$%C')
SELECT str ILIKE pat ESCAPE '$' FROM tbl
SELECT str NOT ILIKE pat ESCAPE '$' FROM tbl
SELECT NULL ILIKE pat ESCAPE '$' FROM tbl
SELECT str ILIKE NULL ESCAPE '$' FROM tbl
SELECT instr(s,'h') FROM strings
SELECT position('h' in s) FROM strings
SELECT instr(s,'e') FROM strings
SELECT instr(s,'d') FROM strings
SELECT instr(s,'he') FROM strings
SELECT position('he' in s) FROM strings
SELECT instr(s,'ello') FROM strings
SELECT instr(s,'lo') FROM strings
SELECT instr(s,'he-man') FROM strings
SELECT instr(s,'o'),s FROM strings
SELECT instr(NULL,'o') FROM strings
SELECT instr(s,NULL) FROM strings
SELECT INSTR(s,'á') FROM strings
SELECT POSITION('á' in s) FROM strings
SELECT INSTR(s,'olá mundo') FROM strings
SELECT INSTR(s,'你好世界') FROM strings
SELECT instr(s,'two ñ thr') FROM strings
SELECT instr(s,'ñ') FROM strings
SELECT instr(s,'₡ four 🦆 e') FROM strings
SELECT instr(s,'🦆 end') FROM strings
CREATE TABLE t (str VARCHAR)
INSERT INTO t VALUES ('hello1'), ('hello2'), ('hello3'), ('world1'), ('world2'), ('world3')
SELECT COUNT(*) FROM t WHERE str LIKE '%o%'
SELECT COUNT(*) FROM t WHERE str LIKE '%rld%'
SELECT COUNT(*) FROM t WHERE str LIKE '%o%' OR (str LIKE '%o%' AND str LIKE '%rld%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR str LIKE '%o%'
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR (str LIKE '%o%') OR (str LIKE '%o%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR (str LIKE '%o%') OR (str LIKE '%o%' AND str LIKE 'blabla%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%1%') OR (str LIKE '%o%' AND str LIKE '%1%' AND str LIKE 'blabla%') OR (str LIKE '%o%' AND str LIKE '%1%' AND str LIKE 'blabla2%')
SELECT jaccard('hello', 'hello')
SELECT jaccard('hello', NULL)
SELECT jaccard(NULL, 'hello')
SELECT jaccard(NULL, NULL)
SELECT jaccard('ab', 'aabb')
SELECT jaccard('aabb', 'ab')
SELECT jaccard('ab', 'cd')
SELECT jaccard('cd', 'ab')
SELECT round(jaccard('ab', 'aabbcc'), 3)
SELECT round(jaccard('aabbcc', 'ab'), 3)
SELECT round(jaccard('aabbccddeeff', 'ab'), 3)
SELECT round(jaccard('ab', 'aabbccddeeff'), 3)
select jaro_winkler_similarity('CRATE', 'TRACE')
select jaro_winkler_similarity('DwAyNE', 'DuANE')
select jaro_winkler_similarity('0', '0')
select jaro_winkler_similarity('00', '00')
select jaro_winkler_similarity('0', '00')
select jaro_winkler_similarity('00000000000000000000000000000000000000000000000000000000000000000', '00000000000000000000000000000000000000000000000000000000000000000')
select jaro_winkler_similarity('0000000000000000000000000000000000000000000000000000000000000000', '00000000000000000000000000000000000000000000000000000000000000000')
select jaro_winkler_similarity('000000000000000000000000000000000000000000000000000000000000000', '00000000000000000000000000000000000000000000000000000000000000000')
select jaro_winkler_similarity('10000000000000000000000000000000000000000000000000000000000000020', '00000000000000000000000000000000000000000000000000000000000000000')
select jaro_winkler_similarity('0000000000000000000000000000000000000000000000000000000000000000000000000000001', '00000000000000100000000000000000000000010000000000000000000000000')
select jaro_winkler_similarity('01000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000', '00000000000000000000000000000000000000000000000000000000000000000')
select jaro_winkler_similarity(null, null)
DROP TABLE IF EXISTS strings
CREATE TABLE strings(a STRING, b BIGINT)
INSERT INTO STRINGS VALUES ('abcd', 0), ('abc', 1), ('abc', 2), ('abc', 3), ('abc', 4)
INSERT INTO STRINGS VALUES ('abcd', 0), ('abc', -1), ('abc', -2), ('abc', -3), ('abc', -4)
INSERT INTO STRINGS VALUES (NULL, 0), ('abc', NULL), (NULL, NULL)
SELECT LEFT_GRAPHEME('🦆🤦S̈', 0), LEFT_GRAPHEME('🦆🤦S̈', 1), LEFT_GRAPHEME('🦆🤦S̈', 2), LEFT_GRAPHEME('🦆🤦S̈', 3)
SELECT LEFT_GRAPHEME('🦆🤦S̈', 0), LEFT_GRAPHEME('🦆🤦S̈', -1), LEFT_GRAPHEME('🦆🤦S̈', -2), LEFT_GRAPHEME('🦆🤦S̈', -3)
SELECT length(s) FROM strings ORDER BY s
SELECT length(s || ' ' || '🦆') FROM strings ORDER BY s
SELECT char_length('asdf'), CHARACTER_LENGTH('asdf')
SELECT levenshtein('hallo', 'hallo')
SELECT levenshtein('hallo', 'hello')
SELECT levenshtein('hello', 'hallo')
SELECT levenshtein('lawn', 'flaw')
SELECT levenshtein('flaw', 'lawn')
SELECT levenshtein('kitten', 'sitting')
SELECT levenshtein('sitting', 'kitten')
SELECT levenshtein('hallo', 'hoi')
SELECT levenshtein('hoi', 'hallo')
SELECT levenshtein(NULL, 'hi')
SELECT levenshtein('hi', NULL)
SELECT levenshtein(NULL, NULL)
SELECT 'aaa' LIKE 'bbb'
SELECT 'aaa' LIKE 'abab'
SELECT 'aaa' LIKE 'aaa'
SELECT 'aaa' LIKE '%'
SELECT 'aaa' LIKE '%a'
SELECT 'aaa' LIKE '%b'
SELECT 'aaa' LIKE 'a%'
SELECT 'aaa' LIKE 'b%'
SELECT 'aaa' LIKE 'a_a'
SELECT 'aaa' LIKE 'a_'
SELECT 'aaa' LIKE '__%'
SELECT 'aaa' LIKE '____%'
SELECT '%++' LIKE '*%++' ESCAPE '*'
SELECT '%++' NOT LIKE '*%++' ESCAPE '*'
SELECT '\' LIKE '\\' ESCAPE '\'
SELECT '\\' LIKE '\\' ESCAPE '\'
SELECT '%' LIKE '*%' ESCAPE '*'
SELECT '_ ' LIKE '*_ ' ESCAPE '*'
SELECT ' a ' LIKE '*_ ' ESCAPE '*'
SELECT '%_' LIKE '%_' ESCAPE ''
SELECT '*%' NOT LIKE '*%' ESCAPE '*'
CREATE TABLE strings(s STRING, pat STRING)
INSERT INTO strings VALUES ('abab', 'ab%'), ('aaa', 'a*_a'), ('aaa', '*%b'), ('bbb', 'a%')
SELECT s FROM strings
SELECT mismatches('hallo', 'hallo')
SELECT mismatches('hello', 'hallo')
SELECT mismatches('hallo', 'hello')
SELECT mismatches('aloha', 'hallo')
SELECT mismatches('hallo', 'aloha')
SELECT mismatches(NULL, 'hallo')
SELECT mismatches('hello', NULL)
SELECT mismatches(NULL, NULL)
INSERT INTO strings VALUES ('hello'), ('hallo'), ('aloha'), ('world'), (NULL)
SELECT mismatches(s, 'hallo') FROM strings ORDER BY s
SELECT mismatches('hallo', s) FROM strings ORDER BY s
SELECT mismatches(NULL, s) FROM strings ORDER BY s
select LPAD(NULL, 7, '-'), LPAD('Base', NULL, '-'), LPAD('Base', 7, NULL), LPAD(NULL, NULL, '-'), LPAD(NULL, 7, NULL), LPAD('Base', NULL, NULL), LPAD(NULL, NULL, NULL)
select RPAD(NULL, 7, '-'), RPAD('Base', NULL, '-'), RPAD('Base', 7, NULL), RPAD(NULL, NULL, '-'), RPAD(NULL, 7, NULL), RPAD('Base', NULL, NULL), RPAD(NULL, NULL, NULL)
select LPAD('Base', 7, '-'), LPAD('Base', 4, '-'), LPAD('Base', 2, ''), LPAD('Base', -1, '-')
select RPAD('Base', 7, '-'), RPAD('Base', 4, '-'), RPAD('Base', 2, ''), RPAD('Base', -1, '-')
select LPAD('Base', 7, '-|'), LPAD('Base', 6, '-|'), LPAD('Base', 5, '-|'), LPAD('Base', 4, '-|')
select RPAD('Base', 7, '-|'), RPAD('Base', 6, '-|'), RPAD('Base', 5, '-|'), RPAD('Base', 4, '-|')
select LPAD('MotörHead', 16, 'RÄcks'), LPAD('MotörHead', 12, 'RÄcks'), LPAD('MotörHead', 10, 'RÄcks')
select RPAD('MotörHead', 16, 'RÄcks'), RPAD('MotörHead', 12, 'RÄcks'), RPAD('MotörHead', 10, 'RÄcks')
select LPAD(a, 16, b), RPAD(a, 16, b) FROM strings
select LPAD(a, 12, b), RPAD(a, 12, b), UCASE(a), LCASE(a) FROM strings WHERE b IS NOT NULL
SELECT prefix('abcd', 'a')
SELECT prefix('abcd', 'ab')
SELECT prefix('abcd', 'abc')
SELECT prefix('abcd', 'abcd')
SELECT prefix('abcd', 'b')
SELECT prefix('abcdefgh', 'a')
SELECT prefix('abcdefgh', 'ab')
SELECT prefix('abcdefgh', 'abc')
SELECT prefix('abcdefgh', 'abcd')
SELECT prefix('abcdefgh', 'abcde')
SELECT prefix('abcdefgh', 'b')
SELECT prefix('abcdefghijklmnopqrstuvwxyz', 'a')
SELECT printf('hello'), printf(NULL)
SELECT printf('%s', 'hello'), printf('%s: %s', 'hello', 'world')
SELECT printf('%s', NULL), printf(NULL, 'hello', 'world')
SELECT printf('%d', TRUE)
SELECT printf('%d', 33), printf('%d + %d = %d', 3, 5, 3 + 5)
SELECT printf('%d', 18446744073709551615::UBIGINT)
SELECT printf('%04d', 33), printf('%s %02d:%02d:%02d %s', 'time', 12, 3, 16, 'AM'), printf('%10d', 1992)
SELECT printf('%hhd %hd %d %lld', 33::TINYINT, 12::SMALLINT, 40::INTEGER, 80::BIGINT)
SELECT printf('%d %lld %hhd %hd', 33::TINYINT, 12::SMALLINT, 40::INTEGER, 80::BIGINT)
SELECT printf('%s %s = %s', DATE '1992-01-01', TIME '12:01:00', TIMESTAMP '1992-01-01 12:01:00')
SELECT printf('%d', 120381902481294715712::HUGEINT)
SELECT printf('%d', '-170141183460469231731687303715884105728'::HUGEINT)
select REPEAT(NULL, NULL), REPEAT(NULL, 3), REPEAT('MySQL', NULL)
select REPEAT('', 3), REPEAT('MySQL', 3), REPEAT('MotörHead', 2), REPEAT('Hello', -1)
INSERT INTO strings VALUES ('Hello', 'World'), ('HuLlD', NULL), ('MotörHead','RÄcks'), ('', NULL)
select REPEAT(a, 3) FROM strings
select REPEAT(b, 2) FROM strings
select REPEAT(a, 4) FROM strings WHERE b IS NOT NULL
SELECT repeat('', 99)
SELECT repeat('hello world', 0)
SELECT repeat('hello world', -1)
SELECT repeat(blob '00', 2)
select REPLACE('This is the main test string', NULL, 'ALT')
select REPLACE(NULL, 'main', 'ALT')
select REPLACE('This is the main test string', 'main', NULL)
select REPLACE('This is the main test string', 'main', 'ALT')
select REPLACE('This is the main test string', 'main', 'larger-main')
select REPLACE('aaaaaaa', 'a', '0123456789')
select REPLACE(a, 'l', '-') FROM strings
select REPLACE(b, 'Ä', '--') FROM strings
select REPLACE(a, 'H', '') FROM strings WHERE b IS NOT NULL
select REVERSE(''), REVERSE('Hello'), REVERSE('MotörHead'), REVERSE(NULL)
select REVERSE(a) FROM strings
select REVERSE(b) FROM strings
select REVERSE(a) FROM strings WHERE b IS NOT NULL
SELECT RIGHT_GRAPHEME('🦆🤦S̈', 0), RIGHT_GRAPHEME('🦆🤦S̈', 1), RIGHT_GRAPHEME('🦆🤦S̈', 2), RIGHT_GRAPHEME('🦆🤦S̈', 3)
SELECT RIGHT_GRAPHEME('🦆🤦S̈', 0), RIGHT_GRAPHEME('🦆🤦S̈', -1), RIGHT_GRAPHEME('🦆🤦S̈', -2), RIGHT_GRAPHEME('🦆🤦S̈', -3)
SELECT right_grapheme('a', -9223372036854775808)
SELECT "right"('a', -9223372036854775808)
SELECT 'aaa' SIMILAR TO 'bbb'
SELECT 'aaa' SIMILAR TO 'aaa'
SELECT 'aaa' SIMILAR TO '.*'
SELECT 'aaa' SIMILAR TO 'a.*'
SELECT 'aaa' SIMILAR TO '.*a'
SELECT 'aaa' SIMILAR TO '.*b'
SELECT 'aaa' SIMILAR TO 'b.*'
SELECT 'aaa' SIMILAR TO 'a[a-z]a'
SELECT 'aaa' SIMILAR TO 'a[a-z]{2}'
SELECT 'aaa' SIMILAR TO 'a[a-z].*'
SELECT 'aaa' SIMILAR TO '[a-z][a-z].*'
SELECT 'aaa' SIMILAR TO '[a-z]{3}'
select split_part('a,b,c',',',1)
select split_part('a,b,c',',',2)
select split_part('a,,b,,c',',,',2)
SELECT split_part('a,b,c','|',1)
select split_part('a,b,c',',',-1)
select split_part('a,b,c',',',-2)
select split_part('a,b,c',',',0)
select split_part('a,b,c',',',5)
select split_part('a,b,c',',',-5)
select split_part('','',1)
select split_part('a,b,c','',3)
select split_part('',',',1)
SELECT STARTS_WITH('hello world', 'h'), STARTS_WITH('hello world', 'he'), STARTS_WITH('hello world', 'hel'), STARTS_WITH('hello world', 'hell'), STARTS_WITH('hello world', 'hello'), STARTS_WITH('hello world', 'hello '), STARTS_WITH('hello world', 'hello w'), STARTS_WITH('hello world', 'hello wo'), STARTS_WITH('hello world', 'hello wor'), STARTS_WITH('hello world', 'hello worl')
SELECT STARTS_WITH('hello world', 'a'), STARTS_WITH('hello world', 'ha'), STARTS_WITH('hello world', 'hea'), STARTS_WITH('hello world', 'hela'), STARTS_WITH('hello world', 'hella'), STARTS_WITH('hello world', 'helloa'), STARTS_WITH('hello world', 'hello a'), STARTS_WITH('hello world', 'hello wa'), STARTS_WITH('hello world', 'hello woa'), STARTS_WITH('hello world', 'hello wora')
select starts_with('hello', ''), starts_with('', ''), starts_with(NULL, '')
INSERT INTO strings VALUES ('hello', 1, 2), ('world', 2, 3), ('h', 1, 1), (NULL, 2, 2)
SELECT starts_with(s,'h') FROM strings
SELECT starts_with(s,'he') FROM strings
SELECT starts_with(s,'he-man') FROM strings
SELECT starts_with(NULL,'h') FROM strings
SELECT starts_with(s,NULL) FROM strings
SELECT starts_with(NULL,NULL) FROM strings
SELECT starts_with(s,'') FROM strings
SELECT starts_with(s,'á') FROM strings
SELECT starts_with(s,'olá mundo') FROM strings
SELECT starts_with(s,'你好世界') FROM strings
SELECT starts_with(s,'two ñ thr') FROM strings
SELECT starts_with(s,'ñ') FROM strings
SELECT starts_with(s,'₡ four 🦆 e') FROM strings
SELECT 'hello world' ^@ 'h', 'hello world' ^@ 'he', 'hello world' ^@ 'hel', 'hello world' ^@ 'hell', 'hello world' ^@ 'hello', 'hello world' ^@ 'hello ', 'hello world' ^@ 'hello w', 'hello world' ^@ 'hello wo', 'hello world' ^@ 'hello wor', 'hello world' ^@ 'hello worl'
SELECT 'hello world' ^@ 'a', 'hello world' ^@ 'ha', 'hello world' ^@ 'hea', 'hello world' ^@ 'hela', 'hello world' ^@ 'hella', 'hello world' ^@ 'helloa', 'hello world' ^@ 'hello a', 'hello world' ^@ 'hello wa', 'hello world' ^@ 'hello woa', 'hello world' ^@ 'hello wora'
select 'hello' ^@ '', '' ^@ '', NULL ^@ ''
SELECT s ^@ 'h' FROM strings
SELECT s ^@ 'he' FROM strings
SELECT s ^@ 'he-man' FROM strings
SELECT NULL ^@ 'h' FROM strings
SELECT s ^@ NULL FROM strings
SELECT NULL ^@ NULL FROM strings
SELECT s ^@ '' FROM strings
SELECT s ^@ 'á' FROM strings
SELECT s ^@ 'olá mundo' FROM strings
SELECT s ^@ '你好世界' FROM strings
SELECT s ^@ 'two ñ thr' FROM strings
SELECT s ^@ 'ñ' FROM strings
SELECT s ^@ '₡ four 🦆 e' FROM strings
SELECT 'hello'[0:2]
SELECT ('hello')[0:2]
INSERT INTO strings VALUES ('hello', 0, 2), ('world', 1, 3), ('b', 0, 1), (NULL, 1, 2)
SELECT array_slice('🦆ab', 0, 0), array_slice('abc', 0, 0)
SELECT array_slice(s, 0, 2) FROM strings
SELECT list_slice(s, 0, 2) FROM strings
SELECT array_slice(s, 1, 3) FROM strings
SELECT array_slice(s, 2, 3) FROM strings
SELECT array_slice(s, off, length+off) FROM strings
SELECT array_slice(s, off, 2+off) FROM strings
SELECT array_slice(s, 0, length) FROM strings
SELECT array_slice('hello', off, length+off) FROM strings
CREATE TABLE nulltable(n VARCHAR)
INSERT INTO nulltable VALUES (NULL)
SELECT '🦆ab'[0:0], 'abc'[0:0]
SELECT 'MotörHead'[:5]
SELECT s[0:2] FROM strings
SELECT s[1:3] FROM strings
SELECT s[2:3] FROM strings
SELECT s[off:length+off] FROM strings
SELECT s[off:2+off] FROM strings
SELECT s[0:length] FROM strings
SELECT 'hello'[off:length+off] FROM strings
SELECT n[off:length+off] FROM strings, nulltable
SELECT string_split(NULL, NULL)
SELECT * FROM (VALUES (string_split('hello world', ' ')), (string_split(NULL, ' ')), (string_split('a b c', NULL)), (string_split('a b c', ' '))) tbl(i)
CREATE TABLE strings_with_null (s VARCHAR)
INSERT INTO strings_with_null VALUES ('aba'), (NULL), ('ababa')
SELECT UNNEST(string_split(s, 'b')) FROM strings_with_null
SELECT UNNEST(string_split(NULL, ' ')) IS NULL LIMIT 5
SELECT UNNEST(string_split('üüüüü', '◌̈'))
SELECT UNNEST(string_split('üüüüü', '◌'))
SELECT UNNEST(string_split_regex('üüüüü', '◌̈'))
SELECT UNNEST(string_split_regex('üüüüü', '◌'))
SELECT UNNEST(string_split(' 🦆🦆 🦆🦆', ' '))
SELECT UNNEST(string_split('a a a a a', ' '))
SELECT '🦆ab'[1], 'abc'[2]
SELECT s[2] FROM strings
SELECT s[3] FROM strings
SELECT s[off] FROM strings
SELECT 'hello'[off] FROM strings
SELECT 'hello'[NULL] FROM strings
SELECT s[-1] FROM strings
SELECT s[1] FROM strings
SELECT s[6] FROM strings
SELECT s[2147483646] FROM strings
SELECT s[-2147483647] FROM strings
SELECT ([1,2,3])[-2147483647]
SELECT substring(s from 1 for 2) FROM strings
SELECT substring(s from 2 for 2) FROM strings
SELECT substring(s from off for length) FROM strings
SELECT substring(s from off for 2) FROM strings
SELECT substring(s from 1 for length) FROM strings
SELECT substring('hello' from off for length) FROM strings
SELECT substring(NULL from off for length) FROM strings
SELECT substring('hello' from NULL for length) FROM strings
SELECT substring('hello' from off for NULL) FROM strings
SELECT substring(NULL from NULL for length) FROM strings
SELECT substring('hello' from NULL for NULL) FROM strings
SELECT substring(NULL from off for NULL) FROM strings
INSERT INTO strings VALUES ('twoñthree₡four🦆end')
SELECT substring(s from 1 for 7) FROM strings
SELECT substring(s from 10 for 7) FROM strings
SELECT substring(s from 15 for 7) FROM strings
SELECT suffix('abcd', 'd')
SELECT suffix('abcd', 'cd')
SELECT suffix('abcd', 'bcd')
SELECT suffix('abcd', 'abcd')
SELECT suffix('abcd', 'X')
SELECT suffix('abcdefgh', 'h')
SELECT suffix('abcdefgh', 'gh')
SELECT suffix('abcdefgh', 'fgh')
SELECT suffix('abcdefgh', 'efgh')
SELECT suffix('abcdefgh', 'defgh')
SELECT suffix('abcdefgh', 'X')
SELECT suffix('abcdefgh', 'abcdefgh')
SELECT to_base(10, 2)
SELECT to_base(10, 2, 64)
SELECT to_base(10, 3)
SELECT to_base(10, 16)
SELECT to_base(10, 36)
SELECT to_base(42, 36)
SELECT to_base(range, 2), to_base(range, 2, 8), to_base(range, 16), to_base(range, 16, 2), to_base(range, 36), to_base(range, 36, 2) FROM range(1, 43) ORDER BY range
CREATE TABLE fib AS SELECT * FROM (VALUES (0), (1), (1), (2), (3), (5), (8), (13), (21), (34), (55), (89), (144), (233), (377), (610), (987), (1597), (2584), (4181), (6765), (10946), (17711), (28657), (46368) )
SELECT to_base(col0, 2) FROM fib ORDER BY col0
SELECT to_base(col0, 16) FROM fib ORDER BY col0
SELECT to_base(col0, 36) FROM fib ORDER BY col0
select TRANSLATE('This is the main test string', NULL, 'ALT')
select TRANSLATE(NULL, 'main', 'ALT')
select TRANSLATE('This is the main test string', 'main', NULL)
select TRANSLATE('12', '2', 'a')
select TRANSLATE('abcde', 'abcde', 'fghij')
select TRANSLATE('abcde', 'aabcc', '14235')
select TRANSLATE('https://dxyzdb.org', 'zyx.orghttps:/', 'kcu')
select TRANSLATE('12345', '14367', 'ax')
select TRANSLATE('hacco worcdxxx', 'acx2', 'el')
select TRANSLATE('hacCo worcd', 'acC', 'ellaabb')
select TRANSLATE('RÄcks', 'Ä', 'A')
select TRANSLATE('🦆', '🦆', 'D')
select LTRIM(''), LTRIM('Neither'), LTRIM(' Leading'), LTRIM('Trailing '), LTRIM(' Both '), LTRIM(NULL), LTRIM(' ')
select RTRIM(''), RTRIM('Neither'), RTRIM(' Leading'), RTRIM('Trailing '), RTRIM(' Both '), RTRIM(NULL), RTRIM(' ')
select TRIM(''), TRIM('Neither'), TRIM(' Leading'), TRIM('Trailing '), TRIM(' Both '), TRIM(NULL), TRIM(' ')
INSERT INTO strings VALUES ('', 'Neither'), (' Leading', NULL), (' Both ','Trailing '), ('', NULL)
select LTRIM(a) FROM strings
select LTRIM(b) FROM strings
select LTRIM(a) FROM strings WHERE b IS NOT NULL
select RTRIM(a) FROM strings
select RTRIM(b) FROM strings
select RTRIM(a) FROM strings WHERE b IS NOT NULL
select LTRIM('', 'ho'), LTRIM('hello', 'ho'), LTRIM('papapapa', 'pa'), LTRIM('blaHblabla', 'bla'), LTRIM('blabla', NULL), LTRIM(NULL, 'blabla'), LTRIM('blabla', '')
select RTRIM('', 'ho'), RTRIM('hello', 'ho'), RTRIM('papapapa', 'pa'), RTRIM('blaHblabla', 'bla'), RTRIM('blabla', NULL), RTRIM(NULL, 'blabla'), RTRIM('blabla', '')
select UNICODE(NULL), UNICODE(''), UNICODE('$'), UNICODE('¢'), UNICODE('€'), UNICODE('𐍈')
select UNICODE(a) FROM strings
select UNICODE(b) FROM strings
select UNICODE(a) FROM strings WHERE b IS NOT NULL
SELECT url_encode(''), url_decode('')
SELECT url_encode(NULL), url_decode(NULL)
SELECT url_decode(url_encode('http://www.google.com/this is a long url'))
SELECT COUNT(*) from range(1000) t(n) WHERE url_decode(url_encode(chr(n::INT))) = chr(n::INT)
SELECT url_decode('%'), url_decode('%5'), url_decode('%X'), url_decode('%%')
SELECT (ROW(42, 84))[1]
SELECT (ROW(42, 84))[2]
SELECT UNNEST(ROW(42, 84))
with data as ( select * from (VALUES ('Amsterdam', {'x': 1, 'y': 2, 'z': 3}), ('London', {'x': 4, 'y': 5, 'z': 6})) Cities(Name, Id) ) select *, struct_insert(Id, d := 4) from data
SELECT struct_insert ({a: 1, b: 2}, c := 3)
WITH data AS (SELECT 1 AS a, 2 AS b, 3 AS c) SELECT struct_insert (data, d := 4) FROM data
SELECT struct_insert({'a': 1, 'b': 'abc', 'c': true}, d := {'a': 'new stuff'})
INSERT INTO tbl SELECT {'i': range} FROM range(3)
SELECT struct_insert(col, a := col.i + 1, b := NULL::VARCHAR) FROM tbl ORDER BY ALL
SELECT struct_insert(col, a := NULL, b := NULL::VARCHAR, c := [NULL]) FROM tbl ORDER BY ALL
SELECT struct_update ({a: 1, b: 2}, c := 3)
WITH data AS (SELECT 1 AS a, 2 AS b, 3 AS c) SELECT struct_update (data, d := 4) FROM data
SELECT struct_update({'a': 1, 'b': 'abc', 'c': true}, d := {'a': 'new stuff'})
SELECT struct_update({a: 1, b: 2}, a := 3)
SELECT struct_update({a: 1, b: 2}, a := 'c')
SELECT struct_update({a: 1, b: 2}, a := 'c', d := 3)
SELECT struct_update({a: 1, b: 2}, d := 3, a := 'c')
SELECT struct_update(col, i:=10) FROM tbl
SELECT struct_update(col, i:=col.i+1) FROM tbl
SELECT struct_update(col, i:='i='||col.i) FROM tbl
SELECT struct_update(col, a := NULL::VARCHAR) FROM tbl ORDER BY ALL
SELECT struct_update(col, a := NULL, b := NULL::VARCHAR, c := [NULL]) FROM tbl ORDER BY ALL
SELECT AGE(TIMESTAMP '1957-06-13') t
SELECT AGE(TIMESTAMP '2001-04-10', TIMESTAMP '1957-06-13')
SELECT age(TIMESTAMP '2014-04-25', TIMESTAMP '2014-04-17')
SELECT age(TIMESTAMP '2014-04-25', TIMESTAMP '2014-01-01')
SELECT age(TIMESTAMP '2019-06-11', TIMESTAMP '2019-06-11')
SELECT age(TIMESTAMP '2019-06-11', TIMESTAMP '2019-06-11')::VARCHAR
SELECT age(timestamp '2019-06-11 12:00:00', timestamp '2019-07-11 11:00:00')
CREATE TABLE timestamp(t1 TIMESTAMP, t2 TIMESTAMP)
INSERT INTO timestamp VALUES('2001-04-10', '1957-06-13')
INSERT INTO timestamp VALUES('2014-04-25', '2014-04-17')
INSERT INTO timestamp VALUES('2014-04-25','2014-01-01')
INSERT INTO timestamp VALUES('2019-06-11', '2019-06-11')
SET Calendar='gregorian'
SELECT CAST(CURRENT_TIME AS STRING), CAST(CURRENT_DATE AS STRING), CAST(CURRENT_TIMESTAMP AS STRING), CAST(NOW() AS STRING)
SELECT typeof(CURRENT_TIME)
SELECT typeof(CURRENT_DATE)
SELECT typeof(CURRENT_TIMESTAMP)
SELECT typeof(get_current_time())
SELECT CURRENT_TIME AS TIME
SELECT CURRENT_TIME + interval (1) second AS TIME
SET TimeZone='Pacific/Honolulu'
select current_timestamp
select current_time
SET TimeZone = 'America/Chihuahua'
SELECT EXTRACT(MILLENNIUM FROM NOW())
SELECT SUFFIX(CURRENT_TIMESTAMP::VARCHAR, '-06')
SELECT make_timestamp(0) as epoch1, make_timestamp(1574802684123 * 1000) as epoch2, make_timestamp(-291044928000 * 1000) as epoch3, make_timestamp(-291081600000 * 1000) as epoch4, make_timestamp(-291081600001 * 1000) as epoch5, make_timestamp(-290995201000 * 1000) as epoch6
SELECT make_timestamp_ms(0) as epoch1, make_timestamp_ms(1574802684123) as epoch2, make_timestamp_ms(-291044928000) as epoch3, make_timestamp_ms(-291081600000) as epoch4, make_timestamp_ms(-291081600001) as epoch5, make_timestamp_ms(-290995201000) as epoch6
SELECT to_timestamp(0), to_timestamp(1), to_timestamp(1574802684), to_timestamp(-1)
SELECT to_timestamp(1284352323.5)
CREATE TABLE IF NOT EXISTS dates (d date)
SELECT d FROM (SELECT d, make_date(year(d), month(d), day(d)) md FROM dates) tbl WHERE md IS DISTINCT FROM d
SELECT d FROM (SELECT d, make_date(date_part(['year', 'month', 'day'], d)) md FROM dates) tbl WHERE md IS DISTINCT FROM d
SELECT md FROM (SELECT make_date(NULL, month(d), day(d)) md FROM dates) t WHERE md IS NOT NULL
SELECT md FROM (SELECT make_date(year(d), NULL, day(d)) md FROM dates) t WHERE md IS NOT NULL
SELECT md FROM (SELECT make_date(year(d), month(d), NULL) md FROM dates) t WHERE md IS NOT NULL
SELECT * FROM dates WHERE d <> make_date((d - date '1970-01-01')::INT)
SELECT make_date(2021, 12, 30), make_date(NULL, 12, 30), make_date(2021, NULL, 30), make_date(2021, 12, NULL)
CREATE TABLE timestamps(ts TIMESTAMP)
SELECT ts, mts FROM (SELECT ts, make_timestamp(year(ts), month(ts), day(ts), hour(ts), minute(ts), microsecond(ts) / 1000000.0) mts FROM timestamps) t WHERE mts IS DISTINCT FROM ts
SELECT md FROM ( SELECT make_timestamp(NULL, month(ts), day(ts), hour(ts), minute(ts), microsecond(ts) / 1000000.0) md FROM timestamps) t WHERE md IS NOT NULL
SELECT md FROM ( SELECT make_timestamp(year(ts), NULL, day(ts), hour(ts), minute(ts), microsecond(ts) / 1000000.0) md FROM timestamps) t WHERE md IS NOT NULL
SELECT start_ts, end_ts, DATEDIFF('day', start_ts, end_ts) AS dd_hour FROM VALUES ( '1970-01-03 12:12:12'::TIMESTAMP, '1969-12-25 05:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('hour', start_ts, end_ts) AS dd_hour FROM VALUES ( '1970-01-01 12:12:12'::TIMESTAMP, '1969-12-31 05:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('minute', start_ts, end_ts) AS dd_minute FROM VALUES ( '1970-01-01 00:12:12'::TIMESTAMP, '1969-12-31 23:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('second', start_ts, end_ts) AS dd_second FROM VALUES ( '1970-01-01 00:00:12.456'::TIMESTAMP, '1969-12-31 23:59:05.123'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('millisecond', start_ts, end_ts) AS dd_second FROM VALUES ( '1970-01-01 00:00:12.456789'::TIMESTAMP, '1969-12-31 23:59:05.123456'::TIMESTAMP ) x(start_ts, end_ts)
CREATE TABLE millennia AS SELECT * FROM (VALUES ('1001-03-15 (BC) 20:38:40'::TIMESTAMP), ('0044-03-15 (BC) 20:38:40'::TIMESTAMP), ('0998-02-16 20:38:40'::TIMESTAMP), ('1998-02-16 20:38:40'::TIMESTAMP), ('2001-02-16 20:38:40'::TIMESTAMP) ) tbl(ts)
SELECT ts, DATE_PART('millennium', ts) FROM millennia
SELECT ts, DATE_PART('century', ts) FROM millennia
SELECT DATE_PART('isoyear', ts), ts FROM timestamps ORDER BY 2
SELECT DATE_PART('isoyear', ts), ts FROM generate_series('2021-12-26'::TIMESTAMP, '2022-01-12'::TIMESTAMP, INTERVAL 1 DAY) tbl(ts)
SELECT DATE_PART('julian', ts), ts FROM timestamps ORDER BY 2
SELECT ts::DATE AS d, DATE_PART(['year', 'month', 'day'], ts) AS parts FROM timestamps ORDER BY 1
SELECT ts::DATE AS d, DATE_PART(['year', 'month', 'day'], ts) AS parts FROM millennia ORDER BY 1
SELECT ts::DATE AS d, DATE_PART(['era', 'millennium', 'century', 'decade', 'quarter'], ts) AS parts FROM timestamps ORDER BY 1
SELECT ts::DATE AS d, DATE_PART(['era', 'millennium', 'century', 'decade', 'quarter'], ts) AS parts FROM millennia ORDER BY 1
SELECT ts::DATE AS d, DATE_PART(['weekday', 'isodow','doy', 'julian'], ts) AS parts FROM timestamps ORDER BY ts
SELECT DATE_PART(['weekday', 'isodow', 'doy', 'julian'], '2008-01-01 00:00:01.894'::TIMESTAMP) AS parts
CREATE TABLE timestamps(i TIMESTAMP)
INSERT INTO timestamps VALUES ('1993-08-14 08:22:33'), (NULL)
SELECT EXTRACT(year FROM i) FROM timestamps
SELECT EXTRACT(month FROM i) FROM timestamps
SELECT EXTRACT(day FROM i) FROM timestamps
SELECT EXTRACT(week FROM i) FROM timestamps
SELECT EXTRACT(yearweek FROM i) FROM timestamps
SELECT EXTRACT(quarter FROM i) FROM timestamps
SELECT EXTRACT(decade FROM i) FROM timestamps
SELECT EXTRACT(century FROM i) FROM timestamps
SELECT EXTRACT(DOW FROM i) FROM timestamps
SELECT EXTRACT(DOY FROM i) FROM timestamps
INSERT INTO timestamps VALUES ('1993-08-14 08:22:33.42'), (NULL)
SELECT EXTRACT(second FROM i) FROM timestamps
SELECT EXTRACT(minute FROM i) FROM timestamps
SELECT EXTRACT(milliseconds FROM i) FROM timestamps
SELECT EXTRACT(microseconds FROM i) FROM timestamps
SELECT AGE(TIMESTAMPTZ '1957-06-13') t
SELECT AGE(TIMESTAMP '2001-04-10 00:00:00-07', TIMESTAMP '1957-06-13 00:00:00-07')
SELECT age(TIMESTAMP '2014-04-25 00:00:00-07', TIMESTAMP '2014-04-17 00:00:00-07')
SELECT age(TIMESTAMPTZ '2014-04-25', TIMESTAMPTZ '2014-01-01')
SELECT age(TIMESTAMPTZ '2019-06-11', TIMESTAMPTZ '2019-06-11')
SELECT age(TIMESTAMPTZ '2019-06-11', TIMESTAMPTZ '2019-06-11')::VARCHAR
SELECT age(TIMESTAMPTZ '2019-06-11 12:00:00-07', TIMESTAMPTZ '2019-07-11 11:00:00-07')
CREATE TABLE timestamps(t1 TIMESTAMPTZ, t2 TIMESTAMPTZ)
INSERT INTO timestamps VALUES ('2001-04-10', '1957-06-13'), ('2014-04-25', '2014-04-17'), ('2014-04-25','2014-01-01'), ('2019-06-11', '2019-06-11'), (NULL, '2019-06-11'), ('2019-06-11', NULL), (NULL, NULL)
SELECT AGE(t1, TIMESTAMPTZ '1957-06-13') FROM timestamps
SELECT AGE(TIMESTAMPTZ '2001-04-10', t2) FROM timestamps
SELECT AGE(t1, t2) FROM timestamps
SELECT '2021-12-01 13:54:48Z'::TIMESTAMPTZ + INTERVAL 1 DAY
SELECT iv, '2021-12-01 13:54:48.123456Z'::TIMESTAMPTZ + iv FROM intervals
select '1999-12-31 16:00:00-08'::timestamptz + interval 2400 hours
select 'epoch'::timestamptz + '9223372036854774999 microseconds'::interval
select 'epoch'::timestamptz + '-9223372022400001000 microseconds'::interval
SELECT iv, iv + '2021-12-01 13:54:48.123456Z'::TIMESTAMPTZ FROM intervals
select interval 2400 hours + '1999-12-31 16:00:00-08'::timestamptz
select '9223372036854774999 microseconds'::interval + 'epoch'::timestamptz
select '-9223372022400001000 microseconds'::interval + 'epoch'::timestamptz
select 'infinity'::timestamptz + '1 microsecond'::interval
select '1 microsecond'::interval + 'infinity'::timestamptz
select '-infinity'::timestamptz + '1 microsecond'::interval
CREATE TABLE datetime1 AS SELECT '2005-12-31 23:59:59.9999999-08'::TIMESTAMPTZ AS startdate, '2006-01-01 00:00:00.0000000-08'::TIMESTAMPTZ AS enddate
SELECT DATEDIFF('isoyear', '2022-01-01 00:00:00-08'::TIMESTAMPTZ, '2022-01-03 00:00:00-08'::TIMESTAMPTZ)
SELECT *, DATE_DIFF('week', lo, hi) FROM ( SELECT (d - INTERVAL 9 HOUR)::TIMESTAMPTZ AS lo, (d + INTERVAL 7 HOUR)::TIMESTAMPTZ AS hi FROM generate_series('2022-09-01'::DATE, '2022-09-12'::DATE, INTERVAL 1 DAY) tbl(d) )
SELECT date_diff('week', '2015-10-06 04:22:11'::timestamptz, '2016-11-25 23:19:37'::timestamptz)
set timezone='CET'
CREATE TABLE issue9673(starttime TIMESTAMPTZ, recordtime TIMESTAMPTZ)
INSERT INTO issue9673 VALUES ('2022-10-30 02:17:00+02', '2022-10-30 02:00:21+01')
INSERT INTO issue9673 VALUES ('2021-10-31 02:39:00+02', '2021-10-31 02:38:20+01')
SELECT starttime, recordtime, date_diff('minute', starttime, recordtime) FROM issue9673
select date_diff('day', '2022-01-04 19:00:00'::timestamptz, '2024-03-01'::date) as c1
SELECT year(ts), year(ts::TIMESTAMP) FROM timestamps
SELECT month(ts), month(ts::TIMESTAMP) FROM timestamps
SELECT day(ts), day(ts::TIMESTAMP) FROM timestamps
SELECT decade(ts), decade(ts::TIMESTAMP) FROM timestamps
SELECT century(ts), century(ts::TIMESTAMP) FROM timestamps
SELECT millennium(ts), millennium(ts::TIMESTAMP) FROM timestamps
SELECT microsecond(ts), microsecond(ts::TIMESTAMP) FROM timestamps
SELECT millisecond(ts), millisecond(ts::TIMESTAMP) FROM timestamps
SELECT second(ts), second(ts::TIMESTAMP) FROM timestamps
SELECT minute(ts), minute(ts::TIMESTAMP) FROM timestamps
SELECT hour(ts), hour(ts::TIMESTAMP) FROM timestamps
SELECT dayofweek(ts), dayofweek(ts::TIMESTAMP) FROM timestamps
select DATESUB('month', '2004-01-31 12:00:00-08'::TIMESTAMPTZ, '2004-02-29 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('month', '2004-01-29 12:00:00-08'::TIMESTAMPTZ, '2004-02-29 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('month', '2004-02-29 12:00:00-08'::TIMESTAMPTZ, '2004-03-31 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('month', '2004-02-29 13:00:00-08'::TIMESTAMPTZ, '2004-03-31 12:00:00-08'::TIMESTAMPTZ)
select DATESUB('quarter', '2004-01-31 12:00:00-07'::TIMESTAMPTZ, '2004-04-30 13:00:00-07'::TIMESTAMPTZ)
select DATESUB('year', '2004-02-29 12:00:00-08'::TIMESTAMPTZ, '2005-02-28 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('isoyear', '2004-02-29 12:00:00-08'::TIMESTAMPTZ, '2005-02-28 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('decade', '1994-02-28 12:00:00-08'::TIMESTAMPTZ, '2004-02-29 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('century', '1904-02-29 12:00:00-08'::TIMESTAMPTZ, '2005-02-28 13:00:00-08'::TIMESTAMPTZ)
select DATESUB('month', '2004-01-31 13:00:00-08'::TIMESTAMPTZ, '2004-02-29 12:00:00-08'::TIMESTAMPTZ)
select DATESUB('month', '2004-01-29 13:00:00-08'::TIMESTAMPTZ, '2004-02-29 12:00:00-08'::TIMESTAMPTZ)
select DATESUB('quarter', '2004-01-31 13:00:00-08'::TIMESTAMPTZ, '2004-04-30 12:00:00-07'::TIMESTAMPTZ)
CREATE TABLE timestamps(d TIMESTAMPTZ, s VARCHAR)
SELECT date_trunc(NULL::VARCHAR, NULL::TIMESTAMPTZ) FROM timestamps LIMIT 3
SELECT date_trunc(s, NULL::TIMESTAMPTZ) FROM timestamps LIMIT 3
SELECT date_trunc('minute', TIMESTAMPTZ '1992-02-02 04:03:02Z') FROM timestamps LIMIT 1
SELECT date_trunc(s, d), s FROM timestamps
SELECT datetrunc(s, d), s FROM timestamps
SELECT date_trunc('week', TIMESTAMPTZ '2019-01-06 04:03:02-08') FROM timestamps LIMIT 1
SELECT date_trunc('yearweek', TIMESTAMPTZ '2019-01-06 04:03:02-08') FROM timestamps LIMIT 1
SELECT date_trunc('week', TIMESTAMPTZ '2020-01-01 04:03:02-08') FROM timestamps LIMIT 1
SELECT date_trunc('yearweek', TIMESTAMPTZ '2020-01-01 04:03:02-08') FROM timestamps LIMIT 1
SELECT date_trunc('quarter', TIMESTAMPTZ '2020-12-02 04:03:02-08') FROM timestamps LIMIT 1
SELECT date_trunc('quarter', TIMESTAMPTZ '2019-01-06 04:03:02-08') FROM timestamps LIMIT 1
CREATE TABLE timestamps(ts TIMESTAMPTZ)
SELECT era(ts), year(ts), ts FROM timestamps
CREATE MACRO yeartz(ts) AS year(ts::TIMESTAMPTZ) * (CASE WHEN ERA(ts::TIMESTAMPTZ) > 0 THEN 1 ELSE -1 END)
SELECT ts, mts FROM (SELECT ts, make_timestamptz(yeartz(ts), month(ts), day(ts), hour(ts), minute(ts), microsecond(ts) / 1000000.0) mts FROM timestamps) t WHERE mts IS DISTINCT FROM ts ORDER BY 1
SELECT ts, mts FROM (SELECT ts, make_timestamptz(yeartz(ts), NULL, day(ts), hour(ts), minute(ts), microsecond(ts) / 1000000.0) mts FROM timestamps) t WHERE mts IS NOT NULL
SELECT make_timestamptz(2021, 13, 1, 0, 0, 0) mts
SELECT make_timestamptz(2021, -1, 1, 0, 0, 0) mts
SELECT make_timestamptz(0), make_timestamptz(1684509234845000)
SELECT make_timestamptz(2021, 12, 30, 10, 12, 4.123, 'America/New_York')
SELECT make_timestamptz(NULL, 12, 30, 10, 12, 4.123, 'America/New_York')
SELECT make_timestamptz(2021, NULL, 30, 10, 12, 4.123, 'America/New_York')
SELECT make_timestamptz(2021, 12, NULL, 10, 12, 4.123, 'America/New_York')
CREATE TABLE timestamps AS SELECT ts::TIMESTAMPTZ AS ts FROM (VALUES ('-infinity'), ('0044-03-13 (BC) 10:33:41.987654+01'), ('1962-07-31 12:20:48.123456+00'), ('epoch'), ('2021-01-01 00:00:00+00'), ('2021-02-02 00:00:00+00'), ('2021-11-26 10:15:13.123456+00'), ('2021-11-15 02:30:00-08'), ('2021-11-15 02:30:00-07'), ('2021-12-25 00:00:00+02'), ('infinity'), (NULL), ) tbl(ts)
SELECT ts::VARCHAR FROM timestamps
SELECT ts, strftime(ts, '%Y-%m-%d %H:%M:%S.%f %Z') FROM timestamps
SELECT ts, strftime(ts, '%Z %Y-%m-%d %H:%M:%S.%f') FROM timestamps
CREATE TABLE formats (f VARCHAR)
SELECT strftime('2022-04-07 18:12:15.123456+00'::TIMESTAMPTZ, f) FROM formats
SET TimeZone='Asia/Kathmandu'
SELECT ts, strftime(ts, '%Y-%m-%d %H:%M:%S.%f %z') FROM timestamps
SET TimeZone='Canada/Newfoundland'
SELECT strftime(TIMESTAMPTZ '-204873-8-9 6:35:55 America/North_Dakota/Beulah','x%Z')
SELECT strftime(TIMESTAMPTZ '292555-4-29 18:38:18 Asia/Damascus','e%Z')
select strptime('2022-03-05 17:59:17.877 CST', '%Y-%m-%d %H:%M:%S.%g %Z')
select strptime('2022-03-05 17:59:17.877 CST', NULL)
select strptime(NULL, '%Y-%m-%d %H:%M:%S.%g %Z')
select strptime('2022-03-05 17:59:17.123456 CST', '%Y-%m-%d %H:%M:%S.%f %Z')
select strptime('2022-03-05 17:59:17.123456789 CST', '%Y-%m-%d %H:%M:%S.%n %Z')
select '1582-01-01 10:33:41+01'::timestamptz
select '1582-06-01 10:40:43+01'::timestamptz
select '0044-03-13 (BC) 10:33:41+01'::timestamptz
SELECT '1582-10-10'::TIMESTAMPTZ AS ts
SELECT STRPTIME('2025-09-16', ['%Y-%m-%d', '%Y-%m-%d%z'])
SELECT try_strptime('2022-03-05 17:59:17.877 ' || tz_name, '%m/%d/%Y %H:%M:%S.%g %Z') tstz, tz_name FROM zones WHERE tstz IS NOT NULL ORDER BY ALL
CREATE TABLE multiples (s VARCHAR, f VARCHAR)
CREATE TABLE timestamps_tz(w INTERVAL, t TIMESTAMPTZ, shift INTERVAL, origin TIMESTAMPTZ, timezone VARCHAR)
select t, time_bucket('56 seconds'::interval, t) from timestamps_tz
select t, time_bucket('3 days'::interval, t) from timestamps_tz
select t, time_bucket('3 years'::interval, t) from timestamps_tz
select t, time_bucket(null::interval, t) from timestamps_tz
select time_bucket('3 years'::interval, null::timestamptz) from timestamps_tz
select w, t, time_bucket(w, t) from timestamps_tz
select t, time_bucket('4 seconds'::interval, t, '2 seconds'::interval) from timestamps_tz
select t, time_bucket('4 days'::interval, t, '6 hours'::interval) from timestamps_tz
select t, time_bucket('3 months'::interval, t, '6 days 11 hours'::interval) from timestamps_tz
select t, time_bucket(null::interval, t, '2 seconds'::interval) from timestamps_tz
select time_bucket('3 months'::interval, null::timestamptz, '2 seconds'::interval) from timestamps_tz
CREATE TABLE t1(t TIMESTAMP)
INSERT INTO t1 VALUES (NOW())
INSERT INTO t1 SELECT NOW()
SELECT COUNT(DISTINCT t) FROM t1
PREPARE v1 AS INSERT INTO timestamps VALUES(NOW())
SELECT COUNT(DISTINCT ts) FROM timestamps
CREATE TABLE timestamps_default(ts TIMESTAMP DEFAULT NOW())
INSERT INTO timestamps_default DEFAULT VALUES
SELECT COUNT(DISTINCT ts) FROM timestamps_default
SELECT strftime(d, '%a') FROM timestamps ORDER BY d
SELECT strftime(d, '%A') FROM timestamps ORDER BY d
SELECT strftime(d, '%w') FROM timestamps ORDER BY d
SELECT strftime(d, '%u') FROM timestamps ORDER BY d
SELECT strftime(d, '%d') FROM timestamps ORDER BY d
SELECT strftime(d, '%-d') FROM timestamps ORDER BY d
SELECT strftime(d, '%b') FROM timestamps ORDER BY d
SELECT strftime(d, '%h') FROM timestamps ORDER BY d
SELECT strftime(d, '%B') FROM timestamps ORDER BY d
SELECT strftime(d, '%m') FROM timestamps ORDER BY d
SELECT strftime(d, '%-m') FROM timestamps ORDER BY d
SELECT strftime(d, '%y') FROM timestamps ORDER BY d
SELECT strftime(d, '%-y') FROM timestamps ORDER BY d
SELECT strftime(DATE '2001-01-01', '%-y')
SELECT strftime(d, '%Y') FROM timestamps ORDER BY d
SELECT strftime(d, '%G') FROM timestamps ORDER BY d
SELECT strftime(d, '%H') FROM timestamps ORDER BY d
SELECT strftime(d, '%-H') FROM timestamps ORDER BY d
SELECT strftime(d, '%I') FROM timestamps ORDER BY d
SELECT strftime(d, '%-I') FROM timestamps ORDER BY d
SELECT strftime(d, '%p') FROM timestamps ORDER BY d
SELECT strftime(d, '%M') FROM timestamps ORDER BY d
SELECT strftime(d, '%-M') FROM timestamps ORDER BY d
SELECT strftime(d, '%S') FROM timestamps ORDER BY d
SELECT strptime('21 June, 2018', '%d %B, %Y')
SELECT strptime('21/10/2018', '%d/%m/%Y')
SELECT strptime('2018-20-10', '%Y-%d-%m')
SELECT strptime('20182010', '%Y%d%m')
SELECT strptime('Mon 30, June 2003, 12:03:10 AM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT strptime('Mon 30, June 2003, 12:03:10 PM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT strptime('Mon 30, December 2003, 7:3:5 PM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT strptime('Tuesday 30, December 2003, 7:3:5 PM', '%A %d, %B %Y, %I:%M:%S %p')
SELECT strptime('Mon 30, December 30, 7:3:5 PM', '%a %d, %B %y, %I:%M:%S %p')
SELECT strptime('Mon 30, June 2003, 12:03:10 AM', '%a %-d, %B %Y, %-I:%-M:%-S %p')
SELECT strptime('mon', '%a')
SELECT strptime('tuesday', '%A')
CREATE TABLE timestamps(w INTERVAL, t TIMESTAMP, shift INTERVAL, origin TIMESTAMP)
select t, time_bucket('56 seconds'::interval, t) from timestamps
select t, time_bucket('3 days'::interval, t) from timestamps
select t, time_bucket('3 years'::interval, t) from timestamps
select t, time_bucket(null::interval, t) from timestamps
select time_bucket('3 years'::interval, null::timestamp) from timestamps
select w, t, time_bucket(w, t) from timestamps
select t, time_bucket('4 seconds'::interval, t, '2 seconds'::interval) from timestamps
select t, time_bucket('4 days'::interval, t, '6 hours'::interval) from timestamps
select t, time_bucket('3 months'::interval, t, '6 days 11 hours'::interval) from timestamps
select t, time_bucket(null::interval, t, '2 seconds'::interval) from timestamps
select time_bucket('3 months'::interval, null::timestamp, '2 seconds'::interval) from timestamps
SELECT try_strptime('21 June, 2018', '%d %B, %Y')
SELECT try_strptime('21/10/2018', '%d/%m/%Y')
SELECT try_strptime('2018-20-10', '%Y-%d-%m')
SELECT try_strptime('20182010', '%Y%d%m')
SELECT try_strptime('Mon 30, June 2003, 12:03:10 AM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT try_strptime('Mon 30, June 2003, 12:03:10 PM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT try_strptime('Mon 30, December 2003, 7:3:5 PM', '%a %d, %B %Y, %I:%M:%S %p')
SELECT try_strptime('Tuesday 30, December 2003, 7:3:5 PM', '%A %d, %B %Y, %I:%M:%S %p')
SELECT try_strptime('Mon 30, December 30, 7:3:5 PM', '%a %d, %B %y, %I:%M:%S %p')
SELECT try_strptime('Mon 30, June 2003, 12:03:10 AM', '%a %-d, %B %Y, %-I:%-M:%-S %p')
SELECT try_strptime('mon', '%a')
SELECT try_strptime('tuesday', '%A')
CREATE TABLE timetzs(d TIMETZ, s VARCHAR)
SELECT date_part(NULL::VARCHAR, NULL::TIMETZ) FROM timetzs
SELECT date_part(s, NULL::TIMETZ) FROM timetzs
SELECT date_part(NULL, d) FROM timetzs
SELECT date_part(s, '14:28:50.447+07:15'::TIMETZ) FROM timetzs
SELECT date_part('hour', d) FROM timetzs
SELECT date_part(s, d) FROM timetzs
SELECT d, DATE_PART(['hour', 'minute', 'microsecond'], d) AS parts FROM timetzs ORDER BY 1
SELECT d, DATE_PART(['epoch', 'second', 'timezone', 'timezone_hour', 'timezone_minute'], d) AS parts FROM timetzs ORDER BY 1
SELECT d, epoch_ns(d) FROM timetzs ORDER BY ALL
SELECT d, epoch_us(d) FROM timetzs ORDER BY ALL
SELECT d, epoch_ms(d) FROM timetzs ORDER BY ALL
CREATE TABLE timetzs (i TIMETZ)
INSERT INTO timetzs VALUES (NULL), ('00:00:00+1559'), ('00:00:00+1558'), ('02:30:00'), ('02:30:00+04'), ('02:30:00+04:30'), ('02:30:00+04:30:45'), ('16:15:03.123456'), ('02:30:00+1200'), ('02:30:00-1200'), ('24:00:00-1558'), ('24:00:00-1559'),
SELECT EXTRACT(second FROM i) FROM timetzs
SELECT EXTRACT(minute FROM i) FROM timetzs
SELECT EXTRACT(hour FROM i) FROM timetzs
SELECT EXTRACT(milliseconds FROM i) FROM timetzs
SELECT EXTRACT(microseconds FROM i) FROM timetzs
SELECT EXTRACT(epoch FROM i) FROM timetzs
SET CALENDAR='gregorian'
SET TIMEZONE='America/Phoenix'
SELECT '01:00:00'::TIMETZ AS ttz
SELECT '01:00:00+02'::TIMETZ AS ttz
create table timetzs (ttz TIMETZ)
create table time_testtz as select i::timetz as t from generate_series(TIMESTAMPtz '2001-04-10', TIMESTAMPtz '2001-04-11', INTERVAL 30 MINUTE) as t(i)
SELECT TYPEOF(t) FROM (select t from time_testtz group by t) LIMIT 1
CREATE TEMPORARY TABLE t1 AS SELECT gen_random_uuid() a FROM range(0, 16)
CREATE TEMPORARY TABLE t2 AS SELECT uuid() b FROM range(0, 16)
CREATE TEMPORARY TABLE t3 AS SELECT gen_random_uuid() c FROM range(0, 16)
CREATE TABLE uuids(u UUID NOT NULL DEFAULT gen_random_uuid(), a INTEGER)
INSERT INTO uuids (a) VALUES (1), (2), (3), (4), (5), (6), (7), (8), (9), (10)
SELECT COUNT(DISTINCT u) FROM uuids
SELECT * FROM uuids ORDER BY gen_random_uuid()
SELECT DISTINCT substring(uuid()::varchar, 15, 1) FROM range(100)
SELECT DISTINCT substring(uuidv4()::varchar, 15, 1) FROM range(100)
SELECT DISTINCT substring(uuidv7()::varchar, 15, 1) FROM range(100)
SELECT DISTINCT substring(uuid()::varchar, 20, 1) AS x FROM range(100) ORDER BY x
SELECT uuid_extract_version('ac227128-7d55-7ee0-a765-5025cc52e55a')
SELECT uuid_extract_version(uuidv7())
SELECT uuid_extract_version('ac227128-7d55-4ee0-a765-5025cc52e55a')
SELECT uuid_extract_version(uuidv4())
SELECT uuid_extract_version(gen_random_uuid())
SELECT uuid_extract_timestamp('0196f97a-db14-71c3-9132-9f0b1334466f')
SELECT datediff('month', uuid_extract_timestamp(uuidv7()), now())
SELECT base64(encode(''))
SELECT base64(encode('a'))
SELECT base64(encode('ab'))
SELECT base64(encode('abc'))
SELECT base64(encode('üäabcdef'))
SELECT base64(encode('iJWERiuhjruhwuiehr8493231'))
SELECT base64(encode('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890'))
SELECT to_base64(encode('base64 encoded string'))
SELECT from_base64(base64(encode('')))
SELECT from_base64(base64(encode('a')))
SELECT from_base64(base64(encode('ab')))
SELECT from_base64(base64(encode('abc')))
SELECT * FROM integers ORDER BY create_sort_key(i, 'ASC NULLS LAST')
SELECT * FROM integers ORDER BY create_sort_key(i, 'ASC NULLS FIRST')
SELECT * FROM integers ORDER BY create_sort_key(i, 'DESC NULLS LAST')
SELECT * FROM integers ORDER BY create_sort_key(i, 'DESC NULLS FIRST')
INSERT INTO varchars VALUES ('hello'), ('hello' || chr(0) || chr(0)), ('world'), (''), (NULL)
SELECT * FROM varchars ORDER BY create_sort_key(v, 'ASC NULLS LAST')
SELECT * FROM varchars ORDER BY create_sort_key(v, 'ASC NULLS FIRST')
SELECT * FROM varchars ORDER BY create_sort_key(v, 'DESC NULLS LAST')
SELECT * FROM varchars ORDER BY create_sort_key(v, 'DESC NULLS FIRST')
CREATE TABLE int_list(l INT[])
INSERT INTO int_list VALUES ([1, 2, 3]), ([]), ([1]), ([2]), ([NULL]), (NULL)
SELECT l FROM int_list ORDER BY create_sort_key(l, 'ASC NULLS LAST')
SELECT encode('ü')
SELECT decode(encode('ü'))
SELECT decode(encode(a)) || a from (values ('hello'), ('world')) tbl(a)
select array_slice(NULL::BLOB, 4, 6)
SELECT i+2=5, 5=i+2 FROM integers ORDER BY i
SELECT 2+i=5, 5=2+i FROM integers ORDER BY i
SELECT i*2=6, 6=i*2 FROM integers ORDER BY i
SELECT 2*i=6, 6=2*i FROM integers ORDER BY i
SELECT i*2=5 FROM integers ORDER BY i
SELECT i*0=5 FROM integers ORDER BY i
SELECT -i>-2 FROM integers ORDER BY i
SELECT i-2=1, 1=i-2 FROM integers ORDER BY i
SELECT 3-i=1, 1=3-i FROM integers ORDER BY i
SELECT 3-i<2, 2>3-i FROM integers ORDER BY i
SELECT 3-i<=1, 1>=3-i FROM integers ORDER BY i
SELECT i//2=1, 1=i//2 FROM integers ORDER BY i
CREATE TABLE tab0(col0 INTEGER, col1 INTEGER, col2 INTEGER)
CREATE TABLE tab1(col0 INTEGER, col1 INTEGER, col2 INTEGER)
CREATE TABLE tab2(col0 INTEGER, col1 INTEGER, col2 INTEGER)
INSERT INTO tab0 VALUES(97,1,99)
INSERT INTO tab0 VALUES(15,81,47)
INSERT INTO tab0 VALUES(87,21,10)
INSERT INTO tab1 VALUES(51,14,96)
INSERT INTO tab1 VALUES(85,5,59)
INSERT INTO tab1 VALUES(91,47,68)
INSERT INTO tab2 VALUES(64,77,40)
INSERT INTO tab2 VALUES(75,67,58)
INSERT INTO tab2 VALUES(46,51,23)
SELECT 1 << 2, NULL << 2, 2 << NULL
SELECT 16 >> 2, 1 >> 2, NULL >> 2, 2 >> NULL
SELECT 1 & 1, 1 & 0, 0 & 0, NULL & 1, 1 & NULL
SELECT 1 | 1, 1 | 0, 0 | 0, NULL | 1, 1 | NULL
SELECT xor(1, 1), xor(1, 0), xor(0, 0), xor(NULL, 1), xor(1, NULL)
SELECT 1::UTINYINT << 7, 1::USMALLINT << 15, 1::UINT32 << 31, 1::UBIGINT << 63
CREATE TABLE bitwise_test(i TINYINT, j TINYINT)
INSERT INTO bitwise_test VALUES (1, 1), (1, 0), (0, 1), (0, 0), (1, NULL), (NULL, 1), (NULL, NULL)
SELECT i << j, i >> j, i & j, i | j, xor(i, j) FROM bitwise_test
CREATE TABLE bitwise_test(i SMALLINT, j SMALLINT)
CREATE TABLE bitwise_test(i INTEGER, j INTEGER)
CREATE TABLE bitwise_test(i BIGINT, j BIGINT)
SELECT 1 == 1, 1 = 1, 1 == 0, 1 = 0, 1 == NULL
SELECT 1 <> 1, 1 != 1, 1 <> 0, 1 != 0, 1 <> NULL
select '1000' > 20
select '1000' > '20'
select ('abc' between '20' and 'true')
CREATE TABLE a (i integer, j integer)
INSERT INTO a VALUES (3, 4), (4, 5), (5, 6)
SELECT * FROM a WHERE (i > 3 AND j < 5) OR (i > 3 AND j > 5)
explain SELECT * FROM a WHERE (i > 3 AND j < 5) OR (i > 3 AND j > 5)
SELECT true AND true
SELECT true AND false
SELECT false AND true
SELECT false AND false
SELECT false AND NULL
SELECT NULL AND false
SELECT NULL AND true
SELECT true AND NULL
INSERT INTO dates VALUES ('1992-01-01'), ('1992-03-03'), ('1992-05-05'), ('2022-01-01'), ('044-03-15 (BC)'), (NULL)
CREATE TABLE times(t TIME)
SELECT d, t, d + t FROM dates, times ORDER BY 1, 2
SELECT d, t, t + d FROM dates, times ORDER BY 1, 2
CREATE TABLE timetzs(ttz TIMETZ)
INSERT INTO timetzs VALUES ('00:01:20+00'), ('20:08:10.998-07'), ('20:08:10.33+12'), ('20:08:10.001-1559'), (NULL)
SELECT d, ttz, d + ttz FROM dates, timetzs ORDER BY 1, 2
SELECT d, ttz, ttz + d FROM dates, timetzs ORDER BY 1, 2
SELECT (-127)::TINYINT // (-1)::TINYINT
SELECT (-32767)::SMALLINT // (-1)::SMALLINT
SELECT (-2147483647)::INTEGER // (-1)::INTEGER
SELECT (-9223372036854775807)::BIGINT // (-1)::BIGINT
CREATE OR REPLACE TABLE test (a INTEGER)
SELECT * FROM test WHERE a IN ('a', 'b', 'c', 'd', 'e')
INSERT INTO test VALUES (42)
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
INSERT INTO Cities VALUES ('NL', 'Amsterdam', 2000, 1005)
INSERT INTO Cities VALUES ('NL', 'Amsterdam', 2010, 1065)
INSERT INTO Cities VALUES ('NL', 'Amsterdam', 2020, 1158)
INSERT INTO Cities VALUES ('US', 'Seattle', 2000, 564)
INSERT INTO Cities VALUES ('US', 'Seattle', 2010, 608)
INSERT INTO Cities VALUES ('US', 'Seattle', 2020, 738)
INSERT INTO Cities VALUES ('US', 'New York City', 2000, 8015)
INSERT INTO Cities VALUES ('US', 'New York City', 2010, 8175)
INSERT INTO Cities VALUES ('US', 'New York City', 2020, 8772)
PIVOT Cities USING SUM(Population)
PIVOT Cities USING SUM(Population) GROUP BY Country
create table p (col1 timestamp, col2 int)
INSERT INTO p VALUES ('2024-12-04 09:30:01', 100), ('2024-12-04 09:30:02', 100), ('2024-12-04 09:30:03', 100), ('2024-12-04 09:30:04', 100), ('2024-12-04 09:30:05', 100), ('2024-12-04 09:30:06', 100), ('2024-12-04 09:30:07', 100), ('2024-12-04 09:30:08', 100)
pivot p using sum (col2) group by col1 order by col1
CREATE TABLE cpb_tbl AS WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) FROM CPB
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
CREATE OR REPLACE TABLE Produce AS SELECT 'Kale' as product, 51 as Q1, 23 as Q2, 45 as Q3, 3 as Q4 UNION ALL SELECT 'Apple', 77, 0, 25, 2
SELECT * FROM Produce UNPIVOT(sales FOR quarter IN (Q1, Q2, Q3, Q4)) ORDER BY ALL
SELECT product, first_half_sales, second_half_sales, semesters FROM Produce UNPIVOT( (first_half_sales, second_half_sales) FOR semesters IN ((Q1, Q2) AS 'semester_1', (Q3, Q4) AS 'semester_2'))
SET pivot_filter_threshold=1
FROM Cities PIVOT ( array_agg(id) FOR name IN ('test','Test') )
FROM Cities PIVOT ( array_agg(id), sum(id) FOR name IN ('test','Test') )
SELECT year, region, q1, q2, q3, q4 FROM sales PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1, q2, q3, q4 FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_total, q1_avg, q2_total, q2_avg, q3_total, q3_avg, q4_total, q4_avg FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) AS total, avg(sales) AS avg FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT * FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales), avg(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
CREATE OR REPLACE TEMPORARY VIEW sales(location, year, q1, q2, q3, q4) AS VALUES ('Toronto' , 2020, 100 , 80 , 70, 150), ('San Francisco', 2020, NULL, 20 , 50, 60), ('Toronto' , 2021, 110 , 90 , 80, 170), ('San Francisco', 2021, 70 , 120, 85, 105)
SELECT * FROM sales UNPIVOT INCLUDE NULLS (sales FOR quarter IN (q1 AS "Jan-Mar", q2 AS "Apr-Jun", q3 AS "Jul-Sep", q4 AS "Oct-Dec"))
SELECT * FROM oncall UNPIVOT ((name, email, phone) FOR precedence IN ((name1, email1, phone1) AS primary, (name2, email2, phone2) AS secondary))
PIVOT Cities ON Country USING SUM(Population)
PIVOT Cities ON Country, Name USING SUM(Population)
PIVOT Cities ON Country IN ('xx') USING SUM(Population)
PIVOT Cities ON (Country, Name) IN ('xx') USING SUM(Population)
PIVOT Cities ON Country IN ('xx', 'yy') USING SUM(Population)
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
INSERT INTO monthly_sales VALUES (1, 10000, 'JAN'), (1, 400, 'JAN'), (2, 4500, 'JAN'), (2, 35000, 'JAN'), (1, 5000, 'FEB'), (1, 3000, 'FEB'), (2, 200, 'FEB'), (2, 90500, 'FEB'), (1, 6000, 'MAR'), (1, 5000, 'MAR'), (2, 2500, 'MAR'), (2, 9500, 'MAR'), (1, 8000, 'APR'), (1, 10000, 'APR'), (2, 800, 'APR'), (2, 4500, 'APR')
CREATE TYPE unique_months AS ENUM (SELECT DISTINCT month FROM monthly_sales ORDER BY CASE month WHEN 'JAN' THEN 1 WHEN 'FEB' THEN 2 WHEN 'MAR' THEN 3 ELSE 4 END)
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_months) AS p ORDER BY EMPID
CREATE TYPE not_an_enum AS VARCHAR
CREATE TABLE test(i INT, j VARCHAR)
SET pivot_filter_threshold=0
SET pivot_filter_threshold=100
PIVOT Cities ON Country || '_' || Name USING SUM(Population) GROUP BY Year
PIVOT Cities ON (CASE WHEN Country='NL' THEN NULL ELSE Country END) USING SUM(Population) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING COALESCE(SUM(Population), 0) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population)::VARCHAR GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + 42 GROUP BY Year
CREATE TABLE Product(DaysToManufacture int, StandardCost int GENERATED ALWAYS AS (DaysToManufacture * 5))
INSERT INTO Product VALUES (0), (1), (2), (4)
SELECT 'AverageCost' AS Cost_Sorted_By_Production_Days, "0", "1", "2", "3", "4" FROM ( SELECT DaysToManufacture, StandardCost FROM Product ) AS SourceTable PIVOT ( AVG(StandardCost) FOR DaysToManufacture IN (0, 1, 2, 3, 4) ) AS PivotTable
pivot cities on (Country='NL') using avg(Population) group by name
pivot cities on (Country='NL') in (false, true) using avg(Population) group by name
PIVOT Cities ON Year IN (SELECT Year FROM Cities ORDER BY Year DESC) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT YEAR FROM (SELECT Year, SUM(POPULATION) AS popsum FROM Cities GROUP BY Year ORDER BY popsum DESC)) USING SUM(Population)
PIVOT Cities ON Year IN (SELECT '2010' UNION ALL SELECT '2000' UNION ALL SELECT '2020') USING SUM(Population)
INSERT INTO monthly_sales VALUES (1, 10000, '1-JAN'), (1, 400, '1-JAN'), (2, 4500, '1-JAN'), (2, 35000, '1-JAN'), (1, 5000, '2-FEB'), (1, 3000, '2-FEB'), (2, 200, '2-FEB'), (2, 90500, '2-FEB'), (2, 2500, '3-MAR'), (2, 9500, '3-MAR'), (1, 8000, '4-APR'), (1, 10000, '4-APR'), (2, 800, '4-APR'), (2, 4500, '4-APR')
PIVOT monthly_sales ON MONTH USING COALESCE(SUM(AMOUNT), 0)
SELECT mode(column_type) FROM (DESCRIBE PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)::INTEGER)
INSERT INTO monthly_sales VALUES (1, 10000, '1-JAN'), (1, 400, '1-JAN'), (2, 4500, '1-JAN'), (2, 35000, '1-JAN'), (1, 5000, '2-FEB'), (1, 3000, '2-FEB'), (2, 200, '2-FEB'), (2, 90500, '2-FEB'), (1, 6000, '3-MAR'), (1, 5000, '3-MAR'), (2, 2500, '3-MAR'), (2, 9500, '3-MAR'), (1, 8000, '4-APR'), (1, 10000, '4-APR'), (2, 800, '4-APR'), (2, 4500, '4-APR')
PREPARE v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount + ?) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR')) AS p ORDER BY EMPID
EXECUTE v1(0)
PREPARE v2 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT + ?)
CREATE TABLE t(c VARCHAR, v INTEGER)
INSERT INTO t VALUES ('a', 1), ('b', 2)
CREATE TABLE captured AS PIVOT (SELECT c, v, current_query() AS q FROM t) ON c USING ANY_VALUE(q) ORDER BY v
SELECT v, regexp_replace(a, '__pivot_enum_[0-9a-f\-]+', '__pivot_enum_X') AS a, regexp_replace(b, '__pivot_enum_[0-9a-f\-]+', '__pivot_enum_X') AS b FROM captured ORDER BY v
CREATE TABLE t(id INT, jan INT, feb INT)
CREATE VIEW poison_view AS SELECT * FROM t UNPIVOT (val FOR col IN (*))
CREATE VIEW expr_view AS SELECT * FROM t UNPIVOT (val FOR col IN (1+2+id))
CREATE VIEW v AS PIVOT t ON id IN (CASE WHEN true THEN 'a' END) USING (SUM(feb))
CREATE VIEW v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
CREATE MACRO pivot_macro(val) as TABLE SELECT * FROM monthly_sales PIVOT(SUM(amount + val) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
FROM v1
FROM pivot_macro(1)
create table donnees_csv as select {'year': i::varchar, 'month': i::varchar} AS donnee, i%5 as variable_id, i%10 id_niv from range(1000) t(i)
pivot donnees_csv on variable_id using first(donnee) group by id_niv order by all
CREATE OR REPLACE TABLE sales(empid INT, amount INT, d DATE)
INSERT INTO sales VALUES (1, 10000, DATE '2000-01-01'), (1, 400, DATE '2000-01-07'), (2, 4500, DATE '2001-01-21'), (2, 35000, DATE '2001-01-21'), (1, 5000, DATE '2000-02-03'), (1, 3000, DATE '2000-02-07'), (2, 200, DATE '2001-02-05'), (2, 90500, DATE '2001-02-19'), (1, 6000, DATE '2000-03-01'), (1, 5000, DATE '2000-03-09'), (2, 2500, DATE '2001-03-03'), (2, 9500, DATE '2001-03-08')
PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT) ORDER BY ALL
PIVOT (PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)) ON empid USING SUM(COALESCE("2000_1",0) + COALESCE("2000_2",0) + COALESCE("2000_3",0) + COALESCE("2001_1",0) + COALESCE("2001_2",0) + COALESCE("2001_3",0))
CREATE OR REPLACE TABLE sales(empid INT, amount INT, month TEXT, year INT)
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount + year) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SET pivot_limit=10000
CREATE TABLE Product(DaysToManufacture int, StandardCost int)
INSERT INTO Product VALUES (0, 5.0885), (1, 223.88), (2, 359.1082), (4, 949.4105)
SELECT DaysToManufacture, AVG(StandardCost) AS AverageCost FROM Product GROUP BY DaysToManufacture
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount+1) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(COUNT(*) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC') GROUP BY empid) AS p ORDER BY EMPID
SELECT empid, January, February, March, April FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN' AS January, 'FEB' AS February, 'MAR' AS March, 'APR' AS April)) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
INSERT INTO monthly_sales VALUES (1, 250, NULL)
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (NULL, 'JAN', 'FEB', 'MAR', 'APR')) AS p ORDER BY EMPID
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
CREATE OR REPLACE TABLE monthly_sales(empid INT, dept TEXT, Jan INT, Feb INT, Mar INT, April INT)
INSERT INTO monthly_sales VALUES (1, 'electronics', 100, 200, 300, 100), (2, 'clothes', 100, 300, 150, 200), (3, 'cars', 200, 400, 100, 50)
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) ORDER BY empid
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan AS January, feb AS February, mar AS March, april)) ORDER BY empid
SELECT p.id, p.type, p.m, p.vals FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) AS p(id, type, m, vals)
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), (mar, april)))
UNPIVOT (SELECT * FROM monthly_sales) ON jan, feb, mar april INTO NAME month VALUE sales
CREATE TABLE t1(id BIGINT, "Sales (05/19/2020)" BIGINT, "Sales (06/03/2020)" BIGINT, "Sales (10/23/2020)" BIGINT)
INSERT INTO t1 VALUES(10629465, 23, 47, 99)
INSERT INTO t1 VALUES(98765432, 10, 99, 33)
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
ALTER TABLE monthly_sales ADD COLUMN status VARCHAR
UPDATE monthly_sales SET status=CASE WHEN amount >= 10000 THEN 'important' ELSE 'regular' END
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)) ORDER BY ALL
PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY status) ORDER BY ALL
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') GROUP BY status) AS p ORDER BY 1
WITH pivoted_sales AS (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) SELECT * FROM pivoted_sales ORDER BY empid DESC
unpivot (select 42 as col1, 'woot' as col2) on col1::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on COLUMNS(*)::VARCHAR
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR AS c, col2
select * from (select 42 as col1, 'woot' as col2) UNPIVOT ("value" FOR "name" IN (col1::VARCHAR, col2))
CREATE TABLE unpivot_names(unpivot_names VARCHAR, unpivot_list VARCHAR, unpivot_list_2 VARCHAR, col1 INT, col2 INT, col3 INT)
INSERT INTO unpivot_names VALUES ('unpivot_names', 'unpivot_list', 'unpivot_list_2', 1, 2, 3)
UNPIVOT unpivot_names ON COLUMNS('col*')
create table integers(i integer)
CREATE TABLE test(id BIGINT, metric_1 VARCHAR, value_x VARCHAR, metric_2 VARCHAR, value_q VARCHAR, metric_3 VARCHAR, value_j VARCHAR)
INSERT INTO test VALUES(1,'a','a_value','b','b_value','c','c_value')
INSERT INTO test VALUES(2,'d','d_value','e','e_value','f','f_value')
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUES metric_value, metric_type
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select 42) on columns(*))
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select {n : 1 }) on columns(*))
unpivot (select cast(columns(*) as varchar) from (select 42 as col1, 'woot' as col2)) on columns(*)
CREATE TABLE t ( id INTEGER, a INTEGER, b INTEGER )
INSERT INTO t VALUES (1, 10, 20)
CREATE VIEW v AS SELECT * FROM t UNPIVOT (val FOR key IN (a, b))
FROM v
prepare v1 as select $2::int
prepare v2 as select $1::int
prepare v3 as select $1::int where 1=0
execute v3(1)
CREATE TABLE t1(c0 INT2)
INSERT INTO t1 VALUES (1), (2), (3), (5), (7), (10), (20)
PREPARE prepare_query AS SELECT (? NOT IN (SELECT t1.c0 FROM t1 ORDER BY t1.c0 LIMIT ?))
EXECUTE prepare_query(-675986880, 1)
PREPARE q1 AS SELECT (? NOT IN (SELECT t1.c0 FROM t1 ORDER BY t1.c0 LIMIT ?)) AND (? IN (SELECT t1.c0 FROM t1 ORDER BY t1.c0 LIMIT ?))
EXECUTE q1(10, 1, 20, 3)
PREPARE q2 AS SELECT ? IN ( SELECT t1.c0 FROM t1 WHERE t1.c0 IN (SELECT t1.c0 FROM t1 ORDER BY t1.c0 LIMIT ?) AND t1.c0 > ? ORDER BY t1.c0 LIMIT ? )
EXECUTE q2(7, 5, 3, 2)
PREPARE q3 AS SELECT ? = ANY ( SELECT t1.c0 FROM t1 WHERE t1.c0 = ALL (SELECT t1.c0 FROM t1 ORDER BY t1.c0 LIMIT ?) ORDER BY t1.c0 LIMIT ? )
EXECUTE q3(1, 1, 2)
PREPARE q4 AS SELECT (? NOT IN ( SELECT t1.c0 FROM t1 WHERE t1.c0 < ? ORDER BY t1.c0 LIMIT ? ))
EXECUTE q4(10, 5, 2)
PREPARE s1 AS SELECT CAST(? AS INTEGER), CAST(? AS STRING)
EXECUTE s1(42, 'dpfkg')
DEALLOCATE s1
PREPARE s1 AS SELECT CAST(?1 AS INTEGER), CAST(?2 AS STRING)
PREPARE s1 AS SELECT CAST(?2 AS INTEGER), CAST(?1 AS STRING)
EXECUTE s1('dpfkg', 42)
execute q1 (42)
PREPARE q2 AS COPY ( select 42 as 'col' ) to $1 ( FORMAT csv )
PREPARE v1 AS SELECT ?
EXECUTE v1(27)
EXECUTE v1('hello world')
EXECUTE v1([1, 2, 3])
PREPARE v2 AS SELECT ?=?
EXECUTE v2(27, 27)
EXECUTE v2('hello world', 'hello mars')
EXECUTE v2(1, 1.0)
EXECUTE v2([1, 2, 3], '[1, 2, 3]')
PREPARE v3 AS SELECT (SELECT ?)
EXECUTE v3(27)
EXECUTE v3('hello world')
prepare fromFirst as from (select ? fromV) select ? selectV,*
execute fromFirst('from', 'sel')
from (select 'from' fromV) select 'sel' selectV,*
create table test(id varchar)
prepare p as delete from test where ("id") in ((?))
execute p(null)
PREPARE v1 AS SELECT list_transform(?, lambda x: x + 1)
PREPARE v2 AS SELECT list_transform([1, 2, 3], lambda x: x + ?)
PREPARE v3 AS SELECT list_transform(?, lambda x: x + ? + ?)
EXECUTE v3([1, 2, 3], 1, 1)
PREPARE v4 AS SELECT list_filter(?, lambda x: x > 1)
EXECUTE v4([1, 2, 3])
PREPARE v5 AS SELECT list_filter([1, 2, 3], lambda x: x > ?)
EXECUTE v5(1)
PREPARE v6 AS SELECT list_filter(?, lambda x: x > ? AND ?)
EXECUTE v6([1, 2, 3], 1, True)
PREPARE v1 AS SELECT list_aggregate(?, 'min')
EXECUTE v1(['hello', 'world'])
EXECUTE v1(NULL::INT[])
PREPARE v2 AS SELECT array_slice(?, 1, 2)
EXECUTE v2([1, 2, 3])
EXECUTE v2('123')
PREPARE v3 AS SELECT flatten(?)
EXECUTE v3([[1,2,3],[4,5]])
PREPARE v4 AS SELECT list_extract(?, 2)
prepare v1 as select cast(111 as short) * $1
execute v1(1665::BIGINT)
PREPARE v1 AS SELECT $1::INT, $1::BIGINT
EXECUTE v1(42)
PREPARE v2 AS SELECT $1::BIGINT, $1::INT
EXECUTE v2(42)
PREPARE v3 AS SELECT $1::BIGINT, $1::UBIGINT
EXECUTE v3(42)
PREPARE v4 AS SELECT $1::VARCHAR, $1::DATE
EXECUTE v4('1992-01-01')
PREPARE v5 AS SELECT $1::INT, $1::BIGINT, $1::TINYINT, $1::HUGEINT, $1::SMALLINT
EXECUTE v5(42)
PREPARE v6 AS SELECT $1::INT, $1::BIGINT, $1::TINYINT, $1::UBIGINT, $1::SMALLINT, $1::UHUGEINT
EXECUTE v6(42)
PREPARE q AS SELECT x FROM generate_series(1, 10) t(x) OFFSET ? LIMIT ?
EXECUTE q(3, 5)
SELECT x FROM generate_series(1, 10) t(x) OFFSET 3 LIMIT 5
CREATE TABLE accounts AS SELECT 1 id, 'Mark' AS name
SUMMARIZE SELECT * FROM accounts WHERE id = 1
PREPARE query AS SUMMARIZE SELECT * FROM accounts WHERE id = $1
EXECUTE query(1)
PREPARE query AS (SUMMARIZE SELECT * FROM accounts WHERE id = $1)
DESCRIBE SELECT * FROM accounts WHERE id = 1
PREPARE query AS DESCRIBE SELECT * FROM accounts WHERE id = $1
PREPARE query AS (DESCRIBE SELECT * FROM accounts WHERE id = $1)
PREPARE v1 AS SELECT SUM(?) OVER ()
EXECUTE v1(2::HUGEINT)
EXECUTE v1(0.5)
CREATE TABLE v0 ( v2 INTEGER CHECK( v2 BETWEEN 1 AND 1119 ) , v1 INT )
INSERT INTO v0 ( v2 ) VALUES ( 10 )
PREPARE q1 AS SELECT COALESCE ( LEAD ( $1 ) OVER( ) , ( v1 ) ) > $1 FROM v0
EXECUTE q1(1)
prepare q123 as select $param, $other_name, $param
execute q123(param := 5, other_name := 3)
prepare q01 as select $1, ?, $2
PREPARE v1 AS SELECT COALESCE(COALESCE(NULL, $1) / 42::BIGINT, 0.5)
PREPARE v2 AS SELECT COALESCE(CASE WHEN FALSE THEN $1 ELSE NULL END / 42::BIGINT, 0.5)
PREPARE s1 AS SELECT ?::VARCHAR FROM (SELECT ?::INTEGER) tbl(i) WHERE i > ?::INTEGER
EXECUTE s1('hello', 2, 1)
PREPARE s2 AS SELECT FIRST(?::VARCHAR) FROM (VALUES (?::INTEGER)) tbl(i) WHERE i > ?::INTEGER GROUP BY i % ?::INTEGER HAVING SUM(i)::VARCHAR <> ?::VARCHAR
EXECUTE s2('hello', 2, 1, 2, 'blabla')
EXECUTE s2('hello', 2, 1, 2, '2')
PREPARE s3 AS SELECT LENGTH(?::VARCHAR) UNION ALL SELECT ?::INTEGER ORDER BY 1
EXECUTE s3('hello', 3)
PREPARE s4 AS SELECT ?::INTEGER IN (?::INTEGER, ?::INTEGER, ?::INTEGER)
EXECUTE s4(1, 2, 3, 1)
PREPARE s5 AS SELECT ?::INTEGER IN (SELECT i FROM (VALUES (?::INTEGER), (?::INTEGER), (?::INTEGER)) tbl(i))
EXECUTE s5(1, 2, 3, 1)
CREATE TABLE integers(i INTEGER, j VARCHAR)
INSERT INTO integers VALUES (1, 'hello')
PREPARE s1 AS UPDATE integers SET i=?, j=?
EXECUTE s1(2, 'world')
PREPARE s2 AS UPDATE integers SET j=? WHERE i=?
EXECUTE s2('test', 2)
PREPARE s3 AS UPDATE integers SET j=? WHERE i=? AND j=?
EXECUTE s3('test2', 2, 'test')
PREPARE s1 AS SELECT CAST($1 AS INTEGER), CAST($2 AS STRING)
EXECUTE s1(43, 'asdf')
DEALLOCATE s2
PREPARE s1 AS SELECT $1+$2
PREPARE s1 AS SELECT NOT($1), 10+$2, $3+20, 4 IN (2, 3, $4), $5 IN (2, 3, 4)
EXECUTE s1(1, 2, 3, 4, 2)
PREPARE s1 AS SELECT $1
PREPARE s2 AS SELECT (SELECT $1)
PREPARE s3 AS SELECT $1=$2
PREPARE v1 AS SELECT ? + 1.0 AS a
EXECUTE v1(2.0)
PREPARE v2 AS SELECT ? * 2.0 AS a
EXECUTE v2(2.0)
PREPARE v3 AS SELECT ? = 2.0 AS a
EXECUTE v3(2.0)
PREPARE v4 AS SELECT 2.0 IN (1.0, 1.5, ?)
EXECUTE v4(2.0)
EXECUTE v4(2.5)
PREPARE v5 AS SELECT ? IN (1.0, 1.5, 2.0)
EXECUTE v5(2.0)
EXECUTE v5(2.5)
CREATE table T1(A0 TIMESTAMP, A1 INTEGER, A2 VARCHAR, A3 VARCHAR, A4 INTEGER, A5 DOUBLE)
PREPARE v1 AS SELECT (SUM(CASE WHEN ((T1.A2 = ($1)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END) / ((SUM(CASE WHEN ((T1.A2 = ($1)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END) + SUM(CASE WHEN ((T1.A2 = ($2)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END)))::float8) AS A00036933 FROM T1
CREATE TABLE stringliterals AS SELECT 1 AS ID, 1::BIGINT AS a1,'value-1' AS a2,'value-1' AS a3,10::BIGINT AS a4
EXECUTE v1('value-1', 'value-2')
PREPARE v1 AS SELECT CASE ? WHEN ? THEN ? WHEN ? THEN ? ELSE ? END AS x
EXECUTE V1(1, 2, 3, 4, 5, 6)
PREPARE v1 AS SELECT typeof(?)
EXECUTE v1(3::int)
EXECUTE v1('hello')
PREPARE v2 AS SELECT ?
EXECUTE v2(3::int)
EXECUTE v2('hello')
PREPARE v3 AS SELECT ?=?
EXECUTE v3(3::int, 4::bigint)
EXECUTE v3('hello', 'hello')
EXECUTE v3([1, 2, 3], [1, 2, 3])
PREPARE v4 AS SELECT extract(year from ?)
EXECUTE v4(DATE '1992-01-01')
CREATE TABLE b (i TINYINT)
INSERT INTO b VALUES (1), (2), (3), (4), (5)
PREPARE s1 AS DELETE FROM b WHERE i=$1
SELECT * FROM b ORDER BY 1
EXECUTE s1(3)
DROP TABLE b CASCADE
PREPARE s1 AS UPDATE b SET i=$1 WHERE i=$2
EXECUTE s1(6, 3)
CREATE TABLE a (i TINYINT)
PREPARE p1 AS SELECT * FROM a
PREPARE v1 AS SELECT ?::VARCHAR::INT
EXECUTE v1('3')
create table test as select 42 i
prepare q1 as SELECT cast(? AS VARCHAR) FROM test
execute q1('oops')
PREPARE S1 AS SELECT (? / 1) + 1
EXECUTE S1(42)
PREPARE s1 AS INSERT INTO b VALUES ($1)
EXECUTE s1 (NULL)
SELECT i FROM b
PREPARE s2 AS UPDATE b SET i=$1
EXECUTE s2 (NULL)
PREPARE s3 AS DELETE FROM b WHERE i=$1
EXECUTE s3 (NULL)
PREPARE s3 AS SELECT * FROM a WHERE i=$1
EXECUTE s3(10000)
EXECUTE s3(42)
EXECUTE s3(84)
DEALLOCATE s3
PREPARE s1 AS SELECT to_years($1), CAST(list_value($1) AS BIGINT[])
EXECUTE s1(1)
PREPARE v1 AS SELECT * FROM (SELECT $1::INTEGER) sq1
PREPARE v2 AS SELECT * FROM (SELECT $1::INTEGER WHERE 1=0) sq1
PREPARE v3 AS SELECT (SELECT $1::INT+sq1.i) FROM (SELECT 42 AS i) sq1
PREPARE v4 AS SELECT (SELECT (SELECT $1::INT+sq1.i)+$2::INT+sq1.i) FROM (SELECT 42 AS i) sq1
EXECUTE v4(20, 20)
CREATE TABLE test(a TINYINT, b SMALLINT, c INTEGER, d BIGINT, e REAL, f DOUBLE, g DATE, h VARCHAR)
PREPARE s1 AS INSERT INTO test VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
EXECUTE s1(1,2,3,4,1.5,2.5,'1992-10-20', 'hello world')
CREATE TABLE "user" (name string)
PREPARE s2965 AS WITH temp_first AS ( SELECT * FROM "user" WHERE "name" = ? ), temp_second AS ( SELECT * FROM "user" WHERE "name" = ? ) SELECT * FROM temp_second
EXECUTE s2965('val1', 'val2')
DEALLOCATE s2965
CREATE TABLE t (a INTEGER, b INTEGER, c INTEGER)
INSERT INTO t VALUES (1, 5, 3), (1, 2, 3), (1, 5, 11), (2, 5, 3), (NULL, 5, 3)
SELECT * FROM t WHERE (a=1 AND b>3) OR (a=1 AND c<5)
EXPLAIN SELECT * FROM t WHERE (a=1 AND b>3) OR (a=1 AND c<5)
SELECT * FROM t WHERE (a=1 AND b=5) OR (a=2 AND b=5)
EXPLAIN SELECT * FROM t WHERE (a=1 AND b=5) OR (a=2 AND b=5)
SELECT * FROM t WHERE (a=1 AND b=5 AND c>2) OR (a=1 AND b=5 AND c<6)
EXPLAIN SELECT * FROM t WHERE (a=1 AND b=5 AND c>2) OR (a=1 AND b=5 AND c<6)
SELECT * FROM t WHERE (a=1 AND b=5) OR (a=1 AND c>2) OR (a=1 AND b=2)
EXPLAIN SELECT * FROM t WHERE (a=1 AND b=5) OR (a=1 AND c>2) OR (a=1 AND b=2)
SELECT * FROM t WHERE (a=1 AND b>3) OR (a=2 AND c<5)
SELECT * FROM t WHERE a=1 OR a=2
CREATE TABLE orders (id INT, amount INT, status VARCHAR, created_at TIMESTAMP)
CREATE TABLE line_items (id INT, order_id INT, sku VARCHAR, extracted_at TIMESTAMP)
INSERT INTO orders VALUES (1, 50, 'paid', '2025-01-01'), (2, 75, 'paid', '2025-01-02'), (3, 30, 'refunded', '2025-01-03')
INSERT INTO line_items VALUES (1, 1, 'WIDGET', '2025-01-01'), (2, 2, 'GADGET', '2025-01-02'), (3, 3, 'WIDGET', '2025-01-03')
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
CREATE VIEW order_lifecycle AS WITH sku_agg AS ( SELECT order_id, sum(CASE WHEN sku = 'WIDGET' THEN 1 ELSE 0 END) AS widget_count FROM line_items_deduped GROUP BY order_id ) SELECT o.amount, CASE WHEN COALESCE(s.widget_count, 0) > 0 THEN 'widget_order' ELSE 'other' END AS order_type, (o.status != 'refunded') AS is_net_order FROM orders_deduped o LEFT JOIN sku_agg s ON o.id = s.order_id
create table t1(col1 int, col2 int)
create table t2(col3 int)
insert into t1 values (1, 1)
insert into t2 values (1)
select col1, col2, col3 from t1 join t2 on t1.col1 = t2.col3 group by rollup(col1, col2, col3) order by 1, 2 ,3
select col1, col2, col3 from t1 join t2 on t1.col1 = t2.col3 group by cube(col1, col2, col3) order by 1, 2 ,3
select col1, col2, col3 from t1 join t2 on t1.col1 = t2.col3 group by grouping sets (col1, col2, col3), (col1, col2), (col1) order by 1, 2 ,3
pragma explain_output='optimized_only'
pragma disable_verification
explain select col1, col3 from t1 join t2 on t1.col1 = t2.col3 group by col1, col3
create table t3 (a int, b int, c int)
insert into t3 values (1, 1, 1), (1, 2, 2), (1, 1, 1), (1, 2, 1)
create table t (i integer)
insert into t values (1)
insert into t values (2)
select * from t where i in ('1','2','y')
SELECT x::VARCHAR IN ('1', y::VARCHAR) FROM (VALUES (1, 2), (2, 3)) tbl(x, y)
SELECT x::BIGINT IN (1::BIGINT, y) FROM (VALUES (1::INTEGER, 2::BIGINT), (2::INTEGER, 3::BIGINT)) tbl(x, y)
WITH t(a, b) AS ( SELECT a :: int, b :: int FROM (VALUES ('1', '4'), ('5', '3'), ('2', '*'), ('3', '8'), ('7', '*')) AS _(a, b) WHERE position('*' in b) = 0 ) SELECT a, b FROM t WHERE a < b
EXPLAIN WITH t(a, b) AS ( SELECT a :: int, b :: int FROM (VALUES ('1', '4'), ('5', '3'), ('2', '*'), ('3', '8'), ('7', '*')) AS _(a, b) WHERE position('*' in b) = 0 ) SELECT a, b FROM t WHERE a < b
with t(a, b) as ( select a :: varchar, b :: varchar FROM VALUES (1, 2), (3, 3), (5, 6), (7, 6) as _(a, b) where a <= b ) select a, b from t where a[1] = '1'
explain with t(a, b) as ( select a :: varchar, b :: varchar FROM VALUES (1, 2), (3, 3), (5, 6), (7, 6) as _(a, b) where a <= b ) select a, b from t where a[1] = '1'
create or replace table mytablename2 as from (values ('a0'), ('a1'), ('a2'), ('xxx-0'), ('xxx-1'), ('xxx-2'), ('xxx-3'), ('xxxx'), ('xxx0'), ('xxx1'), ('xxx2'), ('xxx3') ) t(mycolname), range(4300) b(someothercolname)
select mycolname[2:]::int as mycolname2 from mytablename2 where mycolname[1:3] != 'xxx' AND mycolname2 = 0 limit 5
CREATE TABLE events( col1 INT, col2 INT, col3 INT, unused1 INT, unused2 INT, unused3 INT )
INSERT INTO events VALUES (1, 1, 1, 100, 200, 300), (1, 2, 2, 100, 200, 300), (2, 1, 3, 100, 200, 300)
PRAGMA explain_output='optimized_only'
EXPLAIN SELECT col1, COUNT(*) FROM events GROUP BY ROLLUP(col1)
CREATE TABLE t2(col4 INT)
INSERT INTO t2 VALUES (1), (2)
EXPLAIN SELECT col1, col2, col4 FROM events JOIN t2 ON events.col1 = t2.col4 GROUP BY ROLLUP(col1, col2, col4)
EXPLAIN WITH limited AS ( SELECT col1, col2 FROM events WHERE col3 = 1 LIMIT 10 ) SELECT e.col1, COUNT(*) FROM events e WHERE e.col1 IN (SELECT col1 FROM limited) GROUP BY ROLLUP(e.col1)
PRAGMA explain_output='all'
SELECT col1, col2, col4, COUNT(*) FROM events JOIN t2 ON events.col1 = t2.col4 GROUP BY ROLLUP(col1, col2, col4)
WITH limited AS ( SELECT col1, col2 FROM events WHERE col3 = 1 LIMIT 10 ) SELECT e.col1, COUNT(*) FROM events e WHERE e.col1 IN (SELECT col1 FROM limited) GROUP BY ROLLUP(e.col1)
CREATE TABLE t1 AS SELECT i + 100 as x FROM range(250000) AS t(i)
SELECT * FROM t1 where rowid = 6
EXPLAIN SELECT * FROM t1 where rowid = 6
SELECT * FROM t1 where rowid = 200000
EXPLAIN SELECT * FROM t1 where rowid = 200000
SELECT * FROM t1 where rowid IN (SELECT rowid FROM t1 ORDER BY rowid DESC LIMIT 10) ORDER BY rowid
SELECT * FROM t1 where rowid IN (6, 9) ORDER BY ALL
EXPLAIN SELECT * FROM t1 where rowid IN (6, 9)
SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
EXPLAIN SELECT * FROM t1 where rowid = 6 OR rowid = 9 ORDER BY ALL
CREATE TABLE tbl_grow_shrink (id_var VARCHAR, id_int INTEGER, id_point BIGINT)
DELETE FROM tbl_grow_shrink WHERE rowid = (SELECT min(rowid) FROM tbl_grow_shrink)
SELECT * FROM lineitem ORDER BY l_orderkey DESC LIMIT 5
SELECT * FROM lineitem WHERE rowid IN (SELECT rowid FROM lineitem ORDER BY l_orderkey DESC LIMIT 5)
SELECT * FROM lineitem WHERE l_orderkey % 20000 == 0
SELECT * FROM lineitem WHERE rowid IN (SELECT rowid FROM lineitem WHERE l_orderkey % 20000 == 0)
EXPLAIN SELECT * FROM lineitem WHERE rowid = 20058
CREATE TABLE services (date DATE, train_number INT)
INSERT INTO services VALUES ('2024-01-01', 100), ('2024-01-01', 100), ('2024-01-01', 101), ('2024-01-02', 100)
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 1
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 1 ORDER BY ALL
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) > 1
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) > 1
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 2
FROM services QUALIFY count(*) OVER(PARTITION BY date, train_number) = 2
EXPLAIN FROM services QUALIFY count(*) OVER() = 1
EXPLAIN FROM services QUALIFY count(*) OVER(PARTITION BY date ORDER BY train_number) = 1
CREATE TABLE items (id INT, category VARCHAR)
INSERT INTO items VALUES (1, 'A'), (2, 'A'), (3, 'B'), (4, 'C'), (5, 'C')
CREATE OR REPLACE TABLE df AS (SELECT * AS i FROM range(10))
CREATE OR REPLACE TABLE wtf AS (SELECT 1 AS i)
explain FROM df, wtf SELECT df.i WHERE df.i > 8 USING SAMPLE 1
CREATE or replace TABLE timeseries AS FROM ( VALUES (timestamp '2026-03-25 05:33:11.822+08', 10), (timestamp '2026-03-26 05:33:11.822+08', 15), (timestamp '2026-03-27 05:33:11.822+08', 12), (timestamp '2026-03-28 05:33:11.822+08', 18), (timestamp '2026-03-29 05:33:11.822+08', 14) ) AS t(date, value)
CREATE OR REPLACE MACRO nextValue(time_serie, ts_col, value_col, ts) AS TABLE ( (SELECT ts_col, value_col FROM query_table(time_serie) WHERE ts_col >= ts order by ts_col limit 1) union select ts AS ts_col, (select value_col from query_table(time_serie) order by ts_col desc limit 1) AS value_col WHERE NOT EXISTS (FROM query_table(time_serie) WHERE ts_col >= ts) )
from range(1,5) as t(days), nextValue(timeseries, date, value, '2026-03-29 05:33:11.822+08'::timestamp - INTERVAL (days) DAY) limit 5
CREATE TABLE t2 (a VARCHAR, b BOOLEAN, c VARCHAR)
INSERT INTO t2 VALUES ('x', false, '2024-01-01')
WITH cte AS ( SELECT a, b, c::TIMESTAMPTZ AS c FROM t2 ) SELECT * FROM cte QUALIFY ROW_NUMBER() OVER (PARTITION BY a ORDER BY c DESC) = 1
CREATE TABLE struct_pushdown_test(id INT, struct_col STRUCT(sub_col1 integer, sub_col2 bool))
INSERT INTO struct_pushdown_test VALUES (1, {'sub_col1': 42, 'sub_col2': true}), (2, NULL), (3, {'sub_col1': 84, 'sub_col2': NULL}), (4, {'sub_col1': NULL, 'sub_col2': false})
PRAGMA explain_output = 'PHYSICAL_ONLY'
CREATE TABLE nested_struct_pushdown_test(id INT, struct_col STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool)))
INSERT INTO nested_struct_pushdown_test VALUES (1, {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}), (2, NULL), (3, {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}), (4, {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}})
CREATE OR REPLACE TABLE nested_struct_pushdown_test(id INT, struct_col STRUCT(s STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool))))
INSERT INTO nested_struct_pushdown_test VALUES (1, {'s': {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}}), (2, NULL), (3, {'s': {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}}), (4, {'s': {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}}})
pragma explain_output='OPTIMIZED_ONLY'
SELECT lhs.id FROM (SELECT 1 id) lhs ANTI JOIN (SELECT 1 id WHERE FALSE) rhs ON lhs.id = rhs.id
EXPLAIN SELECT lhs.id FROM (SELECT 1 id) lhs ANTI JOIN (SELECT 1 id WHERE FALSE) rhs ON lhs.id = rhs.id
set disabled_optimizers to 'build_side_probe_side'
explain from range(10) r1 right join range(10) r2 using (range)
SELECT * FROM (SELECT 1) lhs EXCEPT SELECT * FROM (SELECT 1 WHERE 1=0) rhs
EXPLAIN SELECT * FROM (SELECT 1) lhs EXCEPT SELECT * FROM (SELECT 1 WHERE 1=0) rhs
SELECT * FROM (SELECT 42 EXCEPT SELECT 43) tbl(i) WHERE i = 42
SELECT * FROM integers i1, integers i2 WHERE i1.i=i2.i ORDER BY 1
SELECT * FROM integers i1, integers i2 WHERE i1.i=i2.i AND i1.i>1 ORDER BY 1
SELECT * FROM integers i1, integers i2, integers i3 WHERE i1.i=i2.i AND i1.i=i3.i AND i1.i>1 ORDER BY 1
SELECT * FROM integers i1 JOIN integers i2 ON i1.i=i2.i WHERE i1.i>1 ORDER BY 1
SELECT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=1 WHERE i1.i>2 ORDER BY 2
SELECT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=0 WHERE i2.i IS NOT NULL ORDER BY 2
SELECT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=0 WHERE i2.i>1 ORDER BY 2
SELECT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=0 WHERE CASE WHEN i2.i IS NULL THEN False ELSE True END ORDER BY 2
SELECT DISTINCT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=0 WHERE i2.i IS NULL ORDER BY 1
SELECT * FROM integers i1 LEFT OUTER JOIN integers i2 ON 1=1 WHERE i1.i=i2.i ORDER BY 1
SELECT * FROM integers WHERE i IN ((SELECT * FROM integers)) ORDER BY i
SELECT * FROM integers WHERE i NOT IN ((SELECT * FROM integers WHERE i=1)) ORDER BY i
CREATE TABLE vals1 AS SELECT i AS i, i AS j FROM range(0, 10000, 1) t1(i)
CREATE TABLE vals2(k INTEGER, l INTEGER)
INSERT INTO vals2 SELECT * FROM vals1
SELECT i, k FROM (SELECT i, k FROM vals1, vals2) tbl1 WHERE i=k AND i<5 ORDER BY i
SELECT i, k FROM (SELECT DISTINCT i, k FROM vals1, vals2) tbl1 WHERE i=k AND i<5 ORDER BY i
SELECT i, k, SUM(j) FROM vals1, vals2 GROUP BY i, k HAVING i=k AND i<5 ORDER BY i
SELECT i, k, SUM(j) FROM (SELECT * FROM vals1, vals2) tbl1 GROUP BY i, k HAVING i=k AND i<5 ORDER BY i
SELECT i, k, sum FROM (SELECT i, k, SUM(j) AS sum FROM vals1, vals2 GROUP BY i, k) tbl1 WHERE i=k AND i<5 ORDER BY i
SELECT * FROM vals1 LEFT OUTER JOIN vals2 ON 1=1 WHERE i=k AND k=5
SELECT * FROM vals1 LEFT OUTER JOIN vals2 ON 1=1 WHERE i=k ORDER BY i LIMIT 5
SELECT * FROM (SELECT * FROM vals1, vals2 WHERE j=5 AND l=5) tbl1 LEFT OUTER JOIN (SELECT * FROM vals1, vals2) tbl2 ON tbl1.i=tbl2.i AND tbl1.k=tbl2.k WHERE tbl2.j=5 AND tbl2.l=5
SELECT * FROM (SELECT * FROM vals1, vals2) tbl1 LEFT OUTER JOIN (SELECT * FROM vals1, vals2 WHERE i=5 AND k=10) tbl2 ON tbl1.i=tbl2.i AND tbl1.k=tbl2.k WHERE tbl1.i=5 AND tbl1.k=10
SELECT * FROM (SELECT * FROM vals1, vals2 WHERE i=5 AND k=5) tbl1 LEFT OUTER JOIN (SELECT * FROM vals1, vals2) tbl2 ON tbl2.i=5 AND tbl2.k=5
SELECT * FROM (SELECT * FROM vals1, vals2 WHERE i=5 AND k=5) tbl1 LEFT OUTER JOIN (SELECT * FROM vals1, vals2) tbl2 ON tbl2.i>10000 AND tbl2.k=5
SELECT * FROM (SELECT * FROM vals1, vals2) tbl1 LEFT OUTER JOIN (SELECT * FROM vals1, vals2) tbl2 ON tbl1.i=tbl2.i AND tbl1.k=tbl2.k WHERE tbl1.i=5 AND tbl1.k=10
call dsdgen(sf=0.01)
CREATE TABLE integers AS SELECT i AS i, i AS j FROM range(0, 100) tbl(i)
SELECT j FROM integers where j = 99
SELECT j FROM integers where j = 99 AND i=99
SELECT j FROM integers where j = 99 AND i=90
SELECT count(i) FROM integers where j > 90 and i < 95
SELECT count(i) FROM integers where j > 90 and j < 95
CREATE TABLE test2 (b INTEGER, c INTEGER)
INSERT INTO test2 VALUES (1, 10), (1, 20), (2, 30)
SELECT COUNT(*) FROM test, test2 WHERE test.b = test2.b
SELECT SUM(test.a), MIN(test.a), MAX(test.a) FROM test, test2 WHERE test.b = test2.b
SELECT COUNT(*) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b
SELECT SUM(a1.a) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b
SELECT COUNT(*) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b AND a1.a=11 AND a2.a=11 AND a3.a=11
SELECT (TRUE OR a1.a=a2.b) FROM test a1, test a2 WHERE a1.a=11 AND a2.a>=10
CREATE TABLE t1 as select -1 c1 from range(1)
SELECT t1.c1 FROM t1
SELECT CAST(CAST(t1.c1 AS BIT) AS INTEGER), (1 BETWEEN -1 AND CAST(CAST(t1.c1 AS BIT) AS INTEGER)) FROM t1
select cast(cast(c1 as BIT) as INTEGER) as cast_res, 1 between -1 and cast(cast(c1 as BIT) as INTEGER) as watever from t1
SELECT t1.c1 FROM t1 WHERE (1 BETWEEN -1 AND CAST(CAST(t1.c1 AS BIT) AS INTEGER))
PRAGMA explain_output = 'OPTIMIZED_ONLY'
EXPLAIN SELECT COUNT(*), COUNT(), COUNT(i) FROM integers
SELECT COUNT(*), COUNT(), COUNT(i) FROM integers
EXPLAIN SELECT COUNT(*), COUNT(), SUM(i), COUNT(i), SUM(i) / COUNT(i) FROM integers
SELECT COUNT(*), COUNT(), SUM(i), COUNT(i), SUM(i) / COUNT(i) FROM integers
CREATE TABLE groups(grp INTEGER, aggr1 INTEGER, aggr2 INTEGER, aggr3 INTEGER)
INSERT INTO groups VALUES (1, 1, 2, 3), (1, 2, 4, 6), (2, 1, 2, 3), (2, 3, 6, 9)
SELECT sum(aggr1)::DOUBLE / count(aggr1)::DOUBLE AS avg_qty, sum(aggr2)::DOUBLE / count(aggr2)::DOUBLE AS avg_price, sum(aggr3)::DOUBLE / count(aggr3)::DOUBLE AS avg_disc FROM groups GROUP BY grp ORDER BY grp
WITH results AS ( SELECT '2023-08-17T23:00:08.539Z' as timestamp ) SELECT * FROM results WHERE timestamp::TIME BETWEEN '22:00:00'::TIME AND '23:59:59'::TIME
CREATE TABLE issue8316 (dt TIMESTAMP)
INSERT INTO issue8316 VALUES ('2016-02-14 18:00:05'), ('2016-02-15 10:04:25'), ('2016-02-16 10:04:25'), ('2016-02-16 23:59:55'),
SELECT dt FROM issue8316 WHERE CAST(dt as TIME) = CAST('10:04:25' as TIME) ORDER BY 1
SELECT i FROM integers WHERE (i=1 AND i>0) OR (i=1 AND i<3) ORDER BY i
SELECT i FROM integers WHERE (i=1) OR (i=1) ORDER BY i
SELECT i FROM integers WHERE (i=1) OR (i=1) OR (i=1) OR (i=1) OR (i=1) ORDER BY i
SELECT i FROM integers WHERE (i IS NULL AND i=1) OR (i IS NULL AND i<10) ORDER BY i
SELECT i FROM integers WHERE (i IS NOT NULL AND i>1) OR (i IS NOT NULL AND i<10) ORDER BY i
SELECT i FROM integers WHERE (i IS NULL AND (i+1) IS NULL) OR (i IS NULL AND (i+2) IS NULL) ORDER BY i
SELECT i FROM integers WHERE i=1 OR 1=1 ORDER BY i
SELECT i FROM integers WHERE i=1 OR 1=0 OR 1=1 ORDER BY i
SELECT i FROM integers WHERE (i=1 OR 1=0 OR i=1) AND (0=1 OR 1=0 OR 1=1) ORDER BY i
SELECT (i=1 AND i>0) OR (i=1 AND i<3) FROM integers ORDER BY i
SELECT (i=1) OR (i=1) FROM integers ORDER BY i
SELECT (i=1) OR (i=1) OR (i=1) OR (i=1) OR (i=1) FROM integers ORDER BY i
create table test(a integer)
insert into test values (42)
SELECT (a*2)+(a*2) FROM test
SELECT (a*2)+(a*2)+(a*2)+(a*2)+(a*2) FROM test
SELECT (a*2)+(a*2)+(a*2)+(a*2)+(a*2), a FROM test
SELECT SUM((a*2)+(a*2)+(a*2)+(a*2)+(a*2)) FROM test
SELECT a, SUM((a*2)+(a*2)+(a*2)+(a*2)+(a*2)) FROM test GROUP BY a
SELECT * FROM test WHERE ((a*2)+(a*2))>100
SELECT * FROM test WHERE ((a*2)+(a*2)+(a*2)+(a*2)+(a*2))>400
create table test2(a VARCHAR)
insert into test2 values ('hello'), ('world'), (NULL)
SELECT substring(a, 1, 3)=substring(a, 1, 3) FROM test2 ORDER BY 1
CREATE TABLE dates(lo_commitdate DATE)
INSERT INTO dates VALUES (DATE '1992-02-10')
SELECT CAST('2020-02-20' AS date) - CAST(min("ta_1"."lo_commitdate") AS date) AS "ca_1" FROM dates AS "ta_1" HAVING CAST('2020-02-20' AS date) - CAST(min("ta_1"."lo_commitdate") AS date) > 4 ORDER BY "ca_1" ASC
CREATE TABLE test(i INTEGER, j INTEGER, k integer)
INSERT INTO test VALUES (1,1,3), (2,2,4), (NULL,NULL,NULL)
SELECT i FROM test WHERE (i=j) OR (i IS NULL AND j IS NULL)
EXPLAIN SELECT (i=j) OR (i IS NULL AND j IS NULL) FROM test
SELECT i FROM test WHERE i IS NOT DISTINCT FROM j
EXPLAIN SELECT i IS NOT DISTINCT FROM j FROM test
SELECT i FROM test WHERE (i IS NULL AND j IS NULL) OR (i=j)
EXPLAIN SELECT (i IS NULL AND j IS NULL) OR (i=j) FROM test
SELECT test1.i FROM test AS test1, test AS test2 WHERE (test1.i=test2.j) OR (test1.i IS NULL AND test2.j IS NULL) ORDER BY 1
EXPLAIN SELECT test1.i FROM test AS test1, test AS test2 WHERE (test1.i=test2.j) OR (test1.i IS NULL AND test2.j IS NULL)
EXPLAIN SELECT test1.i FROM test AS test1, test AS test2 WHERE (test1.i IS NULL AND test2.j IS NULL) OR (test1.i=test2.j)
SELECT i FROM test WHERE (i=k) OR (i IS NULL AND j IS NULL)
CREATE TABLE issue13380(c0 TIMESTAMP)
INSERT INTO issue13380(c0) VALUES ('2024-08-09 14:48:00')
SELECT c0::DATE IN ('2024-08-09') d FROM issue13380
SELECT NOT (c0::DATE IN ('2024-08-09')) FROM issue13380
SELECT c0::DATE NOT IN ('2024-08-09') FROM issue13380
SELECT max(distinct x) from range(10) tbl(x)
SELECT x, max(distinct x) over (order by x desc) from range(10) tbl(x)
INSERT INTO vals VALUES (2), (NULL)
DROP TABLE vals
SELECT -v FROM vals WHERE id>0
SELECT -v FROM vals WHERE id>1 ORDER BY id
INSERT INTO test VALUES (42, 10), (43, 100)
SELECT a + 0 FROM test
SELECT 0 + a FROM test
SELECT a - 0 FROM test
SELECT 0 - a FROM test
SELECT a * 1 FROM test
SELECT 1 * a FROM test
SELECT a * 0 FROM test
SELECT 0 * a FROM test
SELECT a / 1 FROM test
SELECT 1 // a FROM test
SELECT a // 0 FROM test
create or replace table table1 ( timestamp_str varchar )
insert into table1 values ('2024-05-03 01:00:00'), ('2024-05-03 01:00:02')
select timestamp_str, cast(timestamp_str as timestamp) from table1 where cast(timestamp_str as timestamp) > cast('2024-05-03 01:00:00' as timestamp)
truncate table table1
insert into table1 values ('2024-05-03T01:00:00+00:00'), ('2024-05-03T01:00:02+00:00')
select timestamp_str, cast(timestamp_str as timestamp) from table1 where cast(timestamp_str as timestamp) > cast('2024-05-03T01:00:00+00:00' as timestamp)
select * from ( select timestamp_str, cast(timestamp_str as timestamp) as timestamp_column from table1 ) where timestamp_column > cast('2024-05-03 01:00:00' as timestamp)
PRAGMA database_size
PRAGMA disabled_compression_methods='dictionary,rle'
SET enable_http_logging=false
SET enable_http_logging=true
PRAGMA enable_profiling='json'
PRAGMA profiling_output='test.json'
PRAGMA profiling_output=''
PRAGMA disable_profiling
PRAGMA memory_limit='1GB'
PRAGMA memory_limit=-1
PRAGMA memory_limit='-1'
PRAGMA memory_limit='none'
PRAGMA memory_limit=' -1'
PRAGMA memory_limit='1G'
PRAGMA memory_limit=' 1G'
PRAGMA memory_limit='1gb'
PRAGMA memory_limit = '1GB'
PRAGMA memory_limit='1.0gb'
PRAGMA memory_limit='1.0 gb'
PRAGMA memory_limit='488.2 MiB'
PRAGMA metadata_info
FROM pragma_metadata_info()
CREATE TABLE db1.integers(i INTEGER, j INTEGER)
FROM pragma_metadata_info('db1')
PRAGMA database_list
SELECT * FROM pragma_database_list
SELECT name, file FROM pragma_database_list
CREATE TABLE db1.integers AS FROM range(1000000)
DROP TABLE db1.integers
SELECT case when used_blocks <= 3 then NULL else t end FROM pragma_database_size() t WHERE database_name='db1'
PRAGMA functions
PRAGMA table_info ('integers')
CREATE TABLE test_table(i INTEGER, j VARCHAR)
INSERT INTO test_table VALUES (1, 'hello'), (2, 'world')
SELECT * FROM test_table ORDER BY i
PRAGMA version
select * from pragma_version()
select library_version from pragma_version()
PRAGMA platform
select * from pragma_platform()
select platform from pragma_platform()
SELECT count(*) FROM pragma_version() WHERE library_version LIKE 'v%'
pragma extension_versions
SET VARIABLE v = (select library_version from pragma_version())
SET VARIABLE v_or_latest = (select CASE WHEN getvariable('v') ILIKE 'v%-dev%' THEN 'latest' WHEN getvariable('v') ILIKE 'v0.0.1' THEN 'latest' ELSE getvariable('v') END)
SET VARIABLE codename = (select CASE WHEN getvariable('v') ILIKE 'v%-dev%' THEN 'Development Version' WHEN getvariable('v') ILIKE 'v0.0.1' THEN 'Unknown Version' ELSE '%' END)
select count(*) FROM (select codename from pragma_version() WHERE codename LIKE getvariable('codename'))
SELECT CURRENT_SETTING('log_query_path')
PRAGMA log_query_path=''
CREATE TABLE "select"(i INTEGER)
CREATE VIEW v1 AS SELECT DATE '1992-01-01' AS k
CREATE TABLE t2 (id INTEGER PRIMARY KEY, j VARCHAR UNIQUE)
CREATE TABLE s1.tbl(i INTEGER UNIQUE)
CREATE INDEX my_index ON s1.tbl(i)
CREATE TABLE tbl(i INTEGER PRIMARY KEY)
CREATE INDEX not_a_table ON tbl(i)
DESCRIBE s1.tbl
DESCRIBE tbl
DESCRIBE t2
PRAGMA "SHOW"('t2')
DESCRIBE TABLES
ATTACH IF NOT EXISTS ':memory:' AS db
USE db
CREATE TABLE main_table1(i INTEGER)
CREATE TABLE test_schema.test_schema_table1(k INTEGER)
CREATE TABLE db1.db1_table1(m INTEGER)
CREATE SCHEMA db1.db1_schema
CREATE TABLE db1.db1_schema.db1_schema_table1(n INTEGER)
ATTACH DATABASE ':memory:' AS "db_quo""ted"
CREATE SCHEMA "db_quo""ted"."db_quo""ted_schema"
CREATE TABLE "db_quo""ted"."db_quo""ted_schema"."db_quo""ted_table1"(m INTEGER)
SHOW TABLES FROM db.main
SHOW TABLES FROM db1
CREATE TEMPORARY VIEW v1 AS SELECT 42 AS a
CREATE VIEW v2 AS SELECT 42 AS b
CREATE VIEW s1.v3 AS SELECT 42 AS c
SET schema='s1'
FROM v2
PRAGMA storage_info('integers')
INSERT INTO integers VALUES (1, 1), (2, NULL), (3, 3), (4, 5)
CREATE VIEW v1 AS SELECT 42
CREATE TABLE different_types(i INTEGER, j VARCHAR, k STRUCT(k INTEGER, l VARCHAR))
INSERT INTO different_types VALUES (1, 'hello', {'k': 3, 'l': 'hello'}), (2, 'world', {'k': 3, 'l': 'thisisaverylongstring'})
PRAGMA storage_info('different_types')
CREATE TABLE nested_lists AS SELECT [1, 2, 3] i, [['hello', 'world'], [NULL]] j, [{'a': 3}, {'a': 4}] k
PRAGMA storage_info('nested_lists')
CREATE TABLE integers(i INTEGER DEFAULT 1+3, j INTEGER)
PRAGMA table_info(integers)
PRAGMA table_info='integers'
PRAGMA table_info=integers
CREATE VIEW v1 AS SELECT 42::INTEGER AS a, 'hello' AS b
PRAGMA table_info('v1')
CREATE VIEW v2(c) AS SELECT 42::INTEGER AS a, 'hello' AS b
PRAGMA table_info('v2')
CREATE VIEW v3(c, d) AS SELECT DATE '1992-01-01' a, 'hello' AS b
PRAGMA table_info('v3')
CREATE VIEW test.v1 AS SELECT 42::INTEGER AS a, 'hello' AS b
PRAGMA table_info('test.v1')
PRAGMA enable_checkpoint_on_shutdown
PRAGMA disable_verify_parallelism
PRAGMA enable_progress_bar
PRAGMA disable_progress_bar
PRAGMA enable_print_progress_bar
PRAGMA disable_print_progress_bar
PRAGMA debug_checkpoint_abort='none'
CALL disable_profiling()
SELECT CURRENT_SETTING('enable_profiling')
CALL enable_profiling()
CALL enable_profiling(format='json')
SELECT CURRENT_SETTING('profiling_coverage')
CALL enable_profiling(coverage='all')
SELECT CURRENT_SETTING('profiling_output')
SELECT CURRENT_SETTING('profiling_mode')
CALL enable_profiling(mode='detailed')
CALL enable_profiling(metrics = {"OPERATOR_CARDINALITY": "true", "OPERATOR_ROWS_SCANNED": "true", "CUMULATIVE_CARDINALITY": "true", "CUMULATIVE_ROWS_SCANNED": "true"})
SELECT column_name FROM (DESCRIBE SELECT UNNEST(CURRENT_SETTING('custom_profiling_settings'))) ORDER BY column_name
CALL enable_profiling(metrics = {'QUERY_NAME': true, 'EXTRA_INFO': true, 'OPERATOR_ROWS_SCANNED': false})
PRAGMA enable_profiling = 'json'
SET profiling_mode='all'
SELECT unnest(['Maia', 'Thijs', 'Mark', 'Hannes', 'Tom', 'Max', 'Carlo', 'Sam', 'Tania']) AS names ORDER BY random()
SELECT unnest(res) FROM ( SELECT current_setting('custom_profiling_settings') AS raw_setting, raw_setting.trim('{}') AS setting, string_split(setting, ', ') AS res ) ORDER BY ALL
SELECT cpu_time, extra_info, rows_returned, latency FROM metrics_output
CREATE TABLE profile_fs.tbl AS SELECT range AS id FROM range(100_000)
PRAGMA custom_profiling_settings='{"WAITING_TO_ATTACH_LATENCY": "true", "ATTACH_LOAD_STORAGE_LATENCY": "true", "ATTACH_REPLAY_WAL_LATENCY": "true", "CHECKPOINT_LATENCY": "true"}'
SET profiling_coverage='ALL'
CHECKPOINT profile_fs
SELECT CASE WHEN checkpoint_latency >= 0 THEN 'true' ELSE 'false' END FROM metrics_output
CREATE TABLE profile_fs.other_tbl AS SELECT range AS id FROM range(100_000)
DETACH profile_fs
SELECT CASE WHEN waiting_to_attach_latency >= 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN attach_load_storage_latency >= 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN attach_replay_wal_latency >= 0 THEN 'true' ELSE 'false' END FROM metrics_output
CREATE TABLE wal_latency.tbl AS SELECT range AS id, 0 AS v FROM range(500_000)
UPDATE wal_latency.tbl SET v = id
PRAGMA custom_profiling_settings='{"COMMIT_LOCAL_STORAGE_LATENCY": "true", "WRITE_TO_WAL_LATENCY": "true"}'
SELECT CASE WHEN COMMIT_LOCAL_STORAGE_LATENCY >= 0 THEN 'true' ELSE 'false' END FROM commit_metrics
SELECT CASE WHEN WRITE_TO_WAL_LATENCY >= 0 THEN 'true' ELSE 'false' END FROM commit_metrics
DETACH wal_latency
PRAGMA custom_profiling_settings='{"WAL_REPLAY_ENTRY_COUNT": "true", "ATTACH_REPLAY_WAL_LATENCY": "true"}'
SELECT CASE WHEN wal_replay_entry_count > 0 THEN 'true' ELSE 'false' END FROM replay_metrics
SELECT CASE WHEN attach_replay_wal_latency > 0 THEN 'true' ELSE 'false' END FROM replay_metrics
PRAGMA threads = 4
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0, 10000) tbl(i)
PRAGMA custom_profiling_settings='{"BLOCKED_THREAD_TIME": "true"}'
SELECT (SELECT COUNT(*) FROM bigdata WHERE col_a = 1) + (SELECT COUNT(*) FROM bigdata WHERE col_b = 1)
SELECT COUNT(blocked_thread_time) FROM metrics_output
PRAGMA custom_profiling_settings='{"CPU_TIME": "false", "EXTRA_INFO": "true", "OPERATOR_CARDINALITY": "true", "OPERATOR_TIMING": "true", "LATENCY": "true"}'
SELECT extra_info, latency FROM metrics_output
PRAGMA custom_profiling_settings='{"QUERY_NAME": "true", "CPU_TIME": "true", "EXTRA_INFO": "true", "CUMULATIVE_CARDINALITY": "true", "CUMULATIVE_ROWS_SCANNED": "true"}'
SELECT CASE WHEN cpu_time > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN cumulative_cardinality > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN cumulative_rows_scanned > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN EXISTS( SELECT 1 FROM information_schema.columns WHERE table_name = 'metrics_output' AND column_name = 'query_name' ) THEN 'true' ELSE 'false' END
SELECT query_name FROM metrics_output
PRAGMA custom_profiling_settings='{"QUERY_NAME": "false"}'
SET profiling_mode='standard'
PRAGMA custom_profiling_settings='{"ALL_OPTIMIZERS": "true"}'
SELECT * FROM ( SELECT unnest(res) str FROM ( SELECT current_setting('custom_profiling_settings') as raw_setting, raw_setting.trim('{}') AS setting, string_split(setting, ', ') AS res ) ) WHERE '"true"' NOT in str ORDER BY ALL
PRAGMA custom_profiling_settings='{}'
PRAGMA custom_profiling_settings='{"OPTIMIZER_JOIN_ORDER": "true"}'
SELECT CASE WHEN optimizer_join_order > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SET disabled_optimizers = 'JOIN_ORDER'
PRAGMA custom_profiling_settings='{"CUMULATIVE_OPTIMIZER_TIMING": "true"}'
SELECT CASE WHEN cumulative_optimizer_timing > 0 THEN 'true' ELSE 'false' END FROM metrics_output
RESET custom_profiling_settings
SET profiling_mode = 'detailed'
SELECT * FROM ( SELECT unnest(res) str FROM ( SELECT current_setting('custom_profiling_settings') AS raw_setting, raw_setting.trim('{}') AS setting, string_split(setting, ', ') AS res ) ) WHERE '"true"' NOT IN str ORDER BY ALL
PRAGMA custom_profiling_settings='{"RESULT_SET_SIZE": "true", "OPERATOR_CARDINALITY": "true"}'
CREATE TYPE Result AS UNION ( Ok BOOLEAN, Err BIGINT )
SELECT CASE WHEN result_set_size = 144 THEN TRUE::Result ELSE result_set_size::Result END AS result FROM metrics_output
CREATE TABLE local_pruned(i BIGINT)
INSERT INTO local_pruned VALUES (1000000), (1000001), (1000002), (1000003), (1000004), (1000005), (1000006), (1000007), (1000008), (1000009)
CREATE TABLE persistent_only(i BIGINT)
INSERT INTO persistent_only VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9)
INSERT INTO local_pruned VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9)
PRAGMA custom_profiling_settings='{"OPERATOR_ROW_GROUPS_SCANNED": "true", "OPERATOR_TOTAL_ROW_GROUPS_TO_SCAN": "true", "OPERATOR_TYPE": "true", "CUMULATIVE_ROW_GROUPS_SCANNED": "true", "CUMULATIVE_TOTAL_ROW_GROUPS_TO_SCAN": "true"}'
SELECT count(*) FROM local_pruned a, persistent_only b WHERE a.i < 100 AND b.i < 100
SELECT * FROM operator_metrics WHERE operator_type = 'TABLE_SCAN' ORDER BY total
SELECT sum(scanned)::VARCHAR || '/' || sum(total)::VARCHAR FROM operator_metrics WHERE operator_type = 'TABLE_SCAN'
PRAGMA custom_profiling_settings='{"OPERATOR_CARDINALITY": "true", "OPERATOR_ROWS_SCANNED": "true", "CUMULATIVE_CARDINALITY": "true", "CUMULATIVE_ROWS_SCANNED": "true"}'
SELECT * FROM integers i1, integers i2 WHERE i1.i = i2.i ORDER BY 1
pragma disable_profiling
SELECT cumulative_cardinality, cumulative_rows_scanned FROM metrics_output
SELECT CASE WHEN cumulative_rows_scanned = 8 THEN 'true' ELSE 'false' END FROM metrics_output
PRAGMA custom_profiling_settings='{"CUMULATIVE_CARDINALITY": "true", "CUMULATIVE_ROWS_SCANNED": "true", "BLOCKED_THREAD_TIME": "true"}'
CREATE TABLE t AS SELECT range i FROM range(400000)
SELECT * FROM t LIMIT 1024
PRAGMA disabled_optimizers='late_materialization'
SELECT * FROM range(10)
SELECT * FROM range(1000) LIMIT 10
SELECT cumulative_rows_scanned FROM metrics_output
SELECT CASE WHEN cumulative_rows_scanned = 10 THEN 'true' ELSE 'false' END FROM metrics_output
PRAGMA custom_profiling_settings='{"TOTAL_MEMORY_ALLOCATED": "true"}'
CREATE OR REPLACE TABLE test AS SELECT range AS id, hash(range) AS data FROM range(100000)
SELECT CASE WHEN total_memory_allocated >= 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT id, COUNT(*) as cnt FROM test GROUP BY id ORDER BY id LIMIT 1000
PRAGMA custom_profiling_settings='{"TOTAL_MEMORY_ALLOCATED": "true", "OPERATOR_NAME": "true", "OPERATOR_TYPE": "true"}'
SELECT * FROM test WHERE id < 1000 ORDER BY id
CREATE OR REPLACE TABLE metrics_children AS SELECT unnest(children, recursive := true) FROM metrics_output
PRAGMA custom_profiling_settings='{"TOTAL_MEMORY_ALLOCATED": "false"}'
CREATE OR REPLACE TABLE test2 AS SELECT range AS id FROM range(10000)
SET memory_limit = '100MB'
CREATE OR REPLACE TABLE large_test AS SELECT range AS id, hash(range) AS data FROM range(1000000)
PRAGMA custom_profiling_settings='{"ALL": "true"}'
PRAGMA custom_profiling_settings='{"CORE": "true"}'
SELECT CPU_TIME, CUMULATIVE_CARDINALITY, CUMULATIVE_ROWS_SCANNED, CUMULATIVE_ROW_GROUPS_SCANNED, CUMULATIVE_TOTAL_ROW_GROUPS_TO_SCAN, EXTRA_INFO, LATENCY, QUERY_NAME, RESULT_SET_SIZE, ROWS_RETURNED FROM metrics_output
PRAGMA custom_profiling_settings='{"DEFAULT": "true"}'
PRAGMA custom_profiling_settings='{"EXECUTION": "true"}'
SELECT BLOCKED_THREAD_TIME, SYSTEM_PEAK_BUFFER_MEMORY, SYSTEM_PEAK_TEMP_DIR_SIZE, TOTAL_MEMORY_ALLOCATED FROM metrics_output
PRAGMA custom_profiling_settings='{"FILE": "true"}'
SELECT ATTACH_LOAD_STORAGE_LATENCY, ATTACH_REPLAY_WAL_LATENCY, CHECKPOINT_LATENCY, COMMIT_LOCAL_STORAGE_LATENCY, CUMULATIVE_VACUUM_TIME, TOTAL_BYTES_READ, TOTAL_BYTES_WRITTEN, WAITING_TO_ATTACH_LATENCY, WAL_REPLAY_ENTRY_COUNT, WRITE_TO_WAL_LATENCY FROM metrics_output
PRAGMA custom_profiling_settings='{"OPERATOR": "true"}'
SELECT OPERATOR_CARDINALITY, OPERATOR_NAME, OPERATOR_ROWS_SCANNED, OPERATOR_ROW_GROUPS_SCANNED, OPERATOR_TIMING, OPERATOR_TOTAL_ROW_GROUPS_TO_SCAN, OPERATOR_TYPE FROM ( SELECT unnest(children, max_depth := 2) FROM metrics_output )
PRAGMA custom_profiling_settings='{"OPTIMIZER": "true"}'
PRAGMA custom_profiling_settings='{"PHASE_TIMING": "true"}'
SELECT * EXCLUDE(value, description) FROM duckdb_profiling_settings()
PRAGMA profiling_coverage='ALL'
PRAGMA profiling_mode='DETAILED'
pragma custom_profiling_settings='{"CPU_TIME": "true"}'
SELECT * EXCLUDE(description) FROM duckdb_profiling_settings()
USE my_db
call enable_logging()
CREATE TABLE small AS FROM range(100)
CREATE TABLE medium AS FROM range(10000)
CREATE TABLE big AS FROM range(1000000)
SELECT count(*) FROM duckdb_logs_parsed('Metrics') WHERE metric == 'CPU_TIME'
PRAGMA custom_profiling_settings='{"TOTAL_BYTES_READ": "true", "TOTAL_BYTES_WRITTEN": "true"}'
SET profiling_coverage='SELECT'
SELECT latency != 0, contains(query_name, 'ATTACH') FROM metrics_output
CREATE TABLE profile_attach.tbl AS SELECT range AS id FROM range(10000)
SELECT latency != 0, contains(query_name, 'CREATE TABLE') FROM metrics_output
INSERT INTO profile_attach.tbl SELECT range + 20000 FROM range(10000)
SELECT latency != 0, contains(query_name, 'INSERT INTO') FROM metrics_output
CREATE INDEX idx ON profile_attach.tbl(id)
SELECT latency != 0, contains(query_name, 'CREATE INDEX') FROM metrics_output
DETACH profile_attach
SELECT latency != 0, contains(query_name, 'DETACH') FROM metrics_output
CREATE TABLE profile_fs.tbl AS SELECT range AS id FROM range(10000)
SELECT total_bytes_written FROM metrics_output
SELECT CASE WHEN total_bytes_written > 0 THEN 'true' ELSE 'false' END FROM metrics_output
CREATE INDEX idx ON profile_fs.tbl(id)
SELECT * FROM profile_fs.tbl
SELECT CASE WHEN total_bytes_read > 0 THEN 'true' ELSE 'false' END FROM metrics_output
RESET profiling_output
PRAGMA enable_profiling = 'query_tree'
PRAGMA enable_profiling = 'GRAPHVIZ'
RESET enable_profiling
SET allowed_configs=['TimeZone']
SET allowed_configs=['pivot_limit', 'EXPLAIN_output', 'memory_limit']
SET lock_configuration=true
SET pivot_limit=20000
SET explain_output='OPTIMIZED_ONLY'
SET memory_limit='2GB'
SET max_memory='2GB'
SET search_path=''
SET TimeZone='America/New_York'
RESET allowed_directories
CREATE TABLE a1.integers(i INTEGER)
RESET allowed_paths
RESET block_allocator_memory
SET block_allocator_memory='100MiB'
SET memory_limit='200MiB'
SET block_allocator_memory='75%'
SELECT value FROM duckdb_settings() WHERE name = 'block_allocator_memory'
SET block_allocator_memory='200MiB'
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
SET default_order = 'ASCENDING'
SET default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'DESCENDING'
SET SESSION default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'ASCENDING'
SET SESSION default_null_order = 'NULLS LAST'
SELECT * FROM tbl ORDER BY i
SELECT * FROM integers ORDER BY i DESC
SELECT FIRST(i ORDER BY i), LAST(i ORDER BY i) FROM integers
SELECT FIRST(i ORDER BY i DESC), LAST(i ORDER BY i DESC) FROM integers
SELECT list_sort(LIST(i)), list_reverse_sort(LIST(i)) FROM integers
SET default_null_order='sqlite'
SET default_null_order='postgres'
create schema my_schema
select current_schema()
SET schema='my_schema'
drop schema my_schema
drop schema schema2
ATTACH ':memory:' as db2
create schema db2.schema1
drop schema db2.schema1
drop schema schema1
SET errors_as_json=true
SELECT 1/2
SELECT 1//2
SET integer_division=true
SET integer_division=false
create schema s2
use s1
use s2
reset schema
reset search_path
SELECT current_setting('null_order'), (SELECT value FROM duckdb_settings() WHERE name='null_order')
SET null_order='NULLS_FIRST'
RESET null_order
PRAGMA default_collation='NOCASE'
CREATE TABLE collate_test(s VARCHAR)
INSERT INTO collate_test VALUES ('hEllO'), ('WöRlD'), ('wozld')
SELECT COUNT(*) FROM collate_test WHERE 'BlA'='bLa'
SELECT * FROM collate_test WHERE s='hello'
SELECT * FROM collate_test ORDER BY s
PRAGMA default_collation='NOCASE.NOACCENT'
SET GLOBAL default_collation='NOCASE'
SET SESSION default_collation='NOCASE'
SET disabled_optimizers=''
SET disabled_optimizers TO 'expression_rewriter'
SET disabled_optimizers TO 'expression_rewriter,filter_pushdown,join_order'
SELECT current_setting('disabled_optimizers')
SELECT * FROM duckdb_settings()
SET enable_progress_bar=true
SELECT * FROM range(3) ORDER BY 1
SELECT value FROM duckdb_settings() WHERE name='preserve_identifier_case'
CREATE SCHEMA MYSCHEMA
CREATE TABLE MYSCHEMA.INTEGERS(I INTEGER)
SELECT duckdb_tables.schema_name, duckdb_tables.table_name, column_name FROM duckdb_tables JOIN duckdb_columns USING (table_oid)
DROP SCHEMA MYSCHEMA CASCADE
SET preserve_identifier_case TO false
SET profiling_mode='detailed'
SET TimeZone='pacific/honolulu'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'TimeZone'
SET Calendar='Coptic'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'Calendar'
SELECT current_setting('disabled_filesystems')
RESET disabled_filesystems
SET disabled_filesystems=''
SET disabled_filesystems='LocalFileSystem'
SELECT current_setting('lock_configuration')
SET memory_limit='8GB'
RESET lock_configuration
SELECT current_setting('custom_user_agent')
SELECT regexp_matches(user_agent, '^duckdb/.*(.*)') FROM pragma_user_agent()
select current_setting('threads')
pragma threads=42
RESET threads
SET temp_directory = '.unrecognized_folder/folder2'
SELECT * FROM tpcds_queries()
