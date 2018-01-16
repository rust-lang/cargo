fn main() {
    let _x: &i32 = &&&&&5;

    struct Foo;
    fn foo(_x: &Foo) { }

    let x = &Foo;
    foo(x);
}
