#![allow(dead_code)]

trait Foo {}

struct Bar<'a> {
    w: &'a (dyn Foo + Send),
}

fn main() {
}
