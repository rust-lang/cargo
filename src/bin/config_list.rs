use std::os;
use std::collections::HashMap;

use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, config};

#[deriving(RustcDecodable)]
struct ConfigListFlags {
    flag_human: bool,
}

#[deriving(RustcEncodable)]
struct ConfigOut {
    values: HashMap<String, config::ConfigValue>
}

pub const USAGE: &'static str = "
Usage:
    cargo config-list --human
    cargo config-list -h | --help

Options:
    -h, --help          Print this message
";

pub fn execute(args: ConfigListFlags,
               _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let cwd = try!(os::getcwd().map_err(|_|
        CliError::new("Couldn't determine the current working directory", 1)));
    let configs = try!(config::all_configs(cwd).map_err(|_|
        CliError::new("Couldn't load configuration", 1)));

    if args.flag_human {
        for (key, value) in configs.iter() {
            println!("{} = {}", key, value);
        }
        Ok(None)
    } else {
        Ok(Some(ConfigOut { values: configs }))
    }
}
