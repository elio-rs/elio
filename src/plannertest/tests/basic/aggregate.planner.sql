-- aggregate sum
MATCH (a)--(b) WITH a, b.age AS b RETURN a, SUM(b)

/*
Error
Variable b is not defined in Aggregation
*/

-- aggregate sum
MATCH (a)--(b) RETURN a, SUM(b.age)

/*
Error
Variable b is not defined in Aggregation
*/

-- aggregate sum with expression
MATCH (a)--(b) RETURN a, SUM(b.age + 1)

/*
Error
Variable b is not defined in Aggregation
*/

-- aggregate sum then project
MATCH (a)--(b) RETURN a, SUM(b.age) + 1

/*
Error
Variable b is not defined in Aggregation
*/

-- aggregate sum then project
MATCH (a)--(b) RETURN a.age + 1, SUM(b.age)

/*
Error
Variable b is not defined in Aggregation
*/

