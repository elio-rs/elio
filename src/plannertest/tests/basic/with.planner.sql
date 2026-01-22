-- with clause with match clause
MATCH (a) WITH a MATCH (b)--(a) RETURN a,b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@1], nodes: [b@2, a@1], rels: [(b@2)<-[anon@3:]->(a@1)] }
  ├─projection
  │ └─Project { items: [a@4 AS a@1, b@5 AS b@2] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0] }
    └─projection
      └─Project { items: [a@1 AS a@0] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@4,b@5 }
  └─Project { exprs: [a@4 AS a@1, b@5 AS b@2] }
    └─Apply
      ├─Project { exprs: [a@1 AS a@0] }
      │ └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: a@1, to: b@2, rel: anon@3, direction: -, types: [] }
        └─Argument { variables: [a@1] }
*/

-- with clause with expression
MATCH (a) WITH a, a.age + 1 AS b RETURN a,b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@3 AS a@1, b@4 AS b@2] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0] }
    └─projection
      └─Project { items: [a@1 AS a@0, b@2 AS add(a@0.age, 1)] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@3,b@4 }
  └─Project { exprs: [a@3 AS a@1, b@4 AS b@2] }
    └─Apply
      ├─Project { exprs: [a@1 AS a@0, b@2 AS add(a@0.age, 1)] }
      │ └─AllNodeScan { variable: a@0 }
      └─Argument { variables: [a@1, b@2] }
*/

-- with clause with single variable
MATCH (a)-[]-(b) WITH a RETURN a

/*
RootIR { names: [a] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@4 AS a@3] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@3 AS a@0] }
RootPlan { names: [a] }
└─ProduceResult { return_columns: a@4 }
  └─Project { exprs: [a@4 AS a@3] }
    └─Apply
      ├─Project { exprs: [a@3 AS a@0] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─Argument { variables: [a@3] }
*/

-- with clause with match clause, SHOULD GENERATE CROSS PRODUCT PLAN
MATCH (a)-[]-(b) WITH a MATCH (b)-[]-(c) RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@3], nodes: [b@4, c@5], rels: [(b@4)<-[anon@6:]->(c@5)] }
  ├─projection
  │ └─Project { items: [a@7 AS a@3, b@8 AS b@4, c@9 AS c@5] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@3 AS a@0] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@7,b@8,c@9 }
  └─Project { exprs: [a@7 AS a@3, b@8 AS b@4, c@9 AS c@5] }
    └─Apply
      ├─Project { exprs: [a@3 AS a@0] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: b@4, to: c@5, rel: anon@6, direction: -, types: [] }
        └─AllNodeScan { variable: b@4, arguments: [a@3] }
*/

-- with clause with match clause, should generate apply plan
MATCH (a)-[]-(b) WITH a, b MATCH (b)-[]-(c) RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [a@3, b@4], nodes: [b@4, c@5], rels: [(b@4)<-[anon@6:]->(c@5)] }
  ├─projection
  │ └─Project { items: [a@7 AS a@3, b@8 AS b@4, c@9 AS c@5] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { nodes: [a@0, b@1], rels: [(a@0)<-[anon@2:]->(b@1)] }
    └─projection
      └─Project { items: [a@3 AS a@0, b@4 AS b@1] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@7,b@8,c@9 }
  └─Project { exprs: [a@7 AS a@3, b@8 AS b@4, c@9 AS c@5] }
    └─Apply
      ├─Project { exprs: [a@3 AS a@0, b@4 AS b@1] }
      │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
      │   └─AllNodeScan { variable: a@0 }
      └─ExpandAll { from: b@4, to: c@5, rel: anon@6, direction: -, types: [] }
        └─Argument { variables: [a@3, b@4] }
*/

-- with clause with cross product
MATCH (a) WITH a MATCH (b) WITH a, b MATCH (c) WITH a, b, c RETURN a,b,c

/*
RootIR { names: [a, b, c] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@9 AS a@6, b@10 AS b@7, c@11 AS c@8] }
  └─IrSingleQueryPart
    ├─match_pattern
    │ └─QueryGraph { input_bindings: [a@3, b@4], nodes: [c@5] }
    ├─projection
    │ └─Project { items: [a@6 AS a@3, b@7 AS b@4, c@8 AS c@5] }
    └─IrSingleQueryPart
      ├─match_pattern
      │ └─QueryGraph { input_bindings: [a@1], nodes: [b@2] }
      ├─projection
      │ └─Project { items: [a@3 AS a@1, b@4 AS b@2] }
      └─IrSingleQueryPart
        ├─match_pattern
        │ └─QueryGraph { nodes: [a@0] }
        └─projection
          └─Project { items: [a@1 AS a@0] }
RootPlan { names: [a, b, c] }
└─ProduceResult { return_columns: a@9,b@10,c@11 }
  └─Project { exprs: [a@9 AS a@6, b@10 AS b@7, c@11 AS c@8] }
    └─Apply
      ├─Project { exprs: [a@6 AS a@3, b@7 AS b@4, c@8 AS c@5] }
      │ └─Apply
      │   ├─Project { exprs: [a@3 AS a@1, b@4 AS b@2] }
      │   │ └─Apply
      │   │   ├─Project { exprs: [a@1 AS a@0] }
      │   │   │ └─AllNodeScan { variable: a@0 }
      │   │   └─AllNodeScan { variable: b@2, arguments: [a@1] }
      │   └─AllNodeScan { variable: c@5, arguments: [a@3, b@4] }
      └─Argument { variables: [a@6, b@7, c@8] }
*/

