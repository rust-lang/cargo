#![feature(phase)]

#[phase(plugin, link)]
extern crate cargo;
extern crate serialize;

#[phase(plugin, link)]
extern crate hammer;

use std::io;

use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell};
use cargo::core::source::CENTRAL;
use cargo::util::{CliResult, CliError};

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    host: Option<String>,
    rest: Vec<String>,
}

hammer_config!(Options "Save an api token from the registry locally")

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let Options { host, mut rest } = options;
    let config = try!(ops::upload_configuration().map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    let token = match rest.remove(0) {
        Some(token) => token,
        None => {
            let host = host.or(config.host).unwrap_or(CENTRAL.to_string());
            println!("please visit {}/me and paste the API Token below", host);
            try!(io::stdin().read_line().map_err(|e| {
                CliError::from_boxed(box e, 101)
            }))
        }
    };

    let token = token.as_slice().trim().to_string();
    try!(ops::upload_login(shell, token).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}

