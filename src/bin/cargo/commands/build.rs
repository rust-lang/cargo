use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("build")
        // subcommand aliases are handled in aliased_command()
        // .alias("b")
        .about("Compile a local package and all of its dependencies")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_package_spec(
            "Package to build (see `cargo help pkgid`)",
            "Build all packages in the workspace",
            "Exclude packages from the build",
        )
        .arg_jobs()
        .arg_targets_all(
            "Build only this package's library",
            "Build only the specified binary",
            "Build all binaries",
            "Build only the specified example",
            "Build all examples",
            "Build only the specified test target",
            "Build all tests",
            "Build only the specified bench target",
            "Build all benches",
            "Build all targets",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg(
            opt(
                "out-dir",
                "Copy final artifacts to this directory (unstable)",
            )
            .value_name("PATH"),
        )
        .arg_manifest_path()
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_build_plan()
        .arg_unit_graph()
        .arg_future_incompat_report()
        .after_help("Run `cargo help build` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let rustc = config.load_global_rustc(Some(&ws));
    let mut compile_opts = args.compile_options(
        config,
        rustc,
        CompileMode::Build,
        Some(&ws),
        ProfileChecking::Checked,
    )?;

    if let Some(out_dir) = args.value_of_path("out-dir", config) {
        compile_opts.build_config.export_dir = Some(out_dir);
    } else if let Some(out_dir) = config.build_config()?.out_dir.as_ref() {
        let out_dir = out_dir.resolve_path(config);
        compile_opts.build_config.export_dir = Some(out_dir);
    }
    if compile_opts.build_config.export_dir.is_some() {
        config
            .cli_unstable()
            .fail_if_stable_opt("--out-dir", 6790)?;
    }
    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
