-- window seed 08: recursive cte
WITH RECURSIVE seq(n) AS (
  SELECT 1
  UNION ALL
  SELECT n + 1 FROM seq WHERE n < 4
)
SELECT n, n * n AS square FROM seq ORDER BY n;
