-- index seed 16: scalar subquery
CREATE TABLE index_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO index_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM index_ss AS s2 WHERE s2.id = index_ss.id)
FROM index_ss
ORDER BY id;
