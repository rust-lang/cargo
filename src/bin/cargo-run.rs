#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::io::process::ExitStatus;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Run the main binary of the local package (src/main.rs)

Usage:
    cargo-run [options] [--] [<args>...]

Options:
    -h, --help              Print this message
    -j N, --jobs N          The number of jobs to run in parallel
    -u, --update-remotes    Deprecated option, use `cargo update` instead
    --manifest-path PATH    Path to the manifest to execute
    -v, --verbose           Use verbose output

All of the trailing arguments are passed as to the binary to run.
",  flag_jobs: Option<uint>, flag_target: Option<String>,
    flag_manifest_path: Option<String>)

fn main() {
    execute_main_without_stdin(execute, true);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let mut compile_opts = ops::CompileOptions {
        update: options.flag_update_remotes,
        env: "compile",
        shell: shell,
        jobs: options.flag_jobs,
        target: None,
        dev_deps: true,
    };

    let err = try!(ops::run(&root, &mut compile_opts,
                            options.arg_args.as_slice()).map_err(|err| {
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

