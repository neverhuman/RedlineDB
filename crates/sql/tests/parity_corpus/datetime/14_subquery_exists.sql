-- datetime seed 14: exists subquery
CREATE TABLE datetime_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO datetime_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM datetime_sx
WHERE EXISTS (SELECT 1 FROM datetime_sx AS s2 WHERE s2.id < datetime_sx.id)
ORDER BY id;
