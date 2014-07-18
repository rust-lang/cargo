use std::io;
use docopt;

use cargo::ops;
use cargo::core::{MultiShell};
use cargo::sources::RegistrySource;
use cargo::util::{CliResult, CliError};

docopt!(Options, "
Save an api token from the registry locally

Usage:
    cargo login [options] [<token>]

Options:
    -h, --help              Print this message
    --host HOST             Host to set the token for
    -v, --verbose           Use verbose output

",  arg_token: Option<String>, flag_host: Option<String>)

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
    try!(ops::upload_login(shell, token).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}

