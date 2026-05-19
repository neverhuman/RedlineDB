-- basic seed 16: scalar subquery
CREATE TABLE basic_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO basic_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM basic_ss AS s2 WHERE s2.id = basic_ss.id)
FROM basic_ss
ORDER BY id;
