use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::{find_root_manifest_for_wd};

#[derive(Deserialize)]
pub struct Options {
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_manifest_path: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    flag_package: Option<String>,
    arg_spec: Option<String>,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Print a fully qualified package specification

Usage:
    cargo pkgid [options] [<spec>]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Argument to get the package id specifier for
    --manifest-path PATH     Path to the manifest to the package to clean
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo

Given a <spec> argument, print out the fully qualified package id specifier.
This command will generate an error if <spec> is ambiguous as to which package
it refers to in the dependency graph. If no <spec> is given, then the pkgid for
the local package is printed.

This command requires that a lockfile is available and dependencies have been
fetched.

Example Package IDs

           pkgid                  |  name  |  version  |          url
    |-----------------------------|--------|-----------|---------------------|
     foo                          | foo    | *         | *
     foo:1.2.3                    | foo    | 1.2.3     | *
     crates.io/foo                | foo    | *         | *://crates.io/foo
     crates.io/foo#1.2.3          | foo    | 1.2.3     | *://crates.io/foo
     crates.io/bar#foo:1.2.3      | foo    | 1.2.3     | *://crates.io/bar
     http://crates.io/foo#1.2.3   | foo    | 1.2.3     | http://crates.io/foo

";

pub fn execute(options: Options,
               config: &Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;
    let root = find_root_manifest_for_wd(options.flag_manifest_path.clone(), config.cwd())?;
    let ws = Workspace::new(&root, config)?;

    let spec = if options.arg_spec.is_some() {
        options.arg_spec
    } else if options.flag_package.is_some() {
        options.flag_package
    } else {
        None
    };
    let spec = spec.as_ref().map(|s| &s[..]);
    let spec = ops::pkgid(&ws, spec)?;
    println!("{}", spec);
    Ok(())
}

