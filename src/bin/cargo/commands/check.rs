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
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_unit_graph()
        .arg_future_incompat_report()
        .arg_only_build_scripts_and_proc_macros()
        .after_help("Run `cargo help check` for more detailed information.\n")
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
