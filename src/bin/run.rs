use std::io::process::ExitStatus;

use cargo::ops;
use cargo::core::{MultiShell};
use cargo::core::manifest::{BinTarget, ExampleTarget};
use cargo::util::{CliResult, CliError, human};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[deriving(Decodable)]
struct Options {
    flag_name: Option<String>,
    flag_example: Option<String>,
    flag_jobs: Option<uint>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_release: bool,
    arg_args: Vec<String>,
}

pub const USAGE: &'static str = "
Run the main binary of the local package (src/main.rs)

Usage:
    cargo run [options] [--] [<args>...]

Options:
    -h, --help              Print this message
    --name NAME             Name of the bin target to run
    --example NAME          Name of the example target to run
    -j N, --jobs N          The number of jobs to run in parallel
    --release               Build artifacts in release mode, with optimizations
    --features FEATURES     Space-separated list of features to also build
    --no-default-features   Do not build the `default` feature
    --target TRIPLE         Build for the target triple
    --manifest-path PATH    Path to the manifest to execute
    -v, --verbose           Use verbose output

If neither `--name` or `--example` are given, then if the project only has one
bin target it will be run. Otherwise `--name` specifies the bin target to run,
and `--example` specifies the example target to run. At most one of `--name` or
`--example` can be provided.

All of the trailing arguments are passed as to the binary to run.
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let env = if options.flag_example.is_some() {
        "test"
    } else if options.flag_release {
        "release"
    } else {
        "compile"
    };

    let mut compile_opts = ops::CompileOptions {
        env: env,
        shell: shell,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| t.as_slice()),
        dev_deps: true,
        features: options.flag_features.as_slice(),
        no_default_features: options.flag_no_default_features,
        spec: None,
        lib_only: false
    };

    let (target_kind, name) = match (options.flag_name, options.flag_example) {
        (Some(bin), None) => (BinTarget, Some(bin)),
        (None, Some(example)) => (ExampleTarget, Some(example)),
        (None, None) => (BinTarget, None),
        (Some(_), Some(_)) => return Err(CliError::from_boxed(
            human("specify either `--name` or `--example`, not both"), 1)),
    };

    let err = try!(ops::run(&root,
                            target_kind,
                            name,
                            &mut compile_opts,
                            options.arg_args.as_slice()).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            Err(match err.exit {
                Some(ExitStatus(i)) => CliError::from_boxed(box err, i as uint),
                _ => CliError::from_boxed(box err, 101),
            })
        }
    }
}
