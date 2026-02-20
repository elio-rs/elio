-- match without index - uses AllNodeScan + Filter
MATCH (n:User {id: '123'}) RETURN n

/*
RootPlan { names: [n] }
└─ProduceResult { return_columns: n@0 }
  └─Project { exprs: [n@0 AS n@0] }
    └─Filter { condition: n@0:User AND eq(n@0.id, '123') }
      └─AllNodeScan { variable: n@0 }
*/

-- create unique constraint
CREATE CONSTRAINT person_email_unique FOR (p:Person) REQUIRE p.email IS UNIQUE

/*

*/

-- match with index - uses NodeIndexSeek
MATCH (p:Person {email: 'alice@example.com'}) RETURN p

/*
RootPlan { names: [p] }
└─ProduceResult { return_columns: p@0 }
  └─Project { exprs: [p@0 AS p@0] }
    └─NodeIndexSeek { variable: p@0, label: ResolvedIrToken(Person, 0), index: person_email_unique, properties: [ResolvedIrToken(email, 1) = 'alice@example.com'] }
*/

-- match with partial index coverage - NodeIndexSeek + Filter for remaining
MATCH (p:Person {email: 'alice@example.com', name: 'Alice'}) RETURN p

/*
RootPlan { names: [p] }
└─ProduceResult { return_columns: p@0 }
  └─Project { exprs: [p@0 AS p@0] }
    └─Filter { condition: eq(p@0.name, 'Alice') }
      └─NodeIndexSeek { variable: p@0, label: ResolvedIrToken(Person, 0), index: person_email_unique, properties: [ResolvedIrToken(email, 1) = 'alice@example.com'] }
*/

