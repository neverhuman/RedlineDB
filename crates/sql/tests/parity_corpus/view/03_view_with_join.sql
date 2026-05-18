CREATE TABLE a(id INTEGER, name TEXT);
CREATE TABLE b(aid INTEGER, score INTEGER);
INSERT INTO a VALUES (1, 'x'), (2, 'y');
INSERT INTO b VALUES (1, 100), (2, 200);
CREATE VIEW v_join AS SELECT a.id, a.name, b.score FROM a JOIN b ON a.id = b.aid;
SELECT id, name, score FROM v_join ORDER BY id
