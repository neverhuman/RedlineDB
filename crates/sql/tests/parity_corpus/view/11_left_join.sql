-- view seed 11: left join null handling
CREATE TABLE view_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE view_b(aid INTEGER, payload TEXT);
INSERT INTO view_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO view_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM view_a AS a
LEFT JOIN view_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
