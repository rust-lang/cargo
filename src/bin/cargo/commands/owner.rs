use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};

pub fn cli() -> App {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg_quiet()
        .arg(Arg::new("crate"))
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
        .arg(opt("list", "List owners of a crate").short('l'))
        .arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help owner` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    config.load_credentials()?;

    let registry = args.registry(config)?;
    let opts = OwnersOptions {
        krate: args.value_of("crate").map(|s| s.to_string()),
        token: args.value_of("token").map(|s| s.to_string()),
        index: args.value_of("index").map(|s| s.to_string()),
        to_add: args
            .values_of("add")
            .map(|xs| xs.map(|s| s.to_string()).collect()),
        to_remove: args
            .values_of("remove")
            .map(|xs| xs.map(|s| s.to_string()).collect()),
        list: args.is_present("list"),
        registry,
    };
    ops::modify_owners(config, &opts)?;
    Ok(())
}
