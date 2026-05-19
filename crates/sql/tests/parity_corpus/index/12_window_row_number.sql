-- index seed 12: window row_number
CREATE TABLE index_w(id INTEGER PRIMARY KEY, grp TEXT, v INTEGER);
INSERT INTO index_w VALUES (1, 'A', 10), (2, 'A', NULL), (3, 'B', 30), (4, 'B', 40);
SELECT id, grp, v, row_number() OVER (PARTITION BY grp ORDER BY id)
FROM index_w
ORDER BY id;
