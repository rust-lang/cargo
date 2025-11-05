use crate::command_prelude::*;

use cargo::ops::{self, PublishOpts};
use cargo_credential::Secret;
use clap_complete::ArgValueCandidates;

pub fn cli() -> Command {
    subcommand("publish")
        .about("Upload a package to the registry")
        .arg_dry_run("Perform all checks without uploading")
        .arg_index("Registry index URL to upload the package to")
        .arg_registry("Registry to upload the package to")
        .arg(
            opt("token", "Token to use when uploading")
                .value_name("TOKEN")
                .hide(true),
        )
        .arg(flag(
            "no-verify",
            "Don't verify the contents by building them",
        ))
        .arg(flag(
            "allow-dirty",
            "Allow dirty working directories to be packaged",
        ))
        .arg_silent_suggestion()
        .arg_package_spec_no_all(
            "Package(s) to publish",
            "Publish all packages in the workspace",
            "Don't publish specified packages",
            ArgValueCandidates::new(get_ws_member_candidates),
        )
        .arg_features()
        .arg_parallel()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help publish</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let reg_or_index = args.registry_or_index(gctx)?;
    let ws = args.workspace(gctx)?;
    if ws.root_maybe().is_embedded() {
        return Err(anyhow::format_err!(
            "{} is unsupported by `cargo publish`",
            ws.root_manifest().display()
        )
        .into());
    }

    let token = args.get_one::<String>("token").cloned().map(Secret::from);
    if token.is_some() {
        let _ = gctx.shell().warn("`cargo publish --token` is deprecated in favor of using `cargo login` and environment variables");
    }

    ops::publish(
        &ws,
        &PublishOpts {
            gctx,
            token,
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
