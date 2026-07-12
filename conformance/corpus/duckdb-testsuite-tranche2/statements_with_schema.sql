# file: test/sql/insert/insert_by_name.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE "My Table"("My Column 1" INT, "My Column 2" INT)
CREATE TABLE tbl ( price INTEGER, total_price AS ((price)::DATE) )
CREATE TABLE tbl2 (a INTEGER, b INTEGER PRIMARY KEY)
CREATE TABLE tbl3 (id INTEGER PRIMARY KEY)
# query
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
# reject
INSERT INTO integers BY NAME SELECT 1 AS xxx
INSERT INTO integers BY NAME SELECT 1 AS i, 2 AS i
INSERT INTO integers (i, i) SELECT 1, 2
INSERT INTO integers BY NAME SELECT 1 AS rowid
INSERT INTO tbl BY NAME SELECT 1 AS total_price
INSERT INTO integers BY NAME VALUES (42, 84)
INSERT INTO integers BY NAME (i) SELECT 1 AS j
# file: test/sql/insert/insert_from_many_grouping_sets.test
# setup
CREATE TABLE integers AS SELECT case when i%2=0 then null else i end AS i, i%2 as j FROM generate_series(0,999999,1) tbl(i)
CREATE TABLE integers2 AS SELECT * FROM integers GROUP BY GROUPING SETS ((), (i), (i, j), (j))
# query
CREATE TABLE integers AS SELECT i, i%2 as j FROM generate_series(0,999999,1) tbl(i)
CREATE TABLE integers2 AS SELECT * FROM integers GROUP BY GROUPING SETS ((), (i), (i, j), (j))
SELECT SUM(i), SUM(j), COUNT(*), COUNT(i), COUNT(j) FROM integers
SELECT SUM(i), SUM(j), COUNT(*), COUNT(i), COUNT(j) FROM integers2
DROP TABLE integers
DROP TABLE integers2
CREATE TABLE integers AS SELECT case when i%2=0 then null else i end AS i, i%2 as j FROM generate_series(0,999999,1) tbl(i)
# file: test/sql/insert/insert_rollback.test
# setup
CREATE TABLE integers(i INTEGER)
# query
CREATE TABLE integers(i INTEGER)
BEGIN TRANSACTION
INSERT INTO integers VALUES (0), (1), (2)
SELECT COUNT(*) FROM integers
ROLLBACK
# file: test/sql/insert/null_values.test
# setup
CREATE TABLE integers(i INTEGER)
# query
INSERT INTO integers SELECT i FROM range(100) tbl(i)
INSERT INTO integers SELECT NULL FROM range(100) tbl(i)
SELECT COUNT(i), SUM(i), MIN(i), MAX(i), COUNT(*) FROM integers
COMMIT
SELECT SUM(CASE WHEN i IS NULL THEN 1 ELSE 0 END) FROM integers
# file: test/sql/insert/test_big_insert.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
# query
INSERT INTO integers SELECT * FROM integers
INSERT INTO integers VALUES (3, 4), (4, 3)
INSERT INTO integers VALUES (DEFAULT, 4)
INSERT INTO integers (i) SELECT j FROM integers
SELECT * FROM integers
# reject
INSERT INTO integers VALUES (DEFAULT+1, 4)
# file: test/sql/insert/test_insert.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE i2 AS SELECT 1 AS i FROM integers WHERE i % 2 <> 0
CREATE TABLE IF NOT EXISTS presentations(presentation_date Date NOT NULL UNIQUE, author VARCHAR NOT NULL, title VARCHAR NOT NULL, bio VARCHAR, abstract VARCHAR, zoom_link VARCHAR)
# query
INSERT INTO integers VALUES (1), (2), (3), (4), (5)
CREATE TABLE i2 AS SELECT 1 AS i FROM integers WHERE i % 2 <> 0
SELECT * FROM i2 ORDER BY 1
UPDATE i2 SET i=NULL
CREATE TABLE IF NOT EXISTS presentations(presentation_date Date NOT NULL UNIQUE, author VARCHAR NOT NULL, title VARCHAR NOT NULL, bio VARCHAR, abstract VARCHAR, zoom_link VARCHAR)
# file: test/sql/insert/test_insert_invalid.test
# setup
CREATE TABLE strings(i STRING)
CREATE TABLE a(i integer, j integer)
# query
CREATE TABLE strings(i STRING)
INSERT INTO strings VALUES ('�(')
SELECT * FROM strings WHERE i = '�('
CREATE TABLE a(i integer, j integer)
INSERT INTO a VALUES (1, 2)
# reject
INSERT INTO a VALUES (1)
INSERT INTO a VALUES (1,2,3)
INSERT INTO a VALUES (1,2),(3)
INSERT INTO a VALUES (1,2),(3,4,5)
INSERT INTO a SELECT 42
# file: test/sql/insert/test_insert_query.test
# setup
CREATE TABLE integers(i INTEGER)
# query
INSERT INTO integers SELECT 42
INSERT INTO integers SELECT CAST(NULL AS VARCHAR)
# file: test/sql/insert/test_insert_type.test
# setup
CREATE TABLE strings(a VARCHAR)
CREATE TABLE integers(i INTEGER)
# query
SET default_null_order='nulls_first'
CREATE TABLE strings(a VARCHAR)
INSERT INTO integers VALUES (3), (4), (NULL)
INSERT INTO strings SELECT * FROM integers
SELECT * FROM strings
UPDATE strings SET a=13 WHERE a='3'
SELECT * FROM strings ORDER BY cast(a AS INTEGER)
# file: test/sql/insert/unaligned_interleaved_appends.test
# setup
CREATE TABLE integers(i INTEGER)
# query
SET immediate_transaction_mode=true
INSERT INTO integers SELECT * FROM range(0, 5)
INSERT INTO integers SELECT * FROM range(0, 17)
INSERT INTO integers SELECT * FROM range(0, 1007)
INSERT INTO integers SELECT * FROM range(0, 3020)
INSERT INTO integers SELECT * FROM range(0, 3)
# file: test/sql/update/null_update_merge.test
# setup
CREATE TABLE test (id INTEGER, a INTEGER)
# query
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
# file: test/sql/update/null_update_merge_transaction.test
# setup
CREATE TABLE test (id INTEGER, a INTEGER)
# query
UPDATE test SET a=CASE WHEN a IS NULL THEN 1 ELSE NULL END
UPDATE test SET a=NULL
# file: test/sql/update/string_update_transaction_local_7348.test
# setup
CREATE TABLE t1(a VARCHAR(256) PRIMARY KEY, b INTEGER)
# query
BEGIN
CREATE TABLE t1(a VARCHAR(256) PRIMARY KEY, b INTEGER)
INSERT INTO t1 VALUES(' 4-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ', 2 + 1)
INSERT INTO t1 VALUES(' 34-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ', 18)
INSERT INTO t1 SELECT b, b + 1 FROM t1 WHERE b < 5
FROM t1
UPDATE t1 SET a = CONCAT(a, 'x') WHERE b % 2 = 0
# file: test/sql/update/test_big_string_update.test
# setup
CREATE TABLE test (a VARCHAR)
# query
CREATE TABLE test (a VARCHAR)
INSERT INTO test VALUES ('abcdefghijklmnopqrstuvwxyz')
INSERT INTO test SELECT a||a||a||a||a||a||a||a||a||a FROM test
DELETE FROM test WHERE length(a) = (SELECT MIN(length(a)) FROM test)
SELECT LENGTH(a) FROM test
UPDATE test SET a='a'
# file: test/sql/update/test_cascading_updates.test
# setup
CREATE TABLE integers(id INTEGER, val INTEGER)
# query
CREATE TABLE integers(id INTEGER, val INTEGER)
INSERT INTO integers SELECT i, i FROM range(10000) t(i)
PRAGMA checkpoint_threshold='1GB'
UPDATE integers SET val=val+1000000 WHERE id=1
UPDATE integers SET val=val+1000000 WHERE id=2
UPDATE integers SET val=val+1000000 WHERE id=3
SELECT COUNT(*) FROM integers WHERE val>1000000
# reject
CHECKPOINT
# file: test/sql/update/test_multiple_assignment.test
# setup
CREATE TABLE tbl (key INT, fruit VARCHAR, cost INT)
# query
CREATE TABLE tbl (key INT, fruit VARCHAR, cost INT)
INSERT INTO tbl VALUES (1, 'apple', 2), (2, 'orange', 3)
UPDATE tbl SET (key, fruit, cost) = (1, 'pear', 2)
SELECT * FROM tbl
UPDATE tbl SET (key, fruit, cost) = (2, 'apple', 3)
UPDATE tbl SET (key, fruit, cost) = 3
UPDATE tbl SET (key, fruit, cost) = ADD(key, cost)
# reject
UPDATE tbl SET (key, fruit, cost) = (1, 2)
UPDATE tbl SET (key, fruit, cost) = (1, 2, 3, 4)
UPDATE tbl SET () = (key, fruit)
UPDATE tbl SET (key, fruit) = ()
# file: test/sql/update/test_null_update.test
# setup
CREATE TABLE test (a INTEGER)
# query
CREATE TABLE test (a INTEGER)
INSERT INTO test VALUES (1), (2), (3), (NULL)
SELECT * FROM test ORDER BY a
UPDATE test SET a=NULL WHERE a=2
UPDATE test SET a=NULL WHERE a=3
UPDATE test SET a=10 WHERE a IS NULL
# file: test/sql/update/test_repeated_string_update.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test VALUES ('hello'), ('world')
UPDATE test SET a='test' WHERE a='hello'
UPDATE test SET a='test2' WHERE a='world'
# file: test/sql/update/test_stress_update_issue_19688.test
# setup
CREATE TABLE test_stress_update_issue_19688 ( id INTEGER, val INTEGER )
# query
DROP TABLE IF EXISTS test_stress_update_issue_19688
CREATE TABLE test_stress_update_issue_19688 ( id INTEGER, val INTEGER )
INSERT INTO test_stress_update_issue_19688 SELECT range AS id, range * 1000 AS val FROM range(1000)
SELECT COUNT(*) FROM test_stress_update_issue_19688
SELECT COUNT(DISTINCT id) FROM test_stress_update_issue_19688
# file: test/sql/update/test_string_update.test
# setup
CREATE TABLE test (a VARCHAR)
# query
DELETE FROM test WHERE a='hello'
UPDATE test SET a='hello'
# file: test/sql/update/test_string_update_many_strings.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test VALUES ('a'), ('b'), ('c'), (NULL)
INSERT INTO test SELECT * FROM test
SELECT DISTINCT a FROM test ORDER BY a
UPDATE test SET a='aa' WHERE a='a'
# file: test/sql/update/test_string_update_null.test
# setup
CREATE TABLE test (a VARCHAR)
# query
UPDATE test SET a=NULL where a='world'
# file: test/sql/update/test_string_update_rollback.test
# setup
CREATE TABLE test (a VARCHAR)
# query
UPDATE test SET a='test2' WHERE a='test'
# file: test/sql/update/test_string_update_rollback_null.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test VALUES ('test'), ('world')
UPDATE test SET a=NULL WHERE a='world'
UPDATE test SET a='world' WHERE a IS NULL
# file: test/sql/update/test_update.test
# setup
CREATE TABLE test (a INTEGER)
# query
INSERT INTO test VALUES (3)
SELECT * FROM test
SELECT * FROM test WHERE a=3
UPDATE test SET a=1
SELECT * FROM test WHERE a=1
UPDATE test SET a=4
# file: test/sql/update/test_update_delete_same_tuple.test
# setup
CREATE TABLE test (a INTEGER)
# query
INSERT INTO test VALUES (1), (2), (3)
UPDATE test SET a=a+1
DELETE FROM test
DROP TABLE test
# file: test/sql/update/test_update_from.test
# setup
CREATE TABLE test (a INTEGER)
CREATE TABLE src (a INTEGER)
CREATE TABLE terms(docid INTEGER, term INTEGER)
CREATE TABLE docs(id INTEGER, len INTEGER)
CREATE VIEW vt AS (SELECT 17 as v)
# query
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
# file: test/sql/update/test_update_issue_3170.test
# setup
CREATE TABLE student(id INTEGER, name VARCHAR, PRIMARY KEY(id))
# query
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
# file: test/sql/update/test_update_many_updaters.test
# setup
CREATE TABLE test (a INTEGER)
# query
UPDATE test SET a=4 WHERE a=1
UPDATE test SET a=5 WHERE a=2
UPDATE test SET a=6 WHERE a=3
UPDATE test SET a=a-3
UPDATE test SET a=7 WHERE a=4
UPDATE test SET a=8 WHERE a=5
UPDATE test SET a=9 WHERE a=6
# reject
UPDATE test SET a=99 WHERE a=1
UPDATE test SET a=99 WHERE a=2
UPDATE test SET a=99 WHERE a=3
# file: test/sql/update/test_update_many_updaters_nulls.test
# setup
CREATE TABLE test (a INTEGER)
# query
UPDATE test SET a=NULL WHERE a=1
SELECT COUNT(*) FROM test WHERE a IS NULL
UPDATE test SET a=99 WHERE a IS NULL
# file: test/sql/update/test_update_mix.test
# setup
CREATE TABLE test (a INTEGER)
# query
SELECT SUM(a) FROM test
INSERT INTO test VALUES (4), (5), (6)
DELETE FROM test WHERE a < 4
# file: test/sql/update/test_update_same_value.test
# setup
CREATE TABLE test (a INTEGER)
# query
SELECT * FROM test WHERE a=4
SELECT * FROM test WHERE a=5
UPDATE test SET a=9 WHERE a=5
UPDATE test SET a=7 WHERE a=3
UPDATE test SET a=8 WHERE a=4
# file: test/sql/update/test_update_with_non_regular_type.test
# setup
CREATE TABLE t1 (id VARCHAR, new_id VARCHAR, tags VARCHAR[], g GEOMETRY)
CREATE TABLE t2 (id INT, val VARCHAR, tags VARCHAR[], g GEOMETRY)
# query
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
# file: test/sql/update/update_after_commit.test
# setup
CREATE TABLE a (b int)
# query
CREATE TABLE a (b int)
UPDATE a SET b = b + 10
SELECT * FROM a
# file: test/sql/update/update_default.test
# setup
CREATE TABLE t1 (c0 INT)
# query
CREATE TABLE t1 (c0 INT)
INSERT INTO t1(c0) VALUES (1),(2),(3)
UPDATE t1 SET c0 = DEFAULT
# file: test/sql/update/update_delete_wal.test
# setup
create table test (id bigint primary key, c1 text)
# query
SET wal_autocheckpoint='1TB'
create table test (id bigint primary key, c1 text)
insert into test (id, c1) values (1, 'foo')
insert into test (id, c1) values (2, 'bar')
begin transaction
delete from test where id = 1
update test set c1='baz' where id=2
commit
# file: test/sql/update/update_error_suggestions.test
# setup
CREATE TABLE tbl(mycol INTEGER)
# query
CREATE TABLE tbl(mycol INTEGER)
# reject
UPDATE tbl SET myco=42
UPDATE tbl SET tbl.mycol=42
# file: test/sql/update/update_join_nulls.test
# setup
CREATE TABLE t(table_id BIGINT, val BOOLEAN)
# query
CREATE TABLE t(table_id BIGINT, val BOOLEAN)
INSERT INTO t VALUES (1, NULL)
WITH new_values(tid, new_val) AS ( VALUES (1, NULL) ) UPDATE t SET val=new_val FROM new_values WHERE table_id=tid
# file: test/sql/update/update_null_integers.test
# setup
CREATE TABLE t(i int, j int)
# query
CREATE TABLE t(i int, j int)
INSERT INTO t SELECT ii, NULL FROM range(1024) tbl(ii)
select COUNT(j), MIN(j), MAX(j) from t
UPDATE t SET j = 1
# file: test/sql/delete/cleanup_delete_on_conflict.test
# setup
CREATE TABLE tbl(i INTEGER)
# query
CREATE TABLE tbl(i INTEGER)
INSERT INTO tbl FROM range(1000) t(i)
DELETE FROM tbl WHERE i BETWEEN 200 AND 300
# reject
DELETE FROM tbl WHERE i <= 500
# file: test/sql/delete/large_deletes_transactions.test
# setup
CREATE TABLE a AS SELECT * FROM range(1000000) t1(i)
# query
CREATE TABLE a AS SELECT * FROM range(1000000) t1(i)
SELECT COUNT(*) FROM a
DELETE FROM a WHERE i%2=0
# file: test/sql/delete/list_delete.test
# setup
CREATE TABLE aggr (k int[])
# query
CREATE TABLE aggr (k int[])
INSERT INTO aggr VALUES ([0, 1, 1, 1, 4, 0, 3, 3, 2, 2, 4, 4, 2, 4, 0, 0, 0, 1, 2, 3, 4, 2, 3, 3, 1])
INSERT INTO aggr VALUES ([]), ([NULL]), (NULL), ([0, 1, 1, 1, 4, NULL, 0, 3, 3, 2, NULL, 2, 4, 4, 2, 4, 0, 0, 0, 1, NULL, 2, 3, 4, 2, 3, 3, 1])
SELECT COUNT(k) FROM aggr
DELETE FROM aggr
# file: test/sql/delete/test_delete.test
# setup
CREATE TABLE a(i INTEGER)
# query
CREATE TABLE a(i INTEGER)
INSERT INTO a VALUES (42)
DELETE FROM a
# file: test/sql/delete/test_delete_indexed.test
# setup
CREATE TABLE t (id INT PRIMARY KEY, s TEXT, j BIGINT)
CREATE INDEX idx ON t(j)
# query
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
# file: test/sql/delete/test_large_delete.test
# setup
CREATE TABLE a AS SELECT * FROM range(0, 10000, 1) t1(i)
# query
CREATE TABLE a AS SELECT * FROM range(0, 10000, 1) t1(i)
SELECT COUNT(*) FROM a WHERE i >= 2000 AND i < 5000
DELETE FROM a WHERE i >= 2000 AND i < 5000
# file: test/sql/delete/test_large_delete_parallel.test
# setup
CREATE TABLE a AS SELECT * FROM range(0, 10000, 1) t1(i)
# query
pragma threads=2
pragma verify_parallelism
# file: test/sql/delete/test_segment_deletes.test
# setup
CREATE TABLE a(i INTEGER)
# query
INSERT INTO a SELECT * FROM range(0, 1024, 1)
DELETE FROM a WHERE i=0
DELETE FROM a WHERE i=1
DELETE FROM a WHERE i=1022
DELETE FROM a WHERE i=1023
# file: test/sql/delete/test_truncate.test
# setup
CREATE TABLE a(i INTEGER)
# query
TRUNCATE TABLE a
TRUNCATE a
# file: test/sql/delete/test_using_delete.test
# setup
CREATE TABLE a(i INTEGER)
# query
INSERT INTO a VALUES (1), (2), (3)
DELETE FROM a USING (values (1)) tbl(i) WHERE a.i=tbl.i
DELETE FROM a USING (values (1)) tbl(i)
DELETE FROM a USING (values (1)) tbl(i), (values (1), (2)) tbl2(i) WHERE a.i=tbl.i AND a.i=tbl2.i
DELETE FROM a USING (values (4)) tbl(i) WHERE a.i=tbl.i
DELETE FROM a USING a a2(i) WHERE a.i>a2.i
# reject
DELETE FROM a USING b WHERE a.i=b.i
DELETE FROM a USING a b WHERE a.i=b.j
# file: test/sql/delete/test_using_delete_duplicates.test
# setup
create table integers2 as select * from generate_series(0, 9, 1)
CREATE or replace TABLE integers AS FROM range(10)
create table integers_copy as select * from integers
create or replace table t1 as select range%1000 a from range(100_000)
create or replace table t2 as select range b from range(100)
create or replace table t2_copy as select * from t2
# query
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
# file: test/sql/merge/merge_into.test
# setup
CREATE TABLE Stock(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
CREATE TABLE Sale(item_id int, volume int)
CREATE TABLE merge_distinct_target(tableticker VARCHAR NOT NULL, figi VARCHAR, cik VARCHAR, lastupdated DATE NOT NULL)
CREATE VIEW my_view AS SELECT 42 item_id
# query
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
# reject
MERGE INTO Stock USING Sale ON Stock.item_id = Sale.item_id WHEN MATCHED AND Sale.volume >= balance THEN DELETE WHEN MATCHED THEN UPDATE SET balance = balance - Sale.volume WHEN NOT MATCHED THEN ERROR CONCAT('Sale item with item id ', Sale.item_id, ' not found')
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id)
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id) WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) WHEN NOT MATCHED THEN ERROR
MERGE INTO my_view USING Sale ON my_view.item_id = Sale.item_id WHEN NOT MATCHED THEN INSERT
# file: test/sql/merge/merge_into_bind_matching_columns.test
# setup
CREATE TABLE dest (id INTEGER, val INTEGER)
CREATE TABLE src (id INTEGER, val INTEGER)
# query
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
# file: test/sql/merge/merge_into_by_source.test
# setup
CREATE TABLE Stock(item_id int, balance int)
# query
INSERT INTO Stock VALUES (5, 10), (10, 20), (20, 30)
MERGE INTO Stock USING (VALUES (5, 20), (10, 30)) new_accounts(item_id, balance) USING (item_id) WHEN MATCHED THEN UPDATE WHEN NOT MATCHED BY TARGET THEN INSERT WHEN NOT MATCHED BY SOURCE THEN DELETE
FROM Stock ORDER BY ALL
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED BY SOURCE THEN DELETE
# file: test/sql/merge/merge_into_constraint.test
# setup
CREATE TABLE Stock(item_id int NOT NULL, balance int, CHECK (balance>0))
CREATE TABLE Items(item_id int NOT NULL, total_cost INTEGER, base_cost INTEGER, tax_cost INTEGER, CHECK (total_cost = base_cost + tax_cost))
# query
CREATE TABLE Stock(item_id int NOT NULL, balance int, CHECK (balance>0))
MERGE INTO Stock USING (VALUES (1, 10)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, new_accounts.balance)
CREATE TABLE Items(item_id int NOT NULL, total_cost INTEGER, base_cost INTEGER, tax_cost INTEGER, CHECK (total_cost = base_cost + tax_cost))
INSERT INTO Items VALUES (1, 10, 8, 2)
MERGE INTO Items USING (VALUES (1, 15)) new_prices(item_id, total_cost) USING (item_id) WHEN MATCHED THEN UPDATE SET total_cost = new_prices.total_cost, base_cost = new_prices.total_cost - 2
FROM Items
# reject
MERGE INTO Stock USING (VALUES (NULL, NULL)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, new_accounts.balance)
MERGE INTO Stock USING (VALUES (1, 15)) sales(item_id, volume) USING (item_id) WHEN MATCHED THEN UPDATE SET balance = balance - volume
MERGE INTO Items USING (VALUES (1, 15)) new_prices(item_id, total_cost) USING (item_id) WHEN MATCHED THEN UPDATE SET total_cost = new_prices.total_cost
# file: test/sql/merge/merge_into_default.test
# setup
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
# query
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, DEFAULT)
MERGE INTO Stock USING (VALUES (20)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT (item_id) VALUES (new_accounts.item_id)
MERGE INTO Stock USING (VALUES (30)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT DEFAULT VALUES
FROM Stock order by all
UPDATE Stock SET balance=100
MERGE INTO Stock USING (VALUES (10)) reset_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE SET balance=DEFAULT WHEN NOT MATCHED THEN ERROR
# file: test/sql/merge/merge_into_error.test
# setup
CREATE TABLE Stock(item_id int, balance int)
CREATE TABLE Buys(item_id int, volume int)
# query
CREATE TABLE Buys(item_id int, volume int)
INSERT INTO Buys VALUES (42, 100)
MERGE INTO Stock USING Buys USING (item_id) WHEN NOT MATCHED AND true THEN INSERT WHEN NOT MATCHED AND error('this should not be executed') THEN INSERT WHEN NOT MATCHED THEN ERROR
SELECT COUNT(*) FROM Stock
FROM Stock
# file: test/sql/merge/merge_into_index.test
# setup
CREATE TABLE Accounts(id INTEGER, username VARCHAR PRIMARY KEY, favorite_numbers INT[])
# query
CREATE TABLE Accounts(id INTEGER, username VARCHAR PRIMARY KEY, favorite_numbers INT[])
INSERT INTO Accounts VALUES (1, 'user1', NULL)
MERGE INTO Accounts USING ( VALUES (1, 'user2', [1, 2, 3]) ) new_account(id) USING (id) WHEN MATCHED THEN UPDATE WHEN NOT MATCHED THEN INSERT
FROM Accounts WHERE username='user2'
# file: test/sql/merge/merge_into_insert_star.test
# setup
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
# query
MERGE INTO Stock USING (VALUES (5, 10)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT *
MERGE INTO Stock USING (VALUES (6, 12)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT
MERGE INTO Stock USING (VALUES (0, 7)) new_accounts(balance, item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
MERGE INTO Stock USING (VALUES (12)) new_accounts(item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
# reject
MERGE INTO Stock USING (VALUES (0, 7)) new_accounts(balanc, item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
# file: test/sql/merge/merge_into_invalid_action.test
# setup
CREATE TABLE t AS SELECT range a FROM generate_series(0,9) t(range)
# query
CREATE TABLE t AS SELECT range a FROM generate_series(0,9) t(range)
# reject
MERGE INTO t USING (SELECT range a from generate_series (10,19) t(range)) AS s USING(a) WHEN NOT MATCHED BY TARGET THEN DELETE RETURNING merge_action, *
MERGE INTO t USING (SELECT range a from generate_series (10,19) t(range)) AS s USING(a) WHEN NOT MATCHED BY TARGET THEN UPDATE RETURNING merge_action, *
# file: test/sql/merge/merge_into_invalid_column.test
# setup
CREATE TABLE v0 (v1 INTEGER PRIMARY KEY)
# query
CREATE TABLE v0 (v1 INTEGER PRIMARY KEY)
# reject
INSERT INTO v0(x) VALUES (2) ON CONFLICT DO NOTHING
# file: test/sql/merge/merge_into_join_as_filter.test
# setup
create table foo (bar integer)
create or replace table aaa (id int, status varchar, flag int, starttime datetime, endtime datetime)
# query
create table foo (bar integer)
insert into foo values (1)
merge into foo as f using (select 2 as bar) b on f.bar is not null when matched then update when not matched then insert
FROM foo
create or replace table aaa (id int, status varchar, flag int, starttime datetime, endtime datetime)
merge into aaa using ( select 1 as id, 'xx' as status, 1 as flag, now() as starttime, null as endtime ) as upserts on (upserts.id = aaa.id and aaa.flag =1::int and aaa.status = upserts.status) when matched then update set endtime = upserts.starttime when not matched then insert by name
# file: test/sql/merge/merge_into_multiple_updates.test
# setup
CREATE TABLE Entry(type varchar, number int, text varchar, country VARCHAR, date DATE)
CREATE TABLE NewEntry(type varchar, number int, text varchar, country VARCHAR, date DATE)
# query
CREATE TABLE Entry(type varchar, number int, text varchar, country VARCHAR, date DATE)
INSERT INTO Entry VALUES ('number', 50, NULL, NULL, NULL), ('text', NULL, 'Hello', NULL, NULL), ('country', NULL, NULL, 'Netherlands', NULL), ('date', NULL, NULL, NULL, DATE '2000-01-01')
CREATE TABLE NewEntry(type varchar, number int, text varchar, country VARCHAR, date DATE)
INSERT INTO NewEntry VALUES ('number', 100, NULL, NULL, NULL), ('text', NULL, 'World', NULL, NULL), ('country', NULL, NULL, 'Germany', NULL), ('date', NULL, NULL, NULL, DATE '2010-01-01')
MERGE INTO Entry USING NewEntry ON Entry.type=NewEntry.type WHEN MATCHED AND Entry.type='number' THEN UPDATE SET number=NewEntry.number WHEN MATCHED AND Entry.type='text' THEN UPDATE SET text=NewEntry.text WHEN MATCHED AND Entry.type='country' THEN UPDATE SET country=NewEntry.country WHEN MATCHED AND Entry.type='date' THEN UPDATE SET date=NewEntry.date WHEN MATCHED THEN ERROR
FROM Entry ORDER BY type
# file: test/sql/merge/merge_into_parenthesis_bug.test
# setup
CREATE TABLE my_timeseries (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
CREATE TABLE my_timeseries_new (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
# query
CREATE TABLE my_timeseries (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
insert into my_timeseries VALUES ('2025-09-15', 43, 39)
CREATE TABLE my_timeseries_new (ts TIMESTAMP, x DOUBLE PRECISION, y DOUBLE PRECISION)
insert into my_timeseries_new VALUES ('2025-09-15', 43, 39)
MERGE INTO my_timeseries old USING my_timeseries_new new ON ( old.x = new.x AND ( old.ts != new.ts OR old.x = 1 ) ) WHEN MATCHED THEN UPDATE
MERGE INTO my_timeseries old USING my_timeseries_new new USING(ts) WHEN MATCHED AND ( old.x IS DISTINCT FROM new.y ) THEN UPDATE
# file: test/sql/merge/merge_into_returning.test
# setup
CREATE TABLE Stock(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
CREATE TABLE Sale(item_id int, volume int)
# query
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON FALSE WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) RETURNING merge_action, *
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id) WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) RETURNING *
MERGE INTO Stock AS s USING Buy AS b ON s.item_id = b.item_id WHEN MATCHED THEN UPDATE SET balance = balance + b.volume WHEN NOT MATCHED THEN INSERT VALUES (b.item_id, b.volume) RETURNING *, merge_action
MERGE INTO Stock USING Sale ON Stock.item_id = Sale.item_id WHEN MATCHED AND Sale.volume > balance THEN ERROR WHEN MATCHED AND Sale.volume = balance THEN DELETE WHEN MATCHED AND TRUE THEN UPDATE SET balance = balance - Sale.volume WHEN MATCHED THEN ERROR WHEN NOT MATCHED THEN ERROR RETURNING Stock.item_id, merge_action, Stock.balance
WITH deleted_stocks(item_id) AS (VALUES (30)) MERGE INTO Stock USING deleted_stocks ON Stock.item_id = deleted_stocks.item_id WHEN MATCHED THEN DELETE RETURNING *, merge_action
# file: test/sql/merge/merge_into_subquery.test
# setup
CREATE TABLE Totals(item_id int, balance int)
CREATE TABLE Buy(item_id int, volume int)
# query
CREATE TABLE Totals(item_id int, balance int)
INSERT INTO Buy values(10, 1000), (30, 300), (20, 2000)
MERGE INTO Totals USING (VALUES (10), (30)) Updates(item_id) ON Totals.item_id = Updates.item_id WHEN MATCHED THEN UPDATE SET balance = (SELECT SUM(volume) FROM Buy WHERE item_id=Totals.item_id) WHEN NOT MATCHED THEN INSERT VALUES (Updates.item_id, (SELECT SUM(volume) FROM Buy WHERE item_id=Updates.item_id))
FROM Totals ORDER BY ALL
INSERT INTO Buy values(10, 2000)
MERGE INTO Totals USING (VALUES (10), (20)) Updates(item_id) ON Totals.item_id = Updates.item_id WHEN MATCHED THEN UPDATE SET balance = (SELECT SUM(volume) FROM Buy WHERE item_id=Totals.item_id) WHEN NOT MATCHED THEN INSERT VALUES (Updates.item_id, (SELECT SUM(volume) FROM Buy WHERE item_id=Updates.item_id))
# file: test/sql/merge/merge_into_subquery_action.test
# setup
CREATE TABLE Totals(item_id int, balance int, biggest_item BOOL)
CREATE TABLE Buy(item_id int, volume int)
CREATE TABLE dummy_edge(id INTEGER, ref_id INTEGER, "value" VARCHAR, note VARCHAR)
CREATE TABLE dummy_user(user_id INTEGER, "name" VARCHAR, email VARCHAR, created_at DATE)
CREATE TABLE dummy_null(id INTEGER, "value" INTEGER, optional_text VARCHAR)
# query
CREATE TABLE Totals(item_id int, balance int, biggest_item BOOL)
MERGE INTO Totals USING Buy USING (item_id) WHEN NOT MATCHED AND Buy.volume = (SELECT MAX(Volume) FROM Buy) THEN INSERT VALUES (Buy.item_id, Buy.volume, true) WHEN NOT MATCHED THEN INSERT VALUES (Buy.item_id, Buy.volume, false)
SELECT * FROM Totals ORDER BY item_id
CREATE TABLE dummy_edge(id INTEGER, ref_id INTEGER, "value" VARCHAR, note VARCHAR)
CREATE TABLE dummy_user(user_id INTEGER, "name" VARCHAR, email VARCHAR, created_at DATE)
CREATE TABLE dummy_null(id INTEGER, "value" INTEGER, optional_text VARCHAR)
MERGE INTO main.dummy_edge as target_0 USING dummy_user as ref_0 ON target_0.note = ref_0.name WHEN NOT MATCHED AND EXISTS ( SELECT id FROM main.dummy_null WHERE true ) THEN DO NOTHING
# file: test/sql/merge/merge_into_subquery_condition.test
# setup
CREATE TABLE target(id INT PRIMARY KEY, val INT)
# query
CREATE TABLE target(id INT PRIMARY KEY, val INT)
INSERT INTO target VALUES (1, 10), (2, 20)
MERGE INTO target AS t USING (VALUES (1, 99)) AS s(id, val) ON t.id = s.id AND t.val > (SELECT 5) WHEN MATCHED THEN UPDATE SET val = s.val
FROM target ORDER BY id
# file: test/sql/merge/merge_into_too_few_columns.test
# setup
CREATE TABLE people (id INTEGER, name VARCHAR, salary FLOAT)
# query
CREATE TABLE people (id INTEGER, name VARCHAR, salary FLOAT)
INSERT INTO people VALUES (1, 'John', 92_000.0), (2, 'Anna', 100_000.0)
# reject
MERGE INTO people USING ( SELECT 3 AS id, 89_000.0 AS salary ) AS upserts ON (upserts.id = people.id) WHEN NOT MATCHED THEN INSERT
# file: test/sql/merge/merge_into_update_star.test
# setup
CREATE TABLE Stock(item_id int, balance int DEFAULT 0)
# query
INSERT INTO Stock (item_id) VALUES (5), (10), (20)
MERGE INTO Stock USING (VALUES (5, 10)) new_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE
MERGE INTO Stock USING (VALUES (10, 30)) new_accounts(item_id, balance) USING (item_id) WHEN MATCHED THEN UPDATE SET *
MERGE INTO Stock USING (VALUES (100, 20)) new_accounts(balance, item_id) USING (item_id) WHEN MATCHED THEN UPDATE BY NAME
# reject
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE SET *
MERGE INTO Stock USING (VALUES (10, 20)) new_accounts(item_id, balanc) USING (item_id) WHEN MATCHED THEN UPDATE BY NAME
# file: test/sql/create/create_as.test
# setup
CREATE TABLE tbl2 AS SELECT 2 AS f
CREATE OR REPLACE TABLE tbl3 AS SELECT 3
CREATE OR REPLACE TABLE tbl1 AS SELECT 5 WHERE false
CREATE OR REPLACE TABLE tbl4(col1, col2) AS SELECT 2, 'duck'
CREATE OR REPLACE TABLE tbl5(col1, "col need ' quote") AS SELECT 3.5, 'quote'
CREATE TABLE tbl6(col1) AS SELECT 4 ,'mismatch'
CREATE TABLE tbl8 AS SELECT 42 WITH NO DATA
CREATE TABLE tbl9 AS SELECT 42 WITH DATA
# query
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
# reject
CREATE TABLE tbl1 AS SELECT 3
CREATE TABLE tbl4 IF NOT EXISTS AS SELECT 4
CREATE OR REPLACE TABLE tbl4 IF NOT EXISTS AS SELECT 4
CREATE TABLE tbl7(col1, col2) AS SELECT 5
# file: test/sql/create/create_as_issue_11968.test
# setup
CREATE TABLE test (x INTEGER[])
CREATE TABLE test2 AS SELECT x FROM test
# query
CREATE TABLE test (x INTEGER[])
INSERT INTO test SELECT CASE WHEN x <= 520 THEN [0, 0] ELSE [0] END FROM generate_series(1, 2048) s(x)
CREATE TABLE test2 AS SELECT x FROM test
# file: test/sql/create/create_as_partition_sorted_options.test
# query
pragma enable_verification
SET VARIABLE location_var='boop'
# reject
CREATE TABLE integers PARTITIONED BY (i) as select range i from range(10)
CREATE TABLE integers SORTED BY (i) as select range i from range(10)
CREATE TABLE integers PARTITIONED BY (i) SORTED BY (i) as select range i from range(10)
CREATE TABLE iceberg_table WITH ( 'location' = 's3://amzn-s3-demo-bucket/iceberg-folder', 'table_type'='ICEBERG', 'format'='parquet' ) as select range i from range(10)
CREATE TABLE iceberg_table PARTITIONED BY (id) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table SORTED BY (date) PARTITIONED BY (id) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range id, range::DATE date from range(10)
# file: test/sql/create/create_index_on_issue_13643.test
# setup
CREATE SCHEMA db0
CREATE TABLE t0 (a BIGINT PRIMARY KEY, b INT, c INT)
CREATE INDEX t0_idx ON t0 (b)
CREATE UNIQUE INDEX t0_uidx ON t0 (c)
CREATE UNIQUE INDEX t0_uidx2 ON db0.t0 (c)
# query
CREATE SCHEMA db0
USE db0
CREATE TABLE t0 (a BIGINT PRIMARY KEY, b INT, c INT)
CREATE INDEX t0_idx ON t0 (b)
CREATE UNIQUE INDEX t0_uidx ON t0 (c)
CREATE UNIQUE INDEX t0_uidx2 ON db0.t0 (c)
# file: test/sql/create/create_objects_readonly.test
# setup
create table t1 as select 'c1' as c1
# query
create table t1 as select 'c1' as c1
# reject
CREATE schema s2
CREATE TABLE test AS SELECT * FROM range(10) t(i)
CREATE view v1 AS SELECT * FROM range(10) t(i)
CREATE macro add(a, b) AS a + b
CREATE TYPE mood AS ENUM ('happy', 'sad', 'curious')
CREATE SEQUENCE serial START 101
# file: test/sql/create/create_or_replace.test
# setup
CREATE TABLE IF NOT EXISTS integers(i INTEGER)
CREATE VIEW integers2 AS SELECT 42
# query
CREATE OR REPLACE TABLE integers(i INTEGER, j INTEGER)
CREATE VIEW integers2 AS SELECT 42
CREATE TABLE IF NOT EXISTS integers(i INTEGER)
INSERT INTO integers VALUES (1, 2)
# reject
CREATE OR REPLACE TABLE integers2(i INTEGER)
CREATE OR REPLACE TABLE IF NOT EXISTS integers(i INTEGER)
# file: test/sql/create/create_table_as_duplicate_names.test
# setup
CREATE TABLE t1 AS SELECT i, i FROM range(5) tbl(i)
CREATE TABLE t2 AS SELECT i, i, i, i FROM range(5) tbl(i)
CREATE TABLE t3 AS SELECT tbl1.i, tbl2.i FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i
CREATE TABLE t4 AS SELECT * FROM range(5) tbl1(i) JOIN range(5) tbl2(i) ON tbl1.i=tbl2.i
# query
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
# file: test/sql/create/create_table_compression.test
# setup
CREATE TABLE T (a INTEGER USING COMPRESSION RLE, b INTEGER USING COMPRESSION BITPACKING, C INTEGER USING COMPRESSION UNCOMPRESSED)
# query
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
# reject
CREATE TABLE T (a INTEGER USING COMPRESSION 'bla')
CREATE TABLE T (a INTEGER USING COMPRESSION )
CREATE TABLE T (a INTEGER NOT NULL USING COMPRESSION )
CREATE TABLE T (a INTEGER USING COMPRESSION bla)
# file: test/sql/create/create_table_empty_name.test
# setup
create schema s1
# query
create schema s1
# reject
CREATE TABLE '' AS SELECT 42
CREATE TABLE s1.""(i INTEGER)
# file: test/sql/create/create_table_with_arraybounds.test
# setup
create schema schema2
create schema db2.schema3
create type schema2.foo as VARCHAR
create type db2.schema3.bar as BOOL
create table T ( vis enum ('hide', 'visible')[] )
create table B ( vis schema2.foo[] )
create table C ( vis db2.schema3.bar[] )
# query
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
# file: test/sql/alter/alter_table_set_table_options.test
# setup
CREATE TABLE tbl1(i INTEGER)
# query
CREATE TABLE tbl1(i INTEGER)
set variable location='my/location/path'
# reject
ALTER TABLE tbl1 SET ('foo'='baz', 'buzz'='wizz')
ALTER TABLE tbl1 SET (foo='baz', buzz='wizz')
ALTER TABLE tbl1 SET (foo=getvariable('location'))
ALTER TABLE tbl1 RESET ('foo', 'buzz')
ALTER TABLE tbl1 RESET (foo, buzz)
ALTER TABLE tbl1 RESET ('foo'='baz', 'buzz'='wizz')
ALTER TABLE tbl1 RESET (foo='baz', buzz='wizz')
ALTER TABLE tbl1 RESET ()
# file: test/sql/alter/test_alter_database_rename.test
# setup
CREATE TABLE test_db.sample AS SELECT i FROM range(100) t(i)
# query
ATTACH ':memory:' AS test_db
CREATE TABLE test_db.sample AS SELECT i FROM range(100) t(i)
SELECT COUNT(*) FROM test_db.sample
ALTER DATABASE test_db SET ALIAS TO renamed_db
SELECT COUNT(*) FROM renamed_db.sample
ALTER DATABASE IF EXISTS non_existent SET ALIAS TO something_else
ATTACH ':memory:' AS another_db
# reject
ALTER DATABASE non_existent SET ALIAS TO something_else
ALTER DATABASE another_db SET ALIAS TO renamed_db
ALTER DATABSE renamed_db RENAME TO system
ALTER DATABSE renamed_db RENAME TO temp
# file: test/sql/alter/test_alter_if_exists.test
# setup
CREATE TABLE t0 (c0 INT)
CREATE TABLE unit (price INTEGER, amount_sold INTEGER)
# query
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
# reject
INSERT INTO t0 VALUES (42)
ALTER TABLE t0 ADD COLUMN c0 int
ALTER TABLE t1 DROP COLUMN IF EXISTS c3
ALTER TABLE t1 DROP COLUMN c3
ALTER TABLE t0 DROP COLUMN c3
ALTER TABLE IF EXISTS t0 DROP COLUMN c3
ALTER TABLE IF EXISTS t1 ALTER COLUMN IF EXISTS c0 TYPE varchar
# file: test/sql/alter/alter_col/test_drop_not_null.test
# setup
CREATE TABLE test2(i INTEGER, j INTEGER)
CREATE TABLE test(i AS (1), j INTEGER NOT NULL)
CREATE TEMPORARY TABLE temp_drop_not_null_test(x INTEGER NOT NULL)
# query
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
# reject
INSERT INTO test VALUES (3, NULL)
INSERT INTO test VALUES (NULL)
# file: test/sql/alter/alter_col/test_not_null_in_tran.test
# setup
CREATE TABLE t(i INTEGER, j INTEGER)
# query
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
# reject
ALTER TABLE t ALTER COLUMN j SET NOT NULL
INSERT INTO t VALUES(8888, NULL)
INSERT INTO t VALUES(NULL)
INSERT INTO t VALUES(3, NULL)
# file: test/sql/alter/alter_col/test_not_null_multi_tran.test
# setup
CREATE TABLE t(i INTEGER, j INTEGER)
# query
INSERT INTO t VALUES(7, 7)
SELECT i FROM t
SELECT count(*) from t
# reject
INSERT INTO t VALUES(7777, NULL)
INSERT INTO t VALUES(1, 1)
INSERT INTO t VALUES(1, NULL)
INSERT INTO t VALUES(2, NULL)
# file: test/sql/alter/alter_col/test_set_not_null.test
# setup
CREATE TABLE t0(c0 AS (1), c1 INT)
CREATE TABLE t(i AS (1), j INTEGER)
CREATE TEMPORARY TABLE temp_not_null_test(x INTEGER)
# query
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
# reject
INSERT INTO t VALUES (3, NULL)
INSERT INTO t VALUES (6, NULL)
INSERT INTO t VALUES (2, null)
ALTER TABLE t ALTER COLUMN i SET NOT NULL
INSERT INTO t VALUES (null)
# file: test/sql/alter/rename_table/test_rename_bug4455_schema.test
# setup
create schema public
create table a1 (c int)
create view v1 as select 42
# query
create schema public
set schema=public
create table a1 (c int)
alter table public.a1 rename to a2
alter table a2 rename to a3
create view v1 as select 42
alter view public.v1 rename to v2
alter view v2 rename to v3
# file: test/sql/alter/rename_table/test_rename_table.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE TABLE tbl2(i INTEGER)
CREATE TABLE tbl3(i INTEGER)
CREATE TABLE tbl4(i INTEGER)
CREATE TEMPORARY TABLE temp_tbl(i INTEGER)
# query
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
# reject
SELECT * FROM tbl5
# file: test/sql/alter/rename_table/test_rename_table_case.test
# setup
create table MY_TABLE (i integer)
# query
create table MY_TABLE (i integer)
insert into MY_TABLE values(42)
alter table MY_TABLE rename to my_table
select * from my_table
select * from MY_TABLE
# file: test/sql/alter/rename_table/test_rename_table_chain_commit.test
# setup
CREATE TABLE entry(k INTEGER)
# query
CREATE TABLE entry(i INTEGER)
INSERT INTO entry VALUES (1)
SELECT * FROM entry
ALTER TABLE entry RENAME TO entry2
CREATE TABLE entry(j INTEGER)
INSERT INTO entry VALUES (2)
ALTER TABLE entry2 RENAME TO entry3
CREATE TABLE entry(k INTEGER)
ALTER TABLE entry3 RENAME TO entry4
# reject
SELECT * FROM entry2
SELECT * FROM entry3
SELECT * FROM entry4
# file: test/sql/alter/rename_table/test_rename_table_collision.test
# setup
CREATE TABLE t1(i INTEGER)
CREATE TABLE t2 (i integer)
CREATE TABLE e1 (i INTEGER)
CREATE TABLE e2 (i INTEGER)
# query
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
# reject
SELECT i FROM t3 ORDER BY i
CREATE TABLE t3 (i INTEGER)
ALTER TABLE t2 RENAME TO t4
ALTER TABLE e1 RENAME TO e2
ALTER TABLE e2 RENAME TO e1
# file: test/sql/alter/rename_table/test_rename_table_constraints.test
# setup
CREATE TABLE tbl(i INTEGER PRIMARY KEY, j INTEGER CHECK(j < 10))
# query
CREATE TABLE tbl(i INTEGER PRIMARY KEY, j INTEGER CHECK(j < 10))
INSERT INTO tbl VALUES (999, 4), (1000, 5)
INSERT INTO tbl VALUES (9999, 0), (10000, 1)
ALTER TABLE tbl RENAME TO new_tbl
INSERT INTO new_tbl VALUES (66, 6), (55, 5)
# reject
INSERT INTO tbl VALUES (777, 10), (888, 10)
INSERT INTO new_tbl VALUES (999, 0), (1000, 1)
INSERT INTO new_tbl VALUES (9999, 0), (10000, 1)
INSERT INTO new_tbl VALUES (1, 10), (2, 999)
# file: test/sql/alter/rename_table/test_rename_table_many_transactions.test
# setup
CREATE TABLE tbl1(i INTEGER)
# query
INSERT INTO tbl1 VALUES (999), (100)
ALTER TABLE tbl1 RENAME TO tbl2
# file: test/sql/alter/rename_table/test_rename_table_transactions.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE TABLE tbl2(i INTEGER)
# query
DROP TABLE tbl2
# file: test/sql/alter/rename_table/test_rename_table_view.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE VIEW v1 AS SELECT * FROM tbl
# query
CREATE VIEW v1 AS SELECT * FROM tbl
SELECT * FROM v1
# reject
ALTER TABLE v1 RENAME TO v2
# file: test/sql/alter/rename_table/test_rename_table_with_dependency_check.test
# setup
CREATE TABLE t0 (c0 INT)
CREATE TABLE t3 (c0 INT)
CREATE UNIQUE INDEX i1 ON t0 (c0)
# query
CREATE UNIQUE INDEX i1 ON t0 (c0)
CREATE TABLE t3 (c0 INT)
DROP TABLE t0
# reject
ALTER TABLE t0 RENAME TO t3
ALTER TABLE t0 RENAME TO t4
ANALYZE t4
# file: test/sql/alter/rename_table/test_rename_table_with_insert_transaction.test
# setup
CREATE TABLE t1 (i INTEGER)
# query
CREATE TABLE t1 (i INTEGER)
INSERT INTO t1 VALUES (1)
INSERT INTO t1 VALUES (2)
SELECT * FROM t2
# file: test/sql/alter/alter_type/alter_type_struct.test
# setup
CREATE TABLE test AS SELECT {'t': 42} t
# query
CREATE TABLE test AS SELECT {'t': 42} t
ALTER TABLE test ALTER t TYPE ROW(t VARCHAR) USING {'t': concat('hello', (test.t.t + 42)::varchar)}
ALTER TABLE test ALTER t TYPE ROW(t VARCHAR) USING {'t': concat('hello', (t.t + 42)::varchar)}
# file: test/sql/alter/alter_type/test_alter_type.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE TABLE tbl (col STRUCT(i INT))
# query
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
# reject
ALTER TABLE test ALTER not_a_column SET DATA TYPE INTEGER
ALTER TABLE tbl ALTER col TYPE
# file: test/sql/alter/alter_type/test_alter_type_check.test
# setup
CREATE TABLE test(i INTEGER CHECK(i < 10), j INTEGER)
# query
CREATE TABLE test(i INTEGER CHECK(i < 10), j INTEGER)
ALTER TABLE test ALTER j SET DATA TYPE VARCHAR
# file: test/sql/alter/alter_type/test_alter_type_dependencies.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
PREPARE v1 AS SELECT * FROM test
EXECUTE v1
ALTER TABLE test ALTER i TYPE VARCHAR USING i::VARCHAR
ALTER TABLE test ALTER i TYPE INTEGER USING i::INTEGER
PREPARE v2 AS SELECT i+$1 FROM test
EXECUTE v2(1)
# reject
EXECUTE v2
# file: test/sql/alter/alter_type/test_alter_type_expression.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ALTER i TYPE BIGINT USING i+100
# file: test/sql/alter/alter_type/test_alter_type_index.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE INDEX i_index ON test(i)
# query
CREATE INDEX i_index ON test(i)
DROP INDEX i_index
# file: test/sql/alter/alter_type/test_alter_type_local.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
INSERT INTO test VALUES (3, 3)
ALTER TABLE test ALTER i SET DATA TYPE BIGINT
# file: test/sql/alter/alter_type/test_alter_type_multi_column.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ALTER i TYPE INTEGER USING 2*(i+j)
# file: test/sql/alter/alter_type/test_alter_type_not_null.test
# setup
CREATE TABLE test(i INTEGER NOT NULL, j INTEGER)
# query
CREATE TABLE test(i INTEGER NOT NULL, j INTEGER)
INSERT INTO test VALUES ('hello', 3)
# reject
INSERT INTO test VALUES (NULL, 4)
# file: test/sql/alter/alter_type/test_alter_type_rollback.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
UPDATE test SET i='hello'
# file: test/sql/alter/alter_type/test_alter_type_transactions.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ALTER j TYPE VARCHAR
# reject
ALTER TABLE test ALTER i TYPE VARCHAR
INSERT INTO test (i, j) VALUES (3, 3)
DELETE FROM test WHERE i=1
UPDATE test SET i=1000
UPDATE test SET j=100
CREATE INDEX i_index ON test(j)
# file: test/sql/alter/alter_type/test_alter_type_unique.test
# setup
CREATE TABLE test(i INTEGER UNIQUE, j INTEGER)
# query
CREATE TABLE test(i INTEGER UNIQUE, j INTEGER)
# file: test/sql/alter/alter_type/test_alter_type_with_generated_column.test
# setup
CREATE TABLE test(i AS (1), j INTEGER)
# query
CREATE TABLE test(i AS (1), j INTEGER)
# file: test/sql/alter/rename_col/test_rename_col.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test RENAME COLUMN i TO k
# file: test/sql/alter/rename_col/test_rename_col_check.test
# setup
CREATE TABLE test(i INTEGER CHECK(i < 10), j INTEGER)
# query
INSERT INTO test (i, j) VALUES (1, 2), (2, 3)
INSERT INTO test (k, j) VALUES (1, 2), (2, 3)
# reject
INSERT INTO test (i, j) VALUES (100, 2)
INSERT INTO test (k, j) VALUES (100, 2)
# file: test/sql/alter/rename_col/test_rename_col_dependencies.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
PREPARE v1 AS SELECT i, j FROM test
PREPARE v2 AS SELECT * FROM test
# file: test/sql/alter/rename_col/test_rename_col_failure.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
SELECT i, j FROM test
# reject
ALTER TABLE test RENAME COLUMN blablabla TO k
ALTER TABLE test RENAME COLUMN i TO j
# file: test/sql/alter/rename_col/test_rename_col_rollback.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
START TRANSACTION
SELECT k FROM test
# reject
SELECT i FROM test
# file: test/sql/alter/rename_col/test_rename_col_transactions.test
# setup
CREATE TABLE test( i INTEGER, j INTEGER )
# query
CREATE TABLE test( i INTEGER, j INTEGER )
# file: test/sql/alter/rename_col/test_rename_col_unique.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER, PRIMARY KEY(i, j))
# query
CREATE TABLE test(i INTEGER, j INTEGER, PRIMARY KEY(i, j))
INSERT INTO test (i, j) VALUES (1, 1), (2, 2)
INSERT INTO test (k, j) VALUES (3, 3), (4, 4)
# reject
INSERT INTO test (i, j) VALUES (1, 1)
INSERT INTO test (k, j) VALUES (1, 1)
# file: test/sql/alter/default/drop_default.test
# setup
CREATE TABLE data(id INTEGER, x INTEGER)
# query
CREATE TABLE data(id INTEGER, x INTEGER)
ALTER TABLE data ALTER COLUMN id DROP DEFAULT
INSERT INTO data VALUES (1, 0), (2, 1)
ALTER TABLE data ALTER COLUMN x DROP DEFAULT
# reject
ALTER TABLE data ALTER COLUMN j DROP DEFAULT
# file: test/sql/alter/default/test_set_default.test
# setup
CREATE SEQUENCE seq
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE TABLE constrainty(i INTEGER PRIMARY KEY, j INTEGER)
CREATE TEMPORARY TABLE temp_default_test(x INTEGER DEFAULT 1)
# query
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
# reject
ALTER TABLE test ALTER blabla SET DEFAULT 3
ALTER TABLE test ALTER blabla DROP DEFAULT
# file: test/sql/alter/add_col/test_add_col.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ADD COLUMN k INTEGER
# file: test/sql/alter/add_col/test_add_col_chain.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ADD COLUMN l INTEGER
ALTER TABLE test ADD COLUMN m INTEGER DEFAULT 3
# file: test/sql/alter/add_col/test_add_col_default.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ADD COLUMN l INTEGER DEFAULT 3
SELECT i, j, l FROM test
# file: test/sql/alter/add_col/test_add_col_default_seq.test
# setup
CREATE SEQUENCE seq
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test ADD COLUMN m INTEGER DEFAULT nextval('seq')
ALTER TABLE test ADD COLUMN n INTEGER DEFAULT currval('seq')
# file: test/sql/alter/add_col/test_add_col_incorrect.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE VIEW x(x) AS (SELECT 1)
# query
CREATE VIEW x(x) AS (SELECT 1)
# reject
ALTER TABLE test ADD COLUMN i INTEGER
ALTER VIEW x ADD COLUMN i INTEGER
ALTER TABLE i ADD COLUMN j INT, ADD COLUMN k INT
# file: test/sql/alter/add_col/test_add_col_index.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE INDEX i_index ON test(k)
# query
ALTER TABLE test ADD COLUMN k INTEGER DEFAULT 2
CREATE INDEX i_index ON test(k)
INSERT INTO test VALUES (3, 3, 3)
SELECT * FROM test WHERE k=2
SELECT * FROM test WHERE k=3
# file: test/sql/alter/add_col/test_add_col_stats.test
# setup
CREATE SEQUENCE seq
CREATE TABLE test(i INTEGER, j INTEGER)
# query
SELECT * FROM test WHERE m=2
SELECT stats(m) FROM test LIMIT 1
# file: test/sql/alter/add_col/test_add_col_user_type.test
# setup
CREATE SCHEMA test_schema
CREATE TYPE main_int AS int32
CREATE TYPE test_schema.test_int AS int32
CREATE TABLE test_schema.test_t1 (i INT)
CREATE TABLE main_t1 (i INT)
# query
CREATE SCHEMA test_schema
CREATE TYPE main_int AS int32
CREATE TYPE test_schema.test_int AS int32
CREATE TABLE test_schema.test_t1 (i INT)
CREATE TABLE main_t1 (i INT)
ALTER TABLE test_schema.test_t1 ADD COLUMN not_found main_int
ALTER TABLE test_schema.test_t1 ADD COLUMN l test_int
# reject
ALTER TABLE main_t1 ADD COLUMN j test_int
# file: test/sql/alter/struct/add_col_nested_struct.test
# setup
CREATE TABLE test(s STRUCT(s2 STRUCT(v1 INT, v2 INT)))
# query
CREATE TABLE test(s STRUCT(s2 STRUCT(v1 INT, v2 INT)))
INSERT INTO test VALUES (ROW(ROW(1, 1))), (ROW(ROW(2, 2)))
ALTER TABLE test ADD COLUMN s.s2.k INTEGER
ALTER TABLE test ADD COLUMN IF NOT EXISTS s.s2.v1 VARCHAR
ALTER TABLE test ADD COLUMN s.i INTEGER DEFAULT 100
# reject
ALTER TABLE test ADD COLUMN s.s2.v1 VARCHAR
ALTER TABLE test ADD COLUMN s.s2.v1.x INTEGER
# file: test/sql/alter/struct/add_col_struct.test
# setup
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER))
# query
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER))
INSERT INTO test VALUES (ROW(1, 1)), (ROW(2, 2))
ALTER TABLE test ADD COLUMN s.k INTEGER
ALTER TABLE test ADD COLUMN s.l INTEGER DEFAULT 42
ALTER TABLE test ADD COLUMN s.m INTEGER DEFAULT 42
ALTER TABLE test ADD COLUMN IF NOT EXISTS s.i VARCHAR
# reject
ALTER TABLE test ADD COLUMN s.i VARCHAR
ALTER TABLE test ADD COLUMN s.i.a INTEGER
ALTER TABLE test ADD COLUMN s.x.a INTEGER
# file: test/sql/alter/struct/drop_col_nested_struct.test
# setup
CREATE TABLE test(s STRUCT(i INT, s2 STRUCT(v1 INT, v2 INT)))
# query
CREATE TABLE test(s STRUCT(i INT, s2 STRUCT(v1 INT, v2 INT)))
INSERT INTO test VALUES (ROW(42, ROW(1, 1))), (ROW(84, ROW(2, 2)))
ALTER TABLE test DROP s.s2.v1
ALTER TABLE test DROP COLUMN IF EXISTS s.s2.v1
ALTER TABLE test DROP COLUMN s.s2
# reject
ALTER TABLE test DROP COLUMN s.s2.v1
# file: test/sql/alter/struct/drop_col_struct.test
# setup
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER))
# query
ALTER TABLE test DROP COLUMN s.i
ALTER TABLE test DROP COLUMN IF EXISTS s.v
# reject
ALTER TABLE test DROP COLUMN s.j
ALTER TABLE test DROP COLUMN s.v
ALTER TABLE test DROP COLUMN s.j.a
ALTER TABLE test DROP COLUMN z.j
ALTER TABLE test DROP COLUMN s.v1.a
# file: test/sql/alter/struct/rename_col_nested_struct.test
# setup
CREATE TABLE test(s STRUCT(s2 STRUCT(v1 INT, v2 INT)))
# query
ALTER TABLE test RENAME s.s2.v1 TO i
ALTER TABLE test RENAME COLUMN s.s2 TO x
# reject
ALTER TABLE test RENAME COLUMN s.s2.v2 TO i
# file: test/sql/alter/struct/rename_col_struct.test
# setup
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER))
# query
ALTER TABLE test RENAME s.i TO v1
ALTER TABLE test RENAME s.j TO v2
# reject
ALTER TABLE test RENAME s.j TO v1
ALTER TABLE test RENAME s.j.x TO v2
ALTER TABLE test RENAME s.i TO v2
ALTER TABLE test RENAME x.i TO v2
# file: test/sql/alter/add_pk/test_add_multi_column_pk.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER, d TEXT)
# query
CREATE TABLE test (i INTEGER, j INTEGER, d TEXT)
INSERT INTO test VALUES (3, 4, 'hello'), (44, 45, '56')
ALTER TABLE test ADD PRIMARY KEY (i, j)
INSERT INTO test VALUES (1, 1, 'foo'), (1, 2, 'bar')
# reject
INSERT INTO test VALUES (1, 2, 'oops')
INSERT INTO test VALUES (NULL, 2, 'nada')
# file: test/sql/alter/add_pk/test_add_pk.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
CREATE TABLE reverse (i INTEGER, j INTEGER)
CREATE TABLE scan (i INTEGER, j INTEGER)
CREATE TEMPORARY TABLE temp_pk_test(x INTEGER)
CREATE VIEW view_test AS SELECT 'hello' AS name, 42 AS value
# query
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
# reject
INSERT INTO test VALUES (2, 1)
INSERT INTO test VALUES (2, NULL)
ALTER TABLE test ADD PRIMARY KEY (i)
INSERT INTO reverse (j, i) VALUES (2, 1)
ALTER TABLE view_test ADD PRIMARY KEY (name)
# file: test/sql/alter/add_pk/test_add_pk_alter_in_tx.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
# query
ALTER TABLE test ALTER COLUMN j SET NOT NULL
# file: test/sql/alter/add_pk/test_add_pk_attach.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
# query
ATTACH ':memory:' as memory
USE memory
DETACH test_add_pk_attach
USE test_add_pk_attach
# file: test/sql/alter/add_pk/test_add_pk_catalog_error.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
CREATE TABLE uniq (i INTEGER UNIQUE, j INTEGER)
# query
CREATE TABLE uniq (i INTEGER UNIQUE, j INTEGER)
INSERT INTO uniq VALUES (1, 10), (2, 20), (3, 30)
ALTER TABLE uniq ADD PRIMARY KEY (i)
# reject
ALTER TABLE test ADD PRIMARY KEY (i_do_not_exist)
ALTER TABLE i_do_not_exist ADD PRIMARY KEY (i, j)
INSERT INTO uniq VALUES (1, 100)
INSERT INTO uniq VALUES (1, 101)
INSERT INTO uniq VALUES (NULL, 100)
# file: test/sql/alter/add_pk/test_add_pk_gaps_in_rowids.test
# setup
CREATE TABLE integers(i integer)
# query
CREATE TABLE integers(i integer)
INSERT INTO integers SELECT * FROM range(50000)
SELECT i FROM integers WHERE i = 100
DELETE FROM integers WHERE i = 42
ALTER TABLE integers ADD PRIMARY KEY (i)
# file: test/sql/alter/add_pk/test_add_pk_invalid_data.test
# setup
CREATE TABLE duplicates (i INTEGER, j INTEGER)
CREATE TABLE nulls (i INTEGER, j INTEGER)
CREATE TABLE nulls_compound (i INTEGER, j INTEGER, k VARCHAR)
# query
CREATE TABLE duplicates (i INTEGER, j INTEGER)
INSERT INTO duplicates VALUES (1, 10), (2, 20), (3, 30), (1, 100)
CREATE TABLE nulls (i INTEGER, j INTEGER)
INSERT INTO nulls VALUES (1, 10), (2, NULL), (3, 30), (4, 40)
DROP TABLE nulls
INSERT INTO nulls VALUES (5, 10), (NULL, 20), (7, 30), (8, 100)
CREATE TABLE nulls_compound (i INTEGER, j INTEGER, k VARCHAR)
INSERT INTO nulls_compound VALUES (1, 10, 'hello'), (2, 20, 'world'), (NULL, NULL, NULL), (3, 100, 'yay')
# reject
ALTER TABLE duplicates ADD PRIMARY KEY (i)
ALTER TABLE nulls ADD PRIMARY KEY (i, j)
ALTER TABLE nulls ADD PRIMARY KEY (i)
ALTER TABLE nulls_compound ADD PRIMARY KEY (k, i)
# file: test/sql/alter/add_pk/test_add_pk_invalid_type.test
# setup
CREATE TABLE test (a INTEGER[], b INTEGER)
# query
CREATE TABLE test (a INTEGER[], b INTEGER)
# reject
ALTER TABLE test ADD PRIMARY KEY (a)
ALTER TABLE test ADD PRIMARY KEY (a, b)
# file: test/sql/alter/add_pk/test_add_pk_naming_conflict.test
# setup
CREATE TABLE tbl (i INTEGER)
CREATE TABLE test (i INTEGER)
CREATE INDEX PRIMARY_tbl_i ON tbl(i)
# query
CREATE TABLE tbl (i INTEGER)
INSERT INTO tbl VALUES (1)
CREATE INDEX PRIMARY_tbl_i ON tbl(i)
CREATE TABLE test (i INTEGER)
INSERT INTO test VALUES (1)
# reject
ALTER TABLE tbl ADD PRIMARY KEY (i)
CREATE INDEX PRIMARY_test_i ON test(i)
# file: test/sql/alter/add_pk/test_add_pk_rollback.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
CREATE TABLE other (i INTEGER, j INTEGER)
# query
INSERT INTO test VALUES (1, 1), (2, 1), (2, NULL)
CREATE TABLE other (i INTEGER, j INTEGER)
INSERT INTO other VALUES (1, 1), (2, 1)
ALTER TABLE other ADD PRIMARY KEY (j)
# file: test/sql/alter/add_pk/test_add_pk_wal.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
# query
PRAGMA disable_checkpoint_on_shutdown
PRAGMA wal_autocheckpoint='1TB'
INSERT INTO test VALUES (1, 2), (3, 4)
# reject
INSERT INTO test VALUES (2, 2)
# file: test/sql/alter/add_pk/test_add_pk_with_generated_column.test
# setup
CREATE TABLE test ( a INT NOT NULL, b INT GENERATED ALWAYS AS (a) VIRTUAL, c INT, )
# query
CREATE TABLE test ( a INT NOT NULL, b INT GENERATED ALWAYS AS (a) VIRTUAL, c INT, )
INSERT INTO test VALUES (5, 4)
ALTER TABLE test ADD PRIMARY KEY (c)
# reject
ALTER TABLE test ADD PRIMARY KEY (b)
ALTER TABLE test ADD PRIMARY KEY (b, c)
INSERT INTO test VALUES (1, 4)
# file: test/sql/alter/add_pk/test_add_same_pk_twice.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
CREATE TABLE other (i INTEGER PRIMARY KEY, j INTEGER)
# query
CREATE TABLE other (i INTEGER PRIMARY KEY, j INTEGER)
# reject
ALTER TABLE other ADD PRIMARY KEY (i, j)
ALTER TABLE other ADD PRIMARY KEY (i)
# file: test/sql/alter/map/add_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a MAP( STRUCT( n INTEGER, m INTEGER ), STRUCT( i INTEGER, j INTEGER ) ) ) )
# query
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
# reject
ALTER TABLE test ADD COLUMN s.a.not_key INTEGER
ALTER TABLE test ADD COLUMN s.a.key INTEGER
# file: test/sql/alter/map/drop_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a MAP( STRUCT( n INTEGER, m INTEGER ), STRUCT( i INTEGER, j INTEGER ) ) ) )
# query
ALTER TABLE test DROP COLUMN s.value.j
ALTER TABLE test DROP COLUMN s.key.n
ALTER TABLE test DROP COLUMN s.a.key.m
ALTER TABLE test DROP COLUMN s.a.value.j
# reject
ALTER TABLE test DROP COLUMN s.key
ALTER TABLE test DROP COLUMN s.value
# file: test/sql/alter/map/rename_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a MAP( STRUCT( n INTEGER, m INTEGER ), STRUCT( i INTEGER, j INTEGER ) ) ) )
# query
ALTER TABLE test RENAME COLUMN s.value.j TO abc
ALTER TABLE test RENAME COLUMN s.key.n TO def
ALTER TABLE test RENAME COLUMN s.a.key.m TO abc
ALTER TABLE test RENAME COLUMN s.a.value.j TO def
# reject
ALTER TABLE test RENAME COLUMN s.key to anything
ALTER TABLE test RENAME COLUMN s.value to anything
# file: test/sql/alter/rename_view/test_rename_view.test
# setup
CREATE VIEW vw AS SELECT i+1 AS i FROM tbl
# query
ALTER VIEW vw RENAME TO vw2
SELECT * FROM vw2
CREATE VIEW vw AS SELECT i+1 AS i FROM tbl
# reject
SELECT * FROM vw
ALTER VIEW sqlite_master RENAME TO my_sqlite_master
ALTER VIEW nonexistingview RENAME TO my_new_view
# file: test/sql/alter/rename_view/test_rename_view_incorrect.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE VIEW vw AS SELECT * FROM tbl
CREATE VIEW vw2 AS SELECT 1729 AS i
# query
CREATE VIEW vw AS SELECT * FROM tbl
CREATE VIEW vw2 AS SELECT 1729 AS i
# reject
ALTER VIEW non_view RENAME TO vw
ALTER VIEW vw2 RENAME TO vw
# file: test/sql/alter/rename_view/test_rename_view_many_transactions.test
# setup
CREATE TABLE tbl1(i INTEGER)
CREATE VIEW vw1 AS SELECT * FROM tbl1
# query
CREATE VIEW vw1 AS SELECT * FROM tbl1
ALTER VIEW vw1 RENAME TO vw2
ALTER VIEW vw2 RENAME TO vw3
ALTER VIEW vw3 RENAME TO vw4
SELECT * FROM vw1
# reject
SELECT * FROM vw3
SELECT * FROM vw4
# file: test/sql/alter/list/add_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a STRUCT(i INTEGER, j INTEGER)[] ) )
# query
WITH cte AS ( SELECT a::STRUCT(i INTEGER, j INTEGER)[] a FROM VALUES ([ROW(1, 1)]), ([ROW(2, 2)]) t(a) ) SELECT remap_struct( a, NULL::STRUCT(i INTEGER, j INTEGER, k INTEGER)[], {'list': ('list', {'i': 'i', 'j': 'j'})}, {'list': {'k': NULL::INTEGER}} ) FROM cte
CREATE TABLE test(s STRUCT(i INTEGER, j INTEGER)[])
INSERT INTO test VALUES ([ROW(1, 1)]), ([ROW(2, 2)])
ALTER TABLE test ADD COLUMN s.element.k INTEGER
CREATE TABLE test( s STRUCT( a STRUCT(i INTEGER, j INTEGER)[] ) )
INSERT INTO test VALUES (ROW([ROW(1, 1)])), (ROW([ROW(2, 2)]))
ALTER TABLE test ADD COLUMN s.a.element.k INTEGER
# reject
ALTER TABLE test ADD COLUMN s.a.not_element INTEGER
# file: test/sql/alter/list/drop_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a STRUCT(i INTEGER, j INTEGER)[] ) )
# query
ALTER TABLE test DROP COLUMN s.element.j
ALTER TABLE test DROP COLUMN s.a.element.i
# reject
ALTER TABLE test DROP COLUMN s.element
# file: test/sql/alter/list/rename_column_in_struct.test
# setup
CREATE TABLE test( s STRUCT( a STRUCT( i INTEGER, j INTEGER )[] ) )
# query
CREATE TABLE test( s STRUCT( i INTEGER, j INTEGER )[] )
ALTER TABLE test RENAME COLUMN s.element.j TO k
CREATE TABLE test( s STRUCT( a STRUCT( i INTEGER, j INTEGER )[] ) )
ALTER TABLE test RENAME COLUMN s.a.element.i TO k
# reject
ALTER TABLE test RENAME COLUMN s.element TO not_element
# file: test/sql/alter/drop_col/test_drop_col.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
ALTER TABLE test DROP COLUMN j
# file: test/sql/alter/drop_col/test_drop_col_check.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER CHECK(j < 10))
CREATE TABLE test2(i INTEGER, j INTEGER CHECK(i+j < 10))
# query
CREATE TABLE test(i INTEGER, j INTEGER CHECK(j < 10))
CREATE TABLE test2(i INTEGER, j INTEGER CHECK(i+j < 10))
SELECT * FROM test2
# reject
ALTER TABLE test2 DROP COLUMN j
# file: test/sql/alter/drop_col/test_drop_col_check_next.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER CHECK(j < 10))
# query
ALTER TABLE test DROP COLUMN i
# reject
INSERT INTO test VALUES (20)
# file: test/sql/alter/drop_col/test_drop_col_concurrent_dml_conflict.test
# setup
CREATE TABLE t1 (id INTEGER PRIMARY KEY, val INTEGER, extra INTEGER)
# query
CREATE TABLE t1 (id INTEGER PRIMARY KEY, val INTEGER, extra INTEGER)
INSERT INTO t1 SELECT i, i * 10, i FROM range(1000) tbl(i)
DELETE FROM t1 WHERE id < 500
ALTER TABLE t1 DROP COLUMN extra
EXPLAIN ANALYZE SELECT val FROM t1 WHERE id = 5
SELECT val FROM t1 WHERE id = 499
EXPLAIN ANALYZE SELECT val FROM t1 WHERE id = 500
SELECT val FROM t1 WHERE id = 500
# file: test/sql/alter/drop_col/test_drop_col_failure.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE TABLE test2 (id INT PRIMARY KEY, name TEXT, surname TEXT, age INT, UNIQUE(surname, age))
# query
ALTER TABLE test DROP COLUMN IF EXISTS blabla
CREATE TABLE test2 (id INT PRIMARY KEY, name TEXT, surname TEXT, age INT, UNIQUE(surname, age))
# reject
ALTER TABLE test DROP COLUMN blabla
ALTER TABLE test2 DROP COLUMN surname
ALTER TABLE test2 DROP COLUMN age
# file: test/sql/alter/drop_col/test_drop_col_not_null_next.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER, k INTEGER NOT NULL)
# query
CREATE TABLE test(i INTEGER, j INTEGER, k INTEGER NOT NULL)
INSERT INTO test VALUES (1, 1, 11), (2, 2, 12)
INSERT INTO test VALUES (3, 13)
# file: test/sql/alter/drop_col/test_drop_col_operations.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# query
INSERT INTO test SELECT i, i FROM range(100) tbl(i)
DELETE FROM test WHERE j%2=0
SELECT COUNT(j), SUM(j) FROM test
UPDATE test SET j=j+100
# file: test/sql/alter/drop_col/test_drop_col_pk.test
# setup
CREATE TABLE test(i INTEGER PRIMARY KEY, j INTEGER)
# query
CREATE TABLE test(i INTEGER PRIMARY KEY, j INTEGER)
# file: test/sql/alter/drop_col/test_drop_col_with_generated_cols.test
# setup
create table t(i int, j as (2), k int, m as (3), n int)
# query
create table t(i int, j as (2), k int, m as (3), n int)
alter table t drop column n
alter table t drop column m
alter table t drop column k
alter table t drop column j
# file: test/sql/index/create_index_options.test
# setup
CREATE TABlE t1 (foo INT)
# query
CREATE TABlE t1 (foo INT)
# reject
CREATE INDEX i3 ON t1 USING random_index_method (foo) WITH (my_option = 2, is_cool)
CREATE INDEX i3 ON t1 USING random_index_method (foo) WITH (my_option = getenv('some_env_variable'), is_cool)
# file: test/sql/index/art/test_art_tx_update_key.test
# setup
CREATE TABLE test_table (id INTEGER PRIMARY KEY)
# query
CREATE TABLE test_table (id INTEGER PRIMARY KEY)
INSERT INTO test_table VALUES (1)
SELECT id FROM test_table LIMIT 1
UPDATE test_table SET id = 1 WHERE id = 1
SELECT rowid FROM test_table LIMIT 1
# file: test/sql/index/art/types/test_art_boolean.test
# setup
CREATE TABLE t0(c0 BOOLEAN, c1 INT)
CREATE INDEX i0 ON t0(c1, c0)
# query
CREATE TABLE t0(c0 BOOLEAN, c1 INT)
CREATE INDEX i0 ON t0(c1, c0)
INSERT INTO t0(c1) VALUES (0)
SELECT * FROM t0
# file: test/sql/index/art/types/test_art_coverage_types.test
# setup
CREATE TABLE duplicate_id (id UINT32, id2 INT64)
CREATE TABLE int128_first (id INT128, id2 INT128)
CREATE TABLE uint8_first (id UINT8, id2 UINT8)
CREATE TABLE uint64_first (id UINT64, id2 UINT32, id3 UINT64, id4 FLOAT)
CREATE TABLE int128_point AS SELECT range::INT128 AS id FROM range(5000)
CREATE TABLE uint64_point AS SELECT range::UINT64 AS id FROM range(5000)
CREATE TABLE uint32_point AS SELECT range::UINT32 AS id FROM range(5000)
CREATE TABLE uint8_point AS SELECT range::UINT8 AS id FROM range(128)
CREATE UNIQUE INDEX idx_1 ON int128_first(id, id2)
CREATE INDEX idx_2 ON uint8_first(id, id2)
CREATE INDEX idx_3 ON uint64_first(id, id2, id3, id4)
CREATE INDEX idx_int128_point ON int128_point(id)
CREATE INDEX idx_uint64_point ON uint64_point(id)
CREATE INDEX idx_uint32_point ON uint32_point(id)
CREATE INDEX idx_uint8_point ON uint8_point(id)
# query
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
# reject
CREATE UNIQUE INDEX idx ON duplicate_id(id, id2)
# file: test/sql/index/art/types/test_art_double.test
# setup
CREATE TABLE numbers(i DOUBLE)
CREATE INDEX i_index ON numbers(i)
# query
CREATE TABLE numbers(i DOUBLE)
INSERT INTO numbers VALUES (CAST(0 AS DOUBLE))
INSERT INTO numbers VALUES (CAST(-0 AS DOUBLE))
CREATE INDEX i_index ON numbers(i)
SELECT COUNT(i) FROM numbers WHERE i = CAST(0 AS DOUBLE)
SELECT COUNT(i) FROM numbers WHERE i = CAST(-0 AS DOUBLE)
# file: test/sql/index/art/types/test_art_expression_key.test
# setup
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR, l BIGINT)
CREATE INDEX i_index ON integers using art((j+l))
# query
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
# file: test/sql/index/art/types/test_art_integer_types.test
# setup
CREATE TABLE integers(i TINYINT, j SMALLINT, k INTEGER, l BIGINT)
CREATE INDEX i_index1 ON integers(i)
CREATE INDEX i_index2 ON integers(j)
CREATE INDEX i_index3 ON integers(k)
CREATE INDEX i_index4 ON integers(l)
# query
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
# file: test/sql/index/art/types/test_art_real.test
# setup
CREATE TABLE numbers(i REAL)
CREATE INDEX i_index ON numbers(i)
# query
CREATE TABLE numbers(i REAL)
INSERT INTO numbers VALUES (CAST(0 AS REAL))
INSERT INTO numbers VALUES (CAST(-0 AS REAL))
SELECT COUNT(i) FROM numbers WHERE i = CAST(0 AS REAL)
SELECT COUNT(i) FROM numbers WHERE i = CAST(-0 AS REAL)
# file: test/sql/index/art/types/test_art_real_pk.test
# setup
CREATE TABLE numbers(i REAL PRIMARY KEY, j INTEGER)
# query
CREATE TABLE numbers(i REAL PRIMARY KEY, j INTEGER)
INSERT INTO numbers VALUES (3.45, 4), (2.2, 5)
SELECT * FROM numbers
INSERT INTO numbers VALUES (6, 6)
# reject
INSERT INTO numbers VALUES (3.45, 4), (3.45, 5)
INSERT INTO numbers VALUES (6, 6), (3.45, 4)
INSERT INTO numbers VALUES (NULL, 4)
UPDATE numbers SET i=NULL
# file: test/sql/index/art/types/test_art_union.test
# setup
CREATE TABLE tbl ( u_2 UNION("string" VARCHAR, "bool" BOOLEAN), u_1 UNION("string" VARCHAR), i INTEGER, u_list UNION("int" INTEGER, "list" INTEGER[], "bool" BOOLEAN))
CREATE INDEX idx_i ON tbl (i)
CREATE UNIQUE INDEX idx_u_2_1 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_u_2_2 ON tbl ((u_2.bool))
CREATE UNIQUE INDEX idx_u_1 ON tbl ((u_1.string))
CREATE UNIQUE INDEX idx_list_1 ON tbl ((u_list.int))
CREATE UNIQUE INDEX idx_list_3 ON tbl ((u_list.bool))
CREATE UNIQUE INDEX idx_c_1 ON tbl ((u_2.string), (u_1.string))
CREATE UNIQUE INDEX idx_c_2 ON tbl ((u_list.int), (u_1.string), (u_2.bool))
# query
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
# reject
CREATE INDEX idx_u_2 ON tbl (u_2)
CREATE INDEX idx_u_1 ON tbl (u_1)
CREATE INDEX idx_u_list ON tbl (u_list)
CREATE INDEX idx_u_list ON tbl (i, u_list)
CREATE UNIQUE INDEX idx_list_2 ON tbl ((u_list.list))
INSERT INTO tbl VALUES ('helloo', 'nop', 7, true)
CREATE INDEX idx_c_fail ON tbl ((u_2.string), u_list)
INSERT INTO tbl VALUES ('sunshine', 'love', 85, true)
# file: test/sql/index/art/types/test_art_varchar.test
# setup
CREATE TABLE strings(i varchar)
CREATE INDEX i_index ON strings(i)
# query
CREATE TABLE strings(i varchar)
CREATE INDEX i_index ON strings(i)
SELECT COUNT(i) FROM strings WHERE i = 'test'
SELECT COUNT(i) FROM strings WHERE i = 'somesuperbigstring'
SELECT COUNT(i) FROM strings WHERE i = 'maybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstring'
SELECT COUNT(i) FROM strings WHERE i = 'maybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstringmaybesomesuperbigstring2'
SELECT COUNT(i) FROM strings WHERE i >= 'somesuperbigstring' and i <='somesuperbigstringz'
SELECT COUNT(i) FROM strings WHERE i = 'somesuperthisdoesnotexist'
DROP TABLE strings
# file: test/sql/index/art/constraints/test_art_compound_key_changes.test
# setup
CREATE TABLE tbl_comp ( a INT, b VARCHAR UNIQUE, gen AS (2 * a), c INT, d VARCHAR, PRIMARY KEY (c, b))
CREATE UNIQUE INDEX unique_idx ON tbl_comp((d || 'hello'), (a + 42))
CREATE INDEX normal_idx ON tbl_comp(d, a, c)
CREATE UNIQUE INDEX lookup_idx ON tbl_comp(c)
# query
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
# reject
INSERT INTO tbl_comp VALUES (2, 'hola', 5, 'world')
INSERT INTO tbl_comp VALUES (3, 'hoi', 1, 'wereld')
INSERT INTO tbl_comp VALUES (42, 'hoi', 2, 'welt')
# file: test/sql/index/art/constraints/test_art_eager_batch_insert.test
# setup
CREATE TABLE test1 (id INT PRIMARY KEY, payload VARCHAR)
CREATE TABLE test2 (id INT PRIMARY KEY, payload VARCHAR)
# query
CREATE TABLE test1 (id INT PRIMARY KEY, payload VARCHAR)
CREATE TABLE test2 (id INT PRIMARY KEY, payload VARCHAR)
INSERT INTO test1 VALUES (1, 'row 1')
INSERT INTO test2 VALUES (1, 'row 1 from test 2')
SELECT id, payload FROM test1
DELETE FROM test1 WHERE id = 1
INSERT INTO test1 SELECT * FROM test2
# file: test/sql/index/art/constraints/test_art_eager_constraint_checking.test
# setup
CREATE TABLE t_7182 (it INTEGER PRIMARY KEY, jt INTEGER)
CREATE TABLE u_7182 (iu INTEGER PRIMARY KEY, ju INTEGER REFERENCES t_7182 (it))
CREATE TABLE tunion_5807 (id INTEGER PRIMARY KEY, u UNION (i int))
CREATE TABLE IF NOT EXISTS workers_5771 ( id INTEGER PRIMARY KEY NOT NULL, worker VARCHAR(150) UNIQUE NOT NULL, phone VARCHAR(20) NOT NULL)
CREATE TABLE test_4886 (i INTEGER PRIMARY KEY)
CREATE TABLE tbl_1631 ( id INTEGER PRIMARY KEY, c1 text NOT NULL UNIQUE, c2 text NOT NULL)
CREATE TABLE c_4214 (id INTEGER NOT NULL PRIMARY KEY)
CREATE TABLE a_4214 ( id INTEGER NOT NULL PRIMARY KEY, c_id INTEGER NOT NULL, FOREIGN KEY(c_id) REFERENCES c_4214 (id) )
CREATE TABLE tag_8764 ( key VARCHAR(65535) NOT NULL, name VARCHAR(65535) NULL, value VARCHAR(65535) NOT NULL, PRIMARY KEY (key, name) )
CREATE TABLE t_11288 (i INT PRIMARY KEY, j MAP(VARCHAR, VARCHAR))
CREATE TABLE t_4807 (id INT PRIMARY KEY, u UNION (i INT))
CREATE TABLE t_14133 (i INT PRIMARY KEY, s VARCHAR)
CREATE TABLE tbl_6500 (i INTEGER, j INTEGER)
CREATE UNIQUE INDEX idx_name_8764 ON tag_8764(name)
CREATE UNIQUE INDEX idx_value_8764 ON tag_8764(value)
CREATE UNIQUE INDEX idx_6500 ON tbl_6500 (i)
# query
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
# file: test/sql/index/art/constraints/test_art_eager_with_wal.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY)
# query
SET checkpoint_threshold = '10.0 GB'
CREATE TABLE tbl (id INT PRIMARY KEY)
DELETE FROM tbl WHERE id = 1
# file: test/sql/index/art/constraints/test_art_large_abort.test
# setup
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT)
# query
CREATE TABLE a(id INTEGER PRIMARY KEY, c INT)
INSERT INTO a VALUES (1, 4)
INSERT INTO a SELECT i id, NULL c FROM range(-2, -250000, -1) tbl(i)
SELECT c FROM a WHERE id=1
INSERT INTO a SELECT i id, -i c FROM range(-2, -250000, -1) tbl(i)
# reject
INSERT INTO a VALUES (1, 5)
# file: test/sql/index/art/constraints/test_art_simple_update.test
# setup
CREATE TABLE tbl (i BIGINT PRIMARY KEY, l1 BIGINT[])
# query
CREATE TABLE tbl (i BIGINT PRIMARY KEY, l1 BIGINT[])
INSERT INTO tbl VALUES(1, [1, 2, 3]), (2, [42])
SELECT i, l1, rowid FROM tbl ORDER BY ALL
UPDATE tbl SET l1 = [1, 2, 4] WHERE i = 1
INSERT OR REPLACE INTO tbl VALUES (2, [43])
INSERT OR REPLACE INTO tbl VALUES (2, [44])
# file: test/sql/index/art/constraints/test_art_tx_conflict_revert.test
# setup
CREATE TABLE tbl(i INT PRIMARY KEY)
CREATE TABLE tbl2(i INT)
# query
CREATE TABLE tbl(i INT PRIMARY KEY)
INSERT INTO tbl FROM range(100_000)
CREATE TABLE tbl2(i INT)
INSERT INTO tbl2 VALUES (42)
DELETE FROM tbl
DELETE FROM tbl2
ALTER TABLE tbl2 ADD COLUMN j INTEGER
FROM tbl WHERE i=50_000
DROP TABLE tbl
# reject
INSERT INTO tbl VALUES (50_000)
# file: test/sql/index/art/constraints/test_art_tx_delete_with_global_nested.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR[])
# query
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl VALUES (1, ['first payload'])
INSERT INTO tbl VALUES (1, ['con1 payload'])
INSERT INTO tbl VALUES (1, ['con2 payload'])
SELECT id, payload, rowid FROM tbl WHERE id = 1
# file: test/sql/index/art/constraints/test_art_tx_deletes_list.test
# setup
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
# query
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_list VALUES (1, ['first payload'])
INSERT INTO tbl_list VALUES (5, ['old payload'])
DELETE FROM tbl_list
INSERT INTO tbl_list VALUES (1, ['con1 payload'])
SELECT id, payload, rowid FROM tbl_list WHERE id = 1
SELECT id, payload, rowid FROM tbl_list ORDER BY ALL
# file: test/sql/index/art/constraints/test_art_tx_deletes_rollback.test
# setup
CREATE TABLE tbl_rollback (id INT PRIMARY KEY, payload VARCHAR[])
# query
CREATE TABLE tbl_rollback (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_rollback VALUES (1, ['first payload'])
DELETE FROM tbl_rollback
INSERT INTO tbl_rollback VALUES (1, ['con1 payload'])
SELECT id, payload, rowid FROM tbl_rollback ORDER BY ALL
# reject
SELECT 42
# file: test/sql/index/art/constraints/test_art_tx_deletes_varchar.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR)
# query
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR)
INSERT INTO tbl VALUES (1, 'first payload')
INSERT INTO tbl VALUES (5, 'old payload')
INSERT INTO tbl VALUES (1, 'con1 payload')
SELECT id, payload, rowid FROM tbl ORDER BY ALL
# file: test/sql/index/art/constraints/test_art_tx_over_eager.test
# setup
CREATE TABLE tbl(i INT PRIMARY KEY, v VARCHAR)
# query
CREATE TABLE tbl(i INT PRIMARY KEY, v VARCHAR)
INSERT INTO tbl VALUES (1, 'row 1'), (2, 'row 2'), (3, 'row 3')
DELETE FROM tbl WHERE i=2
SELECT * FROM tbl WHERE i=2
INSERT INTO tbl VALUES (2, 'new row')
# file: test/sql/index/art/constraints/test_art_tx_returning.test
# setup
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT INTO tbl_list SELECT range, [range || ' payload'] FROM range(5)
UPDATE tbl_list SET id = id + 5 RETURNING id, payload
INSERT INTO tbl_list SELECT range + 10, [(range + 10) || ' payload'] FROM range(3000)
# reject
UPDATE tbl_list SET id = id + 1 RETURNING id, payload
# file: test/sql/index/art/constraints/test_art_tx_same_row_id.test
# setup
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT INTO tbl_list SELECT range, [range || ' payload'] FROM range(10)
DELETE FROM tbl_list USING range(100) t(i) RETURNING id, payload
# file: test/sql/index/art/constraints/test_art_tx_update_with_global_nested.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR[])
# query
UPDATE tbl SET payload = ['con1 payload'] WHERE id = 1
UPDATE tbl SET payload = ['con2 payload'] WHERE id = 1
# file: test/sql/index/art/constraints/test_art_tx_updates_list.test
# setup
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT INTO tbl_list VALUES (1, ['first payload']), (2, ['second payload'])
UPDATE tbl_list SET payload = ['con1 payload'] WHERE id = 1
UPDATE tbl_list SET id = 3 WHERE id = 2
INSERT INTO tbl_list VALUES (2, ['new payload'])
SELECT id, payload, rowid FROM tbl_list WHERE id = 2
SELECT id, payload, rowid FROM tbl_list WHERE id = 3
# reject
UPDATE tbl_list SET payload = ['second payload'] WHERE id = 1
# file: test/sql/index/art/constraints/test_art_tx_updates_pk_col.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR)
# query
UPDATE tbl SET id = 3 WHERE id = 1
INSERT INTO tbl VALUES (1, 'new payload')
UPDATE tbl SET payload = 'second payload' WHERE id = 1
SELECT id, payload FROM tbl WHERE id = 1
SELECT id, payload FROM tbl WHERE id = 3
SELECT id, payload, rowid FROM tbl WHERE id = 3
# file: test/sql/index/art/constraints/test_art_tx_updates_rollback.test
# setup
CREATE TABLE tbl_rollback (id INT PRIMARY KEY, payload VARCHAR[])
# query
UPDATE tbl_rollback SET payload = ['con1 payload'] WHERE id = 1
# file: test/sql/index/art/constraints/test_art_tx_upsert_with_global_nested.test
# setup
CREATE TABLE tbl (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT OR REPLACE INTO tbl VALUES (1, ['con1 payload'])
INSERT OR REPLACE INTO tbl VALUES (1, ['con2 payload'])
# file: test/sql/index/art/constraints/test_art_tx_upserts_list.test
# setup
CREATE TABLE tbl_list (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT OR REPLACE INTO tbl_list VALUES (1, ['con1 payload'])
# reject
INSERT OR REPLACE INTO tbl_list VALUES (1, ['second payload'])
# file: test/sql/index/art/constraints/test_art_tx_upserts_local.test
# setup
CREATE TABLE tbl_local (id INT PRIMARY KEY, payload VARCHAR[])
# query
CREATE TABLE tbl_local (id INT PRIMARY KEY, payload VARCHAR[])
INSERT INTO tbl_local VALUES (1, ['first payload'])
INSERT OR REPLACE INTO tbl_local VALUES (1, ['con1 payload'])
INSERT OR REPLACE INTO tbl_local VALUES (1, ['local payload'])
SELECT id, payload, rowid FROM tbl_local WHERE id = 1
INSERT OR REPLACE INTO tbl_local VALUES (1, ['val2 payload']), (1, ['val2 payload'])
# file: test/sql/index/art/constraints/test_art_tx_upserts_rollback.test
# setup
CREATE TABLE tbl_rollback (id INT PRIMARY KEY, payload VARCHAR[])
# query
INSERT OR REPLACE INTO tbl_rollback VALUES (1, ['con1 payload'])
INSERT INTO tbl_rollback VALUES (2, ['second payload'])
# file: test/sql/index/art/constraints/test_art_upsert_duplicate.test
# setup
CREATE TABLE hero ( name VARCHAR NOT NULL, secret_name VARCHAR NOT NULL, age INTEGER, PRIMARY KEY (name))
CREATE INDEX ix_hero_age ON hero (age)
# query
CREATE TABLE hero ( name VARCHAR NOT NULL, secret_name VARCHAR NOT NULL, age INTEGER, PRIMARY KEY (name))
CREATE INDEX ix_hero_age ON hero (age)
INSERT INTO hero (name, secret_name, age) VALUES ('Captain North America', 'Esteban Rogelios', 93), ('Rusty-Man', 'Tommy Sharp', 48), ('Tarantula', 'Natalia Roman-on', 32), ('Spider-Boy', 'Pedro Parqueador', 17), ('Captain North America', 'Esteban Rogelios', 93) ON CONFLICT (name) DO UPDATE SET secret_name = EXCLUDED.secret_name, age = EXCLUDED.age
# file: test/sql/index/art/constraints/test_art_upsert_other_index.test
# setup
CREATE TABLE kvp ( "key" VARCHAR PRIMARY KEY, "value" VARCHAR, expiration BIGINT, "cache" BOOLEAN)
CREATE INDEX kve_idx ON kvp (expiration)
# query
CREATE TABLE kvp ( "key" VARCHAR PRIMARY KEY, "value" VARCHAR, expiration BIGINT, "cache" BOOLEAN)
CREATE INDEX kve_idx ON kvp (expiration)
INSERT OR REPLACE INTO kvp VALUES ('/key', 'value', 0, false)
SELECT key, value, expiration, cache FROM kvp
INSERT OR REPLACE INTO kvp VALUES ('/key', 'value', 10000000, false)
INSERT INTO kvp VALUES ('/key', 'value', 20000000, false) ON CONFLICT DO UPDATE SET value = excluded.value, expiration = excluded.expiration, cache = excluded.cache
# file: test/sql/index/art/nodes/test_art_leaf_coverage.test
# setup
CREATE TABLE duplicates (id UBIGINT)
CREATE TABLE leaf_merge_1 (id UINT32, id2 INT64)
CREATE TABLE leaf_merge_2 (id UINT32, id2 INT64)
CREATE TABLE tbl_dup_ser (id INTEGER)
CREATE INDEX idx_duplicates ON duplicates(id)
CREATE INDEX idx_merge_1 ON leaf_merge_1(id, id2)
CREATE INDEX idx_merge_2 ON leaf_merge_2(id, id2)
CREATE INDEX idx_dup_ser ON tbl_dup_ser(id)
# query
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
# file: test/sql/index/art/nodes/test_art_nested_leaf_coverage.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
CREATE INDEX i_index ON integers(i)
INSERT INTO integers VALUES (2)
DELETE FROM integers where rowid = 1
DELETE FROM integers where rowid = 2
DELETE FROM integers where rowid = 3
# file: test/sql/index/art/nodes/test_art_node_16.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
SELECT sum(i) FROM integers WHERE i <= 2
SELECT sum(i) FROM integers WHERE i > 4
DELETE FROM integers WHERE i = 0
# file: test/sql/index/art/nodes/test_art_node_256.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
SELECT sum(i) FROM integers WHERE i > 15
DELETE FROM integers WHERE i=16
INSERT INTO integers VALUES (16)
# file: test/sql/index/art/nodes/test_art_node_4.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
SELECT sum(i) FROM integers WHERE i > 1
# file: test/sql/index/art/nodes/test_art_node_48.test
# setup
CREATE TABLE integers(i integer)
CREATE TABLE n48_tbl(i varchar, k integer)
CREATE TABLE n48_free (id INTEGER)
CREATE INDEX i_index ON integers(i)
CREATE INDEX n48_tbl_idx ON n48_tbl(i, k)
CREATE INDEX idx_n48_free ON n48_free(id)
# query
CREATE TABLE n48_tbl(i varchar, k integer)
INSERT INTO n48_tbl SELECT 'a', range FROM range(10000)
INSERT INTO n48_tbl SELECT 'b', range FROM range(25)
INSERT INTO n48_tbl SELECT 'c', range FROM range(25)
CREATE INDEX n48_tbl_idx ON n48_tbl(i, k)
CREATE TABLE n48_free (id INTEGER)
INSERT INTO n48_free SELECT range % 100 FROM range(2048)
CREATE INDEX idx_n48_free ON n48_free(id)
# file: test/sql/index/art/nodes/test_art_prefix_transform_deprecated_create.test
# setup
CREATE TABLE db.t (id VARCHAR, ts TIMESTAMP, value INTEGER, PRIMARY KEY (id, ts))
# query
CREATE TABLE db.t (id VARCHAR, ts TIMESTAMP, value INTEGER, PRIMARY KEY (id, ts))
INSERT OR IGNORE INTO db.t SELECT range || 'hello this is a long prefix', current_timestamp, range FROM range(1_000_000)
CHECKPOINT db
# file: test/sql/index/art/nodes/test_art_prefixes_restart.test
# setup
CREATE TABLE tbl (id INTEGER)
CREATE TABLE tbl_varchar (id VARCHAR)
CREATE INDEX idx ON tbl(id)
CREATE INDEX idx_varchar ON tbl_varchar(id)
# query
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
# file: test/sql/index/art/nodes/test_art_sparse_merge.test
# setup
CREATE TABLE tbl1 (i INT)
CREATE INDEX idx ON tbl1(i)
# query
CREATE TABLE tbl1 (i INT)
INSERT INTO tbl1 SELECT range FROM range(50000)
DELETE FROM tbl1 WHERE i > 4
CREATE INDEX idx ON tbl1(i)
SELECT COUNT(i) FROM tbl1 WHERE i = 1
# file: test/sql/index/art/vacuum/test_art_vacuum_rollback.test
# query
INSERT INTO t7 VALUES (42)
# file: test/sql/index/art/multi_column/test_art_multi_column.test
# setup
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR)
CREATE INDEX i_index ON integers using art(j)
# query
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR)
CREATE INDEX i_index ON integers using art(j)
INSERT INTO integers VALUES (10, 1, 'hello'), (11, 2, 'world')
SELECT i FROM integers WHERE i=10
SELECT * FROM integers WHERE i=10
SELECT j FROM integers WHERE j=1
SELECT * FROM integers WHERE j=1
SELECT k FROM integers WHERE k='hello'
SELECT i, k FROM integers WHERE k='hello'
# file: test/sql/index/art/multi_column/test_art_multi_predicate.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE INDEX i_index ON integers using art(i)
# query
CREATE INDEX i_index ON integers using art(i)
INSERT INTO integers VALUES (1, 2), (1, 3)
SELECT * FROM integers WHERE i = 1 AND j = 2
# file: test/sql/index/art/scan/test_art_adaptive_scan.test
# setup
CREATE TABLE integers AS SELECT 42 AS i FROM range(2050)
CREATE INDEX i_index ON integers USING ART(i)
# query
CREATE TABLE integers AS SELECT 42 AS i FROM range(2050)
INSERT INTO integers SELECT 42 + 1 + range FROM range(5000)
CREATE INDEX i_index ON integers USING ART(i)
SET index_scan_percentage = 1.0
SET index_scan_max_count = 0
EXPLAIN ANALYZE SELECT COUNT(i) FROM integers WHERE i = 42
SELECT COUNT(i) FROM integers WHERE i = 42
# file: test/sql/index/art/scan/test_art_many_matches.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers using art(i)
# query
INSERT INTO integers SELECT i FROM RANGE(0, 1024, 1) t2(j), (VALUES (0), (1)) t1(i) ORDER BY j, i
SELECT COUNT(*) FROM integers WHERE i<1
SELECT COUNT(*) FROM integers WHERE i<=1
SELECT COUNT(*) FROM integers WHERE i=0
SELECT COUNT(*) FROM integers WHERE i=1
SELECT COUNT(*) FROM integers WHERE i>0
SELECT COUNT(*) FROM integers WHERE i>=0
INSERT INTO integers SELECT i FROM RANGE(0, 2048, 1) t2(j), (VALUES (0), (1)) t1(i) ORDER BY j, i
# file: test/sql/index/art/scan/test_art_negative_range_scan.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
INSERT INTO integers SELECT * FROM range(-500, 500, 1)
SELECT sum(i) FROM integers WHERE i >= -500 AND i <= -498
SELECT sum(i) FROM integers WHERE i >= -10 AND i <= 5
SELECT sum(i) FROM integers WHERE i >= 10 AND i <= 15
# file: test/sql/index/art/scan/test_art_null_bytes.test
# setup
CREATE TABLE varchars(v VARCHAR PRIMARY KEY)
CREATE TABLE blobs(b BLOB PRIMARY KEY)
# query
CREATE TABLE varchars(v VARCHAR PRIMARY KEY)
INSERT INTO varchars VALUES ('hello'), ('hello' || chr(0)), ('hello' || chr(0) || chr(0)), ('hello' || chr(0) || chr(0) || chr(0))
SELECT * FROM varchars WHERE v = 'hello'
SELECT * FROM varchars WHERE v = 'hello' || chr(0)
SELECT * FROM varchars WHERE v = 'hello' || chr(0) || chr(0)
SELECT * FROM varchars WHERE v = 'hello' || chr(0) || chr(0) || chr(0)
CREATE TABLE blobs(b BLOB PRIMARY KEY)
SELECT * FROM blobs WHERE b = ''
# reject
INSERT INTO varchars VALUES ('hello' || chr(0) || chr(0) || chr(0))
# file: test/sql/index/art/scan/test_art_prepared_scan.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers(i)
# query
INSERT INTO integers VALUES (1), (2), (4)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 2
SELECT i FROM integers WHERE i = 2
PREPARE v1 AS SELECT * FROM integers WHERE i = $1
EXPLAIN ANALYZE EXECUTE v1(2)
EXECUTE v1(2)
# file: test/sql/index/art/scan/test_art_range_scan.test
# setup
CREATE TABLE test (x USMALLINT PRIMARY KEY)
# query
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
# file: test/sql/index/art/scan/test_art_scan_coverage.test
# setup
CREATE TABLE tab0(pk INTEGER PRIMARY KEY, col0 INTEGER, col1 FLOAT, col2 TEXT, col3 INTEGER, col4 FLOAT, col5 TEXT)
CREATE TABLE tab1(pk INTEGER PRIMARY KEY, col0 INTEGER, col1 FLOAT, col2 TEXT, col3 INTEGER, col4 FLOAT, col5 TEXT)
CREATE TABLE t0_varchar(c0 VARCHAR)
CREATE TABLE t0_scan(c0 DATE)
CREATE INDEX idx_tab1_0 on tab1 (col0)
CREATE INDEX idx_tab1_1 on tab1 (col1)
CREATE INDEX idx_tab1_3 on tab1 (col3)
CREATE INDEX idx_tab1_4 on tab1 (col4)
CREATE INDEX t0i0_idx ON t0_varchar(c0 )
CREATE INDEX t0i0 ON t0_scan(c0 DESC)
# query
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
# file: test/sql/index/art/scan/test_art_scan_duplicate_filters.test
# setup
CREATE TABLE t_1 (fIdx VARCHAR, sIdx UUID,)
CREATE TABLE t_3 (fIdx VARCHAR, sIdx UUID)
CREATE TABLE t_4 (sIdx UUID)
CREATE TABLE t_5 (sIdx UUID)
CREATE UNIQUE INDEX _pk_idx_t_5 ON t_5 (sIdx)
# query
CREATE TABLE t_1 (fIdx VARCHAR, sIdx UUID,)
CREATE TABLE t_3 (fIdx VARCHAR, sIdx UUID)
CREATE TABLE t_4 (sIdx UUID)
CREATE TABLE t_5 (sIdx UUID)
CREATE UNIQUE INDEX _pk_idx_t_5 ON t_5 (sIdx)
INSERT INTO t_4 (sIdx) VALUES ('1381e0ce-6b3e-43f5-9536-5e7af3a512a5'::UUID), ('6880cdba-09f5-3c4f-8eb8-391aefdd8052'::UUID), ('a3e876dd-5e50-3af7-9649-689fd938daeb'::UUID), ('e0abc0d3-63be-41d8-99ca-b1269ed153a8'::UUID)
WITH cte_5 AS ( SELECT sIdx FROM t_4 ANTI JOIN t_3 USING (sIdx) ), cte_6 AS MATERIALIZED ( SELECT COALESCE(cte_5.sIdx, t_1.sIdx) AS sIdx, COALESCE(t_1.fIdx, cte_5.sIdx::VARCHAR) AS fIdx, FROM cte_5 FULL JOIN t_1 USING (sIdx) ), cte_7 AS ( SELECT t_5.sIdx, FROM t_5 WHERE sIdx IN (SELECT sIdx FROM cte_6) ) SELECT fIdx, FROM cte_6 JOIN cte_7 USING (sIdx) ORDER BY fIdx
# file: test/sql/index/art/scan/test_art_scan_normal_to_nested.test
# setup
CREATE TABLE integers (i BIGINT)
CREATE TABLE t0(c1 TIMESTAMP)
CREATE INDEX idx_integers ON integers (i)
CREATE INDEX i0 ON t0(c1)
# query
CREATE TABLE integers (i BIGINT)
CREATE INDEX idx_integers ON integers (i)
INSERT INTO integers (i) VALUES ('1'), ('-1'), ('1')
SELECT i FROM integers WHERE i <= 0
CREATE TABLE t0(c1 TIMESTAMP)
INSERT INTO t0(c1) VALUES ('2020-02-29 12:00:00'), ('1969-12-09 09:26:38'), ('2020-02-29 12:00:00')
CREATE INDEX i0 ON t0(c1)
SELECT c1 FROM t0 WHERE c1 <= '2007-07-07 07:07:07'
# file: test/sql/index/art/scan/test_art_scan_thresholds.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX idx ON integers(i)
# query
SET index_scan_max_count = 1
INSERT INTO integers SELECT 42 FROM range(1000)
INSERT INTO integers SELECT 43 FROM range(10000)
CREATE INDEX idx ON integers(i)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 42
SET index_scan_percentage = 0.000001
SET index_scan_max_count = 4000
INSERT INTO integers SELECT 4242 FROM range(4000)
EXPLAIN ANALYZE SELECT i FROM integers WHERE i = 4242
# file: test/sql/index/art/scan/test_hash_join_in_filter_index_scan.test
# setup
CREATE TABLE random_orders AS ( (SELECT o_orderkey FROM orders OFFSET 100 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 + 100000 LIMIT 3))
CREATE TABLE orders_shuffled AS FROM orders ORDER BY random()
# query
CALL dbgen(sf=0.01)
CREATE TABLE random_orders AS ( (SELECT o_orderkey FROM orders OFFSET 100 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 LIMIT 3) UNION (SELECT o_orderkey FROM orders OFFSET (SELECT COUNT(*) FROM orders) / 2 + 100000 LIMIT 3))
CREATE TABLE orders_shuffled AS FROM orders ORDER BY random()
EXPLAIN ANALYZE SELECT o_orderkey FROM orders_shuffled WHERE o_orderkey IN ( SELECT UNNEST(LIST(o_orderkey)) FROM random_orders ) ORDER BY ALL
ALTER TABLE orders_shuffled ADD PRIMARY KEY (o_orderkey)
# file: test/sql/index/art/scan/test_in_filter_index_scan.test
# setup
CREATE TABLE tbl AS SELECT range AS i FROM range(500000)
# query
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
# file: test/sql/index/art/scan/test_random_uuid.test
# setup
create or replace table t as select id: uuid(), v: i from generate_series(1, 700000) s(i)
create unique index uid on t(id)
# query
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
# file: test/sql/index/art/storage/test_art_auto_checkpoint.test
# setup
CREATE TABLE tbl (i INTEGER PRIMARY KEY)
CREATE INDEX idx ON tbl(i)
# query
PRAGMA wal_autocheckpoint='400KB'
CREATE TABLE tbl AS SELECT range AS i FROM range(40000)
SELECT used_blocks FROM pragma_database_size()
CREATE INDEX idx ON tbl(i)
SELECT used_blocks > 0 FROM pragma_database_size()
CREATE TABLE tbl (i INTEGER PRIMARY KEY)
INSERT INTO tbl SELECT range FROM range(40000)
# file: test/sql/index/art/storage/test_art_buffered_replays_chunk_edges.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE UNIQUE INDEX idx_tbl_i ON tbl(i)
# query
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
# file: test/sql/index/art/storage/test_art_buffered_replays_interleaved.test
# setup
CREATE TABLE tbl (i INTEGER)
CREATE UNIQUE INDEX idx_i ON tbl (i)
# query
CREATE UNIQUE INDEX idx_i ON tbl (i)
SELECT i FROM tbl WHERE i = 12501
SELECT i FROM tbl WHERE i = 1
# reject
INSERT INTO tbl VALUES (12501)
# file: test/sql/index/art/storage/test_art_buffered_replays_interval_merging.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE UNIQUE INDEX idx_tbl_i ON tbl(i)
# query
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
# file: test/sql/index/art/storage/test_art_buffered_replays_multi_col.test
# setup
CREATE TABLE tbl( col1 INTEGER, col2 INTEGER, idx_col INTEGER, gen_col INTEGER GENERATED ALWAYS AS (col1 + col2) VIRTUAL, col5 VARCHAR )
CREATE UNIQUE INDEX idx_tbl_idx_col ON tbl(idx_col)
# query
CREATE TABLE tbl( col1 INTEGER, col2 INTEGER, idx_col INTEGER, gen_col INTEGER GENERATED ALWAYS AS (col1 + col2) VIRTUAL, col5 VARCHAR )
CREATE UNIQUE INDEX idx_tbl_idx_col ON tbl(idx_col)
INSERT INTO tbl (col1, col2, idx_col, col5) SELECT r, r * 2, r, 'val' || r::VARCHAR FROM range(0, 1001) t(r)
DELETE FROM tbl WHERE idx_col % 2 = 0
EXPLAIN ANALYZE SELECT idx_col FROM tbl WHERE idx_col = 0
SELECT idx_col FROM tbl WHERE idx_col = 0
EXPLAIN ANALYZE SELECT idx_col FROM tbl WHERE idx_col = 10
SELECT idx_col FROM tbl WHERE idx_col = 10
# file: test/sql/index/art/storage/test_art_checkpoint.test
# setup
CREATE TABLE integers (i INTEGER)
CREATE INDEX idx ON integers(i)
# query
CREATE TABLE integers (i INTEGER PRIMARY KEY)
CREATE TABLE integers (i INTEGER)
INSERT INTO integers (SELECT range FROM range(512) UNION ALL SELECT 55)
SELECT total_blocks < 5 FROM pragma_database_size()
# reject
INSERT INTO integers VALUES (1)
# file: test/sql/index/art/storage/test_art_duckdb_versions.test
# setup
CREATE INDEX ART_index ON idx_tbl(i)
# query
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
# reject
CREATE INDEX idx_1 ON idx_tbl(i)
# file: test/sql/index/art/storage/test_art_import.test
# setup
CREATE TABLE tracking("nflId" VARCHAR , "frameId" INTEGER, "gameId" INTEGER, "playId" INTEGER)
CREATE INDEX nflid_idx ON tracking (nflid)
CREATE UNIQUE INDEX tracking_key_idx ON tracking (gameId, playId, frameId, nflId)
# query
CREATE TABLE tracking("nflId" VARCHAR , "frameId" INTEGER, "gameId" INTEGER, "playId" INTEGER)
INSERT INTO tracking values ('a', 0,0,0)
CREATE INDEX nflid_idx ON tracking (nflid)
CREATE UNIQUE INDEX tracking_key_idx ON tracking (gameId, playId, frameId, nflId)
# file: test/sql/index/art/storage/test_art_import_export.test
# setup
CREATE TABLE raw( "year" SMALLINT, "month" TINYINT, "day" TINYINT, "customer_ID" BIGINT )
CREATE UNIQUE INDEX customer_year_month_idx ON raw (customer_ID, year, month)
# query
CREATE TABLE raw( "year" SMALLINT, "month" TINYINT, "day" TINYINT, "customer_ID" BIGINT )
INSERT INTO raw VALUES (1, 1, 1, 1)
CREATE UNIQUE INDEX customer_year_month_idx ON raw (customer_ID, year, month)
# file: test/sql/index/art/storage/test_art_mem_limit.test
# setup
CREATE TABLE tbl AS SELECT range AS id FROM range(200000)
CREATE INDEX idx ON tbl(id)
# query
SET threads=1
SET memory_limit = '10MB'
CREATE TABLE tbl AS SELECT range AS id FROM range(200000)
FROM duckdb_memory()
# file: test/sql/index/art/storage/test_art_names.test
# setup
CREATE TABLE tbl (i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
CREATE TABLE fk_tbl (i INTEGER, j INTEGER, FOREIGN KEY (i) REFERENCES tbl(i), FOREIGN KEY (j) REFERENCES tbl(j))
# query
CREATE TABLE tbl (i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
INSERT INTO tbl SELECT range, range FROM range (3000)
CREATE TABLE fk_tbl (i INTEGER, j INTEGER, FOREIGN KEY (i) REFERENCES tbl(i), FOREIGN KEY (j) REFERENCES tbl(j))
INSERT INTO fk_tbl SELECT range, range FROM range (3000)
# reject
CREATE INDEX PRIMARY_tbl_0 ON tbl(i)
CREATE INDEX UNIQUE_tbl_1 ON tbl(j)
INSERT INTO tbl VALUES (4000, 20)
INSERT INTO tbl VALUES (20, 4000)
INSERT INTO fk_tbl VALUES (4000, 20)
INSERT INTO fk_tbl VALUES (20, 4000)
CREATE INDEX FOREIGN_fk_tbl_0 ON fk_tbl(i)
CREATE INDEX FOREIGN_fk_tbl_1 ON fk_tbl(j)
# file: test/sql/index/art/storage/test_art_readonly.test
# setup
CREATE TABLE tbl (i INTEGER)
CREATE INDEX idx_drop ON tbl(i)
# query
CREATE INDEX idx_drop ON tbl(i)
SELECT index_name FROM duckdb_indexes()
# reject
CREATE INDEX idx ON tbl (i)
DROP INDEX idx_drop
# file: test/sql/index/art/storage/test_art_storage.test
# setup
CREATE TABLE integers(i integer,j integer)
CREATE TABLE tbl_deser_scan(id INTEGER)
CREATE TABLE max_row_id AS SELECT max(rowid) AS id FROM tbl_deser_scan WHERE id = 424242
CREATE TABLE tbl_m (x integer, y varchar)
CREATE MACRO m(x, y := 7) AS x + y
CREATE INDEX i_index ON integers(i)
CREATE INDEX idx_deser_scan ON tbl_deser_scan(id)
CREATE UNIQUE INDEX idx_m on tbl_m (m(tbl_m.x))
# query
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
# reject
insert into tbl_m VALUES (10, 'world')
# file: test/sql/index/art/storage/test_art_storage_long_prefixes.test
# setup
CREATE TABLE history(id TEXT, type TEXT, PRIMARY KEY(id, type))
# query
SET wal_autocheckpoint = '10GB'
CREATE TABLE history(id TEXT, type TEXT, PRIMARY KEY(id, type))
INSERT INTO history(id, type) VALUES ('5_create_aaaaaaaaaaa_mapping', 'sql')
INSERT INTO history(id, type) VALUES ('m0001_initialize', 'sql')
INSERT INTO history(id, type) VALUES ('m0005_create_aaaaaaaaaaa_mapping_table', 'sql')
# file: test/sql/index/art/storage/test_art_storage_multi_checkpoint.test
# setup
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
CREATE TABLE pk_integers2(i INTEGER PRIMARY KEY)
# query
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
INSERT INTO pk_integers VALUES (1)
CREATE TABLE pk_integers2(i INTEGER PRIMARY KEY)
INSERT INTO pk_integers2 VALUES (1)
SELECT i FROM pk_integers WHERE i = 1
# file: test/sql/index/art/storage/test_art_wal_checkpoint_minimal.test
# setup
CREATE TABLE minimal_tbl(i INTEGER)
CREATE UNIQUE INDEX idx_minimal ON minimal_tbl(i)
# query
CREATE TABLE minimal_tbl(i INTEGER)
CREATE UNIQUE INDEX idx_minimal ON minimal_tbl(i)
INSERT INTO minimal_tbl VALUES (42)
INSERT INTO minimal_tbl VALUES (43)
INSERT INTO minimal_tbl VALUES (44)
DELETE FROM minimal_tbl where i = 42
# file: test/sql/index/art/storage/test_art_wal_replay_drop_table.test
# setup
CREATE TABLE test (a INTEGER)
CREATE TABLE alter_test (a INTEGER)
CREATE INDEX other_idx ON test(a)
CREATE UNIQUE INDEX i_index ON test(a)
# query
INSERT INTO test SELECT range + 42 FROM range(100)
CREATE TABLE alter_test (a INTEGER)
INSERT INTO alter_test SELECT range + 42 FROM range(100)
CREATE INDEX other_idx ON test(a)
INSERT INTO test VALUES (0), (1)
INSERT INTO alter_test VALUES (0), (1)
CREATE UNIQUE INDEX i_index ON test(a)
ALTER TABLE alter_test ADD PRIMARY KEY(a)
DROP TABLE alter_test
# file: test/sql/index/art/storage/test_art_wal_replay_in_tx.test
# setup
CREATE TABLE test (a INTEGER)
CREATE TABLE alter_test (a INTEGER)
CREATE TABLE drop_test (a INTEGER)
CREATE INDEX other_idx ON test(a)
CREATE UNIQUE INDEX i_index ON test(a)
CREATE INDEX drop_idx ON drop_test(a)
CREATE INDEX drop_idx ON test(a)
# query
CREATE TABLE drop_test (a INTEGER)
INSERT INTO drop_test SELECT range + 42 FROM range(100)
INSERT INTO drop_test VALUES (0), (1)
CREATE INDEX drop_idx ON drop_test(a)
DROP INDEX drop_idx
DELETE FROM test WHERE a = 1
DELETE FROM alter_test WHERE a = 1
INSERT INTO alter_test VALUES (1)
CREATE INDEX drop_idx ON test(a)
# reject
INSERT INTO test VALUES (0)
INSERT INTO alter_test VALUES (0)
# file: test/sql/index/art/storage/test_art_wal_replay_with_buffer.test
# setup
CREATE TABLE tbl (u_2 UNION("string" VARCHAR, "bool" BOOLEAN))
CREATE UNIQUE INDEX idx_u_2_1 ON tbl ((u_2.string))
# query
CREATE TABLE tbl (u_2 UNION("string" VARCHAR, "bool" BOOLEAN))
INSERT INTO tbl VALUES ('helloo')
INSERT INTO tbl VALUES ('hellooo')
# file: test/sql/index/art/storage/test_art_wal_replay_with_buffer_concurrent.test
# setup
CREATE TABLE tbl (u_2 UNION("string" VARCHAR, "bool" BOOLEAN))
CREATE UNIQUE INDEX idx_1 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_2 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_3 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_4 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_5 ON tbl ((u_2.string))
# query
CREATE UNIQUE INDEX idx_1 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_2 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_3 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_4 ON tbl ((u_2.string))
CREATE UNIQUE INDEX idx_5 ON tbl ((u_2.string))
# file: test/sql/index/art/insert_update_delete/test_art_sel_vector.test
# setup
CREATE TABLE source(i INTEGER)
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers using art(i)
# query
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
# file: test/sql/index/art/insert_update_delete/test_art_simple_update.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers using art(i)
# query
UPDATE integers SET i=10 WHERE i=1
SELECT * FROM integers WHERE i < 5
SELECT * FROM integers WHERE i > 0
# file: test/sql/index/art/insert_update_delete/test_art_update.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE INDEX i_index ON integers using art(j)
# query
INSERT INTO integers VALUES (1, 2), (2, 2)
UPDATE integers SET j=10 WHERE i=1
UPDATE integers SET j=10 WHERE rowid=0
DELETE FROM integers WHERE rowid=1
SELECT * FROM integers WHERE j>5
# file: test/sql/index/art/insert_update_delete/test_art_update_other_column.test
# setup
CREATE TABLE integers(i BIGINT, j INTEGER, k VARCHAR)
CREATE INDEX i_index ON integers using art(j)
# query
UPDATE integers SET i=100, k='update' WHERE j=1
UPDATE integers SET i=20, k='t1' WHERE j=1
UPDATE integers SET i=21, k='t2' WHERE j=2
SELECT * FROM integers WHERE j=2
SELECT * FROM integers ORDER BY j
# file: test/sql/index/art/insert_update_delete/test_art_update_with_dict_fsst.test
# setup
CREATE OR REPLACE TABLE bar (col1 VARCHAR, col2 VARCHAR UNIQUE)
# query
CREATE OR REPLACE TABLE bar (col1 VARCHAR, col2 VARCHAR UNIQUE)
INSERT INTO bar (col1, col2) VALUES (NULL, 'one')
UPDATE bar AS original SET col1 = 'a'
SELECT col1 FROM bar WHERE col2 = 'one'
# file: test/sql/index/art/issues/test_art_fuzzer.test
# setup
CREATE TABLE t1 (c1 DECIMAL(4, 3))
CREATE TABLE t2 (c1 VARCHAR)
CREATE TABLE t3(c1 INT)
CREATE TABLE t4 (c1 BOOLEAN)
CREATE TABLE t_leak (c1 INT)
CREATE TABLE t21 (c1 INT)
CREATE INDEX i1 ON t1 (TRY_CAST(c1 AS USMALLINT))
CREATE INDEX i2 ON t2 (c1)
CREATE INDEX i22 ON t2 (c1)
CREATE INDEX i3 ON t3 (c1, (TRY_CAST(c1 AS USMALLINT)))
CREATE INDEX i4 ON t4 (c1)
# query
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
# reject
CREATE UNIQUE INDEX i_leak ON t_leak (c1)
# file: test/sql/index/art/issues/test_art_fuzzer_persisted.test
# setup
CREATE TABLE t1 AS (SELECT 1 c1, 'a' c2)
CREATE INDEX i1 ON t1 (c1)
# query
CREATE TABLE t1 AS (SELECT 1 c1, 'a' c2)
CREATE INDEX i1 ON t1 (c1)
PRAGMA MEMORY_LIMIT='4MB'
INSERT INTO t1(c2) (SELECT DISTINCT 'b')
# file: test/sql/index/art/issues/test_art_internal_issue_4742.test
# setup
create or replace table test as select 9223372036854776 + range * 9223372036854776 i from range(100)
create or replace table sample as select i from test using sample reservoir(10) repeatable (42)
create index my_index on test(i)
# query
create or replace table test as select 9223372036854776 + range * 9223372036854776 i from range(100)
create index my_index on test(i)
explain analyze select i from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i)
select count(*) from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i)
select i from test SEMI JOIN (select i from test using sample reservoir(10) repeatable (42)) USING (i) order by all
create or replace table sample as select i from test using sample reservoir(10) repeatable (42)
explain analyze select i from test SEMI JOIN sample USING (i)
select count(*) from test SEMI JOIN sample USING (i)
select i from test SEMI JOIN sample USING (i) order by all
# file: test/sql/index/art/issues/test_art_issue_21394.test
# setup
CREATE TABLE t0(c0 INT)
CREATE INDEX i1 ON t0(c0)
# query
CREATE TABLE t0(c0 INT)
INSERT INTO t0(c0) VALUES (2)
UPDATE t0 SET c0=0
CREATE INDEX i1 ON t0(c0)
DELETE FROM t0
# file: test/sql/index/art/issues/test_art_issue_4976.test
# setup
CREATE TABLE t0(c0 DOUBLE, c1 TIMESTAMP DEFAULT(TIMESTAMP '1970-01-04 12:58:32'))
CREATE INDEX i2 ON t0(c1, c0)
# query
CREATE TABLE t0(c0 DOUBLE, c1 TIMESTAMP DEFAULT(TIMESTAMP '1970-01-04 12:58:32'))
INSERT INTO t0(c1, c0) VALUES (TIMESTAMP '1969-12-28 23:02:08', 1)
INSERT INTO t0(c0) VALUES (DEFAULT)
CREATE INDEX i2 ON t0(c1, c0)
# file: test/sql/index/art/issues/test_art_issue_6603.test
# setup
CREATE SEQUENCE seq
CREATE TABLE path ( it INTEGER, x0 TEXT NOT NULL, x1 TEXT NOT NULL )
CREATE TABLE edge ( id INTEGER DEFAULT nextval('seq'), it INTEGER DEFAULT 0, x0 TEXT, x1 TEXT )
CREATE INDEX edge1_idx ON edge (x1)
# query
CREATE TABLE path ( it INTEGER, x0 TEXT NOT NULL, x1 TEXT NOT NULL )
CREATE TABLE edge ( id INTEGER DEFAULT nextval('seq'), it INTEGER DEFAULT 0, x0 TEXT, x1 TEXT )
CREATE INDEX edge1_idx ON edge (x1)
INSERT INTO edge (x0, x1) VALUES ('n2880','n3966')
INSERT INTO path SELECT 1, y0, y1 FROM (SELECT DISTINCT edge0.x0 AS y0, edge0.x1 AS y1 FROM edge AS edge0 WHERE edge0.it = 0 AND true AND NOT EXISTS (SELECT * from path AS pre WHERE pre.x0 = edge0.x0 AND pre.x1 = edge0.x1))
SELECT 1, y0, y1 FROM (SELECT DISTINCT edge0.x0 AS y0, path1.x1 AS y1 FROM edge AS edge0,path AS path1 WHERE edge0.it = 0 AND edge0.x1 = path1.x0 AND NOT EXISTS (SELECT * from path AS pre WHERE pre.x0 = edge0.x0 AND pre.x1 = path1.x1))
# file: test/sql/index/art/issues/test_art_issue_6799.test
# setup
CREATE TABLE key_value_pairs (key VARCHAR PRIMARY KEY, value VARCHAR)
CREATE TABLE keys_to_lookup (key VARCHAR PRIMARY KEY)
# query
CREATE TABLE key_value_pairs (key VARCHAR PRIMARY KEY, value VARCHAR)
INSERT INTO key_value_pairs SELECT concat('key_', i::VARCHAR), concat('value_', i::VARCHAR) FROM range(10000) t(i) WHERE random() < 0.5
CREATE TABLE keys_to_lookup (key VARCHAR PRIMARY KEY)
INSERT INTO keys_to_lookup SELECT concat('key_', i::VARCHAR) FROM range(100) t(i)
SELECT COUNT(*) FROM ( SELECT key, value FROM keys_to_lookup JOIN key_value_pairs USING(key) )
# file: test/sql/index/art/issues/test_art_issue_7349.test
# setup
CREATE TABLE td(tz VARCHAR(30) NOT NULL)
CREATE TABLE tab0(c2 DATE NOT NULL)
CREATE TABLE tab1(c2 DATE NOT NULL)
CREATE UNIQUE INDEX sqlsim0 ON td(tz)
# query
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
# file: test/sql/index/art/issues/test_art_issue_7530.test
# setup
CREATE TABLE t14(c0 BIGINT)
CREATE INDEX i1 ON t14(c0 )
# query
CREATE TABLE t14(c0 BIGINT)
INSERT INTO t14(c0) VALUES ((1)), ((1)), ((1))
CREATE INDEX i1 ON t14(c0 )
DELETE FROM t14 WHERE t14.rowid
# file: test/sql/index/art/issues/test_art_view_col_binding.test
# setup
create or replace table test as ( select cast(unnest(range(1000)) as varchar) as x, cast(unnest(range(2000,3000)) as varchar) as y, cast(unnest(range(3000,4000)) as varchar) as z )
create view test_view as (select z, y, x from test)
create index test_x on test(x)
create index test_upper_x on test(upper(x))
# query
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
# file: test/sql/index/art/create_drop/test_art_big_compound_key.test
# setup
CREATE TABLE v0 (v2 VARCHAR, v1 INT)
CREATE UNIQUE INDEX v3 ON v0 (v1, v1, v1, v1, v1, v2, v1, v2, v1, v2, v2, v1, v2, v2, v2, v2, v2, v2, v1, v1, v2, v2, v1, v1, v2, v1)
# query
CREATE TABLE v0 (v2 VARCHAR, v1 INT)
INSERT INTO v0 (v2 ,v1 ) VALUES ('358677 4 2 1', 7), ('a%', 1)
CREATE UNIQUE INDEX v3 ON v0 (v1, v1, v1, v1, v1, v2, v1, v2, v1, v2, v2, v1, v2, v2, v2, v2, v2, v2, v1, v1, v2, v2, v1, v1, v2, v1)
# file: test/sql/index/art/create_drop/test_art_create_if_exists.test
# setup
CREATE TABLE tbl AS SELECT range AS i FROM range(100)
CREATE INDEX IF NOT EXISTS my_idx ON tbl(i)
# query
PRAGMA immediate_transaction_mode = True
CREATE TABLE tbl AS SELECT range AS i FROM range(100)
CREATE INDEX IF NOT EXISTS my_idx ON tbl(i)
SELECT COUNT(*) FROM duckdb_indexes
DROP INDEX my_idx
# reject
CREATE INDEX my_idx ON tbl(i)
# file: test/sql/index/art/create_drop/test_art_create_index_delete.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers(i)
# query
INSERT INTO integers SELECT * FROM range(10)
DELETE FROM integers WHERE i=2 OR i=7
SELECT * FROM integers WHERE i=1
SELECT * FROM integers WHERE i=2
# file: test/sql/index/art/create_drop/test_art_create_index_duplicate_deletes.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers(i)
# query
DELETE FROM integers
# file: test/sql/index/art/create_drop/test_art_create_many_duplicates.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers(i)
# query
INSERT INTO integers SELECT * FROM repeat(1, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(2, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(3, 1500) t1(i)
INSERT INTO integers SELECT * FROM repeat(4, 1500) t1(i)
SELECT count(i) FROM integers WHERE i > 1 AND i < 3
SELECT count(i) FROM integers WHERE i >= 1 AND i < 3
SELECT count(i) FROM integers WHERE i > 1
SELECT count(i) FROM integers WHERE i < 4
SELECT count(i) FROM integers WHERE i < 5
# file: test/sql/index/art/create_drop/test_art_create_many_duplicates_deletes.test
# setup
CREATE TABLE integers(i integer)
CREATE INDEX i_index ON integers(i)
# query
INSERT INTO integers SELECT * FROM repeat(5, 1500) t1(i)
DELETE FROM integers WHERE i = 5
# file: test/sql/index/art/create_drop/test_art_create_unique.test
# setup
CREATE TABLE t0(c0 INTEGER)
CREATE TABLE merge_violation (id INT)
CREATE UNIQUE INDEX i0 ON t0(c0)
# query
CREATE TABLE t0(c0 INTEGER)
CREATE UNIQUE INDEX i0 ON t0(c0)
INSERT INTO t0(c0) VALUES (1)
SELECT * FROM t0 WHERE t0.c0 = 1
CREATE TABLE merge_violation (id INT)
INSERT INTO merge_violation SELECT range FROM range(2048)
INSERT INTO merge_violation SELECT range + 10000 FROM range(2048)
INSERT INTO merge_violation VALUES (2047)
# reject
CREATE UNIQUE INDEX idx ON merge_violation(id)
# file: test/sql/index/art/create_drop/test_art_drop_index.test
# setup
CREATE TABLE A (A1 INTEGER,A2 VARCHAR, A3 INTEGER)
CREATE TABLE B (B1 INTEGER,B2 INTEGER, B3 INTEGER)
CREATE TABLE C (C1 VARCHAR, C2 INTEGER, C3 INTEGER)
CREATE INDEX A_index ON A (A1)
CREATE INDEX B_index ON B (B1)
CREATE INDEX C_index ON C (C2)
# query
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
# file: test/sql/index/art/create_drop/test_art_invalid_create_index.test
# setup
CREATE TABLE integers(i integer, j integer, k BOOLEAN)
create table lists(id int, l int[])
# query
CREATE TABLE integers(i integer, j integer, k BOOLEAN)
create table lists(id int, l int[])
# reject
CREATE INDEX ON integers(i)
CREATE INDEX i_index ON integers(i COLLATE "NOCASE")
CREATE INDEX i_index ON integers(i COLLATE "de_DE")
CREATE INDEX i_index ON integers using blabla(i)
CREATE INDEX i_index ON integers(f)
create index i_index on lists(l)
create index i_index on lists(id, l)
create index i_index on integers(('hello'))
# file: test/sql/index/art/create_drop/test_art_many_versions.test
# setup
CREATE TABLE integers(i INTEGER)
# query
INSERT INTO integers SELECT * FROM range(1, 20001, 1)
UPDATE integers SET i=i+1
SELECT SUM(i) FROM integers
SELECT SUM(i) FROM integers WHERE i > 0
# file: test/sql/index/art/create_drop/test_art_single_value.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE INDEX i_index ON integers using art(i)
# query
SELECT * FROM integers WHERE i < 3
SELECT * FROM integers WHERE i <= 1
SELECT * FROM integers WHERE i >= 1
SELECT * FROM integers WHERE i = 1
SELECT * FROM integers WHERE i < 1
SELECT * FROM integers WHERE i <= 0
SELECT * FROM integers WHERE i > 1
SELECT * FROM integers WHERE i >= 2
SELECT * FROM integers WHERE i = 2
# file: test/sql/constraints/test_constraint_with_updates.test
# setup
CREATE TABLE integers(i INTEGER NOT NULL, j INTEGER NOT NULL)
# query
CREATE TABLE integers(i INTEGER, j INTEGER CHECK(i + j < 5), k INTEGER)
INSERT INTO integers VALUES (1, 2, 4)
UPDATE integers SET k=7
UPDATE integers SET i=i, j=3
UPDATE integers SET j=2
CREATE TABLE integers(i INTEGER NOT NULL, j INTEGER NOT NULL)
UPDATE integers SET j=3
# reject
UPDATE integers SET i=i, i=10
UPDATE integers SET i=i, j=10
UPDATE integers SET j=10
UPDATE integers SET i=NULL
UPDATE integers SET j=NULL
# file: test/sql/constraints/test_not_null.test
# setup
CREATE TABLE integers(i INTEGER NOT NULL)
CREATE TABLE integers_with_null(i INTEGER)
# query
CREATE TABLE integers(i INTEGER NOT NULL)
INSERT INTO integers VALUES (3)
UPDATE integers SET i=4
CREATE TABLE integers_with_null(i INTEGER)
INSERT INTO integers_with_null VALUES (3), (4), (5), (NULL)
INSERT INTO integers (i) SELECT * FROM integers_with_null WHERE i IS NOT NULL
SELECT * FROM integers ORDER BY i
UPDATE integers SET i=4 WHERE i>4
# reject
INSERT INTO integers VALUES (NULL)
INSERT INTO integers (i) SELECT * FROM integers_with_null
# file: test/sql/constraints/unique/test_unique.test
# setup
CREATE TABLE integers(i INTEGER UNIQUE, j INTEGER)
# query
CREATE TABLE integers(i INTEGER UNIQUE, j INTEGER)
INSERT INTO integers VALUES (3, 4), (2, 5)
INSERT INTO integers VALUES (NULL, 6), (NULL, 7)
SELECT * FROM integers ORDER BY i, j
UPDATE integers SET i=77 WHERE i IS NULL AND j=6
# reject
INSERT INTO integers VALUES (6, 6), (3, 4)
UPDATE integers SET i=77 WHERE i IS NULL
INSERT INTO integers VALUES (NULL, 6), (3, 7)
# file: test/sql/constraints/unique/test_unique_error.test
# setup
CREATE TEMPORARY TABLE integers(i INTEGER, j VARCHAR)
# query
CREATE TEMPORARY TABLE integers(i INTEGER, j VARCHAR)
INSERT INTO integers VALUES (3, '4'), (2, '4')
# reject
CREATE UNIQUE INDEX uidx ON integers (j)
# file: test/sql/constraints/unique/test_unique_multi_column.test
# setup
CREATE TEMPORARY TABLE integers(i INTEGER, j INTEGER)
CREATE UNIQUE INDEX uidx ON integers (i,j)
# query
CREATE TEMPORARY TABLE integers(i INTEGER, j INTEGER)
CREATE UNIQUE INDEX uidx ON integers (i,j)
INSERT INTO integers VALUES (NULL, 6), (NULL, 6), (NULL, 7)
UPDATE integers SET i=77 WHERE i IS NULL AND j=7
# reject
INSERT INTO integers VALUES (3, 4)
# file: test/sql/constraints/unique/test_unique_multi_constraint.test
# setup
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
# query
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER UNIQUE)
INSERT INTO integers VALUES (1, 1), (2, 2)
INSERT INTO integers VALUES (3, 3), (4, 4)
INSERT INTO integers VALUES (5, 5), (6, 6)
INSERT INTO integers VALUES (100, 100)
# reject
INSERT INTO integers VALUES (3, 3), (4, 1)
UPDATE integers SET i=4, j=100 WHERE i=1
UPDATE integers SET i=100, j=4 WHERE j=1
# file: test/sql/constraints/unique/test_unique_string.test
# setup
CREATE TEMPORARY TABLE integers(i INTEGER, j VARCHAR)
CREATE UNIQUE INDEX "uidx" ON "integers" ("j")
# query
CREATE UNIQUE INDEX "uidx" ON "integers" ("j")
INSERT INTO integers VALUES (3, '4'), (2, '5')
INSERT INTO integers VALUES (6,NULL), (7,NULL)
UPDATE integers SET j='7777777777777777777777777777' WHERE j IS NULL AND i=6
# reject
INSERT INTO integers VALUES (6, '6'), (3, '4')
UPDATE integers SET j='77' WHERE j IS NULL
INSERT INTO integers VALUES (3, '4')
# file: test/sql/constraints/unique/test_unique_temp.test
# setup
CREATE TEMPORARY TABLE integers(i INTEGER, j INTEGER)
CREATE UNIQUE INDEX uidx ON integers (i)
# query
CREATE UNIQUE INDEX uidx ON integers (i)
# file: test/sql/constraints/primarykey/test_pk_bool.test
# setup
CREATE TABLE integers(i INTEGER, j BOOLEAN, PRIMARY KEY(i, j))
# query
CREATE TABLE integers(i INTEGER, j BOOLEAN, PRIMARY KEY(i, j))
INSERT INTO integers VALUES (1, false), (1, true), (2, false)
INSERT INTO integers VALUES (2, true)
SELECT * FROM integers ORDER BY 1, 2
# reject
INSERT INTO integers VALUES (1, false)
# file: test/sql/constraints/primarykey/test_pk_col_subset.test
# setup
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b))
# query
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b))
INSERT INTO numbers VALUES (1,1,1,1,1),(1,2,1,1,1),(2,1,2,1,1),(2,2,2,2,2)
INSERT INTO numbers VALUES (1,5,1,1,4)
UPDATE numbers SET c=1 WHERE c=2
UPDATE numbers SET b=3 WHERE b=2
# reject
INSERT INTO numbers VALUES (1,1,1,1,1), (1,1,1,1,1)
INSERT INTO numbers VALUES (1,1,1,1,1),(1,5,1,1,4)
UPDATE numbers SET b=1 WHERE b=2
# file: test/sql/constraints/primarykey/test_pk_concurrency_conflicts.test
# setup
CREATE TABLE integers(i INTEGER PRIMARY KEY)
# query
CREATE TABLE integers(i INTEGER PRIMARY KEY)
INSERT INTO integers VALUES (1), (2), (3)
UPDATE integers SET i=4 WHERE i=2
UPDATE integers SET i=5 WHERE i=3
# reject
UPDATE integers SET i=5 WHERE i=2
DELETE FROM integers WHERE i=2
# file: test/sql/constraints/primarykey/test_pk_many_columns.test
# setup
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b,c,d,e))
# query
CREATE TABLE numbers(a integer, b integer, c integer, d integer, e integer, PRIMARY KEY(a,b,c,d,e))
INSERT INTO numbers VALUES (1,1,1,1,1),(1,2,1,1,1),(1,1,2,1,1),(2,2,2,2,2)
INSERT INTO numbers VALUES (1,1,1,1,4)
# reject
INSERT INTO numbers VALUES (1,1,1,1,1),(1,1,1,1,4)
# file: test/sql/constraints/primarykey/test_pk_multi_column.test
# setup
CREATE TABLE integers(i INTEGER, j VARCHAR, PRIMARY KEY(i, j))
# query
CREATE TABLE integers(i INTEGER, j VARCHAR, PRIMARY KEY(i, j))
INSERT INTO integers VALUES (3, 'hello'), (3, 'world')
INSERT INTO integers VALUES (6, 'bla')
# reject
INSERT INTO integers VALUES (6, 'bla'), (3, 'hello')
# file: test/sql/constraints/primarykey/test_pk_multi_string.test
# setup
CREATE TABLE tst(a varchar, b varchar,PRIMARY KEY(a,b))
# query
CREATE TABLE tst(a varchar, b varchar,PRIMARY KEY(a,b))
INSERT INTO tst VALUES ('hell', 'hello'), ('hello','hell'), ('hel','hell'), ('hell','hel')
INSERT INTO tst VALUES ('hel', 'hello')
UPDATE tst SET b='hell' WHERE b='hel'
# reject
INSERT INTO tst VALUES ('hell', 'hello'), ('hell','hello')
INSERT INTO tst VALUES ('hell', 'hello'),('hel', 'hello')
UPDATE tst SET b='hello' WHERE b='hel'
# file: test/sql/constraints/primarykey/test_pk_string.test
# setup
CREATE TABLE numbers(i varchar PRIMARY KEY, j INTEGER)
# query
CREATE TABLE numbers(i varchar PRIMARY KEY, j INTEGER)
INSERT INTO numbers VALUES ('1', 4), ('2', 5)
INSERT INTO numbers VALUES ('6', 6)
# reject
INSERT INTO numbers VALUES ('1', 4), ('1', 5)
INSERT INTO numbers VALUES ('6', 6), ('1', 4)
# file: test/sql/constraints/primarykey/test_pk_update_delete.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER)
# query
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER)
INSERT INTO test VALUES (11, 1), (12, 2), (13, 3)
UPDATE test SET b=2 WHERE b=3
DELETE FROM test WHERE a=11
INSERT INTO test VALUES (11, 1)
UPDATE test SET a=4 WHERE b=1
# reject
UPDATE test SET a=a+1 WHERE b=1
UPDATE test SET a=NULL WHERE b=1
# file: test/sql/constraints/primarykey/test_pk_updel_local.test
# setup
CREATE TABLE integers(i INTEGER PRIMARY KEY)
# query
UPDATE integers SET i=33
INSERT INTO integers VALUES (33)
# file: test/sql/constraints/primarykey/test_pk_updel_multi_column.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR, PRIMARY KEY(a, b))
# query
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
# reject
UPDATE test SET a = 15 WHERE a = 14
UPDATE test SET a = 4
UPDATE test SET b = NULL WHERE a = 13
# file: test/sql/constraints/primarykey/test_primary_key.test
# setup
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER)
# query
CREATE TABLE integers(i INTEGER PRIMARY KEY, j INTEGER)
INSERT INTO integers VALUES (6, 6)
INSERT INTO integers VALUES (7, 8)
INSERT INTO integers VALUES (7, 33)
# reject
INSERT INTO integers VALUES (3, 4), (3, 5)
INSERT INTO integers VALUES (NULL, 4)
# file: test/sql/constraints/check/check_struct.test
# setup
CREATE TABLE tbl(t ROW(t INTEGER) CHECK(tbl.t.t=42))
# query
CREATE TABLE tbl(t ROW(t INTEGER) CHECK(t.t=42))
INSERT INTO tbl VALUES ({'t': 42})
CREATE TABLE tbl(t ROW(t INTEGER) CHECK(tbl.t.t=42))
# reject
INSERT INTO tbl VALUES ({'t': 43})
# file: test/sql/constraints/check/test_check.test
# setup
CREATE TABLE integers(i INTEGER CHECK(i + j < 10), j INTEGER)
CREATE TABLE integers4(i INTEGER CHECK(integers4.i < 10), j INTEGER)
# query
CREATE TABLE integers(i INTEGER CHECK(i < 5))
CREATE TABLE integers(i INTEGER CHECK(i + j < 10), j INTEGER)
INSERT INTO integers VALUES (3, 3)
CREATE TABLE integers4(i INTEGER CHECK(integers4.i < 10), j INTEGER)
# reject
INSERT INTO integers VALUES (7)
INSERT INTO integers VALUES (5, 5)
INSERT INTO integers VALUES (3, 3), (5, 5)
CREATE TABLE indirect_subq( i INTEGER, CHECK (i > (2 * (SELECT(1)))) )
CREATE TABLE integers2(i INTEGER CHECK(i > (SELECT 42)), j INTEGER)
CREATE TABLE integers2(i INTEGER CHECK(i > SUM(j)), j INTEGER)
CREATE TABLE integers3(i INTEGER CHECK(k < 10), j INTEGER)
CREATE TABLE integers3(i INTEGER CHECK(integers3.k < 10), j INTEGER)
# file: test/sql/constraints/check/test_update_check_non_updated_columns.test
# setup
CREATE TABLE v0 ( v3 INTEGER, v2 INTEGER, v4 INTEGER, v1 INTEGER, CHECK ( ( ( NOT (v2 = v1) = v3 - 0 ) > (18 = 10) ) ), CHECK ( v3 = v4 ) )
CREATE TABLE v1 ( a INTEGER, b INTEGER, c INTEGER, CHECK (a = b), CHECK (b = c) )
CREATE TABLE v2 ( x INTEGER, y INTEGER, z INTEGER, CHECK (x + y = z) )
CREATE TABLE v3 ( id INTEGER, payload INTEGER, flag BOOLEAN, CHECK (flag IS NOT NULL) )
CREATE TABLE v_char ( username VARCHAR, status VARCHAR, CHECK (length(username) >= 3), CHECK (status IN ('active', 'inactive')) )
# query
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
# reject
UPDATE v1 SET c = 2
UPDATE v2 SET z = 7 WHERE x = 2
INSERT INTO v_char VALUES ('ab', 'active')
INSERT INTO v_char VALUES ('charlie', 'pending')
UPDATE v_char SET status = 'deleted' WHERE username = 'alice'
# file: test/sql/constraints/foreignkey/fk_19469.test
# setup
CREATE TABLE B (b1 INTEGER, b2 INTEGER, PRIMARY KEY(b1, b2))
CREATE TABLE A (a1 VARCHAR(1), a2 VARCHAR(1), a3 VARCHAR(1), a4 VARCHAR(1), a5 INTEGER, a6 INTEGER, PRIMARY KEY(a1, a2), UNIQUE(a3, a4), FOREIGN KEY (a5, a6) REFERENCES B(b1, b2))
CREATE TABLE C ( c1 INTEGER, c2 INTEGER, c3 VARCHAR(1), c4 VARCHAR(1), PRIMARY KEY (c1, c2), UNIQUE (c3, c4) )
CREATE TABLE D ( d1 INTEGER, d2 INTEGER, d3 VARCHAR(1), d4 VARCHAR(1), payload INTEGER, FOREIGN KEY (d1, d2) REFERENCES C (c1, c2), FOREIGN KEY (d3, d4) REFERENCES C (c3, c4) )
# query
CREATE TABLE B (b1 INTEGER, b2 INTEGER, PRIMARY KEY(b1, b2))
CREATE TABLE A (a1 VARCHAR(1), a2 VARCHAR(1), a3 VARCHAR(1), a4 VARCHAR(1), a5 INTEGER, a6 INTEGER, PRIMARY KEY(a1, a2), UNIQUE(a3, a4), FOREIGN KEY (a5, a6) REFERENCES B(b1, b2))
INSERT INTO B (b1, b2) VALUES (1, 2), (2, 3), (6, 7)
CREATE TABLE C ( c1 INTEGER, c2 INTEGER, c3 VARCHAR(1), c4 VARCHAR(1), PRIMARY KEY (c1, c2), UNIQUE (c3, c4) )
CREATE TABLE D ( d1 INTEGER, d2 INTEGER, d3 VARCHAR(1), d4 VARCHAR(1), payload INTEGER, FOREIGN KEY (d1, d2) REFERENCES C (c1, c2), FOREIGN KEY (d3, d4) REFERENCES C (c3, c4) )
INSERT INTO C VALUES (0, 1, 'a', 'b'), (1, 0, 'a', 'c'), (2, 2, 'd', 'e')
INSERT INTO D VALUES (0, 1, 'a', 'b', 10), (1, 0, 'a', 'c', 20), (2, 2, 'd', 'e', 30)
# reject
INSERT INTO A (a1, a2, a3, a4, a5, a6) VALUES ('x', 'y', 'z', 'u', 1, 2), ('y', 'z', 'x', 'v', 1, 2), ('x', 'x', 'y', 'y', 2, 3), ('z', 'z', 'v', 'x', 4, 5)
INSERT INTO D VALUES (9, 9, 'a', 'b', 40)
INSERT INTO D VALUES (0, 1, 'x', 'y', 50)
# file: test/sql/constraints/foreignkey/fk_20530.test
# setup
CREATE SCHEMA freddy
CREATE TABLE zippy( id INTEGER PRIMARY KEY )
CREATE TABLE george( zippy_id INTEGER, FOREIGN KEY (zippy_id) REFERENCES zippy(id) )
CREATE TABLE zippy_main( id INTEGER PRIMARY KEY )
CREATE TABLE george_main( zippy_id INTEGER, FOREIGN KEY (zippy_id) REFERENCES zippy_main(id) )
# query
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
# file: test/sql/constraints/foreignkey/fk_4309.test
# setup
CREATE TABLE tf_1 ( a integer, b integer, c integer, PRIMARY KEY (a), UNIQUE (b), UNIQUE (c) )
CREATE TABLE tf_2 ( d integer, e integer, f integer, FOREIGN KEY (d) REFERENCES tf_1 (a), FOREIGN KEY (e) REFERENCES tf_1 (b), FOREIGN KEY (f) REFERENCES tf_1 (c) )
# query
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
# reject
INSERT INTO tf_2 VALUES (1, 1, 1)
INSERT INTO tf_2 VALUES (2, 1, 1)
INSERT INTO tf_2 VALUES (1, 2, 1)
INSERT INTO tf_2 VALUES (1, 1, 2)
DELETE FROM tf_1 WHERE a = 2
# file: test/sql/constraints/foreignkey/fk_4365.test
# setup
create table x (c1 integer, primary key (c1))
create table y (c1 integer, foreign key (c1) references x (c1))
# query
create table x (c1 integer, primary key (c1))
create table y (c1 integer, foreign key (c1) references x (c1))
select count(*) from duckdb_constraints() where constraint_type = 'NOT NULL'
# file: test/sql/constraints/foreignkey/fk_case_insensitivity.test
# setup
create table a (i int primary key)
create table b (i int references A (i))
create table c (i int primary key, j int references C (i))
# query
create table a (a int not null, constraint pk_a primary key (A))
create table b (a int references a (a))
drop table b
drop table a
create table a (i int primary key)
create table b (i int references A (i))
create table c (i int primary key, j int references C (i))
# file: test/sql/constraints/foreignkey/fk_implicit_primary_key.test
# setup
create table b (i int references a)
create table a (i int, j int, primary key(i,j))
# query
create table b (i int references a)
insert into a values (1)
create table a (i int)
create table a (i int, j int, primary key(i,j))
# reject
insert into b values (1)
# file: test/sql/constraints/foreignkey/foreign_key_matching_columns.test
# setup
CREATE TABLE agency ( agency_id TEXT, agency_id_2 TEXT, agency_name TEXT NOT NULL, PRIMARY KEY (agency_id, agency_id_2) )
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (route_id, agency_id) REFERENCES agency )
# query
CREATE TABLE agency ( agency_id TEXT PRIMARY KEY, agency_name TEXT UNIQUE NOT NULL )
INSERT INTO agency VALUES (1, 1)
DROP TABLE routes
CREATE TABLE agency ( agency_id TEXT, agency_name TEXT NOT NULL )
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (agency_id) REFERENCES routes )
INSERT INTO routes VALUES (1, NULL)
INSERT INTO routes VALUES (2, 1)
CREATE TABLE agency ( agency_id TEXT, agency_id_2 TEXT, agency_name TEXT NOT NULL, PRIMARY KEY (agency_id, agency_id_2) )
# reject
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (agency_id) REFERENCES agency )
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (route_id, agency_id) REFERENCES agency )
INSERT INTO routes VALUES (1, 1)
DROP TABLE agency
INSERT INTO routes VALUES (2, 2)
# file: test/sql/constraints/foreignkey/test_action.test
# setup
CREATE TABLE t1(id INTEGER PRIMARY KEY)
CREATE TABLE t2(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id))
CREATE TABLE t3(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE NO ACTION)
CREATE TABLE t4(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE NO ACTION)
CREATE TABLE t5(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE RESTRICT)
CREATE TABLE t6(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE RESTRICT)
# query
CREATE TABLE t1(id INTEGER PRIMARY KEY)
CREATE TABLE t2(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id))
CREATE TABLE t3(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE NO ACTION)
CREATE TABLE t4(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE NO ACTION)
CREATE TABLE t5(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE RESTRICT)
CREATE TABLE t6(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE RESTRICT)
# reject
CREATE TABLE t7(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE CASCADE)
CREATE TABLE t8(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE CASCADE)
CREATE TABLE t9(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE SET DEFAULT)
CREATE TABLE t10(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE SET DEFAULT)
CREATE TABLE t11(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE SET NULL)
CREATE TABLE t12(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE SET NULL)
# file: test/sql/constraints/foreignkey/test_fk_alter.test
# setup
CREATE TABLE departments ( department_id INTEGER PRIMARY KEY, department_name VARCHAR(100) NOT NULL )
CREATE TABLE employees ( employee_id INTEGER PRIMARY KEY, employee_name VARCHAR(100) NOT NULL, department_id INT REFERENCES departments(department_id) )
# query
CREATE TABLE departments ( department_id INTEGER PRIMARY KEY, department_name VARCHAR(100) NOT NULL )
CREATE TABLE employees ( employee_id INTEGER PRIMARY KEY, employee_name VARCHAR(100) NOT NULL, department_id INT REFERENCES departments(department_id) )
ALTER TABLE employees RENAME TO old_employees
# reject
drop table departments
ALTER TABLE departments RENAME TO old_departments
# file: test/sql/constraints/foreignkey/test_fk_chain.test
# setup
CREATE TABLE t1(i1 INTEGER UNIQUE)
CREATE TABLE t2(i2 INTEGER PRIMARY KEY, FOREIGN KEY (i2) REFERENCES t1(i1))
CREATE TABLE t3(i3 INTEGER UNIQUE, FOREIGN KEY (i3) REFERENCES t2(i2))
CREATE TABLE t4(i4 INTEGER, FOREIGN KEY (i4) REFERENCES t3(i3))
# query
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
# reject
INSERT INTO t2 VALUES (5)
INSERT INTO t3 VALUES (4)
INSERT INTO t4 VALUES (3)
DELETE FROM t1 WHERE i1=4
DELETE FROM t2 WHERE i2=3
DELETE FROM t3 WHERE i3=2
DROP TABLE t1
# file: test/sql/constraints/foreignkey/test_fk_concurrency_conflicts.test
# setup
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
CREATE TABLE fk_integers_another(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# query
INSERT INTO pk_integers VALUES (1), (2), (3)
CREATE TABLE fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
INSERT INTO fk_integers VALUES (1)
INSERT INTO fk_integers VALUES (1), (2)
CREATE TABLE fk_integers_another(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# reject
DROP TABLE fk_integers
INSERT INTO fk_integers VALUES (4), (5)
# file: test/sql/constraints/foreignkey/test_fk_create_type.test
# setup
create type custom_type as integer
create type another_custom_type as integer
create table parent ( id custom_type primary key )
create table child ( parent another_custom_type references parent )
# query
create type custom_type as integer
create table parent ( id custom_type primary key )
create table child ( parent custom_type references parent )
drop table child
create table child ( parent integer references parent )
create type another_custom_type as integer
create table child ( parent another_custom_type references parent )
# file: test/sql/constraints/foreignkey/test_fk_cross_schema.test
# setup
CREATE SCHEMA s1
CREATE SCHEMA s2
CREATE TABLE s1.pk_integers(i INTEGER PRIMARY KEY)
# query
CREATE SCHEMA s1
CREATE SCHEMA s2
CREATE TABLE s1.pk_integers(i INTEGER PRIMARY KEY)
INSERT INTO s1.pk_integers VALUES (1), (2), (3)
# reject
CREATE TABLE s2.fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES s1.pk_intexgers(i))
# file: test/sql/constraints/foreignkey/test_fk_eager_constraint_checking.test
# setup
CREATE TABLE tbl_pk (i INT PRIMARY KEY, payload STRUCT(v VARCHAR, i INTEGER[]))
CREATE TABLE tbl_fk (i INT REFERENCES tbl_pk(i))
# query
SET storage_compatibility_version = 'v0.10.3'
USE fk_db
CREATE TABLE tbl_pk (i INT PRIMARY KEY, payload STRUCT(v VARCHAR, i INTEGER[]))
INSERT INTO tbl_pk VALUES (1, {'v': 'hello', 'i': [42]}), (2, {'v': 'world', 'i': [43]})
CREATE TABLE tbl_fk (i INT REFERENCES tbl_pk(i))
INSERT INTO tbl_fk VALUES (1), (1), (1)
USE other_fk_db
CHECKPOINT fk_db
DETACH fk_db
# reject
UPDATE fk_db.tbl_pk SET payload = {'v': 'new hello', 'i': [7]} WHERE i = 1
# file: test/sql/constraints/foreignkey/test_fk_export.test
# setup
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# query
DROP TABLE pk_integers
INSERT INTO fk_integers VALUES (3)
DELETE FROM fk_integers WHERE j=3
# reject
INSERT INTO fk_integers VALUES (4)
DELETE FROM pk_integers WHERE i=3
UPDATE pk_integers SET i=5 WHERE i=2
UPDATE fk_integers SET i=4 WHERE j=2
# file: test/sql/constraints/foreignkey/test_fk_multiple.test
# setup
CREATE TABLE pkt1( i1 INTEGER PRIMARY KEY CHECK(i1 < 3), j1 INTEGER UNIQUE )
CREATE TABLE pkt2( i2 INTEGER PRIMARY KEY, j2 INTEGER UNIQUE CHECK (j2 > 1000) )
CREATE TABLE fkt1( k1 INTEGER, l1 INTEGER, FOREIGN KEY(k1) REFERENCES pkt1(i1), FOREIGN KEY(l1) REFERENCES pkt2(i2) )
CREATE TABLE fkt2( k2 INTEGER, l2 INTEGER, FOREIGN KEY(k2) REFERENCES pkt1(j1), FOREIGN KEY(l2) REFERENCES pkt2(j2) )
# query
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
# reject
INSERT INTO pkt1 VALUES (3, 11)
INSERT INTO pkt2 VALUES (101, 1000)
INSERT INTO fkt1 VALUES (3, 101)
INSERT INTO fkt1 VALUES (2, 103)
INSERT INTO fkt2 VALUES (13, 1002)
INSERT INTO fkt1 VALUES (12, 1003)
DELETE FROM pkt1 WHERE i1=1
DELETE FROM pkt2 WHERE i2=102
# file: test/sql/constraints/foreignkey/test_fk_on_view_error.test
# setup
CREATE TABLE vdata AS SELECT * FROM (VALUES ('v2',)) v(id)
CREATE VIEW v AS SELECT * FROM vdata
# query
CREATE TABLE vdata AS SELECT * FROM (VALUES ('v2',)) v(id)
CREATE VIEW v AS SELECT * FROM vdata
# reject
CREATE TABLE t(v_id TEXT, FOREIGN KEY (v_id) REFERENCES v(id))
# file: test/sql/constraints/foreignkey/test_fk_pk_multi_transaction_delete.test
# setup
CREATE TABLE primary_table (id INT PRIMARY KEY)
CREATE TABLE secondary_table (primary_id INT, FOREIGN KEY (primary_id) REFERENCES primary_table(id))
# query
CREATE TABLE primary_table (id INT PRIMARY KEY)
CREATE TABLE secondary_table (primary_id INT, FOREIGN KEY (primary_id) REFERENCES primary_table(id))
INSERT INTO primary_table VALUES (42)
SELECT id FROM primary_table LIMIT 1
DELETE FROM primary_table WHERE id = 42
# reject
INSERT INTO secondary_table VALUES (42)
# file: test/sql/constraints/foreignkey/test_fk_rollback.test
# setup
CREATE TABLE pk_integers(i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# query
INSERT INTO pk_integers VALUES (1), (2)
INSERT INTO fk_integers VALUES (2)
# reject
DELETE FROM pk_integers WHERE i=2
# file: test/sql/constraints/foreignkey/test_fk_self_referencing.test
# setup
CREATE TABLE employee( id INTEGER PRIMARY KEY, managerid INTEGER, name VARCHAR, FOREIGN KEY(managerid) REFERENCES employee(id))
# query
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
# reject
CREATE TABLE employee( id INTEGER PRIMARY KEY, managerid INTEGER, name VARCHAR, FOREIGN KEY(managerid) REFERENCES employee(emp_id))
INSERT INTO employee VALUES (4, 4, 'Mark')
UPDATE employee SET id = 5 WHERE id = 2
DELETE FROM employee WHERE id = 2
UPDATE employee SET id = 2 WHERE id = 3
UPDATE employee SET managerid = 5 WHERE id = 4
ALTER TABLE employee RENAME COLUMN managerid TO managerid_new
ALTER TABLE employee ALTER COLUMN id SET DATA TYPE TEXT
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
# reject
INSERT INTO song VALUES (11, 1, 'A', 'A_song'), (12, 2, 'E', 'B_song'), (13, 3, 'C', 'C_song')
INSERT INTO song VALUES (11, 1, 'A', 'A_song'), (12, 5, 'D', 'B_song'), (13, 3, 'C', 'C_song')
DELETE FROM album WHERE albumname = 'C'
UPDATE song SET songartist = 5, songalbum = 'A' WHERE songname = 'B_song'
UPDATE album SET albumname='B' WHERE albumcover='C_cover'
UPDATE song SET songalbum='E' WHERE albumcover='C_song'
ALTER TABLE album RENAME COLUMN albumname TO albumname_new
ALTER TABLE song RENAME COLUMN songalbum TO songalbum_new
# file: test/sql/constraints/foreignkey/test_fk_transaction.test
# setup
CREATE TABLE pkt(i INTEGER PRIMARY KEY)
CREATE TABLE fkt(j INTEGER, FOREIGN KEY (j) REFERENCES pkt(i))
CREATE TABLE a(i INTEGER PRIMARY KEY)
CREATE TABLE b(j INTEGER, FOREIGN KEY (j) REFERENCES a(i))
# query
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
# reject
INSERT INTO fkt VALUES (3)
DELETE FROM pkt WHERE i = 1
DELETE FROM pkt WHERE i = 2
DROP TABLE pkt
# file: test/sql/constraints/foreignkey/test_fk_with_attached_db.test
# setup
CREATE TABLE IF NOT EXISTS t1 ( cache_key VARCHAR PRIMARY KEY, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, )
CREATE TABLE IF NOT EXISTS t2 ( cache_key VARCHAR NOT NULL, dose DOUBLE NOT NULL, PRIMARY KEY (cache_key, dose), FOREIGN KEY (cache_key) REFERENCES t1 (cache_key) )
# query
USE db1
CREATE TABLE IF NOT EXISTS t1 ( cache_key VARCHAR PRIMARY KEY, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, )
CREATE TABLE IF NOT EXISTS t2 ( cache_key VARCHAR NOT NULL, dose DOUBLE NOT NULL, PRIMARY KEY (cache_key, dose), FOREIGN KEY (cache_key) REFERENCES t1 (cache_key) )
ATTACH ':memory:' AS other
USE other
DETACH db1
# file: test/sql/constraints/foreignkey/test_foreignkey.test
# setup
CREATE SCHEMA s1
CREATE TABLE album(artistid INTEGER, albumname TEXT, albumcover TEXT, UNIQUE (artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, albumname))
CREATE TABLE s1.pkt(i INTEGER PRIMARY KEY)
CREATE TABLE s1.fkt(j INTEGER, FOREIGN KEY (j) REFERENCES s1.pkt(i))
CREATE TABLE pkt(i INTEGER UNIQUE)
CREATE TABLE fkt(j INTEGER, FOREIGN KEY (j) REFERENCES pkt(i))
CREATE TABLE t (id INT PRIMARY KEY, parent INT REFERENCES t (id))
CREATE INDEX k_index ON pkt(k)
CREATE INDEX l_index ON fkt(l)
# query
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
# reject
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songalbum) REFERENCES album(artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES albumlist(artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, album_name))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, song_album) REFERENCES album(artistid, albumname))
DELETE FROM album WHERE albumname='C'
UPDATE song SET songartist=5, songalbum='A' WHERE songname='B_song'
ALTER TABLE song ALTER COLUMN songartist SET DATA TYPE TEXT
# file: test/sql/copy/file_size_bytes.test
# setup
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0,10000) tbl(i)
# query
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0,10000) tbl(i)
set threads=1
INSERT INTO bigdata SELECT bigdata.* FROM bigdata, range(9)
PRAGMA verify_parallelism
pragma threads=4
# file: test/sql/copy/format_uuid.test
# setup
CREATE TABLE test2 as SELECT i as a, (i*2) as b, power(i,2) as c from range(0,10) tbl(i)
CREATE TABLE test3 as SELECT i as a, (i*3) as b, power(i,3) as c from range(0,10) tbl(i)
CREATE TABLE test4 as SELECT i as a, (i*4) as b, power(i,4) as c from range(0,10) tbl(i)
CREATE TABLE test5 as SELECT i as a, (i*5) as b, power(i,5) as c from range(0,10) tbl(i)
CREATE TABLE testpto as SELECT i as a, (i*10) as b, (i*100) as c from range(0,10000) tbl(i)
# query
PRAGMA threads=4
CREATE TABLE test2 as SELECT i as a, (i*2) as b, power(i,2) as c from range(0,10) tbl(i)
CREATE TABLE test3 as SELECT i as a, (i*3) as b, power(i,3) as c from range(0,10) tbl(i)
CREATE TABLE test4 as SELECT i as a, (i*4) as b, power(i,4) as c from range(0,10) tbl(i)
CREATE TABLE test5 as SELECT i as a, (i*5) as b, power(i,5) as c from range(0,10) tbl(i)
CREATE TABLE testpto as SELECT i as a, (i*10) as b, (i*100) as c from range(0,10000) tbl(i)
PRAGMA threads=1
# file: test/sql/copy/return_files.test
# setup
CREATE TABLE integers AS SELECT range i FROM range(200000)
CREATE TABLE integers2 AS SELECT range i, range % 4 j FROM range(200000)
# query
CREATE TABLE integers AS SELECT range i FROM range(200000)
SET preserve_insertion_order=false
SET preserve_insertion_order=true
SET threads=2
CREATE TABLE integers2 AS SELECT range i, range % 4 j FROM range(200000)
# file: test/sql/copy/return_stats.test
# setup
CREATE TABLE integers AS SELECT range i FROM range(200000)
CREATE TABLE bools AS SELECT i::bool i FROM range(2) i(i)
CREATE TABLE multi_column_test AS SELECT range i, range%10 j, case when range%2=0 then null else range end k FROM range(2500)
CREATE TABLE floating_point_test AS SELECT case when i%10=0 then null else i/10.0 end as fp FROM range(2500) t(i)
CREATE TABLE floating_point_nan AS SELECT case when i%10=0 then 'nan'::double when i%4=0 then null else i/10.0 end as fp FROM range(2500) t(i)
CREATE TABLE fp_nan_only AS SELECT 'nan'::float as float_val
CREATE TABLE string_test AS SELECT concat('thisisalongstring_', range) s FROM range(2500)
CREATE TABLE date_test AS SELECT (TIMESTAMP '2000-01-01' + interval (range) day)::DATE dt, TIMESTAMP '2000-01-01 12:12:12.123456' + interval (range) day ts, (TIMESTAMP '2000-01-01 12:12:12' + interval (range) day)::TIMESTAMP_S ts_s, (TIMESTAMP '2000-01-01 12:12:12.123' + interval (range) day)::TIMESTAMP_MS ts_ms, concat((TIMESTAMP '2000-01-01 12:12:12.123456' + interval (range) day)::VARCHAR, '789')::TIMESTAMP_NS ts_ns, TIME '00:00:00' + interval (10 * range) second t FROM range(2500)
CREATE TABLE empty_test AS FROM range(2500) LIMIT 0
CREATE TABLE decimal_test AS SELECT 25.3::DECIMAL(4,1) AS dec_i16, 123456.789::DECIMAL(9,3) AS dec_i32, 123456789123.456::DECIMAL(18,3) AS dec_i64, 12345678912345678912345678912345678.912::DECIMAL(38,3) AS dec_i128 UNION ALL SELECT 1.1::DECIMAL(4,1), 2.123::DECIMAL(9,3), 3.456::DECIMAL(18,3), 4.567::DECIMAL(38,3)
CREATE TABLE struct_test AS SELECT case when i%10=0 then null else {'x': i, 'y': case when i%2=0 then 100 + i else null end} end struct_val FROM range(2500) t(i)
CREATE TABLE list_test AS SELECT [i] l1, case when i%10=0 then null else [case when i%2=0 then 100 + i else null end] end l2 FROM range(2500) t(i)
CREATE TABLE medium_list_test AS SELECT [i, i, i] l1, case when i%10=0 then null else [case when i%2=0 then 100 + i else null end, null, case when i%2=0 then null else 100 + i end] end l2 FROM range(2500) t(i)
CREATE TABLE nested_struct_test AS SELECT {'s1': {'x': i}, 's2': {'s3': {'y': i}, 'l': [i]}} n FROM range(2500) t(i)
CREATE TABLE funky_names AS SELECT {'quoted ''"field name"': 42} """quoted col name"""
CREATE TABLE map_test AS SELECT MAP {'key'||i: i} AS map_val FROM range(2500) t(i)
CREATE TABLE array_test AS SELECT [i, i + 1, i + 2]::INT[3] AS array_val FROM range(2500) t(i)
CREATE TABLE partitioned_test AS SELECT range%2 as partition_key, range val FROM range(2500)
CREATE TABLE multi_partitioned_test AS SELECT range%2 as partition_key, range % 3 as partition_key2, range val FROM range(2500)
CREATE TABLE large_string AS SELECT repeat('A', 254) || '🦆' AS val UNION ALL SELECT repeat('Z', 254) || '🦆'
CREATE TABLE uuids AS SELECT uuid '47183823-2574-4bfd-b411-99ed177d3e43' uuid_val union all select uuid '00112233-4455-6677-8899-aabbccddeeff'
# query
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
# file: test/sql/copy/row_groups_per_file.test
# setup
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0,10000) tbl(i)
# query
set threads=4
# file: test/sql/copy/encryption/different_aes_ciphers.test
# setup
create table encrypted.fuu as select 42
# query
create table encrypted.fuu as select 42
DETACH encrypted
FROM encrypted.fuu
# file: test/sql/copy/encryption/encryption_storage_versions.test
# query
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
# file: test/sql/copy/encryption/force_mbedtls.test
# query
SET force_mbedtls_unsafe = 'true'
CREATE OR REPLACE TABLE encrypted.tbl AS SELECT * FROM range(10) t(i)
SET force_mbedtls_unsafe = 'false'
# file: test/sql/copy/encryption/test_different_encryption_versions.test
# query
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
# file: test/sql/copy/encryption/unencrypted_to_encrypted.test
# query
COPY FROM DATABASE unencrypted to encrypted
SELECT l_suppkey FROM encrypted.lineitem limit 10
# file: test/sql/copy/csv/19578.test
# query
SELECT Delimiter, Quote, Escape FROM sniff_csv("data/19578.csv")
SELECT Delimiter, Quote, Escape FROM sniff_csv("data/19578.csv", strict_mode=false)
# file: test/sql/copy/csv/21248.test
# query
SELECT * FROM read_csv(['data/csv/unionbyname_21248_*.csv'], union_by_name = true, ignore_errors = true, all_varchar = true)
# file: test/sql/copy/csv/column_names.test
# query
SELECT rsID, chr, pos, refb, altb FROM t1
SELECT rsID, chr, pos, refb, altb FROM t2
SELECT rsID, chr, pos, refb, altb FROM t3
# file: test/sql/copy/csv/copy_disable_parallelism.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER, c VARCHAR(10))
# query
CREATE TABLE test (a INTEGER, b INTEGER, c VARCHAR(10))
# file: test/sql/copy/csv/copy_expression.test
# setup
CREATE TABLE tbl(i INTEGER)
# query
COPY (SELECT * FROM range(5) t(i)) TO (getvariable('copy_target')) WITH (HEADER)
COPY tbl FROM (getvariable('copy_target'))
PREPARE v1 AS COPY (SELECT 'hello world' str) TO $1
# file: test/sql/copy/csv/csv_copy_sniffer.test
# setup
CREATE TABLE sales ( salesid INTEGER NOT NULL PRIMARY KEY, listid INTEGER NOT NULL, sellerid INTEGER NOT NULL, buyerid INTEGER NOT NULL, eventid INTEGER NOT NULL, dateid SMALLINT NOT NULL, qtysold SMALLINT NOT NULL, pricepaid DECIMAL (8,2), commission DECIMAL (8,2), saletime TIMESTAMP)
# query
CREATE TABLE sales ( salesid INTEGER NOT NULL PRIMARY KEY, listid INTEGER NOT NULL, sellerid INTEGER NOT NULL, buyerid INTEGER NOT NULL, eventid INTEGER NOT NULL, dateid SMALLINT NOT NULL, qtysold SMALLINT NOT NULL, pricepaid DECIMAL (8,2), commission DECIMAL (8,2), saletime TIMESTAMP)
# file: test/sql/copy/csv/csv_decimal_separator.test
# query
SELECT commas, periods FROM decimal_separators
SELECT typeof(commas), typeof(periods) FROM decimal_separators limit 1
SELECT commas, periods FROM decimal_separators2
SELECT typeof(commas), typeof(periods) FROM decimal_separators2 limit 1
SELECT commas, periods FROM decimal_separators3
SELECT commas, periods FROM decimal_separators4
SELECT typeof(commas), typeof(periods) FROM decimal_separators4 limit 1
# file: test/sql/copy/csv/csv_dtypes_union_by_name.test
# setup
CREATE TABLE ubn1(a BIGINT)
CREATE TABLE ubn2(a INTEGER, b INTEGER)
CREATE TABLE ubn3(a INTEGER, c INTEGER)
# query
CREATE TABLE ubn1(a BIGINT)
CREATE TABLE ubn2(a INTEGER, b INTEGER)
CREATE TABLE ubn3(a INTEGER, c INTEGER)
INSERT INTO ubn1 VALUES (1), (2), (9223372036854775807)
INSERT INTO ubn2 VALUES (3,4), (5, 6)
INSERT INTO ubn3 VALUES (100,101), (102, 103)
# file: test/sql/copy/csv/csv_enum.test
# setup
CREATE TYPE bla AS ENUM ('Y', 'N')
# query
CREATE TYPE bla AS ENUM ('Y', 'N')
# file: test/sql/copy/csv/csv_external_access.test
# setup
CREATE TABLE date_test(d date)
# query
CREATE TABLE date_test(d date)
SET enable_external_access=false
# reject
SET enable_external_access=true
# file: test/sql/copy/csv/csv_home_directory.test
# setup
CREATE TABLE integers AS SELECT * FROM range(10)
CREATE TABLE integers_load(i INTEGER)
# query
CREATE TABLE integers AS SELECT * FROM range(10)
SELECT * FROM '~/integers.csv'
CREATE TABLE integers_load(i INTEGER)
COPY integers_load FROM '~/integers.csv'
SELECT * FROM integers_load
SELECT COUNT(*) FROM '~/homedir_integers*.csv'
# file: test/sql/copy/csv/csv_limit_copy.test
# setup
CREATE TABLE integers AS FROM range(1000000) t(i)
# query
CREATE TABLE integers AS FROM range(1000000) t(i)
# file: test/sql/copy/csv/csv_line_too_long.test
# setup
CREATE TABLE T1 (name VARCHAR)
# query
CREATE TABLE T1 (name VARCHAR)
# file: test/sql/copy/csv/csv_nullstr_list.test
# setup
CREATE TABLE data (a VARCHAR, b VARCHAR, c VARCHAR)
# query
CREATE TABLE data (a VARCHAR, b VARCHAR, c VARCHAR)
FROM data
# file: test/sql/copy/csv/csv_projection_pushdown.test
# setup
CREATE TABLE tbl(i INT, j VARCHAR, k DATE)
# query
CREATE TABLE tbl(i INT, j VARCHAR, k DATE)
INSERT INTO tbl VALUES (42, 'hello world', NULL), (NULL, NULL, DATE '1992-01-01'), (100, 'thisisalongstring', DATE '2000-01-01')
SELECT COUNT(*) FROM v1
SELECT i, j, k FROM v1 ORDER BY i NULLS LAST
SELECT j FROM v1 ORDER BY j NULLS LAST
SELECT filename.replace('\', '/').split('/')[-1] FROM v1 LIMIT 1
# file: test/sql/copy/csv/csv_windows_mixed_separators.test
# query
CREATE TABLE s1.tbl AS SELECT * FROM range(10) t(i)
SELECT SUM(i) FROM s1.tbl
DETACH s1
# file: test/sql/copy/csv/empty_string_quote.test
# setup
CREATE TABLE customer(c_customer_sk INTEGER, c_customer_id VARCHAR, c_current_cdemo_sk INTEGER, c_current_hdemo_sk INTEGER, c_current_addr_sk INTEGER, c_first_shipto_date_sk INTEGER, c_first_sales_date_sk INTEGER, c_salutation VARCHAR, c_first_name VARCHAR, c_last_name VARCHAR, c_preferred_cust_flag VARCHAR, c_birth_day INTEGER, c_birth_month INTEGER, c_birth_year INTEGER, c_birth_country VARCHAR, c_login VARCHAR, c_email_address VARCHAR, c_last_review_date_sk INTEGER)
CREATE TABLE customer_quoted_nulls(c_customer_sk INTEGER, c_customer_id VARCHAR, c_current_cdemo_sk INTEGER, c_current_hdemo_sk INTEGER, c_current_addr_sk INTEGER, c_first_shipto_date_sk INTEGER, c_first_sales_date_sk INTEGER, c_salutation VARCHAR, c_first_name VARCHAR, c_last_name VARCHAR, c_preferred_cust_flag VARCHAR, c_birth_day INTEGER, c_birth_month INTEGER, c_birth_year INTEGER, c_birth_country VARCHAR, c_login VARCHAR, c_email_address VARCHAR, c_last_review_date_sk INTEGER)
# query
SELECT * FROM customer
SELECT COUNT(c_login) FROM customer_quoted_nulls
# file: test/sql/copy/csv/leading_zeros_autodetect.test
# query
SELECT CODGEO FROM leading_zeros LIMIT 1
SELECT typeof(CODGEO) FROM leading_zeros LIMIT 1
SELECT * FROM leading_zeros2
SELECT typeof(comune), typeof(codice_regione), typeof(codice_provincia) FROM leading_zeros2 LIMIT 1
select '09001'::int
select '00009001'::int
# file: test/sql/copy/csv/null_padding_big.test
# setup
CREATE TABLE test (a VARCHAR, b INTEGER, c INTEGER)
CREATE TABLE test2 (a VARCHAR, b INTEGER, c INTEGER, d INTEGER)
# query
CREATE TABLE test (a VARCHAR, b INTEGER, c INTEGER)
CREATE TABLE test2 (a VARCHAR, b INTEGER, c INTEGER, d INTEGER)
# file: test/sql/copy/csv/null_terminator.test
# setup
create or replace table t as (from values ('a' || chr(0) || 'b') t(i))
# query
create or replace table t as (from values ('a' || chr(0) || 'b') t(i))
# file: test/sql/copy/csv/plus_autodetect.test
# query
SELECT phone FROM phone_numbers
SELECT typeof(phone) FROM phone_numbers LIMIT 1
# file: test/sql/copy/csv/read_csv_variable.test
# query
SET VARIABLE csv_files=(SELECT LIST(file ORDER BY file) FROM globbed_files)
SELECT [parse_path(x)[-2:] for x in getvariable('csv_files')]
SELECT * FROM read_csv(getvariable('csv_files')) ORDER BY 1
# file: test/sql/copy/csv/recursive_csv_union_by_name.test
# query
WITH RECURSIVE t(i, j) AS ( SELECT 1, 0 UNION ALL ( SELECT i + 1, j + a FROM t, r WHERE i <= part ) ) SELECT * FROM t ORDER BY i
# file: test/sql/copy/csv/recursive_read_csv.test
# query
WITH RECURSIVE t(i) AS ( SELECT 1, NULL::DATE UNION ALL ( SELECT i+1, d FROM t, r WHERE i<5 ) ) SELECT * FROM t ORDER BY i
# file: test/sql/copy/csv/relaxed_quotes.test
# query
select count(*) from t
drop table t
# file: test/sql/copy/csv/test_15473.test
# setup
CREATE TABLE t1 AS select '2024/12/12' as a, '01:02:03' as b, '2020/01/01 01:02:03' as c from range(0,10000)
# query
CREATE TABLE t1 AS select '2024/12/12' as a, '01:02:03' as b, '2020/01/01 01:02:03' as c from range(0,10000)
insert into t1 values ('1','1','1')
# file: test/sql/copy/csv/test_auto_date.test
# setup
CREATE TABLE date_tests (a DATE)
CREATE TABLE stg_device_metadata_with_dates ( device_id VARCHAR, device_name VARCHAR, device_type VARCHAR, manufacturer VARCHAR, model_number VARCHAR, firmware_version VARCHAR, installation_date DATE, location_id VARCHAR, location_name VARCHAR, facility_zone VARCHAR, is_active BOOLEAN, expected_lifetime_months INT, maintenance_interval_days INT, last_maintenance_date DATE )
# query
CREATE TABLE date_tests (a DATE)
FROM date_tests
DROP TABLE date_tests
CREATE TABLE stg_device_metadata_with_dates ( device_id VARCHAR, device_name VARCHAR, device_type VARCHAR, manufacturer VARCHAR, model_number VARCHAR, firmware_version VARCHAR, installation_date DATE, location_id VARCHAR, location_name VARCHAR, facility_zone VARCHAR, is_active BOOLEAN, expected_lifetime_months INT, maintenance_interval_days INT, last_maintenance_date DATE )
FROM stg_device_metadata_with_dates
# file: test/sql/copy/csv/test_bgzf_read.test
# query
SELECT COUNT(*) FROM bgzf
SELECT COUNT(*) FROM concat
# file: test/sql/copy/csv/test_big_header.test
# setup
CREATE TABLE test (foo INTEGER, bar VARCHAR(10), baz VARCHAR(10), bam VARCHAR(10))
# query
CREATE TABLE test (foo INTEGER, bar VARCHAR(10), baz VARCHAR(10), bam VARCHAR(10))
SELECT COUNT(bam) FROM test WHERE bam = '!'
# file: test/sql/copy/csv/test_blob.test
# setup
CREATE TABLE blobs (b BYTEA)
# query
CREATE TABLE blobs (b BYTEA)
SELECT b FROM blobs
DELETE FROM blobs
# file: test/sql/copy/csv/test_compression_flag.test
# setup
CREATE TABLE lineitem(a INT NOT NULL, b INT NOT NULL, c INT NOT NULL)
# query
CREATE TABLE lineitem(a INT NOT NULL, b INT NOT NULL, c INT NOT NULL)
SELECT COUNT(*) FROM lineitem
SELECT a, b, c FROM lineitem ORDER BY a
DROP TABLE lineitem
# file: test/sql/copy/csv/test_copy.test
# setup
CREATE TABLE test2 (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE test_too_few_rows (a INTEGER, b INTEGER, c VARCHAR, d INTEGER)
CREATE TABLE test3 (a INTEGER, b INTEGER)
CREATE TABLE test4 (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE test (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE empty_table (a INTEGER, b INTEGER, c VARCHAR(10))
CREATE TABLE unterminated (a VARCHAR)
CREATE TABLE vsize (a INTEGER, b INTEGER, c VARCHAR(10))
# query
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
# file: test/sql/copy/csv/test_copy_default.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR DEFAULT('hello'), c INTEGER DEFAULT(3+4))
# query
CREATE TABLE test (a INTEGER, b VARCHAR DEFAULT('hello'), c INTEGER DEFAULT(3+4))
SELECT COUNT(a), COUNT(b), COUNT(c), MIN(LENGTH(b)), MAX(LENGTH(b)), SUM(a), SUM(c) FROM test
# file: test/sql/copy/csv/test_copy_gzip.test
# setup
CREATE TABLE lineitem(l_orderkey INT NOT NULL, l_partkey INT NOT NULL, l_suppkey INT NOT NULL, l_linenumber INT NOT NULL, l_quantity INTEGER NOT NULL, l_extendedprice DECIMAL(15, 2) NOT NULL, l_discount DECIMAL(15, 2) NOT NULL, l_tax DECIMAL(15, 2) NOT NULL, l_returnflag VARCHAR(1) NOT NULL, l_linestatus VARCHAR(1) NOT NULL, l_shipdate DATE NOT NULL, l_commitdate DATE NOT NULL, l_receiptdate DATE NOT NULL, l_shipinstruct VARCHAR(25) NOT NULL, l_shipmode VARCHAR(10) NOT NULL, l_comment VARCHAR(44) NOT NULL)
# query
SELECT l_partkey FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
SELECT COUNT(*) FROM (FROM lineitem EXCEPT FROM lineitem_rt)
# file: test/sql/copy/csv/test_copy_null.test
# setup
CREATE TABLE test_null_option (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10), col_e VARCHAR)
CREATE TABLE test_null_option_2 (col_a INTEGER, col_b INTEGER, col_c VARCHAR(10), col_d VARCHAR(10))
# query
CREATE TABLE test_null_option (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10), col_e VARCHAR)
SELECT * FROM test_null_option ORDER BY 1 LIMIT 3
DELETE FROM test_null_option
CREATE TABLE test_null_option_2 (col_a INTEGER, col_b INTEGER, col_c VARCHAR(10), col_d VARCHAR(10))
# file: test/sql/copy/csv/test_csv_error_message_type.test
# setup
CREATE TABLE venue ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats INTEGER )
CREATE TABLE venue_2 ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats VARCHAR )
# query
CREATE TABLE venue ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats INTEGER )
CREATE TABLE venue_2 ( venueid SMALLINT NOT NULL /*PRIMARY KEY*/ , venuename VARCHAR (100) , venuecity VARCHAR (30) , venuestate CHAR (2) , venueseats VARCHAR )
SELECT COUNT(*) from venue_2
DROP TABLE venue_2
# file: test/sql/copy/csv/test_csv_json.test
# setup
create table t (a json)
# query
create table t (a json)
FROM t
# file: test/sql/copy/csv/test_csv_no_trailing_newline.test
# setup
CREATE TABLE no_newline (a INTEGER, b INTEGER, c VARCHAR(10))
# query
CREATE TABLE no_newline (a INTEGER, b INTEGER, c VARCHAR(10))
# file: test/sql/copy/csv/test_csv_timestamp_tz.test
# query
SET TimeZone='UTC'
# file: test/sql/copy/csv/test_csv_timestamp_tz_icu.test
# query
SET Calendar = 'gregorian'
SET TimeZone = 'America/Los_Angeles'
# file: test/sql/copy/csv/test_date.test
# setup
CREATE TABLE date_test(d date)
# query
SELECT cast(d as string) FROM date_test
# file: test/sql/copy/csv/test_dateformat.test
# setup
CREATE TABLE dates (d DATE)
CREATE TABLE new_dates (d DATE)
CREATE TABLE timestamps(t TIMESTAMP)
CREATE TABLE new_timestamps (t TIMESTAMP)
# query
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
# file: test/sql/copy/csv/test_empty_quote.test
# setup
CREATE TABLE no_quote(a VARCHAR, b VARCHAR)
# query
CREATE TABLE no_quote(a VARCHAR, b VARCHAR)
SELECT * FROM no_quote
# file: test/sql/copy/csv/test_enum_csv.test
# setup
CREATE TYPE mood AS ENUM ('happy', 'sad', 'angry')
# query
CREATE TYPE mood AS ENUM ('happy', 'sad', 'angry')
# file: test/sql/copy/csv/test_escape_long_value.test
# setup
CREATE TABLE long_escaped_value (a INTEGER, b INTEGER, c VARCHAR)
CREATE TABLE long_escaped_value_unicode (a INTEGER, b INTEGER, c VARCHAR)
# query
select count(*) from T
CREATE TABLE long_escaped_value (a INTEGER, b INTEGER, c VARCHAR)
SELECT * FROM long_escaped_value
CREATE TABLE long_escaped_value_unicode (a INTEGER, b INTEGER, c VARCHAR)
SELECT * FROM long_escaped_value_unicode
# file: test/sql/copy/csv/test_export_force_quotes.test
# setup
create table integers(i int)
# query
create table integers(i int)
insert into integers values (42)
drop table integers
select * from integers
# file: test/sql/copy/csv/test_export_not_null.test
# setup
create table tbl(a VARCHAR NOT NULL)
create table tbl_2(a VARCHAR NOT NULL, b VARCHAR NOT NULL, c VARCHAR NOT NULL, d VARCHAR)
# query
create table tbl(a VARCHAR NOT NULL)
insert into tbl values ('')
abort
create table tbl_2(a VARCHAR NOT NULL, b VARCHAR NOT NULL, c VARCHAR NOT NULL, d VARCHAR)
insert into tbl_2 values ('','','','')
select * from tbl_2
# file: test/sql/copy/csv/test_force_not_null.test
# setup
CREATE TABLE test (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
# query
CREATE TABLE test (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
SELECT * FROM test ORDER BY 1
# file: test/sql/copy/csv/test_force_quote.test
# setup
CREATE TABLE test (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
CREATE TABLE test2 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
CREATE TABLE test3 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
# query
CREATE TABLE test2 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
CREATE TABLE test3 (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10))
# file: test/sql/copy/csv/test_glob_reorder_lineitem.test
# query
call dbgen(sf=0.1)
# file: test/sql/copy/csv/test_glob_reorder_null.test
# setup
create table t1 as select 1 as a,1 as b from range(3)
create table t2 (b integer, a integer)
# query
create table t1 as select 1 as a,1 as b from range(3)
create table t2 (b integer, a integer)
insert into t2 select NULL as b,NULL as a from range(30000)
insert into t2 values (3,4)
# file: test/sql/copy/csv/test_glob_type.test
# setup
CREATE TABLE T AS SELECT 'bar,baz', UNION ALL SELECT ',baz' from range (0,100000)
# query
CREATE TABLE T AS SELECT 'bar,baz', UNION ALL SELECT ',baz' from range (0,100000)
# file: test/sql/copy/csv/test_greek_utf8.test
# query
SELECT * FROM greek_utf8 ORDER BY 1
DELETE FROM greek_utf8
SELECT * FROM greek_utf8
# file: test/sql/copy/csv/test_header_only.test
# query
DESCRIBE T
# file: test/sql/copy/csv/test_ignore_errors.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE nullable_type (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10))
# query
SELECT * FROM integers AS too_little_columns
SELECT * FROM integers AS too_many_columns
CREATE TABLE nullable_type (col_a INTEGER, col_b VARCHAR(10), col_c VARCHAR(10), col_d VARCHAR(10))
SELECT * FROM nullable_type
# file: test/sql/copy/csv/test_ignore_errors_end_of_chunk.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
# query
SELECT * FROM integers limit 1
select count(*) from integers
# file: test/sql/copy/csv/test_imdb.test
# setup
CREATE TABLE movie_info (id integer NOT NULL PRIMARY KEY, movie_id integer NOT NULL, info_type_id integer NOT NULL, info text NOT NULL, note text)
# query
CREATE TABLE movie_info (id integer NOT NULL PRIMARY KEY, movie_id integer NOT NULL, info_type_id integer NOT NULL, info text NOT NULL, note text)
SELECT * FROM movie_info
# file: test/sql/copy/csv/test_infinite_loop_escape.test
# setup
CREATE TABLE trigger_loop (a VARCHAR)
# query
CREATE TABLE trigger_loop (a VARCHAR)
INSERT INTO trigger_loop VALUES ('"')
# file: test/sql/copy/csv/test_insert_into_types.test
# setup
CREATE TABLE ppl ( name VARCHAR )
CREATE TABLE users ( id INTEGER NOT NULL, /*primary key*/ name VARCHAR(10) NOT NULL, email VARCHAR )
CREATE TABLE proj ( email VARCHAR, id integer NOT NULL )
CREATE TABLE users_age ( id INTEGER NOT NULL, name VARCHAR(10) NOT NULL, email VARCHAR, age integer )
create table timestamps(ts timestamp, dt date)
# query
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
# file: test/sql/copy/csv/test_issue3562_assertion.test
# query
select objectid, name from test ORDER BY objectid limit 10
# file: test/sql/copy/csv/test_lineitem.test
# setup
CREATE TABLE lineitem(l_orderkey INT NOT NULL, l_partkey INT NOT NULL, l_suppkey INT NOT NULL, l_linenumber INT NOT NULL, l_quantity INTEGER NOT NULL, l_extendedprice DECIMAL(15,2) NOT NULL, l_discount DECIMAL(15,2) NOT NULL, l_tax DECIMAL(15,2) NOT NULL, l_returnflag VARCHAR(1) NOT NULL, l_linestatus VARCHAR(1) NOT NULL, l_shipdate DATE NOT NULL, l_commitdate DATE NOT NULL, l_receiptdate DATE NOT NULL, l_shipinstruct VARCHAR(25) NOT NULL, l_shipmode VARCHAR(10) NOT NULL, l_comment VARCHAR(44) NOT NULL)
# query
SELECT l_partkey, l_comment FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
DELETE FROM lineitem
SELECT * FROM lineitem
# file: test/sql/copy/csv/test_lineitem_gz.test
# query
CALL dbgen(sf=10)
set memory_limit='32gb'
# file: test/sql/copy/csv/test_long_line.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR, c INTEGER)
# query
CREATE TABLE test (a INTEGER, b VARCHAR, c INTEGER)
SELECT LENGTH(b) FROM test ORDER BY a
SELECT SUM(a), SUM(c) FROM test
# file: test/sql/copy/csv/test_many_columns.test
# query
SHOW t
# file: test/sql/copy/csv/test_ncvoter.test
# setup
CREATE TABLE IF NOT EXISTS ncvoters(county_id INTEGER, county_desc STRING, voter_reg_num STRING,status_cd STRING, voter_status_desc STRING, reason_cd STRING, voter_status_reason_desc STRING, absent_ind STRING, name_prefx_cd STRING,last_name STRING, first_name STRING, midl_name STRING, name_sufx_cd STRING, full_name_rep STRING,full_name_mail STRING, house_num STRING, half_code STRING, street_dir STRING, street_name STRING, street_type_cd STRING, street_sufx_cd STRING, unit_designator STRING, unit_num STRING, res_city_desc STRING,state_cd STRING, zip_code STRING, res_street_address STRING, res_city_state_zip STRING, mail_addr1 STRING, mail_addr2 STRING, mail_addr3 STRING, mail_addr4 STRING, mail_city STRING, mail_state STRING, mail_zipcode STRING, mail_city_state_zip STRING, area_cd STRING, phone_num STRING, full_phone_number STRING, drivers_lic STRING, race_code STRING, race_desc STRING, ethnic_code STRING, ethnic_desc STRING, party_cd STRING, party_desc STRING, sex_code STRING, sex STRING, birth_age STRING, birth_place STRING, registr_dt STRING, precinct_abbrv STRING, precinct_desc STRING,municipality_abbrv STRING, municipality_desc STRING, ward_abbrv STRING, ward_desc STRING, cong_dist_abbrv STRING, cong_dist_desc STRING, super_court_abbrv STRING, super_court_desc STRING, judic_dist_abbrv STRING, judic_dist_desc STRING, nc_senate_abbrv STRING, nc_senate_desc STRING, nc_house_abbrv STRING, nc_house_desc STRING,county_commiss_abbrv STRING, county_commiss_desc STRING, township_abbrv STRING, township_desc STRING,school_dist_abbrv STRING, school_dist_desc STRING, fire_dist_abbrv STRING, fire_dist_desc STRING, water_dist_abbrv STRING, water_dist_desc STRING, sewer_dist_abbrv STRING, sewer_dist_desc STRING, sanit_dist_abbrv STRING, sanit_dist_desc STRING, rescue_dist_abbrv STRING, rescue_dist_desc STRING, munic_dist_abbrv STRING, munic_dist_desc STRING, dist_1_abbrv STRING, dist_1_desc STRING, dist_2_abbrv STRING, dist_2_desc STRING, confidential_ind STRING, age STRING, ncid STRING, vtd_abbrv STRING, vtd_desc STRING)
# query
SELECT county_id, county_desc, vtd_desc, name_prefx_cd FROM ncvoters
DELETE FROM ncvoters
SELECT * FROM ncvoters
# file: test/sql/copy/csv/test_nfc.test
# setup
CREATE TABLE nfcstrings (s STRING)
# query
CREATE TABLE nfcstrings (s STRING)
SELECT COUNT(*) FROM nfcstrings WHERE s COLLATE NFC = 'ü'
# file: test/sql/copy/csv/test_nfc_suite.test
# setup
CREATE TABLE nfcstrings (source STRING, nfc STRING, nfd STRING)
# query
CREATE TABLE nfcstrings (source STRING, nfc STRING, nfd STRING)
SELECT COUNT(*) FROM nfcstrings
SELECT COUNT(*) FROM nfcstrings WHERE source COLLATE NFC=nfc
SELECT COUNT(*) FROM nfcstrings WHERE nfc COLLATE NFC=nfd
DROP TABLE nfcstrings
# file: test/sql/copy/csv/test_non_unicode_header.test
# query
drop table if exists reject_errors
select * exclude(scan_id ) from reject_errors order by all limit 5
# file: test/sql/copy/csv/test_null_padding_projection.test
# setup
create view T_2 as SELECT * EXCLUDE (SETTLEMENTDATE, XX, filename, I), CAST(SETTLEMENTDATE AS TIMESTAMP) AS SETTLEMENTDATE, split(filename, '/')[8] AS file, isoyear(CAST(SETTLEMENTDATE AS TIMESTAMP)) AS "YEAR" FROM T
# query
from np
select a from np
select b,d from np
set threads =1
create view T_2 as SELECT * EXCLUDE (SETTLEMENTDATE, XX, filename, I), CAST(SETTLEMENTDATE AS TIMESTAMP) AS SETTLEMENTDATE, split(filename, '/')[8] AS file, isoyear(CAST(SETTLEMENTDATE AS TIMESTAMP)) AS "YEAR" FROM T
select count(*) from T_2
# file: test/sql/copy/csv/test_null_padding_union.test
# query
select * from v limit 10
select count(*) from v where a is null
select count(*) from v where b is null
select count(*) from v where c is null
select count(*) from v where d is null
# file: test/sql/copy/csv/test_ontime.test
# setup
CREATE TABLE ontime(year SMALLINT, quarter SMALLINT, month SMALLINT, dayofmonth SMALLINT, dayofweek SMALLINT, flightdate DATE, uniquecarrier CHAR(7), airlineid DECIMAL(8,2), carrier CHAR(2), tailnum VARCHAR(50), flightnum VARCHAR(10), originairportid INTEGER, originairportseqid INTEGER, origincitymarketid INTEGER, origin CHAR(5), origincityname VARCHAR(100), originstate CHAR(2), originstatefips VARCHAR(10), originstatename VARCHAR(100), originwac DECIMAL(8,2), destairportid INTEGER, destairportseqid INTEGER, destcitymarketid INTEGER, dest CHAR(5), destcityname VARCHAR(100), deststate CHAR(2), deststatefips VARCHAR(10), deststatename VARCHAR(100), destwac DECIMAL(8,2), crsdeptime DECIMAL(8,2), deptime DECIMAL(8,2), depdelay DECIMAL(8,2), depdelayminutes DECIMAL(8,2), depdel15 DECIMAL(8,2), departuredelaygroups DECIMAL(8,2), deptimeblk VARCHAR(20), taxiout DECIMAL(8,2), wheelsoff DECIMAL(8,2), wheelson DECIMAL(8,2), taxiin DECIMAL(8,2), crsarrtime DECIMAL(8,2), arrtime DECIMAL(8,2), arrdelay DECIMAL(8,2), arrdelayminutes DECIMAL(8,2), arrdel15 DECIMAL(8,2), arrivaldelaygroups DECIMAL(8,2), arrtimeblk VARCHAR(20), cancelled SMALLINT, cancellationcode CHAR(1), diverted SMALLINT, crselapsedtime DECIMAL(8,2), actualelapsedtime DECIMAL(8,2), airtime DECIMAL(8,2), flights DECIMAL(8,2), distance DECIMAL(8,2), distancegroup SMALLINT, carrierdelay DECIMAL(8,2), weatherdelay DECIMAL(8,2), nasdelay DECIMAL(8,2), securitydelay DECIMAL(8,2), lateaircraftdelay DECIMAL(8,2), firstdeptime VARCHAR(10), totaladdgtime VARCHAR(10), longestaddgtime VARCHAR(10), divairportlandings VARCHAR(10), divreacheddest VARCHAR(10), divactualelapsedtime VARCHAR(10), divarrdelay VARCHAR(10), divdistance VARCHAR(10), div1airport VARCHAR(10), div1aiportid INTEGER, div1airportseqid INTEGER, div1wheelson VARCHAR(10), div1totalgtime VARCHAR(10), div1longestgtime VARCHAR(10), div1wheelsoff VARCHAR(10), div1tailnum VARCHAR(10), div2airport VARCHAR(10), div2airportid INTEGER, div2airportseqid INTEGER, div2wheelson VARCHAR(10), div2totalgtime VARCHAR(10), div2longestgtime VARCHAR(10), div2wheelsoff VARCHAR(10), div2tailnum VARCHAR(10), div3airport VARCHAR(10), div3airportid INTEGER, div3airportseqid INTEGER, div3wheelson VARCHAR(10), div3totalgtime VARCHAR(10), div3longestgtime VARCHAR(10), div3wheelsoff VARCHAR(10), div3tailnum VARCHAR(10), div4airport VARCHAR(10), div4airportid INTEGER, div4airportseqid INTEGER, div4wheelson VARCHAR(10), div4totalgtime VARCHAR(10), div4longestgtime VARCHAR(10), div4wheelsoff VARCHAR(10), div4tailnum VARCHAR(10), div5airport VARCHAR(10), div5airportid INTEGER, div5airportseqid INTEGER, div5wheelson VARCHAR(10), div5totalgtime VARCHAR(10), div5longestgtime VARCHAR(10), div5wheelsoff VARCHAR(10), div5tailnum VARCHAR(10))
# query
SELECT year, uniquecarrier, origin, origincityname, div5longestgtime FROM ontime
DELETE FROM ontime
# file: test/sql/copy/csv/test_partition_compression.test
# setup
CREATE TABLE test AS VALUES ('a', 'foo', 1), ('a', 'foo', 2), ('a', 'bar', 1), ('b', 'bar', 1)
# query
CREATE TABLE test AS VALUES ('a', 'foo', 1), ('a', 'foo', 2), ('a', 'bar', 1), ('b', 'bar', 1)
# file: test/sql/copy/csv/test_quoted_later_escaped.test
# setup
CREATE TABLE T as select '1, "Oogie Boogie"' from range (100000)
CREATE TABLE T_2 as select '1, "Oogie Boogie"' from range (5000)
# query
CREATE TABLE T as select '1, "Oogie Boogie"' from range (100000)
insert into T values ('2, """sir"" Oogie Boogie"')
CREATE TABLE T_2 as select '1, "Oogie Boogie"' from range (5000)
insert into T_2 values ('2, "\"sir\" Oogie Boogie"')
# file: test/sql/copy/csv/test_quoted_newline.test
# setup
CREATE TABLE test (a VARCHAR, b INTEGER)
# query
CREATE TABLE test (a VARCHAR, b INTEGER)
SELECT SUM(b) FROM test
SELECT string_split_regex(a, '[\r\n]+') FROM test ORDER BY a
# file: test/sql/copy/csv/test_read_csv.test
# setup
CREATE TABLE dates (d DATE)
# query
SELECT * FROM dates ORDER BY 1
SELECT l_partkey, RTRIM(l_comment) FROM lineitem WHERE l_orderkey=1 ORDER BY l_linenumber
# file: test/sql/copy/csv/test_skip_bom.test
# query
SELECT * FROM people
SELECT * FROM people2
# file: test/sql/copy/csv/test_thijs_unquoted_file.test
# query
select * from T limit 1
# file: test/sql/copy/csv/test_thousands_separator.test
# setup
CREATE TABLE T (name varchar, money double, city varchar)
# query
CREATE TABLE T (name varchar, money double, city varchar)
FROM T
# file: test/sql/copy/csv/test_timestamptz_12926.test
# setup
CREATE TABLE test (column0 timestamptz)
# query
CREATE TABLE test (column0 timestamptz)
FROM test
# file: test/sql/copy/csv/test_web_page.test
# setup
CREATE TABLE web_page(wp_web_page_sk integer not null, wp_web_page_id char(16) not null, wp_rec_start_date date, wp_rec_end_date date, wp_creation_date_sk integer, wp_access_date_sk integer, wp_autogen_flag char(1), wp_customer_sk integer, wp_url varchar(100), wp_type char(50), wp_char_count integer, wp_link_count integer, wp_image_count integer, wp_max_ad_count integer, primary key (wp_web_page_sk))
# query
SELECT * FROM web_page ORDER BY wp_web_page_sk LIMIT 3
DELETE FROM web_page
SELECT * FROM web_page
# file: test/sql/copy/csv/test_windows_newline.test
# setup
CREATE TABLE test (a INTEGER)
# query
SELECT SUM(a), MIN(LENGTH(b)), MAX(LENGTH(b)), SUM(LENGTH(b)), SUM(c) FROM test
# file: test/sql/copy/csv/timestamp_with_tz.test
# setup
CREATE TABLE tbl(id int, ts timestamp)
CREATE TABLE tbl_tz(id int, ts timestamptz)
# query
CREATE TABLE tbl(id int, ts timestamp)
SELECT TRY_CAST('2022/01/27 11:04:57 PM' AS TIMESTAMPTZ)
CREATE TABLE tbl_tz(id int, ts timestamptz)
SELECT * FROM tbl_tz
# file: test/sql/copy/csv/tsv_copy.test
# setup
CREATE TABLE people(id INTEGER, name VARCHAR)
# query
CREATE TABLE people(id INTEGER, name VARCHAR)
INSERT INTO people VALUES (1, 'Mark'), (2, 'Hannes')
# file: test/sql/copy/csv/write_header_default.test
# setup
create table t (a integer)
# query
create table t (a integer)
insert into t values (1),(2),(NULL)
# file: test/sql/copy/csv/zstd_crash.test
# setup
CREATE TABLE test_zst(a INTEGER, b INTEGER, c INTEGER, d VARCHAR, e VARCHAR)
# query
CREATE TABLE test_zst(a INTEGER, b INTEGER, c INTEGER, d VARCHAR, e VARCHAR)
# file: test/sql/copy/csv/multidelimiter/test_abac.test
# setup
CREATE TABLE abac_tbl (a VARCHAR)
# query
CREATE TABLE abac_tbl (a VARCHAR, b VARCHAR, c VARCHAR)
SELECT * FROM abac_tbl
DELETE FROM abac_tbl
DROP TABLE abac_tbl
CREATE TABLE abac_tbl (a VARCHAR)
CREATE TABLE abac_tbl (a VARCHAR, b VARCHAR)
# file: test/sql/copy/csv/duck_fuzz/test_internal_4048.test
# setup
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
# query
create table all_types as select * exclude(small_enum, medium_enum, large_enum) from test_all_types() limit 0
# reject
SELECT DISTINCT NULL, c3, (c4 <= c1), (c3 BETWEEN c4 AND c2) FROM sniff_csv('1a616242-1dcd-4914-99d1-16119d9b6e4c', "names" := ['1970-01-01'::DATE, 'infinity'::DATE, '-infinity'::DATE, NULL, '2022-05-12'::DATE], filename := '9be2bc9d-d49f-4564-bfa4-6336b211a874') AS t5(c1, c2, c3, c4) WHERE c1 GROUP BY c3 LIMIT ('c4000757-69ca-400e-b58a-1dac73b85595' IS NULL)
# file: test/sql/copy/csv/parallel/csv_parallel_null_option.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE exprtest (a INTEGER, b INTEGER)
# query
PRAGMA enable_profiling
PRAGMA profiling_mode = detailed
SELECT min (i + i) FROM integers
CREATE TABLE exprtest (a INTEGER, b INTEGER)
INSERT INTO exprtest VALUES (42, 10), (43, 100), (NULL, 1), (45, -1)
SELECT min (a + a ) FROM exprtest
SELECT a FROM exprtest WHERE a BETWEEN 43 AND 44
SELECT CASE a WHEN 42 THEN 100 WHEN 43 THEN 200 ELSE 300 END FROM exprtest
# file: test/sql/copy/csv/parallel/test_parallel_error_messages.test
# query
SET threads=4
# file: test/sql/copy/csv/overwrite/test_copy_overwrite.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR(10))
# query
CREATE TABLE test (a INTEGER, b VARCHAR(10))
INSERT INTO test VALUES (1, 'hello'), (2, 'world '), (3, ' xx')
# file: test/sql/copy/csv/overwrite/test_overwrite_pipe_windows.test
# query
copy (select 42) to 'con:'
# file: test/sql/copy/csv/rejects/csv_incorrect_columns_amount_rejects.test
# query
SELECT * EXCLUDE (scan_id) FROM reject_errors order by all
DROP TABLE reject_errors
DROP TABLE reject_scans
# file: test/sql/copy/csv/rejects/csv_rejects_auto.test
# query
SELECT COUNT(*) FROM reject_errors
SELECT COUNT(*) FROM csv_rejects_table
DROP TABLE csv_rejects_table
# file: test/sql/copy/csv/rejects/csv_rejects_read.test
# query
SELECT * EXCLUDE (scan_id) FROM reject_errors
SELECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_scans ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_errors ORDER BY ALL
SELECT * EXCLUDE (scan_id, file_id) FROM reject_errors ORDER BY column_name
# file: test/sql/copy/csv/rejects/csv_rejects_two_tables.test
# setup
create temporary table t (a integer)
# query
SELECT * EXCLUDE (scan_id) FROM reject_scans order by all
drop table reject_scans
SELECT * EXCLUDE (scan_id) FROM rejects_errors_2 order by all
drop table reject_errors
SELECT * EXCLUDE (scan_id) FROM rejects_scan_2 order by all
SELECT * EXCLUDE (scan_id) FROM rejects_scan_3 order by all
SELECT * EXCLUDE (scan_id) FROM rejects_errors_3 order by all
create temporary table t (a integer)
# file: test/sql/copy/csv/rejects/csv_unquoted_rejects.test
# query
SELECT regexp_replace(file_path, '\\', '/', 'g'), line, column_idx, column_name, error_type, csv_line,line_byte_position, byte_position FROM reject_scans inner join reject_errors on (reject_scans.scan_id = reject_errors.scan_id and reject_scans.file_id = reject_errors.file_id)
SELECT regexp_replace(file_path, '\\', '/', 'g'), line, column_idx, column_name, error_type, line_byte_position,byte_position FROM reject_scans inner join reject_errors on (reject_scans.scan_id = reject_errors.scan_id and reject_scans.file_id = reject_errors.file_id)
# file: test/sql/copy/csv/rejects/test_multiple_errors_same_line.test
# query
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY ALL
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY byte_position
SElECT * EXCLUDE (scan_id) FROM reject_errors ORDER BY byte_position, error_message
# file: test/sql/copy/csv/auto/test_auto_8231.test
# query
SELECT * from locations_header_trailing_comma
describe locations_header_trailing_comma
# file: test/sql/copy/csv/auto/test_auto_cranlogs.test
# query
SELECT COUNT(*) FROM cranlogs
SELECT * FROM cranlogs LIMIT 5
(SELECT * FROM cranlogs EXCEPT SELECT * FROM cranlogs2) UNION ALL (SELECT * FROM cranlogs2 EXCEPT SELECT * FROM cranlogs)
# file: test/sql/copy/csv/auto/test_auto_greek_ncvoter.test
# setup
CREATE TABLE IF NOT EXISTS ncvoters(county_id INTEGER, county_desc STRING, voter_reg_num STRING,status_cd STRING, voter_status_desc STRING, reason_cd STRING, voter_status_reason_desc STRING, absent_ind STRING, name_prefx_cd STRING,last_name STRING, first_name STRING, midl_name STRING, name_sufx_cd STRING, full_name_rep STRING,full_name_mail STRING, house_num STRING, half_code STRING, street_dir STRING, street_name STRING, street_type_cd STRING, street_sufx_cd STRING, unit_designator STRING, unit_num STRING, res_city_desc STRING,state_cd STRING, zip_code STRING, res_street_address STRING, res_city_state_zip STRING, mail_addr1 STRING, mail_addr2 STRING, mail_addr3 STRING, mail_addr4 STRING, mail_city STRING, mail_state STRING, mail_zipcode STRING, mail_city_state_zip STRING, area_cd STRING, phone_num STRING, full_phone_number STRING, drivers_lic STRING, race_code STRING, race_desc STRING, ethnic_code STRING, ethnic_desc STRING, party_cd STRING, party_desc STRING, sex_code STRING, sex STRING, birth_age STRING, birth_place STRING, registr_dt STRING, precinct_abbrv STRING, precinct_desc STRING,municipality_abbrv STRING, municipality_desc STRING, ward_abbrv STRING, ward_desc STRING, cong_dist_abbrv STRING, cong_dist_desc STRING, super_court_abbrv STRING, super_court_desc STRING, judic_dist_abbrv STRING, judic_dist_desc STRING, nc_senate_abbrv STRING, nc_senate_desc STRING, nc_house_abbrv STRING, nc_house_desc STRING,county_commiss_abbrv STRING, county_commiss_desc STRING, township_abbrv STRING, township_desc STRING,school_dist_abbrv STRING, school_dist_desc STRING, fire_dist_abbrv STRING, fire_dist_desc STRING, water_dist_abbrv STRING, water_dist_desc STRING, sewer_dist_abbrv STRING, sewer_dist_desc STRING, sanit_dist_abbrv STRING, sanit_dist_desc STRING, rescue_dist_abbrv STRING, rescue_dist_desc STRING, munic_dist_abbrv STRING, munic_dist_desc STRING, dist_1_abbrv STRING, dist_1_desc STRING, dist_2_abbrv STRING, dist_2_desc STRING, confidential_ind STRING, age STRING, ncid STRING, vtd_abbrv STRING, vtd_desc STRING)
CREATE TABLE ncvoters2 AS SELECT * FROM ncvoters LIMIT 0
# query
CREATE TABLE ncvoters2 AS SELECT * FROM ncvoters LIMIT 0
(SELECT * FROM ncvoters EXCEPT SELECT * FROM ncvoters2) UNION ALL (SELECT * FROM ncvoters2 EXCEPT SELECT * FROM ncvoters)
# file: test/sql/copy/csv/auto/test_auto_greek_utf8.test
# query
SELECT COUNT(*) FROM greek_utf8
# file: test/sql/copy/csv/auto/test_auto_imdb.test
# query
SELECT COUNT(*) FROM movie_info
(FROM movie_info EXCEPT FROM movie_info2) UNION ALL (FROM movie_info2 EXCEPT FROM movie_info)
# file: test/sql/copy/csv/auto/test_auto_lineitem.test
# setup
CREATE TABLE lineitem(l_orderkey INT NOT NULL, l_partkey INT NOT NULL, l_suppkey INT NOT NULL, l_linenumber INT NOT NULL, l_quantity INTEGER NOT NULL, l_extendedprice DECIMAL(15,2) NOT NULL, l_discount DECIMAL(15,2) NOT NULL, l_tax DECIMAL(15,2) NOT NULL, l_returnflag VARCHAR(1) NOT NULL, l_linestatus VARCHAR(1) NOT NULL, l_shipdate DATE NOT NULL, l_commitdate DATE NOT NULL, l_receiptdate DATE NOT NULL, l_shipinstruct VARCHAR(25) NOT NULL, l_shipmode VARCHAR(10) NOT NULL, l_comment VARCHAR(44) NOT NULL)
CREATE TABLE lineitem2 AS SELECT * FROM lineitem LIMIT 0
# query
CREATE TABLE lineitem2 AS SELECT * FROM lineitem LIMIT 0
(SELECT * FROM lineitem EXCEPT SELECT * FROM lineitem2) UNION ALL (SELECT * FROM lineitem2 EXCEPT SELECT * FROM lineitem)
# file: test/sql/copy/csv/auto/test_auto_ontime.test
# setup
CREATE TABLE ontime(year SMALLINT, quarter SMALLINT, month SMALLINT, dayofmonth SMALLINT, dayofweek SMALLINT, flightdate DATE, uniquecarrier CHAR(7), airlineid DECIMAL(8,2), carrier CHAR(2), tailnum VARCHAR(50), flightnum VARCHAR(10), originairportid INTEGER, originairportseqid INTEGER, origincitymarketid INTEGER, origin CHAR(5), origincityname VARCHAR(100), originstate CHAR(2), originstatefips VARCHAR(10), originstatename VARCHAR(100), originwac DECIMAL(8,2), destairportid INTEGER, destairportseqid INTEGER, destcitymarketid INTEGER, dest CHAR(5), destcityname VARCHAR(100), deststate CHAR(2), deststatefips VARCHAR(10), deststatename VARCHAR(100), destwac DECIMAL(8,2), crsdeptime DECIMAL(8,2), deptime DECIMAL(8,2), depdelay DECIMAL(8,2), depdelayminutes DECIMAL(8,2), depdel15 DECIMAL(8,2), departuredelaygroups DECIMAL(8,2), deptimeblk VARCHAR(20), taxiout DECIMAL(8,2), wheelsoff DECIMAL(8,2), wheelson DECIMAL(8,2), taxiin DECIMAL(8,2), crsarrtime DECIMAL(8,2), arrtime DECIMAL(8,2), arrdelay DECIMAL(8,2), arrdelayminutes DECIMAL(8,2), arrdel15 DECIMAL(8,2), arrivaldelaygroups DECIMAL(8,2), arrtimeblk VARCHAR(20), cancelled DECIMAL(8,2), cancellationcode CHAR(1), diverted DECIMAL(8,2), crselapsedtime DECIMAL(8,2), actualelapsedtime DECIMAL(8,2), airtime DECIMAL(8,2), flights DECIMAL(8,2), distance DECIMAL(8,2), distancegroup DECIMAL(8,2), carrierdelay DECIMAL(8,2), weatherdelay DECIMAL(8,2), nasdelay DECIMAL(8,2), securitydelay DECIMAL(8,2), lateaircraftdelay DECIMAL(8,2), firstdeptime VARCHAR(10), totaladdgtime VARCHAR(10), longestaddgtime VARCHAR(10), divairportlandings VARCHAR(10), divreacheddest VARCHAR(10), divactualelapsedtime VARCHAR(10), divarrdelay VARCHAR(10), divdistance VARCHAR(10), div1airport VARCHAR(10), div1aiportid INTEGER, div1airportseqid INTEGER, div1wheelson VARCHAR(10), div1totalgtime VARCHAR(10), div1longestgtime VARCHAR(10), div1wheelsoff VARCHAR(10), div1tailnum VARCHAR(10), div2airport VARCHAR(10), div2airportid INTEGER, div2airportseqid INTEGER, div2wheelson VARCHAR(10), div2totalgtime VARCHAR(10), div2longestgtime VARCHAR(10), div2wheelsoff VARCHAR(10), div2tailnum VARCHAR(10), div3airport VARCHAR(10), div3airportid INTEGER, div3airportseqid INTEGER, div3wheelson VARCHAR(10), div3totalgtime VARCHAR(10), div3longestgtime VARCHAR(10), div3wheelsoff VARCHAR(10), div3tailnum VARCHAR(10), div4airport VARCHAR(10), div4airportid INTEGER, div4airportseqid INTEGER, div4wheelson VARCHAR(10), div4totalgtime VARCHAR(10), div4longestgtime VARCHAR(10), div4wheelsoff VARCHAR(10), div4tailnum VARCHAR(10), div5airport VARCHAR(10), div5airportid INTEGER, div5airportseqid INTEGER, div5wheelson VARCHAR(10), div5totalgtime VARCHAR(10), div5longestgtime VARCHAR(10), div5wheelsoff VARCHAR(10), div5tailnum VARCHAR(10))
CREATE TABLE ontime2 AS SELECT * FROM ontime LIMIT 0
# query
CREATE TABLE ontime2 AS SELECT * FROM ontime LIMIT 0
(SELECT * FROM ontime EXCEPT SELECT * FROM ontime2) UNION ALL (SELECT * FROM ontime2 EXCEPT SELECT * FROM ontime)
# file: test/sql/copy/csv/auto/test_auto_web_page.test
# query
SELECT COUNT(*) FROM web_page
SELECT * FROM web_page ORDER BY column00 LIMIT 3
(SELECT * FROM web_page EXCEPT SELECT * FROM web_page2) UNION ALL (SELECT * FROM web_page2 EXCEPT SELECT * FROM web_page)
# file: test/sql/copy/csv/auto/test_csv_auto.test
# query
SELECT * FROM test ORDER BY column0
SELECT a, b FROM test
# file: test/sql/copy/csv/auto/test_describe_order.test
# query
describe v
describe select * from v
# file: test/sql/copy/csv/auto/test_header_completion.test
# query
SELECT a, column1, c FROM test ORDER BY a
SELECT a, b, a_1 FROM test ORDER BY a
SELECT a, b, a_1, a_1_1 FROM test ORDER BY a
SELECT column0, column1, column2 FROM test ORDER BY column0
SELECT a, column01, column12 FROM test
SELECT a, a_8, a_9, column12 FROM test
SELECT a, a_8, a_9, column12, column11, column12_1 FROM test
SELECT column00, column01, column02, column03, column04, column05, column06, column07, column08, column09, column10, column11, column12 FROM test
# file: test/sql/copy/csv/auto/test_header_detection.test
# setup
CREATE TABLE my_varchars(a VARCHAR, b VARCHAR, c VARCHAR)
# query
SELECT number, text, date FROM test ORDER BY number
SELECT column0, column1 FROM test ORDER BY column0
SELECT id FROM test
CREATE TABLE my_varchars(a VARCHAR, b VARCHAR, c VARCHAR)
INSERT INTO my_varchars VALUES ('Hello', 'Beautiful', 'World')
FROM my_varchars
# file: test/sql/copy/csv/auto/test_normalize_names.test
# query
SELECT a, b, c FROM test ORDER BY a
SELECT A, B, C FROM test ORDER BY a
SELECT _select, _insert, _join FROM test ORDER BY _select
SELECT _0_a, _1_b, _9_c FROM test ORDER BY _0_a
SELECT allo, teost, _ FROM test ORDER BY allo
SELECT aax, hello_world, qty_m2 FROM test ORDER BY aax
# file: test/sql/copy/csv/auto/test_sample_size.test
# setup
CREATE TABLE test (TestInteger integer, TestDouble double, TestDate varchar, TestText varchar)
# query
SELECT typeof(TestInteger), typeof(TestDouble), typeof(TestDate), typeof(TestText) FROM test LIMIT 1
SELECT TestInteger, TestDouble, TestDate, TestText FROM test WHERE TestDouble is not NULL
CREATE TABLE test (TestInteger integer, TestDouble double, TestDate varchar, TestText varchar)
# file: test/sql/copy/csv/auto/test_sniffer_blob.test
# setup
create table t ( a blob)
# query
create table t ( a blob)
# file: test/sql/copy/csv/auto/test_timings_csv.test
# setup
CREATE OR REPLACE TABLE timings(tool string, sf float, day string, batch_type string, q string, parameters string, time float)
# query
CREATE OR REPLACE TABLE timings(tool string, sf float, day string, batch_type string, q string, parameters string, time float)
# file: test/sql/copy/csv/auto/test_type_candidates.test
# setup
create table t (a integer, b double, c varchar)
# query
create table t (a integer, b double, c varchar)
insert into t values (1,1.1,'bla')
# file: test/sql/copy/csv/auto/test_type_detection.test
# query
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
# file: test/sql/copy/csv/afl/fuzz_20250226.test
# query
select count(file) from glob('./data/csv/afl/20250226_csv_fuzz_error/*')
# file: test/sql/copy/csv/afl/test_fuzz_3977.test
# query
select count(file) from glob('./data/csv/afl/3977/*')
# file: test/sql/copy/csv/code_cov/buffer_manager_finalize.test
# setup
CREATE TABLE t1 AS select i, (i+1) as j from range(0,3000) tbl(i)
# query
CREATE TABLE t1 AS select i, (i+1) as j from range(0,3000) tbl(i)
# file: test/sql/copy/csv/code_cov/csv_sniffer_header.test
# query
describe t
# file: test/sql/copy/csv/unquoted_escape/basic.test
# setup
CREATE TABLE special_char(a INT, b STRING)
# query
CREATE TABLE special_char(a INT, b STRING)
INSERT INTO special_char VALUES (0, E'\\'), (1, E'\t'), (2, E'\n'), (3, E'a\\a'), (4, E'b\tb'), (5, E'c\nc'), (6, E'\\d'), (7, E'\te'), (8, E'\nf'), (9, E'g\\'), (10, E'h\t'), (11, E'i\n'), (12, E'\\j'), (13, E'\tk'), (14, E'\nl'), (15, E'\\\\'), (16, E'\t\t'), (17, E'\n\n'), (18, E'\\\t\n')
# file: test/sql/copy/csv/unquoted_escape/human_eval.test
# setup
CREATE TABLE human_eval_csv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
CREATE TABLE human_eval_tsv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
# query
CREATE TABLE human_eval_jsonl AS SELECT REPLACE(COLUMNS(*), ' ', E'\t') FROM read_ndjson_auto( 'https://raw.githubusercontent.com/openai/human-eval/refs/heads/master/data/HumanEval.jsonl.gz')
DELETE FROM human_eval_jsonl WHERE split_part(task_id, '/', 2)::int >= 10
CREATE TABLE human_eval_csv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
CREATE TABLE human_eval_tsv(task_id TEXT, prompt TEXT, entry_point TEXT, canonical_solution TEXT, test TEXT)
TRUNCATE human_eval_csv
TRUNCATE human_eval_tsv
INSERT INTO human_eval_csv SELECT replace(COLUMNS(*), E'\r\n', E'\n') FROM read_csv('data/csv/unquoted_escape/human_eval.csv', quote = '', escape = '\', sep = ',', header = false, strict_mode = false)
INSERT INTO human_eval_tsv SELECT replace(COLUMNS(*), E'\r\n', E'\n') FROM read_csv('data/csv/unquoted_escape/human_eval.tsv', quote = '', escape = '\', sep = '\t', header = false, strict_mode = false)
# file: test/sql/copy/csv/glob/copy_csv_glob.test
# setup
CREATE TABLE dates(d DATE)
# query
CREATE TABLE dates(d DATE)
# file: test/sql/copy/csv/glob/read_csv_glob.test
# query
select count(*) from glob('/rewoiarwiouw3rajkawrasdf790273489*.csv') limit 10
select count(*) from glob('~/rewoiarwiouw3rajkawrasdf790273489*.py') limit 10
SELECT COUNT(*) FROM glob('*/*.csv')
SELECT COUNT(*) FROM glob('*.csv')
SELECT COUNT(*) FROM glob('csv/glob/*/*.csv')
set file_search_path=''
# reject
SELECT * FROM read_csv_auto([]) ORDER BY 1
SELECT * FROM read_csv_auto([]::VARCHAR[]) ORDER BY 1
SELECT * FROM read_csv_auto(NULL) ORDER BY 1
SELECT * FROM read_csv_auto([NULL]) ORDER BY 1
SELECT * FROM read_csv_auto(NULL::VARCHAR) ORDER BY 1
SELECT * FROM read_csv_auto(NULL::VARCHAR[]) ORDER BY 1
# file: test/sql/copy/partitioned/hive_partition_append.test
# setup
CREATE TABLE sensor_data(ts TIMESTAMP, value INT)
# query
CREATE TABLE sensor_data(ts TIMESTAMP, value INT)
INSERT INTO sensor_data VALUES (TIMESTAMP '2000-01-01 01:02:03', 42), (TIMESTAMP '2000-02-01 01:02:03', 100), (TIMESTAMP '2000-03-01 12:11:10', 1000)
DELETE FROM sensor_data
INSERT INTO sensor_data VALUES (TIMESTAMP '2000-01-01 02:02:03', 62), (TIMESTAMP '2000-03-01 13:11:10', 50)
# file: test/sql/copy/partitioned/hive_partition_escape.test
# setup
CREATE SEQUENCE seq
CREATE TABLE weird_tbl(id INT DEFAULT nextval('seq'), key VARCHAR)
# query
CREATE TABLE weird_tbl(id INT DEFAULT nextval('seq'), key VARCHAR)
INSERT INTO weird_tbl (key) VALUES ('/'), ('\/\/'), ('==='), ('value with strings'), ('?:&'), ('🦆'), ('==='), ('===')
ALTER TABLE weird_tbl RENAME COLUMN key TO "=/ \\/"
# file: test/sql/copy/partitioned/hive_partition_join_pushdown.test
# setup
CREATE TABLE tbl AS SELECT i//1000 AS partition, i FROM range(10000) t(i)
CREATE TABLE tbl2 AS SELECT (date '2000-01-01' + interval (i//2000) years)::DATE AS part1, i%2 AS part2, i FROM range(10000) t(i)
# query
CREATE TABLE tbl AS SELECT i//1000 AS partition, i FROM range(10000) t(i)
CREATE TABLE tbl2 AS SELECT (date '2000-01-01' + interval (i//2000) years)::DATE AS part1, i%2 AS part2, i FROM range(10000) t(i)
# file: test/sql/copy/partitioned/hive_partition_recursive_cte.test
# setup
CREATE TABLE t AS SELECT 2000+i%10 AS year, 1+i%3 AS month, i%4 AS c, i%5 AS d FROM RANGE(0,20) tbl(i)
# query
CREATE TABLE t AS SELECT 2000+i%10 AS year, 1+i%3 AS month, i%4 AS c, i%5 AS d FROM RANGE(0,20) tbl(i)
WITH RECURSIVE cte AS ( SELECT 0 AS count, 1999 AS selected_year UNION ALL SELECT COUNT(*) AS count, MAX(partitioned_tbl.year) FROM partitioned_tbl, (SELECT MAX(selected_year) AS next_year FROM cte) WHERE partitioned_tbl.year = (SELECT MAX(selected_year) + 1 FROM cte) HAVING COUNT(*)>0 ) SELECT SUM(count), MIN(selected_year), MAX(selected_year) FROM cte WHERE count>0
# file: test/sql/copy/partitioned/hive_partitioned_write.test
# setup
CREATE TABLE test as SELECT i%2 as part_col, (i+1)%5 as value_col, i as value2_col from range(0,10) tbl(i)
# query
CREATE TABLE test as SELECT i%2 as part_col, (i+1)%5 as value_col, i as value2_col from range(0,10) tbl(i)
# file: test/sql/copy/partitioned/partitioned_group_by.test
# setup
CREATE TABLE partitioned_tbl AS SELECT i%2 AS partition, i col1, i // 7 col2, (i%3)::VARCHAR col3 FROM range(10000) t(i)
CREATE TABLE partitioned_tbl2 AS SELECT i%2 AS partition1, i%3 AS partition2, i col1, i + 1 col2 FROM range(10000) t(i)
# query
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
# file: test/sql/copy/parquet/attach_parquet.test
# query
USE attached_parquet
SELECT * FROM file
SELECT * FROM attached_parquet
# file: test/sql/copy/parquet/bloom_filters.test
# setup
CREATE MACRO assert_bloom_filter_hit(file, col, val) AS TABLE SELECT COUNT(*) > 0 AND COUNT(*) < MAX(row_group_id+1) FROM parquet_bloom_probe(file, col, val) WHERE NOT bloom_filter_excludes
# query
CREATE MACRO assert_bloom_filter_hit(file, col, val) AS TABLE SELECT COUNT(*) > 0 AND COUNT(*) < MAX(row_group_id+1) FROM parquet_bloom_probe(file, col, val) WHERE NOT bloom_filter_excludes
# file: test/sql/copy/parquet/copy_option_non_foldable.test
# query
EXECUTE statement2
# file: test/sql/copy/parquet/copy_option_prepared.test
# query
execute statement(42)
# file: test/sql/copy/parquet/corrupt_stats.test
# query
PRAGMA disable_optimizer
# file: test/sql/copy/parquet/dictionary_compression_ratio_threshold.test
# setup
CREATE OR REPLACE TABLE test AS SELECT 'coolstring' || range i FROM range(100000)
# query
CREATE TABLE test AS SELECT 'thisisaverylongstringbutitrepeatsmanytimessoitshighlycompressible' || (range % 10) i FROM range(100000)
CREATE OR REPLACE TABLE test AS SELECT 'coolstring' || range i FROM range(100000)
# file: test/sql/copy/parquet/file_metadata.test
# query
SET parquet_metadata_cache = true
SELECT unnest(parquet_file_metadata, recursive:=True) FROM parquet_full_metadata('data/parquet-testing/arrow/column_orders.parquet')
# file: test/sql/copy/parquet/hive_timestamps.test
# setup
CREATE TABLE raw_data ( ts TIMESTAMP_S NOT NULL, hits INTEGER NOT NULL )
CREATE TABLE timeseries AS ( SELECT DATE_TRUNC('hour', ts) AS bucket, SUM(hits)::BIGINT AS total FROM raw_data GROUP BY bucket )
# query
CREATE TABLE raw_data ( ts TIMESTAMP_S NOT NULL, hits INTEGER NOT NULL )
INSERT INTO raw_data SELECT *, (random() * 500)::INTEGER FROM RANGE(TIMESTAMP '2023-11-01', TIMESTAMP '2023-11-06', INTERVAL 1 MINUTE)
CREATE TABLE timeseries AS ( SELECT DATE_TRUNC('hour', ts) AS bucket, SUM(hits)::BIGINT AS total FROM raw_data GROUP BY bucket )
SELECT * FROM timeseries ORDER BY ALL LIMIT 5
# file: test/sql/copy/parquet/infer_copy_format.test
# setup
CREATE TABLE integers AS SELECT * FROM range(6) tbl(i)
# query
CREATE TABLE integers AS SELECT * FROM range(6) tbl(i)
# file: test/sql/copy/parquet/invalid_utf8_stats.test
# query
FROM 'data/parquet-testing/invalid_utf8_stats.parquet'
# file: test/sql/copy/parquet/json_parquet.test
# query
SELECT json_extract(TX_JSON[1], 'block_hash') FROM json_tbl
# file: test/sql/copy/parquet/lineitem_arrow.test
# query
PRAGMA tpch(1)
PRAGMA tpch(6)
# file: test/sql/copy/parquet/metadata_full.test
# query
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
# file: test/sql/copy/parquet/multi_file_conversion_error.test
# setup
CREATE TABLE integers(i INT)
# query
CREATE TABLE integers(i INT)
# file: test/sql/copy/parquet/parallel_parquet_glob.test
# query
SET parquet_metadata_cache=true
# file: test/sql/copy/parquet/parquet_1588.test
# setup
create table some_bools (val boolean)
# query
create table some_bools (val boolean)
insert into some_bools values (TRUE)
select count(*) from some_bools where val = 1
select count(*) from some_bools where val = '1'::bool
# file: test/sql/copy/parquet/parquet_3896.test
# setup
CREATE VIEW v1 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x
CREATE VIEW v2 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x UNION ALL SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}])
CREATE VIEW v3 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':2}]} AS x
CREATE VIEW v4 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':[2]}]} AS x
# query
CREATE VIEW v1 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x
CREATE VIEW v2 AS SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}]) AS x UNION ALL SELECT map([2], [{'key1': map([3,4],[1,2]), 'key2':2}])
SELECT * FROM v2
CREATE VIEW v3 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':2}]} AS x
SELECT * FROM v3
CREATE VIEW v4 AS SELECT {'key': [2], 'val': [{'key1': {'key': [3,4], 'val': [1,2]}, 'key2':[2]}]} AS x
SELECT * FROM v4
# file: test/sql/copy/parquet/parquet_3989.test
# setup
CREATE TABLE lists as SELECT i as id, [i] as list from range(0,10000) tbl(i)
# query
CREATE TABLE lists as SELECT i as id, [i] as list from range(0,10000) tbl(i)
# file: test/sql/copy/parquet/parquet_5209.test
# setup
CREATE TABLE test_5209 AS SELECT range FROM range(10000)
# query
CREATE TABLE test_5209 AS SELECT range FROM range(10000)
# file: test/sql/copy/parquet/parquet_6933.test
# setup
CREATE TABLE table1 ( name VARCHAR, )
CREATE TABLE table2 ( name VARCHAR, number INTEGER, )
# query
CREATE TABLE table1 ( name VARCHAR, )
INSERT INTO table1 VALUES ('Test value 1!')
INSERT INTO table1 VALUES ('Test value 2!')
CREATE TABLE table2 ( name VARCHAR, number INTEGER, )
INSERT INTO table2 VALUES ('Other test value', 1)
INSERT INTO table2 VALUES ('Other test value', 2)
set parquet_metadata_cache=true
# file: test/sql/copy/parquet/parquet_blob_string.test
# query
SET binary_as_string=true
SET binary_as_string=false
PRAGMA binary_as_string=1
# reject
SET binary_as_sting=true
# file: test/sql/copy/parquet/parquet_copy_type_mismatch.test
# setup
CREATE TABLE integers(i INTEGER)
# query
SET storage_compatibility_version='v1.1.0'
# file: test/sql/copy/parquet/parquet_encryption.test
# setup
CREATE OR REPLACE TABLE test (i INTEGER)
# query
PRAGMA add_parquet_key('key128', '0123456789112345')
PRAGMA add_parquet_key('key192', '012345678911234501234567')
PRAGMA add_parquet_key('key256', '01234567891123450123456789112345')
CREATE OR REPLACE TABLE test (i INTEGER)
PRAGMA add_parquet_key('key256base64', 'MDEyMzQ1Njc4OTExMjM0NTAxMjM0NTY3ODkxMTIzNDU=')
# reject
PRAGMA add_parquet_key('my_cool_key', '42')
PRAGMA add_parquet_key('my_invalid_duck_key', 'ZHVjaw==')
# file: test/sql/copy/parquet/parquet_expression_filter.test
# setup
CREATE TABLE tbl AS SELECT i, 'thisisalongstring'||(i%5000)::VARCHAR AS str FROM range(100000) t(i)
# query
CREATE TABLE tbl AS SELECT i, 'thisisalongstring'||(i%5000)::VARCHAR AS str FROM range(100000) t(i)
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str AND str >= 'this'
SELECT COUNT(*) FROM parq WHERE least(str, 'thisisalongstring50') = str AND str >= 'thisisalongstring2000' AND str <= 'thisisalongstring4000'
# file: test/sql/copy/parquet/parquet_filename.test
# setup
CREATE TABLE test_csv AS SELECT 1 as id, 'test_csv_content' as filename
CREATE TABLE test AS SELECT 1 as id, 'test' as filename
CREATE TABLE test_copy (i INT, j VARCHAR, filename VARCHAR)
CREATE TABLE test_table_large AS SELECT * FROM range(0,10000) tbl(i)
# query
CREATE TABLE test_csv AS SELECT 1 as id, 'test_csv_content' as filename
CREATE TABLE test AS SELECT 1 as id, 'test' as filename
CREATE TABLE test_copy (i INT, j VARCHAR, filename VARCHAR)
SELECT i, j, parse_path(filename)[-2:] FROM test_copy
CREATE TABLE test_table_large AS SELECT * FROM range(0,10000) tbl(i)
# file: test/sql/copy/parquet/parquet_filter_bug1391.test
# query
SELECT ORGUNITID FROM tbl LIMIT 10
SELECT COUNT(*) FROM tbl WHERE Namevalidfrom <= '2017-03-01' AND Namevalidto >= '2017-03-01' AND Parentnamevalidfrom <= '2017-03-01' AND Parentnamevalidto >= '2017-03-01' AND CustomerCode = 'CODE'
# file: test/sql/copy/parquet/parquet_hive.test
# setup
Create table t1 (a int, b int, c int)
# query
Create table t1 (a int, b int, c int)
# file: test/sql/copy/parquet/parquet_hive2.test
# setup
create or replace table orders(m int,v int,j int)
# query
create or replace table orders(m int,v int,j int)
insert into orders select i%12+1,i,j from range(360)t(i),range(1000)s(j)
# file: test/sql/copy/parquet/parquet_hive_null.test
# setup
create table test as select i%5 as a, i%2 as b from range(0,10) tbl(i)
create table test2 as select i%5 as a, i%2 as b, i as c from range(0,10) tbl(i)
create table test_null_write as select 1 as c, NULL::INT as a, NULL::INT as b
# query
create table test as select i%5 as a, i%2 as b from range(0,10) tbl(i)
create table test2 as select i%5 as a, i%2 as b, i as c from range(0,10) tbl(i)
create table test_null_write as select 1 as c, NULL::INT as a, NULL::INT as b
# file: test/sql/copy/parquet/parquet_late_materialization.test
# query
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
# file: test/sql/copy/parquet/parquet_metadata.test
# query
SELECT column_id, name FROM parquet_schema('data/parquet-testing/lineitem-top10000.gzip.parquet') ORDER BY column_id
WITH per_file AS ( SELECT file_name, COUNT(*) AS rows_per_file FROM parquet_schema('data/parquet-testing/glob3/**/*.parquet') GROUP BY file_name ) SELECT SUM(rows_per_file) AS total_rows, MAX(rows_per_file) AS max_rows_per_filename, (SELECT COUNT(DISTINCT column_id) FROM parquet_schema('data/parquet-testing/glob3/**/*.parquet')) AS distinct_column_ids FROM per_file
# file: test/sql/copy/parquet/parquet_row_number.test
# query
PRAGMA explain_output = OPTIMIZED_ONLY
# file: test/sql/copy/parquet/parquet_schema_evolution.test
# setup
CREATE TABLE copy_test(a INT, b INT)
# query
CREATE TABLE copy_test(a INT, b INT)
DROP TABLE copy_test
# file: test/sql/copy/parquet/parquet_schema_num_children_fix.test
# setup
CREATE TABLE test_nested AS SELECT 1 as id, {'a': {'b': {'c': 123}}} as deep_nested, {'x': 1, 'y': 2} as simple_struct
CREATE TABLE test_lists AS SELECT [1, 2, 3] as simple_list, [{'x': 1}, {'x': 2}] as list_of_structs, [[1, 2], [3, 4]] as nested_list
CREATE TABLE test_maps AS SELECT MAP {'a': 1, 'b': 2} as simple_map, MAP {'nested': {'inner': 123}} as map_of_struct
CREATE TABLE test_nullable AS SELECT {'a': NULL, 'b': 2} as partial_null, NULL::STRUCT(x INT, y INT) as full_null
# query
CREATE TABLE test_nested AS SELECT 1 as id, {'a': {'b': {'c': 123}}} as deep_nested, {'x': 1, 'y': 2} as simple_struct
CREATE TABLE test_lists AS SELECT [1, 2, 3] as simple_list, [{'x': 1}, {'x': 2}] as list_of_structs, [[1, 2], [3, 4]] as nested_list
CREATE TABLE test_maps AS SELECT MAP {'a': 1, 'b': 2} as simple_map, MAP {'nested': {'inner': 123}} as map_of_struct
CREATE TABLE test_nullable AS SELECT {'a': NULL, 'b': 2} as partial_null, NULL::STRUCT(x INT, y INT) as full_null
# file: test/sql/copy/parquet/parquet_union_by_name.test
# setup
CREATE OR REPLACE TABLE ubn1(a BIGINT)
CREATE OR REPLACE TABLE ubn2(a INTEGER, b INTEGER)
CREATE OR REPLACE TABLE ubn3(a INTEGER, c INTEGER)
# query
CREATE OR REPLACE TABLE ubn1(a BIGINT)
CREATE OR REPLACE TABLE ubn2(a INTEGER, b INTEGER)
CREATE OR REPLACE TABLE ubn3(a INTEGER, c INTEGER)
# file: test/sql/copy/parquet/partition_by_bind_issues.test
# setup
CREATE TABLE test AS SELECT 'test' AS user, '2025' AS year
# query
CREATE TABLE test AS SELECT 'test' AS user, '2025' AS year
# file: test/sql/copy/parquet/read_parquet_parameter.test
# query
PREPARE v1 AS SELECT * FROM parquet_scan($1) ORDER BY 1
# file: test/sql/copy/parquet/recursive_parquet_union_by_name.test
# query
WITH RECURSIVE t(it, accum) AS ( SELECT 1, 0 UNION ALL ( SELECT it + 1, accum + j FROM t, r WHERE it <= x ) ) SELECT * FROM t ORDER BY it, accum
# file: test/sql/copy/parquet/test_parquet_scan.test
# query
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
# reject
SELECT * FROM parquet_scan('does_not_exist')
# file: test/sql/copy/parquet/test_parquet_stats.test
# query
PRAGMA explain_output = PHYSICAL_ONLY
pragma disable_object_cache
# file: test/sql/copy/parquet/timestamp_s.test
# setup
create table t (ts TIMESTAMP_S)
# query
create table t (ts TIMESTAMP_S)
insert into t select make_timestamp((1706961600 + (360 * i))::BIGINT * 1000000) from range(10000) range(i)
select * from t limit 3
# file: test/sql/copy/parquet/timezone.test
# query
SET timezone='UTC'
# file: test/sql/copy/parquet/union_by_name_hive_partitioning.test
# setup
CREATE TABLE selected_values AS SELECT 2 x
# query
CREATE TABLE selected_values AS SELECT 2 x
# file: test/sql/copy/parquet/encryption/arrow_compatibility.test
# query
PRAGMA add_parquet_key('arrow_key', '0123456789012345')
PRAGMA add_parquet_key('arrow_key_generated_files', '0123456789abcdef')
# file: test/sql/copy/parquet/multi_file/multi_file_filter_integer_types.test
# query
SELECT f, i FROM integer_file_first WHERE i='042'
SELECT f, i FROM bigint_file_first WHERE i='042' ORDER BY ALL
SELECT f, i FROM integer_file_first WHERE i>10 ORDER BY ALL
SELECT f, i FROM bigint_file_first WHERE i>'10' ORDER BY ALL
SELECT f, i FROM integer_file_first WHERE i IS NULL
# file: test/sql/copy/parquet/multi_file/multi_file_filter_mixed.test
# query
SELECT f, i FROM string_file_first WHERE i='042'
SELECT f, i FROM string_file_first WHERE i>'10' ORDER BY ALL
# file: test/sql/copy/parquet/multi_file/multi_file_filter_struct.test
# query
SELECT struct_val.i FROM integer_file_first ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i='042'
SELECT struct_val.i FROM bigint_file_first WHERE struct_val.i='042' ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i>10 ORDER BY ALL
SELECT struct_val.i FROM bigint_file_first WHERE struct_val.i>'10' ORDER BY ALL
SELECT struct_val.f, struct_val.i FROM integer_file_first WHERE struct_val.i IS NULL
# file: test/sql/copy/parquet/writer/parquet_test_all_types.test
# setup
CREATE TABLE all_types AS SELECT * EXCLUDE (bit, "union") REPLACE ( case when extract(month from interval) <> 0 then interval '1 month 1 day 12:13:34.123' else interval end AS interval ) FROM test_all_types()
# query
CREATE TABLE all_types AS SELECT * EXCLUDE (bit, "union") REPLACE ( case when extract(month from interval) <> 0 then interval '1 month 1 day 12:13:34.123' else interval end AS interval ) FROM test_all_types()
SELECT * REPLACE ( hugeint::DOUBLE AS hugeint, uhugeint::DOUBLE AS uhugeint, time_tz::TIME::TIMETZ AS time_tz ) FROM all_types
# file: test/sql/copy/parquet/writer/parquet_write_booleans.test
# setup
CREATE TABLE bools(b BOOL)
# query
CREATE TABLE bools(b BOOL)
INSERT INTO bools SELECT CASE WHEN i%2=0 THEN NULL ELSE i%7=0 OR i%3=0 END b FROM range(10000) tbl(i)
SELECT COUNT(*), COUNT(b), BOOL_AND(b), BOOL_OR(b), SUM(CASE WHEN b THEN 1 ELSE 0 END) true_count, SUM(CASE WHEN b THEN 0 ELSE 1 END) false_count FROM bools
# file: test/sql/copy/parquet/writer/parquet_write_compression_level.test
# setup
CREATE TABLE integers AS FROM range(100) t(i)
# query
CREATE TABLE integers AS FROM range(100) t(i)
# file: test/sql/copy/parquet/writer/parquet_write_date.test
# setup
CREATE TABLE dates(d DATE)
# query
INSERT INTO dates VALUES (DATE '1992-01-01'), (DATE '1900-01-01'), (NULL), (DATE '2020-09-27')
# file: test/sql/copy/parquet/writer/parquet_write_decimals.test
# setup
CREATE TABLE decimals( dec4 DECIMAL(4,1), dec9 DECIMAL(9,2), dec18 DECIMAL(18,3), dec38 DECIMAL(38,4) )
# query
CREATE TABLE decimals( dec4 DECIMAL(4,1), dec9 DECIMAL(9,2), dec18 DECIMAL(18,3), dec38 DECIMAL(38,4) )
INSERT INTO decimals VALUES ( -999.9, -9999999.99, -999999999999999.999, -999999999999999999999999999999999.9999 ), ( NULL, NULL, NULL, NULL ), ( 42, 42, 42, 42 ), ( -42, -42, -42, -42 ), ( 0, 0, 0, 0 ), ( 999.9, 9999999.99, 999999999999999.999, 999999999999999999999999999999999.9999 )
SELECT * FROM decimals
DELETE FROM decimals WHERE dec4<-42 OR dec4>42
# file: test/sql/copy/parquet/writer/parquet_write_enums.test
# setup
CREATE TYPE mood AS ENUM ('joy', 'ok', 'happy')
CREATE TABLE enums(m mood)
CREATE TABLE t AS SELECT 'joy'::mood AS m FROM range(10) t(i)
# query
CREATE TYPE mood AS ENUM ('joy', 'ok', 'happy')
CREATE TABLE enums(m mood)
INSERT INTO enums VALUES ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('joy')
UPDATE enums SET m=NULL WHERE m='joy'
UPDATE enums SET m=NULL
CREATE TABLE t AS SELECT 'joy'::mood AS m FROM range(10) t(i)
# file: test/sql/copy/parquet/writer/parquet_write_field_id.test
# query
set variable field_id_values={i:{__duckdb_field_id:42,key:43,value:{__duckdb_field_id:44,element:{__duckdb_field_id:45,j:46}}}}
# file: test/sql/copy/parquet/writer/parquet_write_home_directory.test
# setup
CREATE TABLE integers AS SELECT * FROM range(10)
CREATE TABLE integers_load(i INTEGER)
# query
SELECT * FROM '~/integers.parquet'
COPY integers_load FROM '~/integers.parquet'
SELECT COUNT(*) FROM '~/homedir_integers*.parquet'
# file: test/sql/copy/parquet/writer/parquet_write_hugeint.test
# setup
CREATE TABLE hugeints(h HUGEINT)
# query
CREATE TABLE hugeints(h HUGEINT)
INSERT INTO hugeints VALUES (-1180591620717411303424), (0), (NULL), (1180591620717411303424)
# file: test/sql/copy/parquet/writer/parquet_write_interval.test
# setup
CREATE TABLE IF NOT EXISTS intervals (i interval)
# query
CREATE TABLE IF NOT EXISTS intervals (i interval)
INSERT INTO intervals VALUES (interval '1' day), (interval '00:00:01'), (NULL), (interval '0' month), (interval '1' month)
# file: test/sql/copy/parquet/writer/parquet_write_issue_5779.test
# setup
CREATE TABLE empty_lists(i INTEGER[])
CREATE TABLE empty_lists_varchar(i VARCHAR[])
CREATE TABLE empty_list_nested(i INT[][])
# query
CREATE TABLE empty_lists(i INTEGER[])
INSERT INTO empty_lists SELECT [] FROM range(10) UNION ALL SELECT [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
CREATE TABLE empty_lists_varchar(i VARCHAR[])
INSERT INTO empty_lists_varchar SELECT [] FROM range(10) UNION ALL SELECT ['hello', 'world', 'this', 'is', 'a', 'varchar', 'list']
CREATE TABLE empty_list_nested(i INT[][])
INSERT INTO empty_list_nested SELECT [] FROM range(10) UNION ALL SELECT [[1, 2, 3], [4, 5], [6, 7, 8]]
# file: test/sql/copy/parquet/writer/parquet_write_memory_usage.test
# query
set memory_limit='4gb'
# file: test/sql/copy/parquet/writer/parquet_write_signed.test
# setup
CREATE TABLE values_TINYINT AS SELECT d::TINYINT d FROM (VALUES (-128), (42), (NULL), (127)) tbl (d)
CREATE TABLE values_SMALLINT AS SELECT d::SMALLINT d FROM (VALUES (-32768), (42), (NULL), (32767)) tbl (d)
CREATE TABLE values_INTEGER AS SELECT d::INTEGER d FROM (VALUES (-2147483648), (42), (NULL), (2147483647)) tbl (d)
CREATE TABLE values_BIGINT AS SELECT d::BIGINT d FROM (VALUES (-9223372036854775808), (42), (NULL), (9223372036854775807)) tbl (d)
# query
CREATE TABLE values_TINYINT AS SELECT d::TINYINT d FROM (VALUES (-128), (42), (NULL), (127)) tbl (d)
CREATE TABLE values_SMALLINT AS SELECT d::SMALLINT d FROM (VALUES (-32768), (42), (NULL), (32767)) tbl (d)
CREATE TABLE values_INTEGER AS SELECT d::INTEGER d FROM (VALUES (-2147483648), (42), (NULL), (2147483647)) tbl (d)
CREATE TABLE values_BIGINT AS SELECT d::BIGINT d FROM (VALUES (-9223372036854775808), (42), (NULL), (9223372036854775807)) tbl (d)
# file: test/sql/copy/parquet/writer/parquet_write_strings.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
CREATE TABLE strings(s VARCHAR)
INSERT INTO strings VALUES ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('happy'), ('happy'), ('joy'), ('joy'), ('surprise')
UPDATE strings SET s=NULL WHERE s='joy'
UPDATE strings SET s=NULL
DELETE FROM strings
INSERT INTO strings VALUES ('0'), ('1'), ('2'), ('3'), ('4'), ('5'), ('6'), ('7'), ('8'), ('9'), ('10'), ('11'), ('12'), ('13'), ('14'), ('15'), ('16'), ('17'), ('18'), ('19'), ('20'), ('21'), ('22'), ('23'), ('24'), ('25'), ('26'), ('27'), ('28'), ('29')
INSERT INTO strings VALUES ('0'), ('1'), ('2'), (NULL), ('4'), ('5'), ('6'), (NULL), ('8'), ('9'), ('10'), ('11'), ('12'), ('13'), ('14'), ('15'), ('16'), ('17'), ('18'), ('19'), ('20'), (NULL), ('22'), ('23'), ('24'), ('25'), (NULL), ('27'), ('28'), ('29')
# file: test/sql/copy/parquet/writer/parquet_write_timestamp.test
# setup
CREATE OR REPLACE TABLE timestamps(d TIMESTAMP_NS)
# query
INSERT INTO timestamps VALUES (TIMESTAMP '1992-01-01 12:03:27'), (TIMESTAMP '1900-01-01 03:08:47'), (NULL), (TIMESTAMP '2020-09-27 13:12:01')
CREATE OR REPLACE TABLE timestamps(d TIMESTAMP_NS)
INSERT INTO timestamps VALUES ('1992-01-01 12:03:27.123456789'), ('1900-01-01 03:08:47.987654321'), (NULL), ('2020-09-27 13:12:01')
# file: test/sql/copy/parquet/writer/parquet_write_uhugeint.test
# setup
CREATE TABLE hugeints(h UHUGEINT)
# query
CREATE TABLE hugeints(h UHUGEINT)
INSERT INTO hugeints VALUES (0), (1), (NULL), (1180591620717411303424)
# file: test/sql/copy/parquet/writer/parquet_write_unsigned.test
# setup
CREATE TABLE values_UTINYINT AS SELECT d::UTINYINT d FROM (VALUES (0), (42), (NULL), (255)) tbl (d)
CREATE TABLE values_USMALLINT AS SELECT d::USMALLINT d FROM (VALUES (0), (42), (NULL), (65535)) tbl (d)
CREATE TABLE values_UINTEGER AS SELECT d::UINTEGER d FROM (VALUES (0), (42), (NULL), (4294967295)) tbl (d)
CREATE TABLE values_UBIGINT AS SELECT d::UBIGINT d FROM (VALUES (0), (42), (NULL), (18446744073709551615)) tbl (d)
# query
CREATE TABLE values_UTINYINT AS SELECT d::UTINYINT d FROM (VALUES (0), (42), (NULL), (255)) tbl (d)
CREATE TABLE values_USMALLINT AS SELECT d::USMALLINT d FROM (VALUES (0), (42), (NULL), (65535)) tbl (d)
CREATE TABLE values_UINTEGER AS SELECT d::UINTEGER d FROM (VALUES (0), (42), (NULL), (4294967295)) tbl (d)
CREATE TABLE values_UBIGINT AS SELECT d::UBIGINT d FROM (VALUES (0), (42), (NULL), (18446744073709551615)) tbl (d)
# file: test/sql/copy/parquet/writer/parquet_write_uuid.test
# setup
CREATE TABLE IF NOT EXISTS uuid (u uuid)
CREATE TABLE uuid2 AS SELECT uuid '47183823-2574-4bfd-b411-99ed177d3e43' uuid_val union all select uuid '00112233-4455-6677-8899-aabbccddeeff'
# query
CREATE TABLE IF NOT EXISTS uuid (u uuid)
CREATE TABLE uuid2 AS SELECT uuid '47183823-2574-4bfd-b411-99ed177d3e43' uuid_val union all select uuid '00112233-4455-6677-8899-aabbccddeeff'
# file: test/sql/copy/parquet/writer/partition_without_hive.test
# setup
CREATE TABLE t1(part_key INT, val INT)
# query
CREATE TABLE t1(part_key INT, val INT)
INSERT INTO t1 SELECT i%2, i FROM range(10) t(i)
# file: test/sql/copy/parquet/writer/skip_empty_write.test
# setup
CREATE TABLE empty_tbl(i INT, j VARCHAR)
CREATE TABLE tbl AS FROM range(10000) t(i) UNION ALL SELECT 100000
# query
CREATE TABLE empty_tbl(i INT, j VARCHAR)
CREATE TABLE tbl AS FROM range(10000) t(i) UNION ALL SELECT 100000
# file: test/sql/copy/parquet/writer/test_parquet_write.test
# setup
CREATE TABLE empty(i INTEGER)
# query
CREATE TABLE empty(i INTEGER)
# file: test/sql/copy/parquet/writer/write_complex_nested.test
# setup
CREATE TABLE struct_of_lists AS SELECT * FROM (VALUES ({'a': [1, 2, 3], 'b': ['hello', 'world']}), ({'a': [4, NULL, 5], 'b': ['duckduck', 'goose']}), ({'a': NULL, 'b': ['longlonglonglonglonglong', NULL, NULL]}), (NULL), ({'a': [], 'b': []}), ({'a': [1, 2, 3], 'b': NULL}) ) tbl(i)
CREATE TABLE list_of_structs AS SELECT * FROM (VALUES ([{'a': 1, 'b': 100}, NULL, {'a': 2, 'b': 101}]), (NULL), ([]), ([{'a': NULL, 'b': 102}, {'a': 3, 'b': NULL}, NULL]) ) tbl(i)
CREATE TABLE list_of_struct_of_structs AS SELECT * FROM (VALUES ([{'a': {'x': 33}, 'b': {'y': 42, 'z': 99}}, NULL, {'a': {'x': NULL}, 'b': {'y': 43, 'z': 100}}]), (NULL), ([]), ([{'a': NULL, 'b': {'y': NULL, 'z': 101}}, {'a': {'x': 34}, 'b': {'y': 43, 'z': NULL}}]), ([{'a': NULL, 'b': NULL}]) ) tbl(i)
CREATE TABLE list_of_lists_simple AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5]]), ([[6, 7]]), ([[8, 9, 10], [11, 12]]) ) tbl(i)
CREATE TABLE list_of_lists AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5], [], [6, 7]]), ([[8, NULL, 10], NULL, []]), ([]), (NULL), ([[11, 12, 13, 14], [], NULL, [], [], [15], [NULL, NULL, NULL]]) ) tbl(i)
CREATE TABLE list_of_lists_of_lists_of_lists AS SELECT [LIST(i)] i FROM list_of_lists UNION ALL SELECT NULL UNION ALL SELECT [NULL] UNION ALL SELECT [[], NULL, [], []] UNION ALL SELECT [[[NULL, NULL, [NULL]], NULL, [[], [7, 8, 9], [NULL], NULL, []]], [], [NULL]]
# query
CREATE TABLE struct_of_lists AS SELECT * FROM (VALUES ({'a': [1, 2, 3], 'b': ['hello', 'world']}), ({'a': [4, NULL, 5], 'b': ['duckduck', 'goose']}), ({'a': NULL, 'b': ['longlonglonglonglonglong', NULL, NULL]}), (NULL), ({'a': [], 'b': []}), ({'a': [1, 2, 3], 'b': NULL}) ) tbl(i)
CREATE TABLE list_of_structs AS SELECT * FROM (VALUES ([{'a': 1, 'b': 100}, NULL, {'a': 2, 'b': 101}]), (NULL), ([]), ([{'a': NULL, 'b': 102}, {'a': 3, 'b': NULL}, NULL]) ) tbl(i)
CREATE TABLE list_of_struct_of_structs AS SELECT * FROM (VALUES ([{'a': {'x': 33}, 'b': {'y': 42, 'z': 99}}, NULL, {'a': {'x': NULL}, 'b': {'y': 43, 'z': 100}}]), (NULL), ([]), ([{'a': NULL, 'b': {'y': NULL, 'z': 101}}, {'a': {'x': 34}, 'b': {'y': 43, 'z': NULL}}]), ([{'a': NULL, 'b': NULL}]) ) tbl(i)
CREATE TABLE list_of_lists_simple AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5]]), ([[6, 7]]), ([[8, 9, 10], [11, 12]]) ) tbl(i)
CREATE TABLE list_of_lists AS SELECT * FROM (VALUES ([[1, 2, 3], [4, 5], [], [6, 7]]), ([[8, NULL, 10], NULL, []]), ([]), (NULL), ([[11, 12, 13, 14], [], NULL, [], [], [15], [NULL, NULL, NULL]]) ) tbl(i)
CREATE TABLE list_of_lists_of_lists_of_lists AS SELECT [LIST(i)] i FROM list_of_lists UNION ALL SELECT NULL UNION ALL SELECT [NULL] UNION ALL SELECT [[], NULL, [], []] UNION ALL SELECT [[[NULL, NULL, [NULL]], NULL, [[], [7, 8, 9], [NULL], NULL, []]], [], [NULL]]
# file: test/sql/copy/parquet/writer/write_list.test
# setup
CREATE TABLE list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([8, 9, 10, 11]) ) tbl(i)
CREATE TABLE null_empty_list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([NULL]), ([]), ([]), ([]), ([]), ([8, NULL, 10, 11]), (NULL) ) tbl(i)
# query
CREATE TABLE list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([8, 9, 10, 11]) ) tbl(i)
CREATE TABLE null_empty_list AS SELECT * FROM (VALUES ([1, 2, 3]), ([4, 5]), ([6, 7]), ([NULL]), ([]), ([]), ([]), ([]), ([8, NULL, 10, 11]), (NULL) ) tbl(i)
# file: test/sql/copy/parquet/writer/write_map.test
# setup
CREATE TABLE int_maps(m MAP(INTEGER,INTEGER))
CREATE TABLE string_map(m MAP(VARCHAR,VARCHAR))
CREATE TABLE list_map(m MAP(INT[],INT[]))
# query
CREATE TABLE int_maps(m MAP(INTEGER,INTEGER))
INSERT INTO int_maps VALUES (MAP([42, 84], [1, 2])), (MAP([101, 201, 301], [3, NULL, 5])), (MAP([55, 66, 77], [6, 7, NULL]))
CREATE TABLE string_map(m MAP(VARCHAR,VARCHAR))
INSERT INTO string_map VALUES (MAP(['key1', 'key2'], ['value1', 'value2'])), (MAP(['best band', 'best boyband', 'richest person'], ['Tenacious D', 'Backstreet Boys', 'Jon Lajoie'])), (MAP([], [])), (NULL), (MAP(['option'], [NULL]))
CREATE TABLE list_map(m MAP(INT[],INT[]))
INSERT INTO list_map VALUES (MAP([[1, 2, 3], [], [4, 5]], [[6, 7, 8], NULL, [NULL]])), (MAP([], [])), (MAP([[1]], [NULL])), (MAP([[10, 12, 14, 16, 18, 20], []], [[1], [2]]))
# reject
INSERT INTO int_maps VALUES (MAP([NULL], [NULL]))
INSERT INTO string_map VALUES (MAP([NULL], [NULL]))
INSERT INTO list_map VALUES (MAP([NULL], [NULL]))
# file: test/sql/copy/parquet/writer/write_stats_big_string.test
# setup
CREATE TABLE varchar(v VARCHAR)
# query
CREATE TABLE varchar(v VARCHAR)
INSERT INTO varchar VALUES (NULL), ('hello'), (NULL), ('world'), (NULL)
INSERT INTO varchar SELECT repeat('A', 100000) v
# file: test/sql/copy/parquet/writer/write_stats_null_count.test
# setup
CREATE TABLE structs AS SELECT {'a': NULL, 'b': 'hello'} i UNION ALL SELECT NULL UNION ALL SELECT {'a': 84, 'b': 'world'}
# query
CREATE TABLE structs AS SELECT {'a': NULL, 'b': 'hello'} i UNION ALL SELECT NULL UNION ALL SELECT {'a': 84, 'b': 'world'}
# file: test/sql/copy/parquet/writer/write_struct.test
# setup
CREATE TABLE struct AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': 33, 'b': 32}), ({'a': 42, 'b': 27}) ) tbl(i)
CREATE TABLE struct_nulls AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': NULL, 'b': 32}), (NULL), ({'a': 42, 'b': NULL}) ) tbl(i)
CREATE TABLE struct_nested AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': 27, 'y1': 44}}), ({'a': {'x': 9, 'x1': 26}, 'b': {'y': 1, 'y1': 999}}), ({'a': {'x': 17, 'x1': 23}, 'b': {'y': 3, 'y1': 9999}}) ) tbl(i)
CREATE TABLE struct_nested_null AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': NULL, 'y1': 44}}), ({'a': {'x': NULL, 'x1': 26}, 'b': {'y': 1, 'y1': NULL}}), ({'a': {'x': 17, 'x1': NULL}, 'b': {'y': 3, 'y1': 9999}}), (NULL), ({'a': NULL, 'b': NULL}) ) tbl(i)
CREATE TABLE single_struct AS SELECT * FROM (VALUES ({'a': 42}), ({'a': 33}), ({'a': 42}) ) tbl(i)
CREATE TABLE single_struct_null AS SELECT * FROM (VALUES ({'a': 42}), ({'a': NULL}), (NULL) ) tbl(i)
CREATE TABLE nested_single_struct AS SELECT * FROM (VALUES ({'a': {'b': 42}}), ({'a': {'b': NULL}}), ({'a': NULL}), (NULL) ) tbl(i)
# query
CREATE TABLE struct AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': 33, 'b': 32}), ({'a': 42, 'b': 27}) ) tbl(i)
CREATE TABLE struct_nulls AS SELECT * FROM (VALUES ({'a': 42, 'b': 84}), ({'a': NULL, 'b': 32}), (NULL), ({'a': 42, 'b': NULL}) ) tbl(i)
CREATE TABLE struct_nested AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': 27, 'y1': 44}}), ({'a': {'x': 9, 'x1': 26}, 'b': {'y': 1, 'y1': 999}}), ({'a': {'x': 17, 'x1': 23}, 'b': {'y': 3, 'y1': 9999}}) ) tbl(i)
CREATE TABLE struct_nested_null AS SELECT * FROM (VALUES ({'a': {'x': 3, 'x1': 22}, 'b': {'y': NULL, 'y1': 44}}), ({'a': {'x': NULL, 'x1': 26}, 'b': {'y': 1, 'y1': NULL}}), ({'a': {'x': 17, 'x1': NULL}, 'b': {'y': 3, 'y1': 9999}}), (NULL), ({'a': NULL, 'b': NULL}) ) tbl(i)
CREATE TABLE single_struct AS SELECT * FROM (VALUES ({'a': 42}), ({'a': 33}), ({'a': 42}) ) tbl(i)
CREATE TABLE single_struct_null AS SELECT * FROM (VALUES ({'a': 42}), ({'a': NULL}), (NULL) ) tbl(i)
CREATE TABLE nested_single_struct AS SELECT * FROM (VALUES ({'a': {'b': 42}}), ({'a': {'b': NULL}}), ({'a': NULL}), (NULL) ) tbl(i)
# file: test/sql/storage/append_strings_to_persistent.test
# setup
CREATE TABLE vals(i INTEGER, v VARCHAR)
# query
CREATE TABLE vals(i INTEGER, v VARCHAR)
INSERT INTO vals VALUES (1, 'hello')
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(2,10000) t(i)
SELECT MIN(i), MAX(i), MIN(v), MAX(v) FROM vals
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(10001,100000) t(i)
# file: test/sql/storage/buffer_manager_temp_dir.test
# setup
CREATE TABLE t2 AS SELECT * FROM range(1000000)
# query
PRAGMA temp_directory=''
PRAGMA memory_limit='2MB'
# reject
CREATE TABLE t2 AS SELECT * FROM range(1000000)
# file: test/sql/storage/checkpointed_self_append.test
# setup
CREATE TABLE vals(i INTEGER)
# query
CREATE TABLE vals(i INTEGER)
INSERT INTO vals SELECT CASE WHEN i % 2 = 0 THEN NULL ELSE i END FROM range(200000) tbl(i)
SELECT MIN(i), MAX(i), COUNT(i), COUNT(*) FROM vals
INSERT INTO vals SELECT * FROM vals
# file: test/sql/storage/checkpointed_self_append_tinyint.test
# setup
CREATE TABLE vals(i TINYINT)
# query
CREATE TABLE vals(i TINYINT)
INSERT INTO vals SELECT (CASE WHEN i % 2 = 0 THEN NULL ELSE i % 100 END)::TINYINT i FROM range(200000) tbl(i)
# file: test/sql/storage/commit_abort.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
# query
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
INSERT INTO test VALUES (11, 22, 'hello'), (13, 22, 'world'), (12, 21, 'test'), (10, NULL, NULL)
INSERT INTO test VALUES (14, 10, 'con')
INSERT INTO test VALUES (15, 10, 'con2')
INSERT INTO test VALUES (14, 10, 'con2')
INSERT INTO test VALUES (15, NULL, NULL)
SELECT COUNT(*), COUNT(a), COUNT(b), SUM(a), SUM(b), SUM(LENGTH(c)) FROM test
SELECT * FROM test ORDER BY a, b, c
# file: test/sql/storage/commit_abort_medium.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
# query
INSERT INTO test SELECT i, NULL, NULL FROM range(15, 10000) tbl(i)
INSERT INTO test VALUES (16, 24, 'blabla')
# file: test/sql/storage/commit_index_deletes.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
# query
DELETE FROM test WHERE a=14
INSERT INTO test VALUES (14, 11, 'bla')
# file: test/sql/storage/distinct_statistics_storage.test
# setup
create table test as select range % 10 i, range % 30 j from range(100)
# query
create table test as select range % 10 i, range % 30 j from range(100)
select stats(i), stats(j) from test limit 1
# file: test/sql/storage/filter_pushdown_struct.test
# setup
CREATE TABLE tbl (a STRUCT("id" VARCHAR), b STRUCT("id" VARCHAR))
# query
CREATE TABLE tbl (a STRUCT("id" VARCHAR), b STRUCT("id" VARCHAR))
INSERT INTO tbl SELECT {'id': LPAD(i::VARCHAR, 4, '0')}, {'id': 'abc'} FROM range(10000) t(i)
SELECT COUNT(*) FROM (SELECT * FROM tbl WHERE b.id='abc') t
INSERT INTO tbl SELECT {'id': LPAD((i + 10000)::VARCHAR, 4, '0')}, {'id': 'bcd'} FROM range(10000) t(i)
SELECT COUNT(*) FROM (SELECT * FROM tbl WHERE b.id='bcd') t
# file: test/sql/storage/icu_collation.test
# query
SELECT * FROM strings ORDER BY 1
# file: test/sql/storage/issue3789_node_segment_tree.test
# setup
CREATE TABLE table1 (column1 integer, column2 integer)
# query
CREATE TABLE table1 (column1 integer, column2 integer)
INSERT INTO table1(column1, column2) values(1, 1)
INSERT INTO table1(column1, column2) values(1, 2)
UPDATE table1 SET column2 = 3 FROM table1 s WHERE s.column1 = 1
# file: test/sql/storage/issue7582_list_storage.test
# setup
CREATE TABLE tbl (n TEXT[])
# query
SET wal_autocheckpoint='1GB'
CREATE TABLE tbl (n TEXT[])
INSERT INTO tbl (n) SELECT CASE WHEN i < 100 THEN ['a', 'b'] ELSE [] END l FROM range(1026) t(i)
FROM tbl
# file: test/sql/storage/many_checkpoints.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER, c VARCHAR)
# query
SELECT COUNT(*) FROM test
SELECT * FROM test ORDER BY 1, 2, 3
# reject
INSERT INTO test VALUES (11, 22, 'hello')
# file: test/sql/storage/many_self_append.test
# setup
CREATE TABLE vals(i TINYINT)
# query
INSERT INTO vals SELECT (CASE WHEN i % 2 = 0 THEN NULL ELSE i % 100 END)::TINYINT i FROM range(10) tbl(i)
# file: test/sql/storage/multiple_clients_checkpoint_pending_updates.test
# setup
CREATE TABLE test (i INTEGER)
# query
INSERT INTO test SELECT * FROM range(1000000)
UPDATE test SET i=i+1
SELECT MIN(i), MAX(i), COUNT(*) FROM test
UPDATE test SET i=i+1 WHERE i < 1000
UPDATE test SET i=i+1 WHERE i > 1000 AND i < 2000
UPDATE test SET i=i+1 WHERE i > 2000 AND i < 3000
UPDATE test SET i=i+1 WHERE i > 3000 AND i < 4000
# file: test/sql/storage/null_byte_storage.test
# setup
CREATE TABLE null_byte AS SELECT concat('goo', chr(0), i) AS v FROM range(10000) tbl(i)
CREATE INDEX i_index ON null_byte(v)
# query
CREATE TABLE null_byte AS SELECT concat('goo', chr(0), i) AS v FROM range(10000) tbl(i)
SELECT MIN(v), MAX(v) FROM null_byte
SELECT * FROM null_byte WHERE v=concat('goo', chr(0), 42)
CREATE INDEX i_index ON null_byte(v)
DROP TABLE null_byte
# file: test/sql/storage/shutdown_create_index.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE INDEX i_index ON test using art(a)
# query
CREATE TABLE test (a INTEGER, b INTEGER)
INSERT INTO test VALUES (11, 22), (13, 22)
CREATE INDEX i_index ON test using art(a)
INSERT INTO test VALUES (11, 24)
SELECT a, b FROM test WHERE a=11 ORDER BY b
SELECT a, b FROM test WHERE a>11 ORDER BY b
DELETE FROM test WHERE a=11 AND b=24
DELETE FROM test WHERE a=11 AND b=22
UPDATE test SET b=22 WHERE a=11
# file: test/sql/storage/shutdown_running_transaction.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (22, 23)
# file: test/sql/storage/shutdown_unique_index.test
# setup
CREATE TABLE test (a INTEGER PRIMARY KEY, b INTEGER)
# query
INSERT INTO test VALUES (12, 24)
SELECT * FROM test WHERE a=12
# file: test/sql/storage/storage_exceeds_block_large_string.test
# setup
CREATE TABLE test (a VARCHAR, j BIGINT)
# query
SET force_compression='uncompressed'
CREATE TABLE test (a VARCHAR, j BIGINT)
INSERT INTO test VALUES (repeat('a', 64), 1)
# file: test/sql/storage/storage_exceeds_single_block.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (11, 22), (13, 22), (12, 21), (NULL, NULL)
INSERT INTO test FROM test
SELECT SUM(a) + SUM(b) FROM test
# file: test/sql/storage/storage_exceeds_single_block_strings.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test VALUES ('a'), ('bb'), ('ccc'), ('dddd'), ('eeeee')
SELECT a, COUNT(*) FROM test GROUP BY a ORDER BY a
SELECT count(a) FROM test WHERE a='a'
UPDATE test SET a='aaa' WHERE a='a'
# file: test/sql/storage/storage_exceeds_single_block_types.test
# setup
CREATE TABLE test (a INTEGER, b BIGINT)
# query
CREATE TABLE test (a INTEGER, b BIGINT)
# file: test/sql/storage/storage_types.test
# setup
CREATE TABLE a_interval AS SELECT interval (range) year i FROM range(1,1001)
CREATE TABLE a_bool AS SELECT range % 2 = 0 AS i FROM range(1000)
# query
CREATE TABLE a_interval AS SELECT interval (range) year i FROM range(1,1001)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval WHERE i = interval 1 year
CREATE TABLE a_bool AS SELECT range % 2 = 0 AS i FROM range(1000)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool WHERE NOT i
# file: test/sql/storage/storage_version_65.test
# query
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_versions65'
set storage_compatibility_version='v0.10.2'
SELECT tags FROM duckdb_databases() WHERE database_name = 'regular_file'
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_version64'
SELECT tags FROM duckdb_databases() WHERE database_name = 'storage_versions66'
# file: test/sql/storage/storage_versions.test
# query
SELECT tags FROM duckdb_databases() WHERE database_name LIKE 'empty%' ORDER BY database_name
# file: test/sql/storage/store_group_order_all.test
# setup
CREATE TABLE integers( g integer, i integer )
CREATE VIEW v1 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL
CREATE VIEW v2 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL DESC NULLS LAST
# query
CREATE TABLE integers( g integer, i integer )
INSERT INTO integers values (0, 1), (0, 2), (1, 3), (1, NULL)
CREATE VIEW v1 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL
CREATE VIEW v2 AS SELECT g, i, g%2, SUM(i), SUM(g) FROM integers GROUP BY ALL ORDER BY ALL DESC NULLS LAST
# file: test/sql/storage/test_empty_table.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR)
# query
CREATE TABLE test (a INTEGER, b VARCHAR)
# file: test/sql/storage/test_ignore_duplicate_deletes_unique_index.test
# setup
CREATE TABLE TBL (id INT NOT NULL, age INT NOT NULL, PRIMARY KEY ( id ))
# query
CREATE TABLE TBL (id INT NOT NULL, age INT NOT NULL, PRIMARY KEY ( id ))
INSERT INTO TBL VALUES (1, 1)
DELETE FROM TBL WHERE id = 1
SELECT * FROM TBL
SELECT * FROM TBL WHERE id=1
# file: test/sql/storage/test_index_checkpoint.test
# setup
CREATE TABLE t2 (i INTEGER, uid VARCHAR)
CREATE UNIQUE INDEX iu ON t2(uid)
# query
CREATE TABLE t2 (i INTEGER, uid VARCHAR)
INSERT INTO t2 SELECT i.range AS i, gen_random_uuid() AS uid FROM range(50000) AS i
CREATE UNIQUE INDEX iu ON t2(uid)
SELECT total_blocks < 6291456 / get_block_size('index_checkpoint') * 1.2 FROM pragma_database_size()
# file: test/sql/storage/test_large_commits.test
# setup
CREATE TABLE test(i INTEGER)
# query
PRAGMA wal_autocheckpoint='10KB'
CREATE TABLE test(i INTEGER)
INSERT INTO test SELECT * FROM range(100000) tbl(i)
# file: test/sql/storage/test_read_only_wal_replay.test
# setup
CREATE TABLE db.my_tbl(i INTEGER PRIMARY KEY)
# query
SET checkpoint_threshold='1TB'
CREATE TABLE db.my_tbl(i INTEGER PRIMARY KEY)
INSERT INTO db.my_tbl FROM range(200_000)
DETACH db
# file: test/sql/storage/test_show_tables_persistent.test
# setup
create table anno as select 42
# query
show tables
select current_user
create table anno as select 42
drop table if exists anno
# file: test/sql/storage/test_storage_scan.test
# setup
CREATE TABLE test (a INTEGER)
# query
INSERT INTO test VALUES (11), (12), (13), (14), (15), (NULL)
DELETE FROM test WHERE a=12
DELETE FROM test WHERE a=13
# file: test/sql/storage/test_store_integers.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE TABLE test2 (a INTEGER)
# query
CREATE TABLE test2 (a INTEGER)
INSERT INTO test2 VALUES (13), (12), (11)
SELECT * FROM test2 ORDER BY a
INSERT INTO test VALUES (14, 23)
DROP TABLE test2
# file: test/sql/storage/test_store_nulls_strings.test
# setup
CREATE TABLE IF NOT EXISTS test (a INTEGER, b STRING)
# query
CREATE TABLE test (a INTEGER, b STRING)
INSERT INTO test VALUES (NULL, 'hello'), (13, 'abcdefgh'), (12, NULL)
SELECT a, b FROM test ORDER BY a
CREATE TABLE IF NOT EXISTS test (a INTEGER, b STRING)
# file: test/sql/storage/test_truncate_persistent.test
# setup
CREATE TABLE test AS FROM range(250000) t(i)
# query
CREATE TABLE test AS FROM range(250000) t(i)
DELETE FROM test WHERE i < 150000
TRUNCATE test
# file: test/sql/storage/test_unaligned_scan.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR)
# query
INSERT INTO test SELECT CASE WHEN i%2=0 THEN i ELSE NULL END, CASE WHEN i%2=0 THEN 'hello'||i::VARCHAR ELSE NULL END FROM range(10000) tbl(i)
SELECT COUNT(*), SUM(a), MIN(a), MAX(a), MIN(b), MAX(b), COUNT(a), COUNT(b) FROM test
# file: test/sql/storage/test_unique_index_checkpoint.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
CREATE TABLE IF NOT EXISTS unique_index_test AS SELECT i AS ordernumber, j AS quantity FROM test
CREATE UNIQUE INDEX idx ON test (i)
CREATE UNIQUE INDEX unique_index_test_ordernumber_idx_unique ON unique_index_test (ordernumber)
# query
INSERT INTO test VALUES (1,100),(2,200)
CREATE UNIQUE INDEX idx ON test (i)
CREATE TABLE IF NOT EXISTS unique_index_test AS SELECT i AS ordernumber, j AS quantity FROM test
CREATE UNIQUE INDEX unique_index_test_ordernumber_idx_unique ON unique_index_test (ordernumber)
# reject
INSERT INTO test VALUES (1,101),(2,201)
INSERT INTO unique_index_test VALUES (1,101),(2,201)
# file: test/sql/storage/unzip.test
# query
SELECT a+1 FROM tbl
SELECT a+2 FROM tbl
# file: test/sql/storage/encryption/wal/encrypted_wal_blob_storage.test
# setup
CREATE TABLE enc.blobs (b BLOB)
# query
CREATE TABLE enc.blobs (b BLOB)
DETACH enc
SELECT * FROM enc.blobs
# file: test/sql/storage/encryption/wal/encrypted_wal_lazy_creation.test
# setup
CREATE TABLE attach_no_wal.integers(i INTEGER)
# query
CREATE TABLE attach_no_wal.integers(i INTEGER)
INSERT INTO attach_no_wal.integers FROM range(10000)
DETACH attach_no_wal
SELECT COUNT(*) FROM attach_no_wal.integers
# file: test/sql/storage/encryption/wal/encrypted_wal_pragmas.test
# setup
CREATE TABLE enc.test (a INTEGER, b INTEGER)
# query
CREATE TABLE enc.test (a INTEGER, b INTEGER)
INSERT INTO enc.test VALUES (11, 22), (13, 22), (12, 21)
ALTER TABLE enc.test ALTER b TYPE VARCHAR
SELECT * FROM enc.test ORDER BY 1
INSERT INTO enc.test VALUES (10, 'hello')
# file: test/sql/storage/encryption/temp_files/encrypted_tmp_file_setting.test
# setup
CREATE TEMPORARY TABLE tbl AS FROM range(10_000_000)
# query
SET temp_file_encryption = false
SET memory_limit = '8MB'
CREATE TEMPORARY TABLE tbl AS FROM range(10_000_000)
# reject
RESET temp_directory
SET temp_file_encryption = true
# file: test/sql/storage/encryption/temp_files/temp_directory_enable_external_access.test
# setup
CREATE TEMPORARY TABLE tbl AS FROM range(10_000_000)
# query
USE enc
# file: test/sql/storage/lazy_load/lazy_load_limit.test
# setup
CREATE TABLE vals(i INTEGER, v VARCHAR)
# query
INSERT INTO vals SELECT i, i::VARCHAR FROM generate_series(1000000) t(i)
# file: test/sql/storage/types/test_bit_storage.test
# setup
CREATE TABLE bits (b BIT)
# query
CREATE TABLE bits (b BIT)
INSERT INTO bits VALUES('1'), ('010111'), ('111110010011'), (NULL), ('000000000000000000'), ('00100110010100100101001010010101010011110101000000000111100100110')
SELECT * FROM bits
# file: test/sql/storage/types/test_blob_storage.test
# setup
CREATE TABLE blobs (b BLOB)
# query
CREATE TABLE blobs (b BLOB)
SELECT * FROM blobs
# file: test/sql/storage/types/test_hugeint_storage.test
# setup
CREATE TABLE hugeints (h HUGEINT)
# query
CREATE TABLE hugeints (h HUGEINT)
INSERT INTO hugeints VALUES (1043178439874412422424), (42), (NULL), (47289478944894789472897441242)
SELECT * FROM hugeints
SELECT * FROM hugeints WHERE h = 42
SELECT h FROM hugeints WHERE h < 10 ORDER BY 1
# file: test/sql/storage/types/test_interval_storage.test
# setup
CREATE TABLE interval (t INTERVAL)
# query
CREATE TABLE interval (t INTERVAL)
INSERT INTO interval VALUES (INTERVAL '1' DAY), (NULL), (INTERVAL '3 months 2 days 5 seconds')
SELECT * FROM interval
SELECT t FROM interval WHERE t = INTERVAL '1' DAY
SELECT t FROM interval WHERE t >= INTERVAL '1' DAY ORDER BY 1
SELECT t FROM interval WHERE t > INTERVAL '10' YEAR ORDER BY 1
# file: test/sql/storage/types/test_timestamp_storage.test
# setup
CREATE TABLE timestamp (sec TIMESTAMP_S, milli TIMESTAMP_MS,micro TIMESTAMP_US, nano TIMESTAMP_NS )
# query
CREATE TABLE timestamp (sec TIMESTAMP_S, milli TIMESTAMP_MS,micro TIMESTAMP_US, nano TIMESTAMP_NS )
INSERT INTO timestamp VALUES (NULL,NULL,NULL,NULL )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01','2008-01-01 00:00:01.594','2008-01-01 00:00:01.88926','2008-01-01 00:00:01.889268321' )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:51','2008-01-01 00:00:01.894','2008-01-01 00:00:01.99926','2008-01-01 00:00:01.999268321' )
INSERT INTO timestamp VALUES ('2008-01-01 00:00:11','2008-01-01 00:00:01.794','2008-01-01 00:00:01.98926','2008-01-01 00:00:01.899268321' )
SELECT * FROM timestamp ORDER BY sec
SELECT * FROM timestamp WHERE micro=TIMESTAMP '2008-01-01 00:00:01.88926' ORDER BY micro
SELECT * FROM timestamp WHERE micro=TIMESTAMP '2020-01-01 00:00:01.88926' ORDER BY micro
# file: test/sql/storage/types/test_uhugeint_storage.test
# setup
CREATE TABLE uhugeints (h UHUGEINT)
# query
CREATE TABLE uhugeints (h UHUGEINT)
INSERT INTO uhugeints VALUES (0), (42), (NULL), ('340282366920938463463374607431768211455'::UHUGEINT)
SELECT * FROM uhugeints
SELECT * FROM uhugeints WHERE h = 42
SELECT h FROM uhugeints WHERE h < 10 ORDER BY 1
# file: test/sql/storage/types/test_unsigned_storage.test
# setup
CREATE TABLE unsigned (a utinyint, b usmallint, c uinteger, d ubigint)
# query
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
# file: test/sql/storage/types/test_uuid.storage.test
# setup
CREATE TABLE uuids (u uuid)
# query
CREATE TABLE uuids (u uuid)
INSERT INTO uuids VALUES ('A0EEBC99-9C0B-4EF8-BB6D-6BB9BD380A11'), (NULL), ('47183823-2574-4bfd-b411-99ed177d3e43'), ('{10203040506070800102030405060708}')
SELECT * FROM uuids
SELECT * FROM uuids WHERE u = 'A0EEBC99-9C0B-4EF8-BB6D-6BB9BD380A11'
SELECT * FROM uuids WHERE u = 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'
SELECT u FROM uuids WHERE u > '10203040-5060-7080-0102-030405060708' ORDER BY 1
# file: test/sql/storage/types/struct/default_struct.test
# setup
CREATE TABLE a(i ROW(a INT, b INT) DEFAULT ({'a': 7, 'b': 2}))
# query
CREATE TABLE a(i ROW(a INT, b INT) DEFAULT ({'a': 7, 'b': 2}))
INSERT INTO a VALUES (DEFAULT)
# file: test/sql/storage/types/struct/nested_struct_storage.test
# setup
CREATE TABLE a AS SELECT { 'r1': { 'a': 'hello', 'b': 3 }, 'r2': { 'a': 'world', 'b': 17, 'c': NULL } } c
# query
CREATE TABLE a AS SELECT { 'r1': { 'a': 'hello', 'b': 3 }, 'r2': { 'a': 'world', 'b': 17, 'c': NULL } } c
SELECT c['r1']['a'] from a
UPDATE a SET c={ 'r1': { 'a': 'blabla', 'b': 3 }, 'r2': { 'a': 'world', 'b': 18, 'c': NULL } }
INSERT INTO a VALUES ( { 'r1': { 'a': NULL, 'b': 3 }, 'r2': { 'a': NULL, 'b': 17, 'c': NULL } })
INSERT INTO a VALUES ({ 'r1': NULL, 'r2': { 'a': NULL, 'b': 17, 'c': NULL } })
INSERT INTO a VALUES ({ 'r1': NULL, 'r2': NULL })
INSERT INTO a VALUES(NULL)
select column_path, stats from pragma_storage_info('a') where stats LIKE '%[Min: -2147483648, Max: -2147483648]%'
DROP TABLE a
# file: test/sql/storage/types/struct/pushdown_extract_validity.test
# setup
CREATE TABLE structs(s STRUCT(id INT))
# query
CREATE TABLE structs(s STRUCT(id INT))
INSERT INTO structs SELECT {'id': CASE WHEN r%3=0 THEN NULL ELSE i END } FROM ( SELECT UNNEST(range(1200)) r, UNNEST(repeat([1], 1000)) i UNION ALL SELECT UNNEST(range(1000)) r, UNNEST(repeat([2], 1000)) i )
INSERT INTO structs SELECT CASE WHEN r%13=0 THEN NULL ELSE {'id': CASE WHEN r%7=0 THEN NULL ELSE i END } END FROM ( SELECT UNNEST(range(2000)) r, UNNEST(repeat([1], 2000)) i )
SELECT DISTINCT s.id FROM structs ORDER BY ALL
# file: test/sql/storage/types/struct/struct_of_empty_list.test
# setup
create table tbl (col STRUCT(a VARCHAR[]))
# query
create table tbl (col STRUCT(a VARCHAR[]))
insert into tbl SELECT {'a': []} from range(122881)
# file: test/sql/storage/types/struct/struct_storage.test
# setup
CREATE TABLE a(b STRUCT(i INTEGER, j INTEGER))
# query
CREATE TABLE a(b STRUCT(i INTEGER, j INTEGER))
INSERT INTO a VALUES ({'i': 1, 'j': 2}), (NULL), ({'i': NULL, 'j': 2}), (ROW(1, NULL))
SELECT COUNT(*) FROM a WHERE b IS NULL
DELETE FROM a WHERE (b).i=1
UPDATE a SET b={i: 7, j: 9} WHERE b IS NULL
# file: test/sql/storage/types/map/map_storage.test
# setup
CREATE TABLE a(b MAP(INTEGER,INTEGER))
# query
CREATE TABLE a(b MAP(INTEGER,INTEGER))
INSERT INTO a VALUES (MAP([1], [2])), (MAP([1, 2, 3], [4, 5, 6]))
# file: test/sql/storage/types/list/default_list.test
# setup
CREATE TABLE a(i INT[] DEFAULT ([1, 2, 3]))
# query
CREATE TABLE a(i INT[] DEFAULT ([1, 2, 3]))
# file: test/sql/storage/types/list/empty_float_arrays.test
# setup
CREATE TABLE test_table ( id INTEGER, emb FLOAT[], emb_arr FLOAT[3] )
# query
CREATE TABLE test_table ( id INTEGER, emb FLOAT[], emb_arr FLOAT[3] )
INSERT INTO test_table (id) VALUES (42)
FROM test_table
DROP TABLE test_table
# file: test/sql/storage/types/list/list_index_compression.test
# setup
CREATE TABLE a(id INTEGER, c INT[])
CREATE INDEX a_index ON a(id)
# query
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
# file: test/sql/storage/types/list/persistent_list_storage.test
# setup
CREATE TABLE a(b INTEGER[])
CREATE TABLE b(b INTEGER[][])
CREATE TABLE c(b VARCHAR[])
# query
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
# file: test/sql/storage/types/list/store_list_of_struct.test
# setup
CREATE TABLE a(id INTEGER, b ROW(a INTEGER, b INTEGER)[])
# query
CREATE TABLE a(id INTEGER, b ROW(a INTEGER, b INTEGER)[])
INSERT INTO a VALUES (1, [{'a': 3, 'b': 7}, {'a': NULL, 'b': 7}, NULL]), (2, []), (3, NULL), (4, [NULL, {'a': 7, 'b': NULL}, {'a': 1, 'b': 1}])
SELECT * FROM a ORDER BY id
UPDATE a SET b=[] WHERE id=3
# file: test/sql/storage/types/variant/append_shredded.test
# setup
create table tbl (col VARIANT)
# query
create table tbl (col VARIANT)
insert into tbl SELECT NULL from range(154840)
insert into tbl SELECT True from range(5000)
# file: test/sql/storage/types/variant/extension_types.test
# setup
CREATE TABLE tbl (col VARIANT)
# query
CREATE TABLE tbl (col VARIANT)
INSERT into tbl select '127.0.0.1'::INET
select * from tbl
select COLUMNS(*)::INET from tbl
# file: test/sql/storage/types/variant/index_fetch.test
# setup
CREATE TABLE tbl(i INT PRIMARY KEY, v VARIANT)
# query
CREATE TABLE tbl(i INT PRIMARY KEY, v VARIANT)
INSERT INTO tbl select i, {'a': i, 'b': i % 5} from range(100) t(i)
SELECT v FROM tbl WHERE i=42
USE db2
pragma verify_fetch_row
select v from tbl WHERE i < 10
# file: test/sql/storage/types/variant/struct_of_variant.test
# setup
CREATE TABLE variant_list( col STRUCT( f1 INTEGER, f2 VARIANT, f3 VARCHAR, f4 BOOL ) )
# query
SET variant_minimum_shredding_size = 0
CREATE TABLE variant_list( col STRUCT( f1 INTEGER, f2 VARIANT, f3 VARCHAR, f4 BOOL ) )
INSERT INTO variant_list SELECT { 'f1': i, 'f2': {'a': i::INTEGER, 'b': 'val' || i}, 'f3': 'test', 'f4': i % 2 == 0 } from range(1000) t(i)
select col.f2.a::INTEGER, col.f2.b::VARCHAR, col.f4 from variant_list limit 10
# file: test/sql/storage/types/variant/test_all_types_single_object.test
# setup
create table tbl ( col VARIANT )
create or replace table intermediate as from query($$select col."$$ || getvariable('col_name') || $$" extracted from tbl$$)
# query
create table tbl ( col VARIANT )
insert into tbl select t::VARIANT var from test_all_types() t
from query($$select col."$$ || getvariable('col_name') || $$"::$$ || getvariable('col_type') || ' from tbl')
create or replace table intermediate as from query($$select col."$$ || getvariable('col_name') || $$" extracted from tbl$$)
from query('select extracted::' || getvariable('col_type') || ' from intermediate')
# file: test/sql/storage/types/variant/test_all_types_single_table.test
# setup
create table tbl (col VARIANT)
create or replace table "tbl2" as select * from tbl
# query
SET force_variant_shredding = getvariable('my_type')
create or replace table "tbl2" as select * from tbl
select * from "tbl2"
SET variant_minimum_shredding_size = -1
# file: test/sql/storage/types/variant/test_all_types_variant.test
# setup
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
# query
create table tbl as select COLUMNS(*)::VARIANT from test_all_types()
# file: test/sql/storage/types/variant/update.test
# setup
create table tbl (a VARIANT)
# query
create table tbl (a VARIANT)
insert into tbl VALUES (42)
update tbl SET a = 21
# file: test/sql/storage/types/variant/variant_case_sensitive_fields_shredded.test
# setup
create or replace table test_structs( id int, s VARIANT )
# query
set variant_minimum_shredding_size = 0
create or replace table test_structs( id int, s VARIANT )
insert into test_structs values (1, { 'name': { 'v': 'row 1', 'id': 1 }, 'nested_struct': { 'a': 42, 'b': true } }), (2, null), (3, { 'name': { 'v': 'row 3', 'id': 3 }, 'nested_struct': { 'a': 84, 'b': null } }), (4, { 'name': null, 'nested_struct': { 'A': null, 'b': false } })
# file: test/sql/storage/types/variant/variant_extract_stats.test
# setup
create table tbl(col VARIANT)
# query
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
# file: test/sql/storage/types/variant/variant_index.test
# query
INSERT into tbl select {'a': i, 'b': i % 5} col, i from range(1000) t(i)
select b from tbl where col == {'b': 2, 'a': 2}
# reject
CREATE TABLE tbl(col VARIANT UNIQUE, b INTEGER)
# file: test/sql/storage/types/variant/variant_null_missing.test
# setup
create table shredded_values (col VARIANT)
create table nested_shredded_values (col VARIANT)
create table shredded_array (col VARIANT)
# query
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
# file: test/sql/storage/types/variant/variant_null_shredding_inconsistency_issue.test
# setup
CREATE TABLE messages AS SELECT ( CASE WHEN i % 100 = 0 THEN '{"action": "block", "extra_field": null}' ELSE '{"action": "build"}' END )::JSON AS msg_json FROM range(10000) t(i)
# query
CREATE TABLE messages AS SELECT ( CASE WHEN i % 100 = 0 THEN '{"action": "block", "extra_field": null}' ELSE '{"action": "build"}' END )::JSON AS msg_json FROM range(10000) t(i)
ALTER TABLE messages ADD COLUMN msg VARIANT
UPDATE messages SET msg = msg_json::VARIANT
select count(*) from messages where msg.action::VARCHAR = 'block'
# file: test/sql/storage/types/variant/variant_parquet_shredding_bug.test
# setup
CREATE TABLE t AS SELECT ('{"id":"' || md5(i::VARCHAR) || md5((i+9)::VARCHAR) || '","x":' || CASE WHEN i < 150000 THEN '"a string"' ELSE '[1,2,3]' END || '}')::JSON::VARIANT AS v FROM range(300000) tbl(i) ORDER BY i
# query
CREATE TABLE t AS SELECT ('{"id":"' || md5(i::VARCHAR) || md5((i+9)::VARCHAR) || '","x":' || CASE WHEN i < 150000 THEN '"a string"' ELSE '[1,2,3]' END || '}')::JSON::VARIANT AS v FROM range(300000) tbl(i) ORDER BY i
# file: test/sql/storage/types/variant/variant_shredding_empty_keys.test
# setup
create table succeeds as SELECT '{"": 1, "x": {"y": "t"}}'::JSON::VARIANT
create table fails as SELECT '{"x": "hello", "": "world"}'::JSON::VARIANT AS j
# query
SET variant_minimum_shredding_size=0
create table succeeds as SELECT '{"": 1, "x": {"y": "t"}}'::JSON::VARIANT
select * from succeeds
create table fails as SELECT '{"x": "hello", "": "world"}'::JSON::VARIANT AS j
select * from fails
# file: test/sql/storage/types/variant/variant_shredding_inconsistent.test
# setup
create table bluesky (col VARIANT)
# query
create table bluesky (col VARIANT)
# file: test/sql/storage/types/variant/variant_shredding_omit_untyped.test
# setup
create table shredded_integer (col VARIANT)
# query
create table shredded_integer (col VARIANT)
insert into shredded_integer select (i % 100)::INTEGER from range(100) t(i)
SELECT COUNT(*) FROM pragma_storage_info('shredded_integer') WHERE column_path = '[0, 2, 2]'
SELECT SUM(TRY_CAST(col AS INT)), COUNT(*) FROM shredded_integer
insert into shredded_integer values ('hello world')
# file: test/sql/storage/types/variant/variant_storage_version.test
# setup
create table t3 (i INT)
# query
use variant
create table t3 (i INT)
# reject
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
create table t1 (v VARIANT)
create table t2 as select '1'::VARIANT v
alter table t3 add column v VARIANT
alter table t3 alter column i set type VARIANT
# file: test/sql/storage/checkpoint/checkpoint_with_outstanding_insertions.test
# setup
create or replace table z(id integer)
# query
create or replace table z(id integer)
insert into z from range(200_000)
set checkpoint_threshold='1TB'
set immediate_transaction_mode=true
begin
insert into z from range(200_000, 400_000)
select min(id), max(id) from z
# file: test/sql/storage/checkpoint/indexed_table_delete_insert_fragmentation.test
# setup
CREATE OR REPLACE TABLE snap.snapshot(pk BIGINT PRIMARY KEY, val1 VARCHAR, val2 VARCHAR)
CREATE OR REPLACE TABLE novelty_inserts AS SELECT pk, uuid()::VARCHAR as val1, uuid()::VARCHAR as val2 FROM novelty_deletes
# query
CREATE OR REPLACE TABLE snap.snapshot(pk BIGINT PRIMARY KEY, val1 VARCHAR, val2 VARCHAR)
INSERT INTO snap.snapshot SELECT pk, uuid()::VARCHAR as val1, uuid()::VARCHAR as val2 FROM generate_series(1, 4096) t(pk)
CHECKPOINT snap
DETACH snap
CREATE OR REPLACE TABLE novelty_inserts AS SELECT pk, uuid()::VARCHAR as val1, uuid()::VARCHAR as val2 FROM novelty_deletes
DELETE FROM snap.snapshot WHERE pk IN (SELECT pk FROM novelty_deletes)
INSERT INTO snap.snapshot SELECT * FROM novelty_inserts
# file: test/sql/storage/checkpoint/test_checkpoint_failure_delayed_commit.test
# setup
CREATE TABLE db.integers AS SELECT * FROM range(100) tbl(i)
# query
SET threads = 1
PRAGMA wal_autocheckpoint = '1TB'
PRAGMA debug_checkpoint_abort = 'before_header'
CREATE TABLE db.integers AS SELECT * FROM range(100) tbl(i)
INSERT INTO db.integers VALUES (42)
# file: test/sql/storage/checkpoint/test_checkpoint_failure_on_detach.test
# setup
CREATE TABLE fail_detach.integers AS SELECT * FROM range(100) tbl(i)
# query
CREATE TABLE fail_detach.integers AS SELECT * FROM range(100) tbl(i)
# reject
DETACH fail_detach
# file: test/sql/storage/memory/in_memory_compress.test
# setup
CREATE TABLE memory_compressed.a(i INTEGER)
# query
ATTACH ':memory:' AS memory_compressed (COMPRESS)
CREATE TABLE memory_compressed.a(i INTEGER)
INSERT INTO memory_compressed.a FROM range(10000000)
PRAGMA force_checkpoint
FORCE CHECKPOINT memory_compressed
SELECT case when memory_usage_bytes < 1000000 then 'success' else error(concat('Expected less than ', 1000000, ' bytes, but got ', memory_usage_bytes)) end FROM duckdb_memory() WHERE tag='IN_MEMORY_TABLE'
# file: test/sql/storage/memory/in_memory_disabled_zstd.test
# setup
create table tbl as select i // 5_000 as num, num::varchar || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
# query
attach ':memory:' as db2 (compress)
use db2
pragma force_compression='zstd'
create table tbl as select i // 5_000 as num, num::varchar || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
force checkpoint
select distinct compression = 'Uncompressed' from pragma_storage_info('tbl') where segment_type = 'VARCHAR'
# file: test/sql/storage/read_duckdb/read_duckdb_basic.test
# setup
CREATE TABLE read_duckdb_test.my_tbl AS SELECT 42 i
CREATE TABLE read_duckdb_test2.other_tbl AS SELECT 100 i
CREATE TABLE read_duckdb_test.my_tbl2 AS SELECT 84 j
# query
CREATE TABLE read_duckdb_test.my_tbl AS SELECT 42 i
DETACH read_duckdb_test
SELECT COUNT(*) FROM duckdb_databases
CREATE TABLE read_duckdb_test2.other_tbl AS SELECT 100 i
DETACH read_duckdb_test2
CREATE TABLE read_duckdb_test.my_tbl2 AS SELECT 84 j
# file: test/sql/storage/read_duckdb/read_duckdb_generated.test
# query
CREATE TABLE rd.tbl ( price INTEGER, amount_sold INTEGER, total_profit AS (price * amount_sold), non_generated INTEGER )
INSERT INTO rd.tbl VALUES (5,4, 100)
DETACH rd
# file: test/sql/storage/read_duckdb/read_duckdb_index.test
# setup
CREATE TABLE read_duckdb_index.my_tbl(i INTEGER PRIMARY KEY)
# query
CREATE TABLE read_duckdb_index.my_tbl(i INTEGER PRIMARY KEY)
INSERT INTO read_duckdb_index.my_tbl SELECT i + 1 FROM range(1000000) t(i)
DETACH read_duckdb_index
# file: test/sql/storage/read_duckdb/read_duckdb_schema.test
# setup
CREATE SCHEMA rd.s1
CREATE SCHEMA rd.s2
CREATE TABLE rd.s1.my_tbl AS SELECT 42 i
CREATE TABLE rd.s2.my_tbl AS SELECT 84 i
# query
CREATE SCHEMA rd.s1
CREATE SCHEMA rd.s2
CREATE TABLE rd.s1.my_tbl AS SELECT 42 i
CREATE TABLE rd.s2.my_tbl AS SELECT 84 i
# file: test/sql/storage/read_duckdb/read_duckdb_suggested.test
# query
DETACH suggested
# file: test/sql/storage/read_duckdb/read_duckdb_transaction.test
# setup
CREATE TABLE rd.my_tbl AS SELECT 42 i
# query
CREATE TABLE rd.my_tbl AS SELECT 42 i
# file: test/sql/storage/read_duckdb/read_duckdb_union_by_name.test
# setup
CREATE TABLE rd.my_tbl AS SELECT 200 i, 84 col2
# query
CREATE TABLE rd.my_tbl AS SELECT 100 i, 84 col1
CREATE TABLE rd.my_tbl AS SELECT 200 i, 84 col2
# file: test/sql/storage/update/dictionary_update_null.test
# setup
CREATE OR REPLACE TABLE 'everflow_daily' AS SELECT case when i%10=0 THEN uuid()::VARCHAR ELSE 'N/A' END sub4 FROM range(10000) t(i)
# query
SET force_compression='dictionary'
CREATE OR REPLACE TABLE 'everflow_daily' AS SELECT case when i%10=0 THEN uuid()::VARCHAR ELSE 'N/A' END sub4 FROM range(10000) t(i)
UPDATE everflow_daily SET sub4 = NULL WHERE sub4 = 'N/A'
select count(*) from everflow_daily where sub4 = 'N/A'
# file: test/sql/storage/update/test_store_null_updates.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (11, 22), (NULL, 22), (12, 21)
UPDATE test SET b=b+1 WHERE a=11
UPDATE test SET b=NULL WHERE a=11
# file: test/sql/storage/update/test_store_updates.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (11, 22), (13, 22), (12, 21)
# file: test/sql/storage/update/wal_restart_update_insert.test
# setup
CREATE TABLE test(i INTEGER PRIMARY KEY, j INTEGER)
# query
INSERT INTO test SELECT r, r FROM range(2000) t(r)
INSERT INTO test SELECT r, r FROM range(2000,200000) t(r)
UPDATE test SET j=j+1
INSERT INTO test SELECT r, r FROM range(200000,400000) t(r)
select count(*) FROM test
# file: test/sql/storage/constraints/foreignkey/foreign_key_persistent.test
# setup
CREATE TABLE pk_integers (i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers (j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# query
CREATE TABLE pk_integers (i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers (j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
DELETE FROM fk_integers WHERE j = 3
# reject
DELETE FROM pk_integers WHERE i = 3
UPDATE pk_integers SET i = 5 WHERE i = 2
UPDATE fk_integers SET j = 4 WHERE j = 2
# file: test/sql/storage/constraints/foreignkey/foreign_key_persistent_memory_limit.test
# setup
CREATE TABLE pk_integers (i INTEGER PRIMARY KEY)
CREATE TABLE fk_integers (j INTEGER, FOREIGN KEY (j) REFERENCES pk_integers(i))
# query
SET memory_limit = '1925kB'
SET threads = 2
# file: test/sql/storage/delete/load_delete_modify.test
# setup
CREATE TABLE integers AS SELECT * FROM generate_series(0,599999) t(i)
# query
CREATE TABLE integers AS SELECT * FROM generate_series(0,599999) t(i)
DELETE FROM integers WHERE i%2=0
ALTER TABLE integers ADD COLUMN k INTEGER
SELECT COUNT(*), COUNT(i), COUNT(k) FROM integers
UPDATE integers SET k=i+1
SELECT COUNT(*), COUNT(i), COUNT(k), SUM(k) - SUM(i) FROM integers
DELETE FROM integers WHERE i%3=0
# file: test/sql/storage/delete/repeated_deletes.test
# setup
CREATE TABLE test (i INTEGER)
# query
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
# file: test/sql/storage/delete/test_store_deletes.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (11, 22), (12, 21), (13, 22), (12, 21)
INSERT INTO test VALUES (11, 24), (12, 25)
# file: test/sql/storage/delete/test_unchanged_deletes.test
# setup
CREATE TABLE integers AS FROM range(4) t(i)
CREATE TABLE integers2(i int)
# query
CREATE TABLE integers AS FROM range(4) t(i)
CREATE TABLE integers2(i int)
# file: test/sql/storage/delete/test_unchanged_deletes_large.test
# setup
CREATE TABLE integers AS SELECT * FROM generate_series(0,599999) t(i)
# query
INSERT INTO integers VALUES (42)
INSERT INTO integers VALUES (84)
# file: test/sql/storage/wal/test_wal_bc.test
# query
SELECT COUNT(*), SUM(i) FROM integers
# file: test/sql/storage/wal/wal_autocheckpoint_entries.test
# setup
CREATE TABLE autocheckpoint_db.delete_tbl(x INTEGER)
CREATE TABLE entry_count_db.delete_tbl(x INTEGER)
# query
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
# file: test/sql/storage/wal/wal_check_constraint.test
# setup
CREATE TABLE test(a INTEGER CHECK (a<10), b INTEGER CHECK(CASE WHEN b < 10 THEN a < b ELSE a + b < 100 END))
# query
CREATE TABLE test(a INTEGER CHECK (a<10), b INTEGER CHECK(CASE WHEN b < 10 THEN a < b ELSE a + b < 100 END))
INSERT INTO test VALUES (3, 7)
INSERT INTO test VALUES (9, 90)
# reject
INSERT INTO test VALUES (12, 13)
INSERT INTO test VALUES (5, 3)
INSERT INTO test VALUES (9, 99)
# file: test/sql/storage/wal/wal_create_index.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE UNIQUE INDEX i_index ON integers(i)
CREATE UNIQUE INDEX i_index ON integers USING art((i + j))
CREATE UNIQUE INDEX i_index ON integers USING art((j + i))
CREATE UNIQUE INDEX i_index ON integers USING art((j + i), j, i)
# query
INSERT INTO integers VALUES (1, 1), (2, 2), (3, 3)
CREATE UNIQUE INDEX i_index ON integers(i)
EXPLAIN ANALYZE SELECT i, j FROM integers WHERE i = 1
SELECT i, j FROM integers WHERE i = 1
CREATE UNIQUE INDEX i_index ON integers USING art((i + j))
SELECT i, j FROM integers WHERE i + j = 2
CREATE UNIQUE INDEX i_index ON integers USING art((j + i))
SELECT i, j FROM integers WHERE j + i = 2
CREATE UNIQUE INDEX i_index ON integers USING art((j + i), j, i)
# reject
INSERT INTO integers VALUES (1, 1)
# file: test/sql/storage/wal/wal_create_insert_drop.test
# setup
create or replace table bla as select 42
# query
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
# file: test/sql/storage/wal/wal_drop_table.test
# setup
CREATE SCHEMA test
CREATE TABLE test.test (a INTEGER, b INTEGER)
# query
CREATE SCHEMA test
CREATE TABLE test.test (a INTEGER, b INTEGER)
INSERT INTO test.test VALUES (11, 22), (13, 22)
DROP TABLE test.test
DROP SCHEMA test
# file: test/sql/storage/wal/wal_index_delete.test
# setup
CREATE TABLE tbl(a INTEGER, b VARCHAR, c DOUBLE, d TIMESTAMP)
CREATE INDEX idx_ab ON tbl(a, b)
CREATE INDEX idx_a ON tbl(a)
# query
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
# file: test/sql/storage/wal/wal_index_delete_gen.test
# setup
CREATE TABLE tbl(a BIGINT, b INT AS (2*a), c VARCHAR, d DOUBLE, e as (d + 2), f TIMESTAMP)
CREATE INDEX idx_cd ON tbl(c,d)
CREATE INDEX idx_df ON tbl(d, f)
# query
CREATE TABLE tbl(a BIGINT, b INT AS (2*a), c VARCHAR, d DOUBLE, e as (d + 2), f TIMESTAMP)
CREATE INDEX idx_cd ON tbl(c,d)
CREATE INDEX idx_df ON tbl(d, f)
INSERT INTO tbl VALUES (1, 'foo', 10.5, '2023-01-01 10:00:00'), (2, 'bar', 20.5, '2023-02-01 11:00:00'), (3, 'baz', 30.5, '2023-03-01 12:00:00')
SELECT a, b, c, d, e, f FROM tbl ORDER BY a
DELETE FROM tbl WHERE a in (2)
INSERT INTO tbl VALUES (1, 'foo', 10.5, '2023-01-01 10:00:00')
SELECT b, e FROM tbl WHERE (c,d) = ('baz', 30.5)
# file: test/sql/storage/wal/wal_index_interleaved.test
# setup
CREATE TABLE tbl(a INTEGER)
CREATE INDEX idx_a ON tbl(a)
# query
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
# file: test/sql/storage/wal/wal_index_large_batch_interleaved.test
# setup
CREATE TABLE tbl(a INTEGER)
CREATE INDEX idx_a ON tbl(a)
# query
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 25010
SELECT * FROM tbl WHERE a = 25010
SELECT * FROM tbl WHERE a = 24999
# file: test/sql/storage/wal/wal_index_replay.test
# setup
CREATE TABLE tbl(a INTEGER)
CREATE INDEX idx_a ON tbl(a)
# query
INSERT INTO tbl SELECT range FROM range(100)
INSERT INTO tbl SELECT range + 100 FROM range(50)
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 1
SELECT * FROM tbl WHERE a = 1
EXPLAIN ANALYZE SELECT * FROM tbl WHERE a = 5
SELECT * FROM tbl WHERE a = 5
INSERT INTO tbl VALUES (5)
# file: test/sql/storage/wal/wal_prepared_storage.test
# setup
CREATE TABLE t (a INTEGER)
# query
CREATE TABLE t (a INTEGER)
PREPARE p1 AS INSERT INTO t VALUES ($1)
EXECUTE p1(42)
EXECUTE p1(43)
DEALLOCATE p1
SELECT a FROM t
PREPARE p1 AS DELETE FROM t WHERE a=$1
PREPARE p1 AS UPDATE t SET a = $1
# file: test/sql/storage/wal/wal_promote_version.test
# setup
CREATE TABLE wal_promote.T AS (FROM range(10))
# query
CREATE TABLE wal_promote.T AS (FROM range(10))
DETACH wal_promote
INSERT INTO wal_promote.T VALUES (42)
# file: test/sql/storage/wal/wal_replay_consistency_add_column_current_timestamp.test
# setup
CREATE TABLE t AS SELECT range::INT AS id FROM range(10)
CREATE TABLE original AS SELECT d FROM t
# query
USE wal_replay
CREATE TABLE t AS SELECT range::INT AS id FROM range(10)
ALTER TABLE t ADD COLUMN d TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
CREATE TABLE original AS SELECT d FROM t
DETACH wal_replay
select COUNT(d) != 0 from t
select d from t
select d from original
# file: test/sql/storage/wal/wal_replay_consistency_add_column_random.test
# setup
CREATE TABLE t AS SELECT range::INT AS id FROM range(10)
CREATE TABLE original AS SELECT id, r FROM t
# query
ALTER TABLE t ADD COLUMN r DOUBLE DEFAULT RANDOM()
CREATE TABLE original AS SELECT id, r FROM t
select COUNT(r) != 0 from t
select r from t
select r from original
# file: test/sql/storage/wal/wal_sequence_uncommitted_transaction.test
# setup
CREATE SEQUENCE seq
# query
SELECT nextval('seq')
# file: test/sql/storage/wal/wal_storage_types.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE a_interval AS SELECT interval (range) year i FROM range(1,1001)
CREATE TABLE a_bool AS SELECT range%2=0 i FROM range(1000)
CREATE TABLE person ( name text, current_mood mood )
# query
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_interval WHERE i=interval 1 year
CREATE TABLE a_bool AS SELECT range%2=0 i FROM range(1000)
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM a_bool WHERE not i
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE person ( name text, current_mood mood )
INSERT INTO person VALUES ('Moe', 'happy')
select * from person
drop table person
drop TYPE mood
# reject
CREATE TABLE aliens ( name text, current_mood mood )
# file: test/sql/storage/wal/wal_store_add_column.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
SELECT k FROM test ORDER BY k
INSERT INTO test(a, b) VALUES (1, 1)
# file: test/sql/storage/wal/wal_store_alter_type.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
ALTER TABLE test ALTER b TYPE VARCHAR
INSERT INTO test VALUES (10, 'hello')
DELETE FROM test WHERE b='hello'
# file: test/sql/storage/wal/wal_store_default_sequence.test
# setup
CREATE SEQUENCE seq
CREATE TABLE test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
# query
CREATE TABLE test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
INSERT INTO test (b) VALUES (11)
SELECT * FROM test ORDER BY b
INSERT INTO test (b) VALUES (12)
INSERT INTO test (b) VALUES (13)
INSERT INTO test (b) VALUES (14)
INSERT INTO test (b) VALUES (15)
# file: test/sql/storage/wal/wal_store_defaults.test
# setup
CREATE TABLE test (a INTEGER DEFAULT 1, b INTEGER)
# query
CREATE TABLE test (a INTEGER DEFAULT 1, b INTEGER)
INSERT INTO test (b) VALUES (12), (13)
INSERT INTO test (b) VALUES (14), (15)
# file: test/sql/storage/wal/wal_store_mixed_updates_big.test
# setup
CREATE TABLE test AS SELECT -i a, -i b FROM range(100000) tbl(i)
# query
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
# file: test/sql/storage/wal/wal_store_mixed_updates_big_null.test
# setup
CREATE TABLE test AS SELECT -i a, -i b FROM range(100000) tbl(i)
# query
UPDATE test SET b=NULL WHERE a>0 AND a%2=0
SELECT COUNT(*) FROM test WHERE a>0 AND b IS NULL
SELECT COUNT(*), SUM(a), SUM(b), MIN(a), MAX(a), MIN(b), MAX(b), COUNT(b) FROM test WHERE a>0
# file: test/sql/storage/wal/wal_store_remove_column.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
ALTER TABLE test DROP COLUMN b
# file: test/sql/storage/wal/wal_store_rename_column.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
SELECT a FROM test ORDER BY a
ALTER TABLE test RENAME COLUMN a TO k
# reject
SELECT a FROM test
# file: test/sql/storage/wal/wal_store_rename_table.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
ALTER TABLE test RENAME TO new_name
SELECT a FROM new_name ORDER BY 1
# file: test/sql/storage/wal/wal_store_rename_view.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE VIEW vtest AS SELECT * FROM test
# query
CREATE VIEW vtest AS SELECT * FROM test
SELECT a FROM vtest ORDER BY a
ALTER VIEW vtest RENAME TO new_name
# file: test/sql/storage/wal/wal_store_sequences.test
# setup
CREATE SEQUENCE seq
CREATE SEQUENCE seq_cycle INCREMENT 1 MAXVALUE 3 START 2 CYCLE
CREATE SEQUENCE seq2
# query
CREATE SEQUENCE seq_cycle INCREMENT 1 MAXVALUE 3 START 2 CYCLE
SELECT nextval('seq_cycle')
CREATE SEQUENCE seq2
DROP SEQUENCE seq2
SELECT nextval('seq'), nextval('seq')
DROP SEQUENCE seq
# reject
SELECT nextval('seq2')
# file: test/sql/storage/wal/wal_store_temporary.test
# setup
CREATE TEMPORARY SEQUENCE seq2
CREATE TEMPORARY SEQUENCE seq
CREATE TABLE persistent (i INTEGER)
CREATE TEMPORARY TABLE temp.a (i INTEGER)
CREATE TEMPORARY TABLE a (i INTEGER)
CREATE TEMPORARY VIEW v2 AS SELECT 42
CREATE TEMPORARY VIEW v1 AS SELECT 42
# query
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
# reject
DELETE FROM asdf.a
# file: test/sql/storage/wal/wal_store_updates_big.test
# setup
CREATE TABLE test AS SELECT -i a, -i b FROM range(100000) tbl(i)
# query
SELECT a, b FROM test WHERE a>0 OR a IS NULL ORDER BY a
# file: test/sql/storage/wal/wal_store_updates_big_varchar.test
# setup
CREATE TABLE test AS SELECT (-i)::VARCHAR a, (-i)::VARCHAR b FROM range(100000) tbl(i)
# query
CREATE TABLE test AS SELECT (-i)::VARCHAR a, (-i)::VARCHAR b FROM range(100000) tbl(i)
INSERT INTO test VALUES ('11', '22'), (NULL, '22'), ('12', '21')
UPDATE test SET b=(b::INT+1)::VARCHAR WHERE a='11'
SELECT a, b FROM test WHERE a::INTEGER>0 OR a IS NULL ORDER BY a
UPDATE test SET b=NULL WHERE a='11'
# file: test/sql/storage/wal/wal_test_string_null_updates.test
# setup
CREATE TABLE test (a VARCHAR, b VARCHAR)
# query
CREATE TABLE test (a VARCHAR, b VARCHAR)
# file: test/sql/storage/wal/wal_timestamp_storage.test
# setup
CREATE TABLE timestamp (t TIMESTAMP)
# query
CREATE TABLE timestamp (t TIMESTAMP)
INSERT INTO timestamp VALUES ('2008-01-01 00:00:01'), (NULL), ('2007-01-01 00:00:01'), ('2008-02-01 00:00:01'), ('2008-01-02 00:00:01'), ('2008-01-01 10:00:00'), ('2008-01-01 00:10:00'), ('2008-01-01 00:00:10')
SELECT * FROM timestamp ORDER BY t
SELECT * FROM timestamp WHERE t=TIMESTAMP '2007-01-01 00:00:01' ORDER BY t
SELECT * FROM timestamp WHERE t=TIMESTAMP '2000-01-01 00:00:01' ORDER BY t
# file: test/sql/storage/wal/wal_uhugeint_storage.test
# setup
CREATE TABLE uhugeints (h UHUGEINT)
# query
INSERT INTO uhugeints VALUES (1043178439874412422424), (42), (NULL), (47289478944894789472897441242)
# file: test/sql/storage/wal/wal_view_explicit_aliases.test
# setup
CREATE SCHEMA test
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v (b,c) AS SELECT * FROM test.t
# query
set enable_view_dependencies=true
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v (b,c) AS SELECT * FROM test.t
PRAGMA table_info('test.v')
SELECT * FROM test.v
DROP TABLE test.t CASCADE
SELECT * FROM test.t
# reject
SELECT b,c FROM test.v
# file: test/sql/storage/wal/wal_view_explicit_aliases_no_view_dependencies.test
# setup
CREATE SCHEMA test
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v (b,c) AS SELECT * FROM test.t
# query
DROP TABLE test.t
# file: test/sql/storage/wal/wal_view_storage.test
# setup
CREATE TABLE test.t (a INTEGER, b INTEGER)
CREATE VIEW test.v2 AS SELECT 42
CREATE VIEW test.v AS SELECT * FROM test.t
# query
drop table test.t cascade
CREATE VIEW test.v2 AS SELECT 42
DROP VIEW test.v2
CREATE VIEW test.v AS SELECT * FROM test.t
# reject
drop table test.t
SELECT * FROM test.v2
# file: test/sql/storage/bc/internal_schemas_0102.test
# query
SELECT database_name, schema_name FROM duckdb_schemas WHERE NOT internal
# file: test/sql/storage/bc/test_broken_view_v092.test
# query
FROM duckdb_columns()
# file: test/sql/storage/bc/test_view_v092.test
# query
SHOW TABLES
FROM duckdb_views()
# file: test/sql/storage/temp_directory/max_swap_space_error.test
# setup
CREATE OR REPLACE TABLE t2 AS SELECT random() FROM range(200000)
# query
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
# reject
CREATE OR REPLACE TABLE t2 AS SELECT random() FROM range(1000000)
CREATE OR REPLACE TABLE t2 AS SELECT * FROM range(1000000)
# file: test/sql/storage/temp_directory/max_swap_space_explicit.test
# setup
CREATE TABLE t2 AS SELECT * FROM range(1000000)
# query
set max_temp_directory_size='15gb'
# file: test/sql/storage/temp_directory/max_swap_space_inmemory.test
# setup
CREATE TABLE t2 AS SELECT * FROM range(1000000)
# query
set temp_directory=''
PRAGMA memory_limit='3MB'
select current_setting('max_temp_directory_size') a where a == '0 bytes'
reset max_temp_directory_size
reset temp_directory
# file: test/sql/storage/temp_directory/max_swap_space_persistent.test
# setup
CREATE TABLE t2 AS SELECT * FROM range(1000000)
# query
SELECT current_setting('temp_directory').split('/')[-1]
SET temp_directory=''
PRAGMA memory_limit='3MiB'
SELECT current_setting('max_temp_directory_size')
SET max_temp_directory_size='15GB'
SELECT current_setting('max_temp_directory_size') a WHERE a == '0 bytes'
RESET max_temp_directory_size
SELECT current_setting('max_temp_directory_size') a where a == '0 bytes'
# file: test/sql/storage/temp_directory/max_swap_space_unlimited.test
# query
PRAGMA max_temp_directory_size='-1'
# file: test/sql/storage/temp_directory/temp_directory_null.test
# query
select value from duckdb_settings() where name = 'temp_directory'
set temp_directory=null
# file: test/sql/storage/temp_directory/test_default_temp_directory.test
# setup
CREATE TEMPORARY TABLE t AS FROM range(1_000_000)
# query
SET memory_limit='2MB'
CREATE TEMPORARY TABLE t AS FROM range(1_000_000)
# file: test/sql/storage/catalog/store_collate.test
# setup
CREATE TABLE collate_test(s VARCHAR COLLATE NOACCENT)
# query
CREATE TABLE collate_test(s VARCHAR COLLATE NOACCENT)
INSERT INTO collate_test VALUES ('Mühleisen'), ('Hëllö')
SELECT * FROM collate_test WHERE s='Muhleisen'
SELECT * FROM collate_test WHERE s='mühleisen'
SELECT * FROM collate_test WHERE s='Hello'
# file: test/sql/storage/catalog/test_macro_storage.test
# setup
CREATE MACRO plus1(a) AS a+1
CREATE MACRO plus2(a, b := 2) AS a + b
CREATE MACRO addition(a) AS a, (a,b) AS a + b
# query
CREATE MACRO plus1(a) AS a+1
SELECT plus1(2)
DROP MACRO plus1
CREATE MACRO plus2(a, b := 2) AS a + b
SELECT plus2(3)
SELECT plus2(4)
CREATE MACRO addition(a) AS a, (a,b) AS a + b
SELECT addition(2), addition(1, 2)
# file: test/sql/storage/catalog/test_not_distinct_from_default.test
# setup
CREATE SEQUENCE seq
CREATE TABLE test_default (a BOOL DEFAULT nextval('seq') is not distinct from nextval('seq'), b INTEGER)
# query
CREATE TABLE test_default (a BOOL DEFAULT nextval('seq') is not distinct from nextval('seq'), b INTEGER)
INSERT INTO test_default (b) VALUES (2), (4), (6)
select * from test_default
# file: test/sql/storage/catalog/test_not_null_constraint.test
# setup
CREATE TABLE test(a INTEGER NOT NULL)
# query
CREATE TABLE test(a INTEGER NOT NULL)
# file: test/sql/storage/catalog/test_store_alter_type.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
SELECT * FROM test WHERE b='hello'
# file: test/sql/storage/catalog/test_store_temporary.test
# setup
CREATE TEMPORARY SEQUENCE seq2
CREATE TEMPORARY SEQUENCE seq
CREATE TABLE persistent (i INTEGER)
CREATE TEMPORARY TABLE temp.a (i INTEGER)
CREATE TEMPORARY TABLE a (i INTEGER)
CREATE TEMPORARY VIEW v2 AS SELECT 42
CREATE TEMPORARY VIEW v1 AS SELECT 42
# query
SELECT * FROM persistent
CREATE TEMPORARY TABLE a (i INTEGER)
# file: test/sql/storage/catalog/test_table_macro_storage.test
# setup
CREATE TABLE test_tbl (id INT, name string, height double)
CREATE MACRO xt(a, _name) as TABLE SELECT * FROM test_tbl WHERE id<=a or name = _name
CREATE TEMPORARY MACRO my_seq(start , finish, stride:=3) as TABLE SELECT * FROM generate_series(start , finish , stride)
CREATE MACRO my_range(rend) AS TABLE SELECT * FROM range(rend)
# query
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
# reject
SELECT * from my_seq(0,10,2)
SELECT * FROM my_seq(0,3,2)
# file: test/sql/storage/catalog/generated_columns/virtual/basic.test
# setup
CREATE TABLE tbl ( price INTEGER, gcol AS (price) )
# query
CREATE TABLE tbl ( price INTEGER, gcol AS (price) )
SELECT gcol FROM tbl
# file: test/sql/storage/catalog/generated_columns/virtual/constraints.test
# setup
CREATE TABLE base ( price INTEGER PRIMARY KEY )
CREATE TABLE tbl ( gcol2 AS (gcol1), price INTEGER DEFAULT (5), gcol1 AS (price), )
# query
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
# reject
INSERT INTO tbl VALUES (5, 'test')
ALTER TABLE tbl DROP COLUMN gcol1
# file: test/sql/storage/mix/test_update_delete_string.test
# setup
CREATE TABLE test (a INTEGER, b STRING)
# query
SELECT * FROM test WHERE a IS NULL
UPDATE test SET b=NULL WHERE a IS NULL
INSERT INTO test VALUES (12, NULL)
UPDATE test SET b='test123' WHERE a=12
# file: test/sql/storage/mix/updates_deletes_big_table.test
# setup
CREATE TABLE test (a INTEGER)
# query
INSERT INTO test SELECT a FROM range(0, 1000) tbl1(a), repeat(0, 100) tbl2(b)
UPDATE test SET a=2000 WHERE a=1
DELETE FROM test WHERE a=2 OR a=17
SELECT SUM(a), COUNT(a) FROM test
SELECT COUNT(a) FROM test WHERE a=0
SELECT COUNT(a) FROM test WHERE a=1
SELECT COUNT(a) FROM test WHERE a=2
SELECT COUNT(a) FROM test WHERE a=17
# file: test/sql/storage/mix/updates_deletes_persistent_segments.test
# setup
CREATE TABLE test(a INTEGER, b INTEGER)
# query
CREATE TABLE test(a INTEGER, b INTEGER)
INSERT INTO test VALUES (1, 3), (NULL, NULL)
UPDATE test SET b=4 WHERE a=1
UPDATE test SET a=4, b=4 WHERE a=1
UPDATE test SET b=5, a=6 WHERE a=4
DELETE FROM test WHERE a=2
UPDATE test SET b=7 WHERE a=3
# file: test/sql/storage/extensions/extension_default.test
# setup
CREATE TABLE t1(v VARCHAR DEFAULT CURRENT_SCHEMA())
# query
CREATE TABLE t1(v VARCHAR DEFAULT CURRENT_SCHEMA())
INSERT INTO t1 VALUES (DEFAULT)
# file: test/sql/storage/extensions/extension_views.test
# setup
CREATE VIEW v1 AS SELECT current_schema()
# query
CREATE VIEW v1 AS SELECT current_schema()
# file: test/sql/storage/alter/alter_add_col_defaultexpr_sequence.test
# setup
CREATE SEQUENCE seq
# query
attach ':memory:' as db1
use db1
CREATE TABLE db1.tbl (id INTEGER DEFAULT nextval('seq'), s VARCHAR)
ALTER TABLE db1.tbl ADD COLUMN m INTEGER DEFAULT nextval('seq')
# file: test/sql/storage/alter/alter_column_constraint.test
# setup
CREATE TABLE IF NOT EXISTS a(id INT PRIMARY KEY)
# query
CREATE TABLE IF NOT EXISTS a(id INT PRIMARY KEY)
INSERT INTO a(id) VALUES (1)
ALTER TABLE a ADD COLUMN c REAL
ALTER TABLE a ALTER COLUMN c SET DEFAULT 10
ALTER TABLE a RENAME c TO d
ALTER TABLE a RENAME TO b
ALTER TABLE b DROP d
INSERT INTO b(id) VALUES (2)
# reject
INSERT INTO b(id) VALUES (1)
# file: test/sql/storage/reclaim_space/test_reclaim_space_update_large_string.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test VALUES (repeat('a', 1000000))
SELECT LENGTH(SUBSTRING(a, 0, 1000000)) FROM test
UPDATE test SET a=concat(a, 'a')
select total_blocks from pragma_database_size()
# file: test/sql/storage/external_file_cache/external_file_cache_validate.test
# query
set enable_external_file_cache=true
set prefetch_all_parquet_files=true
select current_setting('validate_external_file_cache')
set validate_external_file_cache='VALIDATE_REMOTE'
set validate_external_file_cache='NO_VALIDATION'
set validate_external_file_cache='VALIDATE_ALL'
# reject
set validate_external_file_cache='INVALID_VALUE'
# file: test/sql/storage/compression/compression_selection.test
# setup
CREATE TABLE test_rle (a INTEGER)
CREATE TABLE test_constant (a INTEGER)
CREATE TABLE test_dict (a VARCHAR)
CREATE TABLE test_bp (a INTEGER)
# query
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
# file: test/sql/storage/compression/compression_selection_dict_fsst.test
# setup
CREATE TABLE test_rle (a INTEGER)
CREATE TABLE test_constant (a INTEGER)
CREATE TABLE test_dict (a VARCHAR)
CREATE TABLE test_bp (a INTEGER)
# query
SELECT compression FROM pragma_storage_info('test_bp') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
# file: test/sql/storage/compression/simple_compression.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
INSERT INTO test VALUES (11, 22), (11, 22), (12, 21), (NULL, NULL)
SELECT SUM(a), SUM(b) FROM test
# file: test/sql/storage/compression/test_using_compression.test
# setup
CREATE OR REPLACE TABLE t( x VARCHAR USING COMPRESSION Dictionary )
# query
CREATE OR REPLACE TABLE t( x VARCHAR USING COMPRESSION Dictionary )
# reject
CREATE OR REPLACE TABLE t( x VARCHAR USING COMPRESSION chimp )
CREATE OR REPLACE TABLE t( x BIGINT USING COMPRESSION Dictionary )
create table foo (str VARCHAR USING COMPRESSION 'dict_fsst')
# file: test/sql/storage/compression/roaring/roaring_analyze_array.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i%25=0 then 1337 else null end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
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
# file: test/sql/storage/compression/roaring/roaring_analyze_bitset.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then 1337 else null end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then 1337 else null end FROM range(getvariable('dataset_size')) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_analyze_run.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then null else 1337 end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then null else 1337 end FROM range(getvariable('dataset_size')) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_array_simple.test
# setup
CREATE TABLE test (a BIGINT)
# query
CREATE TABLE test (a BIGINT)
INSERT INTO test SELECT case when i%25=0 then 1337 else null end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' and compression != 'Roaring'
select count(*) from test WHERE a IS NOT NULL
select sum(a), min(a), max(a) from test
delete from test
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then 1337 else null end from intermediates
# file: test/sql/storage/compression/roaring/roaring_bitset_simple.test
# setup
CREATE TABLE test (a BIGINT)
# query
INSERT INTO test SELECT CASE WHEN i % 3 = 0 THEN 1337 ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS NOT NULL
SELECT SUM(a), MIN(a), MAX(a) FROM test
# file: test/sql/storage/compression/roaring/roaring_bool_analyze_array.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i%25=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
PRAGMA force_compression='BitPacking'
CREATE TABLE test_uncompressed AS SELECT case when i%25=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_uncompressed') and message.contains('BOOLEAN') and message.contains('BITPACKING')
SELECT message.split(': ')[2]::INTEGER FROM duckdb_logs where message.starts_with('ColumnDataCheckpointer FinalAnalyze') and message.contains('test_roaring') and message.contains('BOOLEAN') and message.contains('ROARING')
# file: test/sql/storage/compression/roaring/roaring_bool_analyze_bitset.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
CREATE TABLE test_uncompressed AS SELECT case when i%3=0 then true else false end FROM range(getvariable('dataset_size')) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_bool_analyze_run.test
# setup
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then false else true end FROM range(getvariable('dataset_size')) tbl(i)
CREATE TABLE test_roaring AS select * from test_uncompressed
# query
CREATE TABLE test_uncompressed AS SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then false else true end FROM range(getvariable('dataset_size')) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_bool_array_simple.test
# setup
CREATE TABLE test (a BOOL)
# query
CREATE TABLE test (a BOOL)
INSERT INTO test SELECT case when i%25=0 then true else false end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' and compression != 'Roaring'
select count(*) from test WHERE a IS true
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then true else false end from intermediates
# file: test/sql/storage/compression/roaring/roaring_bool_array_simple_w_null.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT case when i%50=0 then false when i%25=0 then true else NULL end FROM range(0,10_000) tbl(i)
select count(*) from test WHERE a IS NULL
select count(*) from test WHERE a IS false
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 then false when i = 1500 or i = 2000 then true else null end from intermediates
# file: test/sql/storage/compression/roaring/roaring_bool_bitset_simple.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT CASE WHEN i % 3 = 0 THEN true ELSE false END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOL' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS true
SELECT COUNT(*) FROM test WHERE a IS false
# file: test/sql/storage/compression/roaring/roaring_bool_bitset_simple_w_null.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT CASE WHEN i % 6 = 0 THEN true WHEN i % 3 = 0 THEN false ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' AND compression != 'Roaring'
SELECT COUNT(*) FROM test WHERE a IS null
# file: test/sql/storage/compression/roaring/roaring_bool_fetch_row.test
# setup
CREATE TABLE test ( a BOOL )
# query
CREATE TABLE test ( a BOOL )
pragma force_compression='roaring'
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VALIDITY' and compression != 'Constant'
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then false else true end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%3=0 then true else false end FROM range(0,10000) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_bool_first_is_null.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test VALUES (null), (true), (true), (true), (true), (true), (true), (true), (null), (null), (null), (null), (null), (null), (null), (null), (false), (false), (false), (false), (false), (false), (false), (false), (null), (true), (null), (false), (false), (false), (false), (false)
# file: test/sql/storage/compression/roaring/roaring_bool_inverted_array_simple.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT case when i%25=0 then false else true end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then false else true end from intermediates
# file: test/sql/storage/compression/roaring/roaring_bool_inverted_run_simple.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then true else false end FROM range(0,10000) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_bool_run_simple.test
# setup
CREATE TABLE test (a BOOL)
# query
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) INSERT INTO test SELECT case when (i >= 0 and i < 110) or (i >= 1500 and i < 1800) or (i >= 2000) then false else true end FROM intermediates
# file: test/sql/storage/compression/roaring/roaring_bool_run_simple_w_null.test
# setup
CREATE TABLE test (a BOOL)
# query
INSERT INTO test SELECT CASE WHEN i % 1000 < 100 THEN true WHEN i % 1000 < 200 THEN false ELSE NULL END FROM range(0, 10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOL' and compression != 'Roaring'
select count(*) from test WHERE a IS null
# file: test/sql/storage/compression/roaring/roaring_bool_smaller_than_vector.test
# setup
CREATE TABLE test (a BOOL)
# query
set checkpoint_threshold = '10mb'
INSERT INTO test SELECT case when i%25=0 then true else false end FROM range(0,1025) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_bool_uncompressed_under_v1_5_0.test
# setup
CREATE TABLE test (a BOOL)
# query
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BOOLEAN' and compression != 'Uncompressed'
# file: test/sql/storage/compression/roaring/roaring_fetch_row.test
# setup
CREATE TABLE test ( a INT )
# query
CREATE TABLE test ( a INT )
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then null else 1337 end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%3=0 then 1337 else null end FROM range(0,10000) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_inverted_array_simple.test
# setup
CREATE TABLE test (a BIGINT)
# query
INSERT INTO test SELECT case when i%25=0 then null else 1337 end FROM range(0,10000) tbl(i)
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) insert into test select case when i = 0 or i = 6 or i = 1000 or i = 1500 or i = 2000 then null else 1337 end from intermediates
# file: test/sql/storage/compression/roaring/roaring_inverted_run_simple.test
# setup
CREATE TABLE test (a BIGINT)
# query
INSERT INTO test SELECT case when i = 0 or (i % 512 != 0 and (i % 512) < 350 or (i % 512) > 450) then 1337 else null end FROM range(0,10000) tbl(i)
# file: test/sql/storage/compression/roaring/roaring_run_simple.test
# setup
CREATE TABLE test (a BIGINT)
# query
with intermediates as ( select i % 2048 as i from range(0, 10_000) t(i) ) INSERT INTO test SELECT case when (i >= 0 and i < 110) or (i >= 1500 and i < 1800) or (i >= 2000) then null else 1337 end FROM intermediates
# file: test/sql/storage/compression/roaring/roaring_smaller_than_vector.test
# setup
CREATE TABLE test (a BIGINT)
# query
INSERT INTO test SELECT case when i%25=0 then 1337 else null end FROM range(0,1025) tbl(i)
# file: test/sql/storage/compression/fsst/fsst_disable_compression.test
# setup
CREATE TABLE test AS SELECT concat('longprefix', i) FROM range(30000) t(i)
# query
CREATE TABLE test AS SELECT concat('longprefix', i) FROM range(30000) t(i)
SELECT DISTINCT compression FROM pragma_storage_info('test') where segment_type = 'VARCHAR'
SET disabled_compression_methods='fsst'
SELECT BOOL_OR(compression ILIKE 'fsst%') FROM pragma_storage_info('test')
# file: test/sql/storage/compression/fsst/fsst_storage_info.test
# setup
CREATE TABLE test (a VARCHAR, b VARCHAR)
# query
PRAGMA force_compression = 'fsst'
INSERT INTO test VALUES ('11', '22'), ('11', '22'), ('12', '21'), (NULL, NULL)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'VARCHAR' LIMIT 1
# file: test/sql/storage/compression/fsst/issue_5675.test
# setup
CREATE TABLE TEST (col VARCHAR)
CREATE TABLE TEST2 as SELECT * FROM TEST
# query
pragma threads=1
CREATE TABLE TEST (col VARCHAR)
INSERT INTO TEST SELECT '' FROM range(0,100000) tbl(i)
pragma force_compression='fsst'
CREATE TABLE TEST2 as SELECT * FROM TEST
# file: test/sql/storage/compression/fsst/issue_5759.test
# setup
CREATE TABLE trigger5759 AS SELECT CASE WHEN RANDOM() > 0.95 THEN repeat('ab', 1500) ELSE 'c' END FROM range(0,1000)
# query
CREATE TABLE trigger5759 AS SELECT CASE WHEN RANDOM() > 0.95 THEN repeat('ab', 1500) ELSE 'c' END FROM range(0,1000)
# file: test/sql/storage/compression/bitpacking/bitpacking_constant_delta.test
# setup
CREATE TABLE test (c INT64)
# query
PRAGMA force_compression = 'bitpacking'
SELECT compression FROM pragma_storage_info('test') where segment_type != 'VALIDITY' and compression != 'BitPacking'
CREATE TABLE test (c INT64)
INSERT INTO test SELECT i from range(0,130000) tbl(i)
SELECT avg(c) FROM test
# file: test/sql/storage/compression/bitpacking/bitpacking_delta_for.test
# setup
create table aux as select range::INT x from range(-2_000_000_000, 2_000_000_000, 2_000_000)
create table tt as select (x + if (random() > 0.5, 1, -1)) x from aux
# query
PRAGMA force_compression='bitpacking'
create table aux as select range::INT x from range(-2_000_000_000, 2_000_000_000, 2_000_000)
create table tt as select (x + if (random() > 0.5, 1, -1)) x from aux
select compression from pragma_storage_info('tt') where segment_type != 'VALIDITY'
# file: test/sql/storage/compression/bitpacking/bitpacking_filter_pushdown.test
# setup
CREATE TABLE test (id VARCHAR, col INTEGER)
# query
CREATE TABLE test (id VARCHAR, col INTEGER)
INSERT INTO test SELECT i::VARCHAR id, i b FROM range(10000) tbl(i)
INSERT INTO test SELECT i::VARCHAR id, 1337 FROM range(20000, 30000) tbl(i)
INSERT INTO test SELECT i::VARCHAR id, i b FROM range(30000,40000) tbl(i)
SELECT compression FROM pragma_storage_info('test') where segment_type = 'INTEGER' and compression != 'BitPacking'
SELECT SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE col=1337
SELECT MIN(id), MAX(id), SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE id='5000'
SELECT MIN(id), MAX(id), SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE id::INT64%1000=0
# file: test/sql/storage/compression/bitpacking/bitpacking_hugeint.test
# setup
CREATE TABLE test (id VARCHAR, a HUGEINT)
# query
PRAGMA force_bitpacking_mode='constant'
CREATE TABLE test (id VARCHAR, a HUGEINT)
select a from test limit 5
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'HUGEINT'
# file: test/sql/storage/compression/bitpacking/bitpacking_mode.test
# query
SET force_compression = 'bitpacking'
SELECT current_setting('force_bitpacking_mode')
# reject
SET force_bitpacking_mode='xxx'
# file: test/sql/storage/compression/bitpacking/bitpacking_nulls.test
# setup
CREATE TABLE test (a BIGINT)
# query
INSERT INTO test SELECT case when i%5=0 then null else 1337 end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%5=0 then null else i end FROM range(0,10000) tbl(i)
INSERT INTO test SELECT case when i%5=0 then null else i//2 end FROM range(0,10000) tbl(i)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BIGINT' and compression != 'BitPacking'
# file: test/sql/storage/compression/bitpacking/bitpacking_simple.test
# setup
CREATE TABLE test (id VARCHAR, a BIGINT)
# query
CREATE TABLE test (id VARCHAR, a BIGINT)
INSERT INTO test SELECT i::VARCHAR, -i FROM range(0,10000) tbl(i)
INSERT INTO test SELECT i::VARCHAR, 13371337 FROM range(0,10000) tbl(i)
select a from test limit 5 offset 12000
select avg(a) from test
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'BIGINT'
# file: test/sql/storage/compression/bitpacking/bitpacking_simple_hugeint.test
# setup
CREATE TABLE test (id VARCHAR, a HUGEINT)
# query
INSERT INTO test SELECT i::VARCHAR, -i::HUGEINT + -1234567891011121314151617180000::HUGEINT FROM range(0, 10000) tbl(i)
# file: test/sql/storage/compression/bitpacking/bitpacking_size_calculation.test
# query
pragma force_compression='bitpacking'
CREATE OR REPLACE TABLE toy_table AS SELECT * FROM 'https://github.com/duckdb/duckdb-data/releases/download/v1.0/bp_bug.parquet'
# file: test/sql/storage/compression/bitpacking/bitpacking_storage_info.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE OR REPLACE TABLE test_bp (a INTEGER)
# query
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'INTEGER' LIMIT 1
INSERT INTO test_bp SELECT 1 FROM range(0, 10000) tbl(i)
INSERT INTO test_bp SELECT 2 FROM range(0, 10000) tbl(i)
SELECT segment_info FROM pragma_storage_info('test_bp') WHERE segment_type NOT IN ('VALIDITY')
PRAGMA force_bitpacking_mode = 'delta_for'
CREATE OR REPLACE TABLE test_bp (a INTEGER)
INSERT INTO test_bp SELECT 3*(i // 1000) + (i%10) FROM range(0, 10000) tbl(i)
# reject
SET SESSION force_bitpacking_mode = 'delta_for'
RESET SESSION force_bitpacking_mode
# file: test/sql/storage/compression/bitpacking/bitpacking_table_copy.test
# setup
CREATE TABLE test (a integer)
CREATE TABLE test_2 AS SELECT a FROM test
# query
CREATE TABLE test (a integer)
INSERT INTO test SELECT i FROM range(0,150000) tbl(i)
CREATE TABLE test_2 AS SELECT a FROM test
select sum(a) from test
select sum(a) from test_2
drop table test_2
# file: test/sql/storage/compression/bitpacking/bitpacking_uhugeint.test
# setup
CREATE TABLE test (id VARCHAR, a UHUGEINT)
# query
CREATE TABLE test (id VARCHAR, a UHUGEINT)
SELECT compression FROM pragma_storage_info('test') WHERE segment_type ILIKE 'UHUGEINT'
# file: test/sql/storage/compression/bitpacking/force_bitpacking.test
# setup
CREATE TABLE test_bp (a INTEGER)
# query
INSERT INTO test_bp SELECT 1 FROM range(0, 1000) tbl(i)
INSERT INTO test_bp SELECT 2 FROM range(0, 1000) tbl(i)
# file: test/sql/storage/compression/bitpacking/struct_bitpacking.test
# setup
CREATE TABLE test (s ROW(a INTEGER))
# query
CREATE TABLE test (s ROW(a INTEGER))
SELECT SUM(s['a']), MIN(s['a']), MAX(s['a']), COUNT(*) FROM test
# file: test/sql/storage/compression/alp/alp_corrupted_file.test
# query
SELECT compression FROM pragma_storage_info('alp') WHERE compression='ALP'
# reject
SELECT * FROM alp LIMIT 1
# file: test/sql/storage/compression/alp/alp_corrupted_offsets.test
# query
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE compression='ALP'
# reject
SELECT * FROM random_alp_double LIMIT 1
# file: test/sql/storage/compression/alp/alp_corrupted_vector_size.test
# query
SELECT compression FROM pragma_storage_info('two_alp') WHERE compression='ALP'
# reject
SELECT * FROM two_alp LIMIT 1
# file: test/sql/storage/compression/alp/alp_inf_null_nan.test
# query
select d, f from tbl1_uncompressed
select d, f from tbl1_alp
select d, f from tbl2_uncompressed
select d, f from tbl2_alp
select d, f from tbl3_uncompressed
select d, f from tbl3_alp
# file: test/sql/storage/compression/alp/alp_list_skip.test
# setup
create or replace table list_doubles as select 5700 i, [5700.0] l UNION ALL select i, CASE WHEN i%128=0 THEN [i::DOUBLE] ELSE []::DOUBLE[] END as data from range(10000) tbl(i) union all select 5700, [i] FROM range(100) tbl(i)
# query
create or replace table list_doubles as select 5700 i, [5700.0] l UNION ALL select i, CASE WHEN i%128=0 THEN [i::DOUBLE] ELSE []::DOUBLE[] END as data from range(10000) tbl(i) union all select 5700, [i] FROM range(100) tbl(i)
SELECT * FROM list_doubles WHERE i=5700
# file: test/sql/storage/compression/alp/alp_min_max.test
# query
PRAGMA force_compression='alp'
DROP TABLE all_types
# file: test/sql/storage/compression/alp/alp_negative_numbers.test
# setup
create table random_alp_double as select * from random_double
# query
SELECT compression FROM pragma_storage_info('random_double') WHERE segment_type == 'double' AND compression != 'Uncompressed'
create table random_alp_double as select * from random_double
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE segment_type == 'double' AND compression != 'ALP'
select * from random_double
select * from random_alp_double
# file: test/sql/storage/compression/alp/alp_simple.test
# setup
create table random_double as select round(random(), 6)::DOUBLE as data from range(1024) tbl(i)
create table random_alp_double as select * from random_double
# query
create table random_double as select round(random(), 6)::DOUBLE as data from range(1024) tbl(i)
# file: test/sql/storage/compression/alp/alp_simple_float.test
# setup
create table random_float as select round(random(), 6)::FLOAT as data from range(1024) tbl(i)
create table random_alp_float as select * from random_float
# query
create table random_float as select round(random(), 6)::FLOAT as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_float') WHERE segment_type == 'float' AND compression != 'Uncompressed'
create table random_alp_float as select * from random_float
SELECT compression FROM pragma_storage_info('random_alp_float') WHERE segment_type == 'float' AND compression != 'ALP'
select * from random_float
select * from random_alp_float
# file: test/sql/storage/compression/alp/alp_zeros.test
# setup
create table random_double as select 0::DOUBLE as data from range(1024) tbl(i)
create table random_alp_double as select * from random_double
# query
create table random_double as select 0::DOUBLE as data from range(1024) tbl(i)
# file: test/sql/storage/compression/patas/patas_corrupted_file.test
# query
SELECT compression FROM pragma_storage_info('temperatures_double') WHERE compression='Patas'
# reject
SELECT * FROM temperatures_double LIMIT 1
# file: test/sql/storage/compression/rle/force_rle.test
# setup
CREATE TABLE test_rle (a INTEGER)
# query
PRAGMA force_compression = 'rle'
INSERT INTO test_rle SELECT i FROM range(0, 2000) tbl(i)
SELECT compression FROM pragma_storage_info('test_rle') WHERE segment_type ILIKE 'INTEGER'
# file: test/sql/storage/compression/rle/rle_bool.test
# setup
CREATE TABLE test (a BOOLEAN)
# query
CREATE TABLE test (a BOOLEAN)
INSERT INTO test select false from range(2048)
INSERT INTO test select true from range(2048)
SELECT COUNT(*) FROM test WHERE a=false
# file: test/sql/storage/compression/rle/rle_constant.test
# setup
CREATE TABLE test (a INTEGER)
# query
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
# file: test/sql/storage/compression/rle/rle_corrupted_count_offset.test
# query
SELECT compression FROM pragma_storage_info('t') WHERE compression='RLE'
# reject
SELECT * FROM t LIMIT 1
# file: test/sql/storage/compression/rle/rle_filter.test
# setup
CREATE TABLE tbl AS SELECT i id, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
# query
CREATE TABLE tbl AS SELECT i id, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
SELECT * FROM tbl WHERE id = 5040 AND rle_val=100
SELECT * FROM tbl WHERE id = 5040 AND substr(rle_val::VARCHAR, 1, 3)='100'
SELECT * FROM tbl WHERE id >= 5020 AND rle_val=100
SELECT * FROM tbl WHERE rle_val=100
# file: test/sql/storage/compression/rle/rle_filter_pushdown.test
# setup
CREATE TABLE test (id VARCHAR, col INTEGER)
# query
INSERT INTO test SELECT i::VARCHAR id, 1 b FROM range(5000) tbl(i)
INSERT INTO test SELECT (5000 + i)::VARCHAR id, 2 b FROM range(5000) tbl(i)
SELECT SUM(col), MIN(col), MAX(col), COUNT(*) FROM test WHERE col=2
# file: test/sql/storage/compression/rle/rle_index_fetch.test
# setup
CREATE TABLE test(id INTEGER PRIMARY KEY, col INTEGER)
# query
CREATE TABLE test(id INTEGER PRIMARY KEY, col INTEGER)
# file: test/sql/storage/compression/rle/rle_medium.test
# setup
CREATE TABLE test (a INTEGER)
# query
SELECT SUM(a), MIN(a), MAX(a), COUNT(*) FROM test
# file: test/sql/storage/compression/rle/rle_nulls_edge_case.test
# setup
CREATE TABLE integers(i INTEGER)
# query
PRAGMA force_compression='RLE'
INSERT INTO integers SELECT NULL FROM range(65535)
INSERT INTO integers SELECT 1
INSERT INTO integers SELECT 2
INSERT INTO integers SELECT 3
SELECT MIN(i), MAX(i), COUNT(*), COUNT(i) FROM integers
# file: test/sql/storage/compression/rle/rle_select.test
# setup
CREATE TABLE tbl AS SELECT i id, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
CREATE TABLE tbl2 AS SELECT i id, i%5 id_modulo, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
# query
SELECT * FROM tbl WHERE id >= 75 and id <= 125 and id%4=0
SELECT * FROM tbl WHERE id >= 33380 and id <= 33410 and id%4=0
CREATE TABLE tbl2 AS SELECT i id, i%5 id_modulo, i // 50 rle_val, case when i%8=0 then null else i // 50 end rle_val_null FROM range(100000) t(i)
SELECT COUNT(*), SUM(rle_val), MIN(rle_val), MAX(rle_val), SUM(rle_val_null), COUNT(rle_val_null) FROM tbl2 WHERE id >= 1500 and id <= 2500 AND id_modulo=3
SELECT COUNT(*), SUM(rle_val), MIN(rle_val), MAX(rle_val), SUM(rle_val_null), COUNT(rle_val_null) FROM tbl2 WHERE id >= 1500 and id <= 19500 AND id_modulo<=2
# file: test/sql/storage/compression/rle/rle_select_list.test
# setup
create table tbl as SELECT * FROM ( VALUES (['first name', 'last name', 'username'], 60), (['first name'], 0), (['username'], 0), (['first name', 'last name', 'username'], 0), (['first name', 'last name', 'username'], 0), (['username'], 0), (['username'], 0) ) AS t(attributes, minutes_duration)
# query
pragma force_compression='rle'
create table tbl as SELECT * FROM ( VALUES (['first name', 'last name', 'username'], 60), (['first name'], 0), (['username'], 0), (['first name', 'last name', 'username'], 0), (['first name', 'last name', 'username'], 0), (['username'], 0), (['username'], 0) ) AS t(attributes, minutes_duration)
SELECT "minutes_duration" FROM tbl WHERE NOT list_sort(['first name']) = tbl."attributes" ORDER BY ALL
# file: test/sql/storage/compression/dictionary/dictionary_storage_info.test
# setup
CREATE TABLE test (a VARCHAR, b VARCHAR)
# query
PRAGMA force_compression = 'dictionary'
# file: test/sql/storage/compression/dictionary/fetch_row.test
# setup
CREATE TABLE test ( a INTEGER, b VARCHAR )
# query
CREATE TABLE test ( a INTEGER, b VARCHAR )
INSERT INTO test (a, b) SELECT x AS a, CASE x % 5 WHEN 0 THEN 'aaaa' WHEN 1 THEN 'bbbb' WHEN 2 THEN 'cccc' WHEN 3 THEN 'dddd' WHEN 4 THEN NULL END AS b FROM range(10_000) t(x)
select distinct b from test order by a % 5
# file: test/sql/storage/compression/dictionary/force_dictionary.test
# setup
CREATE TABLE test_dict (a VARCHAR)
# query
INSERT INTO test_dict SELECT i::VARCHAR FROM range(0, 2000) tbl(i)
# file: test/sql/storage/compression/zstd/fetch_row.test
# setup
CREATE TABLE big_string ( a VARCHAR, id INT )
# query
SET storage_compatibility_version='v1.2.0'
CREATE TABLE big_string ( a VARCHAR, id INT )
INSERT INTO big_string values (concat(range(0,500000)::VARCHAR), 5)
SELECT a[1], strlen(a) from big_string
# file: test/sql/storage/compression/zstd/nulls.test
# setup
create table tbl ( a varchar )
# query
create table tbl ( a varchar )
set variable my_string = ( select concat(range(0,1000)::VARCHAR) )
INSERT INTO tbl (a) SELECT CASE WHEN (i % 7) = 0 THEN NULL ELSE getvariable('my_string') || i END FROM range(5000) t(i)
select count(*) from tbl where a IS NULL
# file: test/sql/storage/compression/zstd/test_skipping.test
# setup
create table tbl as select i // 5_000 as num, num::VARCHAR || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
# query
create table tbl as select i // 5_000 as num, num::VARCHAR || list_reduce([uuid()::varchar for x in range(10)], lambda x, y: concat(x, y)) str from range(20_000) t(i) order by num
select str[0:1]::BIGINT from tbl where num = 1 limit 10
# file: test/sql/storage/compression/zstd/zstd.test
# setup
CREATE TABLE test (a VARCHAR)
# query
SET default_block_size = '16384'
PRAGMA force_compression = 'zstd'
INSERT INTO test VALUES ('11'), ('11'), ('12'), (NULL)
# file: test/sql/storage/compression/zstd/zstd_big_badly_compressed_list.test
# setup
CREATE TABLE zstd(big_list VARCHAR[])
# query
USE zstd
SET zstd_min_string_length = 1
SET force_compression = 'zstd'
CREATE TABLE zstd(big_list VARCHAR[])
INSERT INTO zstd select [ chr(x::INT) for x in generate_series( ord('a'), ord('z') ) ] FROM range(1_000_000)
CHECKPOINT zstd
SELECT avg(len(big_list)) FROM zstd
# file: test/sql/storage/compression/zstd/zstd_force_compression.test
# setup
CREATE TABLE zstd_data AS SELECT concat('thisisalongstring', i) str FROM range(1000) t(i)
# query
SET force_compression='zstd'
CREATE TABLE zstd_data AS SELECT concat('thisisalongstring', i) str FROM range(1000) t(i)
select count(*) from pragma_storage_info('zstd_data') where compression='ZSTD'
# file: test/sql/storage/compression/alprd/alprd_corrupted_dict_size.test
# query
SELECT compression FROM pragma_storage_info('t') WHERE segment_type = 'DOUBLE' AND compression != 'ALPRD'
# file: test/sql/storage/compression/alprd/alprd_inf_null_nan.test
# query
select d, f from tbl1_alprd
select d, f from tbl2_alprd
select d, f from tbl3_alprd
# file: test/sql/storage/compression/alprd/alprd_min_max.test
# query
PRAGMA force_compression='alprd'
# file: test/sql/storage/compression/alprd/alprd_negative_numbers.test
# setup
create table random_double as select round(cos(1 / (random() + 0.001)), 15)::DOUBLE * -1 as data from range(1024) tbl(i)
create table random_alp_double as select * from random_double
# query
create table random_double as select round(cos(1 / (random() + 0.001)), 15)::DOUBLE * -1 as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_alp_double') WHERE segment_type == 'double' AND compression != 'ALPRD'
# file: test/sql/storage/compression/alprd/alprd_simple.test
# setup
create table random_double as select random()::DOUBLE as data from range(1024) tbl(i)
create table random_alprd_double as select * from random_double
# query
create table random_double as select random()::DOUBLE as data from range(1024) tbl(i)
create table random_alprd_double as select * from random_double
SELECT compression FROM pragma_storage_info('random_alprd_double') WHERE segment_type == 'double' AND compression != 'ALPRD'
select * from random_alprd_double
# file: test/sql/storage/compression/alprd/alprd_simple_float.test
# setup
create table random_float as select random()::FLOAT as data from range(1024) tbl(i)
create table random_alp_float as select * from random_float
# query
create table random_float as select random()::FLOAT as data from range(1024) tbl(i)
SELECT compression FROM pragma_storage_info('random_alp_float') WHERE segment_type == 'float' AND compression != 'ALPRD'
# file: test/sql/storage/compression/dict_fsst/dict_fsst_test.test
# setup
create table uncompressed_data as select i, repeat( (i % 200)::INTEGER::VARCHAR, 2047 // len((i % 200)::INTEGER::VARCHAR) ) a from range(20000) t(i)
create table compressed_data as select * from uncompressed_data
# query
pragma force_compression='uncompressed'
create table uncompressed_data as select i, repeat( (i % 200)::INTEGER::VARCHAR, 2047 // len((i % 200)::INTEGER::VARCHAR) ) a from range(20000) t(i)
select * from uncompressed_data order by i
pragma force_compression='dict_fsst'
create table compressed_data as select * from uncompressed_data
select count(distinct a) from compressed_data
select * from compressed_data order by i
select count(distinct a) from compressed_data where contains(a, '11')
select count(distinct a) from compressed_data where i%10=0
# file: test/sql/storage/compression/dict_fsst/dictionary_covers_validity.test
# setup
CREATE OR REPLACE TABLE tbl AS SELECT { 'a': i, 'b': NULL::VARCHAR } col FROM range(5000) t(i) union all select { 'a': 10000, 'b': 'hello' } FROM range(2)
# query
set checkpoint_threshold='10mb'
CREATE TABLE tbl AS SELECT { 'a': i, 'b': NULL::VARCHAR } col FROM range(5000) t(i) union all select { 'a': 10000, 'b': 'hello' }
set force_compression='dict_fsst'
SELECT segment_type, compression FROM pragma_storage_info('tbl') WHERE segment_type != 'BIGINT'
set force_compression='zstd'
CREATE OR REPLACE TABLE tbl AS SELECT { 'a': i, 'b': NULL::VARCHAR } col FROM range(5000) t(i) union all select { 'a': 10000, 'b': 'hello' } FROM range(2)
select segment_type, compression from pragma_storage_info('tbl') where segment_type IN ('VARCHAR', 'VALIDITY') order by all
SELECT col FROM tbl ORDER BY col.a DESC LIMIT 3
# file: test/sql/storage/compression/dict_fsst/dictionary_storage_info.test
# setup
CREATE TABLE test (a VARCHAR, b VARCHAR)
# query
PRAGMA force_compression = 'dict_fsst'
# file: test/sql/storage/compression/dict_fsst/fetch_row.test
# setup
CREATE TABLE test (a INTEGER, b VARCHAR)
# query
INSERT INTO test (a, b) SELECT x AS a, CASE x % 5 WHEN 0 THEN 'aaaa' WHEN 1 THEN 'bbbb' WHEN 2 THEN 'cccc' WHEN 3 THEN 'this is not an inlined string' WHEN 4 THEN NULL END AS b FROM range(80) t(x)
SELECT DISTINCT b FROM test ORDER BY a % 5
# file: test/sql/storage/compression/dict_fsst/test_dict_fsst_with_smaller_block_size.test
# query
SET storage_compatibility_version='latest'
SELECT COUNT("XXX XXX/XXX") FROM db.t WHERE "XXX XXX/XXX" IS NOT NULL
SELECT COUNT(*) FROM db.t WHERE "XXX XXX/XXX" IS NULL
SELECT * FROM db.t
# file: test/sql/storage/compression/dict_fsst/test_null_filter_pushdown.test
# setup
CREATE OR REPLACE TABLE t1(type VARCHAR, id VARCHAR, problem VARCHAR)
# query
pragma force_compression='DICT_FSST'
CREATE OR REPLACE TABLE t1(type VARCHAR, id VARCHAR, problem VARCHAR)
INSERT INTO t1(type,id,problem) select 'events', 'test', NULL from range(40)
SELECT COUNT(*) FROM t1 WHERE problem IS NULL
# file: test/sql/storage/compression/dict_fsst/test_null_update.test
# setup
CREATE OR REPLACE TABLE t( compressed VARCHAR USING COMPRESSION 'DICT_FSST' )
# query
CREATE OR REPLACE TABLE t( compressed VARCHAR USING COMPRESSION 'DICT_FSST' )
INSERT INTO t VALUES ('Error3')
UPDATE t SET compressed = NULL
SELECT * FROM t AS e WHERE e.compressed IS NULL
# file: test/sql/storage/compression/string/big_strings.test
# setup
CREATE TABLE normal_string (a VARCHAR)
CREATE TABLE big_string (a VARCHAR)
# query
USE db_v1
USE db_v13
CREATE TABLE normal_string (a VARCHAR)
SELECT list_aggr(str_split(a,''),'min'), list_aggr(str_split(a,''),'min'), strlen(a) from normal_string
CREATE TABLE big_string (a VARCHAR)
SELECT list_aggr(str_split(a,''),'min'), list_aggr(str_split(a,''),'min'), strlen(a) from big_string
SELECT lower(compression) FROM pragma_storage_info('big_string') WHERE segment_type ILIKE 'VARCHAR' LIMIT 1
DROP TABLE big_string
DROP TABLE normal_string
# file: test/sql/storage/compression/string/blob.test
# setup
CREATE TABLE blobs (b BYTEA)
CREATE TABLE blob_empty (b BYTEA)
# query
CREATE TABLE blob_empty (b BYTEA)
INSERT INTO blob_empty VALUES(''), (''::BLOB)
INSERT INTO blob_empty VALUES(NULL), (NULL::BLOB)
SELECT * FROM blob_empty
SELECT lower(compression)!='fsst' FROM pragma_storage_info('blob_empty') WHERE segment_type ILIKE 'BLOB' LIMIT 1
DROP TABLE blobs
DROP TABLE blob_empty
# file: test/sql/storage/compression/string/empty.test
# setup
CREATE TABLE test_empty (a VARCHAR)
CREATE TABLE test_empty_large AS SELECT '' as a from range(0,10000) union all select 'A' union all select ''
# query
CREATE TABLE test_empty (a VARCHAR)
select * from test_empty
CREATE TABLE test_empty_large AS SELECT '' as a from range(0,10000) union all select 'A' union all select ''
select count(*), min(a[1]), max(a[1]) from test_empty_large limit 5
DROP TABLE test_empty
DROP TABLE test_empty_large
# file: test/sql/storage/compression/string/filter_pushdown.test
# setup
CREATE TABLE test (id INT, col VARCHAR)
# query
CREATE TABLE test (id INT, col VARCHAR)
INSERT INTO test SELECT i::INT id, concat('BLEEPBLOOP-', (i%10)::VARCHAR) col FROM range(10000) tbl(i)
SELECT MIN(col), MAX(col), COUNT(*) FROM test WHERE col >= 'BLEEPBLOOP-5'
SELECT MIN(id), MAX(id), MIN(col), MAX(col), COUNT(*) FROM test WHERE id='5000'
# file: test/sql/storage/compression/string/index_fetch.test
# setup
create type test_result as UNION( ok BOOL, err STRUCT( expected VARCHAR, actual VARCHAR ) )
CREATE TABLE test(id INTEGER PRIMARY KEY, col VARCHAR)
# query
drop type if exists test_result
create type test_result as UNION( ok BOOL, err STRUCT( expected VARCHAR, actual VARCHAR ) )
CREATE TABLE test(id INTEGER PRIMARY KEY, col VARCHAR)
INSERT INTO test SELECT i id, i::VARCHAR b FROM range(10000) tbl(i)
SELECT MIN(id), MAX(id), SUM(col::INT), MIN(col::INT), MAX(col::INT), COUNT(*) FROM test WHERE id=5000
# file: test/sql/storage/compression/string/medium.test
# setup
CREATE TABLE test (a VARCHAR)
# query
SET storage_compatibility_version='v1.0.0'
SET storage_compatibility_version='v1.3.0'
INSERT INTO test SELECT (i%500)::VARCHAR FROM range(0, 10000) tbl(i)
SELECT SUM(a::INT), MIN(a::INT), MAX(a::INT), COUNT(*) FROM test
# file: test/sql/storage/compression/string/simple.test
# setup
CREATE TABLE test (a VARCHAR)
# query
INSERT INTO test SELECT CONCAT('A-',(i%5)::VARCHAR) FROM range(0,1025) tbl(i)
select * from test limit 5
select a[3] from test limit 5
# file: test/sql/storage/compression/string/struct.test
# setup
CREATE TABLE test (s ROW(a VARCHAR))
# query
CREATE TABLE test (s ROW(a VARCHAR))
SELECT SUM(s['a']::INT), MIN(s['a']::INT), MAX(s['a']::INT), COUNT(*) FROM test
# file: test/sql/storage/compression/string/table_copy.test
# setup
CREATE TABLE test (a VARCHAR)
CREATE TABLE test_2 AS SELECT a FROM test
# query
INSERT INTO test SELECT (i%500)::VARCHAR FROM range(0,150000) tbl(i)
select sum(a::INT) from test
select sum(a::INT) from test_2
DROP TABLE test_2
# file: test/sql/storage/partial_blocks/many_columns_rle.test
# setup
CREATE TABLE integers(i0 INTEGER, i1 INTEGER, i2 INTEGER, i3 INTEGER, i4 INTEGER, i5 INTEGER, i6 INTEGER, i7 INTEGER, i8 INTEGER, i9 INTEGER, i10 INTEGER, i11 INTEGER, i12 INTEGER, i13 INTEGER, i14 INTEGER, i15 INTEGER, i16 INTEGER, i17 INTEGER, i18 INTEGER, i19 INTEGER, i20 INTEGER, i21 INTEGER, i22 INTEGER, i23 INTEGER, i24 INTEGER, i25 INTEGER, i26 INTEGER, i27 INTEGER, i28 INTEGER, i29 INTEGER, i30 INTEGER, i31 INTEGER, i32 INTEGER, i33 INTEGER, i34 INTEGER, i35 INTEGER, i36 INTEGER, i37 INTEGER, i38 INTEGER, i39 INTEGER, i40 INTEGER, i41 INTEGER, i42 INTEGER, i43 INTEGER, i44 INTEGER, i45 INTEGER, i46 INTEGER, i47 INTEGER, i48 INTEGER, i49 INTEGER, i50 INTEGER, i51 INTEGER, i52 INTEGER, i53 INTEGER, i54 INTEGER, i55 INTEGER, i56 INTEGER, i57 INTEGER, i58 INTEGER, i59 INTEGER, i60 INTEGER, i61 INTEGER, i62 INTEGER, i63 INTEGER, i64 INTEGER, i65 INTEGER, i66 INTEGER, i67 INTEGER, i68 INTEGER, i69 INTEGER, i70 INTEGER, i71 INTEGER, i72 INTEGER, i73 INTEGER, i74 INTEGER, i75 INTEGER, i76 INTEGER, i77 INTEGER, i78 INTEGER, i79 INTEGER, i80 INTEGER, i81 INTEGER, i82 INTEGER, i83 INTEGER, i84 INTEGER, i85 INTEGER, i86 INTEGER, i87 INTEGER, i88 INTEGER, i89 INTEGER, i90 INTEGER, i91 INTEGER, i92 INTEGER, i93 INTEGER, i94 INTEGER, i95 INTEGER, i96 INTEGER, i97 INTEGER, i98 INTEGER, i99 INTEGER)
# query
PRAGMA force_compression='rle'
SELECT total_blocks * block_size < 10 * 262144 FROM pragma_database_size()
# file: test/sql/storage/partial_blocks/many_columns_storage.test
# setup
CREATE TABLE integers(i0 INTEGER, i1 INTEGER, i2 INTEGER, i3 INTEGER, i4 INTEGER, i5 INTEGER, i6 INTEGER, i7 INTEGER, i8 INTEGER, i9 INTEGER, i10 INTEGER, i11 INTEGER, i12 INTEGER, i13 INTEGER, i14 INTEGER, i15 INTEGER, i16 INTEGER, i17 INTEGER, i18 INTEGER, i19 INTEGER, i20 INTEGER, i21 INTEGER, i22 INTEGER, i23 INTEGER, i24 INTEGER, i25 INTEGER, i26 INTEGER, i27 INTEGER, i28 INTEGER, i29 INTEGER, i30 INTEGER, i31 INTEGER, i32 INTEGER, i33 INTEGER, i34 INTEGER, i35 INTEGER, i36 INTEGER, i37 INTEGER, i38 INTEGER, i39 INTEGER, i40 INTEGER, i41 INTEGER, i42 INTEGER, i43 INTEGER, i44 INTEGER, i45 INTEGER, i46 INTEGER, i47 INTEGER, i48 INTEGER, i49 INTEGER, i50 INTEGER, i51 INTEGER, i52 INTEGER, i53 INTEGER, i54 INTEGER, i55 INTEGER, i56 INTEGER, i57 INTEGER, i58 INTEGER, i59 INTEGER, i60 INTEGER, i61 INTEGER, i62 INTEGER, i63 INTEGER, i64 INTEGER, i65 INTEGER, i66 INTEGER, i67 INTEGER, i68 INTEGER, i69 INTEGER, i70 INTEGER, i71 INTEGER, i72 INTEGER, i73 INTEGER, i74 INTEGER, i75 INTEGER, i76 INTEGER, i77 INTEGER, i78 INTEGER, i79 INTEGER, i80 INTEGER, i81 INTEGER, i82 INTEGER, i83 INTEGER, i84 INTEGER, i85 INTEGER, i86 INTEGER, i87 INTEGER, i88 INTEGER, i89 INTEGER, i90 INTEGER, i91 INTEGER, i92 INTEGER, i93 INTEGER, i94 INTEGER, i95 INTEGER, i96 INTEGER, i97 INTEGER, i98 INTEGER, i99 INTEGER)
# query
select count(*) from pragma_storage_info('integers') where block_id IS NULL
SELECT total_blocks FROM pragma_database_size()
# file: test/sql/storage/partial_blocks/many_columns_strings.test
# setup
CREATE TABLE strings(i0 VARCHAR, i1 VARCHAR, i2 VARCHAR, i3 VARCHAR, i4 VARCHAR, i5 VARCHAR, i6 VARCHAR, i7 VARCHAR, i8 VARCHAR, i9 VARCHAR, i10 VARCHAR, i11 VARCHAR, i12 VARCHAR, i13 VARCHAR, i14 VARCHAR, i15 VARCHAR, i16 VARCHAR, i17 VARCHAR, i18 VARCHAR, i19 VARCHAR, i20 VARCHAR, i21 VARCHAR, i22 VARCHAR, i23 VARCHAR, i24 VARCHAR, i25 VARCHAR, i26 VARCHAR, i27 VARCHAR, i28 VARCHAR, i29 VARCHAR, i30 VARCHAR, i31 VARCHAR, i32 VARCHAR, i33 VARCHAR, i34 VARCHAR, i35 VARCHAR, i36 VARCHAR, i37 VARCHAR, i38 VARCHAR, i39 VARCHAR, i40 VARCHAR, i41 VARCHAR, i42 VARCHAR, i43 VARCHAR, i44 VARCHAR, i45 VARCHAR, i46 VARCHAR, i47 VARCHAR, i48 VARCHAR, i49 VARCHAR, i50 VARCHAR, i51 VARCHAR, i52 VARCHAR, i53 VARCHAR, i54 VARCHAR, i55 VARCHAR, i56 VARCHAR, i57 VARCHAR, i58 VARCHAR, i59 VARCHAR, i60 VARCHAR, i61 VARCHAR, i62 VARCHAR, i63 VARCHAR, i64 VARCHAR, i65 VARCHAR, i66 VARCHAR, i67 VARCHAR, i68 VARCHAR, i69 VARCHAR, i70 VARCHAR, i71 VARCHAR, i72 VARCHAR, i73 VARCHAR, i74 VARCHAR, i75 VARCHAR, i76 VARCHAR, i77 VARCHAR, i78 VARCHAR, i79 VARCHAR, i80 VARCHAR, i81 VARCHAR, i82 VARCHAR, i83 VARCHAR, i84 VARCHAR, i85 VARCHAR, i86 VARCHAR, i87 VARCHAR, i88 VARCHAR, i89 VARCHAR, i90 VARCHAR, i91 VARCHAR, i92 VARCHAR, i93 VARCHAR, i94 VARCHAR, i95 VARCHAR, i96 VARCHAR, i97 VARCHAR, i98 VARCHAR, i99 VARCHAR)
# query
SELECT total_blocks * block_size < 15 * 262144 FROM pragma_database_size()
# file: test/sql/storage/partial_blocks/many_columns_validity.test
# setup
CREATE TABLE integers(i0 INTEGER, i1 INTEGER, i2 INTEGER, i3 INTEGER, i4 INTEGER, i5 INTEGER, i6 INTEGER, i7 INTEGER, i8 INTEGER, i9 INTEGER, i10 INTEGER, i11 INTEGER, i12 INTEGER, i13 INTEGER, i14 INTEGER, i15 INTEGER, i16 INTEGER, i17 INTEGER, i18 INTEGER, i19 INTEGER, i20 INTEGER, i21 INTEGER, i22 INTEGER, i23 INTEGER, i24 INTEGER, i25 INTEGER, i26 INTEGER, i27 INTEGER, i28 INTEGER, i29 INTEGER, i30 INTEGER, i31 INTEGER, i32 INTEGER, i33 INTEGER, i34 INTEGER, i35 INTEGER, i36 INTEGER, i37 INTEGER, i38 INTEGER, i39 INTEGER, i40 INTEGER, i41 INTEGER, i42 INTEGER, i43 INTEGER, i44 INTEGER, i45 INTEGER, i46 INTEGER, i47 INTEGER, i48 INTEGER, i49 INTEGER, i50 INTEGER, i51 INTEGER, i52 INTEGER, i53 INTEGER, i54 INTEGER, i55 INTEGER, i56 INTEGER, i57 INTEGER, i58 INTEGER, i59 INTEGER, i60 INTEGER, i61 INTEGER, i62 INTEGER, i63 INTEGER, i64 INTEGER, i65 INTEGER, i66 INTEGER, i67 INTEGER, i68 INTEGER, i69 INTEGER, i70 INTEGER, i71 INTEGER, i72 INTEGER, i73 INTEGER, i74 INTEGER, i75 INTEGER, i76 INTEGER, i77 INTEGER, i78 INTEGER, i79 INTEGER, i80 INTEGER, i81 INTEGER, i82 INTEGER, i83 INTEGER, i84 INTEGER, i85 INTEGER, i86 INTEGER, i87 INTEGER, i88 INTEGER, i89 INTEGER, i90 INTEGER, i91 INTEGER, i92 INTEGER, i93 INTEGER, i94 INTEGER, i95 INTEGER, i96 INTEGER, i97 INTEGER, i98 INTEGER, i99 INTEGER)
# query
INSERT INTO integers (i1) VALUES (NULL)
# file: test/sql/storage/nested/struct_of_lists_unaligned.test
# setup
CREATE TABLE test_list_2 (a integer, b STRUCT(c VARCHAR[], d VARCHAR[], e INTEGER[]))
# query
CREATE TABLE test_list_2 (a integer, b STRUCT(c VARCHAR[], d VARCHAR[], e INTEGER[]))
INSERT INTO test_list_2 SELECT 1, row(['a', 'b', 'c', 'd', 'e', 'f'], ['A', 'B'], [1, 5, 9]) FROM range(10)
# file: test/sql/storage/optimistic_write/optimistic_write_delete.test
# setup
CREATE TABLE test (a INTEGER)
# query
DELETE FROM test WHERE a=0
DELETE FROM test WHERE a=1
# file: test/sql/storage/optimistic_write/optimistic_write_large_strings.test
# setup
CREATE TABLE test(val VARCHAR)
# query
CREATE TABLE test(val VARCHAR)
SELECT strlen(val) FROM test
# file: test/sql/storage/optimistic_write/optimistic_write_not_trigger_checkpoint.test
# setup
CREATE TABLE t1 (id INTEGER, c0 DOUBLE)
# query
SET checkpoint_threshold = '5 KB'
SELECT wal_size == '0 bytes' FROM pragma_database_size()
CREATE TABLE t1 (id INTEGER, c0 DOUBLE)
INSERT INTO t1 SELECT *, random() FROM range(200000)
SELECT wal_size != '0 bytes' FROM pragma_database_size()
# file: test/sql/storage/optimistic_write/optimistic_write_nulls.test
# setup
CREATE TABLE test (a INTEGER)
# query
INSERT INTO test SELECT case when i%3=0 then null else i end FROM range(1000000) t(i)
SELECT SUM(a), COUNT(a), COUNT(*) FROM test
# file: test/sql/storage/optimistic_write/optimistic_write_temp_table.test
# setup
CREATE TABLE integers AS SELECT 42 i
CREATE TEMPORARY TABLE test (a INTEGER)
CREATE TEMPORARY TABLE test2 (a INTEGER)
# query
CREATE TABLE integers AS SELECT 42 i
SELECT total_blocks < 10 FROM pragma_database_size()
CREATE TEMPORARY TABLE test (a INTEGER)
CREATE TEMPORARY TABLE test2 (a INTEGER)
INSERT INTO test2 SELECT * FROM range(1000000)
# file: test/sql/storage/optimistic_write/optimistic_write_update.test
# setup
CREATE TABLE test (a INTEGER)
# query
UPDATE test SET a=500000 WHERE a=0
# file: test/sql/storage/metadata/full_table_metadata_reuse.test
# setup
CREATE TABLE bigtbl(i INT)
CREATE TABLE little_tbl(i INT)
# query
CREATE TABLE bigtbl(i INT)
INSERT INTO bigtbl FROM range(1000000)
CREATE TABLE little_tbl(i INT)
INSERT INTO little_tbl VALUES (1)
SELECT COUNT(*), SUM(i) FROM bigtbl
# file: test/sql/storage/metadata/full_table_metadata_reuse_v7.test
# setup
CREATE TABLE tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4 FROM range(200000) t(i)
CREATE TABLE other_tbl AS SELECT i AS x FROM range(1000) t(i)
# query
SET force_column_metadata_reuse=true
CREATE TABLE tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4 FROM range(200000) t(i)
SELECT COUNT(*), SUM(c0) FROM tbl
CREATE TABLE other_tbl AS SELECT i AS x FROM range(1000) t(i)
INSERT INTO other_tbl SELECT i FROM range(1000, 2000) t(i)
SELECT COUNT(*), SUM(x) FROM other_tbl
# file: test/sql/storage/metadata/multi_table_metadata_reuse.test
# setup
CREATE TABLE ducklake_table(end_snapshot BIGINT)
CREATE TABLE ducklake_column(end_snapshot BIGINT)
CREATE TABLE my_table (a INTEGER, b INTEGER)
# query
SET experimental_metadata_reuse=true
CREATE TABLE ducklake_table(end_snapshot BIGINT)
CREATE TABLE ducklake_column(end_snapshot BIGINT)
INSERT INTO ducklake_table VALUES (1)
INSERT INTO ducklake_column VALUES (1)
UPDATE ducklake_table SET end_snapshot = 3
UPDATE ducklake_column SET end_snapshot = 3
CREATE TABLE my_table (a INTEGER, b INTEGER)
# file: test/sql/storage/metadata/partial_column_metadata_reuse_small_block.test
# setup
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15 FROM range(500000) t(i)
# query
USE partial_reuse_carries_column_extras
SET debug_skip_checkpoint_on_commit=true
SET debug_verify_blocks=true
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15 FROM range(500000) t(i)
UPDATE wide_tbl SET c6 = c6 + 1, c7 = c7 + 1 WHERE c0 < 100
SELECT COUNT(*), SUM(c0) FROM wide_tbl
# file: test/sql/storage/metadata/partial_column_metadata_reuse_v7.test
# setup
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15, i AS c16, i AS c17, i AS c18, i AS c19 FROM range(200000) t(i)
# query
CREATE TABLE wide_tbl AS SELECT i AS c0, i AS c1, i AS c2, i AS c3, i AS c4, i AS c5, i AS c6, i AS c7, i AS c8, i AS c9, i AS c10, i AS c11, i AS c12, i AS c13, i AS c14, i AS c15, i AS c16, i AS c17, i AS c18, i AS c19 FROM range(200000) t(i)
ALTER TABLE wide_tbl ADD COLUMN c20 INTEGER DEFAULT 42
SELECT COUNT(*), SUM(c0), SUM(c20) FROM wide_tbl
ALTER TABLE wide_tbl DROP COLUMN c10
UPDATE wide_tbl SET c0 = c0 + 1 WHERE c0 < 100
# file: test/sql/storage/metadata/partial_table_metadata_reuse.test
# setup
CREATE TABLE bigtbl(i INT)
# query
INSERT INTO bigtbl VALUES (NULL)
# file: test/sql/storage/compact_block_size/block_size_with_rollback.test
# query
CREATE TABLE rollback.tbl AS SELECT range AS i FROM range(100)
# file: test/sql/storage/compact_block_size/compact_block_size.test
# query
SELECT * FROM block_size_16kb.tbl
# file: test/sql/storage/compact_block_size/compact_vector_size.test
# query
SELECT * FROM vector_size_512.tbl
# file: test/sql/storage/compact_block_size/create_table_compression.test
# setup
CREATE TABLE T (a INTEGER USING COMPRESSION RLE, b INTEGER USING COMPRESSION BITPACKING, C INTEGER USING COMPRESSION UNCOMPRESSED)
# query
SELECT COUNT(*) > 0 FROM pragma_storage_info('T') WHERE segment_type ILIKE 'INTEGER' AND compression = 'RLE'
SELECT * FROM T_1
SELECT COUNT(*) > 0 FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' AND compression = 'RLE'
ALTER TABLE T_1 DROP COLUMN c_1
ALTER TABLE T_1 DROP COLUMN b_1
SELECT compression FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' LIMIT 2
ALTER TABLE T_1 ADD COLUMN b INTEGER DEFAULT 2
SELECT compression FROM pragma_storage_info('T_1') WHERE segment_type ILIKE 'INTEGER' LIMIT 3
# file: test/sql/storage/compact_block_size/ensure_bitpacking.test
# query
CREATE TABLE smaller_block_size.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE larger_block_size.tbl AS SELECT range AS i FROM range(10000)
CHECKPOINT smaller_block_size
CHECKPOINT larger_block_size
SELECT COUNT(*) > 0 FROM pragma_storage_info('larger_block_size.tbl') WHERE compression = 'BitPacking'
# file: test/sql/storage/compact_block_size/ensure_no_bitpacking.test
# query
CREATE TABLE no_bitpacking.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE has_bitpacking.tbl AS SELECT range AS i FROM range(10000)
CHECKPOINT has_bitpacking
CHECKPOINT no_bitpacking
SELECT COUNT(*) FROM pragma_storage_info('no_bitpacking.tbl') WHERE compression = 'BitPacking'
# file: test/sql/storage/compact_block_size/insertion_order_odd_batches.test
# query
CREATE TABLE integers AS SELECT * FROM range(100000) tbl(i)
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers')
SELECT MEDIAN(count) FROM pragma_storage_info('integers')
SELECT * FROM integers_parquet LIMIT 5
SELECT * FROM integers_parquet LIMIT 5 OFFSET 73654
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers_parquet')
SELECT MEDIAN(count) FROM pragma_storage_info('integers_parquet')
SELECT COUNT(DISTINCT block_id) < 60 FROM pragma_storage_info('integers_parquet_no_order')
SELECT MEDIAN(count) FROM pragma_storage_info('integers_parquet_no_order')
# file: test/sql/storage/compact_block_size/mixed_block_sizes.test
# query
CREATE TABLE small.tbl AS SELECT range AS i FROM range(10000)
CREATE TABLE large.tbl AS SELECT range AS i FROM range(10000)
SELECT list_sum(LIST(t1.i) || LIST(t2.i)) FROM large.tbl AS t1 JOIN small.tbl AS t2 ON t1.i = t2.i
# file: test/sql/attach/attach_all_types.test
# setup
CREATE TABLE db1.all_types AS SELECT * FROM test_all_types()
# query
CREATE TABLE db1.all_types AS SELECT * FROM test_all_types()
SELECT * FROM test_all_types()
SELECT * FROM db1.all_types
# file: test/sql/attach/attach_catalog_error_early_out.test
# query
SET catalog_error_max_schemas = 0
# file: test/sql/attach/attach_checkpoint_vacuum.test
# setup
CREATE TABLE db1.integers(i INTEGER)
# query
CREATE TABLE db1.integers(i INTEGER)
CHECKPOINT db1
VACUUM db1.integers
# file: test/sql/attach/attach_copy.test
# setup
CREATE TABLE db1.test(a INTEGER, b INTEGER, c VARCHAR(10))
# query
ATTACH DATABASE ':memory:' AS db1
CREATE TABLE db1.test(a INTEGER, b INTEGER, c VARCHAR(10))
# file: test/sql/attach/attach_create_index.test
# setup
CREATE TABLE tmp.t1(id int)
CREATE INDEX idx ON tmp.t1(id)
# query
ATTACH '' AS tmp
CREATE TABLE tmp.t1(id int)
CREATE INDEX idx ON tmp.t1(id)
# file: test/sql/attach/attach_cross_catalog.test
# setup
CREATE TYPE db1.mood AS ENUM('ok', 'sad', 'happy')
CREATE TABLE test(a INTEGER)
CREATE TABLE db1.integers(i mood)
CREATE INDEX index ON test(a)
# query
CREATE TABLE test(a INTEGER)
CREATE INDEX index ON test(a)
CREATE TYPE db1.mood AS ENUM('ok', 'sad', 'happy')
CREATE TABLE db1.integers(i mood)
# reject
CREATE INDEX db1.index ON test(a)
CREATE TABLE integers(i mood)
SELECT 'happy'::mood
# file: test/sql/attach/attach_custom_block_size.test
# query
DETACH default_size
DETACH dbname
SET default_block_size = '262144'
# reject
SET default_block_size = '123456'
SET default_block_size = '128'
# file: test/sql/attach/attach_database_options.test
# query
ATTACH DATABASE ':memory:' AS new_database (BLOCK_SIZE 262144, ROW_GROUP_SIZE 2048)
SELECT options['block_size'] from duckdb_databases() where database_name = 'new_database'
SELECT options['row_group_size'] from duckdb_databases() where database_name = 'new_database'
# file: test/sql/attach/attach_database_size.test
# query
SELECT database_name FROM pragma_database_size() WHERE database_name = 'db1'
ATTACH ':memory:' AS db2
SELECT database_name FROM pragma_database_size() WHERE database_name = 'db1' OR database_name = 'db2' ORDER BY ALL
# file: test/sql/attach/attach_dbname_quotes.test
# setup
CREATE SCHEMA "my""db"."my""schema"
CREATE SCHEMA """"
# query
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
# file: test/sql/attach/attach_default_database_case.test
# query
ATTACH ':memory:' AS MyDB
USE MyDB
SELECT current_database()
# reject
DETACH mydb
DETACH MyDB
# file: test/sql/attach/attach_default_table.test
# setup
CREATE OR REPLACE TABLE ddb.my_table AS (SELECT 1337 as value)
create table ddb as select 42 as value
CREATE VIEW ddb as SELECT 1
# query
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
# reject
FROM ddb
# file: test/sql/attach/attach_defaults.test
# query
ATTACH DATABASE ':memory:' AS new_database
# reject
ATTACH ':memory:' AS new_database
ATTACH ':memory:'
# file: test/sql/attach/attach_dependencies.test
# setup
CREATE TABLE pk_tbl (id INTEGER PRIMARY KEY, name VARCHAR UNIQUE)
CREATE TABLE fk_tbl (id INTEGER REFERENCES pk_tbl(id))
CREATE TABLE tbl_alter_column (id INT, other INT, nn_col INT NOT NULL, rm INT, rename_c INT, my_def INT, drop_def INT DEFAULT 10, new_null_col INT)
# query
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
# file: test/sql/attach/attach_did_you_mean.test
# setup
CREATE SCHEMA db1.myschema
CREATE TABLE hello(i INTEGER)
CREATE TABLE db1.test(a INTEGER)
CREATE TABLE db1.myschema.blablabla(i INTEGER)
# query
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
# reject
SELECT * FROM blablabla
SELECT * FROM hello
# file: test/sql/attach/attach_different_alias.test
# query
create table alias1.tbl1 as select 1 as a
FROM alias1.tbl1
DETACH alias1
FROM alias2.tbl1
# file: test/sql/attach/attach_duckdb_type.test
# query
SELECT database_name FROM duckdb_databases() WHERE database_name = 'first'
# file: test/sql/attach/attach_enable_external_access.test
# setup
CREATE TABLE a1.test (a INTEGER PRIMARY KEY, b INTEGER)
CREATE TABLE a2.test (a INTEGER PRIMARY KEY, b INTEGER)
# query
CREATE TABLE a1.test (a INTEGER PRIMARY KEY, b INTEGER)
CHECKPOINT a1
CREATE TABLE a2.test (a INTEGER PRIMARY KEY, b INTEGER)
CHECKPOINT a2
# file: test/sql/attach/attach_encrypted_db_key_test.test
# query
DETACH encrypted_aws
# file: test/sql/attach/attach_encryption_block_header.test
# query
SELECT tags FROM duckdb_databases() WHERE database_name LIKE '%encrypted%' ORDER BY database_name
# file: test/sql/attach/attach_encryption_downgrade_prevention.test
# query
ATTACH 'data/attach_test/encrypted_ctr_key=abcde.db' as enc1 (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'CTR')
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc2 (ENCRYPTION_KEY 'abcde')
# reject
ATTACH 'data/attach_test/encrypted_ctr_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde')
# file: test/sql/attach/attach_encryption_fallback_readonly.test
# setup
CREATE TABLE enc.test AS SELECT 1 as a
# query
set autoinstall_known_extensions=false
set autoload_known_extensions=false
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'GCM', READ_ONLY)
FROM enc.test ORDER BY value
CREATE TABLE enc.test AS SELECT 1 as a
FROM enc.test
# reject
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'GCM')
# file: test/sql/attach/attach_enums.test
# setup
CREATE TYPE db1.mood AS ENUM ('sad', 'ok', 'happy')
CREATE TYPE db2.mood AS ENUM ('ble','grr','kkcry')
CREATE TABLE db1.person ( name text, current_mood mood )
CREATE TABLE db2.person ( name text, current_mood mood )
# query
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
# reject
SELECT enum_range(NULL::xx.db1.main.mood) AS my_enum_range
# file: test/sql/attach/attach_export_import.test
# setup
CREATE TABLE db1.integers(i INTEGER)
CREATE TABLE other.dont_export_me (i integer)
CREATE VIEW db1.integers_view AS SELECT * FROM integers
# query
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
# reject
SELECT * FROM other.dont_export_me
# file: test/sql/attach/attach_expr.test
# query
SET VARIABLE db_type='DUCKDB'
ATTACH ':memory:' AS db1 (TYPE getvariable('db_type'))
SET VARIABLE db_type='UNKNOWN_TYPE'
# reject
ATTACH ':memory:' AS db2 (TYPE getvariable('db_type'))
# file: test/sql/attach/attach_filepath_roundtrip.test
# query
SELECT database_name FROM duckdb_databases() WHERE database_name = 'concurrent'
DETACH concurrent
DETACH con2_rollback_detach
DETACH con1
# file: test/sql/attach/attach_foreign_key.test
# setup
CREATE TABLE album(artistid INTEGER, albumname TEXT, albumcover TEXT, UNIQUE (artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, albumname))
# query
ATTACH DATABASE ':memory:' AS db2
INSERT INTO db1.song VALUES (11, 1, 'A', 'A_song'), (12, 2, 'B', 'B_song'), (13, 3, 'C', 'C_song')
# reject
CREATE TABLE db1.song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, albumname))
# file: test/sql/attach/attach_fsspec.test
# query
CREATE TABLE dummy.tbl(i INTEGER)
DETACH dummy
FROM dummy.tbl
# reject
ATTACH 'dummy_extension:/hello.world'
# file: test/sql/attach/attach_hidden.test
# query
ATTACH ':memory:' AS hidden_db (HIDDEN true)
SELECT database_name FROM duckdb_databases() WHERE database_name = 'hidden_db'
SELECT database_name FROM duckdb_tables() WHERE database_name = 'hidden_db'
CREATE TABLE hidden_db.main.tbl AS SELECT 42 AS i
SELECT * FROM hidden_db.main.tbl
DETACH hidden_db
# file: test/sql/attach/attach_home_directory.test
# setup
CREATE TABLE s1.integers AS FROM range(10) t(i)
# query
CREATE TABLE s1.integers AS FROM range(10) t(i)
SELECT SUM(i) FROM s1.integers
ATTACH '~/home_dir.db' AS s1
# reject
ATTACH '~/home_dir.db' AS s2
# file: test/sql/attach/attach_icu_collation.test
# query
SELECT * FROM db.strings
SELECT * FROM db.strings ORDER BY 1
# file: test/sql/attach/attach_if_not_exists.test
# setup
CREATE TABLE db1.integers(i INTEGER)
# query
ATTACH IF NOT EXISTS ':memory:' AS db1
# file: test/sql/attach/attach_if_not_exists_detach.test
# query
CREATE TABLE db1.tbl(i INTEGER)
# file: test/sql/attach/attach_index.test
# setup
CREATE TABLE tbl_a ( a_id INTEGER PRIMARY KEY, value VARCHAR NOT NULL )
CREATE INDEX idx_tbl_a ON tbl_a (value)
# query
USE attach_index_db
CREATE TABLE tbl_a ( a_id INTEGER PRIMARY KEY, value VARCHAR NOT NULL )
CREATE INDEX idx_tbl_a ON tbl_a (value)
INSERT INTO tbl_a VALUES (1, 'x')
INSERT INTO tbl_a VALUES (2, 'y')
SELECT * FROM tbl_a WHERE a_id = 2
USE other_attach_index
DETACH attach_index_db
SELECT * FROM attach_index_db.tbl_a WHERE a_id = 2
# file: test/sql/attach/attach_issue16122.test
# setup
create table mytable (C1 VARCHAR(10))
create table TOMERGE.mytable (C1 VARCHAR(10))
# query
create table mytable (C1 VARCHAR(10))
insert into mytable values ('a')
create table TOMERGE.mytable (C1 VARCHAR(10))
insert into TOMERGE.mytable SELECT * FROM mytable
select * from TOMERGE.mytable
# file: test/sql/attach/attach_issue7567.test
# setup
create schema schema1
create table schema1.table1 as select 1 as a
# query
attach ':memory:' as test
use test
create schema schema1
create table schema1.table1 as select 1 as a
set schema='schema1'
select * from table1
# reject
set schema='schema2'
# file: test/sql/attach/attach_issue_7660.test
# setup
create table tbl1 as select 1 as a
# query
create table tbl1 as select 1 as a
FROM test.tbl1
DETACH test
FROM tbl1
# file: test/sql/attach/attach_macros.test
# setup
CREATE MACRO db1.two_x_plus_y(x, y) AS 2 * x + y
# query
CREATE TABLE db1.tbl AS SELECT 42 AS x, 3 AS y
CREATE MACRO db1.two_x_plus_y(x, y) AS 2 * x + y
SELECT db1.two_x_plus_y(x, y) FROM db1.tbl
SELECT db1.main.two_x_plus_y(x, y) FROM db1.tbl
SELECT two_x_plus_y(x, y) FROM db1.tbl
# file: test/sql/attach/attach_modify_multiple_databases.test
# setup
CREATE TABLE database.integers(i INTEGER)
CREATE TABLE integers(i INTEGER)
# query
ATTACH DATABASE ':memory:' AS database
CREATE TABLE database.integers(i INTEGER)
INSERT INTO database.integers SELECT * FROM range(10)
# file: test/sql/attach/attach_multi_identifiers.test
# setup
CREATE SCHEMA db1.s1
CREATE SCHEMA db2.s1
CREATE TABLE db2.s1.t(c INT)
CREATE OR REPLACE TABLE db1.s1.t ( c INT, c_squared AS (c * c), )
# query
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
# reject
SELECT c FROM db1.s1.t, db2.s1.t
SELECT t.c FROM db1.s1.t, db2.s1.t
SELECT s1.t.c FROM db1.s1.t, db2.s1.t
# file: test/sql/attach/attach_nested_types.test
# setup
CREATE SCHEMA database.schema
CREATE TABLE database.schema.table(col ROW(field INTEGER))
# query
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
# file: test/sql/attach/attach_new_compression.test
# setup
CREATE TABLE db1.str_tbl AS SELECT STRING_AGG('long_string_' || i, '-') FROM range(1000) t(i)
CREATE TABLE db1.str_tbl2 AS FROM db1.str_tbl
# query
SET force_compression='roaring'
CREATE TABLE db1.tbl AS SELECT CASE WHEN i%2=0 THEN NULL ELSE i END i FROM range(10000) t(i)
CREATE TABLE db1.str_tbl AS SELECT STRING_AGG('long_string_' || i, '-') FROM range(1000) t(i)
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.tbl') WHERE compression='Roaring'
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.str_tbl') WHERE compression='ZSTD'
CREATE TABLE db1.tbl2 AS FROM db1.tbl
CREATE TABLE db1.str_tbl2 AS FROM db1.str_tbl
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.tbl2') WHERE compression='Roaring'
SELECT COUNT(*)>0 FROM pragma_storage_info('db1.str_tbl2') WHERE compression='ZSTD'
# file: test/sql/attach/attach_no_wal_writes_mode.test
# query
CREATE TABLE no_wal_writes.tbl AS SELECT range AS id, 0 AS v FROM range(5_000)
UPDATE no_wal_writes.tbl SET v = id
DETACH no_wal_writes
SELECT COUNT(*) FROM no_wal_writes.tbl WHERE v = 0
CHECKPOINT no_wal_writes
# file: test/sql/attach/attach_or_replace.test
# setup
CREATE TABLE db1.all_types AS SELECT * FROM test_all_types()
CREATE TABLE db2.all_types_new AS SELECT * FROM test_all_types()
# query
CREATE TABLE db2.all_types_new AS SELECT * FROM test_all_types()
DETACH db2
SELECT * FROM db1.all_types_new
SELECT * FROM db2.all_types
# file: test/sql/attach/attach_persistent.test
# setup
CREATE TABLE persistent_attach.integers(i INTEGER)
# query
CREATE TABLE persistent_attach.integers(i INTEGER)
INSERT INTO persistent_attach.integers VALUES (42)
SELECT SUM(i) FROM persistent_attach.integers
DETACH persistent_attach
# file: test/sql/attach/attach_pragma_storage_info.test
# setup
CREATE OR REPLACE TABLE persistent.T1 (A0 int)
# query
CREATE OR REPLACE TABLE persistent.T1 (A0 int)
insert into persistent.T1 values (5)
SELECT column_name from pragma_storage_info('persistent.T1')
# file: test/sql/attach/attach_read_only.test
# setup
CREATE TABLE db1.integers AS SELECT * FROM range(10) t(i)
CREATE TABLE db2.integers AS SELECT * FROM db1.integers
CREATE TABLE db1.test AS SELECT * FROM integers
# query
CREATE TABLE db1.integers AS SELECT * FROM range(10) t(i)
SELECT SUM(i) FROM db1.integers
CREATE TABLE db2.integers AS SELECT * FROM db1.integers
SELECT SUM(i) FROM db2.integers
ATTACH ':memory:' AS db1 (READ_WRITE)
CREATE TABLE db1.test AS SELECT * FROM integers
# reject
ATTACH ':memory:' AS db1 (READONLY 1)
ATTACH ':memory:' AS db1 (BLABLABLA 1)
CREATE TABLE db1.test AS SELECT * FROM range(10) t(i)
CREATE TABLE test AS SELECT * FROM db1.test
# file: test/sql/attach/attach_read_only_transaction.test
# setup
CREATE TABLE db1.integers(i INTEGER)
# query
INSERT INTO db1.integers VALUES (42)
BEGIN TRANSACTION READ ONLY
FROM db1.integers
# reject
INSERT INTO db1.integers VALUES (48)
# file: test/sql/attach/attach_replay_with_no_wal_writes.test
# query
CREATE TABLE wal_writes.tbl AS SELECT range AS id, 0 AS v FROM range(5_000)
DETACH wal_writes
INSERT INTO wal_writes.tbl VALUES (42, 42)
CHECKPOINT wal_writes
UPDATE wal_writes.tbl SET v = id
# file: test/sql/attach/attach_reserved.test
# setup
CREATE TABLE temp_db.integers(i INTEGER)
CREATE TABLE system_db.integers(i INTEGER)
# query
CREATE TABLE temp_db.integers(i INTEGER)
DETACH temp_db
CREATE TABLE system_db.integers(i INTEGER)
DETACH system_db
# reject
ATTACH DATABASE ':memory:' AS temp
ATTACH DATABASE ':memory:' AS main
ATTACH DATABASE ':memory:' AS system
# file: test/sql/attach/attach_row_group_size.test
# query
CREATE TABLE db1.tbl AS FROM range(10000) t(i)
INSERT INTO db1.tbl FROM range(10000)
# file: test/sql/attach/attach_row_group_size_decreasing.test
# setup
CREATE TABLE test.data (key BIGINT PRIMARY KEY)
# query
CREATE TABLE test.data (key BIGINT PRIMARY KEY)
INSERT INTO test.data SELECT * FROM range(8190)
SELECT COUNT(DISTINCT row_group_id) FROM pragma_storage_info('test.data')
INSERT INTO test.data VALUES(8190), (8191)
SELECT row_group_id, SUM(count) FROM pragma_storage_info('test.data') where segment_type != 'VALIDITY' GROUP BY (row_group_id) ORDER BY row_group_id
# file: test/sql/attach/attach_schema.test
# setup
CREATE SCHEMA new_database.s1
# query
CREATE SCHEMA new_database.s1
# reject
CREATE SCHEMA new_database.s1.xxx
CREATE SCHEMA IF NOT EXISTS new_database.s1.xxx
# file: test/sql/attach/attach_sequence.test
# setup
CREATE SEQUENCE seq
CREATE SEQUENCE db1.seq
CREATE TABLE db1.integers(i INTEGER DEFAULT nextval('db1.seq'))
# query
CREATE SEQUENCE db1.seq
CREATE TABLE db1.integers(i INTEGER DEFAULT nextval('db1.seq'))
SELECT nextval('db1.seq')
detach db1
# reject
CREATE TABLE db1.integers(i INTEGER DEFAULT nextval('seq'))
CREATE TABLE integers(i INTEGER DEFAULT nextval('db1.seq'))
# file: test/sql/attach/attach_serialize_dependency.test
# setup
CREATE TABLE A (A1 INTEGER PRIMARY KEY,A2 VARCHAR, A3 INTEGER)
CREATE TABLE B(B1 INTEGER REFERENCES A(A1))
CREATE INDEX A_index ON A (A2)
# query
set storage_compatibility_version='latest'
CREATE TABLE A (A1 INTEGER PRIMARY KEY,A2 VARCHAR, A3 INTEGER)
CREATE INDEX A_index ON A (A2)
CREATE TABLE B(B1 INTEGER REFERENCES A(A1))
USE db1_other
detach db2
# file: test/sql/attach/attach_show_all_tables.test
# setup
CREATE SCHEMA new_database.s1
CREATE TABLE tbl(a INTEGER)
# query
CREATE TABLE new_database.tbl(b INTEGER)
CREATE TABLE new_database.s1.tbl(c INTEGER)
SHOW ALL TABLES
# file: test/sql/attach/attach_show_table.test
# setup
CREATE SCHEMA db2.test_schema
CREATE TABLE db1.table_in_db1(i int)
CREATE TABLE db2.table_in_db2(i int)
CREATE TABLE db2.test_schema.table_in_db2_test_schema(i int)
# query
CREATE TABLE db1.table_in_db1(i int)
CREATE TABLE db2.table_in_db2(i int)
CREATE SCHEMA db2.test_schema
CREATE TABLE db2.test_schema.table_in_db2_test_schema(i int)
USE DB1
USE db2.test_schema
USE DB2.TEST_sChEmA
FROM table_in_db2
FROM table_in_db2_test_schema
# file: test/sql/attach/attach_storage_version.test
# query
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
# file: test/sql/attach/attach_table_constraints.test
# query
CREATE TABLE test.tbl(i INTEGER PRIMARY KEY)
select constraint_catalog, table_catalog, table_name from information_schema.table_constraints limit 1
# file: test/sql/attach/attach_table_info.test
# setup
CREATE SCHEMA new_database.new_schema
CREATE TABLE new_database.integers(i INTEGER)
CREATE TABLE new_database.new_schema.integers(i INTEGER)
# query
CREATE TABLE new_database.integers(i INTEGER)
PRAGMA table_info('new_database.integers')
CREATE SCHEMA new_database.new_schema
CREATE TABLE new_database.new_schema.integers(i INTEGER)
PRAGMA table_info('new_database.new_schema.integers')
USE new_database.new_schema
PRAGMA table_info('integers')
# file: test/sql/attach/attach_transactionality.test
# setup
CREATE TABLE attach_transaction.integers(i INTEGER)
# query
CREATE TABLE attach_transaction.integers(i INTEGER)
INSERT INTO attach_transaction.integers VALUES (42)
DETACH attach_transaction
INSERT INTO attach_transaction.integers VALUES (84)
SELECT * FROM attach_transaction.integers
SELECT * FROM attach_transaction.integers ORDER BY 1
# file: test/sql/attach/attach_use_rollback.test
# query
attach ':memory:' as mem
use mem
# reject
create table tbl(i int)
# file: test/sql/attach/attach_view_search_path.test
# setup
CREATE SCHEMA my_schema
CREATE TABLE my_tbl(i INTEGER)
CREATE VIEW my_view AS FROM my_tbl
# query
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
# file: test/sql/attach/attach_views.test
# setup
CREATE SCHEMA new_database.s1
CREATE TABLE t1 AS SELECT 42 i
# query
CREATE TABLE t1 AS SELECT 42 i
# file: test/sql/attach/attach_wal_alter.test
# setup
CREATE TABLE t2(c1 INT)
# query
CREATE TABLE t2(c1 INT)
ALTER TABLE t2 ALTER c1 SET DEFAULT 0
ATTACH DATABASE ':memory:' as db2
INSERT INTO db1.t2 DEFAULT VALUES
SELECT * FROM db1.t2
# file: test/sql/attach/attach_wal_alter_sequence.test
# setup
CREATE SEQUENCE db1.seq
CREATE TABLE db1.test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
# query
CREATE TABLE db1.test (a INTEGER DEFAULT nextval('seq'), b INTEGER, c INTEGER DEFAULT currval('seq'))
INSERT INTO db1.test (b) VALUES (1)
alter table db1.test RENAME TO blubb
INSERT INTO db1.blubb (b) VALUES (10)
SELECT * FROM db1.blubb
INSERT INTO db2.blubb (b) VALUES (100)
SELECT * FROM db2.blubb
# file: test/sql/attach/detach_keyword.test
# query
ATTACH DATABASE ':memory:' AS varchar
DETACH varchar
# file: test/sql/attach/in_memory_attach.test
# setup
CREATE TABLE new_database.integers(i INTEGER)
# query
INSERT INTO new_database.integers VALUES (42)
INSERT INTO new_database.main.integers VALUES (84)
SELECT * FROM new_database.integers ORDER BY i
SELECT * FROM new_database.main.integers ORDER BY i
SELECT * FROM new_database.integers ORDER BY new_database.integers.i
SELECT * FROM new_database.main.integers ORDER BY new_database.main.integers.i
# reject
SELECT * FROM new_database.integers ORDER BY new_database.i
# file: test/sql/attach/reattach_schema.test
# setup
CREATE SCHEMA new_db.my_schema
CREATE SEQUENCE new_db.my_schema.my_sequence
CREATE TABLE new_db.my_schema.my_table(col INTEGER)
CREATE VIEW new_db.my_schema.my_view AS SELECT 84
CREATE MACRO new_db.my_schema.one() AS (SELECT 1)
CREATE MACRO new_db.my_schema.range(a) as TABLE SELECT * FROM range(a)
# query
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
# reject
USE new_name.my_schema.my_table
# file: test/sql/attach/remote_file_concurrently.test
# query
ATTACH 'https://raw.githubusercontent.com/duckdb/duckdb/main/data/attach_test/attach.db' AS db
ATTACH 'https://raw.githubusercontent.com/duckdb/duckdb/main/data/attach_test/attach.db' AS db2
# file: test/sql/attach/show_databases.test
# setup
CREATE TABLE tbl AS SELECT 42 i
# query
SHOW DATABASES
SELECT name FROM pragma_database_list ORDER BY name
USE new_database
CREATE TABLE tbl AS SELECT 42 i
SELECT * FROM new_database.tbl
# reject
USE blablabla
# file: test/sql/attach/show_schemas.test
# query
SHOW SCHEMAS
USE new_database.new_s2
DROP SCHEMA new_database.new_s2
DETACH memory
DESCRIBE SCHEMAS
# file: test/sql/function/autocomplete/alter_table.test
# setup
CREATE TABLE my_table(first_column bigint)
# query
CREATE TABLE my_table(first_column bigint)
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table DROP COLUMN fi') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table ALTER COLUMN fi') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('ALTER TABLE my_table RENAME COLUMN fi') LIMIT 1
# file: test/sql/function/autocomplete/copy.test
# setup
CREATE TABLE my_table(my_column INTEGER)
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('COP') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY tbl FRO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY tbl FROM ''file.csv'' HEAD') LIMIT 1
CREATE TABLE my_table(my_column INTEGER)
SELECT suggestion, suggestion_start FROM sql_auto_complete('COPY my_') LIMIT 1
# file: test/sql/function/autocomplete/create_function.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE F') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name(a) A') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name(a) AS a+1, (b) A') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name (a) AS TA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE MACRO name (a) AS TABLE SEL') LIMIT 1
# file: test/sql/function/autocomplete/create_schema.test
# query
SELECT suggestion, suggestion_start, suggestion_type FROM sql_auto_complete('CREATE SCH') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA I') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA IF NO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA IF NOT EX') LIMIT 1
ATTACH ':memory:' AS attached_in_memory
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SCHEMA attac') LIMIT 1
# file: test/sql/function/autocomplete/create_sequence.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQ') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQUENCE seq CYC') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE SEQUENCE seq INC') LIMIT 1
# file: test/sql/function/autocomplete/create_table.test
# setup
CREATE SCHEMA abcdefgh
CREATE SCHEMA "SCHEMA"
# query
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
# reject
FROM sql_auto_complete(NULL)
# file: test/sql/function/autocomplete/create_type.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TY') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS ENU') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS TIME WITH TI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CREATE TYPE my_type AS ROW(ts TIMESTAMP WITH TIME ZON') LIMIT 1
# file: test/sql/function/autocomplete/drop.test
# setup
CREATE SCHEMA my_schema
CREATE TABLE my_table(my_column INTEGER)
CREATE TABLE my_schema.table_in_schema(my_column INTEGER)
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('DRO') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TA') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP VI') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE IF EX') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE tbl CAS') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_') LIMIT 1
CREATE TABLE my_schema.table_in_schema(my_column INTEGER)
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_s') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DROP TABLE my_schema.t') LIMIT 1
# file: test/sql/function/autocomplete/expressions.test
# query
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
# file: test/sql/function/autocomplete/identical_schema_table.test
# setup
CREATE SCHEMA my_catalog_entry
CREATE TABLE my_catalog_entry(i INT)
# query
CREATE SCHEMA my_catalog_entry
CREATE TABLE my_catalog_entry(i INT)
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM my_c') LIMIT 1
# file: test/sql/function/autocomplete/insert_into.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('INS') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT IN') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl VAL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl(c1, c2) VAL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT INTO tbl(c1, c2) SEL') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT OR IG') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('INSERT OR REP') LIMIT 1
# file: test/sql/function/autocomplete/pragma.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA show_t') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA enable_che') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA disable_che') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('PRAGMA thre') LIMIT 1
# file: test/sql/function/autocomplete/scalar_functions.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('select gam') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('select nexta') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('select bit_l') LIMIT 1
# file: test/sql/function/autocomplete/select.test
# setup
CREATE TABLE my_table(my_column INTEGER)
CREATE TABLE MyTable(MyColumn Varchar)
# query
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
# file: test/sql/function/autocomplete/setting.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET e_directory') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET timez') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SET memory') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('set thr') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('set allowed_p') LIMIT 1
# file: test/sql/function/autocomplete/show.test
# setup
CREATE SCHEMA my_schema
CREATE TABLE my_table(my_column INTEGER)
CREATE TABLE my_schema.table_in_schema(my_column INTEGER)
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('DESCR') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SHOW my_') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('SHOW my_s') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('DESCRIBE my_schema.t') LIMIT 1
# file: test/sql/function/autocomplete/table_functions.test
# query
SELECT suggestion, suggestion_start FROM sql_auto_complete('call histo') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call histogram_') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call duckdb_ty') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM duckdb_c') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call read_cs') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('FROM read_csv_a') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('call unnes') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('CALL glo') LIMIT 1
SELECT suggestion, suggestion_start FROM sql_auto_complete('from ran') LIMIT 1
# file: test/sql/function/autocomplete/tpch.test
# query
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
# file: test/sql/function/autocomplete/views.test
# setup
CREATE VIEW v1 AS SELECT 42 my_column_name
CREATE VIEW v2(alias_name) AS SELECT 42 alias
# query
CREATE VIEW v1 AS SELECT 42 my_column_name
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT my_col') LIMIT 1
CREATE VIEW v2(alias_name) AS SELECT 42 alias
SELECT suggestion, suggestion_start FROM sql_auto_complete('SELECT alias') LIMIT 1
# file: test/sql/function/autocomplete/window.test
# query
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
# file: test/sql/function/array/array_and_map.test
# query
SELECT MAP([MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2])], [1])
SELECT MAP([2], [{'key1': MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2])}])
SELECT [MAP([2], [{'key1': MAP([ARRAY_VALUE('1', NULL), ARRAY_VALUE(NULL, '2')], [1, 2]), 'key2': 2}])]
# file: test/sql/function/array/array_cosine_distance.test
# query
INSERT INTO arrays VALUES ([1, 2, 3]), ([4, 5, 6]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
# file: test/sql/function/array/array_distance.test
# query
INSERT INTO arrays VALUES ([1, 2, 3]), ([1, 2, 4]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
# file: test/sql/function/array/array_flatten.test
# query
select flatten([['a'], ['b'], ['c']]::varchar[1][3])
# reject
select flatten(['a', 'b', 'c']::varchar[3])
# file: test/sql/function/array/array_length.test
# setup
create table arrays(a int[3])
# query
SELECT length(array_value(1, 2, 3))
create table arrays(a int[3])
insert into arrays values ([1, 2, 3]), ([4, 5, 6])
select length(a) from arrays
select length(NULL::int[3]) from arrays
insert into arrays values (NULL)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 1)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 2)
# reject
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 3)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 0)
# file: test/sql/function/array/array_list_functions.test
# query
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
# file: test/sql/function/interval/test_date_part.test
# setup
CREATE TABLE intervals(i INTERVAL, s VARCHAR)
# query
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
# reject
SELECT dayofweek(i) FROM intervals
SELECT isodow(i) FROM intervals
SELECT dayofyear(i) FROM intervals
SELECT week(i) FROM intervals
SELECT era(i) FROM intervals
SELECT julian(i) FROM intervals
SELECT extract(era from i) FROM intervals
SELECT extract(julian from i) FROM intervals
# file: test/sql/function/interval/test_extract.test
# setup
CREATE TABLE intervals(i INTERVAL)
# query
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
# reject
SELECT EXTRACT(dayofweek FROM i) FROM intervals
SELECT EXTRACT(isodow FROM i) FROM intervals
SELECT EXTRACT(dayofyear FROM i) FROM intervals
SELECT EXTRACT(week FROM i) FROM intervals
SELECT EXTRACT(yearweek FROM i) FROM intervals
SELECT EXTRACT(doy FROM interval '6 months ago')
SELECT EXTRACT(dow FROM interval '6 months ago')
# file: test/sql/function/interval/test_interval_muldiv.test
# setup
CREATE TABLE INTERVAL_MULDIV_TBL (span interval)
# query
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
# file: test/sql/function/interval/test_interval_trunc.test
# setup
CREATE TABLE intervals(i INTERVAL, s VARCHAR)
# query
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
# file: test/sql/function/date/date_add.test
# setup
CREATE TABLE dates(d DATE)
# query
INSERT INTO dates VALUES (DATE '1992-01-01')
SELECT DATE_ADD(DATE '2008-12-25', INTERVAL 5 DAY) AS five_days_later
SELECT DATE_ADD(TIMESTAMP '2008-12-25 00:00:00', INTERVAL 5 DAY) AS five_days_later
# file: test/sql/function/date/date_diff_extreme_dates.test
# query
SELECT datediff('week', DATE '-5877641-06-25', DATE '5881580-07-10')
SELECT datediff('day', DATE '-5877641-06-25', DATE '5881580-07-10')
SELECT datediff('day', DATE '-5877641-06-25', DATE '5881580-07-10') / 7
SELECT datediff('week', DATE '5881580-07-10', DATE '-5877641-06-25')
SELECT datediff('microsecond', DATE '2000-01-01', DATE '2000-01-02')
SELECT datediff('microsecond', DATE '2000-01-02', DATE '2000-01-01')
# reject
SELECT datediff('microsecond', DATE '-290000-01-01', DATE '290000-01-01')
# file: test/sql/function/date/date_part_stats.test
# setup
CREATE TABLE dates(d DATE)
# query
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
# file: test/sql/function/date/date_trunc_4202.test
# setup
create table t1 (date timestamp)
# query
create table t1 (date timestamp)
insert into t1 values ('2016-12-16T00:00:00.000Z')
insert into t1 values ('2020-02-17T23:59:59.998Z')
insert into t1 values ('2020-02-17T23:59:59.999Z')
insert into t1 values ('2020-02-18T00:00:00.000Z')
select * from t1 WHERE (date_trunc('DAY', T1.date) < ('2020-02-17T23:59:59.999Z'::timestamp)) ORDER BY 1
# file: test/sql/function/date/date_trunc_stats.test
# setup
CREATE table T1(A0 TIMESTAMP)
CREATE TABLE events as FROM (VALUES (TIMESTAMP '1992-09-20 20:38:40', 'Event A'), (TIMESTAMP '1992-09-20 21:45:15', 'Event B'), (TIMESTAMP '1992-09-20 22:15:30', 'Event C')) t(event_time, event_name)
CREATE TABLE users as FROM (VALUES (1, TIMESTAMP '1992-09-20 20:00:00'), (2, TIMESTAMP '1992-09-20 22:05:00')) t(user_id, created_at)
# query
CREATE table T1(A0 TIMESTAMP)
SELECT date_trunc('DAY', A0) FROM T1
CREATE TABLE events as FROM (VALUES (TIMESTAMP '1992-09-20 20:38:40', 'Event A'), (TIMESTAMP '1992-09-20 21:45:15', 'Event B'), (TIMESTAMP '1992-09-20 22:15:30', 'Event C')) t(event_time, event_name)
CREATE TABLE users as FROM (VALUES (1, TIMESTAMP '1992-09-20 20:00:00'), (2, TIMESTAMP '1992-09-20 22:05:00')) t(user_id, created_at)
SELECT u.user_id, date_trunc('minute', e.event_time) AS truncated_minute FROM users u LEFT JOIN events e ON u.user_id = 1 ORDER BY e.event_time ASC
# reject
SELECT datetrunc('milliseconds', DATE '-2005205-7-28')
# file: test/sql/function/date/test_date_part.test
# setup
CREATE TABLE dates(d DATE, s VARCHAR)
CREATE TABLE specifiers (specifier VARCHAR)
# query
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
# reject
SELECT date_part('timezone', d) FROM dates
SELECT date_part('timezone_hour', d) FROM dates
SELECT date_part('timezone_minute', d) FROM dates
SELECT DATE_PART(['hour', 'minute'], '2023-09-17'::DATE) AS parts
# file: test/sql/function/date/test_date_trunc.test
# setup
CREATE TABLE dates(d DATE, s VARCHAR)
CREATE TABLE timestamps(d TIMESTAMP, s VARCHAR)
# query
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
# reject
SELECT date_trunc('duck', TIMESTAMP '2019-01-06 04:03:02') FROM timestamps LIMIT 1
# file: test/sql/function/date/test_extract.test
# setup
CREATE TABLE dates(i DATE)
# query
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
# file: test/sql/function/date/test_extract_edge_cases.test
# query
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
# file: test/sql/function/date/test_extract_month.test
# query
select date '1992-01-01' + interval (i) days, month(date '1992-01-01' + interval (i) days) from range(0, 366) tbl(i)
select date '1993-01-01' + interval (i) days, month(date '1993-01-01' + interval (i) days) from range(0, 366) tbl(i)
# file: test/sql/function/date/test_extract_year.test
# setup
CREATE TABLE dates AS SELECT date '1970-01-01' + concat(i, ' years')::interval AS d from range(0, 430) tbl(i)
CREATE TABLE dates2 AS SELECT date '1970-01-01' + concat(i * 6, ' months')::interval AS d from range(0, 200) tbl(i)
# query
CREATE TABLE dates AS SELECT date '1970-01-01' + concat(i, ' years')::interval AS d from range(0, 430) tbl(i)
CREATE TABLE dates2 AS SELECT date '1970-01-01' + concat(i * 6, ' months')::interval AS d from range(0, 200) tbl(i)
SELECT EXTRACT(year FROM d) FROM dates ORDER BY 1
SELECT EXTRACT(year FROM d) FROM dates2 ORDER BY 1
# file: test/sql/function/date/test_strftime.test
# query
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
# reject
SELECT strftime(d, d::VARCHAR) FROM dates ORDER BY d
SELECT strftime(DATE '1992-01-01', '%')
SELECT strftime(DATE '1992-01-01', '%R')
SELECT strptime('-1', '%g')
SELECT strptime('1000', '%g')
SELECT strftime('%Y', '1992-01-01')
# file: test/sql/function/date/test_strftime_exhaustive.test
# query
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
# reject
SELECT strftime(date '-99999-01-01', random()::varchar)
# file: test/sql/function/date/test_time_bucket_date.test
# setup
CREATE TABLE dates(w INTERVAL, d DATE, shift INTERVAL, origin DATE)
# query
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
# reject
select time_bucket('-3 hours'::interval, '2019-04-05'::date)
select time_bucket('-3 hours'::interval, '2019-04-05'::date, '1 hour 30 minutes'::interval)
select time_bucket('-3 hours'::interval, '2019-04-05'::date, '2019-04-05'::date)
select time_bucket('-1 month'::interval, '2019-04-05'::date)
select time_bucket('-1 month'::interval, '2019-04-05'::date, '1 week'::interval)
select time_bucket('-1 month'::interval, '2019-04-05'::date, '2019-04-05'::date)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05'::date)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05'::date, '1 hour 30 minutes'::interval)
# file: test/sql/function/enum/test_enum_code.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy', 'anxious')
CREATE TABLE test (x mood)
# query
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy', 'anxious')
CREATE TABLE test (x mood)
INSERT INTO test VALUES ('ok'), ('sad'), ('anxious'), ('happy')
SELECT enum_code(x) FROM test
PREPARE p1 as SELECT enum_code(x) FROM test
EXECUTE p1
PREPARE p2 as SELECT enum_code(?)
EXECUTE p2('happy'::mood)
# reject
SELECT enum_code('bla')
# file: test/sql/function/enum/test_enum_first.test
# setup
CREATE TYPE rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
# query
CREATE TYPE rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
SELECT enum_first(null::rainbow)
# reject
SELECT enum_first('bla')
# file: test/sql/function/enum/test_enum_last.test
# setup
CREATE TYPE rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
# query
SELECT enum_last(null::rainbow)
# reject
SELECT enum_last('bla')
# file: test/sql/function/enum/test_enum_range.test
# setup
CREATE TYPE rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
CREATE TYPE currency AS ENUM ('usd', 'brl', 'eur')
# query
CREATE TYPE currency AS ENUM ('usd', 'brl', 'eur')
SELECT enum_range(null::rainbow)
SELECT enum_range_boundary('orange'::rainbow, 'green'::rainbow)
SELECT enum_range_boundary('green'::rainbow, 'orange'::rainbow)
SELECT enum_range_boundary(NULL, 'green'::rainbow)
SELECT enum_range_boundary('orange'::rainbow, NULL)
# reject
SELECT enum_range_boundary('orange'::rainbow, 'brl'::currency)
SELECT enum_range_boundary(NULL, NULL)
SELECT enum_range_boundary('orange'::rainbow, 1)
SELECT enum_range_boundary(1, 'orange'::rainbow)
# file: test/sql/function/time/epoch.test
# query
select epoch(TIME '14:21:13')
select extract(epoch from TIME '14:21:13')
select extract(seconds from TIME '14:21:13')
# file: test/sql/function/time/test_date_part.test
# setup
CREATE TABLE times(d TIME, s VARCHAR)
# query
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
# reject
SELECT era(i) FROM times
SELECT year(i) FROM times
SELECT month(i) FROM times
SELECT day(i) FROM times
SELECT decade(i) FROM times
SELECT century(i) FROM times
SELECT millennium(i) FROM times
SELECT quarter(i) FROM times
# file: test/sql/function/time/test_extract.test
# setup
CREATE TABLE times(i TIME)
# query
CREATE TABLE times(i TIME)
INSERT INTO times VALUES ('00:01:20'), ('20:08:10.998'), ('20:08:10.33'), ('20:08:10.001'), (NULL)
SELECT EXTRACT(second FROM i) FROM times
SELECT EXTRACT(minute FROM i) FROM times
SELECT EXTRACT(hour FROM i) FROM times
SELECT EXTRACT(milliseconds FROM i) FROM times
SELECT EXTRACT(microseconds FROM i) FROM times
SELECT EXTRACT(epoch FROM i) FROM times
# reject
SELECT EXTRACT(year FROM i) FROM times
SELECT EXTRACT(month FROM i) FROM times
SELECT EXTRACT(day FROM i) FROM times
SELECT EXTRACT(decade FROM i) FROM times
SELECT EXTRACT(century FROM i) FROM times
SELECT EXTRACT(millennium FROM i) FROM times
SELECT EXTRACT(quarter FROM i) FROM times
SELECT EXTRACT(dayofweek FROM i) FROM times
# file: test/sql/function/time/test_extract_stats.test
# setup
CREATE TABLE times(i TIME)
# query
SELECT stats(EXTRACT(second FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(minute FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(hour FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(milliseconds FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(microseconds FROM i)) FROM times LIMIT 1
SELECT stats(EXTRACT(epoch FROM i)) FROM times LIMIT 1
# file: test/sql/function/numeric/abs.test
# query
SELECT abs('-0.0'::float), abs('-0.0'::double)
# file: test/sql/function/numeric/decimal_mod.test
# query
SELECT 10 % 2.4, -10 % 2.4
SELECT 10.0 % 2.4, -10.0 % 2.4
SELECT 12345678901111111 % 2.0
select 12345678901234567890 % 123
SELECT 10000000000000000000000000000000000001::DECIMAL(38,0) % 0.00000000000000000000000000000000004
SELECT typeof(10.0 % 2.0), typeof(10.0 % 2.0 % 2.0 % 2.0)
SELECT 10.0 % 0.0
# file: test/sql/function/numeric/set_seed_for_sample.test
# setup
create table t1 as select * from generate_series(1,50) as t(number)
# query
create table t1 as select * from generate_series(1,50) as t(number)
select * from t1 using sample 5
# file: test/sql/function/numeric/test_arithmetic_aliases.test
# setup
CREATE TABLE test(a integer)
# query
CREATE TABLE test(a integer)
insert into test values (1), (2), (3), (NULL)
select add(a,a) from test
select subtract(a,a) from test
select multiply(a,a) from test
select divide(a,a) from test
# file: test/sql/function/numeric/test_bit_count.test
# setup
CREATE TABLE bits(t tinyint, s smallint, i integer, b bigint, h hugeint)
# query
CREATE TABLE bits(t tinyint, s smallint, i integer, b bigint, h hugeint)
INSERT INTO bits VALUES (NULL, NULL, NULL, NULL, NULL), (31, 1023, 11834119, 50827156903621017, 3141592653589793238462643383279528841), (-59, -517, -575693, -9876543210, -148873535527910577765226390751398592512)
select bit_count(t), bit_count(s), bit_count(i), bit_count(b), bit_count(h) from bits
# file: test/sql/function/numeric/test_even.test
# query
select i, even(i + 0.4) from generate_series(-4,4) tbl(i)
select i, even(i + 0.9) from generate_series(-4,4) tbl(i)
SELECT even(19.4), even(-19.4)
SELECT even(8.9), even(-8.9)
SELECT even(45::DOUBLE), even(-35::DOUBLE)
SELECT even(NULL)
SELECT even(1.7976931348623155e+308)
SELECT even(-1.7976931348623155e+308)
# reject
SELECT even('abcd')
# file: test/sql/function/numeric/test_factorial.test
# query
SELECT factorial(0)
SELECT factorial(NULL)
SELECT factorial(2)
SELECT factorial(10)
SELECT 10!
SELECT factorial(20)
SELECT factorial(30)
# reject
SELECT factorial(-1)
SELECT factorial(40)
# file: test/sql/function/numeric/test_fdiv_fmod.test
# setup
CREATE TABLE rs(x DOUBLE, y INTEGER)
# query
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
# file: test/sql/function/numeric/test_floor_ceil.test
# setup
CREATE TABLE numbers(n DOUBLE)
# query
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
# file: test/sql/function/numeric/test_gamma.test
# query
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
# reject
SELECT gamma(0)
SELECT gamma('asdf')
SELECT lgamma(0)
SELECT lgamma('asdf')
# file: test/sql/function/numeric/test_gcd_lcm.test
# query
SELECT a, b, gcd(a, b), gcd(a, -b), gcd(b, a), gcd(-b, a) FROM (VALUES (0::int8, 0::int8), (0::int8, 29893644334::int8), (288484263558::int8, 29893644334::int8), (-288484263558::int8, 29893644334::int8), ((-9223372036854775808)::int8, 1::int8), ((-9223372036854775808)::int8, 9223372036854775807::int8), ((-9223372036854775808)::int8, 4611686018427387904::int8)) AS v(a, b)
SELECT gcd(42, NULL)
select lcm(120,25)
SELECT a, b, lcm(a, b), lcm(a, -b), lcm(b, a), lcm(-b, a) FROM (VALUES (0::int8, 0::int8), (0::int8, 29893644334::int8), (29893644334::int8, 29893644334::int8), (288484263558::int8, 29893644334::int8), (-288484263558::int8, 29893644334::int8), ((-9223372036854775808)::int8, 0::int8)) AS v(a, b)
SELECT lcm(42, NULL)
# reject
SELECT gcd(42, 'abcd')
SELECT lcm(42, 'abcd')
select lcm(4200000000000000000,5700000000000000000)
# file: test/sql/function/numeric/test_geomean.test
# setup
CREATE TABLE numbers(x DOUBLE)
# query
CREATE TABLE numbers(x DOUBLE)
INSERT INTO numbers VALUES (NULL), (1), (2)
SELECT geomean(x) FROM numbers
SELECT geomean(x::integer) FROM numbers
SELECT geomean(i) FROM generate_series(1000, 2000) tbl(i)
# file: test/sql/function/numeric/test_invalid_math.test
# query
SELECT SQRT(0)
SELECT POW(1e300,100), POW(-1e300,100), POW(-1.0, 0.5)
SELECT EXP(1e300), EXP(1e100)
SELECT DEGREES(1e308)
# reject
SELECT log(0)
SELECT log(-1)
SELECT ln(0)
SELECT ln(-1)
SELECT log10(0)
SELECT log10(-1)
SELECT sqrt(-1)
# file: test/sql/function/numeric/test_is_nan.test
# query
INSERT INTO floats VALUES (3), ('nan'), ('inf'), ('-inf'), (NULL)
SELECT f, isnan(f), isinf(f), isfinite(f) FROM floats ORDER BY f
DROP TABLE floats
# file: test/sql/function/numeric/test_mod.test
# setup
CREATE TABLE modme(a DOUBLE, b INTEGER)
# query
CREATE TABLE modme(a DOUBLE, b INTEGER)
INSERT INTO modme VALUES (42.123456, 3)
select mod(a, 40) from modme
select mod(42, 0)
select mod(a, 2) from modme
select mod(b, 2.1) from modme
# file: test/sql/function/numeric/test_nextafter.test
# setup
create table test (a FLOAT)
create table test_twoc (a FLOAT, b FLOAT)
# query
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
# reject
select nextafter()
select nextafter('bla','bla')
# file: test/sql/function/numeric/test_pg_math.test
# query
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
# reject
select log(0, 64)
select log(2, 0)
select log(-1, 64)
select log(2, -1)
select log(1, 64)
select log('-Inf'::DOUBLE, 64)
select log(64, '-Inf'::DOUBLE)
# file: test/sql/function/numeric/test_pow.test
# setup
CREATE TABLE powerme(a DOUBLE, b INTEGER)
# query
CREATE TABLE powerme(a DOUBLE, b INTEGER)
INSERT INTO powerme VALUES (2.1, 3)
select pow(a, 0) from powerme
select pow(b, -2) from powerme
select pow(a, b) from powerme
select pow(b, a) from powerme
select power(b, a) from powerme
# file: test/sql/function/numeric/test_random.test
# setup
CREATE TABLE t4 AS SELECT [random() + range * 0 for a IN range(1)] FROM range(2)
CREATE TEMPORARY TABLE t1 AS SELECT RANDOM() a
CREATE TEMPORARY TABLE t2 AS SELECT RANDOM() b
CREATE TEMPORARY TABLE t3 AS SELECT RANDOM() c
CREATE TABLE seeds(a DOUBLE)
CREATE TABLE numbers(a INTEGER)
# query
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
# reject
select setseed(1.1)
select setseed(-1.1)
# file: test/sql/function/numeric/test_round.test
# setup
CREATE TABLE roundme(a DOUBLE, b INTEGER)
# query
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
# file: test/sql/function/numeric/test_round_even.test
# query
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
# file: test/sql/function/numeric/test_round_integers.test
# setup
CREATE TABLE zz AS SELECT CAST(i AS SMALLINT) AS id, CAST(i AS SMALLINT) AS si FROM generate_series(1, 1000) t(i)
# query
CREATE TABLE zz AS SELECT CAST(i AS SMALLINT) AS id, CAST(i AS SMALLINT) AS si FROM generate_series(1, 1000) t(i)
SELECT ROUND(53) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
SELECT ROUND(53, si) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
SELECT ROUND(53, -si) AS ag_column3 FROM zz GROUP BY ag_column3 ORDER BY ag_column3
select round(100::INTEGER, int) from test_all_types()
# file: test/sql/function/numeric/test_sign_bit.test
# query
INSERT INTO floats VALUES (3), (1.0::float), (-0.0::float), ('inf'), ('-inf'), (NULL)
SELECT f, signbit(f), isinf(f), isfinite(f) FROM floats ORDER BY f
SELECT signbit(1.0 / 0.0)
# file: test/sql/function/numeric/test_trigo.test
# setup
CREATE TABLE numbers(n DOUBLE)
# query
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
# reject
SELECT cast(ASIN(n)*1000 as bigint) FROM numbers ORDER BY n
select asin(-2)
select acos(-2)
# file: test/sql/function/numeric/test_trunc.test
# setup
CREATE TABLE truncme(a DOUBLE, b INTEGER, c UINTEGER)
# query
CREATE TABLE truncme(a DOUBLE, b INTEGER, c UINTEGER)
INSERT INTO truncme VALUES (42.123456, 3, 19), (-3.141592, -7, 5)
# file: test/sql/function/numeric/test_trunc_precision.test
# setup
CREATE TABLE truncme(a DOUBLE, b INTEGER, c UINTEGER)
# query
INSERT INTO truncme VALUES (42.123456, 37, 19), (-3.141592, -75, 5)
# file: test/sql/function/numeric/test_type_resolution.test
# query
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
# reject
SELECT 1::TINYINT + 1::VARCHAR
SELECT 1::SMALLINT + 1::VARCHAR
SELECT 1::INTEGER + 1::VARCHAR
SELECT 1::BIGINT + 1::VARCHAR
SELECT 1::REAL + 1::VARCHAR
SELECT 1::DOUBLE + 1::VARCHAR
# file: test/sql/function/numeric/test_unary.test
# setup
CREATE TABLE test(i INTEGER)
CREATE TABLE minima (t TINYINT, s SMALLINT, i INTEGER, b BIGINT)
CREATE TABLE dates(d DATE)
# query
INSERT INTO test VALUES (2)
SELECT ++-++-+i FROM test
SELECT +i FROM test
SELECT -i FROM test
SELECT +++++++i FROM test
SELECT -+-+-+-+-i FROM test
CREATE TABLE minima (t TINYINT, s SMALLINT, i INTEGER, b BIGINT)
INSERT INTO minima VALUES (-128, -32768, -2147483648, -9223372036854775808)
INSERT INTO dates VALUES ('1992-02-02')
# reject
SELECT -t from minima
SELECT -s from minima
SELECT -i from minima
SELECT -b from minima
SELECT +'hello'
SELECT -'hello'
SELECT +d FROM dates
SELECT -d FROM dates
# file: test/sql/function/list/array_length.test
# setup
CREATE TABLE lists AS SELECT * FROM (VALUES ([1, 2]), ([NULL]), (NULL), ([]), ([3, 4, 5, 6, 7])) tbl(l)
# query
SELECT length([1,2,3])
SELECT length([])
SELECT len(NULL)
SELECT array_length(ARRAY[1, 2, 3], 1)
SELECT len([1]) FROM range(3)
CREATE TABLE lists AS SELECT * FROM (VALUES ([1, 2]), ([NULL]), (NULL), ([]), ([3, 4, 5, 6, 7])) tbl(l)
SELECT len(l) FROM lists
# reject
SELECT array_length(ARRAY[1, 2, 3], 2)
SELECT array_length(ARRAY[1, 2, 3], 0)
# file: test/sql/function/list/array_to_string.test
# query
SELECT array_to_string([1,2,3], '')
SELECT array_to_string([1,2,3], '-')
SELECT array_to_string(NULL, '-')
SELECT array_to_string([1, 2, 3], NULL)
SELECT array_to_string([], '-')
SELECT array_to_string([i, i + 1], '-') FROM range(6) t(i) WHERE i<=2 OR i>4
# reject
SELECT array_to_string([1, 2, 3], k) FROM repeat(',', 5) t(k)
# file: test/sql/function/list/array_to_string_comma_default.test
# query
SELECT array_to_string_comma_default([1,2,3])
SELECT array_to_string_comma_default([1,2,3], sep:=',')
SELECT array_to_string_comma_default([1,2,3], sep:='')
SELECT array_to_string_comma_default([1,2,3], sep:='-')
SELECT array_to_string_comma_default(NULL, sep:='-')
SELECT array_to_string_comma_default([1, 2, 3], sep:=NULL)
SELECT array_to_string_comma_default([], sep:='-')
SELECT array_to_string_comma_default([i, i + 1], sep:='-') FROM range(6) t(i) WHERE i<=2 OR i>4
# reject
SELECT array_to_string_comma_default([1, 2, 3], sep:=k) FROM repeat(',', 5) t(k)
# file: test/sql/function/list/flatten.test
# setup
CREATE TABLE nums AS SELECT range % 8 i, range j FROM range(16)
CREATE TABLE lists AS SELECT i % 4 i, list(j ORDER BY rowid) j FROM nums GROUP BY i
CREATE TABLE nested_lists AS SELECT i, list_sort(list(j ORDER BY rowid)) j FROM lists GROUP BY i ORDER BY i
# query
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
# reject
SELECT flatten(1)
select flatten(42)
select flatten([1, 2])
# file: test/sql/function/list/generate_series.test
# query
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
# file: test/sql/function/list/generate_series_timestamp.test
# query
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
# reject
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-06-01', interval '3' month - interval '3' day)
SELECT generate_series('294247-01-10'::TIMESTAMP, 'infinity'::TIMESTAMP, INTERVAL '1 DAY')
SELECT range('294247-01-10'::TIMESTAMP, 'infinity'::TIMESTAMP, INTERVAL '1 DAY')
SELECT generate_series('-infinity'::TIMESTAMP, '290309-12-22 (BC) 00:00:00'::TIMESTAMP, INTERVAL '1 DAY')
SELECT range('-infinity'::TIMESTAMP, '290309-12-22 (BC) 00:00:00'::TIMESTAMP, INTERVAL '1 DAY')
# file: test/sql/function/list/generate_subscripts.test
# query
SELECT generate_subscripts([4,5,6], 1)
SELECT generate_subscripts([], 1)
SELECT generate_subscripts(NULL, 1)
# reject
SELECT generate_subscripts([[1,2],[3,4],[5,6]], 2)
# file: test/sql/function/list/icu_generate_series_timestamptz.test
# query
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
# reject
SELECT generate_series(timestamptz '2020-01-01', timestamptz '2020-06-01', interval '3' month - interval '3' day)
SELECT generate_series('294247-01-10'::TIMESTAMPTZ, 'infinity'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT range('294247-01-10'::TIMESTAMPTZ, 'infinity'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT generate_series('-infinity'::TIMESTAMPTZ, '290309-12-22 (BC) 00:00:00'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT range('-infinity'::TIMESTAMPTZ, '290309-12-22 (BC) 00:00:00'::TIMESTAMPTZ, INTERVAL '1 DAY')
# file: test/sql/function/list/list_concat.test
# setup
CREATE TABLE test AS SELECT range % 4 i, range j, range k FROM range(16)
CREATE TABLE lists AS SELECT i, list(j) j, list(k) k FROM test GROUP BY i
# query
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
# reject
SELECT list_concat([1, 2], 3)
SELECT i, list_concat(j, cast(k AS VARCHAR)) FROM lists
SELECT concat([42], [84], 'str')
# file: test/sql/function/list/list_contains.test
# setup
create table TEST2 (i int[], j int)
create table STR_TEST (i string[])
CREATE TABLE functions (function_name varchar, function_type varchar, parameter_types varchar[])
CREATE TABLE test (id int, name text[])
CREATE TABLE list_of_list(l1 int[][], l2 int[][])
# query
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
# reject
SELECT list_contains([1.0,2.0,3.0], 'a')
SELECT list_contains('a', 'a')
SELECT list_contains([[1,2,3],[1],[1,2,3])
SELECT list_contains([[1,2,3],[1],[1,2,3]])
SELECT list_contains(1)
SELECT list_contains(1,1)
# file: test/sql/function/list/list_cosine_similarity.test
# query
INSERT INTO lists VALUES ([1, 2, 3]), ([4, 5, 6]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
SELECT list_cosine_similarity(l, [1, 2, 3]) FROM lists
SELECT list_cosine_similarity([], [])
# file: test/sql/function/list/list_distance.test
# query
INSERT INTO lists VALUES ([1, 2, 3]), ([1, 2, 4]), ([7, 8, 9]), ([-1, -2, -3]), (NULL)
SELECT list_distance(l, [1, 2, 3]) FROM lists
SELECT list_distance([], [])
# file: test/sql/function/list/list_distinct.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE integers (l integer[])
CREATE TABLE enums (e mood[])
CREATE TABLE wheretest (name VARCHAR, l INTEGER[])
CREATE TABLE all_types AS SELECT * FROM test_all_types()
# query
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
# reject
SELECT list_distinct()
SELECT list_distinct(*)
SELECT list_distinct([1, 2], 2)
SELECT list_distinct(NULL::boolean)
# file: test/sql/function/list/list_has_any_and_has_all.test
# setup
CREATE TABLE list_data(l1 int[], l2 int[])
create table list_of_list(l1 int[][], l2 int[][])
create table list_of_strings(l1 string[], l2 string[])
create table tbl(l1 int[], l2 int[])
# query
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
# reject
select list_has_any(l1) from list_of_strings
select list_has_any(l1, l2, l1) from list_of_strings
select list_has_all(l1) from list_of_strings
select list_has_all(l1, l2, l1) from list_of_strings
select list_has_all([1, 2], 1)
select list_has_any([[1,2], [2,4]], ['abc', 'def'])
select 'hello' && l1 from tbl
select 'hello' @> l1 from tbl
# file: test/sql/function/list/list_inner_product.test
# query
SELECT list_inner_product([], [])
SELECT list_inner_product(l, [1, 2, 3]) FROM lists
# file: test/sql/function/list/list_intersect.test
# setup
CREATE TABLE list_data(l1 int[], l2 int[])
create table list_of_list(l1 int[][], l2 int[][])
create table list_of_strings(l1 string[], l2 string[])
create table large_lists(l1 int[], l2 int[])
CREATE TABLE all_types AS SELECT * FROM test_all_types()
# query
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
# reject
select list_intersect(l1) from list_of_strings
select list_intersect(l1, l2, l1) from list_of_strings
select list_intersect([[1,2], [2,4]], ['abc', 'def'])
# file: test/sql/function/list/list_position.test
# setup
create table TEST2 (i int[], j int)
create table TEST(i int[], j int)
CREATE TABLE NULL_TABLE (n int[], i int)
create table STR_TEST (i string[], j string)
CREATE TABLE test0 (i int[][], j int[])
# query
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
# reject
SELECT list_position([1.0,2.0,3.0], 'a')
SELECT list_position('a', 'a')
SELECT list_position([[1,2,3],[1],[1,2,3])
SELECT list_position([[1,2,3],[1],[1,2,3]])
SELECT list_position(1)
SELECT list_position(1,1)
# file: test/sql/function/list/list_position_nan.test
# query
SELECT list_position(['NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_position([NULL, 0, 'NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_contains([NULL, 0, 'NaN'::DOUBLE], 'NaN'::DOUBLE)
SELECT list_position([[[NULL, 42]]], [[NULL, 42]])
# file: test/sql/function/list/list_resize.test
# setup
create table tbl(a int[], b int)
create table string_tbl(a string[], b int)
CREATE TABLE nulls(l INT[], b INT)
create table list_tbl(a int[][], b int)
CREATE TABLE def(tbl INT[], b INT, d INT)
CREATE TABLE bool_table(a bool[], b int)
# query
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
# reject
SELECT LIST_RESIZE([1, 2, 3], 9999999999999999999)
SELECT LIST_RESIZE([1, 2, 3], 4000999999999999999)
# file: test/sql/function/list/list_resize_error.test
# query
prepare q1 as select list_resize(?, ?)
prepare q2 as select array_resize(?, ?)
prepare q3 as select list_resize(?, ?, ?)
prepare q4 as select array_resize(?, ?, ?)
# file: test/sql/function/list/list_reverse.test
# setup
create or replace table tbl_big as select range(5000) as list
CREATE TABLE tbl (id INTEGER, list INTEGER[])
CREATE TABLE tbl2 (id INTEGER, list INTEGER[])
CREATE TABLE palindromes (s VARCHAR)
CREATE OR REPLACE TABLE integers AS SELECT LIST(i) AS i FROM range(1, 10, 1) t1(i)
CREATE OR REPLACE TABLE lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
# query
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
# reject
SELECT list_reverse()
SELECT list_reverse(42)
SELECT list_reverse ([1, 3, 2, 42, 117,, NULL])
SELECT list_reverse(*)
SELECT list_reverse([1, 2], 2)
# file: test/sql/function/list/list_sort_having.test
# setup
create or replace table test1 as ( select 'new_customers' as child, 'dim_model_7' as parent union all select 'exposure_1' as child, 'dim_model_7' as parent union all select 'exposure_1' as child, 'exposure_1' as parent union all select 'fct_model_6' as child, 'fct_model_6' as parent union all select 'exposure_1' as child, 'fct_model_6' as parent union all select 'report_1' as child, 'fct_model_6' as parent union all select 'report_2' as child, 'fct_model_6' as parent union all select 'report_3' as child, 'fct_model_6' as parent union all select 'fct_model_9' as child, 'fct_model_9' as parent union all select 'stg_model_5' as child, 'fct_model_9' as parent union all select 'int_model_4' as child, 'int_model_4' as parent union all select 'int_model_5' as child, 'int_model_4' as parent union all select 'dim_model_7' as child, 'int_model_4' as parent union all select 'new_customers' as child, 'int_model_4' as parent union all select 'exposure_1' as child, 'int_model_4' as parent union all select 'int_model_5' as child, 'int_model_5' as parent union all select 'dim_model_7' as child, 'int_model_5' as parent union all select 'new_customers' as child, 'int_model_5' as parent union all select 'exposure_1' as child, 'int_model_5' as parent union all select 'model_8' as child, 'model_8' as parent union all select 'new_customers' as child, 'new_customers' as parent union all select 'report_1' as child, 'report_1' as parent union all select 'report_2' as child, 'report_2' as parent union all select 'report_3' as child, 'report_3' as parent )
# query
select child, count(*) as cnt, list_sort( list( parent ) ) as source_parents from test1 group by 1 having cnt > 1
# file: test/sql/function/list/list_unique.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE integers (l integer[])
CREATE TABLE enums (e mood[])
# query
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
# reject
SELECT list_unique()
SELECT list_unique(*)
SELECT list_unique([1, 2], 2)
SELECT list_unique(NULL::tinyint)
# file: test/sql/function/list/list_value_arrays.test
# setup
CREATE TABLE array_table (a STRING[3], b STRING[3], c STRING[3])
CREATE TABLE nested_array_table (a INTEGER[2][2], b INTEGER[2][2], c INTEGER[2][2])
CREATE TABLE mixed_array_table (a INTEGER[2], b VARCHAR[2], c DOUBLE[2])
# query
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
# reject
SELECT list_value([1, 2]::INTEGER[2], [3, 4, 5]::INTEGER[3], [6]::INTEGER[1])
SELECT list_value(a, b, c) FROM mixed_array_table
# file: test/sql/function/list/list_value_nested_lists.test
# setup
CREATE TABLE test_table (c1 INTEGER[], c2 INTEGER[], c3 INTEGER[])
CREATE TABLE struct_table(a ROW(a INTEGER, b INTEGER)[], b ROW(a INTEGER, b INTEGER)[], c ROW(a INTEGER, b INTEGER)[])
CREATE TABLE string_table(a VARCHAR[], b VARCHAR[], c VARCHAR[])
CREATE TABLE nested_list_table(a INTEGER[][], b INTEGER[][], c INTEGER[][])
# query
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
# reject
SELECT LIST_VALUE([1, 1], ['a', 'a'], [ROW(2, 2), ROW(3, 3)])
# file: test/sql/function/list/list_value_structs.test
# setup
CREATE TABLE tbl (s1 struct(a int, b varchar), s2 struct(a int, b varchar), s3 struct(a int, b varchar))
CREATE TABLE mixed_structs (s struct(a int[], b varchar, c int, d varchar[], e struct(a int, b varchar)))
# query
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
# file: test/sql/function/list/list_where.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE integers (i int[])
CREATE TABLE selections (j boolean[])
CREATE TABLE lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE bools AS SELECT range % 4 a, list((range % 2):: bool) s FROM range(10000) GROUP BY range % 4
CREATE TABLE enums (e mood[])
# query
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
# reject
SELECT list_where([1,2,3], [True, NULL, FALSE])
# file: test/sql/function/list/list_zip.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE integers (i int[])
CREATE TABLE bools (b bool)
CREATE TABLE integers2 (j int[])
CREATE TABLE lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE enums (e mood[])
# query
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
# reject
SELECT list_zip('')
SELECT list_zip(3, 4)
SELECT list_zip(FALSE)
SELECT list_zip(TRUE)
# file: test/sql/function/list/repeat_list.test
# query
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
# reject
SELECT repeat([1], 99999999999999999)
SELECT repeat([1, 2, 3], 6148914691236517206)
# file: test/sql/function/list/test_lambda_with_struct_aliases.test
# setup
CREATE TABLE addresses (i INT, b INT)
CREATE TABLE test (a VARCHAR[])
# query
SELECT COALESCE(*COLUMNS(lambda c: {'title': c}.title IN ('a', 'c'))) FROM (SELECT NULL, 2, 3) t(a, b, c)
CREATE TABLE addresses (i INT, b INT)
INSERT INTO addresses VALUES (1, 10), (2, 20), (1, 52), (3, 7)
SELECT i, sum(b) FROM addresses GROUP BY i HAVING sum(b) >= list_sum(list_transform([20], lambda x: {'title': x}.title + 30))
SELECT list_transform([10], lambda x: sum(1) + {'title': x}.title)
CREATE TABLE test (a VARCHAR[])
INSERT INTO test VALUES (NULL), ([]), (['asdf']), (['qwer', 'CXZASDF'])
ALTER TABLE test ALTER COLUMN a TYPE STRUCT(title VARCHAR)[] USING (list_transform(a, lambda x: {'title': x}))
SELECT a FROM test ORDER BY ALL
# file: test/sql/function/list/lambdas/expression_iterator_cases.test
# setup
CREATE TABLE my_window (l integer[], g integer, o integer)
CREATE MACRO list_contains_macro(x, y) AS (SELECT list_contains(x, y))
# query
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
# reject
SELECT list_transform([2], lambda x: (SELECT 1 - x) * x)
SELECT list_filter([2], lambda x: (SELECT 1 - x) * x > 2)
SELECT list_filter([[1, 2, 1], [1, 2, 3], [1, 1, 1]], lambda x: list_contains_macro(x, 3))
SELECT list_transform([1], lambda x: x = UNNEST([1]))
SELECT list_filter([1], lambda x: x = UNNEST([1]))
# file: test/sql/function/list/lambdas/filter.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE empty_lists (l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE corr_test (n integer, l varchar[], g integer)
CREATE TABLE lambdas AS SELECT [5, 6] AS col1, [4, 8] AS col2
# query
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
# reject
SELECT list_transform([['abc']], lambda x: list_filter(x, lambda y: y))
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
CREATE TABLE incorrect_test (i INTEGER)
CREATE TABLE l_filter_test (l integer[])
CREATE TABLE tbl AS SELECT {'a': 10} AS s
CREATE TABLE nested_list(i INT[][], other INT[])
INSERT INTO nested_list VALUES ([[1, 2]], [3, 4])
CREATE TABLE map_tbl(m MAP(INTEGER, INTEGER))
CREATE TABLE dummy_tbl (y INT)
SET lambda_syntax='ENABLE_SINGLE_ARROW'
CREATE OR REPLACE FUNCTION transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
# reject
SELECT list_reduce([1], x -> x, 3)
SELECT list_reduce([True], x -> x, x -> x)
SELECT [split('01:08:22', ':'), x -> CAST (x AS INTEGER)]
select list_apply(i, x -> x * 3 + 2 / zz) from (values (list_value(1, 2, 3))) tbl(i)
select x -> x + 1 from (values (list_value(1, 2, 3))) tbl(i)
select list_apply(i, y + 1 -> x + 1) from (values (list_value(1, 2, 3))) tbl(i)
SELECT list_apply(i, a.x -> x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
select list_apply(i, x -> x + 1 AND y + 1) from (values (list_value(1, 2, 3))) tbl(i)
# file: test/sql/function/list/lambdas/lambda_scope.test
# setup
CREATE TABLE t1 AS SELECT [1, 2, 3] AS x
CREATE TABLE t2 AS SELECT [[1], [2], [3]] AS x
CREATE TABLE l_test (l integer[])
CREATE TABLE l_filter_test (l integer[])
CREATE TABLE qualified_tbl (x INTEGER[])
CREATE TABLE tbl_qualified AS SELECT 42 AS x
# query
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
# reject
SELECT list_apply(i, lambda a.x: a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
SELECT list_transform(qualified_tbl.x, lambda qualified_tbl.x: qualified_tbl.x + 1) FROM qualified_tbl
SELECT list_transform([1,2,3], lambda sqrt(xxx.z): xxx.z + 1) AS l
SELECT list_reduce([1, 2, 3, 4], lambda x *++++++++* y: x - y) AS l
# file: test/sql/function/list/lambdas/lambdas_and_functions.test
# setup
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
# query
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: z) AS row )
FROM demo(3, 0)
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: 0 + z) AS row )
CREATE OR REPLACE function demo(n, z) AS TABLE ( SELECT list_transform(range(0, n), lambda x: (z -> 'a')) AS row )
FROM demo(3, {'a': 2})
# file: test/sql/function/list/lambdas/lambdas_and_group_by.test
# setup
CREATE TABLE tbl (tag_product VARCHAR)
CREATE TABLE uniform_purchase_forecast AS SELECT 'gold' AS color, 10 AS forecast UNION ALL SELECT 'blue', 15 UNION ALL SELECT 'red', 300
# query
CREATE TABLE tbl (tag_product VARCHAR)
INSERT INTO tbl VALUES ('milk chickpeas apples'), ('chocolate pepper')
SELECT tag_product, list_aggr(list_transform( string_split(tag_product, ' '), lambda word: lower(word)), 'string_agg', ',') AS tag_material, FROM tbl GROUP BY tag_product ORDER BY ALL
SELECT 1, list_transform([5, 4, 3], lambda x: x + 1) AS lst GROUP BY 1
CREATE TABLE uniform_purchase_forecast AS SELECT 'gold' AS color, 10 AS forecast UNION ALL SELECT 'blue', 15 UNION ALL SELECT 'red', 300
FROM uniform_purchase_forecast SELECT list(forecast).list_transform(lambda x: x + 10)
FROM (SELECT 1) GROUP BY ALL HAVING list_filter(NULL, lambda x: x)
FROM test_all_types() GROUP BY ALL HAVING array_intersect(NULL, NULL)
SELECT x FROM (VALUES (42)) t(x) GROUP BY x HAVING list_filter(NULL, lambda lambda_param: lambda_param = 1)
# file: test/sql/function/list/lambdas/lambdas_and_macros.test
# setup
CREATE TABLE test AS SELECT range i FROM range(3)
CREATE MACRO list_contains_macro(x, y) AS (list_contains(x, y))
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, lambda x: x + num))
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, lambda a: x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, lambda x, y: x + y + num))
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, lambda x, y: list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, lambda x: x + y + z))
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], lambda x: 0))
# query
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
# reject
CREATE MACRO my_macro(i) AS (SELECT i IN (SELECT i FROM test))
SELECT some_macro([1, 2], 3, 4)
SELECT other_reduce_macro([1, 2, 3, 4], 5, 6)
# file: test/sql/function/list/lambdas/list_comprehension.test
# setup
CREATE TABLE fruit_tbl AS SELECT ['apple', 'banana', 'cherry', 'kiwi', 'mango'] fruits
CREATE TABLE word_tbl AS SELECT ['goodbye', 'cruel', 'world'] words
# query
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
# reject
SELECT [a for a, b, c in [1, 2, 3]]
SELECT [a for a[1], b[2] in [1, 2, 3, 4]]
# file: test/sql/function/list/lambdas/reduce.test
# setup
CREATE TABLE t1 (a varchar[])
CREATE TABLE right_only (v varchar[], i int)
CREATE TABLE nested (n integer[][][])
CREATE table where_clause (a int[])
CREATE TABLE t_struct (s STRUCT(v VARCHAR, i INTEGER)[])
CREATE OR REPLACE TABLE df(s STRUCT(a INT, b INT)[])
# query
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
# reject
SELECT list_reduce([], lambda x, y, i: x + y + i)
SELECT list_reduce([1, 2, 3], lambda x, y: (x * y)::VARCHAR || 'please work')
SELECT list_reduce([1, 2], lambda x: x)
SELECT list_reduce([1, 2], NULL)
SELECt list_reduce([1, 2], (len('abc') AS x, y) - > x + y)
SELECT list_reduce(a, lambda x, y: x + y) FROM t1
SELECT list_reduce(a, lambda x, y: x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], lambda x, y: list_reduce([], lambda a, b: x + y + a + b))
# file: test/sql/function/list/lambdas/reduce_initial.test
# setup
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')
CREATE TABLE t1 (l varchar[], initial varchar)
CREATE TABLE right_only (v varchar[], i int)
CREATE TABLE nested (n integer[][][], initial integer[][])
CREATE TABLE t_struct (s STRUCT(v VARCHAR, i INTEGER)[], initial STRUCT(v VARCHAR, i INTEGER))
CREATE OR REPLACE TABLE df(s STRUCT(a INT, b INT)[], initial STRUCT(a INT, b INT))
CREATE table where_clause (a int[], initial integer)
CREATE TABLE cast_test (l int[], initial float)
# query
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
# reject
SELECT list_reduce([1, 2, 3], lambda x, y: (x * y), 'i dare you to cast me')
SELECT list_reduce([1, 2], lambda x: x, 100)
SELECT list_reduce([1, 2], NULL, 100)
SELECt list_reduce([1, 2], (len('abc') AS x, y) - > x + y, 100)
SELECT list_reduce(l, lambda x, y: x + y) FROM t1
SELECT list_reduce(l, lambda x, y: x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], lambda x, y: list_reduce([], lambda a, b: x + y + a + b), 1000)
SELECT list_reduce([1, 2, 3], lambda x, y, x_i: list_reduce([], lambda a, b, a_i: x + y + a + b + x_i + a_i), 1000)
# file: test/sql/function/list/lambdas/rhs_parameters.test
# setup
CREATE TABLE lists (i integer, v varchar[])
CREATE TABLE no_overwrite AS SELECT [range, range + 1] l FROM range(3)
# query
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
# file: test/sql/function/list/lambdas/storage.test
# setup
CREATE MACRO my_transform(list) AS list_transform(list, lambda x: x * x)
CREATE MACRO my_filter(list) AS list_filter(list, lambda x: x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, lambda x, y: x + y)
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, lambda elem: list_reduce( list_transform(elem, lambda x: x + 1), lambda x, y: x + y) > 42)
# query
CREATE MACRO my_transform(list) AS list_transform(list, lambda x: x * x)
CREATE MACRO my_filter(list) AS list_filter(list, lambda x: x > 42)
CREATE MACRO my_reduce(list) AS list_reduce(list, lambda x, y: x + y)
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, lambda elem: list_reduce( list_transform(elem, lambda x: x + 1), lambda x, y: x + y) > 42)
SELECT my_transform([1, 2, 3])
SELECT my_filter([41, 42, NULL, 43, 44])
SELECT my_reduce([1, 2, 3])
SELECT my_nested_lambdas([[40, NULL], [20, 21], [10, 10, 20]])
# file: test/sql/function/list/lambdas/table_functions.test
# setup
CREATE TABLE tmp AS SELECT range AS id FROM range(10)
CREATE TABLE cities AS SELECT * FROM (VALUES ('Amsterdam', [90, 10]), ('London', [89, 102])) cities (name, prices)
# query
CREATE TABLE tmp AS SELECT range AS id FROM range(10)
CREATE TABLE cities AS SELECT * FROM (VALUES ('Amsterdam', [90, 10]), ('London', [89, 102])) cities (name, prices)
ALTER TABLE cities ALTER COLUMN prices SET DATA TYPE INTEGER[] USING list_filter(cities.prices, lambda price: price < 100)
SELECT name, prices AS cheap_options FROM cities
# file: test/sql/function/list/lambdas/test_lambda_storage.test
# query
SELECT SUM(list_i[1]) FROM lambda_view
SELECT lambda_macro(1, 2)
# file: test/sql/function/list/lambdas/transform.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE transformed_lists (g integer, l integer[])
CREATE TABLE corr_test (n integer, l integer[], g integer)
create table test(a int, b int)
# query
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
# reject
SELECT list_transform([[1], [4], NULL, [1], [8]], lambda x: list_concat( list_transform(x, lambda y: CASE WHEN y > 1 THEN 'yay' ELSE 'nay' END), x))
# file: test/sql/function/list/lambdas/transform_with_index.test
# setup
CREATE TABLE tbl(a int[], b int, c int)
# query
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
# file: test/sql/function/list/lambdas/vector_types.test
# query
SELECT [x for x in c if x IS NOT NULL] FROM test_vector_types(NULL::INT[]) t(c)
SELECT [x for x in c if x IS NULL] FROM test_vector_types(NULL::INT[]) t(c)
SELECT list_reduce(c, lambda x, y: x + y) FROM test_vector_types(NULL::INT[]) t(c) WHERE len(c) > 0
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
# reject
SELECT list_transform([2], x -> (SELECT 1 - x) * x)
SELECT list_filter([2], x -> (SELECT 1 - x) * x > 2)
SELECT list_filter([[1, 2, 1], [1, 2, 3], [1, 1, 1]], x -> list_contains_macro(x, 3))
SELECT list_transform([1], x -> x = UNNEST([1]))
SELECT list_filter([1], x -> x = UNNEST([1]))
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
# reject
SELECT list_transform([['abc']], x -> list_filter(x, y -> y))
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
SELECT list_transform(qualified_tbl.x, x -> (qualified_tbl.x)[1] + 1 + x) FROM qualified_tbl
SELECT list_transform([1, 2], x -> list_transform([3, 4], x -> x))
SELECT list_has_all([variable_has_all FOR variable_has_all IN ['a']], ['b']) AS list_comp_result
SELECT list_has_all(list_transform(['a'], variable_has_all -> variable_has_all), ['b']) AS list_transform_result
SELECT list_has_any(['b'], list_transform(['a'], variable_has_any -> variable_has_any)) AS list_transform_result
SELECT list_intersect(list_intersect([1], [1]), [1])
SELECT list_intersect([1], list_intersect([1], [1]))
SELECT list_has_any(LIST_VALUE(list_has_any([1], [1])), [1])
# reject
SELECT list_apply(i, a.x -> a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
SELECT list_transform(qualified_tbl.x, qualified_tbl.x -> qualified_tbl.x + 1) FROM qualified_tbl
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
create table test as select range i from range(3)
CREATE MACRO macro_with_lambda(list, num) AS (list_transform(list, x -> x + num))
SELECT list_filter([[1, 2], NULL, [3], [4, NULL]], f -> list_count(macro_with_lambda(f, 2)) > 1)
CREATE MACRO some_macro(x, y, z) AS (SELECT list_transform(x, a -> x + y + z))
CREATE MACRO reduce_macro(list, num) AS (list_reduce(list, (x, y) -> x + y + num))
CREATE MACRO other_reduce_macro(list, num, bla) AS (SELECT list_reduce(list, (x, y) -> list + x + y + num + bla))
CREATE MACRO scoping_macro(x, y, z) AS (SELECT list_transform(x, x -> x + y + z))
CREATE OR REPLACE MACRO foo(bar) AS (SELECT apply([bar], x -> 0))
# reject
create macro my_macro(i) as (select i in (select i from test))
# file: test/sql/function/list/lambdas/arrow/list_comprehension_deprecated.test
# setup
CREATE TABLE fruit_tbl AS SELECT ['apple', 'banana', 'cherry', 'kiwi', 'mango'] fruits
CREATE TABLE word_tbl AS SELECT ['goodbye', 'cruel', 'world'] words
# query
SELECT list_transform(list_filter([0, 1, 2, 3, 4, 5], x -> x % 2 = 0), y -> y * y)
SELECT list_filter(list_filter([2, 4, 3, 1, 20, 10, 3, 30], x -> x % 2 == 0), y -> y % 5 == 0)
SELECT list_filter(['apple', 'banana', 'cherry', 'kiwi', 'mango'], fruit -> contains(fruit, 'a'))
SELECT list_transform([[1, NULL, 2], [3, NULL]], a -> list_filter(a, x -> x IS NOT NULL))
SELECT [len(x) for x in words] from word_tbl
with base as ( select [4,5,6] as l ) select [x for x,i in l if i != 2] as filtered from base
select [x+i for x, i in [10, 9, 8, 7, 6]]
with base as ( select [4,5,6] as l ) select [x + 5 for x in l] as filtered from base
# reject
select [a for a, b, c in [1,2,3]]
select [a for a[1], b[2] in [1,2,3,4]]
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
INSERT INTO t1 VALUES ([666])
INSERT INTO t1 VALUES (NULL)
# reject
SELECT list_reduce([], (x, y, i) -> x + y + i)
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y)::VARCHAR || 'please work')
SELECT list_reduce([1, 2], (x) -> x)
SELECT list_reduce(a, (x, y) -> x + y) FROM t1
SELECT list_reduce(a, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b))
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i))
SELECT list_reduce(n, (x, y) -> list_reduce(l, (a, b) -> x + y + a + b)) FROM nested
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
# reject
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y), 'i dare you to cast me')
SELECT list_reduce([1, 2], (x) -> x, 100)
SELECT list_reduce(l, (x, y) -> x + y) FROM t1
SELECT list_reduce(l, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b), 1000)
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i), 1000)
SELECT list_reduce(n, (x, y) -> list_reduce(l, (a, b) -> x + y + a + b), initial) FROM nested
SELECT list_reduce(n, (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (j, k) -> j + k)), initial) from nested
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
create table no_overwrite as select [range, range + 1] l from range(3)
select l, [[{'x+y': x + y, 'x': x, 'y': y, 'l': l} for y in [42, 43]] for x in l] from no_overwrite
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
CREATE MACRO my_nested_lambdas(nested_list) AS list_filter(nested_list, elem -> list_reduce(list_transform(elem, x -> x + 1), (x, y) -> x + y) > 42)
# file: test/sql/function/list/lambdas/arrow/table_functions_deprecated.test
# setup
CREATE TABLE tmp AS SELECT range AS id FROM range(10)
CREATE TABLE cities AS SELECT * FROM (VALUES ('Amsterdam', [90, 10]), ('London', [89, 102])) cities (name, prices)
# query
ALTER TABLE cities ALTER COLUMN prices SET DATA TYPE INTEGER[] USING list_filter(cities.prices, price -> price < 100)
# file: test/sql/function/list/lambdas/arrow/test_deprecated_lambda.test
# setup
CREATE TABLE varchars(v VARCHAR)
# query
CREATE TABLE varchars(v VARCHAR)
INSERT INTO varchars VALUES ('>>%Test<<'), ('%FUNCTION%'), ('Chaining')
DELETE FROM varchars
INSERT INTO varchars VALUES ('Test Function Chaining Alias')
# reject
SELECT v.split(' ') strings, strings.apply(x -> x.lower()).filter(x -> x[1] == 't') lower, strings.apply(x -> x.upper()).filter(x -> x[1] == 'T') upper, lower + upper AS mix_case_srings FROM varchars
# file: test/sql/function/list/lambdas/arrow/test_lambda_arrow_storage_deprecated.test
# setup
CREATE OR REPLACE FUNCTION deprecated_syntax.transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
# query
CREATE OR REPLACE FUNCTION deprecated_syntax.transpose(lst) AS ( SELECT list_transform(range(1, 1 + length(lst[1])), j -> list_transform(range(1, length(lst) + 1), i -> lst[i][j] ) ) )
DETACH deprecated_syntax
# file: test/sql/function/list/lambdas/arrow/transform_deprecated.test
# setup
CREATE TABLE lists (n integer, l integer[])
CREATE TABLE large_lists AS SELECT range % 4 g, list(range) l FROM range(10000) GROUP BY range % 4
CREATE TABLE transformed_lists (g integer, l integer[])
CREATE TABLE corr_test (n integer, l integer[], g integer)
create table test(a int, b int)
# query
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
# reject
SELECT list_transform([[1], [4], NULL, [1], [8]], x -> list_concat(list_transform(x, y -> CASE WHEN y > 1 THEN 'yay' ELSE 'nay' END), x))
# file: test/sql/function/list/lambdas/arrow/transform_with_index_deprecated.test
# setup
CREATE TABLE tbl(a int[], b int, c int)
# query
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
# file: test/sql/function/list/lambdas/arrow/warn_deprecated_arrow.test
# query
CALL enable_logging(level='error')
SELECT log_level, message[0:37] FROM duckdb_logs
CALL truncate_duckdb_logs()
CALL enable_logging(level='warning')
RESET lambda_syntax
SELECT (SELECT (JSON '{ "key" : "value" }')->k AS v FROM (SELECT 'key' AS k) keys)
# file: test/sql/function/list/aggregates/any_value.test
# setup
CREATE TABLE five_dates AS SELECT LIST(NULLIF(i,0)::integer) AS i, LIST('2021-08-20'::DATE + NULLIF(i,0)::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (NULLIF(i,0)) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (NULLIF(i,0)) MINUTE) AS t, LIST(INTERVAL (NULLIF(i,0)) SECOND) AS s FROM range(0, 6, 1) t1(i)
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (NULLIF(i,0)) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (NULLIF(i,0)) MINUTE)::TIMETZ) AS t, FROM range(0, 6, 1) t1(i)
CREATE TABLE five_complex AS SELECT LIST(NULLIF(i,0)::integer) AS i, LIST(NULLIF(i,0)::VARCHAR) AS s, LIST([NULLIF(i,0)]) AS l, LIST({'a': NULLIF(i,0)}) AS r FROM range(0, 6, 1) t1(i)
# query
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
# reject
SELECT list_any_value()
# file: test/sql/function/list/aggregates/approx_count_distinct.test
# setup
CREATE TABLE list_ints (l INTEGER[])
CREATE TABLE IF NOT EXISTS dates (t date[])
CREATE TABLE IF NOT EXISTS timestamp (t TIMESTAMP[])
CREATE TABLE IF NOT EXISTS names (t string[])
CREATE TABLE list_ints_2 (a INTEGER[], b INTEGER[])
# query
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
# file: test/sql/function/list/aggregates/avg.test
# setup
CREATE SEQUENCE seq
CREATE TABLE integers(i INTEGER[])
CREATE TABLE vals(i INTEGER[], j HUGEINT[])
CREATE TABLE bigints(n HUGEINT[])
CREATE TABLE doubles(n DOUBLE[])
# query
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
# reject
SELECT list_avg()
# file: test/sql/function/list/aggregates/bigints_sum_avg.test
# setup
CREATE TABLE bigints (i BIGINT[])
CREATE TABLE decimals (i DECIMAL(18,1)[])
# query
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
# file: test/sql/function/list/aggregates/bit_and.test
# setup
CREATE SEQUENCE seq
CREATE TABLE integers(i INTEGER[])
# query
SELECT list_bit_and([nextval('seq')])
INSERT INTO integers VALUES ([3, 7, 15, 31, 3, 15])
SELECT list_bit_and([]) FROM integers
INSERT INTO integers VALUES ([]), (NULL), ([NULL]), ([3, 7, NULL, 15, 31, 3, 15, NULL])
SELECT list_bit_and(i), list_bit_and([1, 1, 1, 1, 1, 1]), list_bit_and(NULL) FROM integers
# reject
SELECT list_bit_and()
# file: test/sql/function/list/aggregates/bit_or.test
# setup
CREATE SEQUENCE seq
CREATE TABLE integers(i INTEGER[])
# query
SELECT list_bit_or([nextval('seq')])
SELECT list_bit_or([]) FROM integers
SELECT list_bit_or(i), list_bit_or([1, 1, 1, 1, 1, 1]), list_bit_or(NULL) FROM integers
# reject
SELECT list_bit_or()
# file: test/sql/function/list/aggregates/bit_xor.test
# setup
CREATE SEQUENCE seq
CREATE TABLE integers (i INTEGER[])
# query
SELECT list_bit_xor([nextval('seq')])
CREATE TABLE integers (i INTEGER[])
SELECT list_bit_xor([]) FROM integers
SELECT list_bit_xor(i), list_bit_xor([1, 1, 1, 1, 1, 1]), list_bit_xor(NULL) FROM integers
# reject
SELECT list_bit_xor()
# file: test/sql/function/list/aggregates/bool_and_or.test
# setup
CREATE TABLE bools (l BOOLEAN[])
# query
CREATE TABLE bools (l BOOLEAN[])
INSERT INTO bools SELECT LIST(True) FROM range(100) tbl(i)
INSERT INTO bools SELECT LIST(False) FROM range(100) tbl(i)
INSERT INTO bools VALUES ([True, False])
INSERT INTO bools VALUES ([]), ([NULL]), (NULL), ([NULL, True, False, NULL])
SELECT list_bool_or(l) FROM bools
SELECT list_bool_and(l) FROM bools
# reject
select list_bool_or()
select list_bool_and()
# file: test/sql/function/list/aggregates/count.test
# setup
CREATE TABLE lists (l INTEGER[])
# query
SELECT list_count([1, 2, 3])
SELECT list_count([1]) FROM range(3)
CREATE TABLE lists (l INTEGER[])
INSERT INTO lists VALUES ([1, 2]), ([NULL]), (NULL), ([]), ([3, 4, 5, 6, 7]), ([1, 2, NULL, 1, NULL])
SELECT list_count(l) FROM lists
# reject
select list_count()
# file: test/sql/function/list/aggregates/entropy.test
# setup
create table aggr(k int[])
CREATE TABLE entr (l INTEGER[])
create table aggr2 (k int[])
create table names (name string[])
# query
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
# reject
select list_entropy()
# file: test/sql/function/list/aggregates/first.test
# setup
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
# query
SELECT list_aggr([1, 2], 'arbitrary')
SELECT list_first(i) FROM five
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
SELECT list_first(d), list_first(dt), list_first(t), list_first(s) FROM five_dates
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
SELECT list_first(dt), list_first(t) FROM five_dates_tz
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
SELECT list_first(s), list_first(l), list_first(r) FROM five_complex
DROP TABLE five_complex
# reject
SELECT list_first()
# file: test/sql/function/list/aggregates/histogram.test
# setup
CREATE TABLE const AS SELECT LIST(2) AS i FROM range(200) t1(i)
CREATE TABLE hist_data (g INTEGER[])
create table names (name string[])
# query
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
# reject
select list_histogram()
# file: test/sql/function/list/aggregates/histogram_decimal.test
# query
WITH cte AS (FROM (VALUES (0.0), (9.9)) df(l_orderkey)) SELECT * FROM histogram_values(cte, l_orderkey)
# file: test/sql/function/list/aggregates/hugeint.test
# setup
CREATE TABLE hugeints(h HUGEINT[])
# query
CREATE TABLE hugeints(h HUGEINT[])
INSERT INTO hugeints VALUES ([NULL, 1, 2]), (NULL), ([]), ([NULL]), ([1, 2, 3])
SELECT list_first(h), list_last(h), list_sum(h) FROM hugeints
DELETE FROM hugeints
INSERT INTO hugeints VALUES ([42.0, 1267650600228229401496703205376, -439847238974238975238975, '-12'])
SELECT list_min(h), list_max(h), list_sum(h), list_first(h), list_last(h) FROM hugeints
# file: test/sql/function/list/aggregates/kurtosis.test
# setup
create table aggr(k int[])
# query
select list_kurtosis([1])
select list_kurtosis([0, 0, 0, 0, 0, 0])
insert into aggr values ([1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]), ([10, 10, 10, 10, 20, 20, 25, 30, 30, 30, 30]), ([NULL, 11, 15, 18, 22, 25, NULL, 35, 40, 50, 51]), (NULL), ([]), ([NULL])
select list_kurtosis(k) from aggr
select list_kurtosis_pop(k) from aggr
# reject
select list_kurtosis([2e304, 2e305, 2e306, 2e307])
select list_kurtosis()
# file: test/sql/function/list/aggregates/last.test
# setup
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
# query
INSERT INTO five VALUES (NULL), ([NULL]), ([]), ([1, 2, NULL])
SELECT list_last(i) FROM five
SELECT list_last(d), list_last(dt), list_last(t), list_last(s) FROM five_dates
SELECT list_last(dt), list_last(t) FROM five_dates_tz
SELECT list_last(s), list_last(l), list_last(r) FROM five_complex
# reject
SELECT list_last()
# file: test/sql/function/list/aggregates/mad.test
# setup
CREATE TABLE const AS SELECT LIST(1) AS i FROM range(2000) t1(i)
create table date as select list(('2018-01-01'::DATE + INTERVAL (r) DAY)::DATE) as r from range(10000) tbl(r)
create table hour as select list('2018-01-01'::TIMESTAMP + INTERVAL (r) HOUR) as r from range(10000) tbl(r)
create table second as select list('00:00:00'::TIME + INTERVAL (r) SECOND) as r from range(10000) tbl(r)
# query
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
# reject
SELECT list_mad([INTERVAL 1 YEAR])
SELECT list_mad([NULL::INTERVAL])
# file: test/sql/function/list/aggregates/max.test
# setup
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
# query
SELECT list_max(i) FROM five
SELECT list_max(d), list_max(dt), list_max(t), list_max(s) FROM five_dates
SELECT list_max(dt), list_max(t) FROM five_dates_tz
SELECT list_max(s), list_max(l), list_max(r) FROM five_complex
# reject
SELECT list_max()
# file: test/sql/function/list/aggregates/median.test
# setup
CREATE TABLE quantile AS SELECT LIST(r::tinyint) AS r FROM range(100) t1(r)
CREATE TABLE range AS SELECT LIST(1) AS i FROM range(2000) t1(i)
# query
SELECT list_median(r) FROM quantile
DROP TABLE quantile
CREATE TABLE quantile AS SELECT LIST(r::tinyint) AS r FROM range(100) t1(r)
CREATE TABLE range AS SELECT LIST(1) AS i FROM range(2000) t1(i)
INSERT INTO range VALUES (NULL), ([]), ([NULL])
SELECT list_median(i) FROM range
# file: test/sql/function/list/aggregates/min.test
# setup
CREATE TABLE five_dates AS SELECT LIST(i::integer) AS i, LIST('2021-08-20'::DATE + i::INTEGER) AS d, LIST('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR) AS dt, LIST('14:59:37'::TIME + INTERVAL (i) MINUTE) AS t, LIST(INTERVAL (i) SECOND) AS s FROM range(1, 6, 1) t1(i)
CREATE TABLE five_dates_tz AS SELECT LIST(('2021-08-20'::TIMESTAMP + INTERVAL (i) HOUR)::TIMESTAMPTZ) AS dt, LIST(('14:59:37'::TIME + INTERVAL (i) MINUTE)::TIMETZ) AS t, FROM range(1, 6, 1) t1(i)
CREATE TABLE five_complex AS SELECT LIST(i::integer) AS i, LIST(i::VARCHAR) AS s, LIST([i]) AS l, LIST({'a': i}) AS r FROM range(1, 6, 1) t1(i)
# query
SELECT list_min(i) FROM five
SELECT list_min(d), list_min(dt), list_min(t), list_min(s) FROM five_dates
SELECT list_min(dt), list_min(t) FROM five_dates_tz
SELECT list_min(s), list_min(l), list_min(r) FROM five_complex
# reject
SELECT list_min()
# file: test/sql/function/list/aggregates/minmax_nested.test
# setup
CREATE TABLE structs AS SELECT {'i': i} s FROM range(1000) t(i)
CREATE TABLE varchar_structs AS SELECT {'i': concat('long_prefix_', i)} s FROM range(1000) t(i)
CREATE TABLE multi_member_struct AS SELECT {'i': (1000-i)//5, 'j': i} s FROM range(1000) t(i)
CREATE TABLE lists AS SELECT case when i<500 then [i, i + 1, i + 2] else [i, 0] end AS l FROM range(1000) t(i)
CREATE TABLE list_with_structs AS SELECT case when i<500 then [{'i': i}, {'i': i + 1}, {'i': i + 2}] else [{'i': i}, {'i': 0}] end AS l FROM range(1000) t(i)
CREATE TABLE list_multi_member_struct AS SELECT [NULL, {'i': (1000-i)//5, 'j': i}, NULL] l FROM range(1000) t(i)
CREATE TABLE struct_with_lists AS SELECT {'i': case when i<500 then [i, i + 1, i + 2] else [i, 0] end} AS s FROM range(1000) t(i)
CREATE TABLE arrays AS SELECT (case when i<500 then [i, i + 1, i + 2] else [i, 0, 0] end)::BIGINT[3] AS l FROM range(1000) t(i)
CREATE TABLE float_values(f FLOAT)
CREATE TABLE double_values(d DOUBLE)
# query
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
# file: test/sql/function/list/aggregates/mode.test
# setup
CREATE TABLE range AS SELECT LIST(2) AS i FROM range(100) t1(i)
create table names (name string[])
create table dates (v date[])
create table times (v time[])
create table timestamps (v timestamp[])
create table intervals (v interval[])
create table hugeints (v hugeint[])
create table aggr (v decimal(10,2)[])
# query
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
# reject
select list_mode()
# file: test/sql/function/list/aggregates/nested.test
# setup
CREATE TABLE lists (l1 INTEGER[], l2 INTEGER[])
# query
SELECT list_min(list_concat([1, 2], [-1]))
SELECT list_min(list_aggr([1, 2], 'list'))
CREATE TABLE lists (l1 INTEGER[], l2 INTEGER[])
INSERT INTO lists VALUES ([1, 2, 3], [4]), ([NULL, 1, -4, NULL], [NULL]), (NULL, NULL), ([NULL], [-4]), ([], [])
SELECT list_last(list_concat(l1, l2)) FROM lists
SELECT list_concat(list(list_last(l1)), list(list_first(l2))) FROM lists
SELECT array_aggregate([1, 2], 'min')
SELECT array_aggr([1, 2], 'min')
SELECT list_aggregate([1, 2], 'min')
# file: test/sql/function/list/aggregates/product.test
# setup
CREATE TABLE integers(i INTEGER[])
CREATE TABLE prods AS SELECT LIST(2) AS i FROM range(100 // 2) t1(i)
# query
INSERT INTO integers VALUES ([1, 2, 4]), (NULL), ([]), ([NULL]), ([1, 2, NULL, 4, NULL])
SELECT list_product(i) FROM integers
CREATE TABLE prods AS SELECT LIST(2) AS i FROM range(100) t1(i)
SELECT list_product(i) FROM prods
drop table prods
CREATE TABLE prods AS SELECT LIST(2) AS i FROM range(100 // 2) t1(i)
# reject
select list_product()
# file: test/sql/function/list/aggregates/sem.test
# setup
create table aggr(k int[], v decimal(10,2)[], v2 decimal(10, 2)[])
create table sems (l int[])
# query
select list_sem([1])
create table aggr(k int[], v decimal(10,2)[], v2 decimal(10, 2)[])
insert into aggr values ([1, 2, 2, 2, 2], [10, 10, 20, 25, 30], [NULL, 11, 22, NULL, 35])
select list_sem(k), list_sem(v), list_sem(v2) from aggr
create table sems (l int[])
insert into sems values ([1, 2, 2, 2, 2]), ([1, 2, NULL, 2, 2, NULL, 2]), ([]), ([NULL]), (NULL)
select list_sem(l) from sems
# reject
select list_sem()
# file: test/sql/function/list/aggregates/skewness.test
# setup
CREATE TABLE skew AS SELECT LIST(10) AS i FROM range(5) t1(i)
create table aggr(k int[], v decimal(10,2)[], v2 decimal(10, 2)[])
create table aggr2(v2 decimal(10, 2)[])
# query
select list_skewness([1])
CREATE TABLE skew AS SELECT LIST(10) AS i FROM range(5) t1(i)
select list_skewness (i) from skew
select list_skewness ([1,2])
insert into aggr values ([1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2], [10, 10, 10, 10, 20, 20, 25, 30, 30, 30, 30], [NULL, 11, 15, 18, 22, 25, NULL, 35, 40, 50, 51]), ([], NULL, [NULL])
select list_skewness(k), list_skewness(v), list_skewness(v2) from aggr
create table aggr2(v2 decimal(10, 2)[])
insert into aggr2 values ([NULL, 11, 15, 18]), ([22, 25]), ([NULL]), ([35, 40, 50, 51])
select list_skewness(v2) from aggr2
# reject
select list_skewness()
select list_skewness([-2e307, 0, 2e307])
# file: test/sql/function/list/aggregates/string_agg.test
# setup
CREATE TABLE str_aggs (str varchar[])
CREATE TABLE strings(g INTEGER[], x VARCHAR[], y VARCHAR[])
CREATE TABLE long AS SELECT LIST('a') g FROM range(0, 10, 1) t1(c), range(0, 10, 1) t2(e)
# query
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
# reject
SELECT list_string_agg()
# file: test/sql/function/list/aggregates/sum.test
# setup
CREATE TABLE integers(i INTEGER[])
CREATE TABLE doubles(n DOUBLE[])
CREATE TABLE bigints(i BIGINT[])
# query
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
# file: test/sql/function/list/aggregates/sum_bool.test
# setup
CREATE TABLE integers(i INTEGER)
# query
INSERT INTO integers SELECT CASE WHEN i%3=0 THEN NULL ELSE i END i FROM range(10000) t(i)
SELECT SUM(i > 500), SUM(i=1), SUM(i IS NULL) FROM integers
SELECT COUNTIF(i > 500), COUNT_IF(i=1), COUNTIF(i IS NULL) FROM integers
# file: test/sql/function/list/aggregates/uhugeint.test
# setup
CREATE TABLE uhugeints(h UHUGEINT[])
# query
CREATE TABLE uhugeints(h UHUGEINT[])
INSERT INTO uhugeints VALUES ([NULL, 1, 2]), (NULL), ([]), ([NULL]), ([1, 2, 3])
SELECT list_first(h), list_last(h), list_sum(h) FROM uhugeints
DELETE FROM uhugeints
INSERT INTO uhugeints VALUES ([42.0, 1267650600228229401496703205376, 0, '1'])
SELECT list_min(h), list_max(h), list_sum(h), list_first(h), list_last(h) FROM uhugeints
# file: test/sql/function/list/aggregates/var_stddev.test
# setup
create table stddev_test(val integer[])
CREATE TABLE t0 (c0 DOUBLE[])
# query
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
# reject
select list_aggr([1e301, -1e301], 'stddev')
select list_var_samp([1e301, -1e301])
select list_var_pop([1e301, -1e301])
SELECT list_stddev_samp()
SELECT list_stddev_pop(c0) FROM t0
# file: test/sql/function/variant/variant_extract.test
# setup
CREATE MACRO struct_cast_data() AS TABLE ( SELECT {'a': [ { 'b': 'hello', 'c': NULL, 'a': '1970/03/15'::DATE }, { 'b': NULL, 'c': True, 'a': '2020/11/03'::DATE } ]}::VARIANT AS a UNION ALL SELECT {'a': [ { 'b': 'this is a long string', 'c': False, 'a': '1953/9/16'::DATE } ]}::VARIANT )
# query
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
# file: test/sql/function/variant/variant_extract_try_cast.test
# setup
create table tbl(col VARIANT)
# query
insert into tbl SELECT * FROM UNNEST([ {'almost_a_number': c, 'a_number': CAST(TRY_CAST(c AS INT) AS VARCHAR)} for c in ['12', '24', '25a6', '24c', '16'] ])
from tbl order by all
select col.almost_a_number from tbl order by all
select TRY_CAST(col.almost_a_number AS BIGINT) from tbl order by all
select col.a_number from tbl order by all
select col.a_number::BIGINT from tbl order by all
set explain_output='optimized_only'
EXPLAIN select TRY_CAST(col.almost_a_number AS BIGINT) from tbl order by all
# reject
select col.almost_a_number::BIGINT from tbl order by all
# file: test/sql/function/variant/variant_shredded_extract_nested.test
# setup
create table list_variant as select { 'id': l_orderkey, 'my_list': [ l_orderkey, l_orderkey + 1, l_orderkey + 2 ], 'my_struct': { 'a': l_orderkey, 'b': l_orderkey + 30 } }::variant as list_variant from range(5) t(l_orderkey)
# query
create table list_variant as select { 'id': l_orderkey, 'my_list': [ l_orderkey, l_orderkey + 1, l_orderkey + 2 ], 'my_struct': { 'a': l_orderkey, 'b': l_orderkey + 30 } }::variant as list_variant from range(5) t(l_orderkey)
select list_variant.my_list from list_variant limit 1
select list_variant.my_list::BIGINT[] from list_variant limit 1
select list_variant.my_struct from list_variant limit 1
select list_variant.my_struct::STRUCT(b BIGINT, a BIGINT) from list_variant limit 1
select (list_variant.my_struct::STRUCT(b VARCHAR, a VARCHAR)).b[1] from list_variant limit 1
# file: test/sql/function/variant/variant_typeof.test
# setup
CREATE TABLE T (v VARIANT)
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
# query
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
# file: test/sql/function/generic/can_cast_implicitly.test
# setup
CREATE TABLE tbl AS SELECT * FROM range(10) tbl(i)
# query
CREATE TABLE tbl AS SELECT * FROM range(10) tbl(i)
SELECT can_cast_implicitly(i, NULL::BIGINT) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::HUGEINT) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::INTEGER) FROM tbl LIMIT 1
SELECT can_cast_implicitly(i, NULL::VARCHAR) FROM tbl LIMIT 1
# file: test/sql/function/generic/case_condition.test
# setup
CREATE TABLE tbl AS SELECT * FROM range(10) tbl(i)
# query
SELECT * FROM tbl WHERE CASE WHEN i%2=0 THEN 1 ELSE 0 END AND CASE WHEN i<5 THEN 1 ELSE 0 END
# file: test/sql/function/generic/case_short_circuit.test
# setup
create table t (n text)
# query
create table t (n text)
insert into t values ('1'),('0'),('')
select n, case when n <> '' and cast(substr(n, 1, 1) as int) <= 0 then '0' when n <> '' and cast(substr(n, 1, 1) as int) > 0 then '1' else '2'end as x from t ORDER BY n
# file: test/sql/function/generic/case_varchar.test
# setup
CREATE TABLE tbl AS SELECT i, 'thisisalongstring' || i::VARCHAR s FROM range(10) tbl(i)
# query
CREATE TABLE tbl AS SELECT i, 'thisisalongstring' || i::VARCHAR s FROM range(10) tbl(i)
SELECT i, s, CASE WHEN i%2=0 THEN s ELSE s END FROM tbl
SELECT i, s, CASE WHEN i%2=0 THEN s ELSE s END FROM (SELECT i, s||'_suffix' FROM tbl) tbl(i, s)
# file: test/sql/function/generic/cast_to_type.test
# setup
create table tbl(i int, v varchar)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN cast_to_type(nullif(trim(s::VARCHAR), ''), s) ELSE s END
# query
SELECT cast_to_type(' 42', NULL::INT)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN cast_to_type(nullif(trim(s::VARCHAR), ''), s) ELSE s END
SELECT try_trim_null(42) as trim_int, try_trim_null(' col ') as trim_varchar, try_trim_null('') as trim_empty
create table tbl(i int, v varchar)
insert into tbl values (42, ' hello '), (100, ' ')
SELECT try_trim_null(COLUMNS(*)) FROM tbl
PREPARE v1 AS SELECT cast_to_type(' 42', ?)
EXECUTE v1(NULL::INT)
EXECUTE v1(NULL::VARCHAR)
# reject
SELECT cast_to_type('hello', NULL::INT)
SELECT cast_to_type(42, NULL)
# file: test/sql/function/generic/constant_or_null.test
# query
SELECT constant_or_null(1, NULL), constant_or_null(1, 10)
SELECT constant_or_null(1, case when i%2=0 then null else i end) from range(5) tbl(i)
SELECT constant_or_null(1, case when i%2=0 then null else i end, case when i%2=1 then null else i end) from range(5) tbl(i)
# reject
SELECT constant_or_null(1)
SELECT constant_or_null()
# file: test/sql/function/generic/error.test
# query
SELECT * FROM (SELECT 4 AS x) WHERE IF(x % 2 = 0, true, ERROR(FORMAT('x must be even number but is {}', x)))
# reject
SELECT error('test')
SELECT CASE WHEN value = 'foo' THEN 'Value is foo.' ELSE ERROR(CONCAT('Found unexpected value: ', value)) END AS new_value FROM ( SELECT 'foo' AS value UNION ALL SELECT 'baz' AS value)
SELECT * FROM (SELECT 3 AS x) WHERE IF(x % 2 = 0, true, ERROR(FORMAT('x must be even but is {}', x)))
SELECT 42=error('hello world')
SELECT error('hello world') IS NULL
# file: test/sql/function/generic/hash_func.test
# setup
CREATE TYPE resistor AS ENUM ( 'black', 'brown', 'red', 'orange', 'yellow', 'green', 'blue', 'violet', 'grey', 'white' )
CREATE TABLE structs AS SELECT * FROM (VALUES ({'i': 5, 's': 'string'}), ({'i': -2, 's': NULL}), ({'i': NULL, 's': 'not null'}), ({'i': NULL, 's': NULL}), (NULL) ) tbl(s)
CREATE TABLE lists AS SELECT * FROM (VALUES ([1], ['TGTA']), ([1, 2], ['CGGT']), ([], ['CCTC']), ([1, 2, 3], ['TCTA']), ([1, 2, 3, 4, 5], ['AGGG']), (NULL, NULL) ) tbl(li, lg)
CREATE TABLE maps AS SELECT * FROM (VALUES (MAP([1], ['TGTA'])), (MAP([1, 2], ['CGGT', 'CCTC'])), (MAP([], [])), (MAP([1, 2, 3], ['TCTA', NULL, 'CGGT'])), (MAP([1, 2, 3, 4, 5], ['TGTA', 'CGGT', 'CCTC', 'TCTA', 'AGGG'])), (NULL) ) tbl(m)
CREATE TABLE map_as_list AS SELECT * FROM (VALUES ([{'key':1, 'value':'TGTA'}]), ([{'key':1, 'value':'CGGT'}, {'key':2, 'value':'CCTC'}]), ([]), ([{'key':1, 'value':'TCTA'}, {'key':2, 'value':NULL}, {'key':3, 'value':'CGGT'}]), ([{'key':1, 'value':'TGTA'}, {'key':2, 'value':'CGGT'}, {'key':3, 'value':'CCTC'}, {'key':4, 'value':'TCTA'}, {'key':5, 'value':'AGGG'}]), (NULL) ) tbl(m)
CREATE TABLE enums (r resistor)
CREATE TABLE issue2498 AS SELECT * FROM (VALUES (24, {'x': [{'l4': [52, 53]}, {'l4': [54, 55]}]}), (34, {'x': [{'l4': [52, 53]}, {'l4': [54, 55]}]}) ) tbl(v, k)
# query
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
# reject
SELECT HASH()
SELECT r, HASH() FROM enums
# file: test/sql/function/generic/least_greatest_enum.test
# setup
CREATE TYPE t AS ENUM ('z','y','x')
# query
CREATE TYPE t AS ENUM ('z','y','x')
SELECT greatest('x'::t, 'z'::t), 'x'::t > 'z'::t
# file: test/sql/function/generic/least_greatest_types.test
# setup
CREATE TABLE all_types AS FROM test_all_types()
# query
CREATE TABLE all_types AS FROM test_all_types()
# file: test/sql/function/generic/replace_type.test
# setup
create table tbl(i int, v varchar)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN replace_type(nullif(trim(s::VARCHAR), ''), NULL::VARCHAR, s) ELSE s END
# query
SELECT replace_type(' 42', NULL::VARCHAR, NULL::INT)
CREATE OR REPLACE MACRO try_trim_null(s) AS CASE WHEN typeof(s)=='VARCHAR' THEN replace_type(nullif(trim(s::VARCHAR), ''), NULL::VARCHAR, s) ELSE s END
PREPARE v1 AS SELECT replace_type(' 42', NULL::VARCHAR, ?)
select replace_type({duck: 3.141592653589793::DOUBLE, goose: 2.718281828459045::DOUBLE}, NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type(map {'duck': 3.141592653589793::DOUBLE, 'goose': 2.718281828459045::DOUBLE}, NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type([3.141592653589793, 2.718281828459045]::DOUBLE[], NULL::DOUBLE, NULL::DECIMAL(15,2))
select replace_type([3.141592653589793, 2.718281828459045]::DOUBLE[2], NULL::DOUBLE, NULL::DECIMAL(15,2))
# reject
SELECT replace_type('hello', NULL::VARCHAR, NULL::INT)
SELECT replace_type(42, NULL::INTEGER, NULL)
# file: test/sql/function/generic/table_func_varargs.test
# query
SELECT * FROM repeat_row(1, 2, 'foo', num_rows=3)
# file: test/sql/function/generic/test_approx_database_count.test
# query
SELECT approx_count >= 3 FROM duckdb_approx_database_count()
# file: test/sql/function/generic/test_between.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE issue3588(c0 INT)
# query
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
# file: test/sql/function/generic/test_between_sideeffects.test
# query
PREPARE v1 AS SELECT ? BETWEEN 1 AND 2
EXECUTE v1(1)
EXECUTE v1(3)
PREPARE v2 AS SELECT 1 WHERE ? BETWEEN now() - INTERVAL '1 minute' AND now() + INTERVAL '1 minute'
EXECUTE v2(now())
EXECUTE v2(now() - INTERVAL '10 minute')
SELECT (RANDOM() * 10)::INT BETWEEN 6 AND 5
SELECT (RANDOM() * 10)::INT NOT BETWEEN 6 AND 5
# reject
EXECUTE v1(1, 2)
# file: test/sql/function/generic/test_boolean_test.test
# query
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
# file: test/sql/function/generic/test_connection_count.test
# query
SELECT count FROM duckdb_connection_count()
# file: test/sql/function/generic/test_if.test
# query
SELECT IF(true, 1, 10), IF(false, 1, 10), IF(NULL, 1, 10)
SELECT IF(true, 20, 2000), IF(false, 20, 2000), IF(NULL, 20, 2000)
SELECT IF(true, 20.5, 2000), IF(false, 20, 2000.5), IF(NULL, 20, 2000.5)
SELECT IF(true, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp), IF(false, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp), IF(NULL, '2020-05-05'::date, '1996-11-05 10:11:56'::timestamp)
SELECT IF(true, 'true', 'false'), IF(false, 'true', 'false'), IF(NULL, 'true', 'false')
# file: test/sql/function/generic/test_if_null.test
# query
SELECT IFNULL(NULL, NULL), IFNULL(NULL, 10), IFNULL(1, 10)
SELECT IFNULL(NULL, 2000), IFNULL(20.5, 2000)
SELECT IFNULL(NULL, '1996-11-05 10:11:56'::timestamp), IFNULL('2020-05-05'::date, '1996-11-05 10:11:56'::timestamp)
SELECT IFNULL(NULL, 'not NULL'), IFNULL('NULL', 'not NULL')
# file: test/sql/function/generic/test_in.test
# setup
CREATE TABLE integers(i INTEGER)
# query
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
# file: test/sql/function/generic/test_least_greatest.test
# setup
CREATE TABLE t1(i INTEGER, j INTEGER)
# query
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
# reject
SELECT LEAST(DATE '1992-01-01', 'hello', 123)
# file: test/sql/function/generic/test_null_if.test
# setup
CREATE TABLE test (a STRING)
CREATE TABLE test2 (a STRING, b STRING)
CREATE TABLE test3 (a INTEGER, b INTEGER)
# query
SELECT NULLIF(NULLIF ('hello', 'world'), 'blabla')
CREATE TABLE test (a STRING)
INSERT INTO test VALUES ('hello'), ('world'), ('test')
CREATE TABLE test2 (a STRING, b STRING)
INSERT INTO test2 VALUES ('blabla', 'b'), ('blabla2', 'c'), ('blabla3', 'd')
SELECT NULLIF(NULLIF ((SELECT a FROM test LIMIT 1 offset 1), a), b) FROM test2
INSERT INTO test3 VALUES (11, 22), (13, 22), (12, 21)
SELECT NULLIF(CAST(a AS VARCHAR), '11') FROM test3
SELECT a, CASE WHEN a>11 THEN CAST(a AS VARCHAR) ELSE CAST(b AS VARCHAR) END FROM test3 ORDER BY 1
# file: test/sql/function/generic/test_set.test
# query
SELECT CURRENT_SETTING('default_null_order')
SET default_null_order = 'nulls_last'
SET default_null_order = concat('nulls', '_', 'last')
SELECT CURRENT_SETTING('DEFAULT_NULL_ORDER')
SELECT * FROM range(3) UNION ALL SELECT NULL ORDER BY 1
# reject
SELECT CURRENT_SETTING('a')
SELECT CURRENT_SETTING('memori_limit')
SELECT CURRENT_SETTING(i::VARCHAR) FROM range(1) tbl(i)
SELECT CURRENT_SETTING(NULL)
SELECT CURRENT_SETTING(CAST(NULL AS TEXT))
SELECT CURRENT_SETTING('')
SET default_null_order = colref || '_last'
SET default_null_order = (SELECT 'nulls_last')
# file: test/sql/function/generic/test_sleep.test
# setup
CREATE TABLE test_sleep_table AS SELECT * FROM range(5) tbl(id)
# query
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
# file: test/sql/function/generic/test_stats.test
# setup
CREATE TABLE integers(i INTEGER)
# query
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
# file: test/sql/function/generic/test_table_param.test
# setup
create table a (i double, j double)
# query
create table a (i double, j double)
insert into a values (1, 10), (42, 420)
EXPLAIN SELECT * FROM summary((SELECT * FROM a))
SELECT * FROM summary((SELECT * FROM a))
# file: test/sql/function/string/format_bytes.test
# query
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
# file: test/sql/function/string/hex.test
# query
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
# reject
SELECT from_hex('duckdb')
# file: test/sql/function/string/like_unicode.test
# setup
CREATE TABLE t0 (c0 VARCHAR)
# query
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
# file: test/sql/function/string/md5.test
# setup
CREATE TABLE strings AS SELECT s::VARCHAR s FROM generate_series(0,10,1) t(s)
# query
select md5('hello'), md5(NULL)
select md5_number('hello'), md5_number_upper(NULL)
select md5_number_upper('hello'), md5_number_upper(NULL)
select md5_number_lower('hello'), md5_number_lower(NULL)
CREATE TABLE strings AS SELECT s::VARCHAR s FROM generate_series(0,10,1) t(s)
select md5(s), md5('1') from strings ORDER BY s
select md5(s), md5('1') from strings where s::INTEGER BETWEEN 1 AND 3 ORDER BY s
# file: test/sql/function/string/null_byte.test
# setup
CREATE TABLE more_null_bytes AS SELECT 1 AS id, v FROM null_byte UNION ALL SELECT 2 AS id, substr(v, 4, 1) FROM null_byte UNION ALL SELECT 3 AS id, v FROM null_byte
# query
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
# file: test/sql/function/string/parse_formatted_bytes.test
# query
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
# reject
SELECT parse_formatted_bytes('null')
SELECT parse_formatted_bytes('none')
SELECT parse_formatted_bytes('5')
SELECT parse_formatted_bytes('abc')
SELECT parse_formatted_bytes('1 Ki')
SELECT parse_formatted_bytes(1933)
SELECT parse_formatted_bytes('10000000000 TiB')
SELECT parse_formatted_bytes('1.5.3 GB')
# file: test/sql/function/string/parse_path.test
# query
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
# reject
SELECT parse_path()
SELECT parse_path('/path/to', true, 'system')
SELECT parse_dirname()
SELECT parse_dirname('/path/to', true, 'system')
SELECT parse_dirpath()
SELECT parse_dirpath('/path/to', true, 'system')
SELECT parse_filename(true)
SELECT parse_filename('path/to/file.csv', 'system', true)
# file: test/sql/function/string/parse_path_windows.test
# query
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
# reject
SELECT parse_filename()
# file: test/sql/function/string/regex_capture.test
# setup
CREATE TABLE filenames (filename VARCHAR)
# query
CREATE TABLE filenames (filename VARCHAR)
INSERT INTO filenames VALUES ('rundate_2023-01-01_pass_1'), ('rundate_2023-01-01_pass_2'), ('rundate_2023-01-01_pass_3'), ('rundate_2023-01-10_pass_1'), ('rundate_2023-01-10_pass_2'), ('rundate_2023-02-14_pass_1'), ('invalid'), (NULL)
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.pass::SMALLINT AS PASS, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(1000) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.pass::SMALLINT AS PASS, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_([a-z]+?)_(\d+)', ['rundate', 'opt', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.opt AS opt, groups.pass::SMALLINT AS pass, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
WITH files AS ( SELECT f.*, payload FROM filenames f, range(3) t(payload) ), extracted AS ( SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_([0-9]+?)_(\d+)', ['rundate', 'opt', 'pass']) AS groups, payload FROM files ) SELECT groups.rundate::DATE AS rundate, groups.opt AS opt, groups.pass::SMALLINT AS pass, SUM(payload) FROM extracted WHERE LENGTH(groups.rundate) > 0 GROUP BY ALL
# reject
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', []) AS groups FROM filenames
WITH patterns AS ( SELECT 'rundate_(\d+-\d+-\d+)_pass_(\d+)' AS pattern FROM range(3) ) SELECT regexp_extract(filename, pattern, ['rundate', 'pass']) AS groups FROM filenames, patterns
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', NULL]) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'rundate']) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'RUNDATE']) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass', 'overflow']) AS groups FROM filenames
SELECT regexp_extract(filename, NULL, ['rundate', 'pass']) AS groups FROM filenames
# file: test/sql/function/string/regex_escape.test
# setup
CREATE TABLE tbl (c VARCHAR(255))
# query
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
# file: test/sql/function/string/regex_extract.test
# setup
CREATE TABLE test (s VARCHAR, p VARCHAR, i INT)
# query
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
# reject
SELECT regexp_extract('foobarbaz', '(b..)(b..)', -1)
SELECT regexp_extract('foobarbaz', '(b..)(b..)', 42)
SELECT regexp_extract(s, p, i) FROM test
SELECT regexp_extract(s, '(b..)(b..)', i) FROM test
SELECT regexp_extract('foobarbaz', 'b..', '1')
# file: test/sql/function/string/regex_extract_all.test
# query
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
# reject
select regexp_extract_all('', '(')
SELECT str, REGEXP_EXTRACT_ALL(str,'ab++') AS m1_long, FROM ( VALUES ('acd'), ('abcd'), ('abbcd'), ('abbbcd') ) AS t(str)
select REGEXP_EXTRACT_ALL('hello', '.', 2)
# file: test/sql/function/string/regex_extract_all_struct.test
# query
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
# reject
SELECT regexp_extract_all('abc', '(a)(b)(c)', [])
SELECT regexp_extract_all('abc', '(a)(b)(c)', ['x','x'])
SELECT regexp_extract_all('abc', '(a)(b)', ['g1','g2','g3'])
SELECT regexp_extract_all('abc', NULL, ['g'])
WITH params(name_list) AS (SELECT ['g1','g2']) SELECT regexp_extract_all('abc', '(a)(b)', name_list) FROM params
SELECT regexp_extract_all('abc', '(a)', NULL::VARCHAR[])
SELECT regexp_extract_all('abc', '(a)(b)', []::VARCHAR[])
SELECT regexp_extract_all('abc', '(a)', ['g1', NULL::VARCHAR])
# file: test/sql/function/string/regex_filter_pushdown.test
# setup
CREATE TABLE regex(s STRING)
# query
CREATE TABLE regex(s STRING)
INSERT INTO regex VALUES ('asdf'), ('xxxx'), ('aaaa')
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f')
SELECT s FROM regex WHERE NOT REGEXP_MATCHES(s, 'as(c|d|e)f')
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f') AND s = 'asdf'
SELECT s FROM regex WHERE REGEXP_MATCHES(s, 'as(c|d|e)f') AND REGEXP_MATCHES(s, 'as[a-z]f')
# file: test/sql/function/string/regex_replace.test
# setup
CREATE TABLE test(v VARCHAR)
create table regex (s string, r string)
# query
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
# reject
SELECT regexp_replace(v, 'h.*', 'world', v) FROM test ORDER BY v
SELECT regexp_replace('asdf', '.*SD.*', 'a', 'q')
select regexp_matches('abc', '*')
select regexp_replace('abc', '*', 'X')
select regexp_matches(s, r) from regex
select regexp_replace(s, r, 'X') from regex
# file: test/sql/function/string/regex_search.test
# setup
CREATE TABLE t0 as FROM VALUES('asdf') t(c0)
CREATE TABLE regex(s STRING, p STRING)
CREATE TABLE test(v VARCHAR)
# query
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
# reject
SELECT regexp_matches('', '\X')
SELECT regexp_matches(c0, '.*SD.*', NULL) from t0
SELECT regexp_matches(v, 'h.*', v) FROM test ORDER BY v
SELECT regexp_matches(c0, '.*SD.*', 'q') from t0
SELECT regexp_matches(c0, '.*SD.*', 'g') from t0
SELECT regexp_matches(s, p) FROM regex
# file: test/sql/function/string/regexp_split_to_table.test
# query
SELECT regexp_split_to_table('a b c', ' ')
SELECT regexp_split_to_table('axbyc', '[x|y]')
SELECT regexp_split_to_table('axbyc', '[x|y]'), 42
# file: test/sql/function/string/regexp_unicode_literal.test
# setup
CREATE TABLE data(wsc INT, zipcode VARCHAR)
# query
CREATE TABLE data(wsc INT, zipcode VARCHAR)
INSERT INTO data VALUES (32, '00' || chr(32) || '001'), (160, '00' || chr(160) || '001'), (0, '00🦆001')
# file: test/sql/function/string/sha1.test
# setup
CREATE TABLE strings AS SELECT s::VARCHAR s FROM generate_series(0,10,1) t(s)
# query
SELECT sha1('hello'), sha1(NULL)
SELECT sha1('')
SELECT sha1(s), sha1('1') FROM strings ORDER BY s
SELECT sha1(s), sha1('1') FROM strings WHERE s::INTEGER BETWEEN 1 AND 3 ORDER BY s
SELECT sha1(''::blob)
# reject
SELECT sha1()
SELECT sha1(42)
# file: test/sql/function/string/sha256.test
# setup
CREATE TABLE strings AS SELECT s::VARCHAR s FROM generate_series(0,10,1) t(s)
# query
SELECT sha256('hello'), sha256(NULL)
SELECT sha256('')
SELECT sha256(s), sha256('1') FROM strings ORDER BY s
SELECT sha256(s), sha256('1') FROM strings WHERE s::INTEGER BETWEEN 1 AND 3 ORDER BY s
# reject
SELECT sha256()
# file: test/sql/function/string/strip_accents.test
# setup
CREATE TABLE collate_test(s VARCHAR, str VARCHAR)
# query
SELECT strip_accents('hello'), strip_accents('héllo')
SELECT strip_accents('mühleisen'), strip_accents('hannes mühleisen')
CREATE TABLE collate_test(s VARCHAR, str VARCHAR)
INSERT INTO collate_test VALUES ('äää', 'aaa')
INSERT INTO collate_test VALUES ('hännës mühlëïsën', 'hannes muhleisen')
INSERT INTO collate_test VALUES ('olá', 'ola')
INSERT INTO collate_test VALUES ('ôâêóáëòõç', 'oaeoaeooc')
SELECT strip_accents(s)=strip_accents(str) FROM collate_test
# file: test/sql/function/string/test_array_extract.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER)
# query
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
# reject
SELECT array_extract('1', 9223372036854775807)
SELECT array_extract('0', -9223372036854775808)
# file: test/sql/function/string/test_ascii.test
# query
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
# reject
SELECT ASCII()
SELECT CHR(-10)
SELECT CHR(1073741824)
SELECT CHR()
# file: test/sql/function/string/test_bar.test
# query
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
# reject
select bar(1, '-infinity'::double, 10)
select bar(1, 0, 10, 'nan'::double)
select bar(1, 0, 10, 'infinity'::double)
select bar(1, 0, 10, '-infinity'::double)
select bar(1, 0, 10, 1001)
select bar(1, 0, 10, 0.99)
# file: test/sql/function/string/test_bit_length.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
select BIT_LENGTH(NULL), BIT_LENGTH(''), BIT_LENGTH('$'), BIT_LENGTH('¢'), BIT_LENGTH('€'), BIT_LENGTH('𐍈')
CREATE TABLE strings(a STRING, b STRING)
INSERT INTO strings VALUES ('', 'Zero'), ('$', NULL), ('¢','Two'), ('€', NULL), ('𐍈','Four')
select BIT_LENGTH(a) FROM strings
select BIT_LENGTH(b) FROM strings
select BIT_LENGTH(a) FROM strings WHERE b IS NOT NULL
# reject
select BIT_LENGTH()
select BIT_LENGTH(1, 2)
# file: test/sql/function/string/test_caseconvert.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
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
# file: test/sql/function/string/test_complex_unicode.test
# query
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
# file: test/sql/function/string/test_concat.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
INSERT INTO strings VALUES ('hello'), ('world'), (NULL)
SELECT s || ' ' || s FROM strings ORDER BY s
SELECT s || ' ' || '🦆' FROM strings ORDER BY s
SELECT s || ' ' || '🦆' || NULL FROM strings ORDER BY s
SELECT CONCAT('hello')
SELECT CONCAT('hello', 33, 22)
SELECT CONCAT('hello', 33, NULL, 22, NULL)
SELECT CONCAT('hello', ' ', s) FROM strings ORDER BY s
# reject
SELECT CONCAT()
# file: test/sql/function/string/test_concat_binding.test
# query
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
# reject
select concat([1], 'hello')
SELECT list_concat([1, 2], ['3', '4'])
SELECT list_concat([1, 2], 4)
# file: test/sql/function/string/test_concat_function.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
select CONCAT(a, 'SUFFIX') FROM strings
select CONCAT('PREFIX', b) FROM strings
select CONCAT(a, b) FROM strings
select CONCAT(a, b, 'SUFFIX') FROM strings
select CONCAT(a, b, a) FROM strings
select CONCAT('1', '2', '3', '4', '5', '6', '7', '8', '9', '0')
select '1234567890' || '1234567890', '1234567890' || NULL
select CONCAT('1234567890', '1234567890'), CONCAT('1234567890', NULL)
# file: test/sql/function/string/test_concat_ws.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
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
# reject
select CONCAT_WS()
select CONCAT_WS(',')
# file: test/sql/function/string/test_contains.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# reject
SELECT contains(NULL,NULL) FROM strings
# file: test/sql/function/string/test_contains_utf8.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
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
# file: test/sql/function/string/test_damerau_levenshtein.test
# setup
CREATE TABLE strings(s_left VARCHAR, s_right VARCHAR)
# query
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
# reject
SELECT damerau_levenshtein('one', 'two', 'three')
SELECT damerau_levenshtein('one')
SELECT damerau_levenshtein()
# file: test/sql/function/string/test_format.test
# query
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
# reject
SELECT format('{}')
SELECT format('{} {}', 'hello')
SELECT format('{:s}', 42)
SELECT format('{:d}', 'hello')
# file: test/sql/function/string/test_format_extensions.test
# query
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
# reject
select format('{:t}', 123456789)
select format('{1}', 123456789)
select printf('%:', 123456789)
select printf('%:', 123456789.123)
select printf('%:', 'str')
# file: test/sql/function/string/test_glob.test
# setup
CREATE TABLE strings(s STRING, pat STRING)
# query
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
# file: test/sql/function/string/test_ilike.test
# setup
CREATE TABLE strings(s STRING, pat STRING)
CREATE TABLE unicode_strings(s STRING, pat STRING)
# query
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
# file: test/sql/function/string/test_ilike_escape.test
# setup
CREATE TABLE tbl(str VARCHAR, pat VARCHAR)
# query
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
# reject
select 'a%c' ilike 'a$%C' escape '///'
SELECT str ILIKE pat ESCAPE str FROM tbl
# file: test/sql/function/string/test_instr.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# file: test/sql/function/string/test_instr_utf8.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
SELECT INSTR(s,'á') FROM strings
SELECT POSITION('á' in s) FROM strings
SELECT INSTR(s,'olá mundo') FROM strings
SELECT INSTR(s,'你好世界') FROM strings
SELECT instr(s,'two ñ thr') FROM strings
SELECT instr(s,'ñ') FROM strings
SELECT instr(s,'₡ four 🦆 e') FROM strings
SELECT instr(s,'🦆 end') FROM strings
# file: test/sql/function/string/test_issue_1812.test
# setup
CREATE TABLE t (str VARCHAR)
# query
CREATE TABLE t (str VARCHAR)
INSERT INTO t VALUES ('hello1'), ('hello2'), ('hello3'), ('world1'), ('world2'), ('world3')
SELECT COUNT(*) FROM t WHERE str LIKE '%o%'
SELECT COUNT(*) FROM t WHERE str LIKE '%rld%'
SELECT COUNT(*) FROM t WHERE str LIKE '%o%' OR (str LIKE '%o%' AND str LIKE '%rld%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR str LIKE '%o%'
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR (str LIKE '%o%') OR (str LIKE '%o%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%rld%') OR (str LIKE '%o%') OR (str LIKE '%o%' AND str LIKE 'blabla%')
SELECT COUNT(*) FROM t WHERE (str LIKE '%o%' AND str LIKE '%1%') OR (str LIKE '%o%' AND str LIKE '%1%' AND str LIKE 'blabla%') OR (str LIKE '%o%' AND str LIKE '%1%' AND str LIKE 'blabla2%')
# file: test/sql/function/string/test_jaccard.test
# setup
CREATE TABLE strings(s VARCHAR, t VARCHAR)
# query
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
# reject
SELECT jaccard('hello', '')
SELECT jaccard('', 'hello')
SELECT jaccard('', '')
select round(jaccard('', t), 1) from strings
select round(jaccard(s, ''), 1) from strings
# file: test/sql/function/string/test_jaro_winkler.test
# setup
create table test as select '0000' || range::varchar s from range(10000)
# query
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
# file: test/sql/function/string/test_left.test
# setup
CREATE TABLE strings(a STRING, b BIGINT)
# query
DROP TABLE IF EXISTS strings
CREATE TABLE strings(a STRING, b BIGINT)
INSERT INTO STRINGS VALUES ('abcd', 0), ('abc', 1), ('abc', 2), ('abc', 3), ('abc', 4)
INSERT INTO STRINGS VALUES ('abcd', 0), ('abc', -1), ('abc', -2), ('abc', -3), ('abc', -4)
INSERT INTO STRINGS VALUES (NULL, 0), ('abc', NULL), (NULL, NULL)
SELECT LEFT_GRAPHEME('🦆🤦S̈', 0), LEFT_GRAPHEME('🦆🤦S̈', 1), LEFT_GRAPHEME('🦆🤦S̈', 2), LEFT_GRAPHEME('🦆🤦S̈', 3)
SELECT LEFT_GRAPHEME('🦆🤦S̈', 0), LEFT_GRAPHEME('🦆🤦S̈', -1), LEFT_GRAPHEME('🦆🤦S̈', -2), LEFT_GRAPHEME('🦆🤦S̈', -3)
# file: test/sql/function/string/test_length.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
SELECT length(s) FROM strings ORDER BY s
SELECT length(s || ' ' || '🦆') FROM strings ORDER BY s
SELECT char_length('asdf'), CHARACTER_LENGTH('asdf')
# file: test/sql/function/string/test_levenshtein.test
# setup
CREATE TABLE strings(s VARCHAR, t VARCHAR)
# query
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
# file: test/sql/function/string/test_like.test
# setup
CREATE TABLE strings(s STRING, pat STRING)
# query
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
# reject
SELECT 'hello' LIKE 'hê?llo' COLLATE idontexist
# file: test/sql/function/string/test_like_escape.test
# setup
CREATE TABLE strings(s STRING, pat STRING)
# query
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
# reject
SELECT '%' LIKE '%' ESCAPE '%'
SELECT '%' LIKE '*' ESCAPE '*'
SELECT '%_' LIKE '%_' ESCAPE '\\'
SELECT '%_' LIKE '%_' ESCAPE '**'
# file: test/sql/function/string/test_mismatches.test
# setup
CREATE TABLE strings(s VARCHAR, t VARCHAR)
# query
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
# reject
SELECT mismatches('', '')
SELECT mismatches('hoi', 'hallo')
SELECT mismatches('hallo', 'hoi')
SELECT mismatches('', 'hallo')
SELECT mismatches('hi', '')
SELECT mismatches('', s) FROM strings ORDER BY s
SELECT mismatches(s, '') FROM strings ORDER BY s
SELECT mismatches(s, 'hallo') FROM strings
# file: test/sql/function/string/test_pad.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
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
# reject
select LPAD()
select LPAD(1)
select LPAD(1, 2)
select LPAD('Hello', 10, '')
select LPAD('a', 100000000000000000, 0)
select RPAD()
select RPAD(1)
select RPAD(1, 2)
# file: test/sql/function/string/test_prefix.test
# setup
CREATE TABLE t0(c0 VARCHAR)
# query
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
# file: test/sql/function/string/test_printf.test
# setup
CREATE TABLE strings(idx INTEGER, fmt STRING, pint INTEGER, pstring STRING)
# query
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
# reject
SELECT printf('%s')
SELECT printf('%s %s', 'hello')
SELECT printf('%s', 42)
SELECT printf('%d', 'hello')
SELECT printf(fmt) FROM strings ORDER BY idx
# file: test/sql/function/string/test_repeat.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
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
# reject
select REPEAT()
select REPEAT(1)
select REPEAT('hello', 'world')
select REPEAT('hello', 'world', 3)
# file: test/sql/function/string/test_replace.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
select REPLACE('This is the main test string', NULL, 'ALT')
select REPLACE(NULL, 'main', 'ALT')
select REPLACE('This is the main test string', 'main', NULL)
select REPLACE('This is the main test string', 'main', 'ALT')
select REPLACE('This is the main test string', 'main', 'larger-main')
select REPLACE('aaaaaaa', 'a', '0123456789')
select REPLACE(a, 'l', '-') FROM strings
select REPLACE(b, 'Ä', '--') FROM strings
select REPLACE(a, 'H', '') FROM strings WHERE b IS NOT NULL
# reject
select REPLACE(1)
select REPLACE(1, 2)
select REPLACE(1, 2, 3, 4)
# file: test/sql/function/string/test_reverse.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
select REVERSE(''), REVERSE('Hello'), REVERSE('MotörHead'), REVERSE(NULL)
select REVERSE(a) FROM strings
select REVERSE(b) FROM strings
select REVERSE(a) FROM strings WHERE b IS NOT NULL
# reject
select REVERSE()
select REVERSE(1, 2)
select REVERSE('hello', 'world')
# file: test/sql/function/string/test_right.test
# setup
CREATE TABLE strings(a STRING, b BIGINT)
# query
SELECT RIGHT_GRAPHEME('🦆🤦S̈', 0), RIGHT_GRAPHEME('🦆🤦S̈', 1), RIGHT_GRAPHEME('🦆🤦S̈', 2), RIGHT_GRAPHEME('🦆🤦S̈', 3)
SELECT RIGHT_GRAPHEME('🦆🤦S̈', 0), RIGHT_GRAPHEME('🦆🤦S̈', -1), RIGHT_GRAPHEME('🦆🤦S̈', -2), RIGHT_GRAPHEME('🦆🤦S̈', -3)
SELECT right_grapheme('a', -9223372036854775808)
SELECT "right"('a', -9223372036854775808)
# reject
SELECT right_grapheme('a', 9223372036854775808)
# file: test/sql/function/string/test_similar_to.test
# setup
CREATE TABLE strings (s STRING, p STRING)
# query
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
# reject
SELECT s FROM strings WHERE s SIMILAR TO 'ab.*%' {escape ''}
# file: test/sql/function/string/test_split_part.test
# query
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
# reject
select split_part()
select split_part('a')
select split_part('a','a')
# file: test/sql/function/string/test_starts_with_function.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# file: test/sql/function/string/test_starts_with_function_utf8.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
SELECT starts_with(s,'á') FROM strings
SELECT starts_with(s,'olá mundo') FROM strings
SELECT starts_with(s,'你好世界') FROM strings
SELECT starts_with(s,'two ñ thr') FROM strings
SELECT starts_with(s,'ñ') FROM strings
SELECT starts_with(s,'₡ four 🦆 e') FROM strings
# file: test/sql/function/string/test_starts_with_operator.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# file: test/sql/function/string/test_starts_with_operator_utf8.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
SELECT s ^@ 'á' FROM strings
SELECT s ^@ 'olá mundo' FROM strings
SELECT s ^@ '你好世界' FROM strings
SELECT s ^@ 'two ñ thr' FROM strings
SELECT s ^@ 'ñ' FROM strings
SELECT s ^@ '₡ four 🦆 e' FROM strings
# file: test/sql/function/string/test_string_array_slice.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# file: test/sql/function/string/test_string_slice.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
CREATE TABLE nulltable(n VARCHAR)
# query
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
# reject
SELECT NULL::VARCHAR[off:length+off] FROM strings
SELECT NULL::VARCHAR[NULL:length+NULL] FROM strings
SELECT NULL::VARCHAR[off:NULL+off] FROM strings
SELECT NULL::VARCHAR[NULL:NULL+NULL] FROM strings
# file: test/sql/function/string/test_string_split.test
# setup
CREATE TABLE strings_with_null (s VARCHAR)
CREATE TABLE documents(s VARCHAR)
# query
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
# reject
select string_split()
select string_split('a')
SELECT string_split_regex(a, '[') FROM test ORDER BY a
# file: test/sql/function/string/test_subscript.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER)
# query
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
# reject
SELECT NULL::VARCHAR[off] FROM strings
SELECT NULL::VARCHAR[NULL] FROM strings
# file: test/sql/function/string/test_substring.test
# setup
CREATE TABLE strings(s VARCHAR, off INTEGER, length INTEGER)
# query
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
# file: test/sql/function/string/test_substring_utf8.test
# setup
CREATE TABLE strings(s VARCHAR)
# query
INSERT INTO strings VALUES ('twoñthree₡four🦆end')
SELECT substring(s from 1 for 7) FROM strings
SELECT substring(s from 10 for 7) FROM strings
SELECT substring(s from 15 for 7) FROM strings
# file: test/sql/function/string/test_suffix.test
# query
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
# file: test/sql/function/string/test_to_base.test
# setup
CREATE TABLE fib AS SELECT * FROM (VALUES (0), (1), (1), (2), (3), (5), (8), (13), (21), (34), (55), (89), (144), (233), (377), (610), (987), (1597), (2584), (4181), (6765), (10946), (17711), (28657), (46368) )
# query
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
# reject
SELECT to_base(-10, 2)
SELECT to_base(-10, 2, 64)
SELECT to_base(10, 1)
SELECT to_base(10, 37)
SELECT to_base(10, 0, 10)
SELECT to_base(10, 37, 10)
SELECT to_base(10, 2, -10)
# file: test/sql/function/string/test_translate.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
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
# reject
select TRANSLATE(1)
select TRANSLATE(1, 2)
select TRANSLATE(1, 2, 3, 4)
# file: test/sql/function/string/test_trim.test
# setup
CREATE TABLE strings(a STRING, b STRING)
CREATE TABLE trim_test(a VARCHAR, b VARCHAR)
# query
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
# reject
select LTRIM()
select LTRIM('hello', 'world', 'aaa')
select RTRIM()
select RTRIM('hello', 'world', 'aaa')
select TRIM()
select TRIM('hello', 'world', 'aaa')
# file: test/sql/function/string/test_unicode.test
# setup
CREATE TABLE strings(a STRING, b STRING)
# query
select UNICODE(NULL), UNICODE(''), UNICODE('$'), UNICODE('¢'), UNICODE('€'), UNICODE('𐍈')
select UNICODE(a) FROM strings
select UNICODE(b) FROM strings
select UNICODE(a) FROM strings WHERE b IS NOT NULL
# reject
select UNICODE()
select UNICODE(1, 2)
# file: test/sql/function/string/test_url_encode.test
# query
SELECT url_encode(''), url_decode('')
SELECT url_encode(NULL), url_decode(NULL)
SELECT url_decode(url_encode('http://www.google.com/this is a long url'))
SELECT COUNT(*) from range(1000) t(n) WHERE url_decode(url_encode(chr(n::INT))) = chr(n::INT)
SELECT url_decode('%'), url_decode('%5'), url_decode('%X'), url_decode('%%')
# reject
select url_decode('%FF%FF%FF')
# file: test/sql/function/nested/array_extract_unnamed_struct.test
# query
SELECT (ROW(42, 84))[1]
SELECT (ROW(42, 84))[2]
SELECT UNNEST(ROW(42, 84))
# reject
SELECT (ROW(42, 84))['element']
SELECT (ROW(42, 84))[0]
SELECT (ROW(42, 84))[9999]
SELECT (ROW(42, 84))[-1]
SELECT (ROW(42, 84))[9223372036854775807]
SELECT (ROW(42, 84))[(-9223372036854775808)::BIGINT]
# file: test/sql/function/nested/test_issue_5437.test
# query
with data as ( select * from (VALUES ('Amsterdam', {'x': 1, 'y': 2, 'z': 3}), ('London', {'x': 4, 'y': 5, 'z': 6})) Cities(Name, Id) ) select *, struct_insert(Id, d := 4) from data
# file: test/sql/function/nested/test_struct_insert.test
# setup
CREATE TABLE tbl (col STRUCT(i INT))
# query
SELECT struct_insert ({a: 1, b: 2}, c := 3)
WITH data AS (SELECT 1 AS a, 2 AS b, 3 AS c) SELECT struct_insert (data, d := 4) FROM data
SELECT struct_insert({'a': 1, 'b': 'abc', 'c': true}, d := {'a': 'new stuff'})
INSERT INTO tbl SELECT {'i': range} FROM range(3)
SELECT struct_insert(col, a := col.i + 1, b := NULL::VARCHAR) FROM tbl ORDER BY ALL
SELECT struct_insert(col, a := NULL, b := NULL::VARCHAR, c := [NULL]) FROM tbl ORDER BY ALL
# reject
SELECT struct_insert()
SELECT struct_insert({a: 1, b: 2})
SELECT struct_insert(123, a := 1)
SELECT struct_insert({a: 1, b: 2}, a := 2)
# file: test/sql/function/nested/test_struct_update.test
# setup
CREATE TABLE tbl (col STRUCT(i INT))
# query
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
# reject
SELECT struct_update()
SELECT struct_update({a: 1, b: 2})
SELECT struct_update(123, a := 1)
SELECT struct_update({a: 1, b: 2}, a := 2, a := 3)
# file: test/sql/function/timestamp/age.test
# setup
CREATE TABLE timestamp(t1 TIMESTAMP, t2 TIMESTAMP)
# query
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
# file: test/sql/function/timestamp/current_time.test
# query
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
# file: test/sql/function/timestamp/current_timestamp.test
# query
SET TimeZone = 'America/Chihuahua'
SELECT EXTRACT(MILLENNIUM FROM NOW())
SELECT SUFFIX(CURRENT_TIMESTAMP::VARCHAR, '-06')
# file: test/sql/function/timestamp/epoch.test
# query
SELECT make_timestamp(0) as epoch1, make_timestamp(1574802684123 * 1000) as epoch2, make_timestamp(-291044928000 * 1000) as epoch3, make_timestamp(-291081600000 * 1000) as epoch4, make_timestamp(-291081600001 * 1000) as epoch5, make_timestamp(-290995201000 * 1000) as epoch6
SELECT make_timestamp_ms(0) as epoch1, make_timestamp_ms(1574802684123) as epoch2, make_timestamp_ms(-291044928000) as epoch3, make_timestamp_ms(-291081600000) as epoch4, make_timestamp_ms(-291081600001) as epoch5, make_timestamp_ms(-290995201000) as epoch6
SELECT to_timestamp(0), to_timestamp(1), to_timestamp(1574802684), to_timestamp(-1)
SELECT to_timestamp(1284352323.5)
# reject
SELECT to_timestamp(1284352323::DOUBLE * 100000000)
# file: test/sql/function/timestamp/make_date.test
# setup
CREATE TABLE IF NOT EXISTS dates (d date)
CREATE TABLE timestamps(ts TIMESTAMP)
CREATE TABLE times(t TIME)
# query
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
# reject
SELECT make_timestamp(9223372036854775807)
SELECT make_timestamp_ns(9223372036854775807)
SELECT make_timestamp(294247, 1, 10, 4, 0, 54.775807)
# file: test/sql/function/timestamp/test_date_diff_epoch.test
# query
SELECT start_ts, end_ts, DATEDIFF('day', start_ts, end_ts) AS dd_hour FROM VALUES ( '1970-01-03 12:12:12'::TIMESTAMP, '1969-12-25 05:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('hour', start_ts, end_ts) AS dd_hour FROM VALUES ( '1970-01-01 12:12:12'::TIMESTAMP, '1969-12-31 05:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('minute', start_ts, end_ts) AS dd_minute FROM VALUES ( '1970-01-01 00:12:12'::TIMESTAMP, '1969-12-31 23:05:05'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('second', start_ts, end_ts) AS dd_second FROM VALUES ( '1970-01-01 00:00:12.456'::TIMESTAMP, '1969-12-31 23:59:05.123'::TIMESTAMP ) x(start_ts, end_ts)
SELECT start_ts, end_ts, DATEDIFF('millisecond', start_ts, end_ts) AS dd_second FROM VALUES ( '1970-01-01 00:00:12.456789'::TIMESTAMP, '1969-12-31 23:59:05.123456'::TIMESTAMP ) x(start_ts, end_ts)
# file: test/sql/function/timestamp/test_date_part.test
# setup
CREATE TABLE timestamps(ts TIMESTAMP)
CREATE TABLE millennia AS SELECT * FROM (VALUES ('1001-03-15 (BC) 20:38:40'::TIMESTAMP), ('0044-03-15 (BC) 20:38:40'::TIMESTAMP), ('0998-02-16 20:38:40'::TIMESTAMP), ('1998-02-16 20:38:40'::TIMESTAMP), ('2001-02-16 20:38:40'::TIMESTAMP) ) tbl(ts)
# query
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
# reject
SELECT ts, DATE_PART(['duck', 'month', 'day'], ts) AS parts FROM timestamps ORDER BY 1
SELECT ts, DATE_PART(['year', 'month', 'day', 'year'], ts) AS parts FROM timestamps ORDER BY 1
SELECT DATE_PART([], ts) FROM timestamps
SELECT DATE_PART(['year', NULL, 'month'], ts) FROM timestamps
WITH parts(p) AS (VALUES (['year', 'month', 'day']), (['hour', 'minute', 'microsecond'])) SELECT DATE_PART(p, ts) FROM parts, timestamps
# file: test/sql/function/timestamp/test_extract.test
# setup
CREATE TABLE timestamps(i TIMESTAMP)
# query
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
# file: test/sql/function/timestamp/test_extract_ms.test
# setup
CREATE TABLE timestamps(i TIMESTAMP)
# query
INSERT INTO timestamps VALUES ('1993-08-14 08:22:33.42'), (NULL)
SELECT EXTRACT(second FROM i) FROM timestamps
SELECT EXTRACT(minute FROM i) FROM timestamps
SELECT EXTRACT(milliseconds FROM i) FROM timestamps
SELECT EXTRACT(microseconds FROM i) FROM timestamps
# file: test/sql/function/timestamp/test_icu_age.test
# setup
CREATE TABLE timestamps(t1 TIMESTAMPTZ, t2 TIMESTAMPTZ)
# query
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
# file: test/sql/function/timestamp/test_icu_dateadd.test
# setup
CREATE TABLE intervals AS SELECT iv FROM (VALUES (INTERVAL 1 year), (INTERVAL (-1) year), (INTERVAL 1 month), (INTERVAL (-1) month), (INTERVAL 13 month), (INTERVAL (-15) month), (INTERVAL 1 day), (INTERVAL (-1) day), (INTERVAL 32 day), (INTERVAL (-40) day), (INTERVAL 1 hour), (INTERVAL (-1) hour), (INTERVAL 11 hour), (INTERVAL (-14) hour), (INTERVAL 1 minute), (INTERVAL (-1) minute), (INTERVAL 6 minute), (INTERVAL (-72) minute), (INTERVAL 1 second), (INTERVAL (-1) second), (INTERVAL 23 second), (INTERVAL (-118) second), (INTERVAL 1 millisecond), (INTERVAL (-1) millisecond), (INTERVAL 910 millisecond), (INTERVAL (-150) millisecond), (INTERVAL 1 microsecond), (INTERVAL (-1) microsecond), (INTERVAL 612 microsecond), (INTERVAL (-485) microsecond) ) tbl(iv)
CREATE TABLE limits AS SELECT ts, label FROM (VALUES ('290309-12-22 (BC) 00:00:00Z'::TIMESTAMPTZ, 'tsmin'), ('294247-01-10 04:00:54.775806Z'::TIMESTAMPTZ, 'tsmax') ) tbl(ts, label)
CREATE TABLE london AS ( SELECT * FROM (VALUES ('2000-10-29 03:00:00+00'::TIMESTAMPTZ, '2000-03-26 03:00:00+01'::TIMESTAMPTZ, '2000-01-03 00:00:00+00'::TIMESTAMPTZ) ) tbl(dst2, dst1, origin) )
# query
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
# reject
select 'epoch'::timestamptz + '9223372036854775000 microseconds'::interval
select 'epoch'::timestamptz + '-9223372022400001001 microseconds'::interval
select '9223372036854775000 microseconds'::interval + 'epoch'::timestamptz
select '-9223372022400001001 microseconds'::interval + 'epoch'::timestamptz
select 'epoch'::timestamptz - '9223372022400001001 microseconds'::interval
SELECT ts + (INTERVAL (-1) year) FROM limits WHERE label = 'tsmin'
SELECT ts + (INTERVAL (-1) month) FROM limits WHERE label = 'tsmin'
SELECT ts + (INTERVAL (-15) month) FROM limits WHERE label = 'tsmin'
# file: test/sql/function/timestamp/test_icu_datediff.test
# setup
CREATE TABLE datetime1 AS SELECT '2005-12-31 23:59:59.9999999-08'::TIMESTAMPTZ AS startdate, '2006-01-01 00:00:00.0000000-08'::TIMESTAMPTZ AS enddate
CREATE TABLE issue9673(starttime TIMESTAMPTZ, recordtime TIMESTAMPTZ)
# query
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
# file: test/sql/function/timestamp/test_icu_datepart.test
# setup
CREATE TABLE timestamps AS SELECT * FROM (VALUES ('0044-03-13 (BC) 10:33:41.987654+01'::TIMESTAMPTZ, 'era'), ('1962-07-31 12:20:48.123456+00'::TIMESTAMPTZ, 'epoch'), ('2021-01-01 00:00:00+00'::TIMESTAMPTZ, 'year'), ('2021-02-02 00:00:00+00'::TIMESTAMPTZ, 'month'), ('2021-11-26 10:15:13.123456+00'::TIMESTAMPTZ, 'microsecond'), ('2021-11-15 02:30:00-08'::TIMESTAMPTZ, 'hour'), ('2021-11-15 02:30:00-07'::TIMESTAMPTZ, 'minute'), ('2021-12-25 00:00:00+02'::TIMESTAMPTZ, 'day'), ('infinity'::TIMESTAMPTZ, 'second'), ('-infinity'::TIMESTAMPTZ, 'decade'), (NULL::TIMESTAMPTZ, 'century'), ) tbl(ts, part)
CREATE TABLE februaries AS SELECT ts::TIMESTAMPTZ AS ts FROM (VALUES ('1900-02-12'), ('1992-02-12'), ('2000-02-12') ) tbl(ts)
CREATE TABLE t1(c0 TIMESTAMPTZ)
# query
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
# reject
SELECT DATE_PART(['duck', 'minute', 'microsecond', 'timezone'], ts), ts FROM timestamps ORDER BY 2
SELECT DATE_PART(['era', 'year', 'month', 'era'], ts), ts FROM timestamps ORDER BY 2
# file: test/sql/function/timestamp/test_icu_datesub.test
# setup
CREATE TABLE datetime1 AS SELECT '2004-01-31 12:00:00-08'::TIMESTAMPTZ AS startdate, '2004-02-29 13:05:00-08'::TIMESTAMPTZ AS enddate
CREATE TABLE dateparts AS SELECT datepart FROM (VALUES ('year'), ('quarter'), ('month'), ('day'), ('dayofyear'), ('hour'), ('minute'), ('second'), ('millisecond'), ('microsecond'), ('decade'), ('century'), ('millennium'), ('week'), ('yearweek'), ('isoyear') ) tbl(datepart)
# query
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
# file: test/sql/function/timestamp/test_icu_datetrunc.test
# setup
CREATE TABLE timestamps(d TIMESTAMPTZ, s VARCHAR)
# query
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
# reject
SELECT date_trunc('duck', TIMESTAMPTZ '2019-01-06 04:03:02-08') FROM timestamps LIMIT 1
# file: test/sql/function/timestamp/test_icu_makedate.test
# setup
CREATE TABLE timestamps(ts TIMESTAMPTZ)
CREATE TABLE timezones AS (SELECT mm, tz FROM (VALUES (1, 'America/New_York'), (2, 'America/Los_Angeles'), (3, 'Europe/Rome'), (4, 'Asia/Kathmandu'), (5, 'Canada/Newfoundland'), (7, 'Pacific/Auckland'), (8, 'Asia/Hong_Kong'), (12, 'US/Hawaii') ) tbl(mm, tz) )
CREATE TABLE timeparts AS ( SELECT ts, yeartz(ts) yyyy, month(ts) mm, day(ts) dd, hour(ts) hr, minute(ts) mn, microsecond(ts) / 1000000.0 as ss, tz FROM timestamps t LEFT JOIN timezones z ON (month(t.ts) = z.mm) ORDER BY ts )
CREATE MACRO yeartz(ts) AS year(ts::TIMESTAMPTZ) * (CASE WHEN ERA(ts::TIMESTAMPTZ) > 0 THEN 1 ELSE -1 END)
# query
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
# reject
SELECT ts, make_timestamptz(yyyy, mm, dd, hr, mn, ss, 'Europe/Duck') mts FROM timeparts
WITH all_types AS ( select * exclude(small_enum, medium_enum, large_enum) from test_all_types() ) SELECT make_timestamptz( CAST(century(CAST(a."interval" AS INTERVAL)) AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(txid_current() AS BIGINT), 'UTC') FROM all_types a
SELECT make_timestamptz(9223372036854775807)
SELECT make_timestamptz(294248, 1, 10, 4, 0, 54.775807)
# file: test/sql/function/timestamp/test_icu_strftime.test
# setup
CREATE TABLE timestamps AS SELECT ts::TIMESTAMPTZ AS ts FROM (VALUES ('-infinity'), ('0044-03-13 (BC) 10:33:41.987654+01'), ('1962-07-31 12:20:48.123456+00'), ('epoch'), ('2021-01-01 00:00:00+00'), ('2021-02-02 00:00:00+00'), ('2021-11-26 10:15:13.123456+00'), ('2021-11-15 02:30:00-08'), ('2021-11-15 02:30:00-07'), ('2021-12-25 00:00:00+02'), ('infinity'), (NULL), ) tbl(ts)
CREATE TABLE formats (f VARCHAR)
# query
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
# reject
SELECT ts, strftime(ts, '%C') FROM timestamps
# file: test/sql/function/timestamp/test_icu_strptime.test
# setup
CREATE TABLE zones AS ( FROM (VALUES ('Etc/GMT-14'), ('NZ-CHAT'), ('Pacific/Auckland'), ('Pacific/Enderbury'), ('Australia/LHI'), ('Australia/Melbourne'), ('Pacific/Efate'), ('Australia/Darwin'), ('Asia/Tokyo'), ('Australia/Eucla'), ('Asia/Shanghai'), ('Asia/Novosibirsk'), ('Asia/Yangon'), ('Asia/Omsk'), ('Asia/Kathmandu'), ('Asia/Colombo'), ('Asia/Oral'), ('Asia/Kabul'), ('Europe/Astrakhan'), ('Asia/Tehran'), ('Asia/Kuwait'), ('Asia/Nicosia'), ('Europe/Budapest'), ('Etc/GMT-0'), ('Atlantic/Azores'), ('America/Cayenne'), ('America/Nuuk'), ('CNT'), ('America/Martinique'), ('America/Louisville'), ('America/Rainy_River'), ('America/Shiprock'), ('Mexico/BajaNorte'), ('America/Sitka'), ('Pacific/Marquesas'), ('Pacific/Johnston'), ('Pacific/Niue'), ('Etc/GMT+12'), ) tbl(tz_name) )
CREATE TABLE abbrevs AS ( FROM (VALUES ('Etc/GMT-14'), ('NZ-CHAT'), ('NZ'), ('Pacific/Enderbury'), ('Australia/Hobart'), ('Australia/LHI'), ('Pacific/Efate'), ('Australia/Adelaide'), ('Etc/GMT-9'), ('Australia/Eucla'), ('CTT'), ('Asia/Phnom_Penh'), ('Asia/Yangon'), ('Asia/Thimbu'), ('Asia/Kathmandu'), ('IST'), ('Asia/Qyzylorda'), ('Asia/Kabul'), ('Europe/Samara'), ('Iran'), ('EAT'), ('CAT'), ('Europe/Bratislava'), ('GMT'), ('Atlantic/Azores'), ('America/Cayenne'), ('America/Nuuk'), ('CNT'), ('PRT'), ('America/Panama'), ('America/Rankin_Inlet'), ('Canada/Yukon'), ('PST'), ('America/Nome'), ('Pacific/Marquesas'), ('Pacific/Johnston'), ('Pacific/Niue'), ('Etc/GMT+12'), ) tbl(tz_name) )
CREATE TABLE offsets AS FROM (VALUES ('+14'), ('+13'), ('+12:45'), ('+12'), ('+11'), ('+10:30'), ('+10'), ('+09:30'), ('+09'), ('+08:45'), ('+08'), ('+07'), ('+06:30'), ('+06'), ('+05:45'), ('+05:30'), ('+05'), ('+04:30'), ('+04'), ('+03:30'), ('+03'), ('+02'), ('+01'), ('+00'), ('-01'), ('-02'), ('-03'), ('-03:30'), ('-04'), ('-05'), ('-06'), ('-07'), ('-08'), ('-09'), ('-09:30'), ('-10'), ('-11'), ('-12'), ) tbl(utc_offset)
CREATE TABLE multiples (s VARCHAR, f VARCHAR)
# query
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
# reject
SELECT strptime(s, f) FROM multiples
select strptime('2022-03-05 17:59:17.877 CST', '%C')
select strptime('2022-03-05 17:59:17.877 CST', '%Y-%m-%d %H:%M:%S.%g')
select 'fnord'::timestamptz
SELECT TIMESTAMPTZ '294247-01-10 04:00:54.7758'
# file: test/sql/function/timestamp/test_icu_time_bucket_timestamptz.test
# setup
CREATE TABLE timestamps_tz(w INTERVAL, t TIMESTAMPTZ, shift INTERVAL, origin TIMESTAMPTZ, timezone VARCHAR)
# query
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
# reject
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz, '1 hour 30 minutes'::interval)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz, '2019-04-05 00:00:00+03'::timestamptz)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz, '1 hour 30 minutes'::interval)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz, '2018-04-05 00:00:00+11'::timestamptz)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00+07'::timestamptz)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00+07'::timestamptz, '1 hour 30 minutes'::interval)
# file: test/sql/function/timestamp/test_now.test
# setup
CREATE TABLE t1(t TIMESTAMP)
# query
CREATE TABLE t1(t TIMESTAMP)
INSERT INTO t1 VALUES (NOW())
INSERT INTO t1 SELECT NOW()
SELECT COUNT(DISTINCT t) FROM t1
# file: test/sql/function/timestamp/test_now_prepared.test
# setup
CREATE TABLE timestamps(ts TIMESTAMP)
CREATE TABLE timestamps_default(ts TIMESTAMP DEFAULT NOW())
# query
PREPARE v1 AS INSERT INTO timestamps VALUES(NOW())
SELECT COUNT(DISTINCT ts) FROM timestamps
CREATE TABLE timestamps_default(ts TIMESTAMP DEFAULT NOW())
INSERT INTO timestamps_default DEFAULT VALUES
SELECT COUNT(DISTINCT ts) FROM timestamps_default
# file: test/sql/function/timestamp/test_strftime_timestamp.test
# query
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
# file: test/sql/function/timestamp/test_strftime_timestamp_ns.test
# query
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
# file: test/sql/function/timestamp/test_strptime.test
# query
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
# reject
SELECT strptime('', '')
SELECT strptime(NULL, '')
SELECT strptime('10.28.1910', ['%d-%m-%Y', '%m-%d-%Y', '%d/%m/%Y', '%m/%d/%Y'])
SELECT strptime('Mon Oct 17 2022 22:00:00 GMT+0000 (GMT)', '%a %b %d %Y %X GMT%z (%Z') as broken
select strptime('2020-12-31 21:25:58.745232+0', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+0X', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+X0', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+000', '%Y-%m-%d %H:%M:%S.%f%z')
# file: test/sql/function/timestamp/test_time_bucket_timestamp.test
# setup
CREATE TABLE timestamps(w INTERVAL, t TIMESTAMP, shift INTERVAL, origin TIMESTAMP)
# query
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
# reject
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp, '2018-04-05 00:00:00'::timestamp)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00'::timestamp)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
# file: test/sql/function/timestamp/test_try_strptime.test
# query
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
# reject
SELECT try_strptime('', '')
SELECT try_strptime(NULL, '')
SELECT try_strptime('21/10/2018', '%-q/%m/%Y')
SELECT try_strptime('2000/10/10', random()::varchar)
# file: test/sql/function/timetz/test_date_part.test
# setup
CREATE TABLE timetzs(d TIMETZ, s VARCHAR)
# query
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
# reject
SELECT era(d) FROM timetzs
SELECT year(d) FROM timetzs
SELECT month(d) FROM timetzs
SELECT day(d) FROM timetzs
SELECT decade(d) FROM timetzs
SELECT century(d) FROM timetzs
SELECT millennium(d) FROM timetzs
SELECT quarter(d) FROM timetzs
# file: test/sql/function/timetz/test_extract.test
# setup
CREATE TABLE timetzs (i TIMETZ)
# query
CREATE TABLE timetzs (i TIMETZ)
INSERT INTO timetzs VALUES (NULL), ('00:00:00+1559'), ('00:00:00+1558'), ('02:30:00'), ('02:30:00+04'), ('02:30:00+04:30'), ('02:30:00+04:30:45'), ('16:15:03.123456'), ('02:30:00+1200'), ('02:30:00-1200'), ('24:00:00-1558'), ('24:00:00-1559'),
SELECT EXTRACT(second FROM i) FROM timetzs
SELECT EXTRACT(minute FROM i) FROM timetzs
SELECT EXTRACT(hour FROM i) FROM timetzs
SELECT EXTRACT(milliseconds FROM i) FROM timetzs
SELECT EXTRACT(microseconds FROM i) FROM timetzs
SELECT EXTRACT(epoch FROM i) FROM timetzs
# file: test/sql/function/timetz/test_icu_cast.test
# setup
create table timetzs (ttz TIMETZ)
# query
SET CALENDAR='gregorian'
SET TIMEZONE='America/Phoenix'
SELECT '01:00:00'::TIMETZ AS ttz
SELECT '01:00:00+02'::TIMETZ AS ttz
create table timetzs (ttz TIMETZ)
# reject
insert into timetzs values ('2402:30:00+1200')
# file: test/sql/function/timetz/timetz_group_by.test
# setup
create table time_testtz as select i::timetz as t from generate_series(TIMESTAMPtz '2001-04-10', TIMESTAMPtz '2001-04-11', INTERVAL 30 MINUTE) as t(i)
# query
create table time_testtz as select i::timetz as t from generate_series(TIMESTAMPtz '2001-04-10', TIMESTAMPtz '2001-04-11', INTERVAL 30 MINUTE) as t(i)
SELECT TYPEOF(t) FROM (select t from time_testtz group by t) LIMIT 1
# file: test/sql/function/uuid/test_uuid.test
# setup
CREATE TEMPORARY TABLE t1 AS SELECT gen_random_uuid() a FROM range(0, 16)
CREATE TEMPORARY TABLE t2 AS SELECT uuid() b FROM range(0, 16)
CREATE TEMPORARY TABLE t3 AS SELECT gen_random_uuid() c FROM range(0, 16)
CREATE TABLE uuids(u UUID NOT NULL DEFAULT gen_random_uuid(), a INTEGER)
# query
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
# file: test/sql/function/uuid/test_uuid_function.test
# query
SELECT uuid_extract_version('ac227128-7d55-7ee0-a765-5025cc52e55a')
SELECT uuid_extract_version(uuidv7())
SELECT uuid_extract_version('ac227128-7d55-4ee0-a765-5025cc52e55a')
SELECT uuid_extract_version(uuidv4())
SELECT uuid_extract_version(gen_random_uuid())
SELECT uuid_extract_timestamp('0196f97a-db14-71c3-9132-9f0b1334466f')
SELECT datediff('month', uuid_extract_timestamp(uuidv7()), now())
# reject
SELECT uuid_extract_timestamp(uuidv4())
# file: test/sql/function/blob/base64.test
# query
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
# reject
SELECT from_base64('ab')
SELECT from_base64('üab')
# file: test/sql/function/blob/create_sort_key.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE varchars(v VARCHAR)
CREATE TABLE int_list(l INT[])
CREATE TABLE structs(s ROW(i INT, v VARCHAR))
CREATE TABLE list_of_structs(s ROW(i INT, v VARCHAR)[])
CREATE TABLE nested_lists(s INT[][])
CREATE TABLE blobs(b BLOB, c BLOB)
CREATE TABLE arrays(l INT[3])
# query
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
# file: test/sql/function/blob/encode.test
# query
SELECT encode('ü')
SELECT decode(encode('ü'))
SELECT decode(encode(a)) || a from (values ('hello'), ('world')) tbl(a)
# file: test/sql/function/blob/test_blob_array_slice.test
# query
select array_slice(NULL::BLOB, 4, 6)
# reject
select array_slice('hello world', 1, 8, 2)
# file: test/sql/function/operator/test_arithmetic.test
# setup
CREATE TABLE integers(i INTEGER)
# query
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
# file: test/sql/function/operator/test_arithmetic_sqllogic.test
# setup
CREATE TABLE tab0(col0 INTEGER, col1 INTEGER, col2 INTEGER)
CREATE TABLE tab1(col0 INTEGER, col1 INTEGER, col2 INTEGER)
CREATE TABLE tab2(col0 INTEGER, col1 INTEGER, col2 INTEGER)
# query
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
# file: test/sql/function/operator/test_bitwise_ops.test
# query
SELECT 1 << 2, NULL << 2, 2 << NULL
SELECT 16 >> 2, 1 >> 2, NULL >> 2, 2 >> NULL
SELECT 1 & 1, 1 & 0, 0 & 0, NULL & 1, 1 & NULL
SELECT 1 | 1, 1 | 0, 0 | 0, NULL | 1, 1 | NULL
SELECT xor(1, 1), xor(1, 0), xor(0, 0), xor(NULL, 1), xor(1, NULL)
SELECT 1::UTINYINT << 7, 1::USMALLINT << 15, 1::UINT32 << 31, 1::UBIGINT << 63
# reject
SELECT 1::TINYINT << -1::TINYINT, 1::TINYINT >> -1::TINYINT, 1::TINYINT << 12::TINYINT, 1::TINYINT >> 12::TINYINT
SELECT 1::SMALLINT << -1::SMALLINT, 1::SMALLINT >> -1::SMALLINT, 1::SMALLINT << 20::SMALLINT, 1::SMALLINT >> 20::SMALLINT
SELECT 1::INT << -1::INT, 1::INT >> -1::INT, 1::INT << 40::INT, 1::INT >> 40::INT
SELECT 1::BIGINT << -1::BIGINT, 1::BIGINT >> -1::BIGINT, 1::BIGINT << 1000::BIGINT, 1::BIGINT >> 1000::BIGINT
SELECT 'hello' << 3
SELECT 3 << 'hello'
SELECT 2.0 << 1
SELECT 1::UINT32 << 32
# file: test/sql/function/operator/test_bitwise_ops_types.test
# setup
CREATE TABLE bitwise_test(i BIGINT, j BIGINT)
# query
CREATE TABLE bitwise_test(i TINYINT, j TINYINT)
INSERT INTO bitwise_test VALUES (1, 1), (1, 0), (0, 1), (0, 0), (1, NULL), (NULL, 1), (NULL, NULL)
SELECT i << j, i >> j, i & j, i | j, xor(i, j) FROM bitwise_test
CREATE TABLE bitwise_test(i SMALLINT, j SMALLINT)
CREATE TABLE bitwise_test(i INTEGER, j INTEGER)
CREATE TABLE bitwise_test(i BIGINT, j BIGINT)
# file: test/sql/function/operator/test_comparison.test
# query
SELECT 1 == 1, 1 = 1, 1 == 0, 1 = 0, 1 == NULL
SELECT 1 <> 1, 1 != 1, 1 <> 0, 1 != 0, 1 <> NULL
select '1000' > 20
select '1000' > '20'
select ('abc' between '20' and 'true')
# reject
select ('abc' between 20 and True)
select 'abc' > 10
select 20.0 = 'abc'
# file: test/sql/function/operator/test_conjunction.test
# setup
CREATE TABLE a (i integer, j integer)
# query
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
# file: test/sql/function/operator/test_date_arithmetic.test
# setup
CREATE TABLE dates(d DATE)
CREATE TABLE times(t TIME)
CREATE TABLE timetzs(ttz TIMETZ)
# query
INSERT INTO dates VALUES ('1992-01-01'), ('1992-03-03'), ('1992-05-05'), ('2022-01-01'), ('044-03-15 (BC)'), (NULL)
CREATE TABLE times(t TIME)
SELECT d, t, d + t FROM dates, times ORDER BY 1, 2
SELECT d, t, t + d FROM dates, times ORDER BY 1, 2
CREATE TABLE timetzs(ttz TIMETZ)
INSERT INTO timetzs VALUES ('00:01:20+00'), ('20:08:10.998-07'), ('20:08:10.33+12'), ('20:08:10.001-1559'), (NULL)
SELECT d, ttz, d + ttz FROM dates, timetzs ORDER BY 1, 2
SELECT d, ttz, ttz + d FROM dates, timetzs ORDER BY 1, 2
# reject
SELECT '294247-01-10'::DATE + '04:00:54.775808'::TIME
# file: test/sql/function/operator/test_division_overflow.test
# query
SELECT (-127)::TINYINT // (-1)::TINYINT
SELECT (-32767)::SMALLINT // (-1)::SMALLINT
SELECT (-2147483647)::INTEGER // (-1)::INTEGER
SELECT (-9223372036854775807)::BIGINT // (-1)::BIGINT
# reject
SELECT (-128)::TINYINT // (-1)::TINYINT
SELECT (-32768)::SMALLINT // (-1)::SMALLINT
SELECT (-2147483648)::INTEGER // (-1)::INTEGER
SELECT (-9223372036854775808)::BIGINT // (-1)::BIGINT
# file: test/sql/function/operator/test_in_empty_table.test
# setup
CREATE OR REPLACE TABLE test (a INTEGER)
# query
CREATE OR REPLACE TABLE test (a INTEGER)
SELECT * FROM test WHERE a IN ('a', 'b', 'c', 'd', 'e')
INSERT INTO test VALUES (42)
# file: test/sql/pivot/optional_pivots.test
# setup
CREATE TABLE Cities(Country VARCHAR, Name VARCHAR, Year INT, Population INT)
# query
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
# file: test/sql/pivot/pivot_15141.test
# setup
create table p (col1 timestamp, col2 int)
# query
create table p (col1 timestamp, col2 int)
INSERT INTO p VALUES ('2024-12-04 09:30:01', 100), ('2024-12-04 09:30:02', 100), ('2024-12-04 09:30:03', 100), ('2024-12-04 09:30:04', 100), ('2024-12-04 09:30:05', 100), ('2024-12-04 09:30:06', 100), ('2024-12-04 09:30:07', 100), ('2024-12-04 09:30:08', 100)
pivot p using sum (col2) group by col1 order by col1
# file: test/sql/pivot/pivot_6390.test
# setup
CREATE TABLE cpb_tbl AS WITH CPB(CPDH,NF,JG) AS ( SELECT 'C1',2022,10 UNION ALL SELECT 'C1',2018,20 UNION ALL SELECT 'C1',2017,0 UNION ALL SELECT 'C2',2022,10 UNION ALL SELECT 'C2',2010,30 UNION ALL SELECT 'C3',2010,80 ) FROM CPB
# query
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
# file: test/sql/pivot/pivot_bigquery.test
# setup
CREATE OR REPLACE TABLE Produce AS SELECT 'Kale' as product, 51 as Q1, 23 as Q2, 45 as Q3, 3 as Q4 UNION ALL SELECT 'Apple', 77, 0, 25, 2
# query
SELECT * FROM Produce PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3', 'Q4')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT sales, quarter FROM Produce) PIVOT(SUM(sales) FOR quarter IN ('Q1', 'Q2', 'Q3')) ORDER BY ALL
SELECT * FROM (SELECT product, sales, quarter FROM Produce) PIVOT(SUM(sales) total_sales, COUNT(*) num_records FOR quarter IN ('Q1', 'Q2')) ORDER BY ALL
CREATE OR REPLACE TABLE Produce AS SELECT 'Kale' as product, 51 as Q1, 23 as Q2, 45 as Q3, 3 as Q4 UNION ALL SELECT 'Apple', 77, 0, 25, 2
SELECT * FROM Produce UNPIVOT(sales FOR quarter IN (Q1, Q2, Q3, Q4)) ORDER BY ALL
SELECT product, first_half_sales, second_half_sales, semesters FROM Produce UNPIVOT( (first_half_sales, second_half_sales) FOR semesters IN ((Q1, Q2) AS 'semester_1', (Q3, Q4) AS 'semester_2'))
# file: test/sql/pivot/pivot_case_insensitive.test
# query
SET pivot_filter_threshold=1
FROM Cities PIVOT ( array_agg(id) FOR name IN ('test','Test') )
FROM Cities PIVOT ( array_agg(id), sum(id) FOR name IN ('test','Test') )
# file: test/sql/pivot/pivot_databricks.test
# setup
CREATE OR REPLACE TEMPORARY VIEW sales(location, year, q1, q2, q3, q4) AS VALUES ('Toronto' , 2020, 100 , 80 , 70, 150), ('San Francisco', 2020, NULL, 20 , 50, 60), ('Toronto' , 2021, 110 , 90 , 80, 170), ('San Francisco', 2021, 70 , 120, 85, 105)
CREATE OR REPLACE TEMPORARY VIEW oncall (year, week, area , name1 , email1 , phone1 , name2 , email2 , phone2) AS VALUES (2022, 1 , 'frontend', 'Freddy', 'fred@alwaysup.org' , 15551234567, 'Fanny' , 'fanny@lwaysup.org' , 15552345678), (2022, 1 , 'backend' , 'Boris' , 'boris@alwaysup.org', 15553456789, 'Boomer', 'boomer@lwaysup.org', 15554567890), (2022, 2 , 'frontend', 'Franky', 'frank@lwaysup.org' , 15555678901, 'Fin' , 'fin@alwaysup.org' , 15556789012), (2022, 2 , 'backend' , 'Bonny' , 'bonny@alwaysup.org', 15557890123, 'Bea' , 'bea@alwaysup.org' , 15558901234)
# query
SELECT year, region, q1, q2, q3, q4 FROM sales PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1, q2, q3, q4 FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT year, q1_total, q1_avg, q2_total, q2_avg, q3_total, q3_avg, q4_total, q4_avg FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales) AS total, avg(sales) AS avg FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
SELECT * FROM (SELECT year, quarter, sales FROM sales) AS s PIVOT (sum(sales), avg(sales) FOR quarter IN (1 AS q1, 2 AS q2, 3 AS q3, 4 AS q4))
CREATE OR REPLACE TEMPORARY VIEW sales(location, year, q1, q2, q3, q4) AS VALUES ('Toronto' , 2020, 100 , 80 , 70, 150), ('San Francisco', 2020, NULL, 20 , 50, 60), ('Toronto' , 2021, 110 , 90 , 80, 170), ('San Francisco', 2021, 70 , 120, 85, 105)
SELECT * FROM sales UNPIVOT INCLUDE NULLS (sales FOR quarter IN (q1 AS "Jan-Mar", q2 AS "Apr-Jun", q3 AS "Jul-Sep", q4 AS "Oct-Dec"))
SELECT * FROM oncall UNPIVOT ((name, email, phone) FOR precedence IN ((name1, email1, phone1) AS primary, (name2, email2, phone2) AS secondary))
# reject
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region, too_many_names) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east', 'west') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT * FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'east') AS q1_east_2))
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
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
INSERT INTO monthly_sales VALUES (1, 10000, 'JAN'), (1, 400, 'JAN'), (2, 4500, 'JAN'), (2, 35000, 'JAN'), (1, 5000, 'FEB'), (1, 3000, 'FEB'), (2, 200, 'FEB'), (2, 90500, 'FEB'), (1, 6000, 'MAR'), (1, 5000, 'MAR'), (2, 2500, 'MAR'), (2, 9500, 'MAR'), (1, 8000, 'APR'), (1, 10000, 'APR'), (2, 800, 'APR'), (2, 4500, 'APR')
CREATE TYPE unique_months AS ENUM (SELECT DISTINCT month FROM monthly_sales ORDER BY CASE month WHEN 'JAN' THEN 1 WHEN 'FEB' THEN 2 WHEN 'MAR' THEN 3 ELSE 4 END)
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_months) AS p ORDER BY EMPID
CREATE TYPE not_an_enum AS VARCHAR
# reject
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_monthsx) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN not_an_enum) AS p ORDER BY EMPID
# file: test/sql/pivot/pivot_errors.test
# setup
CREATE TABLE test(i INT, j VARCHAR)
# query
CREATE TABLE test(i INT, j VARCHAR)
SET pivot_filter_threshold=0
SET pivot_filter_threshold=100
# reject
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
# reject
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
CREATE TABLE Product(DaysToManufacture int, StandardCost int GENERATED ALWAYS AS (DaysToManufacture * 5))
INSERT INTO Product VALUES (0), (1), (2), (4)
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
# reject
PIVOT Cities ON Year IN (SELECT xx FROM Cities) USING SUM(Population)
# file: test/sql/pivot/pivot_operator_expression.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
INSERT INTO monthly_sales VALUES (1, 10000, '1-JAN'), (1, 400, '1-JAN'), (2, 4500, '1-JAN'), (2, 35000, '1-JAN'), (1, 5000, '2-FEB'), (1, 3000, '2-FEB'), (2, 200, '2-FEB'), (2, 90500, '2-FEB'), (2, 2500, '3-MAR'), (2, 9500, '3-MAR'), (1, 8000, '4-APR'), (1, 10000, '4-APR'), (2, 800, '4-APR'), (2, 4500, '4-APR')
PIVOT monthly_sales ON MONTH USING COALESCE(SUM(AMOUNT), 0)
SELECT mode(column_type) FROM (DESCRIBE PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)::INTEGER)
# file: test/sql/pivot/pivot_prepare.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
INSERT INTO monthly_sales VALUES (1, 10000, '1-JAN'), (1, 400, '1-JAN'), (2, 4500, '1-JAN'), (2, 35000, '1-JAN'), (1, 5000, '2-FEB'), (1, 3000, '2-FEB'), (2, 200, '2-FEB'), (2, 90500, '2-FEB'), (1, 6000, '3-MAR'), (1, 5000, '3-MAR'), (2, 2500, '3-MAR'), (2, 9500, '3-MAR'), (1, 8000, '4-APR'), (1, 10000, '4-APR'), (2, 800, '4-APR'), (2, 4500, '4-APR')
PREPARE v1 AS SELECT * FROM monthly_sales PIVOT(SUM(amount + ?) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR')) AS p ORDER BY EMPID
EXECUTE v1(0)
PREPARE v2 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT + ?)
# reject
PREPARE v3 AS PIVOT (SELECT empid, amount + ? AS amount, month FROM monthly_sales) ON MONTH USING SUM(AMOUNT)
# file: test/sql/pivot/pivot_query_text.test
# setup
CREATE TABLE t(c VARCHAR, v INTEGER)
CREATE TABLE captured AS PIVOT (SELECT c, v, current_query() AS q FROM t) ON c USING ANY_VALUE(q) ORDER BY v
# query
CREATE TABLE t(c VARCHAR, v INTEGER)
INSERT INTO t VALUES ('a', 1), ('b', 2)
CREATE TABLE captured AS PIVOT (SELECT c, v, current_query() AS q FROM t) ON c USING ANY_VALUE(q) ORDER BY v
SELECT v, regexp_replace(a, '__pivot_enum_[0-9a-f\-]+', '__pivot_enum_X') AS a, regexp_replace(b, '__pivot_enum_[0-9a-f\-]+', '__pivot_enum_X') AS b FROM captured ORDER BY v
# file: test/sql/pivot/pivot_star.test
# setup
CREATE TABLE t(id INT, jan INT, feb INT)
CREATE VIEW v AS PIVOT t ON id IN (CASE WHEN true THEN 'a' END) USING (SUM(feb))
CREATE VIEW poison_view AS SELECT * FROM t UNPIVOT (val FOR col IN (*))
CREATE VIEW expr_view AS SELECT * FROM t UNPIVOT (val FOR col IN (1+2+id))
# query
CREATE TABLE t(id INT, jan INT, feb INT)
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
CREATE OR REPLACE TABLE sales(empid INT, amount INT, d DATE)
INSERT INTO sales VALUES (1, 10000, DATE '2000-01-01'), (1, 400, DATE '2000-01-07'), (2, 4500, DATE '2001-01-21'), (2, 35000, DATE '2001-01-21'), (1, 5000, DATE '2000-02-03'), (1, 3000, DATE '2000-02-07'), (2, 200, DATE '2001-02-05'), (2, 90500, DATE '2001-02-19'), (1, 6000, DATE '2000-03-01'), (1, 5000, DATE '2000-03-09'), (2, 2500, DATE '2001-03-03'), (2, 9500, DATE '2001-03-08')
PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT) ORDER BY ALL
PIVOT (PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)) ON empid USING SUM(COALESCE("2000_1",0) + COALESCE("2000_2",0) + COALESCE("2000_3",0) + COALESCE("2001_1",0) + COALESCE("2001_2",0) + COALESCE("2001_3",0))
# reject
CREATE VIEW pivot_view AS PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)
CREATE MACRO xt2(a) as TABLE PIVOT sales ON d USING SUM(amount)
CREATE MACRO xt2(a) as (PIVOT sales ON d USING SUM(amount))
# file: test/sql/pivot/test_multi_pivot.test
# setup
CREATE OR REPLACE TABLE sales(empid INT, amount INT, month TEXT, year INT)
# query
CREATE OR REPLACE TABLE sales(empid INT, amount INT, month TEXT, year INT)
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SELECT * FROM sales PIVOT( SUM(amount + year) FOR YEAR IN (2020, 2021) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') ) AS p ORDER BY EMPID
SET pivot_limit=10000
# reject
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') amount IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) empid IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) ) AS p ORDER BY EMPID
# file: test/sql/pivot/test_pivot.test
# setup
CREATE TABLE Product(DaysToManufacture int, StandardCost int)
CREATE OR REPLACE TABLE monthly_sales(empid INT, amount INT, month TEXT)
# query
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
# reject
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'JAN')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(COS(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + (SELECT 42)) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + row_number() over ()) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTHx IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ()) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (*)) AS p ORDER BY EMPID
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
# file: test/sql/pivot/test_unpivot.test
# setup
CREATE OR REPLACE TABLE monthly_sales(empid INT, dept TEXT, Jan INT, Feb INT, Mar INT, April INT)
# query
CREATE OR REPLACE TABLE monthly_sales(empid INT, dept TEXT, Jan INT, Feb INT, Mar INT, April INT)
INSERT INTO monthly_sales VALUES (1, 'electronics', 100, 200, 300, 100), (2, 'clothes', 100, 300, 150, 200), (3, 'cars', 200, 400, 100, 50)
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) ORDER BY empid
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (jan AS January, feb AS February, mar AS March, april)) ORDER BY empid
SELECT p.id, p.type, p.m, p.vals FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, april)) AS p(id, type, m, vals)
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), (mar, april)))
UNPIVOT (SELECT * FROM monthly_sales) ON jan, feb, mar april INTO NAME month VALUE sales
# reject
SELECT * FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR (month, month2) IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT(sales_jan_feb FOR month IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT((a, b, c) FOR month IN ((jan, feb), (mar, april)))
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), mar))
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, dec)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (empid, dept, jan, feb, mar, april))
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ()) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ('')) ORDER BY empid
# file: test/sql/pivot/test_unpivot_stmt.test
# setup
CREATE TABLE t1(id BIGINT, "Sales (05/19/2020)" BIGINT, "Sales (06/03/2020)" BIGINT, "Sales (10/23/2020)" BIGINT)
# query
CREATE TABLE t1(id BIGINT, "Sales (05/19/2020)" BIGINT, "Sales (06/03/2020)" BIGINT, "Sales (10/23/2020)" BIGINT)
INSERT INTO t1 VALUES(10629465, 23, 47, 99)
INSERT INTO t1 VALUES(98765432, 10, 99, 33)
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
ALTER TABLE monthly_sales ADD COLUMN status VARCHAR
UPDATE monthly_sales SET status=CASE WHEN amount >= 10000 THEN 'important' ELSE 'regular' END
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)) ORDER BY ALL
PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid ORDER BY ALL
FROM (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY status) ORDER BY ALL
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('1-JAN', '2-FEB', '3-MAR', '4-APR') GROUP BY status) AS p ORDER BY 1
WITH pivoted_sales AS (PIVOT monthly_sales ON MONTH USING SUM(AMOUNT) GROUP BY empid) SELECT * FROM pivoted_sales ORDER BY empid DESC
# reject
CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)
# file: test/sql/pivot/unpivot_expression.test
# query
unpivot (select 42 as col1, 'woot' as col2) on col1::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on COLUMNS(*)::VARCHAR
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on (col1 + 100)::VARCHAR AS c, col2
select * from (select 42 as col1, 'woot' as col2) UNPIVOT ("value" FOR "name" IN (col1::VARCHAR, col2))
# reject
unpivot (select 42 as col1, 'woot' as col2) on (col1 + (SELECT col1))::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on random(), col2
unpivot (select 42 as col1, 'woot' as col2) on col1 + col2
unpivot (select 42 as col1, 'woot' as col2) on t.col1::VARCHAR, col2
# file: test/sql/pivot/unpivot_internal_names.test
# setup
CREATE TABLE unpivot_names(unpivot_names VARCHAR, unpivot_list VARCHAR, unpivot_list_2 VARCHAR, col1 INT, col2 INT, col3 INT)
# query
CREATE TABLE unpivot_names(unpivot_names VARCHAR, unpivot_list VARCHAR, unpivot_list_2 VARCHAR, col1 INT, col2 INT, col3 INT)
INSERT INTO unpivot_names VALUES ('unpivot_names', 'unpivot_list', 'unpivot_list_2', 1, 2, 3)
UNPIVOT unpivot_names ON COLUMNS('col*')
# file: test/sql/pivot/unpivot_no_columns.test
# setup
create table integers(i integer)
# query
create table integers(i integer)
# reject
unpivot integers on columns(* exclude (i))
# file: test/sql/pivot/unpivot_non_aligned_columns.test
# setup
CREATE TABLE test(id BIGINT, metric_1 VARCHAR, value_x VARCHAR, metric_2 VARCHAR, value_q VARCHAR, metric_3 VARCHAR, value_j VARCHAR)
# query
CREATE TABLE test(id BIGINT, metric_1 VARCHAR, value_x VARCHAR, metric_2 VARCHAR, value_q VARCHAR, metric_3 VARCHAR, value_j VARCHAR)
INSERT INTO test VALUES(1,'a','a_value','b','b_value','c','c_value')
INSERT INTO test VALUES(2,'d','d_value','e','e_value','f','f_value')
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUES metric_value, metric_type
# reject
UNPIVOT test ON (metric_1, value_x), metric_2, metric_3
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUE metric_value
# file: test/sql/pivot/unpivot_types.test
# query
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select 42) on columns(*))
SELECT column_name, column_type FROM (DESCRIBE unpivot ( select {n : 1 }) on columns(*))
# file: test/sql/pivot/unpivot_unnamed_subquery.test
# query
unpivot (select cast(columns(*) as varchar) from (select 42 as col1, 'woot' as col2)) on columns(*)
# file: test/sql/pivot/unpivot_view.test
# setup
CREATE TABLE t ( id INTEGER, a INTEGER, b INTEGER )
CREATE VIEW v AS SELECT * FROM t UNPIVOT (val FOR key IN (a, b))
# query
CREATE TABLE t ( id INTEGER, a INTEGER, b INTEGER )
INSERT INTO t VALUES (1, 10, 20)
CREATE VIEW v AS SELECT * FROM t UNPIVOT (val FOR key IN (a, b))
FROM v
# file: test/sql/prepared/invalid_prepare.test
# query
prepare v1 as select $2::int
prepare v2 as select $1::int
prepare v3 as select $1::int where 1=0
execute v3(1)
# reject
select ?
create view v1 as select ?
execute v1(0)
execute v2('hello')
# file: test/sql/prepared/parameter_order_subquery.test
# setup
CREATE TABLE t1(c0 INT2)
# query
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
# file: test/sql/prepared/parameter_variants.test
# query
PREPARE s1 AS SELECT CAST(? AS INTEGER), CAST(? AS STRING)
EXECUTE s1(42, 'dpfkg')
DEALLOCATE s1
PREPARE s1 AS SELECT CAST(?1 AS INTEGER), CAST(?2 AS STRING)
PREPARE s1 AS SELECT CAST(?2 AS INTEGER), CAST(?1 AS STRING)
EXECUTE s1('dpfkg', 42)
# file: test/sql/prepared/prepare_copy.test
# query
execute q1 (42)
PREPARE q2 AS COPY ( select 42 as 'col' ) to $1 ( FORMAT csv )
# file: test/sql/prepared/prepare_default_varchar.test
# query
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
# reject
EXECUTE v2([[1, 2, 3]], [1, 2, 3])
EXECUTE v7('hello world', [1, 2, 3])
# file: test/sql/prepared/prepare_from_first.test
# query
prepare fromFirst as from (select ? fromV) select ? selectV,*
execute fromFirst('from', 'sel')
from (select 'from' fromV) select 'sel' selectV,*
# file: test/sql/prepared/prepare_in.test
# setup
create table test(id varchar)
# query
create table test(id varchar)
prepare p as delete from test where ("id") in ((?))
execute p(null)
# file: test/sql/prepared/prepare_lambda.test
# query
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
# file: test/sql/prepared/prepare_list_functions.test
# query
PREPARE v1 AS SELECT list_aggregate(?, 'min')
EXECUTE v1(['hello', 'world'])
EXECUTE v1(NULL::INT[])
PREPARE v2 AS SELECT array_slice(?, 1, 2)
EXECUTE v2([1, 2, 3])
EXECUTE v2('123')
PREPARE v3 AS SELECT flatten(?)
EXECUTE v3([[1,2,3],[4,5]])
PREPARE v4 AS SELECT list_extract(?, 2)
# file: test/sql/prepared/prepare_maintain_types.test
# query
prepare v1 as select cast(111 as short) * $1
execute v1(1665::BIGINT)
# reject
execute v1(1665)
execute v1('1665')
execute v1('1665'::VARCHAR)
execute v1(1665::SHORT)
# file: test/sql/prepared/prepare_mixed_types.test
# query
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
# reject
EXECUTE v3(-1)
# file: test/sql/prepared/prepare_offset_first.test
# query
PREPARE q AS SELECT x FROM generate_series(1, 10) t(x) OFFSET ? LIMIT ?
EXECUTE q(3, 5)
SELECT x FROM generate_series(1, 10) t(x) OFFSET 3 LIMIT 5
# file: test/sql/prepared/prepare_summarize.test
# setup
CREATE TABLE accounts AS SELECT 1 id, 'Mark' AS name
# query
CREATE TABLE accounts AS SELECT 1 id, 'Mark' AS name
SUMMARIZE SELECT * FROM accounts WHERE id = 1
PREPARE query AS SUMMARIZE SELECT * FROM accounts WHERE id = $1
EXECUTE query(1)
PREPARE query AS (SUMMARIZE SELECT * FROM accounts WHERE id = $1)
DESCRIBE SELECT * FROM accounts WHERE id = 1
PREPARE query AS DESCRIBE SELECT * FROM accounts WHERE id = $1
PREPARE query AS (DESCRIBE SELECT * FROM accounts WHERE id = $1)
# file: test/sql/prepared/prepare_window_functions.test
# setup
CREATE TABLE v0 ( v2 INTEGER CHECK( v2 BETWEEN 1 AND 1119 ) , v1 INT )
# query
PREPARE v1 AS SELECT SUM(?) OVER ()
EXECUTE v1(2::HUGEINT)
EXECUTE v1(0.5)
CREATE TABLE v0 ( v2 INTEGER CHECK( v2 BETWEEN 1 AND 1119 ) , v1 INT )
INSERT INTO v0 ( v2 ) VALUES ( 10 )
PREPARE q1 AS SELECT COALESCE ( LEAD ( $1 ) OVER( ) , ( v1 ) ) > $1 FROM v0
EXECUTE q1(1)
# file: test/sql/prepared/prepared_named_param.test
# query
prepare q123 as select $param, $other_name, $param
execute q123(param := 5, other_name := 3)
prepare q01 as select $1, ?, $2
# reject
execute q123(param := 5, 3)
execute q01(4, 2, 0)
prepare q02 as select $1, $param, $2
execute q01(a, 2, 0)
# file: test/sql/prepared/prepared_null_binding.test
# query
PREPARE v1 AS SELECT COALESCE(COALESCE(NULL, $1) / 42::BIGINT, 0.5)
PREPARE v2 AS SELECT COALESCE(CASE WHEN FALSE THEN $1 ELSE NULL END / 42::BIGINT, 0.5)
# reject
EXECUTE v1(INTERVAL '1' DAY)
EXECUTE v2(INTERVAL '1' DAY)
# file: test/sql/prepared/prepared_select.test
# query
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
# file: test/sql/prepared/prepared_update.test
# setup
CREATE TABLE integers(i INTEGER, j VARCHAR)
# query
CREATE TABLE integers(i INTEGER, j VARCHAR)
INSERT INTO integers VALUES (1, 'hello')
PREPARE s1 AS UPDATE integers SET i=?, j=?
EXECUTE s1(2, 'world')
PREPARE s2 AS UPDATE integers SET j=? WHERE i=?
EXECUTE s2('test', 2)
PREPARE s3 AS UPDATE integers SET j=? WHERE i=? AND j=?
EXECUTE s3('test2', 2, 'test')
# file: test/sql/prepared/test_basic_prepare.test
# query
PREPARE s1 AS SELECT CAST($1 AS INTEGER), CAST($2 AS STRING)
EXECUTE s1(43, 'asdf')
DEALLOCATE s2
PREPARE s1 AS SELECT $1+$2
PREPARE s1 AS SELECT NOT($1), 10+$2, $3+20, 4 IN (2, 3, $4), $5 IN (2, 3, 4)
EXECUTE s1(1, 2, 3, 4, 2)
PREPARE s1 AS SELECT $1
PREPARE s2 AS SELECT (SELECT $1)
PREPARE s3 AS SELECT $1=$2
# reject
EXECUTE s1(43)
EXECUTE s1(43, 'asdf', 42)
EXECUTE s1('asdf', 'asdf')
PREPARE EXPLAIN SELECT 42
PREPARE CREATE TABLE a(i INTEGER)
# file: test/sql/prepared/test_issue_2079.test
# query
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
# file: test/sql/prepared/test_issue_4010.test
# setup
CREATE table T1(A0 TIMESTAMP, A1 INTEGER, A2 VARCHAR, A3 VARCHAR, A4 INTEGER, A5 DOUBLE)
# query
CREATE table T1(A0 TIMESTAMP, A1 INTEGER, A2 VARCHAR, A3 VARCHAR, A4 INTEGER, A5 DOUBLE)
PREPARE v1 AS SELECT (SUM(CASE WHEN ((T1.A2 = ($1)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END) / ((SUM(CASE WHEN ((T1.A2 = ($1)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END) + SUM(CASE WHEN ((T1.A2 = ($2)::text) AND (T1.A3 = ($1)::text)) THEN T1.A4 ELSE (0)::int END)))::float8) AS A00036933 FROM T1
# file: test/sql/prepared/test_issue_4042.test
# setup
CREATE TABLE stringliterals AS SELECT 1 AS ID, 1::BIGINT AS a1,'value-1' AS a2,'value-1' AS a3,10::BIGINT AS a4
# query
CREATE TABLE stringliterals AS SELECT 1 AS ID, 1::BIGINT AS a1,'value-1' AS a2,'value-1' AS a3,10::BIGINT AS a4
EXECUTE v1('value-1', 'value-2')
# file: test/sql/prepared/test_issue_6276.test
# query
PREPARE v1 AS SELECT CASE ? WHEN ? THEN ? WHEN ? THEN ? ELSE ? END AS x
EXECUTE V1(1, 2, 3, 4, 5, 6)
# file: test/sql/prepared/test_prepare_ambiguous_type.test
# query
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
# reject
EXECUTE v5(4)
EXECUTE v6('hello')
EXECUTE v18([1])
EXECUTE v19(0)
# file: test/sql/prepared/test_prepare_delete.test
# setup
CREATE TABLE b (i TINYINT)
# query
CREATE TABLE b (i TINYINT)
INSERT INTO b VALUES (1), (2), (3), (4), (5)
PREPARE s1 AS DELETE FROM b WHERE i=$1
SELECT * FROM b ORDER BY 1
EXECUTE s1(3)
DROP TABLE b CASCADE
# file: test/sql/prepared/test_prepare_delete_update.test
# setup
CREATE TABLE b (i TINYINT)
# query
PREPARE s1 AS UPDATE b SET i=$1 WHERE i=$2
EXECUTE s1(6, 3)
# file: test/sql/prepared/test_prepare_drop.test
# setup
CREATE TABLE a (i TINYINT)
# query
CREATE TABLE a (i TINYINT)
PREPARE p1 AS SELECT * FROM a
# file: test/sql/prepared/test_prepare_error.test
# query
PREPARE v1 AS SELECT ?::VARCHAR::INT
EXECUTE v1('3')
# file: test/sql/prepared/test_prepare_issue_5132.test
# setup
create table test as select 42 i
# query
create table test as select 42 i
prepare q1 as SELECT cast(? AS VARCHAR) FROM test
execute q1('oops')
# file: test/sql/prepared/test_prepare_issue_8500.test
# query
PREPARE S1 AS SELECT (? / 1) + 1
EXECUTE S1(42)
# file: test/sql/prepared/test_prepare_null.test
# setup
CREATE TABLE b (i TINYINT)
# query
PREPARE s1 AS INSERT INTO b VALUES ($1)
EXECUTE s1 (NULL)
SELECT i FROM b
PREPARE s2 AS UPDATE b SET i=$1
EXECUTE s2 (NULL)
PREPARE s3 AS DELETE FROM b WHERE i=$1
EXECUTE s3 (NULL)
# file: test/sql/prepared/test_prepare_select.test
# setup
CREATE TABLE a (i TINYINT)
# query
PREPARE s3 AS SELECT * FROM a WHERE i=$1
EXECUTE s3(10000)
EXECUTE s3(42)
EXECUTE s3(84)
DEALLOCATE s3
PREPARE s1 AS SELECT to_years($1), CAST(list_value($1) AS BIGINT[])
EXECUTE s1(1)
# reject
SELECT * FROM a WHERE i=$1
SELECT * FROM a WHERE i=CAST($1 AS VARCHAR)
# file: test/sql/prepared/test_prepare_subquery.test
# query
PREPARE v1 AS SELECT * FROM (SELECT $1::INTEGER) sq1
PREPARE v2 AS SELECT * FROM (SELECT $1::INTEGER WHERE 1=0) sq1
PREPARE v3 AS SELECT (SELECT $1::INT+sq1.i) FROM (SELECT 42 AS i) sq1
PREPARE v4 AS SELECT (SELECT (SELECT $1::INT+sq1.i)+$2::INT+sq1.i) FROM (SELECT 42 AS i) sq1
EXECUTE v4(20, 20)
# file: test/sql/prepared/test_prepare_types.test
# setup
CREATE TABLE test(a TINYINT, b SMALLINT, c INTEGER, d BIGINT, e REAL, f DOUBLE, g DATE, h VARCHAR)
# query
CREATE TABLE test(a TINYINT, b SMALLINT, c INTEGER, d BIGINT, e REAL, f DOUBLE, g DATE, h VARCHAR)
PREPARE s1 AS INSERT INTO test VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
EXECUTE s1(1,2,3,4,1.5,2.5,'1992-10-20', 'hello world')
# file: test/sql/prepared/test_prepare_unused_cte.test
# setup
CREATE TABLE "user" (name string)
# query
CREATE TABLE "user" (name string)
PREPARE s2965 AS WITH temp_first AS ( SELECT * FROM "user" WHERE "name" = ? ), temp_second AS ( SELECT * FROM "user" WHERE "name" = ? ) SELECT * FROM temp_second
EXECUTE s2965('val1', 'val2')
DEALLOCATE s2965
# file: test/sql/optimizer/predicate_factoring.test
# setup
CREATE TABLE t (a INTEGER, b INTEGER, c INTEGER)
CREATE TABLE s (x INTEGER, y INTEGER)
# query
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
# file: test/sql/optimizer/test_common_subplan_cte_binding_order.test
# setup
CREATE TABLE orders (id INT, amount INT, status VARCHAR, created_at TIMESTAMP)
CREATE TABLE line_items (id INT, order_id INT, sku VARCHAR, extracted_at TIMESTAMP)
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
CREATE VIEW order_lifecycle AS WITH sku_agg AS ( SELECT order_id, sum(CASE WHEN sku = 'WIDGET' THEN 1 ELSE 0 END) AS widget_count FROM line_items_deduped GROUP BY order_id ) SELECT o.amount, CASE WHEN COALESCE(s.widget_count, 0) > 0 THEN 'widget_order' ELSE 'other' END AS order_type, (o.status != 'refunded') AS is_net_order FROM orders_deduped o LEFT JOIN sku_agg s ON o.id = s.order_id
# query
CREATE TABLE orders (id INT, amount INT, status VARCHAR, created_at TIMESTAMP)
CREATE TABLE line_items (id INT, order_id INT, sku VARCHAR, extracted_at TIMESTAMP)
INSERT INTO orders VALUES (1, 50, 'paid', '2025-01-01'), (2, 75, 'paid', '2025-01-02'), (3, 30, 'refunded', '2025-01-03')
INSERT INTO line_items VALUES (1, 1, 'WIDGET', '2025-01-01'), (2, 2, 'GADGET', '2025-01-02'), (3, 3, 'WIDGET', '2025-01-03')
CREATE VIEW orders_deduped AS SELECT id, amount, status FROM orders QUALIFY row_number() OVER (PARTITION BY id ORDER BY created_at DESC) = 1
CREATE VIEW line_items_deduped AS SELECT order_id, sku FROM line_items QUALIFY row_number() OVER (PARTITION BY id ORDER BY extracted_at DESC) = 1
CREATE VIEW order_lifecycle AS WITH sku_agg AS ( SELECT order_id, sum(CASE WHEN sku = 'WIDGET' THEN 1 ELSE 0 END) AS widget_count FROM line_items_deduped GROUP BY order_id ) SELECT o.amount, CASE WHEN COALESCE(s.widget_count, 0) > 0 THEN 'widget_order' ELSE 'other' END AS order_type, (o.status != 'refunded') AS is_net_order FROM orders_deduped o LEFT JOIN sku_agg s ON o.id = s.order_id
# file: test/sql/optimizer/test_duplicate_groups_optimizer.test
# setup
create table t1(col1 int, col2 int)
create table t2(col3 int)
create table t3 (a int, b int, c int)
# query
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
# file: test/sql/optimizer/test_in_rewrite_rule.test
# setup
create table t (i integer)
# query
create table t (i integer)
insert into t values (1)
insert into t values (2)
select * from t where i in ('1','2','y')
SELECT x::VARCHAR IN ('1', y::VARCHAR) FROM (VALUES (1, 2), (2, 3)) tbl(x, y)
SELECT x::BIGINT IN (1::BIGINT, y) FROM (VALUES (1::INTEGER, 2::BIGINT), (2::INTEGER, 3::BIGINT)) tbl(x, y)
# file: test/sql/optimizer/test_no_pushdown_cast_into_cte.test
# setup
create or replace table mytablename2 as from (values ('a0'), ('a1'), ('a2'), ('xxx-0'), ('xxx-1'), ('xxx-2'), ('xxx-3'), ('xxxx'), ('xxx0'), ('xxx1'), ('xxx2'), ('xxx3') ) t(mycolname), range(4300) b(someothercolname)
# query
WITH t(a, b) AS ( SELECT a :: int, b :: int FROM (VALUES ('1', '4'), ('5', '3'), ('2', '*'), ('3', '8'), ('7', '*')) AS _(a, b) WHERE position('*' in b) = 0 ) SELECT a, b FROM t WHERE a < b
EXPLAIN WITH t(a, b) AS ( SELECT a :: int, b :: int FROM (VALUES ('1', '4'), ('5', '3'), ('2', '*'), ('3', '8'), ('7', '*')) AS _(a, b) WHERE position('*' in b) = 0 ) SELECT a, b FROM t WHERE a < b
with t(a, b) as ( select a :: varchar, b :: varchar FROM VALUES (1, 2), (3, 3), (5, 6), (7, 6) as _(a, b) where a <= b ) select a, b from t where a[1] = '1'
explain with t(a, b) as ( select a :: varchar, b :: varchar FROM VALUES (1, 2), (3, 3), (5, 6), (7, 6) as _(a, b) where a <= b ) select a, b from t where a[1] = '1'
create or replace table mytablename2 as from (values ('a0'), ('a1'), ('a2'), ('xxx-0'), ('xxx-1'), ('xxx-2'), ('xxx-3'), ('xxxx'), ('xxx0'), ('xxx1'), ('xxx2'), ('xxx3') ) t(mycolname), range(4300) b(someothercolname)
select mycolname[2:]::int as mycolname2 from mytablename2 where mycolname[1:3] != 'xxx' AND mycolname2 = 0 limit 5
# file: test/sql/optimizer/test_rollup_column_pruning.test
# setup
CREATE TABLE events( col1 INT, col2 INT, col3 INT, unused1 INT, unused2 INT, unused3 INT )
CREATE TABLE t2(col4 INT)
# query
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
# file: test/sql/optimizer/test_rowid_pushdown.test
# setup
CREATE TABLE t1 AS SELECT i + 100 as x FROM range(250000) AS t(i)
# query
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
# file: test/sql/optimizer/test_rowid_pushdown_deletes.test
# setup
CREATE TABLE tbl_grow_shrink (id_var VARCHAR, id_int INTEGER, id_point BIGINT)
# query
CREATE TABLE tbl_grow_shrink (id_var VARCHAR, id_int INTEGER, id_point BIGINT)
DELETE FROM tbl_grow_shrink WHERE rowid = (SELECT min(rowid) FROM tbl_grow_shrink)
# file: test/sql/optimizer/test_rowid_pushdown_plan.test
# query
SELECT * FROM lineitem ORDER BY l_orderkey DESC LIMIT 5
SELECT * FROM lineitem WHERE rowid IN (SELECT rowid FROM lineitem ORDER BY l_orderkey DESC LIMIT 5)
SELECT * FROM lineitem WHERE l_orderkey % 20000 == 0
SELECT * FROM lineitem WHERE rowid IN (SELECT rowid FROM lineitem WHERE l_orderkey % 20000 == 0)
EXPLAIN SELECT * FROM lineitem WHERE rowid = 20058
# file: test/sql/optimizer/test_window_self_join.test
# setup
CREATE TABLE services (date DATE, train_number INT)
CREATE TABLE items (id INT, category VARCHAR)
CREATE TABLE null_partition_test (id INT, category VARCHAR)
CREATE OR REPLACE TABLE foo ( emp_name VARCHAR, dept_name VARCHAR, base_salary FLOAT )
CREATE OR REPLACE TABLE t(id INTEGER, x DOUBLE)
# query
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
# file: test/sql/optimizer/tests_no_pushdown_under_samples.test
# setup
CREATE OR REPLACE TABLE df AS (SELECT * AS i FROM range(10))
CREATE OR REPLACE TABLE wtf AS (SELECT 1 AS i)
# query
CREATE OR REPLACE TABLE df AS (SELECT * AS i FROM range(10))
CREATE OR REPLACE TABLE wtf AS (SELECT 1 AS i)
explain FROM df, wtf SELECT df.i WHERE df.i > 8 USING SAMPLE 1
# file: test/sql/optimizer/topn_window_set_elimination.test
# setup
CREATE or replace TABLE timeseries AS FROM ( VALUES (timestamp '2026-03-25 05:33:11.822+08', 10), (timestamp '2026-03-26 05:33:11.822+08', 15), (timestamp '2026-03-27 05:33:11.822+08', 12), (timestamp '2026-03-28 05:33:11.822+08', 18), (timestamp '2026-03-29 05:33:11.822+08', 14) ) AS t(date, value)
CREATE TABLE t2 (a VARCHAR, b BOOLEAN, c VARCHAR)
CREATE OR REPLACE MACRO nextValue(time_serie, ts_col, value_col, ts) AS TABLE ( (SELECT ts_col, value_col FROM query_table(time_serie) WHERE ts_col >= ts order by ts_col limit 1) union select ts AS ts_col, (select value_col from query_table(time_serie) order by ts_col desc limit 1) AS value_col WHERE NOT EXISTS (FROM query_table(time_serie) WHERE ts_col >= ts) )
# query
CREATE or replace TABLE timeseries AS FROM ( VALUES (timestamp '2026-03-25 05:33:11.822+08', 10), (timestamp '2026-03-26 05:33:11.822+08', 15), (timestamp '2026-03-27 05:33:11.822+08', 12), (timestamp '2026-03-28 05:33:11.822+08', 18), (timestamp '2026-03-29 05:33:11.822+08', 14) ) AS t(date, value)
CREATE OR REPLACE MACRO nextValue(time_serie, ts_col, value_col, ts) AS TABLE ( (SELECT ts_col, value_col FROM query_table(time_serie) WHERE ts_col >= ts order by ts_col limit 1) union select ts AS ts_col, (select value_col from query_table(time_serie) order by ts_col desc limit 1) AS value_col WHERE NOT EXISTS (FROM query_table(time_serie) WHERE ts_col >= ts) )
from range(1,5) as t(days), nextValue(timeseries, date, value, '2026-03-29 05:33:11.822+08'::timestamp - INTERVAL (days) DAY) limit 5
CREATE TABLE t2 (a VARCHAR, b BOOLEAN, c VARCHAR)
INSERT INTO t2 VALUES ('x', false, '2024-01-01')
WITH cte AS ( SELECT a, b, c::TIMESTAMPTZ AS c FROM t2 ) SELECT * FROM cte QUALIFY ROW_NUMBER() OVER (PARTITION BY a ORDER BY c DESC) = 1
# file: test/sql/optimizer/plan/plan_struct_projection_pushdown.test
# setup
CREATE TABLE struct_pushdown_test(id INT, struct_col STRUCT(sub_col1 integer, sub_col2 bool))
CREATE OR REPLACE TABLE nested_struct_pushdown_test(id INT, struct_col STRUCT(s STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool))))
# query
CREATE TABLE struct_pushdown_test(id INT, struct_col STRUCT(sub_col1 integer, sub_col2 bool))
INSERT INTO struct_pushdown_test VALUES (1, {'sub_col1': 42, 'sub_col2': true}), (2, NULL), (3, {'sub_col1': 84, 'sub_col2': NULL}), (4, {'sub_col1': NULL, 'sub_col2': false})
PRAGMA explain_output = 'PHYSICAL_ONLY'
CREATE TABLE nested_struct_pushdown_test(id INT, struct_col STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool)))
INSERT INTO nested_struct_pushdown_test VALUES (1, {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}), (2, NULL), (3, {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}), (4, {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}})
CREATE OR REPLACE TABLE nested_struct_pushdown_test(id INT, struct_col STRUCT(s STRUCT(name STRUCT(v VARCHAR, id INT), nested_struct STRUCT(a integer, b bool))))
INSERT INTO nested_struct_pushdown_test VALUES (1, {'s': {'name': {'v': 'Row 1', 'id': 1}, 'nested_struct': {'a': 42, 'b': true}}}), (2, NULL), (3, {'s': {'name': {'v': 'Row 3', 'id': 3}, 'nested_struct': {'a': 84, 'b': NULL}}}), (4, {'s': {'name': NULL, 'nested_struct': {'a': NULL, 'b': false}}})
# file: test/sql/optimizer/plan/test_anti_join_empty_child.test
# query
pragma explain_output='OPTIMIZED_ONLY'
SELECT lhs.id FROM (SELECT 1 id) lhs ANTI JOIN (SELECT 1 id WHERE FALSE) rhs ON lhs.id = rhs.id
EXPLAIN SELECT lhs.id FROM (SELECT 1 id) lhs ANTI JOIN (SELECT 1 id WHERE FALSE) rhs ON lhs.id = rhs.id
# file: test/sql/optimizer/plan/test_disable_build_side_probe_side.test
# query
set disabled_optimizers to 'build_side_probe_side'
explain from range(10) r1 right join range(10) r2 using (range)
# file: test/sql/optimizer/plan/test_except_empty_rhs.test
# query
SELECT * FROM (SELECT 1) lhs EXCEPT SELECT * FROM (SELECT 1 WHERE 1=0) rhs
EXPLAIN SELECT * FROM (SELECT 1) lhs EXCEPT SELECT * FROM (SELECT 1 WHERE 1=0) rhs
SELECT * FROM (SELECT 42 EXCEPT SELECT 43) tbl(i) WHERE i = 42
# file: test/sql/optimizer/plan/test_filter_pushdown.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE cohort ( person_id INTEGER, cohort_start_date DATE, cohort_end_date DATE )
CREATE TABLE obs ( person_id INTEGER, observation_period_start_date DATE )
CREATE TABLE t0(c0 VARCHAR(500))
# query
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
# file: test/sql/optimizer/plan/test_filter_pushdown_duplicate.test
# setup
CREATE TABLE vals1 AS SELECT i AS i, i AS j FROM range(0, 10000, 1) t1(i)
CREATE TABLE vals2(k INTEGER, l INTEGER)
# query
CREATE TABLE vals1 AS SELECT i AS i, i AS j FROM range(0, 10000, 1) t1(i)
CREATE TABLE vals2(k INTEGER, l INTEGER)
INSERT INTO vals2 SELECT * FROM vals1
# file: test/sql/optimizer/plan/test_filter_pushdown_large.test
# setup
CREATE TABLE vals1 AS SELECT i AS i, i AS j FROM range(0, 10000, 1) t1(i)
CREATE TABLE vals2(k INTEGER, l INTEGER)
# query
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
# file: test/sql/optimizer/plan/test_filter_pushdown_materialized_cte.test
# query
call dsdgen(sf=0.01)
# file: test/sql/optimizer/plan/test_table_filter_pushdown.test
# setup
CREATE TABLE integers AS SELECT i AS i, i AS j FROM range(0, 100) tbl(i)
# query
CREATE TABLE integers AS SELECT i AS i, i AS j FROM range(0, 100) tbl(i)
SELECT j FROM integers where j = 99
SELECT j FROM integers where j = 99 AND i=99
SELECT j FROM integers where j = 99 AND i=90
SELECT count(i) FROM integers where j > 90 and i < 95
SELECT count(i) FROM integers where j > 90 and j < 95
# file: test/sql/optimizer/plan/test_unused_column_after_join.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
CREATE TABLE test2 (b INTEGER, c INTEGER)
# query
CREATE TABLE test2 (b INTEGER, c INTEGER)
INSERT INTO test2 VALUES (1, 10), (1, 20), (2, 30)
SELECT COUNT(*) FROM test, test2 WHERE test.b = test2.b
SELECT SUM(test.a), MIN(test.a), MAX(test.a) FROM test, test2 WHERE test.b = test2.b
SELECT COUNT(*) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b
SELECT SUM(a1.a) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b
SELECT COUNT(*) FROM test a1, test a2, test a3 WHERE a1.b=a2.b AND a2.b=a3.b AND a1.a=11 AND a2.a=11 AND a3.a=11
SELECT (TRUE OR a1.a=a2.b) FROM test a1, test a2 WHERE a1.a=11 AND a2.a>=10
# file: test/sql/optimizer/expression/test_casting_negative_integer_to_bit.test
# setup
CREATE TABLE t1 as select -1 c1 from range(1)
# query
CREATE TABLE t1 as select -1 c1 from range(1)
SELECT t1.c1 FROM t1
SELECT CAST(CAST(t1.c1 AS BIT) AS INTEGER), (1 BETWEEN -1 AND CAST(CAST(t1.c1 AS BIT) AS INTEGER)) FROM t1
select cast(cast(c1 as BIT) as INTEGER) as cast_res, 1 between -1 and cast(cast(c1 as BIT) as INTEGER) as watever from t1
SELECT t1.c1 FROM t1 WHERE (1 BETWEEN -1 AND CAST(CAST(t1.c1 AS BIT) AS INTEGER))
# file: test/sql/optimizer/expression/test_common_aggregate.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE groups(grp INTEGER, aggr1 INTEGER, aggr2 INTEGER, aggr3 INTEGER)
# query
PRAGMA explain_output = 'OPTIMIZED_ONLY'
EXPLAIN SELECT COUNT(*), COUNT(), COUNT(i) FROM integers
SELECT COUNT(*), COUNT(), COUNT(i) FROM integers
EXPLAIN SELECT COUNT(*), COUNT(), SUM(i), COUNT(i), SUM(i) / COUNT(i) FROM integers
SELECT COUNT(*), COUNT(), SUM(i), COUNT(i), SUM(i) / COUNT(i) FROM integers
CREATE TABLE groups(grp INTEGER, aggr1 INTEGER, aggr2 INTEGER, aggr3 INTEGER)
INSERT INTO groups VALUES (1, 1, 2, 3), (1, 2, 4, 6), (2, 1, 2, 3), (2, 3, 6, 9)
SELECT sum(aggr1)::DOUBLE / count(aggr1)::DOUBLE AS avg_qty, sum(aggr2)::DOUBLE / count(aggr2)::DOUBLE AS avg_price, sum(aggr3)::DOUBLE / count(aggr3)::DOUBLE AS avg_disc FROM groups GROUP BY grp ORDER BY grp
# file: test/sql/optimizer/expression/test_comparison_simplification.test
# setup
CREATE TABLE issue8316 (dt TIMESTAMP)
# query
WITH results AS ( SELECT '2023-08-17T23:00:08.539Z' as timestamp ) SELECT * FROM results WHERE timestamp::TIME BETWEEN '22:00:00'::TIME AND '23:59:59'::TIME
CREATE TABLE issue8316 (dt TIMESTAMP)
INSERT INTO issue8316 VALUES ('2016-02-14 18:00:05'), ('2016-02-15 10:04:25'), ('2016-02-16 10:04:25'), ('2016-02-16 23:59:55'),
SELECT dt FROM issue8316 WHERE CAST(dt as TIME) = CAST('10:04:25' as TIME) ORDER BY 1
# file: test/sql/optimizer/expression/test_conjunction_optimization.test
# setup
CREATE TABLE integers(i INTEGER)
# query
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
# file: test/sql/optimizer/expression/test_cse.test
# setup
create table test(a integer)
create table test2(a VARCHAR)
# query
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
# file: test/sql/optimizer/expression/test_date_subtract_filter.test
# setup
CREATE TABLE dates(lo_commitdate DATE)
# query
CREATE TABLE dates(lo_commitdate DATE)
INSERT INTO dates VALUES (DATE '1992-02-10')
SELECT CAST('2020-02-20' AS date) - CAST(min("ta_1"."lo_commitdate") AS date) AS "ca_1" FROM dates AS "ta_1" HAVING CAST('2020-02-20' AS date) - CAST(min("ta_1"."lo_commitdate") AS date) > 4 ORDER BY "ca_1" ASC
# file: test/sql/optimizer/expression/test_equal_or_null_optimization.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER, k integer)
# query
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
# file: test/sql/optimizer/expression/test_in_clause_simplification.test
# setup
CREATE TABLE issue13380(c0 TIMESTAMP)
# query
CREATE TABLE issue13380(c0 TIMESTAMP)
INSERT INTO issue13380(c0) VALUES ('2024-08-09 14:48:00')
SELECT c0::DATE IN ('2024-08-09') d FROM issue13380
SELECT NOT (c0::DATE IN ('2024-08-09')) FROM issue13380
SELECT c0::DATE NOT IN ('2024-08-09') FROM issue13380
# file: test/sql/optimizer/expression/test_indistinct_aggregates.test
# query
SELECT max(distinct x) from range(10) tbl(x)
SELECT x, max(distinct x) over (order by x desc) from range(10) tbl(x)
# file: test/sql/optimizer/expression/test_move_constants.test
# query
INSERT INTO vals VALUES (2), (NULL)
DROP TABLE vals
# file: test/sql/optimizer/expression/test_negation_limits.test
# query
SELECT -v FROM vals WHERE id>0
SELECT -v FROM vals WHERE id>1 ORDER BY id
# reject
SELECT -v FROM vals
# file: test/sql/optimizer/expression/test_nop_arithmetic.test
# setup
CREATE TABLE test (a INTEGER, b INTEGER)
# query
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
# file: test/sql/optimizer/expression/test_timestamp_offset.test
# setup
create or replace table table1 ( timestamp_str varchar )
# query
create or replace table table1 ( timestamp_str varchar )
insert into table1 values ('2024-05-03 01:00:00'), ('2024-05-03 01:00:02')
select timestamp_str, cast(timestamp_str as timestamp) from table1 where cast(timestamp_str as timestamp) > cast('2024-05-03 01:00:00' as timestamp)
truncate table table1
insert into table1 values ('2024-05-03T01:00:00+00:00'), ('2024-05-03T01:00:02+00:00')
select timestamp_str, cast(timestamp_str as timestamp) from table1 where cast(timestamp_str as timestamp) > cast('2024-05-03T01:00:00+00:00' as timestamp)
select * from ( select timestamp_str, cast(timestamp_str as timestamp) as timestamp_column from table1 ) where timestamp_column > cast('2024-05-03 01:00:00' as timestamp)
# file: test/sql/pragma/pragma_database_size_readonly.test
# setup
CREATE TABLE integers(i INTEGER)
# query
PRAGMA database_size
# file: test/sql/pragma/test_disabled_compression.test
# query
PRAGMA disabled_compression_methods='dictionary,rle'
# reject
PRAGMA disabled_compression_methods='uncompressed,rle'
PRAGMA disabled_compression_methods='xzx'
# file: test/sql/pragma/test_enable_http_logging.test
# query
SET enable_http_logging=false
SET enable_http_logging=true
# file: test/sql/pragma/test_enable_profile.test
# query
PRAGMA enable_profiling='json'
PRAGMA profiling_output='test.json'
PRAGMA profiling_output=''
PRAGMA disable_profiling
# reject
PRAGMA enable_profiling()
PRAGMA enable_profiling='unsupported'
PRAGMA profiling_output
# file: test/sql/pragma/test_memory_limit.test
# query
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
# reject
PRAGMA memory_limit=100
PRAGMA memory_limit='0.01BG'
PRAGMA memory_limit='0.01BLA'
PRAGMA memory_limit='0.01PP'
PRAGMA memory_limit='0.01TEST'
PRAGMA memory_limit
PRAGMA memory_limit()
PRAGMA memory_limit(1, 2)
# file: test/sql/pragma/test_metadata_info.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE db1.integers(i INTEGER, j INTEGER)
# query
PRAGMA metadata_info
FROM pragma_metadata_info()
CREATE TABLE db1.integers(i INTEGER, j INTEGER)
FROM pragma_metadata_info('db1')
# reject
FROM pragma_metadata_info(NULL)
# file: test/sql/pragma/test_pragma_database_list.test
# query
PRAGMA database_list
SELECT * FROM pragma_database_list
SELECT name, file FROM pragma_database_list
# reject
PRAGMA database_list()
# file: test/sql/pragma/test_pragma_database_size.test
# setup
CREATE TABLE db1.integers AS FROM range(1000000)
# query
CREATE TABLE db1.integers AS FROM range(1000000)
DROP TABLE db1.integers
SELECT case when used_blocks <= 3 then NULL else t end FROM pragma_database_size() t WHERE database_name='db1'
# file: test/sql/pragma/test_pragma_functions.test
# query
PRAGMA functions
# file: test/sql/pragma/test_pragma_parsing.test
# setup
CREATE TABLE integers(i INTEGER)
# query
PRAGMA table_info ('integers')
# reject
PRAGMA
PRAGMA random_unknown_pragma
PRAGMA table_info = 3
# file: test/sql/pragma/test_pragma_sanitized_inputs.test
# setup
CREATE TABLE test_table(i INTEGER, j VARCHAR)
# query
CREATE TABLE test_table(i INTEGER, j VARCHAR)
INSERT INTO test_table VALUES (1, 'hello'), (2, 'world')
SELECT * FROM test_table ORDER BY i
# file: test/sql/pragma/test_pragma_version.test
# query
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
# file: test/sql/pragma/test_query_log.test
# query
SELECT CURRENT_SETTING('log_query_path')
PRAGMA log_query_path=''
# file: test/sql/pragma/test_show_tables.test
# setup
CREATE SCHEMA s1
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE "select"(i INTEGER)
CREATE TABLE t2 (id INTEGER PRIMARY KEY, j VARCHAR UNIQUE)
CREATE TABLE tbl(i INTEGER PRIMARY KEY)
CREATE VIEW v1 AS SELECT DATE '1992-01-01' AS k
CREATE VIEW show_tables_view AS ( SHOW TABLES )
CREATE INDEX not_a_table ON tbl(i)
# query
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
# reject
DESCRIBE my_index
# file: test/sql/pragma/test_show_tables_from.test
# setup
CREATE SCHEMA test_schema
CREATE SCHEMA "db_quo""ted"."db_quo""ted_schema"
CREATE SCHEMA "Quoted Schema"
CREATE TABLE main_table1(i INTEGER)
CREATE TABLE test_schema.test_schema_table1(k INTEGER)
CREATE TABLE "db_quo""ted"."db_quo""ted_schema"."db_quo""ted_table1"(m INTEGER)
CREATE TABLE "Quoted Schema"."Quoted Table"(r INTEGER)
# query
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
# reject
SHOW TABLES FROM nonexistent_db
SHOW TABLES FROM main.nonexistent_schema
# file: test/sql/pragma/test_show_tables_temp_views.test
# setup
CREATE SCHEMA s1
CREATE TEMPORARY VIEW v1 AS SELECT 42 AS a
CREATE VIEW v2 AS SELECT 42 AS b
CREATE VIEW s1.v3 AS SELECT 42 AS c
# query
CREATE TEMPORARY VIEW v1 AS SELECT 42 AS a
CREATE VIEW v2 AS SELECT 42 AS b
CREATE VIEW s1.v3 AS SELECT 42 AS c
SET schema='s1'
FROM v2
# file: test/sql/pragma/test_storage_info.test
# setup
CREATE TABLE integers(i INTEGER, j INTEGER)
CREATE TABLE different_types(i INTEGER, j VARCHAR, k STRUCT(k INTEGER, l VARCHAR))
CREATE TABLE nested_lists AS SELECT [1, 2, 3] i, [['hello', 'world'], [NULL]] j, [{'a': 3}, {'a': 4}] k
CREATE VIEW v1 AS SELECT 42
# query
PRAGMA storage_info('integers')
INSERT INTO integers VALUES (1, 1), (2, NULL), (3, 3), (4, 5)
CREATE VIEW v1 AS SELECT 42
CREATE TABLE different_types(i INTEGER, j VARCHAR, k STRUCT(k INTEGER, l VARCHAR))
INSERT INTO different_types VALUES (1, 'hello', {'k': 3, 'l': 'hello'}), (2, 'world', {'k': 3, 'l': 'thisisaverylongstring'})
PRAGMA storage_info('different_types')
CREATE TABLE nested_lists AS SELECT [1, 2, 3] i, [['hello', 'world'], [NULL]] j, [{'a': 3}, {'a': 4}] k
PRAGMA storage_info('nested_lists')
# reject
PRAGMA storage_info('v1')
PRAGMA storage_info('bla')
# file: test/sql/pragma/test_table_info.test
# setup
CREATE SCHEMA test
CREATE TABLE integers(i INTEGER DEFAULT 1+3, j INTEGER)
create table tconstraint1(i integer primary key default(3), j blob not null)
create table tconstraint2(i integer, j integer, k integer, l integer unique, primary key(i, j, k))
create table t1 ( c1 int, c2 int generated always as (c1 + 1) )
CREATE VIEW v1 AS SELECT 42::INTEGER AS a, 'hello' AS b
CREATE VIEW v2(c) AS SELECT 42::INTEGER AS a, 'hello' AS b
CREATE VIEW v3(c, d) AS SELECT DATE '1992-01-01' a, 'hello' AS b
CREATE VIEW test.v1 AS SELECT 42::INTEGER AS a, 'hello' AS b
# query
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
# reject
PRAGMA table_info('nonexistant_table')
PRAGMA table_info(1,2,3)
# file: test/sql/pragma/test_various_pragmas.test
# query
PRAGMA enable_checkpoint_on_shutdown
PRAGMA disable_verify_parallelism
PRAGMA enable_progress_bar
PRAGMA disable_progress_bar
PRAGMA enable_print_progress_bar
PRAGMA disable_print_progress_bar
PRAGMA debug_checkpoint_abort='none'
# reject
PRAGMA explain_output='unknown'
PRAGMA debug_checkpoint_abort='unknown'
pragma explain_output =null
# file: test/sql/pragma/profiling/call_enable_profiling_function.test
# query
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
# reject
CALL enable_profiling(format='i dont exist hehe')
CALL enable_profiling(coverage='i dont exist hehe')
CALL enable_profiling(mode='i dont exist hehe')
CALL enable_profiling(mode='true')
CALL enable_profiling(metrics = "hello")
CALL enable_profiling(metrics = ['LATENCY', 'RESULT_SET_SIZE' = true])
CALL enable_profiling(metrics = 'QUERY_NAME': true, 'EXTRA_INFO': true, 'OPERATOR_ROWS_SCANNED': false, "CUMULATIVE_CARDINALITY": "true")
CALL enable_profiling(mode = 'detailed', ['LATENCY', 'RESULT_SET_SIZE'])
# file: test/sql/pragma/profiling/test_all_profiling_settings.test
# query
PRAGMA enable_profiling = 'json'
SET profiling_mode='all'
SELECT unnest(['Maia', 'Thijs', 'Mark', 'Hannes', 'Tom', 'Max', 'Carlo', 'Sam', 'Tania']) AS names ORDER BY random()
SELECT unnest(res) FROM ( SELECT current_setting('custom_profiling_settings') AS raw_setting, raw_setting.trim('{}') AS setting, string_split(setting, ', ') AS res ) ORDER BY ALL
SELECT cpu_time, extra_info, rows_returned, latency FROM metrics_output
# file: test/sql/pragma/profiling/test_attach_and_checkpoint_latency.test
# setup
CREATE TABLE profile_fs.other_tbl AS SELECT range AS id FROM range(100_000)
# query
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
# file: test/sql/pragma/profiling/test_commit_write_wal_latency_and_count.test
# query
CREATE TABLE wal_latency.tbl AS SELECT range AS id, 0 AS v FROM range(500_000)
UPDATE wal_latency.tbl SET v = id
PRAGMA custom_profiling_settings='{"COMMIT_LOCAL_STORAGE_LATENCY": "true", "WRITE_TO_WAL_LATENCY": "true"}'
SELECT CASE WHEN COMMIT_LOCAL_STORAGE_LATENCY >= 0 THEN 'true' ELSE 'false' END FROM commit_metrics
SELECT CASE WHEN WRITE_TO_WAL_LATENCY >= 0 THEN 'true' ELSE 'false' END FROM commit_metrics
DETACH wal_latency
PRAGMA custom_profiling_settings='{"WAL_REPLAY_ENTRY_COUNT": "true", "ATTACH_REPLAY_WAL_LATENCY": "true"}'
SELECT CASE WHEN wal_replay_entry_count > 0 THEN 'true' ELSE 'false' END FROM replay_metrics
SELECT CASE WHEN attach_replay_wal_latency > 0 THEN 'true' ELSE 'false' END FROM replay_metrics
# file: test/sql/pragma/profiling/test_custom_profiling_blocked_thread_time.test
# setup
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0, 10000) tbl(i)
# query
PRAGMA threads = 4
CREATE TABLE bigdata AS SELECT i AS col_a, i AS col_b FROM range(0, 10000) tbl(i)
PRAGMA custom_profiling_settings='{"BLOCKED_THREAD_TIME": "true"}'
SELECT (SELECT COUNT(*) FROM bigdata WHERE col_a = 1) + (SELECT COUNT(*) FROM bigdata WHERE col_b = 1)
SELECT COUNT(blocked_thread_time) FROM metrics_output
# file: test/sql/pragma/profiling/test_custom_profiling_disable_metrics.test
# query
PRAGMA custom_profiling_settings='{"CPU_TIME": "false", "EXTRA_INFO": "true", "OPERATOR_CARDINALITY": "true", "OPERATOR_TIMING": "true", "LATENCY": "true"}'
SELECT extra_info, latency FROM metrics_output
PRAGMA custom_profiling_settings='{"QUERY_NAME": "true", "CPU_TIME": "true", "EXTRA_INFO": "true", "CUMULATIVE_CARDINALITY": "true", "CUMULATIVE_ROWS_SCANNED": "true"}'
SELECT CASE WHEN cpu_time > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN cumulative_cardinality > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN cumulative_rows_scanned > 0 THEN 'true' ELSE 'false' END FROM metrics_output
SELECT CASE WHEN EXISTS( SELECT 1 FROM information_schema.columns WHERE table_name = 'metrics_output' AND column_name = 'query_name' ) THEN 'true' ELSE 'false' END
SELECT query_name FROM metrics_output
PRAGMA custom_profiling_settings='{"QUERY_NAME": "false"}'
# reject
SELECT cpu_time FROM metrics_output
# file: test/sql/pragma/profiling/test_custom_profiling_optimizer_settings.test
# query
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
# file: test/sql/pragma/profiling/test_custom_profiling_result_set_size.test
# setup
CREATE TYPE Result AS UNION ( Ok BOOLEAN, Err BIGINT )
# query
PRAGMA custom_profiling_settings='{"RESULT_SET_SIZE": "true", "OPERATOR_CARDINALITY": "true"}'
CREATE TYPE Result AS UNION ( Ok BOOLEAN, Err BIGINT )
SELECT CASE WHEN result_set_size = 144 THEN TRUE::Result ELSE result_set_size::Result END AS result FROM metrics_output
# file: test/sql/pragma/profiling/test_custom_profiling_row_group_metrics_local_storage.test
# setup
CREATE TABLE local_pruned(i BIGINT)
CREATE TABLE persistent_only(i BIGINT)
# query
CREATE TABLE local_pruned(i BIGINT)
INSERT INTO local_pruned VALUES (1000000), (1000001), (1000002), (1000003), (1000004), (1000005), (1000006), (1000007), (1000008), (1000009)
CREATE TABLE persistent_only(i BIGINT)
INSERT INTO persistent_only VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9)
INSERT INTO local_pruned VALUES (0), (1), (2), (3), (4), (5), (6), (7), (8), (9)
PRAGMA custom_profiling_settings='{"OPERATOR_ROW_GROUPS_SCANNED": "true", "OPERATOR_TOTAL_ROW_GROUPS_TO_SCAN": "true", "OPERATOR_TYPE": "true", "CUMULATIVE_ROW_GROUPS_SCANNED": "true", "CUMULATIVE_TOTAL_ROW_GROUPS_TO_SCAN": "true"}'
SELECT count(*) FROM local_pruned a, persistent_only b WHERE a.i < 100 AND b.i < 100
SELECT * FROM operator_metrics WHERE operator_type = 'TABLE_SCAN' ORDER BY total
SELECT sum(scanned)::VARCHAR || '/' || sum(total)::VARCHAR FROM operator_metrics WHERE operator_type = 'TABLE_SCAN'
# file: test/sql/pragma/profiling/test_custom_profiling_rows_scanned.test
# setup
CREATE TABLE integers(i INTEGER)
CREATE TABLE t AS SELECT range i FROM range(400000)
# query
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
# file: test/sql/pragma/profiling/test_custom_profiling_rows_scanned_parquet.test
# query
SELECT CASE WHEN cumulative_rows_scanned = 10 THEN 'true' ELSE 'false' END FROM metrics_output
# file: test/sql/pragma/profiling/test_custom_profiling_total_memory_allocated.test
# setup
CREATE OR REPLACE TABLE test AS SELECT range AS id, hash(range) AS data FROM range(100000)
CREATE OR REPLACE TABLE metrics_children AS SELECT unnest(children, recursive := true) FROM metrics_output
CREATE OR REPLACE TABLE test2 AS SELECT range AS id FROM range(10000)
CREATE OR REPLACE TABLE large_test AS SELECT range AS id, hash(range) AS data FROM range(1000000)
# query
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
# reject
SELECT total_memory_allocated FROM metrics_children
SELECT total_memory_allocated FROM metrics_output
# file: test/sql/pragma/profiling/test_custom_profiling_using_groups.test
# query
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
# file: test/sql/pragma/profiling/test_duckdb_profiling_settings_function.test
# query
SELECT * EXCLUDE(value, description) FROM duckdb_profiling_settings()
PRAGMA profiling_coverage='ALL'
PRAGMA profiling_mode='DETAILED'
pragma custom_profiling_settings='{"CPU_TIME": "true"}'
SELECT * EXCLUDE(description) FROM duckdb_profiling_settings()
# file: test/sql/pragma/profiling/test_logging_interaction.test
# setup
CREATE TABLE small AS FROM range(100)
CREATE TABLE medium AS FROM range(10000)
CREATE TABLE big AS FROM range(1000000)
# query
USE my_db
call enable_logging()
CREATE TABLE small AS FROM range(100)
CREATE TABLE medium AS FROM range(10000)
CREATE TABLE big AS FROM range(1000000)
SELECT count(*) FROM duckdb_logs_parsed('Metrics') WHERE metric == 'CPU_TIME'
# file: test/sql/pragma/profiling/test_no_reset_setting.test
# query
PRAGMA custom_profiling_settings='{"TOTAL_BYTES_READ": "true", "TOTAL_BYTES_WRITTEN": "true"}'
# file: test/sql/pragma/profiling/test_profiling_all.test
# query
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
# file: test/sql/pragma/profiling/test_profiling_fs.test
# query
CREATE TABLE profile_fs.tbl AS SELECT range AS id FROM range(10000)
SELECT total_bytes_written FROM metrics_output
SELECT CASE WHEN total_bytes_written > 0 THEN 'true' ELSE 'false' END FROM metrics_output
CREATE INDEX idx ON profile_fs.tbl(id)
SELECT * FROM profile_fs.tbl
SELECT CASE WHEN total_bytes_read > 0 THEN 'true' ELSE 'false' END FROM metrics_output
# file: test/sql/pragma/profiling/test_profiling_output_file_overwrite.test
# query
RESET profiling_output
PRAGMA enable_profiling = 'query_tree'
PRAGMA enable_profiling = 'GRAPHVIZ'
RESET enable_profiling
# reject
PRAGMA enable_profiling = 'html'
PRAGMA enable_profiling = 'db'
# file: test/sql/settings/allowed_configs.test
# query
SET allowed_configs=['TimeZone']
SET allowed_configs=['pivot_limit', 'EXPLAIN_output', 'memory_limit']
SET lock_configuration=true
SET pivot_limit=20000
SET explain_output='OPTIMIZED_ONLY'
SET memory_limit='2GB'
SET max_memory='2GB'
SET search_path=''
# reject
SET allowed_configs=['lock_configuration']
SET allowed_configs=['allowed_configs']
SET allowed_configs=['']
SET allowed_configs=['not_a_real_setting']
SET allowed_configs=['threads']
RESET allowed_configs
SET lock_configuration=false
# file: test/sql/settings/allowed_configs_extensions.test
# query
SET TimeZone='America/New_York'
# reject
SET Calendar='japanese'
# file: test/sql/settings/allowed_directories.test
# setup
CREATE TABLE integers(i INT)
CREATE TABLE a1.integers(i INTEGER)
# query
RESET allowed_directories
CREATE TABLE a1.integers(i INTEGER)
# reject
SET allowed_directories=[]
COPY (SELECT 42 i) TO 'permission_test.csv' (FORMAT csv)
COPY integers FROM 'permission_test.csv'
ATTACH 'test.db'
LOAD my_ext
INSTALL my_ext
EXPORT DATABASE a1 TO 'export_test'
IMPORT DATABASE 'export_test'
# file: test/sql/settings/allowed_paths.test
# query
RESET allowed_paths
# reject
SET allowed_paths=[]
# file: test/sql/settings/block_allocator_memory.test
# query
RESET block_allocator_memory
SET block_allocator_memory='100MiB'
SET memory_limit='200MiB'
SET block_allocator_memory='75%'
SELECT value FROM duckdb_settings() WHERE name = 'block_allocator_memory'
SET block_allocator_memory='200MiB'
# reject
SET block_allocator_memory='-3%'
SET block_allocator_memory='150%'
SET block_allocator_memory='50MiB'
SET block_allocator_memory='200TiB'
# file: test/sql/settings/connection_local_settings.test
# setup
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
# query
CREATE TABLE tbl AS FROM (VALUES (1), (2), (3), (NULL)) t(i)
SET default_order = 'ASCENDING'
SET default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'DESCENDING'
SET SESSION default_null_order = 'NULLS FIRST'
SET SESSION default_order = 'ASCENDING'
SET SESSION default_null_order = 'NULLS LAST'
SELECT * FROM tbl ORDER BY i
# file: test/sql/settings/default_null_order_extended.test
# setup
CREATE TABLE integers(i integer)
# query
SELECT * FROM integers ORDER BY i DESC
SELECT FIRST(i ORDER BY i), LAST(i ORDER BY i) FROM integers
SELECT FIRST(i ORDER BY i DESC), LAST(i ORDER BY i DESC) FROM integers
SELECT list_sort(LIST(i)), list_reverse_sort(LIST(i)) FROM integers
SET default_null_order='sqlite'
SET default_null_order='postgres'
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
drop schema schema2
ATTACH ':memory:' as db2
create schema db2.schema1
drop schema db2.schema1
drop schema schema1
# file: test/sql/settings/errors_as_json.test
# query
SET errors_as_json=true
# reject
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
create schema s2
use s1
use s2
reset schema
reset search_path
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
SELECT * FROM collate_test ORDER BY s
PRAGMA default_collation='NOCASE.NOACCENT'
SET GLOBAL default_collation='NOCASE'
SET SESSION default_collation='NOCASE'
# reject
PRAGMA default_collation='unknown'
# file: test/sql/settings/setting_disabled_optimizer.test
# query
SET disabled_optimizers=''
SET disabled_optimizers TO 'expression_rewriter'
SET disabled_optimizers TO 'expression_rewriter,filter_pushdown,join_order'
SELECT current_setting('disabled_optimizers')
# reject
SET disabled_optimizers TO 'expression_rewriteX'
SET disabled_optimizers TO 'unknown_optimizer'
# file: test/sql/settings/setting_exhaustive.test
# query
SELECT * FROM duckdb_settings()
SET enable_progress_bar=true
# reject
SET debug_window_mode='unknown'
SET default_order='unknown'
SET enable_profiling='unknown'
SET GLOBAL enable_progress_bar=true
SET explain_output='unknown'
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
SET preserve_identifier_case TO false
# file: test/sql/settings/setting_profiling_mode.test
# query
SET profiling_mode='detailed'
# reject
SET profiling_mode='unknown'
# file: test/sql/settings/settings_icu.test
# query
SET TimeZone='pacific/honolulu'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'TimeZone'
SET Calendar='Coptic'
SELECT name, value, description, input_type, scope FROM duckdb_settings() WHERE name = 'Calendar'
# reject
SET TimeZone='Pacific/Honolooloo'
SET Calendar='muslim'
# file: test/sql/settings/test_disabled_file_systems.test
# query
SELECT current_setting('disabled_filesystems')
RESET disabled_filesystems
SET disabled_filesystems=''
SET disabled_filesystems='LocalFileSystem'
# reject
SET disabled_filesystems='LocalFileSystem,LocalFileSystem'
SELECT * FROM abc.main.t1
SELECT * FROM abc.t1
SELECT * FROM abc.csv
# file: test/sql/settings/test_lock_configuration.test
# query
SELECT current_setting('lock_configuration')
SET memory_limit='8GB'
RESET lock_configuration
# reject
SET memory_limit='10GB'
RESET memory_limit
# file: test/sql/settings/user_agent.test
# query
SELECT current_setting('custom_user_agent')
SELECT regexp_matches(user_agent, '^duckdb/.*(.*)') FROM pragma_user_agent()
# reject
SET custom_user_agent='something else'
RESET custom_user_agent
SET duckdb_api='something else'
# file: test/sql/settings/reset/reset_threads.test
# query
select current_setting('threads')
pragma threads=42
RESET threads
# file: test/sql/tpch/dbgen_error.test
# query
SET temp_directory = '.unrecognized_folder/folder2'
# reject
CALL dbgen(sf=1)
# file: test/sql/tpcds/tpcds_sf0.test
# query
SELECT * FROM tpcds_queries()
# reject
PRAGMA tpcds(-1)
PRAGMA tpcds(3290819023812038903)
PRAGMA tpcds(32908301298)
PRAGMA tpcds(1.1)
# file: test/sql/create/create_database.test
# reject
CREATE DATABASE mydb
CREATE DATABASE mydb FROM './path'
DROP DATABASE mydb
# file: test/sql/create/create_table_as_error.test
# reject
CREATE TABLE tbl AS EXECUTE tbl
# file: test/sql/create/create_table_extra_options.test
# reject
CREATE TABLE integers(i INTEGER) PARTITIONED BY (i)
CREATE TABLE integers(i INTEGER) SORTED BY (i)
CREATE TABLE integers(i INTEGER) PARTITIONED BY (i) SORTED BY (i)
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( 'location' = 's3://amzn-s3-demo-bucket/iceberg-folder', 'table_type'='ICEBERG', 'format'='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( location = 's3://amzn-s3-demo-bucket/iceberg-folder', table_type = 'ICEBERG', format = 'parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) PARTITIONED BY (id) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
# file: test/sql/create/create_using_index.test
# reject
CREATE TABLE t0 (i INT, CONSTRAINT any_constraint UNIQUE USING INDEX any_non_existed_index)
# file: test/sql/alter/alter_table_set_partitioned_by.test
# setup
CREATE TABLE tbl(i INTEGER)
# reject
ALTER TABLE tbl SET PARTITIONED BY (i)
ALTER TABLE tbl RESET PARTITIONED BY
# file: test/sql/alter/alter_table_set_sorted_by.test
# setup
CREATE TABLE tbl(i INTEGER)
# reject
ALTER TABLE tbl SET SORTED BY (i DESC NULLS FIRST)
ALTER TABLE tbl RESET SORTED BY
# file: test/sql/alter/rename_table/test_rename_table_incorrect.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE TABLE tbl2(i INTEGER)
# reject
ALTER TABLE non_table RENAME TO tbl
ALTER TABLE tbl2 RENAME TO tbl
# file: test/sql/alter/alter_type/test_alter_type_incorrect.test
# setup
CREATE TABLE test(i INTEGER, j INTEGER)
# reject
ALTER TABLE test ALTER blabla SET TYPE VARCHAR
ALTER TABLE test ALTER i SET TYPE VARCHAR USING blabla
ALTER TABLE test ALTER i SET TYPE VARCHAR USING SUM(i)
ALTER TABLE test ALTER i SET TYPE VARCHAR USING row_id() OVER ()
ALTER TABLE test ALTER i SET TYPE VARCHAR USING othertable.j
# file: test/sql/alter/rename_col/test_rename_col_not_null.test
# setup
CREATE TABLE test(i INTEGER NOT NULL, j INTEGER)
# reject
INSERT INTO test (i, j) VALUES (NULL, 2)
INSERT INTO test (k, j) VALUES (NULL, 2)
# file: test/sql/alter/add_pk/test_add_same_pk_simultaneously.test
# setup
CREATE TABLE test (i INTEGER, j INTEGER)
# reject
INSERT INTO test VALUES (1, 1), (1, 1)
# file: test/sql/alter/rename_view/test_rename_view_table.test
# setup
CREATE TABLE tbl(i INTEGER)
CREATE VIEW v1 AS SELECT * FROM tbl
# reject
ALTER VIEW tbl RENAME TO tbl2
# file: test/sql/alter/rename_schema/rename_schema.test
# reject
ALTER SCHEMA a RENAME TO b
# file: test/sql/copy/csv/csv_duck_fuzz.test
# reject
SELECT NULL FROM sniff_csv(NULL)
SELECT NULL FROM read_csv(NULL)
# file: test/sql/copy/csv/read_csv_subquery.test
# reject
WITH urls AS ( SELECT 'a.csv' AS url UNION ALL SELECT 'b.csv' ) SELECT * FROM read_csv_auto((SELECT url FROM urls LIMIT 3), delim=',') WHERE properties.height > -1.0 LIMIT 10
SELECT * FROM read_csv_auto(sum(a) over ())
SELECT * FROM read_csv_auto(sum(a))
SELECT * FROM read_csv_auto('a.csv', delim=',', 42)
# file: test/sql/copy/parquet/broken_parquet.test
# reject
select count(*) from parquet_scan('test/sql/copy/parquet/broken/missingmagicatfront.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/missingmagicatend.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/firstmarker.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/twomarkers.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/footerlengthzero.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/hugefooter.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/garbledfooter.parquet')
from parquet_scan('test/sql/copy/parquet/broken/broken_structure.parquet')
# file: test/sql/copy/parquet/copy_option_suggestion.test
# reject
copy (select 42) to 'file.parquet' (partition_b (a))
copy (select 42) to 'file.csv' (partition_b (a))
# file: test/sql/copy/parquet/parquet_list.test
# reject
select count(*) from parquet_scan([]::varchar[])
select count(*) from parquet_scan([NULL])
select count(*) from parquet_scan(NULL::VARCHAR[])
select count(*) from parquet_scan(NULL::VARCHAR)
# file: test/sql/attach/attach_external_access.test
# reject
ATTACH 'mydb.db' AS db2
# file: test/sql/attach/attach_issue7711.test
# reject
detach test
# file: test/sql/attach/system_catalog.test
# reject
DETACH DATABASE system
DETACH DATABASE temp
CREATE SCHEMA system.eek
CREATE TABLE system.main.integers(i INTEGER)
CREATE VIEW system.main.integers AS SELECT 42
CREATE SEQUENCE system.main.seq
CREATE MACRO system.main.my_macro(a,b) AS a+b
CREATE TYPE system.main.rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
# file: test/sql/function/array/array_inner_product.test
# reject
SELECT array_inner_product('foo', 'bar')
SELECT array_inner_product([1,2,3]::INT[3], ['a','b','c']::VARCHAR[3])
SELECT array_distance(['a','b']::VARCHAR[2],['foo','bar']::VARCHAR[2])
# file: test/sql/function/list/lambda_constant_null.test
# reject
SELECT quantile(NULL, filter(NULL, (lambda c103: 'babea54a-2261-4b0c-b14b-1d0e9b794e1a')))
# file: test/sql/function/list/aggregates/incorrect.test
# reject
SELECT list_aggr([1], 2)
SELECT list_aggr([1], True)
SELECT list_aggr([1], NULL)
SELECT list_aggr([1, 2, NULL], 'count_star')
SELECT list_aggr([1, 2, NULL], 'corr')
SELECT list_aggr([1, 2, NULL], 'covar_pop')
SELECT list_aggr([1, 2, NULL], 'covar_samp')
SELECT list_aggr([1, 2, NULL], 'regr_intercept')
# file: test/sql/function/list/aggregates/sum_no_overflow.test
# reject
SELECT sum_no_overflow(42)
SELECT sum_no_overflow(42.5)
# file: test/sql/pragma/test_db_invalidation_after_load.test
# reject
SET disable_database_invalidation=true
SET allow_unredacted_secrets=true
# file: test/sql/pragma/test_force_compression.test
# reject
PRAGMA force_compression='unknown'
# file: test/sql/pragma/profiling/test_custom_profiling_errors.test
# reject
PRAGMA custom_profiling_settings='}}}}}}'
PRAGMA custom_profiling_settings=BONJOUR
PRAGMA custom_profiling_settings=[NOT_A_JSON]
PRAGMA custom_profiling_settings='{"INVALID_SETTING": "true"}'
# file: test/sql/pragma/profiling/test_empty_profiling_settings.test
# reject
SELECT extra_info FROM metrics_output
SELECT operator_cardinality FROM metrics_output
SELECT operator_timing FROM metrics_output
SELECT cumulative_cardinality FROM metrics_output
# file: test/sql/settings/access_mode.test
# reject
SET access_mode='read_only'
# file: test/sql/settings/set_schema_temp_main.test
# reject
CREATE SCHEMA temp.s1
CREATE SCHEMA system.s1
set schema = 'temp'
set schema = 'system'
# file: test/sql/settings/test_disabled_local_filesystem_metadata.test
# reject
SELECT * FROM duckdb_secrets()
SELECT * FROM duckdb_extensions()
# file: test/sql/settings/test_disabled_local_filesystem_secrets.test
# reject
CREATE PERSISTENT SECRET my_s (TYPE S3)
# file: test/sql/settings/test_external_access_secrets.test
# reject
CREATE PERSISTENT SECRET my_secret (TYPE S3)
# file: test/sql/tpch/dbgen_readonly.test
# setup
CREATE TABLE tbl (i INTEGER)
# reject
CALL dbgen(sf=0, catalog='dbgentest')
# file: test/sql/tpch/tpch_sf0.test
# reject
PRAGMA tpch(-1)
PRAGMA tpch(3290819023812038903)
PRAGMA tpch(32908301298)
PRAGMA tpch(1.1)
# file: test/sql/tpcds/dsdgen_readonly.test
# setup
CREATE TABLE tbl (i INTEGER)
# reject
CALL dsdgen(sf=0)
CALL dsdgen(sf=0, catalog='dsdgentest')
