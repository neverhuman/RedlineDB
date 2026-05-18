-- datetime seed 22: order by expression
CREATE TABLE datetime_ob(id INTEGER PRIMARY KEY, v TEXT);
INSERT INTO datetime_ob VALUES (1, 'b'), (2, NULL), (3, 'a');
SELECT id, v FROM datetime_ob ORDER BY coalesce(v, 'zzz'), id;
