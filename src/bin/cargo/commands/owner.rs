use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo::util::auth::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg_quiet()
        .arg(Arg::new("crate"))
        .arg_required_else_help(true)
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
        .override_usage(
            "\
       cargo owner [OPTIONS] add    <OWNER_NAME> [CRATE_NAME]
       cargo owner [OPTIONS] remove <OWNER_NAME> [CRATE_NAME]
       cargo owner [OPTIONS] list   [CRATE_NAME]",
        )
        .subcommands([
            Command::new("add")
                .about("Name of a user or team to invite as an owner")
                .arg_quiet()
                .args([
                    Arg::new("ownername")
                        .required(true)
                        .value_delimiter(',')
                        .value_name("OWNER_NAME")
                        .help("Name of the owner you want to invite"),
                    Arg::new("cratename")
                        .value_name("CRATE_NAME")
                        .help("Crate name that you want to manage the owner"),
                ]),
            Command::new("remove")
                .about("Name of a user or team to remove as an owner")
                .arg_quiet()
                .args([
                    Arg::new("ownername")
                        .required(true)
                        .value_delimiter(',')
                        .value_name("OWNER_NAME")
                        .help("Name of the owner you want to remove"),
                    Arg::new("cratename")
                        .value_name("CRATE_NAME")
                        .help("Crate name that you want to manage the owner"),
                ]),
            Command::new("list")
                .about("List owners of a crate")
                .arg_quiet()
                .arg(
                    Arg::new("cratename")
                        .value_name("CRATE_NAME")
                        .help("Crate name which you want to list all owner names"),
                ),
        ])
        .arg(opt("index", "Registry index to modify owners for").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help owner` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;

    let (sc, krate, ownernames) = if let Some((sc, arg)) = args.subcommand() {
        let ownernames = if sc == "list" {
            Vec::<String>::new()
        } else {
            arg.get_many::<String>("ownername")
                .map(|s| s.cloned().collect::<Vec<_>>())
                .unwrap()
        };
        (
            sc,
            arg.clone().get_one::<String>("cratename").cloned(),
            ownernames,
        )
    } else {
        (
            "",
            args.get_one::<String>("crate").cloned(),
            Vec::<String>::new(),
        )
    };

    let opts = OwnersOptions {
        krate: krate,
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        index: args.get_one::<String>("index").cloned(),
        to_add: args
            .get_many::<String>("add")
            .map(|xs| xs.cloned().collect()),
        to_remove: args
            .get_many::<String>("remove")
            .map(|xs| xs.cloned().collect()),
        list: args.flag("list"),
        subcommand: Some(sc.to_owned()),
        ownernames: Some(ownernames),
        registry,
    };

    ops::modify_owners(config, &opts)?;
    Ok(())
}
