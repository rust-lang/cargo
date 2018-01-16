#![allow(dead_code)]

trait Foo {}

struct Bar<'a> {
    w: &'a Foo + Send,
}

fn main() {
}
