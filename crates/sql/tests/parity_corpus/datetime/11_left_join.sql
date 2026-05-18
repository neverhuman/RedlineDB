-- datetime seed 11: left join null handling
CREATE TABLE datetime_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE datetime_b(aid INTEGER, payload TEXT);
INSERT INTO datetime_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO datetime_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM datetime_a AS a
LEFT JOIN datetime_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
