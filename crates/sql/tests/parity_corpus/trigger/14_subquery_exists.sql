-- trigger seed 14: exists subquery
CREATE TABLE trigger_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO trigger_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM trigger_sx
WHERE EXISTS (SELECT 1 FROM trigger_sx AS s2 WHERE s2.id < trigger_sx.id)
ORDER BY id;
