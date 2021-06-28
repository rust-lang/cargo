use crate::command_prelude::*;
use cargo::ops::{self, TestOptions};

pub fn cli() -> App {
    subcommand("bench")
        .setting(AppSettings::TrailingVarArg)
        .about("Execute all benchmarks of a local package")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(
            Arg::with_name("BENCHNAME")
                .help("If specified, only run benches containing this string in their names"),
        )
        .arg(
            Arg::with_name("args")
                .help("Arguments for the bench binary")
                .multiple(true)
                .last(true),
        )
        .arg_targets_all(
            "Benchmark only this package's library",
            "Benchmark only the specified binary",
            "Benchmark all binaries",
            "Benchmark only the specified example",
            "Benchmark all examples",
            "Benchmark only the specified test target",
            "Benchmark all tests",
            "Benchmark only the specified bench target",
            "Benchmark all benches",
            "Benchmark all targets",
        )
        .arg(opt("no-run", "Compile, but don't run benchmarks"))
        .arg_package_spec(
            "Package to run benchmarks for",
            "Benchmark all packages in the workspace",
            "Exclude packages from the benchmark",
        )
        .arg_jobs()
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg(opt(
            "no-fail-fast",
            "Run all benchmarks regardless of failure",
        ))
        .arg_unit_graph()
        .after_help("Run `cargo help bench` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    let rustc = config.load_global_rustc(Some(&ws));
    let mut compile_opts = args.compile_options(
        config,
        rustc,
        CompileMode::Bench,
        Some(&ws),
        ProfileChecking::Checked,
    )?;

    compile_opts.build_config.requested_profile =
        args.get_profile_name(config, "bench", ProfileChecking::Checked)?;

    let ops = TestOptions {
        no_run: args.is_present("no-run"),
        no_fail_fast: args.is_present("no-fail-fast"),
        compile_opts,
    };

    let bench_args = args.value_of("BENCHNAME").into_iter();
    let bench_args = bench_args.chain(args.values_of("args").unwrap_or_default());
    let bench_args = bench_args.collect::<Vec<_>>();

    let err = ops::run_benches(&ws, &ops, &bench_args)?;
    match err {
        None => Ok(()),
        Some(err) => Err(match err.code {
            Some(i) => CliError::new(anyhow::format_err!("bench failed"), i),
            None => CliError::new(err.into(), 101),
        }),
    }
}
