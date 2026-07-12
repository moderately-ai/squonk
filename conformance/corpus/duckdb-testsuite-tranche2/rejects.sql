INSERT INTO integers BY NAME SELECT 1 AS xxx
INSERT INTO integers BY NAME SELECT 1 AS i, 2 AS i
INSERT INTO integers (i, i) SELECT 1, 2
INSERT INTO integers BY NAME SELECT 1 AS rowid
INSERT INTO tbl BY NAME SELECT 1 AS total_price
INSERT INTO integers BY NAME VALUES (42, 84)
INSERT INTO integers BY NAME (i) SELECT 1 AS j
INSERT INTO integers VALUES (DEFAULT+1, 4)
INSERT INTO a VALUES (1)
INSERT INTO a VALUES (1,2,3)
INSERT INTO a VALUES (1,2),(3)
INSERT INTO a VALUES (1,2),(3,4,5)
INSERT INTO a SELECT 42
CHECKPOINT
UPDATE tbl SET (key, fruit, cost) = (1, 2)
UPDATE tbl SET (key, fruit, cost) = (1, 2, 3, 4)
UPDATE tbl SET () = (key, fruit)
UPDATE tbl SET (key, fruit) = ()
UPDATE test SET a=99 WHERE a=1
UPDATE test SET a=99 WHERE a=2
UPDATE test SET a=99 WHERE a=3
UPDATE tbl SET myco=42
UPDATE tbl SET tbl.mycol=42
DELETE FROM tbl WHERE i <= 500
DELETE FROM a USING b WHERE a.i=b.i
DELETE FROM a USING a b WHERE a.i=b.j
MERGE INTO Stock USING Sale ON Stock.item_id = Sale.item_id WHEN MATCHED AND Sale.volume >= balance THEN DELETE WHEN MATCHED THEN UPDATE SET balance = balance - Sale.volume WHEN NOT MATCHED THEN ERROR CONCAT('Sale item with item id ', Sale.item_id, ' not found')
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id)
WITH initial_stocks(item_id, balance) AS (VALUES (10, 2200), (20, 1900)) MERGE INTO Stock USING initial_stocks ON (Stock.item_id = initial_stocks.item_id) WHEN NOT MATCHED THEN INSERT VALUES (initial_stocks.item_id, initial_stocks.balance) WHEN NOT MATCHED THEN ERROR
MERGE INTO my_view USING Sale ON my_view.item_id = Sale.item_id WHEN NOT MATCHED THEN INSERT
MERGE INTO Stock USING (VALUES (NULL, NULL)) new_accounts(item_id, balance) USING (item_id) WHEN NOT MATCHED THEN INSERT VALUES (new_accounts.item_id, new_accounts.balance)
MERGE INTO Stock USING (VALUES (1, 15)) sales(item_id, volume) USING (item_id) WHEN MATCHED THEN UPDATE SET balance = balance - volume
MERGE INTO Items USING (VALUES (1, 15)) new_prices(item_id, total_cost) USING (item_id) WHEN MATCHED THEN UPDATE SET total_cost = new_prices.total_cost
MERGE INTO Stock USING (VALUES (0, 7)) new_accounts(balanc, item_id) USING (item_id) WHEN NOT MATCHED THEN INSERT BY NAME
MERGE INTO t USING (SELECT range a from generate_series (10,19) t(range)) AS s USING(a) WHEN NOT MATCHED BY TARGET THEN DELETE RETURNING merge_action, *
MERGE INTO t USING (SELECT range a from generate_series (10,19) t(range)) AS s USING(a) WHEN NOT MATCHED BY TARGET THEN UPDATE RETURNING merge_action, *
INSERT INTO v0(x) VALUES (2) ON CONFLICT DO NOTHING
MERGE INTO people USING ( SELECT 3 AS id, 89_000.0 AS salary ) AS upserts ON (upserts.id = people.id) WHEN NOT MATCHED THEN INSERT
MERGE INTO Stock USING (VALUES (10)) new_accounts(item_id) USING (item_id) WHEN MATCHED THEN UPDATE SET *
MERGE INTO Stock USING (VALUES (10, 20)) new_accounts(item_id, balanc) USING (item_id) WHEN MATCHED THEN UPDATE BY NAME
CREATE TABLE tbl1 AS SELECT 3
CREATE TABLE tbl4 IF NOT EXISTS AS SELECT 4
CREATE OR REPLACE TABLE tbl4 IF NOT EXISTS AS SELECT 4
CREATE TABLE tbl7(col1, col2) AS SELECT 5
CREATE TABLE integers PARTITIONED BY (i) as select range i from range(10)
CREATE TABLE integers SORTED BY (i) as select range i from range(10)
CREATE TABLE integers PARTITIONED BY (i) SORTED BY (i) as select range i from range(10)
CREATE TABLE iceberg_table WITH ( 'location' = 's3://amzn-s3-demo-bucket/iceberg-folder', 'table_type'='ICEBERG', 'format'='parquet' ) as select range i from range(10)
CREATE TABLE iceberg_table PARTITIONED BY (id) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range i, range::DATE date from range(10)
CREATE TABLE iceberg_table SORTED BY (date) PARTITIONED BY (id) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' ) as select range id, range::DATE date from range(10)
CREATE DATABASE mydb
CREATE DATABASE mydb FROM './path'
DROP DATABASE mydb
CREATE schema s2
CREATE TABLE test AS SELECT * FROM range(10) t(i)
CREATE view v1 AS SELECT * FROM range(10) t(i)
CREATE macro add(a, b) AS a + b
CREATE TYPE mood AS ENUM ('happy', 'sad', 'curious')
CREATE SEQUENCE serial START 101
CREATE OR REPLACE TABLE integers2(i INTEGER)
CREATE OR REPLACE TABLE IF NOT EXISTS integers(i INTEGER)
CREATE TABLE tbl AS EXECUTE tbl
CREATE TABLE T (a INTEGER USING COMPRESSION 'bla')
CREATE TABLE T (a INTEGER USING COMPRESSION )
CREATE TABLE T (a INTEGER NOT NULL USING COMPRESSION )
CREATE TABLE T (a INTEGER USING COMPRESSION bla)
CREATE TABLE '' AS SELECT 42
CREATE TABLE s1.""(i INTEGER)
CREATE TABLE integers(i INTEGER) PARTITIONED BY (i)
CREATE TABLE integers(i INTEGER) SORTED BY (i)
CREATE TABLE integers(i INTEGER) PARTITIONED BY (i) SORTED BY (i)
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( 'location' = 's3://amzn-s3-demo-bucket/iceberg-folder', 'table_type'='ICEBERG', 'format'='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( location = 's3://amzn-s3-demo-bucket/iceberg-folder', table_type = 'ICEBERG', format = 'parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) PARTITIONED BY (id) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) SORTED BY (date) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
CREATE TABLE iceberg_table ( id int, data string, category string) WITH ( location='s3://amzn-s3-demo-bucket/iceberg-folder', table_type='ICEBERG', format='parquet' )
CREATE TABLE t0 (i INT, CONSTRAINT any_constraint UNIQUE USING INDEX any_non_existed_index)
ALTER TABLE tbl SET PARTITIONED BY (i)
ALTER TABLE tbl RESET PARTITIONED BY
ALTER TABLE tbl SET SORTED BY (i DESC NULLS FIRST)
ALTER TABLE tbl RESET SORTED BY
ALTER TABLE tbl1 SET ('foo'='baz', 'buzz'='wizz')
ALTER TABLE tbl1 SET (foo='baz', buzz='wizz')
ALTER TABLE tbl1 SET (foo=getvariable('location'))
ALTER TABLE tbl1 RESET ('foo', 'buzz')
ALTER TABLE tbl1 RESET (foo, buzz)
ALTER TABLE tbl1 RESET ('foo'='baz', 'buzz'='wizz')
ALTER TABLE tbl1 RESET (foo='baz', buzz='wizz')
ALTER TABLE tbl1 RESET ()
ALTER DATABASE non_existent SET ALIAS TO something_else
ALTER DATABASE another_db SET ALIAS TO renamed_db
ALTER DATABSE renamed_db RENAME TO system
ALTER DATABSE renamed_db RENAME TO temp
INSERT INTO t0 VALUES (42)
ALTER TABLE t0 ADD COLUMN c0 int
ALTER TABLE t1 DROP COLUMN IF EXISTS c3
ALTER TABLE t1 DROP COLUMN c3
ALTER TABLE t0 DROP COLUMN c3
ALTER TABLE IF EXISTS t0 DROP COLUMN c3
ALTER TABLE IF EXISTS t1 ALTER COLUMN IF EXISTS c0 TYPE varchar
INSERT INTO test VALUES (3, NULL)
INSERT INTO test VALUES (NULL)
ALTER TABLE t ALTER COLUMN j SET NOT NULL
INSERT INTO t VALUES(8888, NULL)
INSERT INTO t VALUES(NULL)
INSERT INTO t VALUES(3, NULL)
INSERT INTO t VALUES(7777, NULL)
INSERT INTO t VALUES(1, 1)
INSERT INTO t VALUES(1, NULL)
INSERT INTO t VALUES(2, NULL)
INSERT INTO t VALUES (3, NULL)
INSERT INTO t VALUES (6, NULL)
INSERT INTO t VALUES (2, null)
ALTER TABLE t ALTER COLUMN i SET NOT NULL
INSERT INTO t VALUES (null)
SELECT * FROM tbl5
SELECT * FROM entry2
SELECT * FROM entry3
SELECT * FROM entry4
SELECT i FROM t3 ORDER BY i
CREATE TABLE t3 (i INTEGER)
ALTER TABLE t2 RENAME TO t4
ALTER TABLE e1 RENAME TO e2
ALTER TABLE e2 RENAME TO e1
INSERT INTO tbl VALUES (777, 10), (888, 10)
INSERT INTO new_tbl VALUES (999, 0), (1000, 1)
INSERT INTO new_tbl VALUES (9999, 0), (10000, 1)
INSERT INTO new_tbl VALUES (1, 10), (2, 999)
ALTER TABLE non_table RENAME TO tbl
ALTER TABLE tbl2 RENAME TO tbl
ALTER TABLE v1 RENAME TO v2
ALTER TABLE t0 RENAME TO t3
ALTER TABLE t0 RENAME TO t4
ANALYZE t4
ALTER TABLE test ALTER not_a_column SET DATA TYPE INTEGER
ALTER TABLE tbl ALTER col TYPE
EXECUTE v2
ALTER TABLE test ALTER blabla SET TYPE VARCHAR
ALTER TABLE test ALTER i SET TYPE VARCHAR USING blabla
ALTER TABLE test ALTER i SET TYPE VARCHAR USING SUM(i)
ALTER TABLE test ALTER i SET TYPE VARCHAR USING row_id() OVER ()
ALTER TABLE test ALTER i SET TYPE VARCHAR USING othertable.j
INSERT INTO test VALUES (NULL, 4)
ALTER TABLE test ALTER i TYPE VARCHAR
INSERT INTO test (i, j) VALUES (3, 3)
DELETE FROM test WHERE i=1
UPDATE test SET i=1000
UPDATE test SET j=100
CREATE INDEX i_index ON test(j)
INSERT INTO test (i, j) VALUES (100, 2)
INSERT INTO test (k, j) VALUES (100, 2)
ALTER TABLE test RENAME COLUMN blablabla TO k
ALTER TABLE test RENAME COLUMN i TO j
INSERT INTO test (i, j) VALUES (NULL, 2)
INSERT INTO test (k, j) VALUES (NULL, 2)
SELECT i FROM test
INSERT INTO test (i, j) VALUES (1, 1)
INSERT INTO test (k, j) VALUES (1, 1)
ALTER TABLE data ALTER COLUMN j DROP DEFAULT
ALTER TABLE test ALTER blabla SET DEFAULT 3
ALTER TABLE test ALTER blabla DROP DEFAULT
ALTER TABLE test ADD COLUMN i INTEGER
ALTER VIEW x ADD COLUMN i INTEGER
ALTER TABLE i ADD COLUMN j INT, ADD COLUMN k INT
ALTER TABLE main_t1 ADD COLUMN j test_int
ALTER TABLE test ADD COLUMN s.s2.v1 VARCHAR
ALTER TABLE test ADD COLUMN s.s2.v1.x INTEGER
ALTER TABLE test ADD COLUMN s.i VARCHAR
ALTER TABLE test ADD COLUMN s.i.a INTEGER
ALTER TABLE test ADD COLUMN s.x.a INTEGER
ALTER TABLE test DROP COLUMN s.s2.v1
ALTER TABLE test DROP COLUMN s.j
ALTER TABLE test DROP COLUMN s.v
ALTER TABLE test DROP COLUMN s.j.a
ALTER TABLE test DROP COLUMN z.j
ALTER TABLE test DROP COLUMN s.v1.a
ALTER TABLE test RENAME COLUMN s.s2.v2 TO i
ALTER TABLE test RENAME s.j TO v1
ALTER TABLE test RENAME s.j.x TO v2
ALTER TABLE test RENAME s.i TO v2
ALTER TABLE test RENAME x.i TO v2
INSERT INTO test VALUES (1, 2, 'oops')
INSERT INTO test VALUES (NULL, 2, 'nada')
INSERT INTO test VALUES (2, 1)
INSERT INTO test VALUES (2, NULL)
ALTER TABLE test ADD PRIMARY KEY (i)
INSERT INTO reverse (j, i) VALUES (2, 1)
ALTER TABLE view_test ADD PRIMARY KEY (name)
ALTER TABLE test ADD PRIMARY KEY (i_do_not_exist)
ALTER TABLE i_do_not_exist ADD PRIMARY KEY (i, j)
INSERT INTO uniq VALUES (1, 100)
INSERT INTO uniq VALUES (1, 101)
INSERT INTO uniq VALUES (NULL, 100)
ALTER TABLE duplicates ADD PRIMARY KEY (i)
ALTER TABLE nulls ADD PRIMARY KEY (i, j)
ALTER TABLE nulls ADD PRIMARY KEY (i)
ALTER TABLE nulls_compound ADD PRIMARY KEY (k, i)
ALTER TABLE test ADD PRIMARY KEY (a)
ALTER TABLE test ADD PRIMARY KEY (a, b)
ALTER TABLE tbl ADD PRIMARY KEY (i)
CREATE INDEX PRIMARY_test_i ON test(i)
INSERT INTO test VALUES (2, 2)
ALTER TABLE test ADD PRIMARY KEY (b)
ALTER TABLE test ADD PRIMARY KEY (b, c)
INSERT INTO test VALUES (1, 4)
INSERT INTO test VALUES (1, 1), (1, 1)
ALTER TABLE other ADD PRIMARY KEY (i, j)
ALTER TABLE other ADD PRIMARY KEY (i)
ALTER TABLE test ADD COLUMN s.a.not_key INTEGER
ALTER TABLE test ADD COLUMN s.a.key INTEGER
ALTER TABLE test DROP COLUMN s.key
ALTER TABLE test DROP COLUMN s.value
ALTER TABLE test RENAME COLUMN s.key to anything
ALTER TABLE test RENAME COLUMN s.value to anything
SELECT * FROM vw
ALTER VIEW sqlite_master RENAME TO my_sqlite_master
ALTER VIEW nonexistingview RENAME TO my_new_view
ALTER VIEW non_view RENAME TO vw
ALTER VIEW vw2 RENAME TO vw
SELECT * FROM vw3
SELECT * FROM vw4
ALTER VIEW tbl RENAME TO tbl2
ALTER SCHEMA a RENAME TO b
ALTER TABLE test ADD COLUMN s.a.not_element INTEGER
ALTER TABLE test DROP COLUMN s.element
ALTER TABLE test RENAME COLUMN s.element TO not_element
ALTER TABLE test2 DROP COLUMN j
INSERT INTO test VALUES (20)
ALTER TABLE test DROP COLUMN blabla
ALTER TABLE test2 DROP COLUMN surname
ALTER TABLE test2 DROP COLUMN age
CREATE INDEX i3 ON t1 USING random_index_method (foo) WITH (my_option = 2, is_cool)
CREATE INDEX i3 ON t1 USING random_index_method (foo) WITH (my_option = getenv('some_env_variable'), is_cool)
CREATE UNIQUE INDEX idx ON duplicate_id(id, id2)
INSERT INTO numbers VALUES (3.45, 4), (3.45, 5)
INSERT INTO numbers VALUES (6, 6), (3.45, 4)
INSERT INTO numbers VALUES (NULL, 4)
UPDATE numbers SET i=NULL
CREATE INDEX idx_u_2 ON tbl (u_2)
CREATE INDEX idx_u_1 ON tbl (u_1)
CREATE INDEX idx_u_list ON tbl (u_list)
CREATE INDEX idx_u_list ON tbl (i, u_list)
CREATE UNIQUE INDEX idx_list_2 ON tbl ((u_list.list))
INSERT INTO tbl VALUES ('helloo', 'nop', 7, true)
CREATE INDEX idx_c_fail ON tbl ((u_2.string), u_list)
INSERT INTO tbl VALUES ('sunshine', 'love', 85, true)
INSERT INTO tbl_comp VALUES (2, 'hola', 5, 'world')
INSERT INTO tbl_comp VALUES (3, 'hoi', 1, 'wereld')
INSERT INTO tbl_comp VALUES (42, 'hoi', 2, 'welt')
INSERT INTO a VALUES (1, 5)
INSERT INTO tbl VALUES (50_000)
SELECT 42
UPDATE tbl_list SET id = id + 1 RETURNING id, payload
UPDATE tbl_list SET payload = ['second payload'] WHERE id = 1
INSERT OR REPLACE INTO tbl_list VALUES (1, ['second payload'])
INSERT INTO varchars VALUES ('hello' || chr(0) || chr(0) || chr(0))
INSERT INTO tbl VALUES (12501)
INSERT INTO integers VALUES (1)
CREATE INDEX idx_1 ON idx_tbl(i)
CREATE INDEX PRIMARY_tbl_0 ON tbl(i)
CREATE INDEX UNIQUE_tbl_1 ON tbl(j)
INSERT INTO tbl VALUES (4000, 20)
INSERT INTO tbl VALUES (20, 4000)
INSERT INTO fk_tbl VALUES (4000, 20)
INSERT INTO fk_tbl VALUES (20, 4000)
CREATE INDEX FOREIGN_fk_tbl_0 ON fk_tbl(i)
CREATE INDEX FOREIGN_fk_tbl_1 ON fk_tbl(j)
CREATE INDEX idx ON tbl (i)
DROP INDEX idx_drop
insert into tbl_m VALUES (10, 'world')
INSERT INTO test VALUES (0)
INSERT INTO alter_test VALUES (0)
CREATE UNIQUE INDEX i_leak ON t_leak (c1)
CREATE INDEX my_idx ON tbl(i)
CREATE UNIQUE INDEX idx ON merge_violation(id)
CREATE INDEX ON integers(i)
CREATE INDEX i_index ON integers(i COLLATE "NOCASE")
CREATE INDEX i_index ON integers(i COLLATE "de_DE")
CREATE INDEX i_index ON integers using blabla(i)
CREATE INDEX i_index ON integers(f)
create index i_index on lists(l)
create index i_index on lists(id, l)
create index i_index on integers(('hello'))
UPDATE integers SET i=i, i=10
UPDATE integers SET i=i, j=10
UPDATE integers SET j=10
UPDATE integers SET i=NULL
UPDATE integers SET j=NULL
INSERT INTO integers VALUES (NULL)
INSERT INTO integers (i) SELECT * FROM integers_with_null
INSERT INTO integers VALUES (6, 6), (3, 4)
UPDATE integers SET i=77 WHERE i IS NULL
INSERT INTO integers VALUES (NULL, 6), (3, 7)
CREATE UNIQUE INDEX uidx ON integers (j)
INSERT INTO integers VALUES (3, 4)
INSERT INTO integers VALUES (3, 3), (4, 1)
UPDATE integers SET i=4, j=100 WHERE i=1
UPDATE integers SET i=100, j=4 WHERE j=1
INSERT INTO integers VALUES (6, '6'), (3, '4')
UPDATE integers SET j='77' WHERE j IS NULL
INSERT INTO integers VALUES (3, '4')
INSERT INTO integers VALUES (1, false)
INSERT INTO numbers VALUES (1,1,1,1,1), (1,1,1,1,1)
INSERT INTO numbers VALUES (1,1,1,1,1),(1,5,1,1,4)
UPDATE numbers SET b=1 WHERE b=2
UPDATE integers SET i=5 WHERE i=2
DELETE FROM integers WHERE i=2
INSERT INTO numbers VALUES (1,1,1,1,1),(1,1,1,1,4)
INSERT INTO integers VALUES (6, 'bla'), (3, 'hello')
INSERT INTO tst VALUES ('hell', 'hello'), ('hell','hello')
INSERT INTO tst VALUES ('hell', 'hello'),('hel', 'hello')
UPDATE tst SET b='hello' WHERE b='hel'
INSERT INTO numbers VALUES ('1', 4), ('1', 5)
INSERT INTO numbers VALUES ('6', 6), ('1', 4)
UPDATE test SET a=a+1 WHERE b=1
UPDATE test SET a=NULL WHERE b=1
UPDATE test SET a = 15 WHERE a = 14
UPDATE test SET a = 4
UPDATE test SET b = NULL WHERE a = 13
INSERT INTO integers VALUES (3, 4), (3, 5)
INSERT INTO integers VALUES (NULL, 4)
INSERT INTO tbl VALUES ({'t': 43})
INSERT INTO integers VALUES (7)
INSERT INTO integers VALUES (5, 5)
INSERT INTO integers VALUES (3, 3), (5, 5)
CREATE TABLE indirect_subq( i INTEGER, CHECK (i > (2 * (SELECT(1)))) )
CREATE TABLE integers2(i INTEGER CHECK(i > (SELECT 42)), j INTEGER)
CREATE TABLE integers2(i INTEGER CHECK(i > SUM(j)), j INTEGER)
CREATE TABLE integers3(i INTEGER CHECK(k < 10), j INTEGER)
CREATE TABLE integers3(i INTEGER CHECK(integers3.k < 10), j INTEGER)
UPDATE v1 SET c = 2
UPDATE v2 SET z = 7 WHERE x = 2
INSERT INTO v_char VALUES ('ab', 'active')
INSERT INTO v_char VALUES ('charlie', 'pending')
UPDATE v_char SET status = 'deleted' WHERE username = 'alice'
INSERT INTO A (a1, a2, a3, a4, a5, a6) VALUES ('x', 'y', 'z', 'u', 1, 2), ('y', 'z', 'x', 'v', 1, 2), ('x', 'x', 'y', 'y', 2, 3), ('z', 'z', 'v', 'x', 4, 5)
INSERT INTO D VALUES (9, 9, 'a', 'b', 40)
INSERT INTO D VALUES (0, 1, 'x', 'y', 50)
INSERT INTO tf_2 VALUES (1, 1, 1)
INSERT INTO tf_2 VALUES (2, 1, 1)
INSERT INTO tf_2 VALUES (1, 2, 1)
INSERT INTO tf_2 VALUES (1, 1, 2)
DELETE FROM tf_1 WHERE a = 2
insert into b values (1)
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (agency_id) REFERENCES agency )
CREATE TABLE routes ( route_id TEXT PRIMARY KEY, agency_id TEXT, FOREIGN KEY (route_id, agency_id) REFERENCES agency )
INSERT INTO routes VALUES (1, 1)
DROP TABLE agency
INSERT INTO routes VALUES (2, 2)
CREATE TABLE t7(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE CASCADE)
CREATE TABLE t8(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE CASCADE)
CREATE TABLE t9(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE SET DEFAULT)
CREATE TABLE t10(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE SET DEFAULT)
CREATE TABLE t11(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON UPDATE SET NULL)
CREATE TABLE t12(id INTEGER PRIMARY KEY, t1_id INTEGER, FOREIGN KEY (t1_id) REFERENCES t1(id) ON DELETE SET NULL)
drop table departments
ALTER TABLE departments RENAME TO old_departments
INSERT INTO t2 VALUES (5)
INSERT INTO t3 VALUES (4)
INSERT INTO t4 VALUES (3)
DELETE FROM t1 WHERE i1=4
DELETE FROM t2 WHERE i2=3
DELETE FROM t3 WHERE i3=2
DROP TABLE t1
DROP TABLE fk_integers
INSERT INTO fk_integers VALUES (4), (5)
CREATE TABLE s2.fk_integers(j INTEGER, FOREIGN KEY (j) REFERENCES s1.pk_intexgers(i))
UPDATE fk_db.tbl_pk SET payload = {'v': 'new hello', 'i': [7]} WHERE i = 1
INSERT INTO fk_integers VALUES (4)
DELETE FROM pk_integers WHERE i=3
UPDATE pk_integers SET i=5 WHERE i=2
UPDATE fk_integers SET i=4 WHERE j=2
INSERT INTO pkt1 VALUES (3, 11)
INSERT INTO pkt2 VALUES (101, 1000)
INSERT INTO fkt1 VALUES (3, 101)
INSERT INTO fkt1 VALUES (2, 103)
INSERT INTO fkt2 VALUES (13, 1002)
INSERT INTO fkt1 VALUES (12, 1003)
DELETE FROM pkt1 WHERE i1=1
DELETE FROM pkt2 WHERE i2=102
CREATE TABLE t(v_id TEXT, FOREIGN KEY (v_id) REFERENCES v(id))
INSERT INTO secondary_table VALUES (42)
DELETE FROM pk_integers WHERE i=2
CREATE TABLE employee( id INTEGER PRIMARY KEY, managerid INTEGER, name VARCHAR, FOREIGN KEY(managerid) REFERENCES employee(emp_id))
INSERT INTO employee VALUES (4, 4, 'Mark')
UPDATE employee SET id = 5 WHERE id = 2
DELETE FROM employee WHERE id = 2
UPDATE employee SET id = 2 WHERE id = 3
UPDATE employee SET managerid = 5 WHERE id = 4
ALTER TABLE employee RENAME COLUMN managerid TO managerid_new
ALTER TABLE employee ALTER COLUMN id SET DATA TYPE TEXT
INSERT INTO song VALUES (11, 1, 'A', 'A_song'), (12, 2, 'E', 'B_song'), (13, 3, 'C', 'C_song')
INSERT INTO song VALUES (11, 1, 'A', 'A_song'), (12, 5, 'D', 'B_song'), (13, 3, 'C', 'C_song')
DELETE FROM album WHERE albumname = 'C'
UPDATE song SET songartist = 5, songalbum = 'A' WHERE songname = 'B_song'
UPDATE album SET albumname='B' WHERE albumcover='C_cover'
UPDATE song SET songalbum='E' WHERE albumcover='C_song'
ALTER TABLE album RENAME COLUMN albumname TO albumname_new
ALTER TABLE song RENAME COLUMN songalbum TO songalbum_new
INSERT INTO fkt VALUES (3)
DELETE FROM pkt WHERE i = 1
DELETE FROM pkt WHERE i = 2
DROP TABLE pkt
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songalbum) REFERENCES album(artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES albumlist(artistid, albumname))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, album_name))
CREATE TABLE song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, song_album) REFERENCES album(artistid, albumname))
DELETE FROM album WHERE albumname='C'
UPDATE song SET songartist=5, songalbum='A' WHERE songname='B_song'
ALTER TABLE song ALTER COLUMN songartist SET DATA TYPE TEXT
SELECT NULL FROM sniff_csv(NULL)
SELECT NULL FROM read_csv(NULL)
SET enable_external_access=true
WITH urls AS ( SELECT 'a.csv' AS url UNION ALL SELECT 'b.csv' ) SELECT * FROM read_csv_auto((SELECT url FROM urls LIMIT 3), delim=',') WHERE properties.height > -1.0 LIMIT 10
SELECT * FROM read_csv_auto(sum(a) over ())
SELECT * FROM read_csv_auto(sum(a))
SELECT * FROM read_csv_auto('a.csv', delim=',', 42)
SELECT DISTINCT NULL, c3, (c4 <= c1), (c3 BETWEEN c4 AND c2) FROM sniff_csv('1a616242-1dcd-4914-99d1-16119d9b6e4c', "names" := ['1970-01-01'::DATE, 'infinity'::DATE, '-infinity'::DATE, NULL, '2022-05-12'::DATE], filename := '9be2bc9d-d49f-4564-bfa4-6336b211a874') AS t5(c1, c2, c3, c4) WHERE c1 GROUP BY c3 LIMIT ('c4000757-69ca-400e-b58a-1dac73b85595' IS NULL)
SELECT * FROM read_csv_auto([]) ORDER BY 1
SELECT * FROM read_csv_auto([]::VARCHAR[]) ORDER BY 1
SELECT * FROM read_csv_auto(NULL) ORDER BY 1
SELECT * FROM read_csv_auto([NULL]) ORDER BY 1
SELECT * FROM read_csv_auto(NULL::VARCHAR) ORDER BY 1
SELECT * FROM read_csv_auto(NULL::VARCHAR[]) ORDER BY 1
select count(*) from parquet_scan('test/sql/copy/parquet/broken/missingmagicatfront.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/missingmagicatend.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/firstmarker.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/twomarkers.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/footerlengthzero.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/hugefooter.parquet')
select count(*) from parquet_scan('test/sql/copy/parquet/broken/garbledfooter.parquet')
from parquet_scan('test/sql/copy/parquet/broken/broken_structure.parquet')
copy (select 42) to 'file.parquet' (partition_b (a))
copy (select 42) to 'file.csv' (partition_b (a))
SET binary_as_sting=true
PRAGMA add_parquet_key('my_cool_key', '42')
PRAGMA add_parquet_key('my_invalid_duck_key', 'ZHVjaw==')
select count(*) from parquet_scan([]::varchar[])
select count(*) from parquet_scan([NULL])
select count(*) from parquet_scan(NULL::VARCHAR[])
select count(*) from parquet_scan(NULL::VARCHAR)
SELECT * FROM parquet_scan('does_not_exist')
INSERT INTO int_maps VALUES (MAP([NULL], [NULL]))
INSERT INTO string_map VALUES (MAP([NULL], [NULL]))
INSERT INTO list_map VALUES (MAP([NULL], [NULL]))
CREATE TABLE t2 AS SELECT * FROM range(1000000)
INSERT INTO test VALUES (11, 22, 'hello')
INSERT INTO test VALUES (1,101),(2,201)
INSERT INTO unique_index_test VALUES (1,101),(2,201)
RESET temp_directory
SET temp_file_encryption = true
CREATE TABLE tbl(col VARIANT UNIQUE, b INTEGER)
create table all_types as select struct_pack(*COLUMNS(*))::VARIANT test from test_all_types()
create table t1 (v VARIANT)
create table t2 as select '1'::VARIANT v
alter table t3 add column v VARIANT
alter table t3 alter column i set type VARIANT
DETACH fail_detach
DELETE FROM pk_integers WHERE i = 3
UPDATE pk_integers SET i = 5 WHERE i = 2
UPDATE fk_integers SET j = 4 WHERE j = 2
INSERT INTO test VALUES (12, 13)
INSERT INTO test VALUES (5, 3)
INSERT INTO test VALUES (9, 99)
INSERT INTO integers VALUES (1, 1)
CREATE TABLE aliens ( name text, current_mood mood )
SELECT a FROM test
SELECT nextval('seq2')
DELETE FROM asdf.a
SELECT b,c FROM test.v
drop table test.t
SELECT * FROM test.v2
CREATE OR REPLACE TABLE t2 AS SELECT random() FROM range(1000000)
CREATE OR REPLACE TABLE t2 AS SELECT * FROM range(1000000)
SELECT * from my_seq(0,10,2)
SELECT * FROM my_seq(0,3,2)
INSERT INTO tbl VALUES (5, 'test')
ALTER TABLE tbl DROP COLUMN gcol1
INSERT INTO b(id) VALUES (1)
set validate_external_file_cache='INVALID_VALUE'
CREATE OR REPLACE TABLE t( x VARCHAR USING COMPRESSION chimp )
CREATE OR REPLACE TABLE t( x BIGINT USING COMPRESSION Dictionary )
create table foo (str VARCHAR USING COMPRESSION 'dict_fsst')
SET force_bitpacking_mode='xxx'
SET SESSION force_bitpacking_mode = 'delta_for'
RESET SESSION force_bitpacking_mode
SELECT * FROM alp LIMIT 1
SELECT * FROM random_alp_double LIMIT 1
SELECT * FROM two_alp LIMIT 1
SELECT * FROM temperatures_double LIMIT 1
SELECT * FROM t LIMIT 1
CREATE INDEX db1.index ON test(a)
CREATE TABLE integers(i mood)
SELECT 'happy'::mood
SET default_block_size = '123456'
SET default_block_size = '128'
DETACH mydb
DETACH MyDB
FROM ddb
ATTACH ':memory:' AS new_database
ATTACH ':memory:'
SELECT * FROM blablabla
SELECT * FROM hello
ATTACH 'data/attach_test/encrypted_ctr_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde')
ATTACH 'data/attach_test/encrypted_gcm_key=abcde.db' as enc (ENCRYPTION_KEY 'abcde', ENCRYPTION_CIPHER 'GCM')
SELECT enum_range(NULL::xx.db1.main.mood) AS my_enum_range
SELECT * FROM other.dont_export_me
ATTACH ':memory:' AS db2 (TYPE getvariable('db_type'))
ATTACH 'mydb.db' AS db2
CREATE TABLE db1.song(songid INTEGER, songartist INTEGER, songalbum TEXT, songname TEXT, FOREIGN KEY(songartist, songalbum) REFERENCES album(artistid, albumname))
ATTACH 'dummy_extension:/hello.world'
ATTACH '~/home_dir.db' AS s2
set schema='schema2'
detach test
SELECT c FROM db1.s1.t, db2.s1.t
SELECT t.c FROM db1.s1.t, db2.s1.t
SELECT s1.t.c FROM db1.s1.t, db2.s1.t
ATTACH ':memory:' AS db1 (READONLY 1)
ATTACH ':memory:' AS db1 (BLABLABLA 1)
CREATE TABLE db1.test AS SELECT * FROM range(10) t(i)
CREATE TABLE test AS SELECT * FROM db1.test
INSERT INTO db1.integers VALUES (48)
ATTACH DATABASE ':memory:' AS temp
ATTACH DATABASE ':memory:' AS main
ATTACH DATABASE ':memory:' AS system
CREATE SCHEMA new_database.s1.xxx
CREATE SCHEMA IF NOT EXISTS new_database.s1.xxx
CREATE TABLE db1.integers(i INTEGER DEFAULT nextval('seq'))
CREATE TABLE integers(i INTEGER DEFAULT nextval('db1.seq'))
create table tbl(i int)
SELECT * FROM new_database.integers ORDER BY new_database.i
USE new_name.my_schema.my_table
USE blablabla
DETACH DATABASE system
DETACH DATABASE temp
CREATE SCHEMA system.eek
CREATE TABLE system.main.integers(i INTEGER)
CREATE VIEW system.main.integers AS SELECT 42
CREATE SEQUENCE system.main.seq
CREATE MACRO system.main.my_macro(a,b) AS a+b
CREATE TYPE system.main.rainbow AS ENUM ('red', 'orange', 'yellow', 'green', 'blue', 'purple')
FROM sql_auto_complete(NULL)
select flatten(['a', 'b', 'c']::varchar[3])
SELECT array_inner_product('foo', 'bar')
SELECT array_inner_product([1,2,3]::INT[3], ['a','b','c']::VARCHAR[3])
SELECT array_distance(['a','b']::VARCHAR[2],['foo','bar']::VARCHAR[2])
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 3)
SELECT array_length(array_value(array_value(1, 2, 2), array_value(3, 4, 3)), 0)
SELECT dayofweek(i) FROM intervals
SELECT isodow(i) FROM intervals
SELECT dayofyear(i) FROM intervals
SELECT week(i) FROM intervals
SELECT era(i) FROM intervals
SELECT julian(i) FROM intervals
SELECT extract(era from i) FROM intervals
SELECT extract(julian from i) FROM intervals
SELECT EXTRACT(dayofweek FROM i) FROM intervals
SELECT EXTRACT(isodow FROM i) FROM intervals
SELECT EXTRACT(dayofyear FROM i) FROM intervals
SELECT EXTRACT(week FROM i) FROM intervals
SELECT EXTRACT(yearweek FROM i) FROM intervals
SELECT EXTRACT(doy FROM interval '6 months ago')
SELECT EXTRACT(dow FROM interval '6 months ago')
SELECT datediff('microsecond', DATE '-290000-01-01', DATE '290000-01-01')
SELECT datetrunc('milliseconds', DATE '-2005205-7-28')
SELECT date_part('timezone', d) FROM dates
SELECT date_part('timezone_hour', d) FROM dates
SELECT date_part('timezone_minute', d) FROM dates
SELECT DATE_PART(['hour', 'minute'], '2023-09-17'::DATE) AS parts
SELECT date_trunc('duck', TIMESTAMP '2019-01-06 04:03:02') FROM timestamps LIMIT 1
SELECT strftime(d, d::VARCHAR) FROM dates ORDER BY d
SELECT strftime(DATE '1992-01-01', '%')
SELECT strftime(DATE '1992-01-01', '%R')
SELECT strptime('-1', '%g')
SELECT strptime('1000', '%g')
SELECT strftime('%Y', '1992-01-01')
SELECT strftime(date '-99999-01-01', random()::varchar)
select time_bucket('-3 hours'::interval, '2019-04-05'::date)
select time_bucket('-3 hours'::interval, '2019-04-05'::date, '1 hour 30 minutes'::interval)
select time_bucket('-3 hours'::interval, '2019-04-05'::date, '2019-04-05'::date)
select time_bucket('-1 month'::interval, '2019-04-05'::date)
select time_bucket('-1 month'::interval, '2019-04-05'::date, '1 week'::interval)
select time_bucket('-1 month'::interval, '2019-04-05'::date, '2019-04-05'::date)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05'::date)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05'::date, '1 hour 30 minutes'::interval)
SELECT enum_code('bla')
SELECT enum_first('bla')
SELECT enum_last('bla')
SELECT enum_range_boundary('orange'::rainbow, 'brl'::currency)
SELECT enum_range_boundary(NULL, NULL)
SELECT enum_range_boundary('orange'::rainbow, 1)
SELECT enum_range_boundary(1, 'orange'::rainbow)
SELECT era(i) FROM times
SELECT year(i) FROM times
SELECT month(i) FROM times
SELECT day(i) FROM times
SELECT decade(i) FROM times
SELECT century(i) FROM times
SELECT millennium(i) FROM times
SELECT quarter(i) FROM times
SELECT EXTRACT(year FROM i) FROM times
SELECT EXTRACT(month FROM i) FROM times
SELECT EXTRACT(day FROM i) FROM times
SELECT EXTRACT(decade FROM i) FROM times
SELECT EXTRACT(century FROM i) FROM times
SELECT EXTRACT(millennium FROM i) FROM times
SELECT EXTRACT(quarter FROM i) FROM times
SELECT EXTRACT(dayofweek FROM i) FROM times
SELECT even('abcd')
SELECT factorial(-1)
SELECT factorial(40)
SELECT gamma(0)
SELECT gamma('asdf')
SELECT lgamma(0)
SELECT lgamma('asdf')
SELECT gcd(42, 'abcd')
SELECT lcm(42, 'abcd')
select lcm(4200000000000000000,5700000000000000000)
SELECT log(0)
SELECT log(-1)
SELECT ln(0)
SELECT ln(-1)
SELECT log10(0)
SELECT log10(-1)
SELECT sqrt(-1)
select nextafter()
select nextafter('bla','bla')
select log(0, 64)
select log(2, 0)
select log(-1, 64)
select log(2, -1)
select log(1, 64)
select log('-Inf'::DOUBLE, 64)
select log(64, '-Inf'::DOUBLE)
select setseed(1.1)
select setseed(-1.1)
SELECT cast(ASIN(n)*1000 as bigint) FROM numbers ORDER BY n
select asin(-2)
select acos(-2)
SELECT 1::TINYINT + 1::VARCHAR
SELECT 1::SMALLINT + 1::VARCHAR
SELECT 1::INTEGER + 1::VARCHAR
SELECT 1::BIGINT + 1::VARCHAR
SELECT 1::REAL + 1::VARCHAR
SELECT 1::DOUBLE + 1::VARCHAR
SELECT -t from minima
SELECT -s from minima
SELECT -i from minima
SELECT -b from minima
SELECT +'hello'
SELECT -'hello'
SELECT +d FROM dates
SELECT -d FROM dates
SELECT array_length(ARRAY[1, 2, 3], 2)
SELECT array_length(ARRAY[1, 2, 3], 0)
SELECT array_to_string([1, 2, 3], k) FROM repeat(',', 5) t(k)
SELECT array_to_string_comma_default([1, 2, 3], sep:=k) FROM repeat(',', 5) t(k)
SELECT flatten(1)
select flatten(42)
select flatten([1, 2])
SELECT generate_series(timestamp '2020-01-01', timestamp '2020-06-01', interval '3' month - interval '3' day)
SELECT generate_series('294247-01-10'::TIMESTAMP, 'infinity'::TIMESTAMP, INTERVAL '1 DAY')
SELECT range('294247-01-10'::TIMESTAMP, 'infinity'::TIMESTAMP, INTERVAL '1 DAY')
SELECT generate_series('-infinity'::TIMESTAMP, '290309-12-22 (BC) 00:00:00'::TIMESTAMP, INTERVAL '1 DAY')
SELECT range('-infinity'::TIMESTAMP, '290309-12-22 (BC) 00:00:00'::TIMESTAMP, INTERVAL '1 DAY')
SELECT generate_subscripts([[1,2],[3,4],[5,6]], 2)
SELECT generate_series(timestamptz '2020-01-01', timestamptz '2020-06-01', interval '3' month - interval '3' day)
SELECT generate_series('294247-01-10'::TIMESTAMPTZ, 'infinity'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT range('294247-01-10'::TIMESTAMPTZ, 'infinity'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT generate_series('-infinity'::TIMESTAMPTZ, '290309-12-22 (BC) 00:00:00'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT range('-infinity'::TIMESTAMPTZ, '290309-12-22 (BC) 00:00:00'::TIMESTAMPTZ, INTERVAL '1 DAY')
SELECT quantile(NULL, filter(NULL, (lambda c103: 'babea54a-2261-4b0c-b14b-1d0e9b794e1a')))
SELECT list_concat([1, 2], 3)
SELECT i, list_concat(j, cast(k AS VARCHAR)) FROM lists
SELECT concat([42], [84], 'str')
SELECT list_contains([1.0,2.0,3.0], 'a')
SELECT list_contains('a', 'a')
SELECT list_contains([[1,2,3],[1],[1,2,3])
SELECT list_contains([[1,2,3],[1],[1,2,3]])
SELECT list_contains(1)
SELECT list_contains(1,1)
SELECT list_distinct()
SELECT list_distinct(*)
SELECT list_distinct([1, 2], 2)
SELECT list_distinct(NULL::boolean)
select list_has_any(l1) from list_of_strings
select list_has_any(l1, l2, l1) from list_of_strings
select list_has_all(l1) from list_of_strings
select list_has_all(l1, l2, l1) from list_of_strings
select list_has_all([1, 2], 1)
select list_has_any([[1,2], [2,4]], ['abc', 'def'])
select 'hello' && l1 from tbl
select 'hello' @> l1 from tbl
select list_intersect(l1) from list_of_strings
select list_intersect(l1, l2, l1) from list_of_strings
select list_intersect([[1,2], [2,4]], ['abc', 'def'])
SELECT list_position([1.0,2.0,3.0], 'a')
SELECT list_position('a', 'a')
SELECT list_position([[1,2,3],[1],[1,2,3])
SELECT list_position([[1,2,3],[1],[1,2,3]])
SELECT list_position(1)
SELECT list_position(1,1)
SELECT LIST_RESIZE([1, 2, 3], 9999999999999999999)
SELECT LIST_RESIZE([1, 2, 3], 4000999999999999999)
SELECT list_reverse()
SELECT list_reverse(42)
SELECT list_reverse ([1, 3, 2, 42, 117,, NULL])
SELECT list_reverse(*)
SELECT list_reverse([1, 2], 2)
SELECT list_unique()
SELECT list_unique(*)
SELECT list_unique([1, 2], 2)
SELECT list_unique(NULL::tinyint)
SELECT list_value([1, 2]::INTEGER[2], [3, 4, 5]::INTEGER[3], [6]::INTEGER[1])
SELECT list_value(a, b, c) FROM mixed_array_table
SELECT LIST_VALUE([1, 1], ['a', 'a'], [ROW(2, 2), ROW(3, 3)])
SELECT list_where([1,2,3], [True, NULL, FALSE])
SELECT list_zip('')
SELECT list_zip(3, 4)
SELECT list_zip(FALSE)
SELECT list_zip(TRUE)
SELECT repeat([1], 99999999999999999)
SELECT repeat([1, 2, 3], 6148914691236517206)
SELECT list_transform([2], lambda x: (SELECT 1 - x) * x)
SELECT list_filter([2], lambda x: (SELECT 1 - x) * x > 2)
SELECT list_filter([[1, 2, 1], [1, 2, 3], [1, 1, 1]], lambda x: list_contains_macro(x, 3))
SELECT list_transform([1], lambda x: x = UNNEST([1]))
SELECT list_filter([1], lambda x: x = UNNEST([1]))
SELECT list_transform([['abc']], lambda x: list_filter(x, lambda y: y))
SELECT list_reduce([1], x -> x, 3)
SELECT list_reduce([True], x -> x, x -> x)
SELECT [split('01:08:22', ':'), x -> CAST (x AS INTEGER)]
select list_apply(i, x -> x * 3 + 2 / zz) from (values (list_value(1, 2, 3))) tbl(i)
select x -> x + 1 from (values (list_value(1, 2, 3))) tbl(i)
select list_apply(i, y + 1 -> x + 1) from (values (list_value(1, 2, 3))) tbl(i)
SELECT list_apply(i, a.x -> x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
select list_apply(i, x -> x + 1 AND y + 1) from (values (list_value(1, 2, 3))) tbl(i)
SELECT list_apply(i, lambda a.x: a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
SELECT list_transform(qualified_tbl.x, lambda qualified_tbl.x: qualified_tbl.x + 1) FROM qualified_tbl
SELECT list_transform([1,2,3], lambda sqrt(xxx.z): xxx.z + 1) AS l
SELECT list_reduce([1, 2, 3, 4], lambda x *++++++++* y: x - y) AS l
CREATE MACRO my_macro(i) AS (SELECT i IN (SELECT i FROM test))
SELECT some_macro([1, 2], 3, 4)
SELECT other_reduce_macro([1, 2, 3, 4], 5, 6)
SELECT [a for a, b, c in [1, 2, 3]]
SELECT [a for a[1], b[2] in [1, 2, 3, 4]]
SELECT list_reduce([], lambda x, y, i: x + y + i)
SELECT list_reduce([1, 2, 3], lambda x, y: (x * y)::VARCHAR || 'please work')
SELECT list_reduce([1, 2], lambda x: x)
SELECT list_reduce([1, 2], NULL)
SELECt list_reduce([1, 2], (len('abc') AS x, y) - > x + y)
SELECT list_reduce(a, lambda x, y: x + y) FROM t1
SELECT list_reduce(a, lambda x, y: x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], lambda x, y: list_reduce([], lambda a, b: x + y + a + b))
SELECT list_reduce([1, 2, 3], lambda x, y: (x * y), 'i dare you to cast me')
SELECT list_reduce([1, 2], lambda x: x, 100)
SELECT list_reduce([1, 2], NULL, 100)
SELECt list_reduce([1, 2], (len('abc') AS x, y) - > x + y, 100)
SELECT list_reduce(l, lambda x, y: x + y) FROM t1
SELECT list_reduce(l, lambda x, y: x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], lambda x, y: list_reduce([], lambda a, b: x + y + a + b), 1000)
SELECT list_reduce([1, 2, 3], lambda x, y, x_i: list_reduce([], lambda a, b, a_i: x + y + a + b + x_i + a_i), 1000)
SELECT list_transform([[1], [4], NULL, [1], [8]], lambda x: list_concat( list_transform(x, lambda y: CASE WHEN y > 1 THEN 'yay' ELSE 'nay' END), x))
SELECT list_transform([2], x -> (SELECT 1 - x) * x)
SELECT list_filter([2], x -> (SELECT 1 - x) * x > 2)
SELECT list_filter([[1, 2, 1], [1, 2, 3], [1, 1, 1]], x -> list_contains_macro(x, 3))
SELECT list_transform([1], x -> x = UNNEST([1]))
SELECT list_filter([1], x -> x = UNNEST([1]))
SELECT list_transform([['abc']], x -> list_filter(x, y -> y))
SELECT list_apply(i, a.x -> a.x + 1) FROM (VALUES (list_value(1, 2, 3))) tbl(i)
SELECT list_transform(qualified_tbl.x, qualified_tbl.x -> qualified_tbl.x + 1) FROM qualified_tbl
SELECT list_transform([1,2,3], sqrt(xxx.z) -> xxx.z + 1) AS l
SELECT list_reduce([1, 2, 3, 4], x *++++++++* y -> x - y) AS l
create macro my_macro(i) as (select i in (select i from test))
select [a for a, b, c in [1,2,3]]
select [a for a[1], b[2] in [1,2,3,4]]
SELECT list_reduce([], (x, y, i) -> x + y + i)
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y)::VARCHAR || 'please work')
SELECT list_reduce([1, 2], (x) -> x)
SELECT list_reduce(a, (x, y) -> x + y) FROM t1
SELECT list_reduce(a, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b))
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i))
SELECT list_reduce(n, (x, y) -> list_reduce(l, (a, b) -> x + y + a + b)) FROM nested
SELECT list_reduce([1, 2, 3], (x, y) -> (x * y), 'i dare you to cast me')
SELECT list_reduce([1, 2], (x) -> x, 100)
SELECT list_reduce(l, (x, y) -> x + y) FROM t1
SELECT list_reduce(l, (x, y) -> x || ' ' || y) FROM t1
SELECT list_reduce([1, 2, 3], (x, y) -> list_reduce([], (a, b) -> x + y + a + b), 1000)
SELECT list_reduce([1, 2, 3], (x, y, x_i) -> list_reduce([], (a, b, a_i) -> x + y + a + b + x_i + a_i), 1000)
SELECT list_reduce(n, (x, y) -> list_reduce(l, (a, b) -> x + y + a + b), initial) FROM nested
SELECT list_reduce(n, (x, y) -> list_pack(list_reduce(x, (l, m) -> l + m) + list_reduce(y, (j, k) -> j + k)), initial) from nested
SELECT v.split(' ') strings, strings.apply(x -> x.lower()).filter(x -> x[1] == 't') lower, strings.apply(x -> x.upper()).filter(x -> x[1] == 'T') upper, lower + upper AS mix_case_srings FROM varchars
SELECT list_transform([[1], [4], NULL, [1], [8]], x -> list_concat(list_transform(x, y -> CASE WHEN y > 1 THEN 'yay' ELSE 'nay' END), x))
SELECT list_any_value()
SELECT list_avg()
SELECT list_bit_and()
SELECT list_bit_or()
SELECT list_bit_xor()
select list_bool_or()
select list_bool_and()
select list_count()
select list_entropy()
SELECT list_first()
select list_histogram()
SELECT list_aggr([1], 2)
SELECT list_aggr([1], True)
SELECT list_aggr([1], NULL)
SELECT list_aggr([1, 2, NULL], 'count_star')
SELECT list_aggr([1, 2, NULL], 'corr')
SELECT list_aggr([1, 2, NULL], 'covar_pop')
SELECT list_aggr([1, 2, NULL], 'covar_samp')
SELECT list_aggr([1, 2, NULL], 'regr_intercept')
select list_kurtosis([2e304, 2e305, 2e306, 2e307])
select list_kurtosis()
SELECT list_last()
SELECT list_mad([INTERVAL 1 YEAR])
SELECT list_mad([NULL::INTERVAL])
SELECT list_max()
SELECT list_min()
select list_mode()
select list_product()
select list_sem()
select list_skewness()
select list_skewness([-2e307, 0, 2e307])
SELECT list_string_agg()
SELECT sum_no_overflow(42)
SELECT sum_no_overflow(42.5)
select list_aggr([1e301, -1e301], 'stddev')
select list_var_samp([1e301, -1e301])
select list_var_pop([1e301, -1e301])
SELECT list_stddev_samp()
SELECT list_stddev_pop(c0) FROM t0
select col.almost_a_number::BIGINT from tbl order by all
SELECT cast_to_type('hello', NULL::INT)
SELECT cast_to_type(42, NULL)
SELECT constant_or_null(1)
SELECT constant_or_null()
SELECT error('test')
SELECT CASE WHEN value = 'foo' THEN 'Value is foo.' ELSE ERROR(CONCAT('Found unexpected value: ', value)) END AS new_value FROM ( SELECT 'foo' AS value UNION ALL SELECT 'baz' AS value)
SELECT * FROM (SELECT 3 AS x) WHERE IF(x % 2 = 0, true, ERROR(FORMAT('x must be even but is {}', x)))
SELECT 42=error('hello world')
SELECT error('hello world') IS NULL
SELECT HASH()
SELECT r, HASH() FROM enums
SELECT replace_type('hello', NULL::VARCHAR, NULL::INT)
SELECT replace_type(42, NULL::INTEGER, NULL)
EXECUTE v1(1, 2)
SELECT LEAST(DATE '1992-01-01', 'hello', 123)
SELECT CURRENT_SETTING('a')
SELECT CURRENT_SETTING('memori_limit')
SELECT CURRENT_SETTING(i::VARCHAR) FROM range(1) tbl(i)
SELECT CURRENT_SETTING(NULL)
SELECT CURRENT_SETTING(CAST(NULL AS TEXT))
SELECT CURRENT_SETTING('')
SET default_null_order = colref || '_last'
SET default_null_order = (SELECT 'nulls_last')
SELECT from_hex('duckdb')
SELECT parse_formatted_bytes('null')
SELECT parse_formatted_bytes('none')
SELECT parse_formatted_bytes('5')
SELECT parse_formatted_bytes('abc')
SELECT parse_formatted_bytes('1 Ki')
SELECT parse_formatted_bytes(1933)
SELECT parse_formatted_bytes('10000000000 TiB')
SELECT parse_formatted_bytes('1.5.3 GB')
SELECT parse_path()
SELECT parse_path('/path/to', true, 'system')
SELECT parse_dirname()
SELECT parse_dirname('/path/to', true, 'system')
SELECT parse_dirpath()
SELECT parse_dirpath('/path/to', true, 'system')
SELECT parse_filename(true)
SELECT parse_filename('path/to/file.csv', 'system', true)
SELECT parse_filename()
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', []) AS groups FROM filenames
WITH patterns AS ( SELECT 'rundate_(\d+-\d+-\d+)_pass_(\d+)' AS pattern FROM range(3) ) SELECT regexp_extract(filename, pattern, ['rundate', 'pass']) AS groups FROM filenames, patterns
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', NULL]) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'rundate']) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'RUNDATE']) AS groups FROM filenames
SELECT regexp_extract(filename, 'rundate_(\d+-\d+-\d+)_pass_(\d+)', ['rundate', 'pass', 'overflow']) AS groups FROM filenames
SELECT regexp_extract(filename, NULL, ['rundate', 'pass']) AS groups FROM filenames
SELECT regexp_extract('foobarbaz', '(b..)(b..)', -1)
SELECT regexp_extract('foobarbaz', '(b..)(b..)', 42)
SELECT regexp_extract(s, p, i) FROM test
SELECT regexp_extract(s, '(b..)(b..)', i) FROM test
SELECT regexp_extract('foobarbaz', 'b..', '1')
select regexp_extract_all('', '(')
SELECT str, REGEXP_EXTRACT_ALL(str,'ab++') AS m1_long, FROM ( VALUES ('acd'), ('abcd'), ('abbcd'), ('abbbcd') ) AS t(str)
select REGEXP_EXTRACT_ALL('hello', '.', 2)
SELECT regexp_extract_all('abc', '(a)(b)(c)', [])
SELECT regexp_extract_all('abc', '(a)(b)(c)', ['x','x'])
SELECT regexp_extract_all('abc', '(a)(b)', ['g1','g2','g3'])
SELECT regexp_extract_all('abc', NULL, ['g'])
WITH params(name_list) AS (SELECT ['g1','g2']) SELECT regexp_extract_all('abc', '(a)(b)', name_list) FROM params
SELECT regexp_extract_all('abc', '(a)', NULL::VARCHAR[])
SELECT regexp_extract_all('abc', '(a)(b)', []::VARCHAR[])
SELECT regexp_extract_all('abc', '(a)', ['g1', NULL::VARCHAR])
SELECT regexp_replace(v, 'h.*', 'world', v) FROM test ORDER BY v
SELECT regexp_replace('asdf', '.*SD.*', 'a', 'q')
select regexp_matches('abc', '*')
select regexp_replace('abc', '*', 'X')
select regexp_matches(s, r) from regex
select regexp_replace(s, r, 'X') from regex
SELECT regexp_matches('', '\X')
SELECT regexp_matches(c0, '.*SD.*', NULL) from t0
SELECT regexp_matches(v, 'h.*', v) FROM test ORDER BY v
SELECT regexp_matches(c0, '.*SD.*', 'q') from t0
SELECT regexp_matches(c0, '.*SD.*', 'g') from t0
SELECT regexp_matches(s, p) FROM regex
SELECT sha1()
SELECT sha1(42)
SELECT sha256()
SELECT array_extract('1', 9223372036854775807)
SELECT array_extract('0', -9223372036854775808)
SELECT ASCII()
SELECT CHR(-10)
SELECT CHR(1073741824)
SELECT CHR()
select bar(1, '-infinity'::double, 10)
select bar(1, 0, 10, 'nan'::double)
select bar(1, 0, 10, 'infinity'::double)
select bar(1, 0, 10, '-infinity'::double)
select bar(1, 0, 10, 1001)
select bar(1, 0, 10, 0.99)
select BIT_LENGTH()
select BIT_LENGTH(1, 2)
SELECT CONCAT()
select concat([1], 'hello')
SELECT list_concat([1, 2], ['3', '4'])
SELECT list_concat([1, 2], 4)
select CONCAT_WS()
select CONCAT_WS(',')
SELECT contains(NULL,NULL) FROM strings
SELECT damerau_levenshtein('one', 'two', 'three')
SELECT damerau_levenshtein('one')
SELECT damerau_levenshtein()
SELECT format('{}')
SELECT format('{} {}', 'hello')
SELECT format('{:s}', 42)
SELECT format('{:d}', 'hello')
select format('{:t}', 123456789)
select format('{1}', 123456789)
select printf('%:', 123456789)
select printf('%:', 123456789.123)
select printf('%:', 'str')
select 'a%c' ilike 'a$%C' escape '///'
SELECT str ILIKE pat ESCAPE str FROM tbl
SELECT jaccard('hello', '')
SELECT jaccard('', 'hello')
SELECT jaccard('', '')
select round(jaccard('', t), 1) from strings
select round(jaccard(s, ''), 1) from strings
SELECT 'hello' LIKE 'hê?llo' COLLATE idontexist
SELECT '%' LIKE '%' ESCAPE '%'
SELECT '%' LIKE '*' ESCAPE '*'
SELECT '%_' LIKE '%_' ESCAPE '\\'
SELECT '%_' LIKE '%_' ESCAPE '**'
SELECT mismatches('', '')
SELECT mismatches('hoi', 'hallo')
SELECT mismatches('hallo', 'hoi')
SELECT mismatches('', 'hallo')
SELECT mismatches('hi', '')
SELECT mismatches('', s) FROM strings ORDER BY s
SELECT mismatches(s, '') FROM strings ORDER BY s
SELECT mismatches(s, 'hallo') FROM strings
select LPAD()
select LPAD(1)
select LPAD(1, 2)
select LPAD('Hello', 10, '')
select LPAD('a', 100000000000000000, 0)
select RPAD()
select RPAD(1)
select RPAD(1, 2)
SELECT printf('%s')
SELECT printf('%s %s', 'hello')
SELECT printf('%s', 42)
SELECT printf('%d', 'hello')
SELECT printf(fmt) FROM strings ORDER BY idx
select REPEAT()
select REPEAT(1)
select REPEAT('hello', 'world')
select REPEAT('hello', 'world', 3)
select REPLACE(1)
select REPLACE(1, 2)
select REPLACE(1, 2, 3, 4)
select REVERSE()
select REVERSE(1, 2)
select REVERSE('hello', 'world')
SELECT right_grapheme('a', 9223372036854775808)
SELECT s FROM strings WHERE s SIMILAR TO 'ab.*%' {escape ''}
select split_part()
select split_part('a')
select split_part('a','a')
SELECT NULL::VARCHAR[off:length+off] FROM strings
SELECT NULL::VARCHAR[NULL:length+NULL] FROM strings
SELECT NULL::VARCHAR[off:NULL+off] FROM strings
SELECT NULL::VARCHAR[NULL:NULL+NULL] FROM strings
select string_split()
select string_split('a')
SELECT string_split_regex(a, '[') FROM test ORDER BY a
SELECT NULL::VARCHAR[off] FROM strings
SELECT NULL::VARCHAR[NULL] FROM strings
SELECT to_base(-10, 2)
SELECT to_base(-10, 2, 64)
SELECT to_base(10, 1)
SELECT to_base(10, 37)
SELECT to_base(10, 0, 10)
SELECT to_base(10, 37, 10)
SELECT to_base(10, 2, -10)
select TRANSLATE(1)
select TRANSLATE(1, 2)
select TRANSLATE(1, 2, 3, 4)
select LTRIM()
select LTRIM('hello', 'world', 'aaa')
select RTRIM()
select RTRIM('hello', 'world', 'aaa')
select TRIM()
select TRIM('hello', 'world', 'aaa')
select UNICODE()
select UNICODE(1, 2)
select url_decode('%FF%FF%FF')
SELECT (ROW(42, 84))['element']
SELECT (ROW(42, 84))[0]
SELECT (ROW(42, 84))[9999]
SELECT (ROW(42, 84))[-1]
SELECT (ROW(42, 84))[9223372036854775807]
SELECT (ROW(42, 84))[(-9223372036854775808)::BIGINT]
SELECT struct_insert()
SELECT struct_insert({a: 1, b: 2})
SELECT struct_insert(123, a := 1)
SELECT struct_insert({a: 1, b: 2}, a := 2)
SELECT struct_update()
SELECT struct_update({a: 1, b: 2})
SELECT struct_update(123, a := 1)
SELECT struct_update({a: 1, b: 2}, a := 2, a := 3)
SELECT to_timestamp(1284352323::DOUBLE * 100000000)
SELECT make_timestamp(9223372036854775807)
SELECT make_timestamp_ns(9223372036854775807)
SELECT make_timestamp(294247, 1, 10, 4, 0, 54.775807)
SELECT ts, DATE_PART(['duck', 'month', 'day'], ts) AS parts FROM timestamps ORDER BY 1
SELECT ts, DATE_PART(['year', 'month', 'day', 'year'], ts) AS parts FROM timestamps ORDER BY 1
SELECT DATE_PART([], ts) FROM timestamps
SELECT DATE_PART(['year', NULL, 'month'], ts) FROM timestamps
WITH parts(p) AS (VALUES (['year', 'month', 'day']), (['hour', 'minute', 'microsecond'])) SELECT DATE_PART(p, ts) FROM parts, timestamps
select 'epoch'::timestamptz + '9223372036854775000 microseconds'::interval
select 'epoch'::timestamptz + '-9223372022400001001 microseconds'::interval
select '9223372036854775000 microseconds'::interval + 'epoch'::timestamptz
select '-9223372022400001001 microseconds'::interval + 'epoch'::timestamptz
select 'epoch'::timestamptz - '9223372022400001001 microseconds'::interval
SELECT ts + (INTERVAL (-1) year) FROM limits WHERE label = 'tsmin'
SELECT ts + (INTERVAL (-1) month) FROM limits WHERE label = 'tsmin'
SELECT ts + (INTERVAL (-15) month) FROM limits WHERE label = 'tsmin'
SELECT DATE_PART(['duck', 'minute', 'microsecond', 'timezone'], ts), ts FROM timestamps ORDER BY 2
SELECT DATE_PART(['era', 'year', 'month', 'era'], ts), ts FROM timestamps ORDER BY 2
SELECT date_trunc('duck', TIMESTAMPTZ '2019-01-06 04:03:02-08') FROM timestamps LIMIT 1
SELECT ts, make_timestamptz(yyyy, mm, dd, hr, mn, ss, 'Europe/Duck') mts FROM timeparts
WITH all_types AS ( select * exclude(small_enum, medium_enum, large_enum) from test_all_types() ) SELECT make_timestamptz( CAST(century(CAST(a."interval" AS INTERVAL)) AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(a."bigint" AS BIGINT), CAST(txid_current() AS BIGINT), 'UTC') FROM all_types a
SELECT make_timestamptz(9223372036854775807)
SELECT make_timestamptz(294248, 1, 10, 4, 0, 54.775807)
SELECT ts, strftime(ts, '%C') FROM timestamps
SELECT strptime(s, f) FROM multiples
select strptime('2022-03-05 17:59:17.877 CST', '%C')
select strptime('2022-03-05 17:59:17.877 CST', '%Y-%m-%d %H:%M:%S.%g')
select 'fnord'::timestamptz
SELECT TIMESTAMPTZ '294247-01-10 04:00:54.7758'
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz, '1 hour 30 minutes'::interval)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00+03'::timestamptz, '2019-04-05 00:00:00+03'::timestamptz)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz, '1 hour 30 minutes'::interval)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00-11'::timestamptz, '2018-04-05 00:00:00+11'::timestamptz)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00+07'::timestamptz)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00+07'::timestamptz, '1 hour 30 minutes'::interval)
SELECT strptime('', '')
SELECT strptime(NULL, '')
SELECT strptime('10.28.1910', ['%d-%m-%Y', '%m-%d-%Y', '%d/%m/%Y', '%m/%d/%Y'])
SELECT strptime('Mon Oct 17 2022 22:00:00 GMT+0000 (GMT)', '%a %b %d %Y %X GMT%z (%Z') as broken
select strptime('2020-12-31 21:25:58.745232+0', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+0X', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+X0', '%Y-%m-%d %H:%M:%S.%f%z')
select strptime('2020-12-31 21:25:58.745232+000', '%Y-%m-%d %H:%M:%S.%f%z')
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
select time_bucket('-3 hours'::interval, '2019-04-05 00:00:00'::timestamp, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
select time_bucket('-1 month'::interval, '2019-04-05 00:00:00'::timestamp, '2018-04-05 00:00:00'::timestamp)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00'::timestamp)
select time_bucket('1 day - 172800 seconds'::interval, '2018-05-05 00:00:00'::timestamp, '1 hour 30 minutes':: interval)
SELECT try_strptime('', '')
SELECT try_strptime(NULL, '')
SELECT try_strptime('21/10/2018', '%-q/%m/%Y')
SELECT try_strptime('2000/10/10', random()::varchar)
SELECT era(d) FROM timetzs
SELECT year(d) FROM timetzs
SELECT month(d) FROM timetzs
SELECT day(d) FROM timetzs
SELECT decade(d) FROM timetzs
SELECT century(d) FROM timetzs
SELECT millennium(d) FROM timetzs
SELECT quarter(d) FROM timetzs
insert into timetzs values ('2402:30:00+1200')
SELECT uuid_extract_timestamp(uuidv4())
SELECT from_base64('ab')
SELECT from_base64('üab')
select array_slice('hello world', 1, 8, 2)
SELECT 1::TINYINT << -1::TINYINT, 1::TINYINT >> -1::TINYINT, 1::TINYINT << 12::TINYINT, 1::TINYINT >> 12::TINYINT
SELECT 1::SMALLINT << -1::SMALLINT, 1::SMALLINT >> -1::SMALLINT, 1::SMALLINT << 20::SMALLINT, 1::SMALLINT >> 20::SMALLINT
SELECT 1::INT << -1::INT, 1::INT >> -1::INT, 1::INT << 40::INT, 1::INT >> 40::INT
SELECT 1::BIGINT << -1::BIGINT, 1::BIGINT >> -1::BIGINT, 1::BIGINT << 1000::BIGINT, 1::BIGINT >> 1000::BIGINT
SELECT 'hello' << 3
SELECT 3 << 'hello'
SELECT 2.0 << 1
SELECT 1::UINT32 << 32
select ('abc' between 20 and True)
select 'abc' > 10
select 20.0 = 'abc'
SELECT '294247-01-10'::DATE + '04:00:54.775808'::TIME
SELECT (-128)::TINYINT // (-1)::TINYINT
SELECT (-32768)::SMALLINT // (-1)::SMALLINT
SELECT (-2147483648)::INTEGER // (-1)::INTEGER
SELECT (-9223372036854775808)::BIGINT // (-1)::BIGINT
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region, too_many_names) IN ((1, 'east') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT year, q1_east, q1_west, q2_east, q2_west, q3_east, q3_west, q4_east, q4_west FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east', 'west') AS q1_east, (1, 'west') AS q1_west, (2, 'east') AS q2_east, (2, 'west') AS q2_west, (3, 'east') AS q3_east, (3, 'west') AS q3_west, (4, 'east') AS q4_east, (4, 'west') AS q4_west))
SELECT * FROM sales PIVOT (sum(sales) FOR (quarter, region) IN ((1, 'east') AS q1_east, (1, 'east') AS q1_east_2))
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN unique_monthsx) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN not_an_enum) AS p ORDER BY EMPID
PIVOT test ON j IN ('a', 'b') USING SUM(test.i)
PIVOT test ON j IN ('a', 'b') USING get_current_timestamp()
PIVOT test ON j IN ('a', 'b') USING sum(41) over ()
PIVOT test ON j IN ('a', 'b') USING sum(sum(41) over ())
FROM tbl PIVOT (c FOR IN enum_val)
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + COUNT(*) GROUP BY Year
PIVOT Cities ON Country || '_' || Name USING SUM(Population) + Population GROUP BY Year
PIVOT Cities ON min(Country) over () USING SUM(Population) GROUP BY Year
PIVOT Cities ON min(Country) USING SUM(Population) GROUP BY Year
PIVOT Cities ON NULL USING SUM(Population) GROUP BY Year
PIVOT Cities ON 'hello world' USING SUM(Population) GROUP BY Year
PIVOT Cities ON (SELECT COUNTRY) USING SUM(Population) GROUP BY Year
PIVOT Cities ON Year IN (SELECT xx FROM Cities) USING SUM(Population)
PREPARE v3 AS PIVOT (SELECT empid, amount + ? AS amount, month FROM monthly_sales) ON MONTH USING SUM(AMOUNT)
CREATE VIEW pivot_view AS PIVOT (SELECT YEAR(d) AS year, MONTH(d) AS month, empid, amount FROM sales) ON YEAR, MONTH USING SUM(AMOUNT)
CREATE MACRO xt2(a) as TABLE PIVOT sales ON d USING SUM(amount)
CREATE MACRO xt2(a) as (PIVOT sales ON d USING SUM(amount))
SELECT * FROM sales PIVOT( SUM(amount) FOR YEAR IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) MONTH IN ('JAN', 'FEB', 'MAR', 'APR') amount IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) empid IN (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20) ) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ('JAN', 'JAN')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(COS(amount) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + (SELECT 42)) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount + row_number() over ()) FOR MONTH IN ('JAN', 'FEB', 'MAR', 'APR')) AS p (EMP_ID_renamed, JAN, FEB, MAR, APR) ORDER BY EMP_ID_renamed
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTHx IN ('JAN', 'FEB', 'MAR', 'DEC')) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN ()) AS p ORDER BY EMPID
SELECT * FROM monthly_sales PIVOT(SUM(amount) FOR MONTH IN (*)) AS p ORDER BY EMPID
SELECT * FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR (month, month2) IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT(sales_jan_feb FOR month IN ((jan, feb), (mar, april)))
SELECT * FROM monthly_sales UNPIVOT((a, b, c) FOR month IN ((jan, feb), (mar, april)))
SELECT empid, dept, month, sales_jan_feb, sales_mar_apr FROM monthly_sales UNPIVOT((sales_jan_feb, sales_mar_apr) FOR month IN ((jan, feb), mar))
SELECT empid, dept, april, month, sales FROM monthly_sales UNPIVOT(sales FOR month IN (jan, feb, mar, dec)) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN (empid, dept, jan, feb, mar, april))
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ()) ORDER BY empid
SELECT * FROM monthly_sales UNPIVOT(sales FOR month IN ('')) ORDER BY empid
CREATE VIEW v1 AS PIVOT monthly_sales ON MONTH USING SUM(AMOUNT)
unpivot (select 42 as col1, 'woot' as col2) on (col1 + (SELECT col1))::VARCHAR, col2
unpivot (select 42 as col1, 'woot' as col2) on random(), col2
unpivot (select 42 as col1, 'woot' as col2) on col1 + col2
unpivot (select 42 as col1, 'woot' as col2) on t.col1::VARCHAR, col2
unpivot integers on columns(* exclude (i))
UNPIVOT test ON (metric_1, value_x), metric_2, metric_3
UNPIVOT test ON (metric_1, value_x), (metric_2, value_q), (metric_3, value_j) INTO NAME metric VALUE metric_value
select ?
create view v1 as select ?
execute v1(0)
execute v2('hello')
EXECUTE v2([[1, 2, 3]], [1, 2, 3])
EXECUTE v7('hello world', [1, 2, 3])
execute v1(1665)
execute v1('1665')
execute v1('1665'::VARCHAR)
execute v1(1665::SHORT)
EXECUTE v3(-1)
execute q123(param := 5, 3)
execute q01(4, 2, 0)
prepare q02 as select $1, $param, $2
execute q01(a, 2, 0)
EXECUTE v1(INTERVAL '1' DAY)
EXECUTE v2(INTERVAL '1' DAY)
EXECUTE s1(43)
EXECUTE s1(43, 'asdf', 42)
EXECUTE s1('asdf', 'asdf')
PREPARE EXPLAIN SELECT 42
PREPARE CREATE TABLE a(i INTEGER)
EXECUTE v5(4)
EXECUTE v6('hello')
EXECUTE v18([1])
EXECUTE v19(0)
SELECT * FROM a WHERE i=$1
SELECT * FROM a WHERE i=CAST($1 AS VARCHAR)
SELECT -v FROM vals
SET disable_database_invalidation=true
SET allow_unredacted_secrets=true
PRAGMA disabled_compression_methods='uncompressed,rle'
PRAGMA disabled_compression_methods='xzx'
PRAGMA enable_profiling()
PRAGMA enable_profiling='unsupported'
PRAGMA profiling_output
PRAGMA force_compression='unknown'
PRAGMA memory_limit=100
PRAGMA memory_limit='0.01BG'
PRAGMA memory_limit='0.01BLA'
PRAGMA memory_limit='0.01PP'
PRAGMA memory_limit='0.01TEST'
PRAGMA memory_limit
PRAGMA memory_limit()
PRAGMA memory_limit(1, 2)
FROM pragma_metadata_info(NULL)
PRAGMA database_list()
PRAGMA
PRAGMA random_unknown_pragma
PRAGMA table_info = 3
DESCRIBE my_index
SHOW TABLES FROM nonexistent_db
SHOW TABLES FROM main.nonexistent_schema
PRAGMA storage_info('v1')
PRAGMA storage_info('bla')
PRAGMA table_info('nonexistant_table')
PRAGMA table_info(1,2,3)
PRAGMA explain_output='unknown'
PRAGMA debug_checkpoint_abort='unknown'
pragma explain_output =null
CALL enable_profiling(format='i dont exist hehe')
CALL enable_profiling(coverage='i dont exist hehe')
CALL enable_profiling(mode='i dont exist hehe')
CALL enable_profiling(mode='true')
CALL enable_profiling(metrics = "hello")
CALL enable_profiling(metrics = ['LATENCY', 'RESULT_SET_SIZE' = true])
CALL enable_profiling(metrics = 'QUERY_NAME': true, 'EXTRA_INFO': true, 'OPERATOR_ROWS_SCANNED': false, "CUMULATIVE_CARDINALITY": "true")
CALL enable_profiling(mode = 'detailed', ['LATENCY', 'RESULT_SET_SIZE'])
SELECT cpu_time FROM metrics_output
PRAGMA custom_profiling_settings='}}}}}}'
PRAGMA custom_profiling_settings=BONJOUR
PRAGMA custom_profiling_settings=[NOT_A_JSON]
PRAGMA custom_profiling_settings='{"INVALID_SETTING": "true"}'
SELECT total_memory_allocated FROM metrics_children
SELECT total_memory_allocated FROM metrics_output
SELECT extra_info FROM metrics_output
SELECT operator_cardinality FROM metrics_output
SELECT operator_timing FROM metrics_output
SELECT cumulative_cardinality FROM metrics_output
PRAGMA enable_profiling = 'html'
PRAGMA enable_profiling = 'db'
SET access_mode='read_only'
SET allowed_configs=['lock_configuration']
SET allowed_configs=['allowed_configs']
SET allowed_configs=['']
SET allowed_configs=['not_a_real_setting']
SET allowed_configs=['threads']
RESET allowed_configs
SET lock_configuration=false
SET Calendar='japanese'
SET allowed_directories=[]
COPY (SELECT 42 i) TO 'permission_test.csv' (FORMAT csv)
COPY integers FROM 'permission_test.csv'
ATTACH 'test.db'
LOAD my_ext
INSTALL my_ext
EXPORT DATABASE a1 TO 'export_test'
IMPORT DATABASE 'export_test'
SET allowed_paths=[]
SET block_allocator_memory='-3%'
SET block_allocator_memory='150%'
SET block_allocator_memory='50MiB'
SET block_allocator_memory='200TiB'
SELECT * FROM nonexistent_table
SELECT cbl FROM (VALUES (42)) t(col)
SECT cbl FROM (VALUES (42)) t(col)
select corr('hello', 'world')
CREATE SCHEMA temp.s1
CREATE SCHEMA system.s1
set schema = 'temp'
set schema = 'system'
PRAGMA default_collation='unknown'
SET disabled_optimizers TO 'expression_rewriteX'
SET disabled_optimizers TO 'unknown_optimizer'
SET debug_window_mode='unknown'
SET default_order='unknown'
SET enable_profiling='unknown'
SET GLOBAL enable_progress_bar=true
SET explain_output='unknown'
SET profiling_mode='unknown'
SET TimeZone='Pacific/Honolooloo'
SET Calendar='muslim'
SET disabled_filesystems='LocalFileSystem,LocalFileSystem'
SELECT * FROM abc.main.t1
SELECT * FROM abc.t1
SELECT * FROM abc.csv
SELECT * FROM duckdb_secrets()
SELECT * FROM duckdb_extensions()
CREATE PERSISTENT SECRET my_s (TYPE S3)
CREATE PERSISTENT SECRET my_secret (TYPE S3)
SET memory_limit='10GB'
RESET memory_limit
SET custom_user_agent='something else'
RESET custom_user_agent
SET duckdb_api='something else'
CALL dbgen(sf=1)
CALL dbgen(sf=0, catalog='dbgentest')
PRAGMA tpch(-1)
PRAGMA tpch(3290819023812038903)
PRAGMA tpch(32908301298)
PRAGMA tpch(1.1)
CALL dsdgen(sf=0)
CALL dsdgen(sf=0, catalog='dsdgentest')
PRAGMA tpcds(-1)
PRAGMA tpcds(3290819023812038903)
PRAGMA tpcds(32908301298)
PRAGMA tpcds(1.1)
