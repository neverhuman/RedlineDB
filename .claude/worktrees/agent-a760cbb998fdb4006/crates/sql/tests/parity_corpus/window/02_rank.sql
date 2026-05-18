CREATE TABLE scores(name TEXT, score INTEGER);
INSERT INTO scores VALUES ('a', 90), ('b', 80), ('c', 80), ('d', 70);
SELECT name, RANK() OVER (ORDER BY score DESC) AS rk FROM scores ORDER BY name
