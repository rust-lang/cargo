use std::os;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct Options {
    flag_package: Option<String>,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Remove artifacts that cargo has generated in the past

Usage:
    cargo clean [options]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to clean artifacts for
    --manifest-path PATH     Path to the manifest to the package to clean
    --target TRIPLE          Target triple to clean output for (default all)
    -v, --verbose            Use verbose output

If the --package argument is given, then SPEC is a package id specification
which indicates which package's artifacts should be cleaned out. If it is not
given, then all packages' artifacts are removed. For more information on SPEC
and its format, see the `cargo help pkgid` command.
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    debug!("executing; cmd=cargo-clean; args={}", os::args());

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    let mut opts = ops::CleanOptions {
        shell: shell,
        spec: options.flag_package.as_ref().map(|s| s.as_slice()),
        target: options.flag_target.as_ref().map(|s| s.as_slice()),
    };
    ops::clean(&root, &mut opts).map(|_| None).map_err(|err| {
      CliError::from_boxed(err, 101)
    })
}
