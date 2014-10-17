use std::os;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

#[deriving(Decodable)]
struct Options {
    flag_verbose: bool,
    flag_bin: bool,
    flag_travis: bool,
    flag_hg: bool,
    flag_git: bool,
    flag_no_git: bool,
    arg_path: String,
}

pub const USAGE: &'static str = "
Create a new cargo package at <path>

Usage:
    cargo new [options] <path>
    cargo new -h | --help

Options:
    -h, --help          Print this message
    --no-git            Don't initialize a new git repository
    --git               Initialize a new git repository, overriding a
                        global `git = false` configuration
    --hg                Initialize a new hg repository
    --travis            Create a .travis.yml file
    --bin               Use a binary instead of a library template
    -v, --verbose       Use verbose output
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let Options { flag_no_git, flag_travis,
                  flag_bin,arg_path, flag_git, flag_hg, .. } = options;

    let opts = ops::NewOptions {
        no_git: flag_no_git,
        git: flag_git,
        hg: flag_hg,
        travis: flag_travis,
        path: arg_path.as_slice(),
        bin: flag_bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


