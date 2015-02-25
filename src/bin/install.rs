use cargo::ops;
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct Options {
    flag_bin: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_prefix: Option<String>,
    flag_no_default_features: bool,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Install the main binary of the local package (src/main.rs)

Usage:
    cargo install [options]

Options:
    -h, --help              Print this message
    --bin NAME              Name of the bin target to run
    -j N, --jobs N          The number of jobs to run in parallel
    --features FEATURES     Space-separated list of features to also build
    --no-default-features   Do not build the `default` feature
    --manifest-path PATH    Path to the manifest to execute
    -v, --verbose           Use verbose output
    --prefix PATH           Installation prefix

If `--bin` is not given, then if the project only has one bin target it will
be installed. Otherwise `--bin` specifies the bin target to install.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let compile_opts = ops::CompileOptions {
        env: "release",
        config: config,
        jobs: options.flag_jobs,
        target: None,
        dev_deps: true,
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: None,
        lib_only: false,
        exec_engine: None,
    };

    ops::install(&root,
                 options.flag_bin,
                 options.flag_prefix,
                 &compile_opts).map_err(|err| {
        CliError::from_boxed(err, 101)
    }).map(|_| None)
}
