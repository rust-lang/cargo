use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("check")
        // subcommand aliases are handled in aliased_command()
        // .alias("c")
        .about("Check a local package and all of its dependencies for errors")
        .arg_future_incompat_report()
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_package_spec(
            "Package(s) to check",
            "Check all packages in the workspace",
            "Exclude packages from the check",
        )
        .arg_targets_all(
            "Check only this package's library",
            "Check only the specified binary",
            "Check all binaries",
            "Check only the specified example",
            "Check all examples",
            "Check only the specified test target",
            "Check all targets that have `test = true` set",
            "Check only the specified bench target",
            "Check all targets that have `bench = true` set",
            "Check all targets",
        )
        .arg_features()
        .arg_parallel()
        .arg_release("Check artifacts in release mode, with optimizations")
        .arg_profile("Check artifacts with the specified profile")
        .arg_target_triple("Check for the target triple")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_compile_time_deps()
        .arg_manifest_path()
        .arg_lockfile_path()
        .arg_ignore_rust_version()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help check</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;
    // This is a legacy behavior that causes `cargo check` to pass `--test`.
    let test = matches!(
        args.get_one::<String>("profile").map(String::as_str),
        Some("test")
    );
    let intent = UserIntent::Check { test };
    let compile_opts =
        args.compile_options(gctx, intent, Some(&ws), ProfileChecking::LegacyTestOnly)?;

    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
