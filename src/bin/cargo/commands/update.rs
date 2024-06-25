use std::collections::HashMap;

use crate::command_prelude::*;

use anyhow::anyhow;
use cargo::ops::{self, UpdateOptions};
use cargo::util::print_available_packages;
use tracing::trace;

pub fn cli() -> Command {
    subcommand("update")
        .about("Update dependencies as recorded in the local lock file")
        .args([clap::Arg::new("package2")
            .action(clap::ArgAction::Append)
            .num_args(1..)
            .value_name("SPEC")
            .help_heading(heading::PACKAGE_SELECTION)
            .group("package-group")
            .help("Package to update")])
        .arg(
            optional_multi_opt("package", "SPEC", "Package to update")
                .short('p')
                .hide(true)
                .help_heading(heading::PACKAGE_SELECTION)
                .group("package-group"),
        )
        .arg_dry_run("Don't actually write the lockfile")
        .arg(
            flag(
                "recursive",
                "Force updating all dependencies of [SPEC]... as well",
            )
            .alias("aggressive")
            .conflicts_with("precise"),
        )
        .arg(
            opt("precise", "Update [SPEC] to exactly PRECISE")
                .value_name("PRECISE")
                .requires("package-group"),
        )
        .arg(
            flag(
                "breaking",
                "Update [SPEC] to latest SemVer-breaking version (unstable)",
            )
            .short('b'),
        )
        .arg_silent_suggestion()
        .arg(
            flag("workspace", "Only update the workspace packages")
                .short('w')
                .help_heading(heading::PACKAGE_SELECTION),
        )
        .arg_manifest_path()
        .arg_ignore_rust_version_with_help(
            "Ignore `rust-version` specification in packages (unstable)",
        )
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help update</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    if args.honor_rust_version().is_some() {
        gctx.cli_unstable().fail_if_stable_opt_custom_z(
            "--ignore-rust-version",
            9930,
            "msrv-policy",
            gctx.cli_unstable().msrv_policy,
        )?;
    }

    let mut ws = args.workspace(gctx)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let to_update = if args.contains_id("package") {
        "package"
    } else {
        "package2"
    };
    let to_update = values(args, to_update);
    for crate_name in to_update.iter() {
        if let Some(toolchain) = crate_name.strip_prefix("+") {
            return Err(anyhow!(
                "invalid character `+` in package name: `+{toolchain}`
    Use `cargo +{toolchain} update` if you meant to use the `{toolchain}` toolchain."
            )
            .into());
        }
    }

    let update_opts = UpdateOptions {
        recursive: args.flag("recursive"),
        precise: args.get_one::<String>("precise").map(String::as_str),
        breaking: args.flag("breaking"),
        to_update,
        dry_run: args.dry_run(),
        workspace: args.flag("workspace"),
        gctx,
    };

    let breaking_update = update_opts.breaking
        || (update_opts.precise.is_some() && gctx.cli_unstable().unstable_options);

    // We are using the term "upgrade" here, which is the typical case, but it
    // can also be a downgrade (in the case of a precise update). In general, it
    // is a change to a version req matching a precise version.
    let upgrades = if breaking_update {
        if update_opts.breaking {
            gctx.cli_unstable()
                .fail_if_stable_opt("--breaking", 12425)?;
        }

        trace!("allowing breaking updates");
        ops::upgrade_manifests(&mut ws, &update_opts.to_update, &update_opts.precise)?
    } else {
        HashMap::new()
    };

    ops::update_lockfile(&ws, &update_opts, &upgrades)?;
    ops::write_manifest_upgrades(&ws, &upgrades, update_opts.dry_run)?;

    if update_opts.dry_run {
        update_opts
            .gctx
            .shell()
            .warn("aborting update due to dry run")?;
    }

    Ok(())
}
