use std::env;

use cargo::ops;
use cargo::util::{CliResult, CliError, Config};

#[derive(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_bin: bool,
    arg_path: String,
    flag_name: Option<String>,
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
    --bin               Use a binary instead of a library template
    --name <name>       Set the resulting package name
    -v, --verbose       Use verbose output
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={:?}", env::args().collect::<Vec<_>>());
    config.shell().set_verbose(options.flag_verbose);

    let Options { flag_bin, arg_path, flag_name, flag_vcs, .. } = options;

    let opts = ops::NewOptions {
        version_control: flag_vcs,
        bin: flag_bin,
        path: &arg_path,
        name: flag_name.as_ref().map(|s| s.as_ref()),
    };

    ops::new(opts, config).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


