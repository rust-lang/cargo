#![crate_name="cargo-test"]
#![feature(phase)]

#[phase(plugin, link)]
extern crate cargo;
extern crate serialize;

#[phase(plugin, link)]
extern crate hammer;

use std::os;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util;
use cargo::util::{CliResult, CliError, CargoError};
use cargo::util::important_paths::find_project_manifest;

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: Option<String>,
    jobs: Option<uint>,
    update: bool,
    rest: Vec<String>,
}

hammer_config!(Options "Run the package's test suite", |c| {
    c.short("jobs", 'j').short("update", 'u')
})

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project_manifest(&os::getcwd(), "Cargo.toml")
                    .map_err(|_| {
                        CliError::new("Could not find Cargo.toml in this \
                                       directory or any parent directory",
                                      102)
                    }))
    };

    let mut compile_opts = ops::CompileOptions {
        update: options.update,
        env: "test",
        shell: shell,
        jobs: options.jobs,
        target: None,
    };

    let test_executables = try!(ops::compile(&root,
                                             &mut compile_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    let test_dir = root.dir_path().join("target").join("test");

    for file in test_executables.iter() {
        try!(util::process(test_dir.join(file.as_slice()))
                  .args(options.rest.as_slice())
                  .exec().map_err(|e| {
            CliError::from_boxed(e.box_error(), 1)
        }));
    }

    Ok(None)
}
