#![crate_name = "cargo-run"]
#![feature(phase)]

#[phase(plugin, link)]
extern crate cargo;
extern crate serialize;

#[phase(plugin, link)]
extern crate hammer;

use std::os;
use std::io::process::ExitStatus;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_project_manifest;

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: Option<String>,
    jobs: Option<uint>,
    update: bool,
    rest: Vec<String>,
}

hammer_config!(Options "Run the package's main executable", |c| {
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
        env: "compile",
        shell: shell,
        jobs: options.jobs,
        target: None,
    };

    let err = try!(ops::run(&root, &mut compile_opts,
                            options.rest.as_slice()).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            Err(match err.exit {
                Some(ExitStatus(i)) => CliError::from_boxed(box err, i as uint),
                _ => CliError::from_boxed(box err, 101),
            })
        }
    }
}

