-- compound seed 16: scalar subquery
CREATE TABLE compound_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO compound_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM compound_ss AS s2 WHERE s2.id = compound_ss.id)
FROM compound_ss
ORDER BY id;
