#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::io::process::ExitStatus;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util;
use cargo::util::{CliResult, CliError, CargoError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Execute all unit and integration tests of a local package

Usage:
    cargo-test [options] [--] [<args>...]

Options:
    -h, --help              Print this message
    -j N, --jobs N          The number of jobs to run in parallel
    -u, --update-remotes    Update all remote packages before compiling
    --manifest-path PATH    Path to the manifest to build tests for
    -v, --verbose           Use verbose output

All of the trailing arguments are passed to the test binaries generated for
filtering tests and generally providing options configuring how they run.
",  flag_jobs: Option<uint>, flag_target: Option<String>,
    flag_manifest_path: Option<String>)

fn main() {
    execute_main_without_stdin(execute, true);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    shell.set_verbose(options.flag_verbose);

    let mut compile_opts = ops::CompileOptions {
        update: options.flag_update_remotes,
        env: "test",
        shell: shell,
        jobs: options.flag_jobs,
        target: None,
    };

    let test_executables = try!(ops::compile(&root,
                                             &mut compile_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));

    let test_dir = root.dir_path().join("target").join("test");

    for file in test_executables.iter() {
        try!(util::process(test_dir.join(file.as_slice()))
                  .args(options.arg_args.as_slice())
                  .exec().map_err(|e| {
            let exit_status = match e.exit {
                Some(ExitStatus(i)) => i as uint,
                _ => 1,
            };
            CliError::from_boxed(e.mark_human(), exit_status)
        }));
    }

    Ok(None)
}
