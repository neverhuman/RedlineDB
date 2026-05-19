-- pragma seed 13: window lag/lead
CREATE TABLE pragma_wl(id INTEGER PRIMARY KEY, grp TEXT, v INTEGER);
INSERT INTO pragma_wl VALUES (1, 'A', 10), (2, 'A', NULL), (3, 'A', 30), (4, 'B', NULL), (5, 'B', 50);
SELECT id, grp, v,
       lag(v) OVER (PARTITION BY grp ORDER BY id),
       lead(v) OVER (PARTITION BY grp ORDER BY id)
FROM pragma_wl
ORDER BY id;
