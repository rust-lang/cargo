use command_prelude::*;

use cargo::ops::{self, PublishOpts};

pub fn cli() -> App {
    subcommand("publish")
        .about("Upload a package to the registry")
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
        .arg_manifest_path()
        .arg_jobs()
        .arg(opt("dry-run", "Perform all checks without uploading"))
        .arg(opt("registry", "Registry to publish to").value_name("REGISTRY"))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    let ws = args.workspace(config)?;
    let index = args.index(config)?;

    ops::publish(
        &ws,
        &PublishOpts {
            config,
            token: args.value_of("token").map(|s| s.to_string()),
            index,
            verify: !args.is_present("no-verify"),
            allow_dirty: args.is_present("allow-dirty"),
            target: args.target(),
            jobs: args.jobs()?,
            dry_run: args.is_present("dry-run"),
            registry,
        },
    )?;
    Ok(())
}
