use common_macros::ScalarPartialOrd;

trait ScalarPartialOrd {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>;
}

#[derive(ScalarPartialOrd, Debug, Clone)]
#[scalar_partial_ord(a, b)]
struct Foo {
    a: i32,
    b: i32,
}

#[derive(ScalarPartialOrd)]
#[scalar_partial_ord(_0)]
struct TupleFoo(i32);

#[derive(ScalarPartialOrd)]
#[scalar_partial_ord(id)]
struct GenericFoo<'a> {
    id: i32,
    #[allow(unused)]
    name: &'a str,
}

#[test]
fn test_struct() {
    let f1 = Foo { a: 1, b: 2 };
    let f2 = Foo { a: 1, b: 3 };
    assert_eq!(f1.scalar_partial_cmp(&f2), Some(std::cmp::Ordering::Less));

    let f3 = Foo { a: 2, b: 1 };
    assert_eq!(f1.scalar_partial_cmp(&f3), Some(std::cmp::Ordering::Less));

    let f4 = Foo { a: 1, b: 2 };
    assert_eq!(f1.scalar_partial_cmp(&f4), Some(std::cmp::Ordering::Equal));
}

#[test]
fn test_tuple() {
    let f1 = TupleFoo(1);
    let f2 = TupleFoo(2);
    assert_eq!(f1.scalar_partial_cmp(&f2), Some(std::cmp::Ordering::Less));

    let f3 = TupleFoo(1);
    assert_eq!(f1.scalar_partial_cmp(&f3), Some(std::cmp::Ordering::Equal));
}

#[test]
fn test_generic() {
    let f1 = GenericFoo { id: 1, name: "a" };
    let f2 = GenericFoo { id: 2, name: "b" };
    assert_eq!(f1.scalar_partial_cmp(&f2), Some(std::cmp::Ordering::Less));

    let f3 = GenericFoo { id: 1, name: "c" };
    assert_eq!(f1.scalar_partial_cmp(&f3), Some(std::cmp::Ordering::Equal));
}
