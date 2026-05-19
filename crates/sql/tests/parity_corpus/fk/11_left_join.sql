-- fk seed 11: left join null handling
CREATE TABLE fk_a(id INTEGER PRIMARY KEY, v TEXT);
CREATE TABLE fk_b(aid INTEGER, payload TEXT);
INSERT INTO fk_a VALUES (1, 'A1'), (2, NULL), (3, 'A3');
INSERT INTO fk_b VALUES (1, 'B1'), (1, NULL), (3, 'B3');
SELECT a.id, coalesce(b.payload, 'missing')
FROM fk_a AS a
LEFT JOIN fk_b AS b ON a.id = b.aid
ORDER BY a.id, b.payload;
