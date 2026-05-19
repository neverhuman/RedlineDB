-- trigger seed 16: scalar subquery
CREATE TABLE trigger_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO trigger_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM trigger_ss AS s2 WHERE s2.id = trigger_ss.id)
FROM trigger_ss
ORDER BY id;
