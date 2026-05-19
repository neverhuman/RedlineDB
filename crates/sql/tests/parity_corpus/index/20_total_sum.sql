-- index seed 20: aggregate nulls
CREATE TABLE index_sum(v INTEGER);
INSERT INTO index_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM index_sum;
