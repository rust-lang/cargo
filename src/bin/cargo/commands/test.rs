use cargo::ops::{self, CompileFilter, FilterRule, LibRule};

use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("test")
        // Subcommand aliases are handled in `aliased_command()`.
        // .alias("t")
        .setting(AppSettings::TrailingVarArg)
        .about("Execute all unit and integration tests and build examples of a local package")
        .arg(
            Arg::with_name("TESTNAME")
                .help("If specified, only run tests containing this string in their names"),
        )
        .arg(
            Arg::with_name("args")
                .help("Arguments for the test binary")
                .multiple(true)
                .last(true),
        )
        .arg(
            opt("quiet", "Display one character per test instead of one line")
            .short("q")
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
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
The test filtering argument TESTNAME and all the arguments following the
two dashes (`--`) are passed to the test binaries and thus to libtest
(rustc's built in unit-test and micro-benchmarking framework). If you're
passing arguments to both Cargo and the binary, the ones after `--` go to the
binary, the ones before go to Cargo. For details about libtest's arguments see
the output of `cargo test -- --help`. As an example, this will run all
tests with `foo` in their name on 3 threads in parallel:

    cargo test foo -- --test-threads 3

If the `--package` argument is given, then SPEC is a package ID specification
which indicates which package should be tested. If it is not given, then the
current package is tested. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are tested if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

The `--jobs` argument affects the building of the test executable but does
not affect how many jobs are used when running the tests. The default value
for the `--jobs` argument is the number of CPUs. If you want to control the
number of simultaneous running test cases, pass the `--test-threads` option
to the test binaries:

    cargo test -- --test-threads=1

Compilation can be configured via the `test` profile in the manifest.

By default the rust test harness hides output from test execution to
keep results readable. Test output can be recovered (e.g., for debugging)
by passing `--nocapture` to the test binaries:

    cargo test -- --nocapture

To get the list of all options available for the test binaries use this:

    cargo test -- --help
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let mut compile_opts = args.compile_options(config, CompileMode::Test, Some(&ws))?;

    // `TESTNAME` is actually an argument of the test binary, but it's
    // important, so we explicitly mention it and reconfigure.
    let test_name: Option<&str> = args.value_of("TESTNAME");
    let test_args = args.value_of("TESTNAME").into_iter();
    let test_args = test_args.chain(args.values_of("args").unwrap_or_default());
    let test_args = test_args.collect::<Vec<_>>();

    let no_run = args.is_present("no-run");
    let doc = args.is_present("doc");
    if doc {
        if let CompileFilter::Only { .. } = compile_opts.filter {
            return Err(CliError::new(
                failure::format_err!("Can't mix --doc with other target selecting options"),
                101,
            ));
        }
        if no_run {
            return Err(CliError::new(
                failure::format_err!("Can't skip running doc tests with --no-run"),
                101,
            ));
        }
        compile_opts.build_config.mode = CompileMode::Doctest;
        compile_opts.filter = ops::CompileFilter::new(
            LibRule::True,
            FilterRule::none(),
            FilterRule::none(),
            FilterRule::none(),
            FilterRule::none(),
        );
    } else if test_name.is_some() {
        if let CompileFilter::Default { .. } = compile_opts.filter {
            compile_opts.filter = ops::CompileFilter::new(
                LibRule::Default, // compile the library, so the unit tests can be run filtered
                FilterRule::All,    // compile the binaries, so the unit tests in binaries can be run filtered
                FilterRule::All,    // compile the tests, so the integration tests can be run filtered
                FilterRule::none(), // specify --examples to unit test binaries filtered
                FilterRule::none(), // specify --benches to unit test benchmarks filtered
            ); // also, specify --doc to run doc tests filtered
        }
    }

    let ops = ops::TestOptions {
        no_run,
        no_fail_fast: args.is_present("no-fail-fast"),
        compile_opts,
    };

    let err = ops::run_tests(&ws, &ops, &test_args)?;
    match err {
        None => Ok(()),
        Some(err) => Err(match err.exit.as_ref().and_then(|e| e.code()) {
            Some(i) => CliError::new(failure::format_err!("{}", err.hint(&ws)), i),
            None => CliError::new(err.into(), 101),
        }),
    }
}
