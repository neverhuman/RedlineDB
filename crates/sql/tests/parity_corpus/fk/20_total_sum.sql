-- fk seed 20: aggregate nulls
CREATE TABLE fk_sum(v INTEGER);
INSERT INTO fk_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM fk_sum;
