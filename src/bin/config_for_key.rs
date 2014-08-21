use std::os;
use std::collections::HashMap;
use docopt;

use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, config};

#[deriving(Encodable)]
struct ConfigOut {
    values: HashMap<String, config::ConfigValue>
}

docopt!(ConfigForKeyFlags, "
Usage: cargo config-for-key --human --key=<key>
")

pub fn execute(args: ConfigForKeyFlags,
               _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let value = try!(config::get_config(os::getcwd(),
                                        args.flag_key.as_slice()).map_err(|_| {
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
