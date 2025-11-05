use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("build")
        // subcommand aliases are handled in aliased_command()
        // .alias("b")
        .about("Compile a local package and all of its dependencies")
        .arg_future_incompat_report()
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_package_spec(
            "Package to build (see `cargo help pkgid`)",
            "Build all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg_targets_all(
            "Build only this package's library",
            "Build only the specified binary",
            "Build all binaries",
            "Build only the specified example",
            "Build all examples",
            "Build only the specified test target",
            "Build all targets that have `test = true` set",
            "Build only the specified bench target",
            "Build all targets that have `bench = true` set",
            "Build all targets",
        )
        .arg_features()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_redundant_default_mode("debug", "build", "release")
        .arg_profile("Build artifacts with the specified profile")
        .arg_parallel()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_artifact_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_compile_time_deps()
        .arg_manifest_path()
        .arg_lockfile_path()
        .arg_ignore_rust_version()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help build</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;
    let mut compile_opts =
        args.compile_options(gctx, UserIntent::Build, Some(&ws), ProfileChecking::Custom)?;

    if let Some(artifact_dir) = args.value_of_path("artifact-dir", gctx) {
        // If the user specifies `--artifact-dir`, use that
        compile_opts.build_config.export_dir = Some(artifact_dir);
    } else if let Some(artifact_dir) = args.value_of_path("out-dir", gctx) {
        // `--out-dir` is deprecated, but still supported for now
        gctx.shell()
            .warn("the --out-dir flag has been changed to --artifact-dir")?;
        compile_opts.build_config.export_dir = Some(artifact_dir);
    } else if let Some(artifact_dir) = gctx.build_config()?.artifact_dir.as_ref() {
        // If a CLI option is not specified for choosing the artifact dir, use the `artifact-dir` from the build config, if
        // present
        let artifact_dir = artifact_dir.resolve_path(gctx);
        compile_opts.build_config.export_dir = Some(artifact_dir);
    } else if let Some(artifact_dir) = gctx.build_config()?.out_dir.as_ref() {
        // As a last priority, check `out-dir` in the build config
        gctx.shell()
            .warn("the out-dir config option has been changed to artifact-dir")?;
        let artifact_dir = artifact_dir.resolve_path(gctx);
        compile_opts.build_config.export_dir = Some(artifact_dir);
    }

    if compile_opts.build_config.export_dir.is_some() {
        gctx.cli_unstable()
            .fail_if_stable_opt("--artifact-dir", 6790)?;
    }

    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
