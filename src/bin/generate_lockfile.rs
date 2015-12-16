use std::env;

use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(RustcDecodable)]
struct Options {
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Generate the lockfile for a project

Usage:
    cargo generate-lockfile [options]

Options:
    -h, --help               Print this message
    --manifest-path PATH     Path to the manifest to generate a lockfile for
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-generate-lockfile; args={:?}", env::args().collect::<Vec<_>>());
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let root = try!(find_root_manifest_for_wd(options.flag_manifest_path, config.cwd()));

    try!(ops::generate_lockfile(&root, config));
    Ok(None)
}
