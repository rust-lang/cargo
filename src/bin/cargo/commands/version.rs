use crate::cli;
use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("version")
        .about("Show version information")
        .arg_silent_suggestion()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help version</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let verbose = args.verbose() > 0;
    let version = cli::get_version_string(verbose);
    cargo::drop_print!(gctx, "{}", version);
    Ok(())
}
