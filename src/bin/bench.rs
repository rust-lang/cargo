use cargo::ops;
use cargo::util::{CliResult, CliError, Human, Config};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct Options {
    flag_no_run: bool,
    flag_package: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_bench: Option<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    arg_args: Vec<String>,
}

pub const USAGE: &'static str = "
Execute all benchmarks of a local package

Usage:
    cargo bench [options] [--] [<args>...]

Options:
    -h, --help               Print this message
    --bench NAME             Name of the bench to run
    --no-run                 Compile, but don't run benchmarks
    -p SPEC, --package SPEC  Package to run benchmarks for
    -j N, --jobs N           The number of jobs to run in parallel
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to build benchmarks for
    -v, --verbose            Use verbose output

All of the trailing arguments are passed to the benchmark binaries generated
for filtering benchmarks and generally providing options configuring how they
run.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be benchmarked. If it is not given, then
the current package is benchmarked. For more information on SPEC and its format,
see the `cargo help pkgid` command.

Compilation can be customized with the `bench` profile in the manifest.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    config.shell().set_verbose(options.flag_verbose);

    let ops = ops::TestOptions {
        name: options.flag_bench.as_ref().map(|s| s.as_slice()),
        no_run: options.flag_no_run,
        compile_opts: ops::CompileOptions {
            env: "bench",
            config: config,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|s| s.as_slice()),
            dev_deps: true,
            features: &options.flag_features,
            no_default_features: options.flag_no_default_features,
            spec: options.flag_package.as_ref().map(|s| s.as_slice()),
            lib_only: false,
            exec_engine: None,
        },
    };

    let err = try!(ops::run_benches(&root, &ops,
                                    &options.arg_args).map_err(|err| {
        CliError::from_boxed(err, 101)
    }));
    match err {
        None => Ok(None),
        Some(err) => {
            Err(match err.exit.as_ref().and_then(|c| c.code()) {
                Some(i) => CliError::new("", i),
                None => CliError::from_error(Human(err), 101)
            })
        }
    }
}
