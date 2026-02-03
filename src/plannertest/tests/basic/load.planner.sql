-- Load a CSV file and create nodes
LOAD CSV FROM 'https://example.com/data.csv' AS row 
CREATE (:Person {name: row.name, age: row.age})

/*
RootIR { names: [row] }
└─IrSingleQueryPart
  ├─mutating_patterns
  │ └─CreatePattern { nodes: [(anon@1):Person create_map{name: row@0.name, age: row@0.age}], rels: [] }
  └─IrSingleQueryPart
    └─projection
      └─Load { variable: row@0, source_url: https://example.com/data.csv, format: CsvLoadFormat { header: true, delimiter: , } }
RootPlan { names: [row] }
└─ProduceResult { return_columns: row@0 }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: anon@1, labels: [Person], properties: create_map{name: row@0.name, age: row@0.age} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─Argument { variables: [row@0] }
RootPlan { names: [row] }
└─ProduceResult { return_columns: row@0 }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: anon@1, labels: [Person], properties: create_map{name: row@0.name, age: row@0.age} }] }
      └─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
*/

-- Load a CSV file and create relationships
LOAD CSV FROM 'https://example.com/data.csv' AS row 
MATCH (f:Forum {id: toInteger(row.`Forum.id`)}), (p:Post {id: toInteger(row.`Post.id`)})
CREATE (f)-[:CONTAINER_OF]->(p)

/*
RootIR { names: [row, f, p] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [row@0], nodes: [f@1, p@2], filter: f@1:Forum AND eq(f@1.id, tointeger(row@0.Forum.id)) AND p@2:Post AND eq(p@2.id, tointeger(row@0.Post.id)) }
  ├─mutating_patterns
  │ └─CreatePattern { nodes: [], rels: [(f@1)-[anon@3:CONTAINER_OF]->(p@2) create_map{}] }
  └─IrSingleQueryPart
    └─projection
      └─Load { variable: row@0, source_url: https://example.com/data.csv, format: CsvLoadFormat { header: true, delimiter: , } }
RootPlan { names: [row, f, p] }
└─ProduceResult { return_columns: row@0,f@1,p@2 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: anon@3, reltype: CONTAINER_OF, start_node: f@1, end_node: p@2, properties: create_map{} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─CrossProduct
          ├─Filter { condition: f@1:Forum AND eq(f@1.id, tointeger(row@0.Forum.id)) }
          │ └─AllNodeScan { variable: f@1, arguments: [row@0] }
          └─Filter { condition: p@2:Post AND eq(p@2.id, tointeger(row@0.Post.id)) }
            └─AllNodeScan { variable: p@2, arguments: [row@0] }
*/

-- Create a unique constraint on the forum id
CREATE CONSTRAINT forum_key FOR (f:Forum) REQUIRE (f.id) IS NODE KEY

/*

*/

-- Create a unique constraint on the post id
CREATE CONSTRAINT post_key FOR (p:Post) REQUIRE (p.id) IS NODE KEY

/*

*/

-- Load a CSV file and create relationships with index seek
LOAD CSV FROM 'https://example.com/data.csv' AS row 
MATCH (f:Forum {id: toInteger(row.`Forum.id`)}), (p:Post {id: toInteger(row.`Post.id`)})
CREATE (f)-[:CONTAINER_OF]->(p)

/*
RootIR { names: [row, f, p] }
└─IrSingleQueryPart
  ├─match_pattern
  │ └─QueryGraph { input_bindings: [row@0], nodes: [f@1, p@2], filter: f@1:Resolved(Forum, 0) AND eq(f@1.Resolved(id, 1), tointeger(row@0.Forum.id)) AND p@2:Resolved(Post, 2) AND eq(p@2.Resolved(id, 1), tointeger(row@0.Post.id)) }
  ├─mutating_patterns
  │ └─CreatePattern { nodes: [], rels: [(f@1)-[anon@3:CONTAINER_OF]->(p@2) create_map{}] }
  └─IrSingleQueryPart
    └─projection
      └─Load { variable: row@0, source_url: https://example.com/data.csv, format: CsvLoadFormat { header: true, delimiter: , } }
RootPlan { names: [row, f, p] }
└─ProduceResult { return_columns: row@0,f@1,p@2 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: anon@3, reltype: CONTAINER_OF, start_node: f@1, end_node: p@2, properties: create_map{} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─CrossProduct
          ├─NodeIndexSeek { variable: f@1, label: ResolvedIrToken(Forum, 0), constraint: forum_key, properties: [ResolvedIrToken(id, 1) = tointeger(row@0.Forum.id)] }
          └─NodeIndexSeek { variable: p@2, label: ResolvedIrToken(Post, 2), constraint: post_key, properties: [ResolvedIrToken(id, 1) = tointeger(row@0.Post.id)] }
RootPlan { names: [row, f, p] }
└─ProduceResult { return_columns: row@0,f@1,p@2 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: anon@3, reltype: CONTAINER_OF, start_node: f@1, end_node: p@2, properties: create_map{} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─CrossProduct
          ├─NodeIndexSeek { variable: f@1, label: ResolvedIrToken(Forum, 0), constraint: forum_key, properties: [ResolvedIrToken(id, 1) = tointeger(row@0.Forum.id)] }
          └─NodeIndexSeek { variable: p@2, label: ResolvedIrToken(Post, 2), constraint: post_key, properties: [ResolvedIrToken(id, 1) = tointeger(row@0.Post.id)] }
*/

