-- aggregate sum
MATCH (a)--(b) RETURN a, SUM(b)

/*
RootIR { names: [a, SUM(b)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMb@3 AS sum(b@1)], post_projection: [a@0 AS a@0, SUMb@3 AS SUMb@3] }
*/

