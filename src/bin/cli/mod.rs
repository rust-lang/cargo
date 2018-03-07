extern crate clap;
#[cfg(never)]
extern crate cargo;

use std::slice;

use cargo;

use clap::{AppSettings, Arg, ArgMatches};
use cargo::{Config, CargoResult};
use cargo::core::Workspace;
use cargo::util::important_paths::find_root_manifest_for_wd;
use cargo::ops::{self, MessageFormat, Packages, CompileOptions, CompileMode};
use cargo::CliError;


pub fn do_main(config: &mut Config) -> Result<(), CliError> {
    let args = cli().get_matches();
    if args.is_present("version") {
        let version = cargo::version();
        println!("{}", version);
        if args.occurrences_of("verbose") > 0 {
            println!("release: {}.{}.{}",
                     version.major,
                     version.minor,
                     version.patch);
            if let Some(ref cfg) = version.cfg_info {
                if let Some(ref ci) = cfg.commit_info {
                    println!("commit-hash: {}", ci.commit_hash);
                    println!("commit-date: {}", ci.commit_date);
                }
            }
        }
        return Ok(());
    }

    fn values<'a>(args: &ArgMatches, name: &str) -> &'a [String] {
        let owned: Vec<String> = args.values_of(name).unwrap_or_default()
            .map(|s| s.to_string())
            .collect();
        let owned = owned.into_boxed_slice();
        let ptr = owned.as_ptr();
        let len = owned.len();
        ::std::mem::forget(owned);
        unsafe {
            slice::from_raw_parts(ptr, len)
        }
    }

    fn config_from_args(config: &mut Config, args: &ArgMatches) -> CargoResult<()> {
        let color = args.value_of("color").map(|s| s.to_string());
        config.configure(
            args.occurrences_of("verbose") as u32,
            if args.is_present("quite") { Some(true) } else { None },
            &color,
            args.is_present("frozen"),
            args.is_present("locked"),
            &args.values_of_lossy("unstable-features").unwrap_or_default(),
        )
    }

    fn workspace_from_args<'a>(config: &'a Config, args: &ArgMatches) -> CargoResult<Workspace<'a>> {
        let manifest_path = args.value_of("manifest-path").map(|s| s.to_string());
        let root = find_root_manifest_for_wd(manifest_path, config.cwd())?;
        Workspace::new(&root, config)
    }

    fn compile_options_from_args<'a>(
        config: &'a Config,
        args: &'a ArgMatches<'a>,
        mode: CompileMode,
    ) -> CargoResult<CompileOptions<'a>> {
        let spec = Packages::from_flags(
            args.is_present("all"),
            &values(args, "exclude"),
            &values(args, "package"),
        )?;

        let release = mode == CompileMode::Bench || args.is_present("release");
        let message_format = match args.value_of("message-format") {
            Some("json") => MessageFormat::Json,
            Some("human") => MessageFormat::Human,
            f => panic!("Impossible message format: {:?}", f),
        };

        let opts = CompileOptions {
            config,
            jobs: args.value_of("jobs").and_then(|v| v.parse().ok()),
            target: args.value_of("target"),
            features: &values(args, "features"),
            all_features: args.is_present("all-features"),
            no_default_features: args.is_present("no-default-features"),
            spec,
            mode,
            release,
            filter: ops::CompileFilter::new(args.is_present("lib"),
                                            values(args, "bin"), args.is_present("bins"),
                                            values(args, "test"), args.is_present("tests"),
                                            values(args, "example"), args.is_present("examples"),
                                            values(args, "bench"), args.is_present("benches"),
                                            args.is_present("all-targets")),
            message_format,
            target_rustdoc_args: None,
            target_rustc_args: None,
        };
        Ok(opts)
    }

    match args.subcommand() {
        ("bench", Some(args)) => {
            config_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let compile_opts = compile_options_from_args(config, args, CompileMode::Bench)?;

            let ops = ops::TestOptions {
                no_run: args.is_present("no-run"),
                no_fail_fast: args.is_present("no-fail-fast"),
                only_doc: false,
                compile_opts,
            };

            let mut bench_args = vec![];
            bench_args.extend(args.value_of("BENCHNAME").into_iter().map(|s| s.to_string()));
            bench_args.extend(args.values_of("args").unwrap_or_default().map(|s| s.to_string()));

            let err = ops::run_benches(&ws, &ops, &bench_args)?;
            return match err {
                None => Ok(()),
                Some(err) => {
                    Err(match err.exit.as_ref().and_then(|e| e.code()) {
                        Some(i) => CliError::new(format_err!("bench failed"), i),
                        None => CliError::new(err.into(), 101)
                    })
                }
            };
        }
        ("build", Some(args)) => {
            config_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let compile_opts = compile_options_from_args(config, args, CompileMode::Build)?;
            ops::compile(&ws, &compile_opts)?;
            return Ok(());
        }
        ("check", Some(args)) => {
            config_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let test = match args.value_of("profile") {
                Some("test") => true,
                None => false,
                Some(profile) => {
                    let err = format_err!("unknown profile: `{}`, only `test` is \
                                       currently supported", profile);
                    return Err(CliError::new(err, 101));
                }
            };
            let mode = CompileMode::Check { test };
            let compile_opts = compile_options_from_args(config, args, mode)?;
            ops::compile(&ws, &compile_opts)?;
            return Ok(());
        }
        ("clean", Some(args)) => {
            config_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let opts = ops::CleanOptions {
                config,
                spec: &values(args, "package"),
                target: args.value_of("target"),
                release: args.is_present("release"),
            };
            ops::clean(&ws, &opts)?;
            return Ok(());
        }
        ("doc", Some(args)) => {
            config_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let mode = ops::CompileMode::Doc { deps: !args.is_present("no-deps") };
            let compile_opts = compile_options_from_args(config, args, mode)?;
            let doc_opts = ops::DocOptions {
                open_result: args.is_present("open"),
                compile_opts,
            };
            ops::doc(&ws, &doc_opts)?;
            return Ok(());
        }
        _ => return Ok(())
    }
}

