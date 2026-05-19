-- pragma seed 10: cross join
CREATE TABLE pragma_lhs(id INTEGER, label TEXT);
CREATE TABLE pragma_rhs(id INTEGER, payload TEXT);
INSERT INTO pragma_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO pragma_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM pragma_lhs AS l
CROSS JOIN pragma_rhs AS r
ORDER BY l.id, r.id, r.payload;
