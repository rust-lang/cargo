use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::{find_root_manifest_for_wd};

#[derive(RustcDecodable)]
struct Options {
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_manifest_path: Option<String>,
    arg_spec: Option<String>,
}

pub const USAGE: &'static str = "
Print a fully qualified package specification

Usage:
    cargo pkgid [options] [<spec>]

Options:
    -h, --help               Print this message
    --manifest-path PATH     Path to the manifest to the package to clean
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

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
               config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let root = try!(find_root_manifest_for_wd(options.flag_manifest_path.clone(), config.cwd()));

    let spec = options.arg_spec.as_ref().map(|s| &s[..]);
    let spec = try!(ops::pkgid(&root, spec, config));
    println!("{}", spec);
    Ok(None)
}

