CREATE TABLE t(id INTEGER, name TEXT);
INSERT INTO t VALUES (1, 'alpha'), (2, 'beta'), (3, 'gamma');
SELECT id, name FROM t ORDER BY id
