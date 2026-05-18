-- pragma seed 20: aggregate nulls
CREATE TABLE pragma_sum(v INTEGER);
INSERT INTO pragma_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM pragma_sum;
