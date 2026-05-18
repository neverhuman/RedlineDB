-- window seed 15: in subquery
CREATE TABLE window_si(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO window_si VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM window_si
WHERE id IN (SELECT id FROM window_si WHERE v IS NOT NULL)
ORDER BY id;
