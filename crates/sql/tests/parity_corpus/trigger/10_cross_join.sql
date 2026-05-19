-- trigger seed 10: cross join
CREATE TABLE trigger_lhs(id INTEGER, label TEXT);
CREATE TABLE trigger_rhs(id INTEGER, payload TEXT);
INSERT INTO trigger_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO trigger_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM trigger_lhs AS l
CROSS JOIN trigger_rhs AS r
ORDER BY l.id, r.id, r.payload;
