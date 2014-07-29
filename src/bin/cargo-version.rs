#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;
#[phase(plugin, link)] extern crate log;

use std::os;
use cargo::execute_main_without_stdin;
use cargo::core::MultiShell;
use cargo::util::CliResult;

docopt!(Options, "
Usage:
    cargo-version [options]

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
")

fn main() {
    execute_main_without_stdin(execute, false);
}

fn execute(_: Options, _: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-version; args={}", os::args());

    println!("cargo {}", env!("CFG_VERSION"));

    Ok(None)
}
