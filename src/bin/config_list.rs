use std::os;
use std::collections::HashMap;
use docopt;

use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, config};

#[deriving(Encodable)]
struct ConfigOut {
    values: HashMap<String, config::ConfigValue>
}

docopt!(ConfigListFlags, "
Usage: cargo config-list --human
")

pub fn execute(args: ConfigListFlags,
               _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let configs = try!(config::all_configs(os::getcwd()).map_err(|_|
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
