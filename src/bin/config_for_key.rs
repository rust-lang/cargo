use std::os;
use std::collections::HashMap;

use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, config};

#[deriving(RustcDecodable)]
struct ConfigForKeyFlags {
    flag_human: bool,
    flag_key: String,
}

#[deriving(RustcEncodable)]
struct ConfigOut {
    values: HashMap<String, config::ConfigValue>
}

pub const USAGE: &'static str = "
Usage:
    cargo config-for-key --human --key=<key>
    cargo config-for-key -h | --help

Options:
    -h, --help          Print this message
";

pub fn execute(args: ConfigForKeyFlags,
               _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let cwd = try!(os::getcwd().map_err(|_|
        CliError::new("Couldn't determine the current working directory", 1)));
    let value = try!(config::get_config(cwd, args.flag_key.as_slice()).map_err(|_| {
        CliError::new("Couldn't load configuration",  1)
    }));

    if args.flag_human {
        println!("{}", value);
        Ok(None)
    } else {
        let mut map = HashMap::new();
        map.insert(args.flag_key.clone(), value);
        Ok(Some(ConfigOut { values: map }))
    }
}
