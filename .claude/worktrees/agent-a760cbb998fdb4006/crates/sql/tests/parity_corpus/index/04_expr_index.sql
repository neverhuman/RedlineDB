CREATE TABLE t(id INTEGER, v TEXT);
INSERT INTO t VALUES (1, 'aa'), (2, 'BB'), (3, 'cc');
CREATE INDEX idx_lower ON t(LOWER(v));
SELECT id FROM t WHERE LOWER(v) = 'bb'
