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
        .arg_message_format()
        .arg_build_plan()
        .after_help(
            "\
All packages in the workspace are built if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let mut compile_opts = args.compile_options(config, CompileMode::Build, Some(&ws))?;

    compile_opts.export_dir = args.value_of_path("out-dir", config);
    if compile_opts.export_dir.is_some() {
        config
            .cli_unstable()
            .fail_if_stable_opt("--out-dir", 6790)?;
    }
    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
