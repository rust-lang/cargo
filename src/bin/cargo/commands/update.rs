use crate::command_prelude::*;

use anyhow::anyhow;
use cargo::ops::{self, UpdateOptions};
use cargo::util::print_available_packages;

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
        .arg_quiet()
        .arg(
            flag("workspace", "Only update the workspace packages")
                .short('w')
                .help_heading(heading::PACKAGE_SELECTION),
        )
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help update</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

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
        to_update,
        dry_run: args.dry_run(),
        workspace: args.flag("workspace"),
        config,
    };
    ops::update_lockfile(&ws, &update_opts)?;
    Ok(())
}
