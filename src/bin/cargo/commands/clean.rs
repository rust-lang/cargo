use crate::command_prelude::*;

use cargo::ops::{self, CleanOptions};

pub fn cli() -> App {
    subcommand("clean")
        .about("Remove artifacts that cargo has generated in the past")
        .arg_package_spec_simple("Package to clean artifacts for")
        .arg_manifest_path()
        .arg_target_triple("Target triple to clean output for (default all)")
        .arg_target_dir()
        .arg_release("Whether or not to clean release artifacts")
        .arg_doc("Whether or not to clean just the documentation directory")
        .after_help(
            "\
If the --package argument is given, then SPEC is a package id specification
which indicates which package's artifacts should be cleaned out. If it is not
given, then all packages' artifacts are removed. For more information on SPEC
and its format, see the `cargo help pkgid` command.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let opts = CleanOptions {
        config,
        spec: values(args, "package"),
        target: args.target(),
        release: args.is_present("release"),
        doc: args.is_present("doc"),
    };
    ops::clean(&ws, &opts)?;
    Ok(())
}
