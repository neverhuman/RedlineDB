-- pragma seed 15: in subquery
CREATE TABLE pragma_si(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO pragma_si VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM pragma_si
WHERE id IN (SELECT id FROM pragma_si WHERE v IS NOT NULL)
ORDER BY id;
