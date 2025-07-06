use std::path::PathBuf;

use cargo_test_support::{ArgLineCommandExt, Execs, execs, process};

pub mod cross_compile;
pub mod ext;
pub mod tools;

/// Run `cargo $arg_line`, see [`Execs`]
pub fn cargo_process(arg_line: &str) -> Execs {
    let cargo = cargo_exe();
    let mut p = process(&cargo);
    p.env("CARGO", cargo);
    p.arg_line(arg_line);
    execs().with_process_builder(p)
}

/// Path to the cargo binary
pub fn cargo_exe() -> PathBuf {
    snapbox::cmd::cargo_bin!("cargo").to_path_buf()
}
