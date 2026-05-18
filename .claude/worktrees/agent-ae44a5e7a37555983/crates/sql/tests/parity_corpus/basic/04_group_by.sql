CREATE TABLE pets(species TEXT, count INTEGER);
INSERT INTO pets VALUES ('cat', 3), ('dog', 5), ('cat', 2), ('dog', 1);
SELECT species, SUM(count) AS total FROM pets GROUP BY species ORDER BY species
