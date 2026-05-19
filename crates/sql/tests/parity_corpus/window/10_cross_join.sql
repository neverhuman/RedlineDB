-- window seed 10: cross join
CREATE TABLE window_lhs(id INTEGER, label TEXT);
CREATE TABLE window_rhs(id INTEGER, payload TEXT);
INSERT INTO window_lhs VALUES (1, 'L1'), (2, NULL);
INSERT INTO window_rhs VALUES (1, 'R1'), (2, NULL);
SELECT l.id, r.id, r.payload
FROM window_lhs AS l
CROSS JOIN window_rhs AS r
ORDER BY l.id, r.id, r.payload;
