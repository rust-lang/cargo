use std::os;
use docopt;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Remove artifacts that cargo has generated in the past

Usage:
    cargo clean [options] [<spec>]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to the package to clean
    --target TRIPLE         Target triple to clean output for (default all)
    -v, --verbose           Use verbose output

If <spec> is provided, then it is interpreted as a package id specification and
only the output for the package specified will be removed. If <spec> is not
provided, then all output from cargo will be cleaned out. Note that a lockfile
must exist for <spec> to be given.

For more information about <spec>, see `cargo help pkgid`.
",  flag_manifest_path: Option<String>, arg_spec: Option<String>,
    flag_target: Option<String>)

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    debug!("executing; cmd=cargo-clean; args={}", os::args());

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    let mut opts = ops::CleanOptions {
        shell: shell,
        spec: options.arg_spec.as_ref().map(|s| s.as_slice()),
        target: options.flag_target.as_ref().map(|s| s.as_slice()),
    };
    ops::clean(&root, &mut opts).map(|_| None).map_err(|err| {
      CliError::from_boxed(err, 101)
    })
}
