-- Create a node with properties, without return clause
CREATE (n:Person {name: 'Alice', age: 30})

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  └─mutating_patterns
    └─CreatePattern { nodes: [(n@0):Person create_map{name: 'Alice', age: 30}], rels: [] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@0 }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: n@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }] }
      └─Unit
*/

-- Create a node with properties, with return clause
CREATE (n:Person {name: 'Alice', age: 30}) RETURN *

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  ├─mutating_patterns
  │ └─CreatePattern { nodes: [(n@0):Person create_map{name: 'Alice', age: 30}], rels: [] }
  └─projection
    └─Project { items: [n@0 AS n@0] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@0 }
  └─Project { exprs: [n@0 AS n@0] }
    └─CreateNode { items: [CreateNodeItem { variable: n@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }] }
      └─Unit
*/

-- Create a node with properties, with return clause, only return node
CREATE (n:Person {name: 'Alice', age: 30}) RETURN n

/*
RootIR { names: [n] }
└─IrSingleQueryPart
  ├─mutating_patterns
  │ └─CreatePattern { nodes: [(n@0):Person create_map{name: 'Alice', age: 30}], rels: [] }
  └─projection
    └─Project { items: [n@0 AS n@0] }
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@0 }
  └─Project { exprs: [n@0 AS n@0] }
    └─CreateNode { items: [CreateNodeItem { variable: n@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }] }
      └─Unit
*/

-- create multiple nodes
CREATE (n:Person {name: 'Alice', age: 30}), (m:Person {name: 'Bob', age: 31})

/*
RootIR { names: [n, m] }
└─IrSingleQueryPart
  └─mutating_patterns
    └─CreatePattern { nodes: [(n@0):Person create_map{name: 'Alice', age: 30}, (m@1):Person create_map{name: 'Bob', age: 31}], rels: [] }
RootPlan { names: [n, m] }
└─ProduceResult { return_columns: n@0,m@1 }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: n@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }, CreateNodeItem { variable: m@1, labels: [Person], properties: create_map{name: 'Bob', age: 31} }] }
      └─Unit
*/

-- create without variable
CREATE (:Person{name: 'Alice', age: 30}), (:Person{name: 'Bob', age: 31})

/*
RootIR { names: [] }
└─IrSingleQueryPart
  └─mutating_patterns
    └─CreatePattern { nodes: [(anon@0):Person create_map{name: 'Alice', age: 30}, (anon@1):Person create_map{name: 'Bob', age: 31}], rels: [] }
RootPlan { names: [] }
└─ProduceResult { return_columns:  }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: anon@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }, CreateNodeItem { variable: anon@1, labels: [Person], properties: create_map{name: 'Bob', age: 31} }] }
      └─Unit
*/

-- create multiple nodes with relationships
CREATE (a:Person {name: 'Alice', age: 30}), (b:Person {name: 'Bob', age: 31}), (a)-[:KNOWS]->(b)

/*
RootIR { names: [a, b] }
└─IrSingleQueryPart
  └─mutating_patterns
    └─CreatePattern { nodes: [(a@0):Person create_map{name: 'Alice', age: 30}, (b@1):Person create_map{name: 'Bob', age: 31}], rels: [(a@0)-[anon@2:KNOWS]->(b@1) create_map{}] }
RootPlan { names: [a, b] }
└─ProduceResult { return_columns: a@0,b@1 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: anon@2, reltype: KNOWS, start_node: a@0, end_node: b@1, properties: create_map{} }] }
      └─CreateNode { items: [CreateNodeItem { variable: a@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }, CreateNodeItem { variable: b@1, labels: [Person], properties: create_map{name: 'Bob', age: 31} }] }
        └─Unit
*/

-- create left direction relationship
CREATE (a:Person {name: 'Alice', age: 30}), (b:Person {name: 'Bob', age: 31}), (a)<-[r:KNOWS]-(b)

/*
RootIR { names: [a, b, r] }
└─IrSingleQueryPart
  └─mutating_patterns
    └─CreatePattern { nodes: [(a@0):Person create_map{name: 'Alice', age: 30}, (b@1):Person create_map{name: 'Bob', age: 31}], rels: [(a@0)<-[r@2:KNOWS]-(b@1) create_map{}] }
RootPlan { names: [a, b, r] }
└─ProduceResult { return_columns: a@0,b@1,r@2 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: r@2, reltype: KNOWS, start_node: b@1, end_node: a@0, properties: create_map{} }] }
      └─CreateNode { items: [CreateNodeItem { variable: a@0, labels: [Person], properties: create_map{name: 'Alice', age: 30} }, CreateNodeItem { variable: b@1, labels: [Person], properties: create_map{name: 'Bob', age: 31} }] }
        └─Unit
*/

