use std::os;
use docopt;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

docopt!(Options, "
Create a new cargo package at <path>

Usage:
    cargo new [options] <path>
    cargo new -h | --help

Options:
    -h, --help          Print this message
    --git               Initialize a new git repository with a .gitignore
    --travis            Create a .travis.yml file
    --bin               Use a binary instead of a library template
    -v, --verbose       Use verbose output
")

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let Options { flag_git, flag_travis, flag_bin, arg_path, .. } = options;

    let opts = ops::NewOptions {
        git: flag_git,
        travis: flag_travis,
        path: arg_path.as_slice(),
        bin: flag_bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


