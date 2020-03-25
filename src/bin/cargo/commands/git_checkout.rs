use crate::command_prelude::*;

const REMOVED: &str = "The `git-checkout` subcommand has been removed.";

pub fn cli() -> App {
    subcommand("git-checkout")
        .about("This subcommand has been removed")
        .settings(&[AppSettings::Hidden])
        .help(REMOVED)
}

pub fn exec(_config: &mut Config, _args: &ArgMatches<'_>) -> CliResult {
    Err(anyhow::format_err!(REMOVED).into())
}
