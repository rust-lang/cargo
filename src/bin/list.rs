use std::env;
use cargo::util::{CliResult, Config};
use cargo::list_commands;
pub const USAGE: &'static str = "
List all commands

Usage:
    cargo list
";

#[derive(RustcDecodable)]
pub struct Options {
    flag_verbose: Option<bool>,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
}

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-list; args={:?}",
           env::args().collect::<Vec<_>>());
    // No options are passed but the execute requires options
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));

    println!("Installed Commands:");
    for command in list_commands(config) {
        println!("    {}", command);
    }
    Ok(None)
}
