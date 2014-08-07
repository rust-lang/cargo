#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;
#[phase(plugin, link)] extern crate log;

use std::os;
use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

docopt!(Options, "
Generate the lockfile for a project

Usage:
    cargo-generate-lockfile [options]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to generate a lockfile for
    -v, --verbose           Use verbose output
",  flag_manifest_path: Option<String>)

fn main() {
    execute_main_without_stdin(execute, false);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-generate-lockfile; args={}", os::args());
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    ops::generate_lockfile(&root, shell)
        .map(|_| None).map_err(|err| CliError::from_boxed(err, 101))
}
