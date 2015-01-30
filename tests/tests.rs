#![deny(warnings)]

extern crate cargo;
extern crate flate2;
extern crate git2;
extern crate hamcrest;
extern crate serialize;
extern crate tar;
extern crate term;
extern crate url;

#[macro_use]
extern crate log;

mod support;
macro_rules! test {
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
            ::support::paths::setup();
            setup();
            $expr;
        }
    )
}

mod test_bad_config;
mod test_cargo;
mod test_cargo_bench;
mod test_cargo_build_auth;
mod test_cargo_build_lib;
mod test_cargo_clean;
mod test_cargo_compile;
mod test_cargo_compile_custom_build;
mod test_cargo_compile_git_deps;
mod test_cargo_compile_path_deps;
mod test_cargo_compile_plugins;
mod test_cargo_cross_compile;
mod test_cargo_doc;
mod test_cargo_features;
mod test_cargo_fetch;
mod test_cargo_freshness;
mod test_cargo_generate_lockfile;
mod test_cargo_new;
mod test_cargo_package;
mod test_cargo_profiles;
mod test_cargo_publish;
mod test_cargo_registry;
mod test_cargo_run;
mod test_cargo_search;
mod test_cargo_test;
mod test_cargo_version;
mod test_shell;
