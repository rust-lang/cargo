use crate::command_prelude::*;
use anyhow::Error;
use cargo::ops;

pub fn cli() -> App {
    subcommand("test")
        // Subcommand aliases are handled in `aliased_command()`.
        // .alias("t")
        .trailing_var_arg(true)
        .about("Execute all unit and integration tests and build examples of a local package")
        .arg(
            Arg::new("TESTNAME")
                .help("If specified, only run tests containing this string in their names"),
        )
        .arg(
            Arg::new("args")
                .help("Arguments for the test binary")
                .multiple_values(true)
                .last(true),
        )
        .arg(
            opt(
                "quiet",
                "Display one character per test instead of one line",
            )
            .short('q'),
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
            "Test all targets",
        )
        .arg(opt("doc", "Test only this library's documentation"))
        .arg(opt("no-run", "Compile, but don't run tests"))
        .arg(opt("no-fail-fast", "Run all tests regardless of failure"))
        .arg_package_spec(
            "Package to run tests for",
            "Test all packages in the workspace",
            "Exclude packages from the test",
        )
        .arg_jobs()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_unit_graph()
        .arg_future_incompat_report()
        .arg_timings()
        .after_help(
            "Run `cargo help test` for more detailed information.\n\
             Run `cargo test -- --help` for test binary options.\n",
        )
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
    let test_name = args.value_of("TESTNAME");
    let test_args = args.value_of("TESTNAME").into_iter();
    let test_args = test_args.chain(args.values_of("args").unwrap_or_default());
    let test_args = test_args.collect::<Vec<_>>();

    let no_run = args.is_present("no-run");
    let doc = args.is_present("doc");
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
        no_fail_fast: args.is_present("no-fail-fast"),
        compile_opts,
    };

    let err = ops::run_tests(&ws, &ops, &test_args)?;
    match err {
        None => Ok(()),
        Some(err) => {
            let context = anyhow::format_err!("{}", err.hint(&ws, &ops.compile_opts));
            let e = match err.code {
                // Don't show "process didn't exit successfully" for simple errors.
                Some(i) if cargo_util::is_simple_exit_code(i) => CliError::new(context, i),
                Some(i) => CliError::new(Error::from(err).context(context), i),
                None => CliError::new(Error::from(err).context(context), 101),
            };
            Err(e)
        }
    }
}
