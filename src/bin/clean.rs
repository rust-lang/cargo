use std::os;
use docopt;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

docopt!(Options, "
Remove artifacts that cargo has generated in the past

Usage:
    cargo clean [options]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to the package to clean
    -v, --verbose           Use verbose output
",  flag_manifest_path: Option<String>)

pub fn execute(options: Options, _shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-clean; args={}", os::args());

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    ops::clean(&root).map(|_| None).map_err(|err| {
      CliError::from_boxed(err, 101)
    })
}
