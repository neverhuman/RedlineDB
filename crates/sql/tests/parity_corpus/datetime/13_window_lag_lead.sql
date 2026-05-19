-- datetime seed 13: window lag/lead
CREATE TABLE datetime_wl(id INTEGER PRIMARY KEY, grp TEXT, v INTEGER);
INSERT INTO datetime_wl VALUES (1, 'A', 10), (2, 'A', NULL), (3, 'A', 30), (4, 'B', NULL), (5, 'B', 50);
SELECT id, grp, v,
       lag(v) OVER (PARTITION BY grp ORDER BY id),
       lead(v) OVER (PARTITION BY grp ORDER BY id)
FROM datetime_wl
ORDER BY id;
