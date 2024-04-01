use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("run-custom-build")
        .about("Run build script build.rs")
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_package_spec(
            "Package to build (see `cargo help pkgid`)",
            "Build all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg_features()
        .arg_redundant_default_mode("debug", "build", "release")
        .arg_parallel()
        .arg_target_dir()
        .arg_build_plan()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help run-custom-build</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;
    let mut compile_opts = args.compile_options(
        gctx,
        CompileMode::RunCustomBuild,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    if let Some(out_dir) = args.value_of_path("out-dir", gctx) {
        compile_opts.build_config.export_dir = Some(out_dir);
    } else if let Some(out_dir) = gctx.build_config()?.out_dir.as_ref() {
        let out_dir = out_dir.resolve_path(gctx);
        compile_opts.build_config.export_dir = Some(out_dir);
    }
    if compile_opts.build_config.export_dir.is_some() {
        gctx.cli_unstable().fail_if_stable_opt("--out-dir", 6790)?;
    }
    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
