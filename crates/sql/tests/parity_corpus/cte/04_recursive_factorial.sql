WITH RECURSIVE fact(n, f) AS (
  SELECT 1, 1 UNION ALL SELECT n + 1, (n + 1) * f FROM fact WHERE n < 5
)
SELECT n, f FROM fact ORDER BY n
