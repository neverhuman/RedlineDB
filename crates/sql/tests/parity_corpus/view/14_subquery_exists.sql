-- view seed 14: exists subquery
CREATE TABLE view_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO view_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM view_sx
WHERE EXISTS (SELECT 1 FROM view_sx AS s2 WHERE s2.id < view_sx.id)
ORDER BY id;
