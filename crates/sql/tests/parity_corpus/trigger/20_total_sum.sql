-- trigger seed 20: aggregate nulls
CREATE TABLE trigger_sum(v INTEGER);
INSERT INTO trigger_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM trigger_sum;
