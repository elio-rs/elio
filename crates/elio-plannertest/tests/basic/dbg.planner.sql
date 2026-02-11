-- aggregate sum with pre projection
MATCH (a)--(b) RETURN a, SUM(b.age)

/*
RootIR { names: [a, SUM(b.age)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage@3 AS sum(b@1.age)], post_projection: [a@0 AS a@0, SUMbage@3 AS SUMbage@3] }
RootPlan { names: [a, SUM(b.age)] }
└─ProduceResult { return_columns: a@0,SUMbage@3 }
  └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage@3 AS sum(anon@0)] }
    └─Project { exprs: [a@0 AS a@0, anon@0 AS b@1.age] }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─AllNodeScan { variable: a@0 }
*/

