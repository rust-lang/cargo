use std::env;

use cargo::ops;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
pub struct Options {
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
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
    --vcs VCS           Initialize a new repository for the given version
                        control system (git or hg) or do not initialize any version
                        control at all (none) overriding a global configuration.
    --bin               Use a binary instead of a library template
    --name NAME         Set the resulting package name
    -v, --verbose       Use verbose output
    -q, --quiet         No output printed to stdout
    --color WHEN        Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={:?}", env::args().collect::<Vec<_>>());
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let Options { flag_bin, arg_path, flag_name, flag_vcs, .. } = options;

    let opts = ops::NewOptions {
        version_control: flag_vcs,
        bin: flag_bin,
        path: &arg_path,
        name: flag_name.as_ref().map(|s| s.as_ref()),
    };

    try!(ops::new(opts, config));
    Ok(None)
}

