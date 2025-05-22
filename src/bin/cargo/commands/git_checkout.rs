//! Removed.

use crate::command_prelude::*;

const REMOVED: &str = "The `git-checkout` command has been removed.";

pub fn cli() -> Command {
    subcommand("git-checkout")
        .about("REMOVED: This command has been removed")
        .hide(true)
        .override_help(REMOVED)
}

pub fn exec(_gctx: &mut GlobalContext, _args: &ArgMatches) -> CliResult {
    Err(anyhow::format_err!(REMOVED).into())
}
