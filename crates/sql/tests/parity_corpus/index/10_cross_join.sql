-- index seed 10: cross join
CREATE TABLE index_lhs(id INTEGER, label TEXT);
CREATE TABLE index_rhs(id INTEGER, payload TEXT);
INSERT INTO index_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO index_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM index_lhs AS l
CROSS JOIN index_rhs AS r
ORDER BY l.id, r.id, r.payload;
