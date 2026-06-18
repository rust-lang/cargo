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
        .subcommand(
            subcommand("add")
                .about("Add a GitHub Actions Trusted Publishing config to a crate")
                .arg(
                    opt("owner", "GitHub repository owner (user or organization)")
                        .value_name("OWNER")
                        .required(true),
                )
                .arg(
                    opt("repo", "GitHub repository name")
                        .value_name("REPO")
                        .required(true),
                )
                .arg(
                    opt("pipeline", "GitHub Actions workflow filename (e.g. `ci.yml`)")
                        .value_name("PIPELINE")
                        .required(true),
                )
                .arg(
                    opt("env", "GitHub Actions environment the workflow must run in")
                        .value_name("ENV"),
                ),
        )
        .subcommand(
            subcommand("remove")
                .about("Remove a Trusted Publishing config from a crate")
                .arg(
                    opt("id", "Id of the config to remove (see `cargo trustpub list`)")
                        .value_name("ID")
                        .value_parser(value_parser!(u32))
                        .required(true),
                ),
        )
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let command = match args.subcommand() {
        Some(("list", _)) => TrustpubCommand::List,
        Some(("add", sub)) => TrustpubCommand::Add {
            repository_owner: sub.get_one::<String>("owner").cloned().unwrap(),
            repository_name: sub.get_one::<String>("repo").cloned().unwrap(),
            workflow_filename: sub.get_one::<String>("pipeline").cloned().unwrap(),
            environment: sub.get_one::<String>("env").cloned(),
        },
        Some(("remove", sub)) => TrustpubCommand::Remove {
            id: *sub.get_one::<u32>("id").unwrap(),
        },
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
