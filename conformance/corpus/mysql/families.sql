-- MySQL top-level statement-family probe corpus (self-authored; see README.md).
--
-- Each `-- family:` header names one top-level statement family; the single
-- non-comment line that follows is that family's minimal authored probe. Blank
-- lines and every other `--` comment line are ignored by the loader
-- (conformance/src/corpus_mysql_verdicts.rs).
--
-- The family SET is derived from a local, read-only reading of MySQL 8.4.10's
-- GPL `sql_yacc.yy` `simple_statement` production for the FACT of which top-level
-- statement families exist — no grammar bytes are copied. Every probe below is
-- ORIGINAL text authored here to be the smallest statement that reaches its
-- family. See README.md and PROVENANCE.toml for the licensing boundary.
--
-- Every probe is verified against the live MySQL oracle (m3, PREPARE-only): the
-- server must recognize the family (parse without ER_PARSE_ERROR 1064). A probe
-- may PREPARE (accept), report ER_UNSUPPORTED_PS 1295 (grammar-valid but not
-- preparable — the large administrative/stored-program surface), or bind-reject a
-- placeholder name (`zzp_*`); all three are positive grammar evidence. Created
-- objects never persist: the oracle only PREPAREs, never executes, so the `zzp_*`
-- names below are never actually created.

-- ==== Query & DML ====
-- family: SELECT
SELECT 1
-- family: TABLE
TABLE t1
-- family: VALUES
VALUES ROW(1, 2)
-- family: INSERT
INSERT INTO t1 (a) VALUES (1)
-- family: REPLACE
REPLACE INTO t1 (a) VALUES (1)
-- family: UPDATE
UPDATE t1 SET a = a + 1
-- family: DELETE
DELETE FROM t1 WHERE a = 1
-- family: DO
DO 1 + 1
-- family: HANDLER
HANDLER t1 OPEN
-- family: LOAD DATA
LOAD DATA INFILE 'zzp_data.tsv' INTO TABLE t1
-- family: LOAD XML
LOAD XML INFILE 'zzp_data.xml' INTO TABLE t1
-- family: LOAD INDEX INTO CACHE
LOAD INDEX INTO CACHE t1
-- family: CACHE INDEX
CACHE INDEX t1 IN zzp_keycache

-- ==== Prepared statements ====
-- family: PREPARE
PREPARE zzp_ps FROM 'SELECT 1'
-- family: EXECUTE
EXECUTE zzp_ps
-- family: DEALLOCATE PREPARE
DEALLOCATE PREPARE zzp_ps

-- ==== Transactions, session & locking ====
-- family: START TRANSACTION
START TRANSACTION
-- family: COMMIT
COMMIT
-- family: ROLLBACK
ROLLBACK
-- family: SAVEPOINT
SAVEPOINT zzp_sp
-- family: RELEASE SAVEPOINT
RELEASE SAVEPOINT zzp_sp
-- family: LOCK TABLES
LOCK TABLES t1 READ
-- family: UNLOCK TABLES
UNLOCK TABLES
-- family: LOCK INSTANCE
LOCK INSTANCE FOR BACKUP
-- family: UNLOCK INSTANCE
UNLOCK INSTANCE
-- family: XA
XA START 'zzp_xid'
-- family: SET
SET @zzp_v = 1
-- family: SET TRANSACTION
SET TRANSACTION ISOLATION LEVEL READ COMMITTED
-- family: USE
USE zzp_db

-- ==== DDL: CREATE ====
-- family: CREATE DATABASE
CREATE DATABASE zzp_db
-- family: CREATE TABLE
CREATE TABLE zzp_t (a INT PRIMARY KEY)
-- family: CREATE INDEX
CREATE INDEX zzp_ix ON t1 (a)
-- family: CREATE VIEW
CREATE VIEW zzp_v AS SELECT 1 AS a
-- family: CREATE TRIGGER
CREATE TRIGGER zzp_tr BEFORE INSERT ON t1 FOR EACH ROW BEGIN END
-- family: CREATE PROCEDURE
CREATE PROCEDURE zzp_p() BEGIN END
-- family: CREATE FUNCTION
CREATE FUNCTION zzp_f() RETURNS INT DETERMINISTIC RETURN 1
-- family: CREATE EVENT
CREATE EVENT zzp_e ON SCHEDULE AT NOW() DO SET @zzp_v = 1
-- family: CREATE USER
CREATE USER zzp_u@localhost
-- family: CREATE ROLE
CREATE ROLE zzp_r
-- family: CREATE SERVER
CREATE SERVER zzp_srv FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'zzp_h')
-- family: CREATE TABLESPACE
CREATE TABLESPACE zzp_ts ADD DATAFILE 'zzp_ts.ibd'
-- family: CREATE LOGFILE GROUP
CREATE LOGFILE GROUP zzp_lg ADD UNDOFILE 'zzp_undo.dat'
-- family: CREATE SPATIAL REFERENCE SYSTEM
CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'zzp_srs' DEFINITION 'LOCAL_CS["z"]'
-- family: CREATE RESOURCE GROUP
CREATE RESOURCE GROUP zzp_rg TYPE = USER

