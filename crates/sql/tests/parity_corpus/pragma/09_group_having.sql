-- pragma seed 09: group and having
CREATE TABLE pragma_grp(k TEXT, v INTEGER);
INSERT INTO pragma_grp VALUES ('A', 1), ('A', NULL), ('B', 2), ('B', 3);
SELECT k, count(*), count(v), sum(COALESCE(v, 0))
FROM pragma_grp
GROUP BY k
HAVING count(*) >= 2
ORDER BY k;
