-- pragma seed 16: scalar subquery
CREATE TABLE pragma_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO pragma_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM pragma_ss AS s2 WHERE s2.id = pragma_ss.id)
FROM pragma_ss
ORDER BY id;
