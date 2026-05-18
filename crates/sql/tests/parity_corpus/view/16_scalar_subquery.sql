-- view seed 16: scalar subquery
CREATE TABLE view_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO view_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM view_ss AS s2 WHERE s2.id = view_ss.id)
FROM view_ss
ORDER BY id;
