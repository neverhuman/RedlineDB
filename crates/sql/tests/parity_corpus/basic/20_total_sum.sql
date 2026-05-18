-- basic seed 20: aggregate nulls
CREATE TABLE basic_sum(v INTEGER);
INSERT INTO basic_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM basic_sum;
