-- basic seed 14: exists subquery
CREATE TABLE basic_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO basic_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM basic_sx
WHERE EXISTS (SELECT 1 FROM basic_sx AS s2 WHERE s2.id < basic_sx.id)
ORDER BY id;
