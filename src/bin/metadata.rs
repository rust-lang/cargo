use cargo::ops::{output_metadata, OutputTo, OutputFormat, OutputMetadataOptions};
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[derive(RustcDecodable)]
struct Options {
    flag_output_path: OutputTo,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_output_format: OutputFormat,
    flag_features: String,
}

pub const USAGE: &'static str = r#"
Output the resolved dependencies of a project, the concrete used versions
including overrides, in machine-readable format.

Warning! This command is experimental and output format it subject to change in future.

Usage:
    cargo metadata [options]

Options:
    -h, --help               Print this message
    -o, --output-path PATH   Path the output is written to, otherwise stdout is used
    -f, --output-format FMT  Output format [default: toml]
                             Valid values: toml, json
    --features FEATURES      Comma-separated list of features [default: default]
    --manifest-path PATH     Path to the manifest
    -v, --verbose            Use verbose output

The TOML format is e.g.:

     root = "libA"

     [packages.libA]
     dependencies = ["libB"]
     path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libA-0.1"
     version = "0.1"

     [packages.libB]
     dependencies = []
     path = "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libB-0.4"
     version = "0.4"

     [packages.libB.features]
     featureA = ["featureB"]
     featureB = []

"#;

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);

    let manifest = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    let options = OutputMetadataOptions {
        manifest_path: &manifest,
        output_to: options.flag_output_path,
        output_format: options.flag_output_format,
        features: options.flag_features.split(',').map(|x| x.to_string()).collect::<Vec<String>>(),
    };

    output_metadata(options, config)
        .map(|_| None)
        .map_err(|err| CliError::from_boxed(err, 101))
}
