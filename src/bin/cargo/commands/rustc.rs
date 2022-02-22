use crate::command_prelude::*;
use cargo::ops;
use cargo::util::interning::InternedString;

const PRINT_ARG_NAME: &str = "print";
const CRATE_TYPE_ARG_NAME: &str = "crate-type";

pub fn cli() -> App {
    subcommand("rustc")
        .trailing_var_arg(true)
        .about("Compile a package, and pass extra options to the compiler")
        .arg_quiet()
        .arg(Arg::new("args").multiple_values(true).help("Rustc flags"))
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
        .arg(multi_opt(
            CRATE_TYPE_ARG_NAME,
            "CRATE-TYPE",
            "Comma separated list of types of crates for the compiler to emit (unstable)",
        ))
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .arg_ignore_rust_version()
        .arg_future_incompat_report()
        .arg_timings()
        .after_help("Run `cargo help rustc` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    // This is a legacy behavior that changes the behavior based on the profile.
    // If we want to support this more formally, I think adding a --mode flag
    // would be warranted.
    let mode = match args.value_of("profile") {
        Some("test") => CompileMode::Test,
        Some("bench") => CompileMode::Bench,
        Some("check") => CompileMode::Check { test: false },
        _ => CompileMode::Build,
    };
    let mut compile_opts = args.compile_options_for_single_package(
        config,
        mode,
        Some(&ws),
        ProfileChecking::LegacyRustc,
    )?;
    if compile_opts.build_config.requested_profile == "check" {
        compile_opts.build_config.requested_profile = InternedString::new("dev");
    }
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
        return Ok(());
    }
    let crate_types = values(args, CRATE_TYPE_ARG_NAME);
    compile_opts.target_rustc_crate_types = if crate_types.is_empty() {
        None
    } else {
        config
            .cli_unstable()
            .fail_if_stable_opt(CRATE_TYPE_ARG_NAME, 10083)?;
        Some(crate_types)
    };
    ops::compile(&ws, &compile_opts)?;

    Ok(())
}
