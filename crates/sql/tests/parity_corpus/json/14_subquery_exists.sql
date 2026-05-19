-- json seed 14: exists subquery
CREATE TABLE json_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO json_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM json_sx
WHERE EXISTS (SELECT 1 FROM json_sx AS s2 WHERE s2.id < json_sx.id)
ORDER BY id;
