use std::env;

use cargo::ops::CompileOptions;
use cargo::ops;
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::util::{CliResult, CliError, Config};

#[derive(RustcDecodable)]
struct Options {
    flag_package: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_release: bool,
    flag_lib: bool
}

pub const USAGE: &'static str = "
Compile a local package and all of its dependencies

Usage:
    cargo build [options]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to build
    -j N, --jobs N           The number of jobs to run in parallel
    --lib                    Build only lib (if present in package)
    --release                Build artifacts in release mode, with optimizations
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to compile
    -v, --verbose            Use verbose output

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-build; args={:?}", env::args().collect::<Vec<_>>());
    config.shell().set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let env = if options.flag_release {
        "release"
    } else {
        "compile"
    };

    let opts = CompileOptions {
        env: env,
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        dev_deps: false,
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: options.flag_package.as_ref().map(|s| &s[..]),
        lib_only: options.flag_lib,
        exec_engine: None,
    };

    ops::compile(&root, &opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
