use std::os;

use cargo;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
struct Options;

pub const USAGE: &'static str = "
Usage:
    cargo version [options]

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
";

pub fn execute(_: Options, _: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-version; args={:?}", os::args());

    println!("{}", cargo::version());

    Ok(None)
}
