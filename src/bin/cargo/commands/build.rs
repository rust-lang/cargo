use command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("build")
        .alias("b")
        .about("Compile a local package and all of its dependencies")
        .arg_package_spec(
            "Package to build",
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
            "Build all targets (lib and bin targets by default)",
        )
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg(opt("out-dir", "Copy final artifacts to this directory").value_name("PATH"))
        .arg_manifest_path()
        .arg_message_format()
        .arg_build_plan()
        .after_help(
            "\
If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are built if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

Compilation can be configured via the use of profiles which are configured in
the manifest. The default profile for this command is `dev`, but passing
the --release flag will use the `release` profile instead.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    let mut compile_opts = args.compile_options(config, CompileMode::Build)?;
    compile_opts.export_dir = args.value_of_path("out-dir", config);
    if compile_opts.export_dir.is_some() && !config.cli_unstable().unstable_options {
        Err(format_err!(
            "`--out-dir` flag is unstable, pass `-Z unstable-options` to enable it"
        ))?;
    };
    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
