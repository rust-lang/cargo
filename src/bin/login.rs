use std::io;

use cargo::ops;
use cargo::core::{MultiShell};
use cargo::sources::RegistrySource;
use cargo::util::{CliResult, CliError};

#[deriving(Decodable)]
struct Options {
    flag_host: Option<String>,
    arg_token: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Save an api token from the registry locally

Usage:
    cargo login [options] [<token>]

Options:
    -h, --help              Print this message
    --host HOST             Host to set the token for
    -v, --verbose           Use verbose output

";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let token = match options.arg_token.clone() {
        Some(token) => token,
        None => {
            let default = RegistrySource::url().unwrap().to_string();
            let host = options.flag_host.unwrap_or(default);
            println!("please visit {}/me and paste the API Token below", host);
            try!(io::stdin().read_line().map_err(|e| {
                CliError::from_boxed(box e, 101)
            }))
        }
    };

    let token = token.as_slice().trim().to_string();
    try!(ops::registry_login(shell, token).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}

