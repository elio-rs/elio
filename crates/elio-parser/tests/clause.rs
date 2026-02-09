use elio_parser::parser::cypher_parser;
use insta::assert_snapshot;

macro_rules! clause {
    ($input:expr) => {
        cypher_parser::clause($input).unwrap().to_string()
    };
}

macro_rules! return_item {
    ($input:expr) => {
        cypher_parser::return_item($input).unwrap().to_string()
    };
}

#[test]
fn test_create() {
    assert_snapshot!(clause!("CREATE (n:Person {name: 'Alice'})"), @"CREATE (n:Person{name: 'Alice'})");
    assert_snapshot!(clause!(
        "CREATE (a:Person{name: 'Alice'}), (b:Person{name: 'Bob'})"), 
        @"CREATE (a:Person{name: 'Alice'}), (b:Person{name: 'Bob'})");
    assert_snapshot!(clause!(
        "CREATE (a:Person{name: 'Alice'}), (b:Person{name: 'Bob'})-[:KNOWS]->(c:Person{name: 'Charlie'})"), 
        @"CREATE (a:Person{name: 'Alice'}), (b:Person{name: 'Bob'})-[:KNOWS]->(c:Person{name: 'Charlie'})");

    assert_snapshot!(clause!(
        "CREATE (a:Person{name: 'Alex', age: 30}), (b:Person{name: 'Bob', age: 20}), (a)-[:KNOWS]->(b), (b)-[:KNOWS]->(a)"),
        @"CREATE (a:Person{name: 'Alex', age: 30}), (b:Person{name: 'Bob', age: 20}), (a)-[:KNOWS]->(b), (b)-[:KNOWS]->(a)");
}

#[test]
fn test_match() {
    assert_snapshot!(clause!("MATCH (n:Person)"), @"MATCH (n:Person)");
    assert_snapshot!(clause!("MATCH (n:Person) WHERE n.name = 'Alice'"), @"MATCH (n:Person) WHERE (n.name) = ('Alice')");
    assert_snapshot!(clause!("MATCH (n:Person) WHERE n.name = 'Alice'"), @"MATCH (n:Person) WHERE (n.name) = ('Alice')");
}

#[test]
fn test_with() {
    assert_snapshot!(clause!("WITH n"), @"WITH n");
    assert_snapshot!(clause!("WITH a.x + b.y AS c"), @"WITH (a.x) + (b.y) AS c");
    assert_snapshot!(clause!("WITH *"), @"WITH *");
    assert_snapshot!(clause!("WITH *, a+b AS c"), @"WITH *, (a) + (b) AS c");
    assert_snapshot!(clause!("WITH *, x AS y"), @"WITH *, x AS y");
}

#[test]
fn test_return() {
    assert_snapshot!(clause!("RETURN n ORDER BY n.name"), @"RETURN n ORDER BY n.name Asc");
    assert_snapshot!(clause!("RETURN n.name"), @"RETURN n.name");
    assert_snapshot!(clause!("RETURN n.name AS name"), @"RETURN n.name AS name");
    assert_snapshot!(clause!("RETURN n.name AS name ORDER BY name"), @"RETURN n.name AS name ORDER BY name Asc");
    assert_snapshot!(clause!("RETURN n.name AS name ORDER BY name DESC"), @"RETURN n.name AS name ORDER BY name Desc");
    assert_snapshot!(clause!("RETURN n.name AS name ORDER BY name DESC SKIP 1 LIMIT 1"), @"RETURN n.name AS name ORDER BY name Desc SKIP 1 LIMIT 1");
}

#[test]
fn test_return_item() {
    assert_snapshot!(return_item!("n"), @"n");
}

#[test]
fn test_load() {
    // Basic load
    assert_snapshot!(clause!("LOAD csv FROM '/path/to/file.csv' AS row"), 
        @"LOAD csv FROM '/path/to/file.csv' AS row");

    // Load with different formats
    assert_snapshot!(clause!("LOAD parquet FROM '/data/users.parquet' AS r"), 
        @"LOAD parquet FROM '/data/users.parquet' AS r");
    assert_snapshot!(clause!("LOAD json FROM '/data/events.json' AS event"), 
        @"LOAD json FROM '/data/events.json' AS event");

    // Load with OPTIONS
    assert_snapshot!(clause!("LOAD csv FROM '/path/to/file.csv' OPTIONS {header: true} AS row"), 
        @"LOAD csv FROM '/path/to/file.csv' OPTIONS {header: TRUE} AS row");

    // Load with multiple options
    assert_snapshot!(clause!("LOAD csv FROM '/data/file.csv' OPTIONS {header: true, delimiter: ','} AS row"), 
        @"LOAD csv FROM '/data/file.csv' OPTIONS {header: TRUE, delimiter: ','} AS row");

    // Load with string, integer, and boolean options
    assert_snapshot!(clause!("LOAD csv FROM '/data/file.csv' OPTIONS {header: false, skip: 1, nullif: 'NA'} AS row"), 
        @"LOAD csv FROM '/data/file.csv' OPTIONS {header: FALSE, skip: 1, nullif: 'NA'} AS row");
}
