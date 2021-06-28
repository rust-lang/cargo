use crate::command_prelude::*;

use cargo::ops;

const PRINT_ARG_NAME: &str = "print";

pub fn cli() -> App {
    subcommand("rustc")
        .setting(AppSettings::TrailingVarArg)
        .about("Compile a package, and pass extra options to the compiler")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("args").multiple(true).help("Rustc flags"))
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
        .arg(
            opt(
                PRINT_ARG_NAME,
                "Output compiler information without compiling",
            )
            .value_name("INFO"),
        )
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .arg_ignore_rust_version()
        .arg_future_incompat_report()
        .after_help("Run `cargo help rustc` for more detailed information.\n")
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
    let rustc = config.load_global_rustc(Some(&ws));
    let mut compile_opts = args.compile_options_for_single_package(
        config,
        rustc,
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
    if let Some(opt_value) = args.value_of(PRINT_ARG_NAME) {
        config
            .cli_unstable()
            .fail_if_stable_opt(PRINT_ARG_NAME, 9357)?;
        ops::print(&ws, &compile_opts, opt_value)?;
    } else {
        ops::compile(&ws, &compile_opts)?;
    }
    Ok(())
}
