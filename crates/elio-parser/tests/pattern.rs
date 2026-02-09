use elio_parser::parser::cypher_parser::{self};
use insta::assert_snapshot;

macro_rules! pattern_part {
    ($query:expr) => {
        cypher_parser::pattern_part($query).unwrap().to_string()
    };
}

macro_rules! pattern {
    ($query:expr) => {
        cypher_parser::pattern($query)
            .unwrap()
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(" ,")
    };
}

macro_rules! quantified_path_pattern {
    ($query:expr) => {
        cypher_parser::quantified_path_pattern($query).unwrap().to_string()
    };
}

#[test]
fn test_node_pattern() {
    assert_snapshot!(pattern_part!("(n)"), @"(n)");
    assert_snapshot!(pattern_part!("(n:Person)"), @"(n:Person)");
    assert_snapshot!(pattern!("(n:Person)"), @"(n:Person)");
}

#[test]
fn test_rel_pattern() {
    assert_snapshot!(pattern_part!("()-[r]->()"), @"()-[r]->()");
    assert_snapshot!(pattern_part!("()-[r:REL]->()"), @"()-[r:REL]->()");
    assert_snapshot!(pattern_part!("()-[r:REL{a: 1}]->()"), @"()-[r:REL{a: 1}]->()");
}

#[test]
fn test_quantified() {
    assert_snapshot!(pattern_part!("(a1)-[]-(a2)"), @"(a1)-[]-(a2)");
    assert_snapshot!(quantified_path_pattern!("( (a1)-[]-(a2) WHERE a1.col1 > 10 )+"), @" ((a1)-[]-(a2) WHERE (a1.col1) > (10))+");
}

#[test]
fn test_quantified_pattern() {
    assert_snapshot!(pattern_part!("(a:Person) ( ()-[]-() ){1,5} (b)"), @"(a:Person) (()-[]-()){1,5} (b)");
    assert_snapshot!(pattern_part!("(a:Person) ( (a1)-[]-(a2) WHERE a1.col1 > 10 )+ (b)"), @"(a:Person) ((a1)-[]-(a2) WHERE (a1.col1) > (10))+ (b)");
}

#[test]
fn test_pattern_with_selector() {
    assert_snapshot!(pattern_part!("p = ALL PATHS (a:Person)-[]-(b)"), @"p = (a:Person)-[]-(b)");
    assert_snapshot!(pattern_part!("p = ANY 42 PATHS (a:Person)-[]-(b)"), @"p = ANY 42 PATHS (a:Person)-[]-(b)");
    assert_snapshot!(pattern_part!("p = ALL SHORTEST PATHS (a:Person)-[]-(b)"), @"p = ALL SHORTEST PATHS (a:Person)-[]-(b)");
    assert_snapshot!(pattern_part!("p = ANY SHORTEST PATHS (a:Person)-[]-(b)"), @"p = ANY SHORTEST PATHS (a:Person)-[]-(b)");
    assert_snapshot!(pattern_part!("p = SHORTEST 42 PATHS (a:Person)-[]-(b)"), @"p = SHORTEST 42 PATHS (a:Person)-[]-(b)");
    assert_snapshot!(pattern_part!("p = SHORTEST 42 PATH GROUPS (a:Person)-[]-(b)"), @"p = SHORTEST 42 PATH GROUPS (a:Person)-[]-(b)");
}

#[test]
fn test_variable_length() {
    assert_snapshot!(pattern_part!("(a)-[r]-(b)"), @"(a)-[r]-(b)");
    assert_snapshot!(pattern_part!("(a)-[r*2]-(b)"), @"(a)-[r*2..2]-(b)");
    assert_snapshot!(pattern_part!("(a)-[r*]-(b)"), @"(a)-[r*]-(b)");
    assert_snapshot!(pattern_part!("(a)-[r*1..]-(b)"), @"(a)-[r*1..18446744073709551615]-(b)");
    assert_snapshot!(pattern_part!("(a)-[r*..3]-(b)"), @"(a)-[r*1..3]-(b)");
    assert_snapshot!(pattern_part!("(a)-[r*1..3]-(b)"), @"(a)-[r*1..3]-(b)");
}
