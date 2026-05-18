-- datetime seed 16: scalar subquery
CREATE TABLE datetime_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO datetime_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM datetime_ss AS s2 WHERE s2.id = datetime_ss.id)
FROM datetime_ss
ORDER BY id;
