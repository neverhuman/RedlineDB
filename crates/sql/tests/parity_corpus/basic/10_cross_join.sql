-- basic seed 10: cross join
CREATE TABLE basic_lhs(id INTEGER, label TEXT);
CREATE TABLE basic_rhs(id INTEGER, payload TEXT);
INSERT INTO basic_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO basic_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM basic_lhs AS l
CROSS JOIN basic_rhs AS r
ORDER BY l.id, r.id, r.payload;
