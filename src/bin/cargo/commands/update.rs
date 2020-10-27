use crate::command_prelude::*;

use cargo::ops::{self, UpdateOptions};
use cargo::util::print_available_packages;

pub fn cli() -> App {
    subcommand("update")
        .about("Update dependencies as recorded in the local lock file")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_package_spec_simple("Package to update")
        .arg(opt(
            "aggressive",
            "Force updating all dependencies of <name> as well",
        ))
        .arg_dry_run("Don't actually write the lockfile")
        .arg(opt("precise", "Update a single dependency to exactly PRECISE").value_name("PRECISE"))
        .arg_manifest_path()
        .after_help("Run `cargo help update` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let update_opts = UpdateOptions {
        aggressive: args.is_present("aggressive"),
        precise: args.value_of("precise"),
        to_update: values(args, "package"),
        dry_run: args.is_present("dry-run"),
        config,
    };
    ops::update_lockfile(&ws, &update_opts)?;
    Ok(())
}
