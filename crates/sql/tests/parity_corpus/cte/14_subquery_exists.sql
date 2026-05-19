-- cte seed 14: exists subquery
CREATE TABLE cte_sx(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO cte_sx VALUES (1, 'A1'), (2, NULL), (3, 'A3');
SELECT id FROM cte_sx
WHERE EXISTS (SELECT 1 FROM cte_sx AS s2 WHERE s2.id < cte_sx.id)
ORDER BY id;
