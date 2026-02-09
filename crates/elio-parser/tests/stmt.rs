use elio_parser::parser::cypher_parser;
use insta::assert_snapshot;

macro_rules! stmt {
    ($query:expr) => {
        cypher_parser::statement($query).unwrap().to_string()
    };
}

#[test]
fn test_query() {
    assert_snapshot!(stmt!("RETURN n"), @"RETURN n");
    assert_snapshot!(stmt!("RETURN n ORDER BY n.name"), @"RETURN n ORDER BY n.name Asc");
    assert_snapshot!(stmt!("MATCH (n:Person) RETURN n.name"), @"MATCH (n:Person) RETURN n.name");
    assert_snapshot!(stmt!("MATCH (n:Person) RETURN n.name AS name"), @"MATCH (n:Person) RETURN n.name AS name");
    assert_snapshot!(stmt!("MATCH (n:Person) RETURN n.name AS name ORDER BY name"), @"MATCH (n:Person) RETURN n.name AS name ORDER BY name Asc");
    assert_snapshot!(stmt!("MATCH (a)-[]-(b) WITH a.x + b.x AS c RETURN c"), @"MATCH (a)-[]-(b) WITH (a.x) + (b.x) AS c RETURN c");
    assert_snapshot!(stmt!("MATCH (a)-[]-(b) WITH a.x + b.x AS c RETURN c ORDER BY c"), @"MATCH (a)-[]-(b) WITH (a.x) + (b.x) AS c RETURN c ORDER BY c Asc");
    assert_snapshot!(stmt!("MATCH (a)-[]-(b) WITH a.x + b.x AS c WHERE c.y > 100 RETURN c ORDER BY c DESC SKIP 1 LIMIT 1"), @"MATCH (a)-[]-(b) WITH (a.x) + (b.x) AS c WHERE (c.y) > (100) RETURN c ORDER BY c Desc SKIP 1 LIMIT 1");
    // test new line
    assert_snapshot!(stmt!(r#"MATCH (n:Person) 
    RETURN n.name"#), @"MATCH (n:Person) RETURN n.name");
}

#[test]
fn test_create_constraint() {
    // Node unique constraint (single property)
    assert_snapshot!(
        stmt!("CREATE CONSTRAINT person_name_unique FOR (p:Person) REQUIRE p.name IS UNIQUE"),
        @"CREATE CONSTRAINT person_name_unique FOR (p:Person) REQUIRE p.name IS UNIQUE"
    );

    // Node unique constraint with IF NOT EXISTS
    assert_snapshot!(
        stmt!("CREATE CONSTRAINT person_email_unique IF NOT EXISTS FOR (p:Person) REQUIRE p.email IS UNIQUE"),
        @"CREATE CONSTRAINT person_email_unique IF NOT EXISTS FOR (p:Person) REQUIRE p.email IS UNIQUE"
    );

    // Node key constraint (composite key)
    assert_snapshot!(
        stmt!("CREATE CONSTRAINT person_key FOR (p:Person) REQUIRE (p.firstname, p.lastname) IS NODE KEY"),
        @"CREATE CONSTRAINT person_key FOR (p:Person) REQUIRE (p.firstname, p.lastname) IS NODE KEY"
    );

    // Not null constraint
    assert_snapshot!(
        stmt!("CREATE CONSTRAINT person_name_not_null FOR (p:Person) REQUIRE p.name IS NOT NULL"),
        @"CREATE CONSTRAINT person_name_not_null FOR (p:Person) REQUIRE p.name IS NOT NULL"
    );

    // Relationship unique constraint
    assert_snapshot!(
        stmt!("CREATE CONSTRAINT knows_since_unique FOR ()-[r:KNOWS]-() REQUIRE r.since IS UNIQUE"),
        @"CREATE CONSTRAINT knows_since_unique FOR ()-[r:KNOWS]-() REQUIRE r.since IS UNIQUE"
    );
}

#[test]
fn test_drop_constraint() {
    // Drop constraint
    assert_snapshot!(
        stmt!("DROP CONSTRAINT person_name_unique"),
        @"DROP CONSTRAINT person_name_unique"
    );

    // Drop constraint with IF EXISTS
    assert_snapshot!(
        stmt!("DROP CONSTRAINT person_name_unique IF EXISTS"),
        @"DROP CONSTRAINT person_name_unique IF EXISTS"
    );
}

#[test]
fn test_load() {
    assert_snapshot!(
        stmt!(r#"LOAD csv FROM 'https://example.com/data.csv' AS row
        CREATE (:Person {name: row.name, age: row.age})"#),
        @"LOAD csv FROM 'https://example.com/data.csv' AS row CREATE (:Person{name: row.name, age: row.age})"
    );
    assert_snapshot!(
        stmt!("LOAD csv FROM 'https://example.com/data.csv' AS row
        CREATE (:Person {name: row.name, age: row.age})"),
        @"LOAD csv FROM 'https://example.com/data.csv' AS row CREATE (:Person{name: row.name, age: row.age})"
    );
    assert_snapshot!(
        stmt!("LOAD csv FROM 'https://example.com/data.csv' AS row
        CREATE (:Person {name: row.name, age: row.age})"),
        @"LOAD csv FROM 'https://example.com/data.csv' AS row CREATE (:Person{name: row.name, age: row.age})"
    );
}
