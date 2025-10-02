use crate::command_prelude::*;
use std::ops::Not;

use cargo::ops::{self, PublishOpts};
use cargo_credential::Secret;

pub fn cli() -> Command {
    subcommand("publish")
        .about("Upload a package to the registry")
        .arg_dry_run("Perform all checks without uploading")
        .arg_index("Registry index URL to upload the package to")
        .arg_registry("Registry to upload the package to")
        .arg(opt("token", "Token to use when uploading").value_name("TOKEN"))
        .arg(flag("token-stdin", "Read token from stdin (unstable)").conflicts_with("token"))
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

    let token_from_cmd = args.get_one::<String>("token");
    let should_read_token_stdin = args.flag("token-stdin");
    if should_read_token_stdin {
        gctx.cli_unstable().fail_if_stable_opt("--token-stdin", 0)?;
    }
    let token = token_from_cmd
        .cloned()
        .or_else(|| {
            if should_read_token_stdin
                && let token_from_stdin = cargo_credential::read_line().unwrap_or_default()
                && token_from_stdin.is_empty().not()
            {
                Some(token_from_stdin)
            } else {
                None
            }
        })
        .map(Secret::from);

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
