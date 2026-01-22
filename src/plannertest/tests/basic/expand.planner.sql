-- expand all
MATCH (a)-[r:KNOWS]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:]->(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─ExpandAll { from: a@0, to: b@1, rel: r@2, direction: -, types: [KNOWS] }
      └─AllNodeScan { variable: a@0 }
*/

-- expand all
MATCH (a)<-[r:KNOWS]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─ExpandAll { from: a@0, to: b@1, rel: r@2, direction: <-, types: [KNOWS] }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand 1..3
MATCH (a)<-[r:KNOWS*1..3]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*1..3]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*1..3]-(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand 1..3
MATCH (a)<-[r:KNOWS*..3]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*1..3]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*1..3]-(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand 1..INF
MATCH (a)<-[r:KNOWS*1..]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*1..18446744073709551615]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*1..18446744073709551615]-(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand 1..INF
MATCH (a)<-[r:KNOWS*]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*1..18446744073709551615]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*1..18446744073709551615]-(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand 2..2
MATCH (a)<-[r:KNOWS*2]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*2..2]-(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*2..2]-(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

-- variable expand undirected
MATCH (a)-[r:KNOWS*]-(b) RETURN *

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[r@2:*1..18446744073709551615]->(b@1)] }
  └─projection
    └─Project { items: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, r@2 AS r@2] }
    └─VarExpandAll { from: a@0, to: b@1, rel_pattern: (a@0)<-[r@2:*1..18446744073709551615]->(b@1), path_mode: Trail }
      └─AllNodeScan { variable: a@0 }
*/

