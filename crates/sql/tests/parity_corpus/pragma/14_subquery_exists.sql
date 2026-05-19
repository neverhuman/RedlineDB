-- pragma seed 14: exists subquery
CREATE TABLE pragma_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO pragma_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM pragma_sx
WHERE EXISTS (SELECT 1 FROM pragma_sx AS s2 WHERE s2.id < pragma_sx.id)
ORDER BY id;
