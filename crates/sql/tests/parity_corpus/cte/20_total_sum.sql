-- cte seed 20: aggregate nulls
CREATE TABLE cte_sum(v INTEGER);
INSERT INTO cte_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM cte_sum;
