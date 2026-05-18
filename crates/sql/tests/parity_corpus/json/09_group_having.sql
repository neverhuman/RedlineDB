-- json seed 09: group and having
CREATE TABLE json_grp(k TEXT, v INTEGER);
INSERT INTO json_grp VALUES ('A', 1), ('A', NULL), ('B', 2), ('B', 3);
SELECT k, count(*), count(v), sum(COALESCE(v, 0))
FROM json_grp
GROUP BY k
HAVING count(*) >= 2
ORDER BY k;
