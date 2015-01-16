use std::io;

use cargo::ops;
use cargo::core::{SourceId, Source};
use cargo::sources::RegistrySource;
use cargo::util::{CliResult, Config, human, ChainError};

#[derive(RustcDecodable)]
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

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);
    let token = match options.arg_token.clone() {
        Some(token) => token,
        None => {
            let src = try!(SourceId::for_central(config));
            let mut src = RegistrySource::new(&src, config);
            try!(src.update());
            let config = try!(src.config());
            let host = options.flag_host.clone().unwrap_or(config.api);
            println!("please visit {}me and paste the API Token below",
                     host);
            try!(io::stdin().read_line().chain_error(|| {
                human("failed to read API token from stdin")
            }))
        }
    };

    let token = token.as_slice().trim().to_string();
    try!(ops::registry_login(config, token));
    Ok(None)
}

