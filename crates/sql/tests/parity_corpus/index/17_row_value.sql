-- index seed 17: row value compare
SELECT (1, 2) IN (SELECT 1, 2) AS row_in, (1, 2) NOT IN (SELECT 1, 3) AS row_not_in;
