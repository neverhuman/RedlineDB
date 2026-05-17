CREATE TABLE t(id INTEGER, name TEXT);
INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c');
CREATE INDEX idx_t_name ON t(name);
SELECT id FROM t WHERE name = 'b'
