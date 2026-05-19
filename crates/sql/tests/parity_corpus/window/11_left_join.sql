-- window seed 11: left join null handling
CREATE TABLE window_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE window_b(aid INTEGER, payload TEXT);
INSERT INTO window_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO window_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM window_a AS a
LEFT JOIN window_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
