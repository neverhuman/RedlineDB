-- fk seed 14: exists subquery
CREATE TABLE fk_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO fk_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM fk_sx
WHERE EXISTS (SELECT 1 FROM fk_sx AS s2 WHERE s2.id < fk_sx.id)
ORDER BY id;
