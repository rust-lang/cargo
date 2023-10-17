use crate::cli;
use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("version")
        .about("Show version information")
        .arg_quiet()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help version</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let verbose = args.verbose() > 0;
    let version = cli::get_version_string(verbose);
    cargo::drop_print!(config, "{}", version);
    Ok(())
}
