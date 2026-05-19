-- datetime seed 12: window row_number
CREATE TABLE datetime_w(id INTEGER PRIMARY KEY, grp TEXT, v INTEGER);
INSERT INTO datetime_w VALUES (1, 'A', 10), (2, 'A', NULL), (3, 'B', 30), (4, 'B', 40);
SELECT id, grp, v, row_number() OVER (PARTITION BY grp ORDER BY id)
FROM datetime_w
ORDER BY id;
