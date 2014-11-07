use std::io::process::ExitStatus;

use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError, CargoError};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[deriving(Decodable)]
struct Options {
    arg_args: Vec<String>,
    flag_features: Vec<String>,
    flag_jobs: Option<uint>,
    flag_manifest_path: Option<String>,
    flag_name: Option<String>,
    flag_no_default_features: bool,
    flag_no_run: bool,
    flag_package: Option<String>,
    flag_target: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Execute all unit and integration tests of a local package

Usage:
    cargo test [options] [--] [<args>...]

Options:
    -h, --help               Print this message
    --name NAME              Name of the test executable to run
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
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    shell.set_verbose(options.flag_verbose);

    let mut ops = ops::TestOptions {
        name: options.flag_name.as_ref().map(|s| s.as_slice()),
        no_run: options.flag_no_run,
        compile_opts: ops::CompileOptions {
            env: "test",
            shell: shell,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|s| s.as_slice()),
            dev_deps: true,
            features: options.flag_features.as_slice(),
            no_default_features: options.flag_no_default_features,
            spec: options.flag_package.as_ref().map(|s| s.as_slice()),
        },
    };

    let err = try!(ops::run_tests(&root, &mut ops,
                                  options.arg_args.as_slice()).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            Err(match err.exit {
                Some(ExitStatus(i)) => CliError::new("", i as uint),
                _ => CliError::from_boxed(err.concrete().mark_human(), 101)
            })
        }
    }
}
