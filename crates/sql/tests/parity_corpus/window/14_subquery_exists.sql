-- window seed 14: exists subquery
CREATE TABLE window_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO window_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM window_sx
WHERE EXISTS (SELECT 1 FROM window_sx AS s2 WHERE s2.id < window_sx.id)
ORDER BY id;
