#[feature(macro_rules)];

extern crate cargo;

macro_rules! test(
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
            setup();
            $expr;
        }
    )
)

mod support;
mod test_cargo_compile;
