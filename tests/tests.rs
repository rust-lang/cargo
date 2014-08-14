#![feature(macro_rules)]
#![feature(phase)]

extern crate term;
extern crate cargo;
extern crate hamcrest;
extern crate url;

#[phase(plugin, link)]
extern crate log;

mod support;
macro_rules! test(
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
            ::support::paths::setup();
            setup();
            $expr;
        }
    )
)

mod test_cargo;
mod test_cargo_bench;
mod test_cargo_clean;
mod test_cargo_compile;
mod test_cargo_compile_git_deps;
mod test_cargo_compile_path_deps;
mod test_cargo_test;
mod test_shell;
mod test_cargo_cross_compile;
mod test_cargo_run;
mod test_cargo_version;
mod test_cargo_new;
mod test_cargo_compile_plugins;
mod test_cargo_doc;
mod test_cargo_freshness;
mod test_cargo_generate_lockfile;
