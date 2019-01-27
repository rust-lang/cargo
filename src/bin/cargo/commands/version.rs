use crate::command_prelude::*;

use crate::cli;

pub fn cli() -> App {
    subcommand("version").about("Show version information")
}

pub fn exec(_config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let verbose = args.occurrences_of("verbose") > 0;
    let version = cli::get_version_string(verbose);
    print!("{}", version);
    Ok(())
}
