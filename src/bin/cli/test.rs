use super::command_prelude::*;
use clap::AppSettings;

pub fn cli() -> App {
    subcommand("test").alias("t")
        .setting(AppSettings::TrailingVarArg)
        .about("Execute all unit and integration tests of a local package")
        .arg(
            Arg::with_name("TESTNAME").help(
                "If specified, only run tests containing this string in their names"
            )
        )
        .arg(
            Arg::with_name("args").help(
                "Arguments for the test binary"
            ).multiple(true).last(true)
        )
        .arg_targets_all(
            "Test only this package's library",
            "Test only the specified binary",
            "Test all binaries",
            "Check that the specified examples compile",
            "Check that all examples compile",
            "Test only the specified test target",
            "Test all tests",
            "Test only the specified bench target",
            "Test all benches",
            "Test all targets (default)",
        )
        .arg(opt("doc", "Test only this library's documentation"))
        .arg(
            opt("no-run", "Compile, but don't run tests")
        )
        .arg(
            opt("no-fail-fast", "Run all tests regardless of failure")
        )
        .arg_package(
            "Package to run tests for",
            "Test all packages in the workspace",
            "Exclude packages from the test",
        )
        .arg_jobs()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_manifest_path()
        .arg_message_format()
        .after_help("\
All of the trailing arguments are passed to the test binaries generated for
filtering tests and generally providing options configuring how they run. For
example, this will run all tests with the name `foo` in their name:

    cargo test foo

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be tested. If it is not given, then the
current package is tested. For more information on SPEC and its format, see the
`cargo help pkgid` command.

All packages in the workspace are tested if the `--all` flag is supplied. The
`--all` flag is automatically assumed for a virtual manifest.
Note that `--exclude` has to be specified in conjunction with the `--all` flag.

The --jobs argument affects the building of the test executable but does
not affect how many jobs are used when running the tests. The default value
for the --jobs argument is the number of CPUs. If you want to control the
number of simultaneous running test cases, pass the `--test-threads` option
to the test binaries:

    cargo test -- --test-threads=1

Compilation can be configured via the `test` profile in the manifest.

By default the rust test harness hides output from test execution to
keep results readable. Test output can be recovered (e.g. for debugging)
by passing `--nocapture` to the test binaries:

    cargo test -- --nocapture

To get the list of all options available for the test binaries use this:

    cargo test -- --help
")
}
