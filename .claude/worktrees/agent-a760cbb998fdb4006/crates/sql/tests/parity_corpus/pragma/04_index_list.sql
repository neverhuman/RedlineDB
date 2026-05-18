CREATE TABLE t(id INTEGER, v TEXT);
CREATE INDEX idx_t_v ON t(v);
PRAGMA index_list(t)
