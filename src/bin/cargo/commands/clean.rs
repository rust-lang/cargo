use crate::command_prelude::*;

use cargo::ops::{self, CleanOptions};
use cargo::util::print_available_packages;

pub fn cli() -> Command {
    subcommand("clean")
        .about("Remove artifacts that cargo has generated in the past")
        .arg_doc("Whether or not to clean just the documentation directory")
        .arg_quiet()
        .arg_package_spec_simple("Package to clean artifacts for")
        .arg_release("Whether or not to clean release artifacts")
        .arg_profile("Clean artifacts of the specified profile")
        .arg_target_triple("Target triple to clean output for")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_dry_run("Display what would be deleted without deleting anything")
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help clean</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let opts = CleanOptions {
        config,
        spec: values(args, "package"),
        targets: args.targets()?,
        requested_profile: args.get_profile_name(config, "dev", ProfileChecking::Custom)?,
        profile_specified: args.contains_id("profile") || args.flag("release"),
        doc: args.flag("doc"),
        dry_run: args.dry_run(),
    };
    ops::clean(&ws, &opts)?;
    Ok(())
}
