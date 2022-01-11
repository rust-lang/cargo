use crate::command_prelude::*;

use cargo::ops::{self, PublishOpts};

pub fn cli() -> App {
    subcommand("publish")
        .about("Upload a package to the registry")
        .arg_quiet()
        .arg_index()
        .arg(opt("token", "Token to use when uploading").value_name("TOKEN"))
        .arg(opt(
            "no-verify",
            "Don't verify the contents by building them",
        ))
        .arg(opt(
            "allow-dirty",
            "Allow dirty working directories to be packaged",
        ))
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_package("Package to publish")
        .arg_manifest_path()
        .arg_features()
        .arg_jobs()
        .arg_dry_run("Perform all checks without uploading")
        .arg(opt("registry", "Registry to publish to").value_name("REGISTRY"))
        .after_help("Run `cargo help publish` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    config.load_credentials()?;

    let registry = args.registry(config)?;
    let ws = args.workspace(config)?;
    let index = args.index()?;

    ops::publish(
        &ws,
        &PublishOpts {
            config,
            token: args.value_of("token").map(|s| s.to_string()),
            index,
            verify: !args.is_present("no-verify"),
            allow_dirty: args.is_present("allow-dirty"),
            to_publish: args.packages_from_flags()?,
            targets: args.targets(),
            jobs: args.jobs()?,
            dry_run: args.is_present("dry-run"),
            registry,
            cli_features: args.cli_features()?,
        },
    )?;
    Ok(())
}