use self::utils::*;

fn cli() -> App {
    let app = App::new("cargo")
        .settings(&[
            AppSettings::DisableVersion,
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::VersionlessSubcommands,
        ])
        .about("Rust's package manager")
        .arg(
            opt("version", "Print version info and exit")
                .short("V")
        )
        .arg(
            opt("list", "List installed commands")
        )
        .arg(
            opt("explain", "Run `rustc --explain CODE`")
                .value_name("CODE")
        )
        .arg(
            opt("verbose", "Use verbose output (-vv very verbose/build.rs output)")
                .short("v").multiple(true).global(true)
        )
        .arg(
            opt("quite", "No output printed to stdout")
                .short("q").global(true)
        )
        .arg(
            opt("color", "Coloring: auto, always, never")
                .value_name("WHEN").global(true)
        )
        .arg(
            Arg::with_name("unstable-features").help("Unstable (nightly-only) flags to Cargo")
                .short("Z").value_name("FLAG").multiple(true).global(true)
        )
        .after_help("\
Some common cargo commands are (see all commands with --list):
    build       Compile the current project
    check       Analyze the current project and report errors, but don't build object files
    clean       Remove the target directory
    doc         Build this project's and its dependencies' documentation
    new         Create a new cargo project
    init        Create a new cargo project in an existing directory
    run         Build and execute src/main.rs
    test        Run the tests
    bench       Run the benchmarks
    update      Update dependencies listed in Cargo.lock
    search      Search registry for crates
    publish     Package and upload this project to the registry
    install     Install a Rust binary
    uninstall   Uninstall a Rust binary

See 'cargo help <command>' for more information on a specific command.
")
        .subcommands(vec![
            bench::cli(),
            build::cli(),
            check::cli(),
            clean::cli(),
            doc::cli(),
        ])
    ;
    app
}

mod bench;
mod build;
mod check;
mod clean;
mod doc;

mod utils {
    use clap::{self, SubCommand, AppSettings};
    pub use clap::Arg;

    pub type App = clap::App<'static, 'static>;

    pub trait CommonArgs: Sized {
        fn _arg(self, arg: Arg<'static, 'static>) -> Self;

        fn arg_package(self, package: &'static str, all: &'static str, exclude: &'static str) -> Self {
            self._arg(opt("package", package).short("p").value_name("SPEC").multiple(true))
                ._arg(opt("all", all))
                ._arg(opt("exclude", exclude).value_name("SPEC").multiple(true))
        }

        fn arg_jobs(self) -> Self {
            self._arg(
                opt("jobs", "Number of parallel jobs, defaults to # of CPUs")
                    .short("j").value_name("N")
            )
        }

        fn arg_targets_all(
            self,
            lib: &'static str,
            bin: &'static str,
            bins: &'static str,
            examle: &'static str,
            examles: &'static str,
            test: &'static str,
            tests: &'static str,
            bench: &'static str,
            benchs: &'static str,
            all: &'static str,
        ) -> Self {
            self.arg_targets_lib_bin(lib, bin, bins)
                ._arg(opt("example", examle).value_name("NAME").multiple(true))
                ._arg(opt("examples", examles))
                ._arg(opt("test", test).value_name("NAME").multiple(true))
                ._arg(opt("tests", tests))
                ._arg(opt("bench", bench).value_name("NAME").multiple(true))
                ._arg(opt("benches", benchs))
                ._arg(opt("all-targets", all))
        }

        fn arg_targets_lib_bin(
            self,
            lib: &'static str,
            bin: &'static str,
            bins: &'static str,
        ) -> Self {
            self._arg(opt("lib", lib))
                ._arg(opt("bin", bin).value_name("NAME").multiple(true))
                ._arg(opt("bins", bins))
        }

        fn arg_features(self) -> Self {
            self
                ._arg(
                    opt("features", "Space-separated list of features to also enable")
                        .value_name("FEATURES")
                )
                ._arg(opt("all-features", "Enable all available features"))
                ._arg(opt("no-default-features", "Do not enable the `default` feature"))
        }

        fn arg_release(self, release: &'static str) -> Self {
            self._arg(opt("release", release))
        }

        fn arg_target_triple(self, target: &'static str) -> Self {
            self._arg(opt("target", target).value_name("TRIPLE"))
        }

        fn arg_manifest_path(self) -> Self {
            self._arg(opt("manifest-path", "Path to Cargo.toml").value_name("PATH"))
        }

        fn arg_message_format(self) -> Self {
            self._arg(
                opt("message-format", "Error format")
                    .value_name("FMT").possible_values(&["human", "json"]).default_value("human")
            )
        }

        fn arg_locked(self) -> Self {
            self._arg(opt("frozen", "Require Cargo.lock and cache are up to date"))
                ._arg(opt("locked", "Require Cargo.lock is up to date"))
        }
    }

    impl CommonArgs for App {
        fn _arg(self, arg: Arg<'static, 'static>) -> Self {
            self.arg(arg)
        }
    }

    pub fn opt(name: &'static str, help: &'static str) -> Arg<'static, 'static> {
        Arg::with_name(name).long(name).help(help)
    }

    pub fn subcommand(name: &'static str) -> App {
        SubCommand::with_name(name)
            .settings(&[
                AppSettings::UnifiedHelpMessage,
                AppSettings::DeriveDisplayOrder,
                AppSettings::TrailingVarArg,
                AppSettings::DontCollapseArgsInUsage,
            ])
    }
}
