use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("check")
        // subcommand aliases are handled in aliased_command()
        // .alias("c")
        .about("Check a local package and all of its dependencies for errors")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_package_spec(
            "Package(s) to check",
            "Check all packages in the workspace",
            "Exclude packages from the check",
        )
        .arg_jobs()
        .arg_targets_all(
            "Check only this package's library",
            "Check only the specified binary",
            "Check all binaries",
            "Check only the specified example",
            "Check all examples",
            "Check only the specified test target",
            "Check all tests",
            "Check only the specified bench target",
            "Check all benches",
            "Check all targets",
        )
        .arg_release("Check artifacts in release mode, with optimizations")
        .arg_profile("Check artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Check for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are checked if the `--workspace` flag is supplied. The
`--workspace` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--workspace` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the `--release` flag will use the `release` profile instead.

The `--profile test` flag can be used to check unit tests with the
`#[cfg(test)]` attribute.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let test = match args.value_of("profile") {
        Some("test") => true,
        None => false,
        Some(profile) => {
            let err = anyhow::format_err!(
                "unknown profile: `{}`, only `test` is \
                 currently supported",
                profile
            );
            return Err(CliError::new(err, 101));
        }
    };
    let mode = CompileMode::Check { test };
    let compile_opts = args.compile_options(config, mode, Some(&ws), ProfileChecking::Unchecked)?;

    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
