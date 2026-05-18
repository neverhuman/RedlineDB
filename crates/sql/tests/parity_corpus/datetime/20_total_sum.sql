-- datetime seed 20: aggregate nulls
CREATE TABLE datetime_sum(v INTEGER);
INSERT INTO datetime_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM datetime_sum;
