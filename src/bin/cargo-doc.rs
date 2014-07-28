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
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_project_manifest;

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: Option<String>,
    jobs: Option<uint>,
    update: bool,
    no_deps: bool,
}

hammer_config!(Options "Build the package's documentation", |c| {
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

    let mut doc_opts = ops::DocOptions {
        all: !options.no_deps,
        compile_opts: ops::CompileOptions {
            update: options.update,
            env: if options.no_deps {"doc"} else {"doc-all"},
            shell: shell,
            jobs: options.jobs,
            target: None,
        },
    };

    try!(ops::doc(&root, &mut doc_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    Ok(None)
}

