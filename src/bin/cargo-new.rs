#![feature(phase)]

extern crate cargo;

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

extern crate serialize;

use std::os;
use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};

#[deriving(PartialEq,Clone,Decodable,Encodable)]
pub struct Options {
    git: bool,
    bin: bool,
    rest: Vec<String>,
}

hammer_config!(Options "Create a new cargo project")

fn main() {
    cargo::execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-new; args={}", os::args());

    let Options { git, mut rest, bin } = options;

    let path = match rest.remove(0) {
        Some(path) => path,
        None => return Err(CliError::new("must have a path as an argument", 1))
    };

    let opts = ops::NewOptions {
        git: git,
        path: path.as_slice(),
        bin: bin,
    };

    ops::new(opts, shell).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}


