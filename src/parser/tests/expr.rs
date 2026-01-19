use elio_parser::parser::cypher_parser;
use insta::assert_snapshot;

macro_rules! expr {
    ($query:expr) => {
        cypher_parser::expr($query).unwrap().to_string()
    };
}

macro_rules! label_expr {
    ($query:expr) => {
        cypher_parser::label_expr($query).unwrap().to_string()
    };
}

#[test]
fn test_escaped_ident() {
    assert_snapshot!(expr!("`a`"), @"a");
    assert_snapshot!(expr!("`a.b`"), @"a.b");
}

#[test]
fn test_literal() {
    assert_snapshot!(expr!("123"), @"123");
    assert_snapshot!(expr!("123.456"), @"123.456");
    assert_snapshot!(expr!("\"hello\""), @"'hello'");
    assert_snapshot!(expr!("true"), @"TRUE");
    assert_snapshot!(expr!("false"), @"FALSE");
    assert_snapshot!(expr!("null"), @"NULL");
    assert_snapshot!(expr!("-42"), @"-(42)");
    assert_snapshot!(expr!("not true"), @"NOT(TRUE)");
    assert_snapshot!(expr!("1 + 2"), @"(1) + (2)");
}

#[test]
fn test_func() {
    assert_snapshot!(expr!("func(1,2)"), @"func(1, 2)");
}

#[test]
fn test_operator() {
    assert_snapshot!(expr!("1 + 2"), @"(1) + (2)");
    assert_snapshot!(expr!("1 * 2"), @"(1) * (2)");
    assert_snapshot!(expr!("1 / 2"), @"(1) / (2)");
    assert_snapshot!(expr!("1 % 2"), @"(1) % (2)");
    assert_snapshot!(expr!("1 ^ 2"), @"(1) ^ (2)");
    assert_snapshot!(expr!("(1 + 2) * 3"), @"((1) + (2)) * (3)");
}

#[test]
fn test_compare() {
    assert_snapshot!(expr!("1 > 2"), @"(1) > (2)");
    assert_snapshot!(expr!("1 >= 2"), @"(1) >= (2)");
    assert_snapshot!(expr!("1 < 2"), @"(1) < (2)");
    assert_snapshot!(expr!("1 <= 2"), @"(1) <= (2)");
    assert_snapshot!(expr!("1 = 2"), @"(1) = (2)");
    assert_snapshot!(expr!("1 <> 2"), @"(1) <> (2)");
    assert_snapshot!(expr!("1 != 2"), @"(1) <> (2)");
}

#[test]
fn test_label_expr() {
    assert_snapshot!(label_expr!(":a|b"), @"(a|b)");
    assert_snapshot!(label_expr!(":a"), @"a");
    assert_snapshot!(label_expr!(":a&b"), @"(a&b)");
    assert_snapshot!(label_expr!(":a&b|c&d"), @"((a&b)|(c&d))");
}

#[test]
fn test_atom() {
    assert_snapshot!(expr!("n"), @"n");
    assert_snapshot!(expr!("n.name"), @"n.name");
}
