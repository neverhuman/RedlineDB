-- trigger seed 22: order by expression
CREATE TABLE trigger_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO trigger_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM trigger_ob ORDER BY coalesce(v, 'zzz'), id;
