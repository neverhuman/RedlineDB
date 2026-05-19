-- json seed 16: scalar subquery
CREATE TABLE json_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO json_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM json_ss AS s2 WHERE s2.id = json_ss.id)
FROM json_ss
ORDER BY id;
