-- aggregate sum
MATCH (a)--(b) RETURN a, SUM(b)

/*
RootIR { names: [a, SUM(b)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMb@3 AS sum(b@1)], post_projection: [a@0 AS a@0, SUMb@3 AS SUMb@3] }
RootPlan { names: [a, SUM(b)] }
└─ProduceResult { return_columns: a@0,SUMb@3 }
  └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMb@3 AS sum(b@1)] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- aggregate sum
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
  └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage@3 AS sum(b@1.age)] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- aggregate sum with expression
MATCH (a)--(b) RETURN a, SUM(b.age + 1)

/*
RootIR { names: [a, SUM((b.age) + (1))] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage1@3 AS sum(add(b@1.age, 1))], post_projection: [a@0 AS a@0, SUMbage1@3 AS SUMbage1@3] }
RootPlan { names: [a, SUM((b.age) + (1))] }
└─ProduceResult { return_columns: a@0,SUMbage1@3 }
  └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage1@3 AS sum(add(b@1.age, 1))] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- aggregate sum then project
MATCH (a)--(b) RETURN a, SUM(b.age) + 1

/*
RootIR { names: [a, (SUM(b.age)) + (1)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [anon@4 AS sum(b@1.age)], post_projection: [a@0 AS a@0, SUMbage1@3 AS add(anon@4, 1)] }
RootPlan { names: [a, (SUM(b.age)) + (1)] }
└─ProduceResult { return_columns: a@0,SUMbage1@3 }
  └─Project { exprs: [a@0 AS a@0, SUMbage1@3 AS add(anon@4, 1)] }
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [anon@4 AS sum(b@1.age)] }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─AllNodeScan { variable: a@0 }
*/

-- aggregate sum then project
MATCH (a)--(b) RETURN a.age + 1, SUM(b.age)

/*
RootIR { names: [(a.age) + (1), SUM(b.age)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { group_by: [aage1@3 AS add(a@0.age, 1)], aggregate: [SUMbage@4 AS sum(b@1.age)], post_projection: [aage1@3 AS aage1@3, SUMbage@4 AS SUMbage@4] }
RootPlan { names: [(a.age) + (1), SUM(b.age)] }
└─ProduceResult { return_columns: aage1@3,SUMbage@4 }
  └─Aggregate { group_by: [aage1@3 AS add(a@0.age, 1)], aggregate: [SUMbage@4 AS sum(b@1.age)] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

