CREATE TABLE sales(region TEXT, amount INTEGER);
INSERT INTO sales VALUES ('e', 100), ('e', 200), ('w', 50);
CREATE VIEW v_agg AS SELECT region, SUM(amount) AS total FROM sales GROUP BY region;
SELECT region, total FROM v_agg ORDER BY region
