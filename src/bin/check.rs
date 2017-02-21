use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, CompileOptions, MessageFormat, Packages};
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

pub const USAGE: &'static str = "
Check a local package and all of its dependencies for errors

Usage:
    cargo check [options]

Options:
    -h, --help                   Print this message
    -p SPEC, --package SPEC ...  Package(s) to check
    --all                        Check all packages in the workspace
    -j N, --jobs N               Number of parallel jobs, defaults to # of CPUs
    --lib                        Check only this package's library
    --bin NAME                   Check only the specified binary
    --example NAME               Check only the specified example
    --test NAME                  Check only the specified test target
    --bench NAME                 Check only the specified benchmark target
    --release                    Check artifacts in release mode, with optimizations
    --features FEATURES          Space-separated list of features to also check
    --all-features               Check all available features
    --no-default-features        Do not check the `default` feature
    --target TRIPLE              Check for the target triple
    --manifest-path PATH         Path to the manifest to compile
    -v, --verbose ...            Use verbose output
    -q, --quiet                  No output printed to stdout
    --color WHEN                 Coloring: auto, always, never
    --message-format FMT         Error format: human, json [default: human]
    --frozen                     Require Cargo.lock and cache are up to date
    --locked                     Require Cargo.lock is up to date

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
";

#[derive(RustcDecodable)]
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
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
    flag_locked: bool,
    flag_frozen: bool,
    flag_all: bool,
}

pub fn execute(options: Options, config: &Config) -> CliResult {
    debug!("executing; cmd=cargo-check; args={:?}",
           env::args().collect::<Vec<_>>());

    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;
    let ws = Workspace::new(&root, config)?;

    let spec = if options.flag_all {
        Packages::All
    } else {
        Packages::Packages(&options.flag_package)
    };

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: spec,
        mode: ops::CompileMode::Check,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin,
                                        &options.flag_test,
                                        &options.flag_example,
                                        &options.flag_bench),
        message_format: options.flag_message_format,
        target_rustdoc_args: None,
        target_rustc_args: None,
    };

    ops::compile(&ws, &opts)?;
    Ok(())
}
