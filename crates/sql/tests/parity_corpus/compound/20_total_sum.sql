-- compound seed 20: aggregate nulls
CREATE TABLE compound_sum(v INTEGER);
INSERT INTO compound_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM compound_sum;
