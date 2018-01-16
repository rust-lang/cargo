trait Foo {}

struct Bar<'a> {
    w: &'a (Foo + Copy),
}

fn main() {
}
