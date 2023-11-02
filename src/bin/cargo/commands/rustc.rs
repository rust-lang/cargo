use crate::command_prelude::*;
use cargo::ops;
use cargo::util::interning::InternedString;

const PRINT_ARG_NAME: &str = "print";
const CRATE_TYPE_ARG_NAME: &str = "crate-type";

pub fn cli() -> Command {
    subcommand("rustc")
        .about("Compile a package, and pass extra options to the compiler")
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .num_args(0..)
                .help("Extra rustc flags")
                .trailing_var_arg(true),
        )
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
            "Comma separated list of types of crates for the compiler to emit",
        ))
        .arg_future_incompat_report()
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_quiet()
        .arg_package("Package to build")
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
        .arg_features()
        .arg_parallel()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Target triple which compiles will be for")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help rustc</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    // This is a legacy behavior that changes the behavior based on the profile.
    // If we want to support this more formally, I think adding a --mode flag
    // would be warranted.
    let mode = match args.get_one::<String>("profile").map(String::as_str) {
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
    if let Some(opt_value) = args.get_one::<String>(PRINT_ARG_NAME) {
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
        Some(crate_types)
    };
    ops::compile(&ws, &compile_opts)?;

    Ok(())
}