-- ==== DDL: ALTER ====
-- family: ALTER DATABASE
ALTER DATABASE zzp_db DEFAULT CHARACTER SET utf8mb4
-- family: ALTER TABLE
ALTER TABLE t1 ADD COLUMN zzp_c INT
-- family: ALTER VIEW
ALTER VIEW zzp_v AS SELECT 2 AS a
-- family: ALTER EVENT
ALTER EVENT zzp_e DISABLE
-- family: ALTER PROCEDURE
ALTER PROCEDURE zzp_p COMMENT 'zzp'
-- family: ALTER FUNCTION
ALTER FUNCTION zzp_f COMMENT 'zzp'
-- family: ALTER USER
ALTER USER zzp_u@localhost IDENTIFIED BY 'zzp_pw'
-- family: ALTER SERVER
ALTER SERVER zzp_srv OPTIONS (HOST 'zzp_h2')
-- family: ALTER TABLESPACE
ALTER TABLESPACE zzp_ts ADD DATAFILE 'zzp_ts2.ibd'
-- family: ALTER UNDO TABLESPACE
ALTER UNDO TABLESPACE zzp_ut SET INACTIVE
-- family: ALTER LOGFILE GROUP
ALTER LOGFILE GROUP zzp_lg ADD UNDOFILE 'zzp_undo2.dat' ENGINE = InnoDB
-- family: ALTER INSTANCE
ALTER INSTANCE RELOAD TLS
-- family: ALTER RESOURCE GROUP
ALTER RESOURCE GROUP zzp_rg VCPU = 0

-- ==== DDL: DROP ====
-- family: DROP DATABASE
DROP DATABASE zzp_db
-- family: DROP TABLE
DROP TABLE zzp_t
-- family: DROP INDEX
DROP INDEX zzp_ix ON t1
-- family: DROP VIEW
DROP VIEW zzp_v
-- family: DROP TRIGGER
DROP TRIGGER zzp_tr
-- family: DROP PROCEDURE
DROP PROCEDURE zzp_p
-- family: DROP FUNCTION
DROP FUNCTION zzp_f
-- family: DROP EVENT
DROP EVENT zzp_e
-- family: DROP USER
DROP USER zzp_u@localhost
-- family: DROP ROLE
DROP ROLE zzp_r
-- family: DROP SERVER
DROP SERVER zzp_srv
-- family: DROP TABLESPACE
DROP TABLESPACE zzp_ts
-- family: DROP UNDO TABLESPACE
DROP UNDO TABLESPACE zzp_ut
-- family: DROP LOGFILE GROUP
DROP LOGFILE GROUP zzp_lg ENGINE = InnoDB
-- family: DROP SPATIAL REFERENCE SYSTEM
DROP SPATIAL REFERENCE SYSTEM 990001
-- family: DROP RESOURCE GROUP
DROP RESOURCE GROUP zzp_rg

-- ==== Schema utility & table maintenance ====
-- family: TRUNCATE TABLE
TRUNCATE TABLE t1
-- family: RENAME TABLE
RENAME TABLE zzp_t TO zzp_t2
-- family: RENAME USER
RENAME USER zzp_u@localhost TO zzp_u2@localhost
-- family: ANALYZE TABLE
ANALYZE TABLE t1
-- family: CHECK TABLE
CHECK TABLE t1
-- family: CHECKSUM TABLE
CHECKSUM TABLE t1
-- family: OPTIMIZE TABLE
OPTIMIZE TABLE t1
-- family: REPAIR TABLE
REPAIR TABLE t1

-- ==== Access control ====
-- family: GRANT
GRANT SELECT ON *.* TO zzp_u@localhost
-- family: REVOKE
REVOKE SELECT ON *.* FROM zzp_u@localhost
-- family: SET ROLE
SET ROLE NONE
-- family: SET RESOURCE GROUP
SET RESOURCE GROUP zzp_rg

-- ==== Diagnostics & signals ====
-- family: SIGNAL
SIGNAL SQLSTATE '45000'
-- family: RESIGNAL
RESIGNAL SQLSTATE '45000'
-- family: GET DIAGNOSTICS
GET DIAGNOSTICS @zzp_n = NUMBER

-- ==== Server administration ====
-- family: FLUSH
FLUSH TABLES
-- family: RESET
RESET PERSIST
-- family: PURGE
PURGE BINARY LOGS BEFORE '2000-01-01 00:00:00'
-- family: KILL
KILL 2147483647
-- family: SHUTDOWN
SHUTDOWN
-- family: RESTART
RESTART
-- family: CLONE
CLONE LOCAL DATA DIRECTORY = '/tmp/zzp_clone'
-- family: INSTALL PLUGIN
INSTALL PLUGIN zzp_pl SONAME 'zzp_plugin.so'
-- family: INSTALL COMPONENT
INSTALL COMPONENT 'file://zzp_component'
-- family: UNINSTALL PLUGIN
UNINSTALL PLUGIN zzp_pl
-- family: UNINSTALL COMPONENT
UNINSTALL COMPONENT 'file://zzp_component'
-- family: IMPORT TABLE
IMPORT TABLE FROM 'zzp_table.sdi'
-- family: HELP
HELP 'CONTENTS'
-- family: BINLOG
BINLOG 'zzp_base64_event'

-- ==== Introspection ====
-- family: SHOW
SHOW DATABASES
-- family: EXPLAIN
EXPLAIN SELECT 1

-- ==== Stored routines ====
-- family: CALL
CALL zzp_p()

-- ==== Replication ====
-- family: CHANGE REPLICATION SOURCE
CHANGE REPLICATION SOURCE TO SOURCE_HOST = 'zzp_h'
-- family: CHANGE REPLICATION FILTER
CHANGE REPLICATION FILTER REPLICATE_DO_DB = (zzp_db)
-- family: START REPLICA
START REPLICA
-- family: STOP REPLICA
STOP REPLICA
-- family: GROUP REPLICATION
START GROUP_REPLICATION
