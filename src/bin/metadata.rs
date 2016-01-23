extern crate cargo;
extern crate docopt;
extern crate rustc_serialize;
extern crate toml;

use std::path::PathBuf;

use cargo::ops::{output_metadata, OutputTo, OutputMetadataOptions};
use cargo::util::important_paths::find_root_manifest_for_wd;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
struct Options {
    flag_color: Option<String>,
    flag_features: Vec<String>,
    flag_format_version: u32,
    flag_manifest_path: Option<String>,
    flag_no_default_features: bool,
    flag_output_format: String,
    flag_output_path: Option<String>,
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
    -o, --output-path PATH     Path the output is written to, otherwise stdout is used
    -f, --output-format FMT    Output format [default: toml]
                               Valid values: toml, json
    --features FEATURES        Space-separated list of features
    --no-default-features      Do not include the `default` feature
    --manifest-path PATH       Path to the manifest
    --format-version VERSION   Format version [default: 1]
                               Valid values: 1
    -v, --verbose              Use verbose output
    -q, --quiet                No output printed to stdout
    --color WHEN               Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let manifest = try!(find_root_manifest_for_wd(options.flag_manifest_path, config.cwd()));

    let output_to = match options.flag_output_path {
        Some(path) => OutputTo::File(PathBuf::from(path)),
        None => OutputTo::StdOut
    };

    let options = OutputMetadataOptions {
        features: options.flag_features,
        manifest_path: &manifest,
        no_default_features: options.flag_no_default_features,
        output_format: options.flag_output_format,
        output_to: output_to,
        version: options.flag_format_version,
    };

    try!(output_metadata(options, config));
    Ok(None)
}
