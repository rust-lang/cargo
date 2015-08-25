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
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_release: bool,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
}

pub const USAGE: &'static str = "
Compile a local package and all of its dependencies

Usage:
    cargo build [options]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to build
    -j N, --jobs N           The number of jobs to run in parallel
    --lib                    Build only this package's library
    --bin NAME               Build only the specified binary
    --example NAME           Build only the specified example
    --test NAME              Build only the specified test target
    --bench NAME             Build only the specified benchmark target
    --release                Build artifacts in release mode, with optimizations
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to compile
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-build; args={:?}",
           env::args().collect::<Vec<_>>());
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: options.flag_package.as_ref().map(|s| &s[..]),
        exec_engine: None,
        mode: ops::CompileMode::Build,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin,
                                        &options.flag_test,
                                        &options.flag_example,
                                        &options.flag_bench),
        target_rustc_args: None,
    };

    ops::compile(&root, &opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
