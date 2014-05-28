#![feature(macro_rules)]
#![allow(deprecated_owned_vector)]

extern crate term;
extern crate cargo;
extern crate hamcrest;

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
mod test_shell;
