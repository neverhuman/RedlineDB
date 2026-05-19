-- datetime seed 06: null coercion
SELECT NULL AS n, coalesce(NULL, 7) AS c, ifnull(NULL, 'fallback') AS f;
