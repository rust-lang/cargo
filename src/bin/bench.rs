use cargo::ops;
use cargo::util::{CliResult, CliError, Human, Config};
use cargo::util::important_paths::{find_root_manifest_for_cwd};

#[derive(RustcDecodable)]
struct Options {
    flag_no_run: bool,
    flag_package: Vec<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
    arg_args: Vec<String>,
}

pub const USAGE: &'static str = "
Execute all benchmarks of a local package

Usage:
    cargo bench [options] [--] [<args>...]

Options:
    -h, --help                   Print this message
    --lib                        Benchmark only this package's library
    --bin NAME                   Benchmark only the specified binary
    --example NAME               Benchmark only the specified example
    --test NAME                  Benchmark only the specified test target
    --bench NAME                 Benchmark only the specified bench target
    --no-run                     Compile, but don't run benchmarks
    -p SPEC, --package SPEC ...  Package to run benchmarks for
    -j N, --jobs N               The number of jobs to run in parallel
    --features FEATURES          Space-separated list of features to also build
    --no-default-features        Do not build the `default` feature
    --target TRIPLE              Build for the target triple
    --manifest-path PATH         Path to the manifest to build benchmarks for
    -v, --verbose                Use verbose output
    -q, --quiet                  No output printed to stdout
    --color WHEN                 Coloring: auto, always, never

All of the trailing arguments are passed to the benchmark binaries generated
for filtering benchmarks and generally providing options configuring how they
run.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be benchmarked. If it is not given, then
the current package is benchmarked. For more information on SPEC and its format,
see the `cargo help pkgid` command.

The --jobs argument affects the building of the benchmark executable but does
not affect how many jobs are used when running the benchmarks.

Compilation can be customized with the `bench` profile in the manifest.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let ops = ops::TestOptions {
        no_run: options.flag_no_run,
        no_fail_fast: false,
        compile_opts: ops::CompileOptions {
            config: config,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|s| &s[..]),
            features: &options.flag_features,
            no_default_features: options.flag_no_default_features,
            spec: &options.flag_package,
            exec_engine: None,
            release: true,
            mode: ops::CompileMode::Bench,
            filter: ops::CompileFilter::new(options.flag_lib,
                                            &options.flag_bin,
                                            &options.flag_test,
                                            &options.flag_example,
                                            &options.flag_bench),
            target_rustdoc_args: None,
            target_rustc_args: None,
        },
    };

    let err = try!(ops::run_benches(&root, &ops,
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
