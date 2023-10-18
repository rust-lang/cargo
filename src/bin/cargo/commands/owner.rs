use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo_credential::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg(Arg::new("crate").value_name("CRATE").action(ArgAction::Set))
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
        .arg_index("Registry index URL to modify owners for")
        .arg_registry("Registry to modify owners for")
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg_quiet()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help owner</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let opts = OwnersOptions {
        krate: args.get_one::<String>("crate").cloned(),
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        reg_or_index: args.registry_or_index(config)?,
        to_add: args
            .get_many::<String>("add")
            .map(|xs| xs.cloned().collect()),
        to_remove: args
            .get_many::<String>("remove")
            .map(|xs| xs.cloned().collect()),
        list: args.flag("list"),
    };
    ops::modify_owners(config, &opts)?;
    Ok(())
}
