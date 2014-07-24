#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;
#[phase(plugin, link)] extern crate log;

use std::os;
use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

docopt!(Options, "
Create a new cargo package at <path>

Usage:
    cargo-new [options] <path>
    cargo-new -h | --help

Options:
    -h, --help          Print this message
    --git               Initialize a new git repository with a .gitignore
    --bin               Use a binary instead of a library template
    -v, --verbose       Use verbose output
")

fn main() {
    cargo::execute_main_without_stdin(execute, false)
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let Options { flag_git, flag_bin, arg_path, .. } = options;

    let opts = ops::NewOptions {
        git: flag_git,
        path: arg_path.as_slice(),
        bin: flag_bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


