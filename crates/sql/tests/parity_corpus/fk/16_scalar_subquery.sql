-- fk seed 16: scalar subquery
CREATE TABLE fk_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO fk_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM fk_ss AS s2 WHERE s2.id = fk_ss.id)
FROM fk_ss
ORDER BY id;
