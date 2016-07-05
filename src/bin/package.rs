use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(RustcDecodable)]
pub struct Options {
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_manifest_path: Option<String>,
    flag_no_verify: bool,
    flag_no_metadata: bool,
    flag_list: bool,
    flag_allow_dirty: bool,
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
    --allow-dirty           Allow dirty working directories to be packaged
    --manifest-path PATH    Path to the manifest to compile
    -v, --verbose ...       Use verbose output
    -q, --quiet             No output printed to stdout
    --color WHEN            Coloring: auto, always, never

";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));
    let root = try!(find_root_manifest_for_wd(options.flag_manifest_path, config.cwd()));
    let ws = try!(Workspace::new(&root, config));
    try!(ops::package(&ws, &ops::PackageOpts {
        config: config,
        verify: !options.flag_no_verify,
        list: options.flag_list,
        check_metadata: !options.flag_no_metadata,
        allow_dirty: options.flag_allow_dirty,
    }));
    Ok(None)
}
