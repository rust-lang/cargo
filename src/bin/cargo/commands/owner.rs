use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo_credential::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg_quiet()
        .arg(Arg::new("crate").hide(true))
        .arg_required_else_help(true)
        .args_conflicts_with_subcommands(true)
        .override_usage(
            "\
       cargo owner add    <OWNER_NAME> [CRATE_NAME] [OPTIONS]
       cargo owner remove <OWNER_NAME> [CRATE_NAME] [OPTIONS]
       cargo owner list   [CRATE_NAME] [OPTIONS]",
        )
        .arg(
            multi_opt(
                "add",
                "LOGIN",
                "Name of a user or team to invite as an owner",
            )
            .short('a')
            .hide(true),
        )
        .arg(
            multi_opt(
                "remove",
                "LOGIN",
                "Name of a user or team to remove as an owner",
            )
            .short('r')
            .hide(true),
        )
        .arg(flag("list", "List owners of a crate").short('l').hide(true))
        .subcommands([
            add_registry_args(
                Command::new("add")
                    .about("Name of a user or team to invite as an owner")
                    .args([
                        Arg::new("add")
                            .required(true)
                            .value_delimiter(',')
                            .value_name("OWNER_NAME")
                            .help("Name of the owner you want to invite"),
                        Arg::new("crate")
                            .value_name("CRATE_NAME")
                            .help("Crate name that you want to manage the owner"),
                    ]),
            )
            .override_usage("cargo owner add <OWNER_NAME> [CRATE_NAME] [OPTIONS]"),
            add_registry_args(
                Command::new("remove")
                    .about("Name of a user or team to remove as an owner")
                    .args([
                        Arg::new("remove")
                            .required(true)
                            .value_delimiter(',')
                            .value_name("OWNER_NAME")
                            .help("Name of the owner you want to remove"),
                        Arg::new("crate")
                            .value_name("CRATE_NAME")
                            .help("Crate name that you want to manage the owner"),
                    ]),
            )
            .override_usage("cargo owner remove <OWNER_NAME> [CRATE_NAME] [OPTIONS]"),
            add_registry_args(
                Command::new("list").about("List owners of a crate").arg(
                    Arg::new("crate")
                        .value_name("CRATE_NAME")
                        .help("Crate name which you want to list all owner names"),
                ),
            )
            .override_usage("cargo owner list [CRATE_NAME] [OPTIONS]"),
        ])
        .arg_index("Registry index URL to modify owners for")
        .arg_registry("Registry to modify owners for")
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help owner</>` for more detailed information.\n"
        ))
}

fn add_registry_args(command: Command) -> Command {
    command
        .arg_index("Registry index URL to modify owners for")
        .arg_registry("Registry to modify owners for")
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let (to_add, to_remove, list) = match args.subcommand() {
        Some(("add", subargs)) => (
            subargs
                .get_many::<String>("add")
                .map(|xs| xs.cloned().collect::<Vec<String>>()),
            None,
            false,
        ),
        Some(("remove", subargs)) => (
            None,
            subargs
                .get_many::<String>("remove")
                .map(|xs| xs.cloned().collect()),
            false,
        ),
        Some(("list", _)) => (None, None, true),
        Some((name, _)) => {
            unreachable!("{name} is not a subcommand of cargo owner, please enter `cargo owner --help` for help.")
        }
        None => (
            args.get_many::<String>("add")
                .map(|xs| xs.cloned().collect::<Vec<String>>()),
            args.get_many::<String>("remove")
                .map(|xs| xs.cloned().collect()),
            args.flag("list"),
        ),
    };

    let common_args = args.subcommand().map(|(_, args)| args).unwrap_or(args);

    let opts = OwnersOptions {
        krate: common_args.clone().get_one::<String>("crate").cloned(),
        token: common_args
            .get_one::<String>("token")
            .cloned()
            .map(Secret::from),
        reg_or_index: args.registry_or_index(config)?,
        to_add: to_add,
        to_remove: to_remove,
        list: list,
    };

    ops::modify_owners(config, &opts)?;
    Ok(())
}
