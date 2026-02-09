-- path with variable
MATCH p = ()-[]-() RETURN p

/*
RootIR { names: [p] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [anon@0, anon@1], rels: [(anon@0)<-[anon@2:]->(anon@1)] }
  └─projection
    └─Project { items: [p@4 AS (anon@0)-[anon@2]-(anon@1)] }
RootPlan { names: [p] }
└─ProduceResult { return_columns: p@4 }
  └─Project { exprs: [p@4 AS (anon@0)-[anon@2]-(anon@1)] }
    └─ExpandAll { from: anon@0, to: anon@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: anon@0 }
*/

-- path with variable
MATCH p = ()-[]-() RETURN *

/*
RootIR { names: [p] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [anon@0, anon@1], rels: [(anon@0)<-[anon@2:]->(anon@1)] }
  └─projection
    └─Project { items: [p@4 AS (anon@0)-[anon@2]-(anon@1)] }
RootPlan { names: [p] }
└─ProduceResult { return_columns: p@4 }
  └─Project { exprs: [p@4 AS (anon@0)-[anon@2]-(anon@1)] }
    └─ExpandAll { from: anon@0, to: anon@1, rel: anon@2, direction: -, types: [] }
      └─AllNodeScan { variable: anon@0 }
*/

