extern crate cargo;
extern crate docopt;
extern crate rustc_serialize;
extern crate toml;

use cargo::ops::{output_metadata, OutputMetadataOptions, ExportInfo};
use cargo::util::important_paths::find_root_manifest_for_wd;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
pub struct Options {
    flag_color: Option<String>,
    flag_features: Vec<String>,
    flag_format_version: u32,
    flag_manifest_path: Option<String>,
    flag_no_default_features: bool,
    flag_no_deps: bool,
    flag_quiet: bool,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Output the resolved dependencies of a project, the concrete used versions
including overrides, in machine-readable format.

Usage:
    cargo metadata [options]

Options:
    -h, --help                 Print this message
    --features FEATURES        Space-separated list of features
    --no-default-features      Do not include the `default` feature
    --no-deps                  Output information only about the root package
                               and don't fetch dependencies.
    --manifest-path PATH       Path to the manifest
    --format-version VERSION   Format version [default: 1]
                               Valid values: 1
    -v, --verbose              Use verbose output
    -q, --quiet                No output printed to stdout
    --color WHEN               Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<ExportInfo>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let manifest = try!(find_root_manifest_for_wd(options.flag_manifest_path, config.cwd()));

    let options = OutputMetadataOptions {
        features: options.flag_features,
        manifest_path: &manifest,
        no_default_features: options.flag_no_default_features,
        no_deps: options.flag_no_deps,
        version: options.flag_format_version,
    };

    let result = try!(output_metadata(options, config));
    Ok(Some(result))
}
