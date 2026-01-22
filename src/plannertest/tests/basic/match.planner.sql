-- match without label
MATCH (n) RETURN n

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [n@0] }
  └─projection
    └─Project { items: [n@1 AS n@0] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@1 }
  └─Project { exprs: [n@1 AS n@0] }
    └─AllNodeScan { variable: n@0 }
*/

-- match with label
MATCH (n:Person) RETURN n

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [n@0], filter: n@0:Person }
  └─projection
    └─Project { items: [n@1 AS n@0] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@1 }
  └─Project { exprs: [n@1 AS n@0] }
    └─Filter { condition: n@0:Person }
      └─AllNodeScan { variable: n@0 }
*/

-- match and return wild card
MATCH (n:Person) RETURN *

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [n@0], filter: n@0:Person }
  └─projection
    └─Project { items: [n@0 AS n@0] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@0 }
  └─Project { exprs: [n@0 AS n@0] }
    └─Filter { condition: n@0:Person }
      └─AllNodeScan { variable: n@0 }
*/

-- match with projection
MATCH (n:Person) RETURN n.name

/*
RootIR { names: [n.name] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [n@0], filter: n@0:Person }
  └─projection
    └─Project { items: [nname@1 AS n@0.name] }
RootPlan { names: [n.name] }
└─ProduceResult { return_columns: nname@1 }
  └─Project { exprs: [nname@1 AS n@0.name] }
    └─Filter { condition: n@0:Person }
      └─AllNodeScan { variable: n@0 }
*/

-- match with cross product
MATCH (a:Person), (b:Person) WHERE a.age > b.age RETURN a, b

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1], filter: a@0:Person AND b@1:Person AND gt(a@0.age, b@1.age) }
  └─projection
    └─Project { items: [a@2 AS a@0, b@3 AS b@1] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@2,b@3 }
  └─Project { exprs: [a@2 AS a@0, b@3 AS b@1] }
    └─Filter { condition: gt(a@0.age, b@1.age) }
      └─CrossProduct
        ├─Filter { condition: a@0:Person }
        │ └─AllNodeScan { variable: a@0 }
        └─Filter { condition: b@1:Person }
          └─AllNodeScan { variable: b@1 }
*/

-- match with cross product and overlapping variables
MATCH (a:Person)--(b), (c:Person)--(d) WHERE a.age > c.age RETURN a, b, c, d

/*
RootIR { names: [a, b, c, d] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { nodes: [a@0, b@1, c@3, d@4], rels: [(a@0)<-[anon@2:]->(b@1), (c@3)<-[anon@5:]->(d@4)], filter: a@0:Person AND c@3:Person AND gt(a@0.age, c@3.age) }
  └─projection
    └─Project { items: [a@6 AS a@0, b@7 AS b@1, c@8 AS c@3, d@9 AS d@4] }
RootPlan { names: [a, b, c, d] }
└─ProduceResult { return_columns: a@6,b@7,c@8,d@9 }
  └─Project { exprs: [a@6 AS a@0, b@7 AS b@1, c@8 AS c@3, d@9 AS d@4] }
    └─Filter { condition: gt(a@0.age, c@3.age) }
      └─CrossProduct
        ├─Filter { condition: a@0:Person }
        │ └─ExpandAll { from: a@0, to: b@1, rel: anon@2, direction: -, types: [] }
        │   └─AllNodeScan { variable: a@0 }
        └─Filter { condition: c@3:Person }
          └─ExpandAll { from: c@3, to: d@4, rel: anon@5, direction: -, types: [] }
            └─AllNodeScan { variable: c@3 }
*/

