-- cte seed 16: scalar subquery
CREATE TABLE cte_ss(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO cte_ss VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id, (SELECT coalesce(v, 'missing') FROM cte_ss AS s2 WHERE s2.id = cte_ss.id)
FROM cte_ss
ORDER BY id;
