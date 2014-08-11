#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::io::process::ExitStatus;

use cargo::ops;
use cargo::execute_main_without_stdin;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, CargoError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Execute all unit and integration tests of a local package

Usage:
    cargo-test [options] [--] [<args>...]

Options:
    -h, --help              Print this message
    -j N, --jobs N          The number of jobs to run in parallel
    --target TRIPLE         Build for the target triple
    -u, --update-remotes    Deprecated option, use `cargo update` instead
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
        target: options.flag_target.as_ref().map(|s| s.as_slice()),
        dev_deps: true,
    };

    let err = try!(ops::run_tests(&root, &mut compile_opts,
                                  options.arg_args.as_slice()).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            let status = match err.exit {
                Some(ExitStatus(i)) => i as uint,
                _ => 101,
            };
            Err(CliError::from_boxed(err.mark_human(), status))
        }
    }
}
