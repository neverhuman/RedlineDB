-- window seed 22: order by expression
CREATE TABLE window_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO window_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM window_ob ORDER BY coalesce(v, 'zzz'), id;
