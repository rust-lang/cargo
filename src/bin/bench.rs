use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, MessageFormat, Packages};
use cargo::util::{CliResult, CliError, Config, CargoErrorKind};
use cargo::util::important_paths::{find_root_manifest_for_wd};

#[derive(Deserialize)]
pub struct Options {
    flag_no_run: bool,
    flag_package: Vec<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_all_features: bool,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_message_format: MessageFormat,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_bins: bool,
    flag_example: Vec<String>,
    flag_examples: bool,
    flag_test: Vec<String>,
    flag_tests: bool,
    flag_bench: Vec<String>,
    flag_benches: bool,
    flag_no_fail_fast: bool,
    flag_frozen: bool,
    flag_locked: bool,
    arg_args: Vec<String>,
    flag_all: bool,
    flag_exclude: Vec<String>,
}

pub const USAGE: &'static str = "
Execute all benchmarks of a local package

Usage:
    cargo bench [options] [--] [<args>...]

Options:
    -h, --help                   Print this message
    --lib                        Benchmark only this package's library
    --bin NAME                   Benchmark only the specified binary
    --bins                       Benchmark all binaries
    --example NAME               Benchmark only the specified example
    --examples                   Benchmark all examples
    --test NAME                  Benchmark only the specified test target
    --tests                      Benchmark all tests
    --bench NAME                 Benchmark only the specified bench target
    --benches                    Benchmark all benches
    --no-run                     Compile, but don't run benchmarks
    -p SPEC, --package SPEC ...  Package to run benchmarks for
    --all                        Benchmark all packages in the workspace
    --exclude SPEC ...           Exclude packages from the benchmark
    -j N, --jobs N               Number of parallel jobs, defaults to # of CPUs
    --features FEATURES          Space-separated list of features to also build
    --all-features               Build all available features
    --no-default-features        Do not build the `default` feature
    --target TRIPLE              Build for the target triple
    --manifest-path PATH         Path to the manifest to build benchmarks for
    -v, --verbose ...            Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet                  No output printed to stdout
    --color WHEN                 Coloring: auto, always, never
    --message-format FMT         Error format: human, json [default: human]
    --no-fail-fast               Run all benchmarks regardless of failure
    --frozen                     Require Cargo.lock and cache are up to date
    --locked                     Require Cargo.lock is up to date

All of the trailing arguments are passed to the benchmark binaries generated
for filtering benchmarks and generally providing options configuring how they
run.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be benchmarked. If it is not given, then
the current package is benchmarked. For more information on SPEC and its format,
see the `cargo help pkgid` command.

All packages in the workspace are benchmarked if the `--all` flag is supplied. The
`--all` flag may be supplied in the presence of a virtual manifest.

The --jobs argument affects the building of the benchmark executable but does
not affect how many jobs are used when running the benchmarks.

Compilation can be customized with the `bench` profile in the manifest.
";

pub fn execute(options: Options, config: &Config) -> CliResult {
    debug!("executing; cmd=cargo-bench; args={:?}",
           env::args().collect::<Vec<_>>());

    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;
    let ws = Workspace::new(&root, config)?;

    let spec = Packages::from_flags(ws.is_virtual(),
                                    options.flag_all,
                                    &options.flag_exclude,
                                    &options.flag_package)?;

    let ops = ops::TestOptions {
        no_run: options.flag_no_run,
        no_fail_fast: options.flag_no_fail_fast,
        only_doc: false,
        compile_opts: ops::CompileOptions {
            config: config,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|s| &s[..]),
            features: &options.flag_features,
            all_features: options.flag_all_features,
            no_default_features: options.flag_no_default_features,
            spec: spec,
            release: true,
            mode: ops::CompileMode::Bench,
            filter: ops::CompileFilter::new(options.flag_lib,
                                            &options.flag_bin, options.flag_bins,
                                            &options.flag_test, options.flag_tests,
                                            &options.flag_example, options.flag_examples,
                                            &options.flag_bench, options.flag_benches,),
            message_format: options.flag_message_format,
            target_rustdoc_args: None,
            target_rustc_args: None,
        },
    };

    let err = ops::run_benches(&ws, &ops, &options.arg_args)?;
    match err {
        None => Ok(()),
        Some(err) => {
            Err(match err.exit.as_ref().and_then(|e| e.code()) {
                Some(i) => CliError::new("bench failed".into(), i),
                None => CliError::new(CargoErrorKind::CargoTestErrorKind(err).into(), 101)
            })
        }
    }
}
