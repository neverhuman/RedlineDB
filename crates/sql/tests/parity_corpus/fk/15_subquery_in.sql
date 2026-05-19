-- fk seed 15: in subquery
CREATE TABLE fk_si(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO fk_si VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM fk_si
WHERE id IN (SELECT id FROM fk_si WHERE v IS NOT NULL)
ORDER BY id;
