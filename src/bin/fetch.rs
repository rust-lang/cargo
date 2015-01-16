use cargo::ops;
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[derive(RustcDecodable)]
struct Options {
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Fetch dependencies of a package from the network.

Usage:
    cargo fetch [options]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to fetch dependencies for
    -v, --verbose           Use verbose output

If a lockfile is available, this command will ensure that all of the git
dependencies and/or registries dependencies are downloaded and locally
available. The network is never touched after a `cargo fetch` unless
the lockfile changes.

If the lockfile is not available, then this is the equivalent of
`cargo generate-lockfile`. A lockfile is generated and dependencies are also
all updated.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    try!(ops::fetch(&root, config).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}


