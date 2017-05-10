use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, CompileOptions, MessageFormat, Packages};
use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::util::{CliResult, Config};

#[derive(Deserialize)]
pub struct Options {
    flag_package: Vec<String>,
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
    flag_locked: bool,
    flag_frozen: bool,
    flag_all: bool,
    flag_exclude: Vec<String>,
}

pub const USAGE: &'static str = "
Compile a local package and all of its dependencies

Usage:
    cargo build [options]

Options:
    -h, --help                   Print this message
    -p SPEC, --package SPEC ...  Package to build
    --all                        Build all packages in the workspace
    --exclude SPEC ...           Exclude packages from the build
    -j N, --jobs N               Number of parallel jobs, defaults to # of CPUs
    --lib                        Build only this package's library
    --bin NAME                   Build only the specified binary
    --bins                       Build all binaries
    --example NAME               Build only the specified example
    --examples                   Build all examples
    --test NAME                  Build only the specified test target
    --tests                      Build all tests
    --bench NAME                 Build only the specified bench target
    --benches                    Build all benches
    --release                    Build artifacts in release mode, with optimizations
    --features FEATURES          Space-separated list of features to also build
    --all-features               Build all available features
    --no-default-features        Do not build the `default` feature
    --target TRIPLE              Build for the target triple
    --manifest-path PATH         Path to the manifest to compile
    -v, --verbose ...            Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet                  No output printed to stdout
    --color WHEN                 Coloring: auto, always, never
    --message-format FMT         Error format: human, json [default: human]
    --frozen                     Require Cargo.lock and cache are up to date
    --locked                     Require Cargo.lock is up to date

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are built if the `--all` flag is supplied. The
`--all` flag may be supplied in the presence of a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
";

pub fn execute(options: Options, config: &Config) -> CliResult {
    debug!("executing; cmd=cargo-build; args={:?}",
           env::args().collect::<Vec<_>>());
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;
    let ws = Workspace::new(&root, config)?;

    let spec = if options.flag_all || ws.is_virtual() {
        Packages::All
    } else {
    	Packages::from_flags(options.flag_all,
                          	&options.flag_exclude,
                          	&options.flag_package)?
    };

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: spec,
        mode: ops::CompileMode::Build,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin, options.flag_bins,
                                        &options.flag_test, options.flag_tests,
                                        &options.flag_example, options.flag_examples,
                                        &options.flag_bench, options.flag_benches,),
        message_format: options.flag_message_format,
        target_rustdoc_args: None,
        target_rustc_args: None,
    };

    ops::compile(&ws, &opts)?;
    Ok(())
}
