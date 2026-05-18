-- fk seed 22: order by expression
CREATE TABLE fk_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO fk_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM fk_ob ORDER BY coalesce(v, 'zzz'), id;
