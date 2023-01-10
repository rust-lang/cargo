use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo::util::auth::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg_quiet()
        .arg(Arg::new("crate").action(ArgAction::Set))
        .arg(
            multi_opt(
                "add",
                "LOGIN",
                "Name of a user or team to invite as an owner",
            )
            .short('a'),
        )
        .arg(
            multi_opt(
                "remove",
                "LOGIN",
                "Name of a user or team to remove as an owner",
            )
            .short('r'),
        )
        .arg(flag("list", "List owners of a crate").short('l'))
        .arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help owner` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    let opts = OwnersOptions {
        krate: args.get_one::<String>("crate").cloned(),
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        index: args.get_one::<String>("index").cloned(),
        to_add: args
            .get_many::<String>("add")
            .map(|xs| xs.cloned().collect()),
        to_remove: args
            .get_many::<String>("remove")
            .map(|xs| xs.cloned().collect()),
        list: args.flag("list"),
        registry,
    };
    ops::modify_owners(config, &opts)?;
    Ok(())
}
