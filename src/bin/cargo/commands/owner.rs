use crate::command_prelude::*;

use cargo::ops::{self, OwnersOptions};
use cargo::util::auth::Secret;

pub fn cli() -> Command {
    subcommand("owner")
        .about("Manage the owners of a crate on the registry")
        .arg(Arg::new("crate").action(ArgAction::Set))
        .arg_required_else_help(true)
        .override_usage(
            "\
       cargo owner [OPTIONS] add    OWNER_NAME <CRATE_NAME>
       cargo owner [OPTIONS] remove OWNER_NAME <CRATE_NAME>
       cargo owner [OPTIONS] list   <CRATE_NAME>",
        )
        .arg_quiet()
        .subcommands([
            Command::new("add")
                .about("Name of a user or team to invite as an owner")
                .override_usage(
                    "\
                   cargo owner [OPTIONS] add [OWNER_NAME] <CRATE_NAME>",
                )
                .arg_quiet()
                .args([
                    Arg::new("ownername")
                        .action(ArgAction::Set)
                        .required(true)
                        .num_args(1)
                        .value_delimiter(',')
                        .value_name("OWNER_NAME"),
                    Arg::new("cratename")
                        .action(ArgAction::Set)
                        .num_args(1)
                        .value_name("CRATE_NAME"),
                ]),
            Command::new("remove")
                .about("Name of a user or team to remove as an owner")
                .override_usage(
                    "\
                   cargo owner [OPTIONS] remove [OWNER_NAME] <CRATE_NAME>",
                )
                .arg_quiet()
                .args([
                    Arg::new("ownername")
                        .action(ArgAction::Set)
                        .required(true)
                        .num_args(1)
                        .value_delimiter(',')
                        .value_name("OWNER_NAME"),
                    Arg::new("cratename")
                        .action(ArgAction::Set)
                        .num_args(1)
                        .value_name("CRATE_NAME"),
                ]),
            Command::new("list")
                .about("List owners of a crate")
                .override_usage(
                    "\
                   cargo owner [OPTIONS] list <CRATE_NAME>",
                )
                .arg_quiet()
                .arg(
                    Arg::new("cratename")
                        .action(ArgAction::Set)
                        .num_args(1)
                        .value_name("CRATE_NAME"),
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
        token: args.get_one::<String>("token").cloned().map(Secret::from),
        index: args.get_one::<String>("index").cloned(),
        subcommand: Some(sc.to_owned()),
        subcommand_arg: Some(arg.clone()),
        registry,
    };

    ops::modify_owners(config, &opts)?;
    Ok(())
}
