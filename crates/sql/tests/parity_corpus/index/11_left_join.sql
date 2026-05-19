-- index seed 11: left join null handling
CREATE TABLE index_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE index_b(aid INTEGER, payload TEXT);
INSERT INTO index_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO index_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM index_a AS a
LEFT JOIN index_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
