use std::env;

use cargo::core::Workspace;
use cargo::ops::{CompileOptions, CompileMode};
use cargo::ops;
use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::util::{CliResult, CliError, Config, human};

#[derive(RustcDecodable)]
pub struct Options {
    arg_opts: Option<Vec<String>>,
    flag_package: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_release: bool,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
    flag_profile: Option<String>,
}

pub const USAGE: &'static str = "
Compile a package and all of its dependencies

Usage:
    cargo rustc [options] [--] [<opts>...]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to build
    -j N, --jobs N           Number of parallel jobs, defaults to # of CPUs
    --lib                    Build only this package's library
    --bin NAME               Build only the specified binary
    --example NAME           Build only the specified example
    --test NAME              Build only the specified test target
    --bench NAME             Build only the specified benchmark target
    --release                Build artifacts in release mode, with optimizations
    --profile PROFILE        Profile to build the selected target for
    --features FEATURES      Features to compile for the package
    --no-default-features    Do not compile default features for the package
    --target TRIPLE          Target triple which compiles will be for
    --manifest-path PATH     Path to the manifest to fetch dependencies for
    -v, --verbose ...        Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

The specified target for the current package (or package specified by SPEC if
provided) will be compiled along with all of its dependencies. The specified
<opts>... will all be passed to the final compiler invocation, not any of the
dependencies. Note that the compiler will still unconditionally receive
arguments such as -L, --extern, and --crate-type, and the specified <opts>...
will simply be added to the compiler invocation.

This command requires that only one target is being compiled. If more than one
target is available for the current package the filters of --lib, --bin, etc,
must be used to select which target is compiled. To pass flags to all compiler
processes spawned by Cargo, use the $RUSTFLAGS environment variable or the
`build.rustflags` configuration option.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-rustc; args={:?}",
           env::args().collect::<Vec<_>>());
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));

    let root = try!(find_root_manifest_for_wd(options.flag_manifest_path,
                                              config.cwd()));
    let mode = match options.flag_profile.as_ref().map(|t| &t[..]) {
        Some("dev") | None => CompileMode::Build,
        Some("test") => CompileMode::Test,
        Some("bench") => CompileMode::Bench,
        Some(mode) => {
            let err = human(format!("unknown profile: `{}`, use dev,
                                     test, or bench", mode));
            return Err(CliError::new(err, 101))
        }
    };

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        no_default_features: options.flag_no_default_features,
        spec: &options.flag_package.map_or(Vec::new(), |s| vec![s]),
        exec_engine: None,
        mode: mode,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin,
                                        &options.flag_test,
                                        &options.flag_example,
                                        &options.flag_bench),
        target_rustdoc_args: None,
        target_rustc_args: options.arg_opts.as_ref().map(|a| &a[..]),
    };

    let ws = try!(Workspace::new(&root, config));
    try!(ops::compile(&ws, &opts));
    Ok(None)
}

