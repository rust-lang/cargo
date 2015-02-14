use std::env;

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
    debug!("executing; cmd=cargo-version; args={:?}", env::args().collect::<Vec<_>>());

    println!("{}", cargo::version());

    Ok(None)
}
