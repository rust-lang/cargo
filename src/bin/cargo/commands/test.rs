use crate::command_prelude::*;
use cargo::ops;

pub fn cli() -> Command {
    subcommand("test")
        // Subcommand aliases are handled in `aliased_command()`.
        // .alias("t")
        .about("Execute all unit and integration tests and build examples of a local package")
        .arg(
            Arg::new("TESTNAME")
                .action(ArgAction::Set)
                .help("If specified, only run tests containing this string in their names"),
        )
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .help("Arguments for the test binary")
                .num_args(0..)
                .last(true),
        )
        .arg(flag("doc", "Test only this library's documentation"))
        .arg(flag("no-run", "Compile, but don't run tests"))
        .arg(flag("no-fail-fast", "Run all tests regardless of failure"))
        .arg_ignore_rust_version()
        .arg_future_incompat_report()
        .arg_message_format()
        .arg(
            flag(
                "quiet",
                "Display one character per test instead of one line",
            )
            .short('q'),
        )
        .arg_package_spec(
            "Package to run tests for",
            "Test all packages in the workspace",
            "Exclude packages from the test",
        )
        .arg_targets_all(
            "Test only this package's library unit tests",
            "Test only the specified binary",
            "Test all binaries",
            "Test only the specified example",
            "Test all examples",
            "Test only the specified test target",
            "Test all tests",
            "Test only the specified bench target",
            "Test all benches",
            "Test all targets (does not include doctests)",
        )
        .arg_features()
        .arg_jobs()
        .arg_unsupported_keep_going()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_unit_graph()
        .arg_timings()
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help test</>` for more detailed information.\n\
             Run `<cyan,bold>cargo test -- --help</>` for test binary options.\n",
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Test,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "test", ProfileChecking::Custom)?;

    // `TESTNAME` is actually an argument of the test binary, but it's
    // important, so we explicitly mention it and reconfigure.
    let test_name = args.get_one::<String>("TESTNAME");
    let test_args = args.get_one::<String>("TESTNAME").into_iter();
    let test_args = test_args.chain(args.get_many::<String>("args").unwrap_or_default());
    let test_args = test_args.map(String::as_str).collect::<Vec<_>>();

    let no_run = args.flag("no-run");
    let doc = args.flag("doc");
    if doc {
        if compile_opts.filter.is_specific() {
            return Err(
                anyhow::format_err!("Can't mix --doc with other target selecting options").into(),
            );
        }
        if no_run {
            return Err(anyhow::format_err!("Can't skip running doc tests with --no-run").into());
        }
        compile_opts.build_config.mode = CompileMode::Doctest;
        compile_opts.filter = ops::CompileFilter::lib_only();
    } else if test_name.is_some() && !compile_opts.filter.is_specific() {
        // If arg `TESTNAME` is provided, assumed that the user knows what
        // exactly they wants to test, so we use `all_test_targets` to
        // avoid compiling unnecessary targets such as examples, which are
        // included by the logic of default target filter.
        compile_opts.filter = ops::CompileFilter::all_test_targets();
    }

    let ops = ops::TestOptions {
        no_run,
        no_fail_fast: args.flag("no-fail-fast"),
        compile_opts,
    };

    ops::run_tests(&ws, &ops, &test_args)
}
