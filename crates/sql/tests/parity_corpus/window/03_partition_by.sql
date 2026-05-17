CREATE TABLE sales(region TEXT, amount INTEGER);
INSERT INTO sales VALUES ('east', 100), ('east', 200), ('west', 150), ('west', 250);
SELECT region, amount, SUM(amount) OVER (PARTITION BY region) AS region_total FROM sales ORDER BY region, amount
