use crate::command_prelude::*;

use cargo::ops::{self, PublishOpts};

pub fn cli() -> Command {
    subcommand("publish")
        .about("Upload a package to the registry")
        .arg_dry_run("Perform all checks without uploading")
        .arg_index("Registry index URL to upload the package to")
        .arg_registry("Registry to upload the package to")
        .arg(opt("token", "Token to use when uploading").value_name("TOKEN"))
        .arg(flag(
            "no-verify",
            "Don't verify the contents by building them",
        ))
        .arg(flag(
            "allow-dirty",
            "Allow dirty working directories to be packaged",
        ))
        .arg_quiet()
        .arg_package("Package to publish")
        .arg_features()
        .arg_parallel()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help publish</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let reg_or_index = args.registry_or_index(config)?;
    let ws = args.workspace(config)?;
    if ws.root_maybe().is_embedded() {
        return Err(anyhow::format_err!(
            "{} is unsupported by `cargo publish`",
            ws.root_manifest().display()
        )
        .into());
    }

    ops::publish(
        &ws,
        &PublishOpts {
            config,
            token: args
                .get_one::<String>("token")
                .map(|s| s.to_string().into()),
            reg_or_index,
            verify: !args.flag("no-verify"),
            allow_dirty: args.flag("allow-dirty"),
            to_publish: args.packages_from_flags()?,
            targets: args.targets()?,
            jobs: args.jobs()?,
            keep_going: args.keep_going(),
            dry_run: args.dry_run(),
            cli_features: args.cli_features()?,
        },
    )?;
    Ok(())
}
