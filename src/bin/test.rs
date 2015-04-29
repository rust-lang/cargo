use cargo::ops;
use cargo::util::{CliResult, CliError, Human, Config};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct Options {
    arg_args: Vec<String>,
    flag_features: Vec<String>,
    flag_jobs: Option<u32>,
    flag_manifest_path: Option<String>,
    flag_no_default_features: bool,
    flag_no_run: bool,
    flag_package: Option<String>,
    flag_target: Option<String>,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Execute all unit and integration tests of a local package

Usage:
    cargo test [options] [--] [<args>...]

Options:
    -h, --help               Print this message
    --lib                    Test only this package's library
    --bin NAME               Test only the specified binary
    --example NAME           Test only the specified example
    --test NAME              Test only the specified integration test
    --bench NAME             Test only the specified benchmark
    --no-run                 Compile, but don't run tests
    -p SPEC, --package SPEC  Package to run tests for
    -j N, --jobs N           The number of jobs to run in parallel
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to build tests for
    -v, --verbose            Use verbose output

All of the trailing arguments are passed to the test binaries generated for
filtering tests and generally providing options configuring how they run. For
example, this will run all tests with the name `foo` in their name:

    cargo test foo

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be tested. If it is not given, then the
current package is tested. For more information on SPEC and its format, see the
`cargo help pkgid` command.

Compilation can be configured via the `test` profile in the manifest.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    config.shell().set_verbose(options.flag_verbose);

    let ops = ops::TestOptions {
        no_run: options.flag_no_run,
        compile_opts: ops::CompileOptions {
            config: config,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|s| &s[..]),
            features: &options.flag_features,
            no_default_features: options.flag_no_default_features,
            spec: options.flag_package.as_ref().map(|s| &s[..]),
            exec_engine: None,
            release: false,
            mode: ops::CompileMode::Test,
            filter: ops::CompileFilter::new(options.flag_lib,
                                            &options.flag_bin,
                                            &options.flag_test,
                                            &options.flag_example,
                                            &options.flag_bench),
        },
    };

    let err = try!(ops::run_tests(&root, &ops,
                                  &options.arg_args).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            Err(match err.exit.as_ref().and_then(|e| e.code()) {
                Some(i) => CliError::new("", i),
                None => CliError::from_error(Human(err), 101)
            })
        }
    }
}
