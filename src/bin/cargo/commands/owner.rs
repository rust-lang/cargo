use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo::util::auth::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg(Arg::new("crate"))
        .arg_required_else_help(true)
        .args_conflicts_with_subcommands(true)
        .override_usage(
            "\
       cargo owner [OPTIONS] add    <OWNER_NAME> [CRATE_NAME]
       cargo owner [OPTIONS] remove <OWNER_NAME> [CRATE_NAME]
       cargo owner [OPTIONS] list   [CRATE_NAME]",
        )
        .arg_quiet()
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

    let Some((sc, arg)) = args.subcommand() else {
    return Err(CliError::new(
        anyhow::format_err!(
            "you need to specify the subcommands to be operated: add, remove or list."
        ),
        101,
    ));
    };

    let opts = OwnersOptions {
        krate: arg.clone().get_one::<String>("cratename").cloned(),
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        index: args.get_one::<String>("index").cloned(),
        subcommand: Some(sc.to_owned()),
        ownernames: Some(
            arg.get_many::<String>("ownername")
                .map(|s| s.cloned().collect::<Vec<_>>())
                .unwrap(),
        ),
        registry,
    };

    ops::modify_owners(config, &opts)?;
    Ok(())
}
