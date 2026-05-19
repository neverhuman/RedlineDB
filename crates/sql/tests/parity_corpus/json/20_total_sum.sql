-- json seed 20: aggregate nulls
CREATE TABLE json_sum(v INTEGER);
INSERT INTO json_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM json_sum;
