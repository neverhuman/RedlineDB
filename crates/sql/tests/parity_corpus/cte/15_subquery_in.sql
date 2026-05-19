-- cte seed 15: in subquery
CREATE TABLE cte_si(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO cte_si VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM cte_si
WHERE id IN (SELECT id FROM cte_si WHERE v IS NOT NULL)
ORDER BY id;
