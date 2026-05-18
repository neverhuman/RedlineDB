-- trigger seed 15: in subquery
CREATE TABLE trigger_si(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO trigger_si VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM trigger_si
WHERE id IN (SELECT id FROM trigger_si WHERE v IS NOT NULL)
ORDER BY id;
