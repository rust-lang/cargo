use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, CompileOptions, CompileMode, MessageFormat, Packages};
use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::util::{CliResult, CliError, Config};

#[derive(Deserialize)]
pub struct Options {
    arg_opts: Option<Vec<String>>,
    flag_package: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_all_features: bool,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_message_format: MessageFormat,
    flag_release: bool,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_bins: bool,
    flag_example: Vec<String>,
    flag_examples: bool,
    flag_test: Vec<String>,
    flag_tests: bool,
    flag_bench: Vec<String>,
    flag_benches: bool,
    flag_all_targets: bool,
    flag_profile: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
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
    --bins                   Build all binaries
    --example NAME           Build only the specified example
    --examples               Build all examples
    --test NAME              Build only the specified test target
    --tests                  Build all tests
    --bench NAME             Build only the specified bench target
    --benches                Build all benches
    --all-targets            Build all targets (lib and bin targets by default)
    --release                Build artifacts in release mode, with optimizations
    --profile PROFILE        Profile to build the selected target for
    --features FEATURES      Features to compile for the package
    --all-features           Build all available features
    --no-default-features    Do not compile default features for the package
    --target TRIPLE          Target triple which compiles will be for
    --manifest-path PATH     Path to the manifest to fetch dependencies for
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --message-format FMT     Error format: human, json [default: human]
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo

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

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    debug!("executing; cmd=cargo-rustc; args={:?}",
           env::args().collect::<Vec<_>>());
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path,
                                         config.cwd())?;
    let mode = match options.flag_profile.as_ref().map(|t| &t[..]) {
        Some("dev") | None => CompileMode::Build,
        Some("test") => CompileMode::Test,
        Some("bench") => CompileMode::Bench,
        Some("check") => CompileMode::Check {test: false},
        Some(mode) => {
            let err = format!("unknown profile: `{}`, use dev,
                                     test, or bench", mode).into();
            return Err(CliError::new(err, 101))
        }
    };

    let spec = options.flag_package.map_or_else(Vec::new, |s| vec![s]);

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: Packages::Packages(&spec),
        mode: mode,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin, options.flag_bins,
                                        &options.flag_test, options.flag_tests,
                                        &options.flag_example, options.flag_examples,
                                        &options.flag_bench, options.flag_benches,
                                        options.flag_all_targets),
        message_format: options.flag_message_format,
        target_rustdoc_args: None,
        target_rustc_args: options.arg_opts.as_ref().map(|a| &a[..]),
    };

    let ws = Workspace::new(&root, config)?;
    ops::compile(&ws, &opts)?;
    Ok(())
}

