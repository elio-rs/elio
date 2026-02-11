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

-- aggregate sum with pre projection
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
  └─Aggregate { group_by: [a@0 AS a@0], aggregate: [SUMbage1@3 AS sum(anon@0)] }
    └─Project { exprs: [a@0 AS a@0, anon@0 AS add(b@1.age, 1)] }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─AllNodeScan { variable: a@0 }
*/

-- aggregate with pre and post projeciton
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
    └─Aggregate { group_by: [a@0 AS a@0], aggregate: [anon@4 AS sum(anon@0)] }
      └─Project { exprs: [a@0 AS a@0, anon@0 AS b@1.age] }
        └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
          └─AllNodeScan { variable: a@0 }
*/

-- aggregate with pre and post projeciton
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
  └─Aggregate { group_by: [aage1@3 AS anon@0], aggregate: [SUMbage@4 AS sum(anon@1)] }
    └─Project { exprs: [anon@0 AS add(a@0.age, 1), anon@1 AS b@1.age] }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─AllNodeScan { variable: a@0 }
*/

-- aggregate with invalid distinct
MATCH (a)--(b) RETURN DISTINCT a, SUM(b)

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

-- distinct agg
MATCH (a)--(b) RETURN DISTINCT a, b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Distinct { group_by: [a@0 AS a@0, b@1 AS b@1] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@0,b@1 }
  └─Distinct { group_exprs: [a@0 AS a@0, b@1 AS b@1] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- distinct agg
MATCH (a)--(b) RETURN DISTINCT a, a.age + b.age 

/*
RootIR { names: [a, (a.age) + (b.age)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Distinct { group_by: [a@0 AS a@0, aagebage@3 AS add(a@0.age, b@1.age)] }
RootPlan { names: [a, (a.age) + (b.age)] }
└─ProduceResult { return_columns: a@0,aagebage@3 }
  └─Distinct { group_exprs: [a@0 AS a@0, aagebage@3 AS add(a@0.age, b@1.age)] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- global sum
MATCH (a)--(b) RETURN SUM(a)

/*
RootIR { names: [SUM(a)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { aggregate: [SUMa@3 AS sum(a@0)], post_projection: [SUMa@3 AS SUMa@3] }
RootPlan { names: [SUM(a)] }
└─ProduceResult { return_columns: SUMa@3 }
  └─Aggregate { aggregate: [SUMa@3 AS sum(a@0)] }
    └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: a@0 }
*/

-- global sum with pre projection
MATCH (a)--(b) RETURN SUM(a.age)

/*
RootIR { names: [SUM(a.age)] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
  └─projection
    └─Aggregate { aggregate: [SUMaage@3 AS sum(a@0.age)], post_projection: [SUMaage@3 AS SUMaage@3] }
RootPlan { names: [SUM(a.age)] }
└─ProduceResult { return_columns: SUMaage@3 }
  └─Aggregate { aggregate: [SUMaage@3 AS sum(anon@0)] }
    └─Project { exprs: [anon@0 AS a@0.age] }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─AllNodeScan { variable: a@0 }
*/

