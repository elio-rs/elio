-- with clause with match clause
MATCH (a) WITH a MATCH (b)--(a) RETURN a,b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@0], nodes: [b@1, a@0], rels: [(b@1)<-[anon@2:]->(a@0)] }
  ├─projection
  │ └─Project { items: [a@0 AS a@0, b@1 AS b@1] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0] }
    └─projection
      └─Project { items: [a@0 AS a@0] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@0,b@1 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0] }
      │ └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        └─Argument { variables: [a@0] }
*/

-- with clause with expression
MATCH (a) WITH a, a.age + 1 AS b RETURN a,b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@0 AS a@0, b@1 AS b@1] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0] }
    └─projection
      └─Project { items: [a@0 AS a@0, b@1 AS add(a@0.age, 1)] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@0,b@1 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0, b@1 AS add(a@0.age, 1)] }
      │ └─AllNodeScan { variable: a@0 }
      └─Argument { variables: [a@0, b@1] }
*/

-- with clause with single variable
MATCH (a)-[]-(b) WITH a RETURN a

/*
RootIR { names: [a] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@0 AS a@0] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@0 AS a@0] }
RootPlan { names: [a] }
└─ProduceResult { return_columns: a@0 }
  └─Project { exprs: [a@0 AS a@0] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─Argument { variables: [a@0] }
*/

-- with clause with match clause, SHOULD GENERATE CROSS PRODUCT PLAN
MATCH (a)-[]-(b) WITH a MATCH (b)-[]-(c) RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@0], nodes: [b@3, c@4], rels: [(b@3)<-[anon@5:]->(c@4)] }
  ├─projection
  │ └─Project { items: [a@0 AS a@0, b@3 AS b@3, c@4 AS c@4] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@0 AS a@0] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@0,b@3,c@4 }
  └─Project { exprs: [a@0 AS a@0, b@3 AS b@3, c@4 AS c@4] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: b@3, to: c@4, rel: anon@5, direction: -, types: [] }
        └─AllNodeScan { variable: b@3, arguments: [a@0] }
*/

-- with clause with match clause, should generate apply plan
MATCH (a)-[]-(b) WITH a, b MATCH (b)-[]-(c) RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@0, b@1], nodes: [b@1, c@3], rels: [(b@1)<-[anon@4:]->(c@3)] }
  ├─projection
  │ └─Project { items: [a@0 AS a@0, b@1 AS b@1, c@3 AS c@3] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@0 AS a@0, b@1 AS b@1] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@0,b@1,c@3 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, c@3 AS c@3] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0, b@1 AS b@1] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: b@1, to: c@3, rel: anon@4, direction: -, types: [] }
        └─Argument { variables: [a@0, b@1] }
*/

-- with clause with cross product
MATCH (a) WITH a MATCH (b) WITH a, b MATCH (c) WITH a, b, c RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@0 AS a@0, b@1 AS b@1, c@2 AS c@2] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { input_bindings: [a@0, b@1], nodes: [c@2] }
    ├─projection
    │ └─Project { items: [a@0 AS a@0, b@1 AS b@1, c@2 AS c@2] }
    └─IrSingleQueryPart
      ├─match_pattern
      │ └─QueryGraph { input_bindings: [a@0], nodes: [b@1] }
      ├─projection
      │ └─Project { items: [a@0 AS a@0, b@1 AS b@1] }
      └─IrSingleQueryPart
        ├─match_pattern
        │ └─QueryGraph { nodes: [a@0] }
        └─projection
          └─Project { items: [a@0 AS a@0] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@0,b@1,c@2 }
  └─Project { exprs: [a@0 AS a@0, b@1 AS b@1, c@2 AS c@2] }
    └─Apply
      ├─Project { exprs: [a@0 AS a@0, b@1 AS b@1, c@2 AS c@2] }
      │ └─Apply
      │   ├─Project { exprs: [a@0 AS a@0, b@1 AS b@1] }
      │   │ └─Apply
      │   │   ├─Project { exprs: [a@0 AS a@0] }
      │   │   │ └─AllNodeScan { variable: a@0 }
      │   │   └─AllNodeScan { variable: b@1, arguments: [a@0] }
      │   └─AllNodeScan { variable: c@2, arguments: [a@0, b@1] }
      └─Argument { variables: [a@0, b@1, c@2] }
*/

