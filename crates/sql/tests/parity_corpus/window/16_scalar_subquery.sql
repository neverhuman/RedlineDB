-- window seed 16: scalar subquery
CREATE TABLE window_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO window_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM window_ss AS s2 WHERE s2.id = window_ss.id)
FROM window_ss
ORDER BY id;
