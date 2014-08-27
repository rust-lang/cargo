use std::os;
use docopt;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

docopt!(Options, "
Generate the lockfile for a project

Usage:
    cargo generate-lockfile [options]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to generate a lockfile for
    -v, --verbose           Use verbose output
",  flag_manifest_path: Option<String>)

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-generate-lockfile; args={}", os::args());
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    ops::generate_lockfile(&root, shell)
        .map(|_| None).map_err(|err| CliError::from_boxed(err, 101))
}
