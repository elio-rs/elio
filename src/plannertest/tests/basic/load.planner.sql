-- Load a CSV file and create nodes
LOAD CSV FROM 'https://example.com/data.csv' AS row 
CREATE (:Person {name: row.name, age: row.age})

/*
RootIR { names: [row] }
└─IrSingleQueryPart
  ├─QueryGraph { imported: [row@0] }
  │ └─mutating_pattern
  │   └─CreatePattern { nodes: [(anon@1):Person create_map{name: row@0.name, age: row@0.age}], rels: [] }
  └─IrSingleQueryPart
    ├─QueryGraph
    └─Load { variable: row@0, source_url: https://example.com/data.csv, format: CsvLoadFormat { header: true, delimiter: , } }
RootPlan { names: [row] }
└─ProduceResult { return_columns: row@0 }
  └─BlackHole
    └─CreateNode { items: [CreateNodeItem { variable: anon@1, labels: [Person], properties: create_map{name: row@0.name, age: row@0.age} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─Argument { variables: [row@0] }
*/

-- Load a CSV file and create relationships
LOAD CSV FROM 'https://example.com/data.csv' AS row 
MATCH (f:Forum {id: toInteger(row.`Forum.id`)}), (p:Post {id: toInteger(row.`Post.id`)})
CREATE (f)-[:CONTAINER_OF]->(p)

/*
RootIR { names: [row, f, p] }
└─IrSingleQueryPart
  ├─QueryGraph { imported: [row@0], nodes: [f@1, p@2], filter: f@1:Forum AND eq(f@1.id, toInteger(row@0.Forum.id)) AND p@2:Post AND eq(p@2.id, toInteger(row@0.Post.id)) }
  │ └─mutating_pattern
  │   └─CreatePattern { nodes: [], rels: [(f@1)-[anon@3:CONTAINER_OF]->(p@2) create_map{}] }
  └─IrSingleQueryPart
    ├─QueryGraph
    └─Load { variable: row@0, source_url: https://example.com/data.csv, format: CsvLoadFormat { header: true, delimiter: , } }
RootPlan { names: [row, f, p] }
└─ProduceResult { return_columns: row@0,f@1,p@2 }
  └─BlackHole
    └─CreateRel { items: [CreateRelItem { variable: anon@3, reltype: CONTAINER_OF, start_node: f@1, end_node: p@2, properties: create_map{} }] }
      └─Apply
        ├─Load { source_url: https://example.com/data.csv, variable: row@0, format: CsvLoadFormat { header: true, delimiter: , } }
        └─CrossProduct
          ├─Filter { condition: f@1:Forum AND eq(f@1.id, toInteger(row@0.Forum.id)) }
          │ └─AllNodeScan { variable: f@1, arguments: [row@0] }
          └─Filter { condition: p@2:Post AND eq(p@2.id, toInteger(row@0.Post.id)) }
            └─AllNodeScan { variable: p@2, arguments: [row@0] }
*/

