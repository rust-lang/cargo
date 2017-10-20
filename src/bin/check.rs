use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, CompileOptions, MessageFormat, Packages};
use cargo::util::{CliResult, CliError, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

pub const USAGE: &'static str = "
Check a local package and all of its dependencies for errors

Usage:
    cargo check [options]

Options:
    -h, --help                   Print this message
    -p SPEC, --package SPEC ...  Package(s) to check
    --all                        Check all packages in the workspace
    --exclude SPEC ...           Exclude packages from the check
    -j N, --jobs N               Number of parallel jobs, defaults to # of CPUs
    --lib                        Check only this package's library
    --bin NAME                   Check only the specified binary
    --bins                       Check all binaries
    --example NAME               Check only the specified example
    --examples                   Check all examples
    --test NAME                  Check only the specified test target
    --tests                      Check all tests
    --bench NAME                 Check only the specified bench target
    --benches                    Check all benches
    --all-targets                Check all targets (lib and bin targets by default)
    --release                    Check artifacts in release mode, with optimizations
    --profile PROFILE            Profile to build the selected target for
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
    -Z FLAG ...                  Unstable (nightly-only) flags to Cargo

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are checked if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.

The `--profile test` flag can be used to check unit tests with the
`#[cfg(test)]` attribute.
";

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
    flag_all_targets: bool,
    flag_locked: bool,
    flag_frozen: bool,
    flag_all: bool,
    flag_exclude: Vec<String>,
    flag_profile: Option<String>,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    debug!("executing; cmd=cargo-check; args={:?}",
           env::args().collect::<Vec<_>>());

    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;
    let ws = Workspace::new(&root, config)?;

    let spec = Packages::from_flags(ws.is_virtual(),
                                    options.flag_all,
                                    &options.flag_exclude,
                                    &options.flag_package)?;

    let test = match options.flag_profile.as_ref().map(|t| &t[..]) {
            Some("test") => true,
            None => false,
            Some(profile) => {
                let err = format!("unknown profile: `{}`, only `test` is currently supported",
                                  profile).into();
                return Err(CliError::new(err, 101))
            }
        };

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: spec,
        mode: ops::CompileMode::Check{test:test},
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin, options.flag_bins,
                                        &options.flag_test, options.flag_tests,
                                        &options.flag_example, options.flag_examples,
                                        &options.flag_bench, options.flag_benches,
                                        options.flag_all_targets),
        message_format: options.flag_message_format,
        target_rustdoc_args: None,
        target_rustc_args: None,
    };

    ops::compile(&ws, &opts)?;
    Ok(())
}
