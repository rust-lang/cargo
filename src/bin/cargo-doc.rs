#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::os;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_project_manifest;

docopt!(Options, "
Build a package's documentation

Usage:
    cargo-doc [options]

Options:
    -h, --help              Print this message
    --no-deps               Don't build documentation for dependencies
    -j N, --jobs N          The number of jobs to run in parallel
    -u, --update-remotes    Update all remote packages before compiling
    --manifest-path PATH    Path to the manifest to compile
    -v, --verbose           Use verbose output

By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.
",  flag_jobs: Option<uint>,
    flag_manifest_path: Option<String>)

fn main() {
    execute_main_without_stdin(execute, false)
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = match options.flag_manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project_manifest(&os::getcwd(), "Cargo.toml")
                    .map_err(|_| {
                        CliError::new("Could not find Cargo.toml in this \
                                       directory or any parent directory",
                                      102)
                    }))
    };

    let mut doc_opts = ops::DocOptions {
        all: !options.flag_no_deps,
        compile_opts: ops::CompileOptions {
            update: options.flag_update_remotes,
            env: if options.flag_no_deps {"doc"} else {"doc-all"},
            shell: shell,
            jobs: options.flag_jobs,
            target: None,
        },
    };

    try!(ops::doc(&root, &mut doc_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    Ok(None)
}

