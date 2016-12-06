use std::env;

use cargo::ops::{self};
use cargo::ops::cargo_check::{Options, with_check_env};
use cargo::util::{CliResult, Config};


pub const USAGE: &'static str = "
Check a local package and all of its dependencies for errors

Usage:
    cargo check [options]

Options:
    -h, --help                   Print this message
    -p SPEC, --package SPEC ...  Package to check
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

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-check; args={:?}",
           env::args().collect::<Vec<_>>());

    with_check_env(options, config, |ws, opts| {
        ops::compile(ws, opts)?;
        Ok(None)
    })
}
