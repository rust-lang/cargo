use crate::command_prelude::*;

use cargo::ops::{self, TrustpubCommand, TrustpubOptions};
use cargo_credential::Secret;

pub fn cli() -> Command {
    subcommand("trustpub")
        .about("Manage Trusted Publishing configuration for a crate on the registry")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            opt("crate", "Crate to operate on")
                .value_name("CRATE")
                .global(true),
        )
        .arg(
            opt("token", "API token to use when authenticating")
                .value_name("TOKEN")
                .global(true),
        )
        .arg_silent_suggestion()
        .subcommand(subcommand("list").about("List the Trusted Publishing configs for a crate"))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let command = match args.subcommand() {
        Some(("list", _)) => TrustpubCommand::List,
        Some((cmd, _)) => unreachable!("unexpected command {}", cmd),
        None => unreachable!("unexpected command"),
    };

    let opts = TrustpubOptions {
        krate: args.get_one::<String>("crate").cloned(),
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        command,
    };
    ops::trusted_publish(gctx, &opts)?;
    Ok(())
}
