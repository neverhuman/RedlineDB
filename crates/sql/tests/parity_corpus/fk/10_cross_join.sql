-- fk seed 10: cross join
CREATE TABLE fk_lhs(id INTEGER, label TEXT);
CREATE TABLE fk_rhs(id INTEGER, payload TEXT);
INSERT INTO fk_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO fk_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM fk_lhs AS l
CROSS JOIN fk_rhs AS r
ORDER BY l.id, r.id, r.payload;
