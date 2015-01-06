use std::os;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

#[derive(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_bin: bool,
    flag_travis: bool,
    arg_path: String,
    flag_vcs: Option<ops::VersionControl>,
}

pub const USAGE: &'static str = "
Create a new cargo package at <path>

Usage:
    cargo new [options] <path>
    cargo new -h | --help

Options:
    -h, --help          Print this message
    --vcs <vcs>         Initialize a new repository for the given version
                        control system (git or hg) or do not initialize any version 
                        control at all (none) overriding a global configuration. 
    --travis            Create a .travis.yml file
    --bin               Use a binary instead of a library template
    -v, --verbose       Use verbose output
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let Options { flag_travis, flag_bin, arg_path, flag_vcs, .. } = options;

    let opts = ops::NewOptions {
        version_control: flag_vcs,
        travis: flag_travis,
        path: arg_path.as_slice(),
        bin: flag_bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


