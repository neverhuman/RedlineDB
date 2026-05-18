CREATE TABLE t(id INTEGER, status TEXT);
INSERT INTO t VALUES (1, 'open'), (2, 'closed'), (3, 'open');
CREATE INDEX idx_open ON t(id) WHERE status = 'open';
SELECT id FROM t WHERE status = 'open' ORDER BY id
