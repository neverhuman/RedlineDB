-- view seed 22: order by expression
CREATE TABLE view_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO view_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM view_ob ORDER BY coalesce(v, 'zzz'), id;
