-- cte seed 22: order by expression
CREATE TABLE cte_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO cte_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM cte_ob ORDER BY coalesce(v, 'zzz'), id;
