#![crate_name="cargo-version"]
#![feature(phase)]

extern crate cargo;

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

extern crate serialize;

use std::os;
use cargo::execute_main_without_stdin;
use cargo::core::MultiShell;
use cargo::util::CliResult;

#[deriving(Decodable,Encodable)]
pub struct Options;

hammer_config!(Options)

 
fn main() {
    execute_main_without_stdin(execute);
}

fn execute(_: Options, _: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-version; args={}", os::args());

    println!("{}", env!("CFG_VERSION"));

    Ok(None)
}
