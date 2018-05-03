use command_prelude::*;

use cargo;

pub fn cli() -> App {
    subcommand("version").about("Show version information")
}

pub fn exec(_config: &mut Config, _args: &ArgMatches) -> CliResult {
    println!("{}", cargo::version());
    Ok(())
}
