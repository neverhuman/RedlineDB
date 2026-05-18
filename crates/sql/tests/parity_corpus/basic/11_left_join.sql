-- basic seed 11: left join null handling
CREATE TABLE basic_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE basic_b(aid INTEGER, payload TEXT);
INSERT INTO basic_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO basic_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM basic_a AS a
LEFT JOIN basic_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
