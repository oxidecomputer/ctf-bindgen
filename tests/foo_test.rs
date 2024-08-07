// mod foo;

#[test]
fn test_foo() {
    let foo = unsafe { crate::foo::boo(47) };
    assert_eq!(foo.bar, 47);
    assert_eq!(foo.baz, 7.0);
}
