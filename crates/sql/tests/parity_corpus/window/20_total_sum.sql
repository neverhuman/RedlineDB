-- window seed 20: aggregate nulls
CREATE TABLE window_sum(v INTEGER);
INSERT INTO window_sum VALUES (NULL), (NULL);
SELECT total(v), sum(v), count(v), count(*) FROM window_sum;
