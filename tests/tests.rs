#![feature(macro_rules)]
#![allow(deprecated_owned_vector)]
#![feature(phase)]

extern crate term;
extern crate cargo;
extern crate hamcrest;

#[phase(syntax, link)]
extern crate log;

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
