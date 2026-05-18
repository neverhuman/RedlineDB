-- view seed 10: cross join
CREATE TABLE view_lhs(id INTEGER, label TEXT);
CREATE TABLE view_rhs(id INTEGER, payload TEXT);
INSERT INTO view_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO view_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM view_lhs AS l
CROSS JOIN view_rhs AS r
ORDER BY l.id, r.id, r.payload;
