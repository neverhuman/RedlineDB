-- index seed 18: case with null
SELECT CASE WHEN NULL IS NULL THEN 'yes' ELSE 'no' END AS first_branch,
       CASE WHEN 0 THEN 'no' ELSE NULL END AS second_branch;
