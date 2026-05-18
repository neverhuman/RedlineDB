-- view seed 20: aggregate nulls
CREATE TABLE view_sum(v INTEGER);
INSERT INTO view_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM view_sum;
