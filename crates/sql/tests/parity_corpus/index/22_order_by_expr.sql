-- index seed 22: order by expression
CREATE TABLE index_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO index_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM index_ob ORDER BY coalesce(v, 'zzz'), id;
