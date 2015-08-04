use cargo::ops;
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[derive(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_manifest_path: Option<String>,
    flag_no_verify: bool,
    flag_no_metadata: bool,
    flag_list: bool,
}

pub const USAGE: &'static str = "
Assemble the local package into a distributable tarball

Usage:
    cargo package [options]

Options:
    -h, --help              Print this message
    -l, --list              Print files included in a package without making one
    --no-verify             Don't verify the contents by building them
    --no-metadata           Ignore warnings about a lack of human-usable metadata
    --manifest-path PATH    Path to the manifest to compile
    -v, --verbose           Use verbose output
    -q, --quiet             No output printed to stdout
    --color WHEN            Coloring: auto, always, never

";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    ops::package(&root, config,
                 !options.flag_no_verify,
                 options.flag_list,
                 !options.flag_no_metadata).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
