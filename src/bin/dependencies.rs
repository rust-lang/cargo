use std::os;
use cargo::ops;
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[derive(RustcDecodable)]
struct Options {
    flag_output_path: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Output the resolved dependencies of a project, the concrete used versions
including overrides, in a TOML format

Usage:
    cargo dependencies [options]

Options:
    -h, --help              Print this message
    -o, --output-path PATH  Path the output is written to, otherwise stdout is used
    --manifest-path PATH    Path to the manifest
    -v, --verbose           Use verbose output

The TOML format is e.g.:

   [dependencies.libA]
   version = \"0.1\",
   path = '/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libA-0.1'
   dependencies = [\"libB\"]
   
   [dependencies.libB]
   version = \"0.4\",
   path = '/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/libB-0.4'
   dependencies = []

";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-dependencies; args={:?}", os::args());
    config.shell().set_verbose(options.flag_verbose);

    let manifest = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let out = options.flag_output_path
        .map_or(ops::OutputTo::StdOut, |str| ops::OutputTo::Path(Path::new(str)));

    let options = ops::OutputOptions { out: out, config: config };

    ops::output_dependencies(&manifest, &options)
        .map(|_| None)
        .map_err(|err| CliError::from_boxed(err, 101))
}
