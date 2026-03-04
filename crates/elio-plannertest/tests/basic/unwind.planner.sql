-- unwind a literal list
UNWIND [1, 2, 3] AS x RETURN x

/*
RootIR { names: [x] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [x@0 AS x@0] }
  └─IrSingleQueryPart
    └─projection
      └─UnwindProjection { variable: x@0, expr: [1, 2, 3] }
RootPlan { names: [x] }
└─ProduceResult { return_columns: x@0 }
  └─Project { exprs: [x@0 AS x@0] }
    └─Apply
      ├─Unwind { variable: x@0, expr: [1, 2, 3] }
      │ └─Unit
      └─Argument { variables: [x@0] }
*/

-- unwind a variable from with clause
MATCH (a) WITH a, a.friends AS friends UNWIND friends AS f RETURN a, f

/*
RootIR { names: [a, f] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [a@0 AS a@0, f@2 AS f@2] }
  └─IrSingleQueryPart
    ├─projection
    │ └─UnwindProjection { variable: f@2, expr: friends@1 }
    └─IrSingleQueryPart
      ├─match_pattern
      │ └─QueryGraph { nodes: [a@0] }
      └─projection
        └─Project { items: [a@0 AS a@0, friends@1 AS a@0.friends] }
RootPlan { names: [a, f] }
└─ProduceResult { return_columns: a@0,f@2 }
  └─Project { exprs: [a@0 AS a@0, f@2 AS f@2] }
    └─Apply
      ├─Unwind { variable: f@2, expr: friends@1 }
      │ └─Apply
      │   ├─Project { exprs: [a@0 AS a@0, friends@1 AS a@0.friends] }
      │   │ └─AllNodeScan { variable: a@0 }
      │   └─Argument { variables: [a@0, friends@1] }
      └─Argument { variables: [a@0, friends@1, f@2] }
*/

-- unwind with where filter
UNWIND [1, 2, 3] AS x WITH x WHERE x > 1 RETURN x

/*
RootIR { names: [x] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [x@0 AS x@0] }
  └─IrSingleQueryPart
    ├─projection
    │ └─Project { items: [x@0 AS x@0], filter: gt(x@0, 1) }
    └─IrSingleQueryPart
      └─projection
        └─UnwindProjection { variable: x@0, expr: [1, 2, 3] }
RootPlan { names: [x] }
└─ProduceResult { return_columns: x@0 }
  └─Project { exprs: [x@0 AS x@0] }
    └─Apply
      ├─Filter { condition: gt(x@0, 1) }
      │ └─Project { exprs: [x@0 AS x@0] }
      │   └─Apply
      │     ├─Unwind { variable: x@0, expr: [1, 2, 3] }
      │     │ └─Unit
      │     └─Argument { variables: [x@0] }
      └─Argument { variables: [x@0] }
*/

-- nested unwind
UNWIND [[1, 2], [3, 4]] AS x UNWIND x AS y RETURN y

/*
RootIR { names: [y] }
└─IrSingleQueryPart
  ├─projection
  │ └─Project { items: [y@1 AS y@1] }
  └─IrSingleQueryPart
    ├─projection
    │ └─UnwindProjection { variable: y@1, expr: x@0 }
    └─IrSingleQueryPart
      └─projection
        └─UnwindProjection { variable: x@0, expr: [[1, 2], [3, 4]] }
RootPlan { names: [y] }
└─ProduceResult { return_columns: y@1 }
  └─Project { exprs: [y@1 AS y@1] }
    └─Apply
      ├─Unwind { variable: y@1, expr: x@0 }
      │ └─Apply
      │   ├─Unwind { variable: x@0, expr: [[1, 2], [3, 4]] }
      │   │ └─Unit
      │   └─Argument { variables: [x@0] }
      └─Argument { variables: [x@0, y@1] }
*/

