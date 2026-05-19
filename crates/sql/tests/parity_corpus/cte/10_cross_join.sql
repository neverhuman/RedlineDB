-- cte seed 10: cross join
CREATE TABLE cte_lhs(id INTEGER, label TEXT);
CREATE TABLE cte_rhs(id INTEGER, payload TEXT);
INSERT INTO cte_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO cte_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM cte_lhs AS l
CROSS JOIN cte_rhs AS r
ORDER BY l.id, r.id, r.payload;
