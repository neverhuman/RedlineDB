-- index seed 14: exists subquery
CREATE TABLE index_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO index_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM index_sx
WHERE EXISTS (SELECT 1 FROM index_sx AS s2 WHERE s2.id < index_sx.id)
ORDER BY id;
