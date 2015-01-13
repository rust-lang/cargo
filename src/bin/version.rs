use std::os;

use cargo;
use cargo::core::MultiShell;
use cargo::util::CliResult;

#[derive(RustcDecodable)]
struct Options;

pub const USAGE: &'static str = "
Usage:
    cargo version [options]

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
";

pub fn execute(_: Options, _: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-version; args={:?}", os::args());

    println!("{}", cargo::version());

    Ok(None)
}
