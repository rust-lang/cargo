use std::io::prelude::*;
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
    flag_quiet: bool,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Save an api token from the registry locally

Usage:
    cargo login [options] [<token>]

Options:
    -h, --help               Print this message
    --host HOST              Host to set the token for
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let token = match options.arg_token.clone() {
        Some(token) => token,
        None => {
            let src = try!(SourceId::for_central(config));
            let mut src = RegistrySource::new(&src, config);
            try!(src.update());
            let config = try!(src.config());
            let host = options.flag_host.clone().unwrap_or(config.api);
            println!("please visit {}me and paste the API Token below", host);
            let mut line = String::new();
            let input = io::stdin();
            try!(input.lock().read_line(&mut line).chain_error(|| {
                human("failed to read stdin")
            }));
            line
        }
    };

    let token = token.trim().to_string();
    try!(ops::registry_login(config, token));
    Ok(None)
}

