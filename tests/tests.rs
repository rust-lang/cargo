#![feature(macro_rules)]
#![feature(phase)]

extern crate term;
extern crate cargo;
extern crate hamcrest;

#[phase(plugin, link)]
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
