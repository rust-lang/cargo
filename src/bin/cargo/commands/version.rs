use crate::cli;
use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("version")
        .about("Show version information")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let verbose = args.occurrences_of("verbose") > 0;
    let version = cli::get_version_string(verbose);
    cargo::drop_print!(config, "{}", version);
    Ok(())
}
