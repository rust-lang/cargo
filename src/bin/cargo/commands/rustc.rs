use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("rustc")
        .setting(AppSettings::TrailingVarArg)
        .about("Compile a package and all of its dependencies")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("args").multiple(true))
        .arg_package("Package to build")
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
        .arg_target_triple("Target triple which compiles will be for")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
The specified target for the current package (or package specified by SPEC if
provided) will be compiled along with all of its dependencies. The specified
<args>... will all be passed to the final compiler invocation, not any of the
dependencies. Note that the compiler will still unconditionally receive
arguments such as -L, --extern, and --crate-type, and the specified <args>...
will simply be added to the compiler invocation.

This command requires that only one target is being compiled. If more than one
target is available for the current package the filters of --lib, --bin, etc,
must be used to select which target is compiled. To pass flags to all compiler
processes spawned by Cargo, use the $RUSTFLAGS environment variable or the
`build.rustflags` configuration option.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let mode = match args.value_of("profile") {
        Some("dev") | None => CompileMode::Build,
        Some("test") => CompileMode::Test,
        Some("bench") => CompileMode::Bench,
        Some("check") => CompileMode::Check { test: false },
        Some(mode) => {
            let err = anyhow::format_err!(
                "unknown profile: `{}`, use dev,
                                   test, or bench",
                mode
            );
            return Err(CliError::new(err, 101));
        }
    };
    let mut compile_opts = args.compile_options_for_single_package(
        config,
        mode,
        Some(&ws),
        ProfileChecking::Unchecked,
    )?;
    let target_args = values(args, "args");
    compile_opts.target_rustc_args = if target_args.is_empty() {
        None
    } else {
        Some(target_args)
    };
    ops::compile(&ws, &compile_opts)?;
    Ok(())
}
