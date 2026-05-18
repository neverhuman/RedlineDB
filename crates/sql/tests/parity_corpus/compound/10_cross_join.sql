-- compound seed 10: cross join
CREATE TABLE compound_lhs(id INTEGER, label TEXT);
CREATE TABLE compound_rhs(id INTEGER, payload TEXT);
INSERT INTO compound_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO compound_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM compound_lhs AS l
CROSS JOIN compound_rhs AS r
ORDER BY l.id, r.id, r.payload;
