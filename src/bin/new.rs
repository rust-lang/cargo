use std::env;
use std::str::{FromStr};

use cargo::ops;
use cargo::util::{CliError, CliResult, Config};

#[derive(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_bin: bool,
    arg_path: String,
    flag_name: Option<String>,
    flag_vcs: Option<ops::VersionControl>,
    flag_license: Option<String>,
}

pub const USAGE: &'static str = r#"
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
    --license LICENSES  License(s) to add to a project
                        Multiple licenses should be separated by a '/' character
                        (Supported values, case insensitive: "MIT", "BSD-3-Clause", "APACHE-2.0",
                         "GPL-3.0", "MPL-2.0")
"#;

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={:?}", env::args().collect::<Vec<_>>());
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let Options { flag_bin, arg_path, flag_name, flag_vcs, flag_license, .. } = options;

    let opts = ops::NewOptions {
        version_control: flag_vcs,
        bin: flag_bin,
        path: &arg_path,
        name: flag_name.as_ref().map(|s| s.as_ref()),
        license: match flag_license {
            Some(input) => {
                let mut licenses: Vec<ops::License> = vec![];
                let split = input.split("/").collect::<Vec<_>>();
                for l in &split {
                    let l = l.trim();
                    licenses.push(match FromStr::from_str(l) {
                        Ok(lic) => lic,
                        _ => return Err(CliError::new(&format!("Unrecognised license '{}'", l),
                                                      127)),
                    });
                }
                Some(licenses)
            },
            None => None
        },
    };

    try!(ops::new(opts, config));
    Ok(None)
}

