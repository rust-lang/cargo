use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions, SubCommand};
use cargo::util::auth::Secret;

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
        // The following three parameters are planned to be replaced in the form of subcommands.
        // refer to issue: https://github.com/rust-lang/cargo/issues/4352
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
            add_arg(
                Command::new("add")
                    .about("Name of a user or team to invite as an owner")
                    .arg_quiet()
                    .args([
                        Arg::new("addowner")
                            .required(true)
                            .value_delimiter(',')
                            .value_name("OWNER_NAME")
                            .help("Name of the owner you want to invite"),
                        Arg::new("crate")
                            .value_name("CRATE_NAME")
                            .help("Crate name that you want to manage the owner"),
                    ]),
            ),
            add_arg(
                Command::new("remove")
                    .about("Name of a user or team to remove as an owner")
                    .arg_quiet()
                    .args([
                        Arg::new("removeowner")
                            .required(true)
                            .value_delimiter(',')
                            .value_name("OWNER_NAME")
                            .help("Name of the owner you want to remove"),
                        Arg::new("crate")
                            .value_name("CRATE_NAME")
                            .help("Crate name that you want to manage the owner"),
                    ]),
            ),
            add_arg(
                Command::new("list")
                    .about("List owners of a crate")
                    .arg_quiet()
                    .arg(
                        Arg::new("crate")
                            .value_name("CRATE_NAME")
                            .help("Crate name which you want to list all owner names"),
                    ),
            ),
        ])
        .arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help owner` for more detailed information.\n")
}

fn add_arg(com: Command) -> Command {
    com.arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let command = args.subcommand();

    let (sc, krate, token, index, registry) = if command.is_some() {
        let (sc, arg) = command.unwrap();
        let subcommand = match sc {
            "list" => SubCommand::List,
            "add" => SubCommand::Add,
            "remove" => SubCommand::Remove,
            _ => SubCommand::None,
        };
        (
            subcommand,
            arg.clone().get_one::<String>("crate").cloned(),
            arg.get_one::<String>("token").cloned().map(Secret::from),
            arg.get_one::<String>("index").cloned(),
            arg.registry(config)?,
        )
    } else {
        (
            SubCommand::None,
            args.clone().get_one::<String>("crate").cloned(),
            args.get_one::<String>("token").cloned().map(Secret::from),
            args.get_one::<String>("index").cloned(),
            args.registry(config)?,
        )
    };

    let addons = args.subcommand_matches("add").and_then(|x| {
        x.get_many::<String>("addowner")
            .map(|xs| xs.cloned().collect::<Vec<String>>())
    });

    let removeons = args.subcommand_matches("remove").and_then(|x| {
        x.get_many::<String>("removeowner")
            .map(|xs| xs.cloned().collect::<Vec<String>>())
    });

    let opts = OwnersOptions {
        krate: krate,
        token: token,
        index: index,
        to_add: addons.or(args
            .get_many::<String>("add")
            .map(|xs| xs.cloned().collect())),
        to_remove: removeons.or(args
            .get_many::<String>("remove")
            .map(|xs| xs.cloned().collect())),
        list: args.flag("list"),
        subcommand: Some(sc),
        registry,
    };

    ops::modify_owners(config, &opts)?;
    Ok(())
}
