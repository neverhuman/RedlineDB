-- datetime seed 10: cross join
CREATE TABLE datetime_lhs(id INTEGER, label TEXT);
CREATE TABLE datetime_rhs(id INTEGER, payload TEXT);
INSERT INTO datetime_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO datetime_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM datetime_lhs AS l
CROSS JOIN datetime_rhs AS r
ORDER BY l.id, r.id, r.payload;
