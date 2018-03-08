extern crate clap;
#[cfg(never)]
extern crate cargo;

use std::slice;
use std::io::{self, Read, BufRead};
use std::path::PathBuf;
use std::cmp::min;
use std::fs::File;
use std::collections::HashMap;
use std::process;

use clap::{AppSettings, Arg, ArgMatches};
use toml;

use cargo::{self, Config, CargoResult, CargoError, CliError};
use cargo::core::{Workspace, Source, SourceId, GitReference, Package};
use cargo::util::{ToUrl, CargoResultExt};
use cargo::util::important_paths::find_root_manifest_for_wd;
use cargo::ops::{self, MessageFormat, Packages, CompileOptions, CompileMode, VersionControl,
                 OutputMetadataOptions, NewOptions};
use cargo::sources::{GitSource, RegistrySource};

use self::utils::*;

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

    config_from_args(config, &args)?;
    match args.subcommand() {
        ("bench", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let mut compile_opts = compile_options_from_args(config, args, CompileMode::Bench)?;
            compile_opts.release = true;

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
            match err {
                None => Ok(()),
                Some(err) => {
                    Err(match err.exit.as_ref().and_then(|e| e.code()) {
                        Some(i) => CliError::new(format_err!("bench failed"), i),
                        None => CliError::new(err.into(), 101)
                    })
                }
            }
        }
        ("build", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            if config.cli_unstable().avoid_dev_deps {
                ws.set_require_optional_deps(false);
            }
            let compile_opts = compile_options_from_args(config, args, CompileMode::Build)?;
            ops::compile(&ws, &compile_opts)?;
            Ok(())
        }
        ("check", Some(args)) => {
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
            Ok(())
        }
        ("clean", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let opts = ops::CleanOptions {
                config,
                spec: &values(args, "package"),
                target: args.value_of("target"),
                release: args.is_present("release"),
            };
            ops::clean(&ws, &opts)?;
            Ok(())
        }
        ("doc", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let mode = ops::CompileMode::Doc { deps: !args.is_present("no-deps") };
            let compile_opts = compile_options_from_args(config, args, mode)?;
            let doc_opts = ops::DocOptions {
                open_result: args.is_present("open"),
                compile_opts,
            };
            ops::doc(&ws, &doc_opts)?;
            Ok(())
        }
        ("fetch", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            ops::fetch(&ws)?;
            Ok(())
        }
        ("generate-lockfile", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            ops::generate_lockfile(&ws)?;
            Ok(())
        }
        ("git-checkout", Some(args)) => {
            let url = args.value_of("url").unwrap().to_url()?;
            let reference = args.value_of("reference").unwrap();

            let reference = GitReference::Branch(reference.to_string());
            let source_id = SourceId::for_git(&url, reference)?;

            let mut source = GitSource::new(&source_id, config)?;

            source.update()?;

            Ok(())
        }
        ("init", Some(args)) => {
            let path = args.value_of("path").unwrap_or(".");
            let opts = new_opts_from_args(args, path)?;
            ops::init(&opts, config)?;
            config.shell().status("Created", format!("{} project", opts.kind))?;
            Ok(())
        }
        ("install", Some(args)) => {
            let mut compile_opts = compile_options_from_args(config, args, CompileMode::Build)?;
            compile_opts.release = !args.is_present("debug");

            let krates = args.values_of("crate").unwrap_or_default().collect::<Vec<_>>();

            let source = if let Some(url) = args.value_of("git") {
                let url = url.to_url()?;
                let gitref = if let Some(branch) = args.value_of("branch") {
                    GitReference::Branch(branch.to_string())
                } else if let Some(tag) = args.value_of("tag") {
                    GitReference::Tag(tag.to_string())
                } else if let Some(rev) = args.value_of("rev") {
                    GitReference::Rev(rev.to_string())
                } else {
                    GitReference::Branch("master".to_string())
                };
                SourceId::for_git(&url, gitref)?
            } else if let Some(path) = args.value_of("path") {
                SourceId::for_path(&config.cwd().join(path))?
            } else if krates.is_empty() {
                SourceId::for_path(config.cwd())?
            } else {
                SourceId::crates_io(config)?
            };

            let version = args.value_of("version");
            let root = args.value_of("root");

            if args.is_present("list") {
                ops::install_list(root, config)?;
            } else {
                ops::install(root, krates, &source, version, &compile_opts, args.is_present("force"))?;
            }
            Ok(())
        }
        ("locate-project", Some(args)) => {
            let root = root_manifest_from_args(config, args)?;

            let root = root.to_str()
                .ok_or_else(|| format_err!("your project path contains characters \
                                            not representable in Unicode"))
                .map_err(|e| CliError::new(e, 1))?
                .to_string();

            #[derive(Serialize)]
            pub struct ProjectLocation {
                root: String
            }

            let location = ProjectLocation { root };

            cargo::print_json(&location);
            Ok(())
        }
        ("login", Some(args)) => {
            let registry = registry_from_args(config, args)?;

            let token = match args.value_of("token") {
                Some(token) => token.to_string(),
                None => {
                    let host = match registry {
                        Some(ref _registry) => {
                            return Err(format_err!("token must be provided when \
                                            --registry is provided.").into());
                        }
                        None => {
                            let src = SourceId::crates_io(config)?;
                            let mut src = RegistrySource::remote(&src, config);
                            src.update()?;
                            let config = src.config()?.unwrap();
                            args.value_of("host").map(|s| s.to_string())
                                .unwrap_or(config.api.unwrap())
                        }
                    };
                    println!("please visit {}me and paste the API Token below", host);
                    let mut line = String::new();
                    let input = io::stdin();
                    input.lock().read_line(&mut line).chain_err(|| {
                        "failed to read stdin"
                    }).map_err(CargoError::from)?;
                    line.trim().to_string()
                }
            };

            ops::registry_login(config, token, registry)?;
            Ok(())
        }
        ("metadata", Some(args)) => {
            let ws = workspace_from_args(config, args)?;

            let version = match args.value_of("format-version") {
                None => {
                    config.shell().warn("\
                        please specify `--format-version` flag explicitly \
                        to avoid compatibility problems"
                    )?;
                    1
                }
                Some(version) => version.parse().unwrap(),
            };

            let options = OutputMetadataOptions {
                features: values(args, "features").to_vec(),
                all_features: args.is_present("all-features"),
                no_default_features: args.is_present("no-default-features"),
                no_deps: args.is_present("no-deps"),
                version,
            };

            let result = ops::output_metadata(&ws, &options)?;
            cargo::print_json(&result);
            Ok(())
        }
        ("new", Some(args)) => {
            let path = args.value_of("path").unwrap();
            let opts = new_opts_from_args(args, path)?;
            ops::new(&opts, config)?;
            config.shell().status("Created", format!("{} `{}` project", opts.kind, path))?;
            Ok(())
        }
        ("owner", Some(args)) => {
            let registry = registry_from_args(config, args)?;
            let opts = ops::OwnersOptions {
                krate: args.value_of("crate").map(|s| s.to_string()),
                token: args.value_of("token").map(|s| s.to_string()),
                index: args.value_of("index").map(|s| s.to_string()),
                to_add: args.values_of("add")
                    .map(|xs| xs.map(|s| s.to_string()).collect()),
                to_remove: args.values_of("remove")
                    .map(|xs| xs.map(|s| s.to_string()).collect()),
                list: args.is_present("list"),
                registry,
            };
            ops::modify_owners(config, &opts)?;
            Ok(())
        }
        ("package", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            ops::package(&ws, &ops::PackageOpts {
                config,
                verify: !args.is_present("no-verify"),
                list: args.is_present("list"),
                check_metadata: !args.is_present("no-metadata"),
                allow_dirty: args.is_present("allow-dirty"),
                target: args.value_of("target"),
                jobs: jobs_from_args(args),
                registry: None,
            })?;
            Ok(())
        }
        ("pkgid", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let spec = args.value_of("spec").or(args.value_of("package"));
            let spec = ops::pkgid(&ws, spec)?;
            println!("{}", spec);
            Ok(())
        }
        ("publish", Some(args)) => {
            let registry = registry_from_args(config, args)?;
            let ws = workspace_from_args(config, args)?;
            let index = index_from_args(config, args)?;

            ops::publish(&ws, &ops::PublishOpts {
                config,
                token: args.value_of("token").map(|s| s.to_string()),
                index,
                verify: !args.is_present("no-verify"),
                allow_dirty: args.is_present("allow-dirty"),
                target: args.value_of("target"),
                jobs: jobs_from_args(args),
                dry_run: args.is_present("dry-run"),
                registry,
            })?;
            Ok(())
        }
        ("read-manifest", Some(args)) => {
            let root = root_manifest_from_args(config, args)?;
            let pkg = Package::for_path(&root, config)?;
            cargo::print_json(&pkg);
            Ok(())
        }
        ("run", Some(args)) => {
            let ws = workspace_from_args(config, args)?;

            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, CompileMode::Build
            )?;
            if !args.is_present("example") && !args.is_present("bin") {
                compile_opts.filter = ops::CompileFilter::Default {
                    required_features_filterable: false,
                };
            };
            match ops::run(&ws, &compile_opts, &values(args, "args"))? {
                None => Ok(()),
                Some(err) => {
                    // If we never actually spawned the process then that sounds pretty
                    // bad and we always want to forward that up.
                    let exit = match err.exit {
                        Some(exit) => exit,
                        None => return Err(CliError::new(err.into(), 101)),
                    };

                    // If `-q` was passed then we suppress extra error information about
                    // a failed process, we assume the process itself printed out enough
                    // information about why it failed so we don't do so as well
                    let exit_code = exit.code().unwrap_or(101);
                    Err(if args.is_present("quite") {
                        CliError::code(exit_code)
                    } else {
                        CliError::new(err.into(), exit_code)
                    })
                }
            }
        }
        ("rustc", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let mode = match args.value_of("profile") {
                Some("dev") | None => CompileMode::Build,
                Some("test") => CompileMode::Test,
                Some("bench") => CompileMode::Bench,
                Some("check") => CompileMode::Check {test: false},
                Some(mode) => {
                    let err = format_err!("unknown profile: `{}`, use dev,
                                   test, or bench", mode);
                    return Err(CliError::new(err, 101))
                }
            };
            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, mode
            )?;
            compile_opts.target_rustc_args = Some(&values(args, "args"));
            ops::compile(&ws, &compile_opts)?;
            Ok(())
        }
        ("rustdoc", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, CompileMode::Doc { deps: false }
            )?;
            compile_opts.target_rustdoc_args = Some(&values(args, "args"));
            let doc_opts = ops::DocOptions {
                open_result: args.is_present("open"),
                compile_opts
            };
            ops::doc(&ws, &doc_opts)?;
            Ok(())
        }
        ("search", Some(args)) => {
            let registry = registry_from_args(config, args)?;
            let index = index_from_args(config, args)?;
            let limit: Option<u8> = args.value_of("limit")
                .and_then(|v| v.parse().ok()); //FIXME: validation
            let limit = min(100, limit.unwrap_or(10));
            let query: Vec<&str> = args.values_of("query").unwrap_or_default().collect();
            let query: String = query.join("+");
            ops::search(&query, config, index, limit, registry)?;
            Ok(())
        }
        ("test", Some(args)) => {
            let ws = workspace_from_args(config, args)?;

            let mut compile_opts = compile_options_from_args(config, args, CompileMode::Test)?;
            let doc = args.is_present("doc");
            if doc {
                compile_opts.mode = ops::CompileMode::Doctest;
                compile_opts.filter = ops::CompileFilter::new(true,
                                                              &[], false,
                                                              &[], false,
                                                              &[], false,
                                                              &[], false,
                                                              false);
            }

            let ops = ops::TestOptions {
                no_run: args.is_present("no-run"),
                no_fail_fast: args.is_present("no-fail-fast"),
                only_doc: doc,
                compile_opts,
            };

            // TESTNAME is actually an argument of the test binary, but it's
            // important so we explicitly mention it and reconfigure
            let mut test_args = vec![];
            test_args.extend(args.value_of("TESTNAME").into_iter().map(|s| s.to_string()));
            test_args.extend(args.values_of("args").unwrap_or_default().map(|s| s.to_string()));

            let err = ops::run_tests(&ws, &ops, &test_args)?;
            return match err {
                None => Ok(()),
                Some(err) => {
                    Err(match err.exit.as_ref().and_then(|e| e.code()) {
                        Some(i) => CliError::new(format_err!("{}", err.hint(&ws)), i),
                        None => CliError::new(err.into(), 101),
                    })
                }
            };
        }
        ("uninstall", Some(args)) => {
            let root = args.value_of("root");
            let specs = args.values_of("spec").unwrap_or_default().collect();
            ops::uninstall(root, specs, values(args, "bin"), config)?;
            Ok(())
        }
        ("update", Some(args)) => {
            let ws = workspace_from_args(config, args)?;

            let update_opts = ops::UpdateOptions {
                aggressive: args.is_present("aggressive"),
                precise: args.value_of("precise"),
                to_update: values(args, "package"),
                config,
            };
            ops::update_lockfile(&ws, &update_opts)?;
            Ok(())
        }
        ("verify-project", Some(args)) => {
            fn fail(reason: &str, value: &str) -> ! {
                let mut h = HashMap::new();
                h.insert(reason.to_string(), value.to_string());
                cargo::print_json(&h);
                process::exit(1)
            }

            let mut contents = String::new();
            let filename = match root_manifest_from_args(config, args) {
                Ok(filename) => filename,
                Err(e) => fail("invalid", &e.to_string()),
            };

            let file = File::open(&filename);
            match file.and_then(|mut f| f.read_to_string(&mut contents)) {
                Ok(_) => {},
                Err(e) => fail("invalid", &format!("error reading file: {}", e))
            };
            if contents.parse::<toml::Value>().is_err() {
                fail("invalid", "invalid-format");
            }

            let mut h = HashMap::new();
            h.insert("success".to_string(), "true".to_string());
            cargo::print_json(&h);
            Ok(())
        }
        ("version", Some(args)) => {
            println!("{}", cargo::version());
            Ok(())
        }
        ("yank", Some(args)) => {
            let registry = registry_from_args(config, args)?;

            ops::yank(config,
                      args.value_of("crate").map(|s| s.to_string()),
                      args.value_of("vers").map(|s| s.to_string()),
                      args.value_of("token").map(|s| s.to_string()),
                      args.value_of("index").map(|s| s.to_string()),
                      args.is_present("undo"),
                      registry)?;
            Ok(())
        }
        (external, Some(args)) => {
            let mut ext_args: Vec<&str> = vec![external];
            ext_args.extend(args.values_of("").unwrap_or_default());
            super::execute_external_subcommand(config, external, &ext_args)
        }
        _ => Ok(())
    }
}


fn cli() -> App {
    let app = App::new("cargo")
        .settings(&[
            AppSettings::DisableVersion,
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::VersionlessSubcommands,
            AppSettings::AllowExternalSubcommands,
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
            opt("frozen", "Require Cargo.lock and cache are up to date")
                .global(true)
        )
        .arg(
            opt("locked", "Require Cargo.lock is up to date")
                .global(true)
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
            fetch::cli(),
            generate_lockfile::cli(),
            git_checkout::cli(),
            init::cli(),
            install::cli(),
            locate_project::cli(),
            login::cli(),
            metadata::cli(),
            new::cli(),
            owner::cli(),
            package::cli(),
            pkgid::cli(),
            publish::cli(),
            read_manifest::cli(),
            run::cli(),
            rustc::cli(),
            rustdoc::cli(),
            search::cli(),
            test::cli(),
            uninstall::cli(),
            update::cli(),
            verify_project::cli(),
            version::cli(),
            yank::cli(),
        ])
    ;
    app
}

mod bench;
mod build;
mod check;
mod clean;
mod doc;
mod fetch;
mod generate_lockfile;

// FIXME: let's just drop this subcommand?
mod git_checkout;

mod init;
mod install;
mod locate_project;
mod login;
mod metadata;
mod new;
mod owner;
mod package;
mod pkgid;
mod publish;
mod read_manifest;
mod run;
mod rustc;
mod rustdoc;
mod search;
mod test;
mod uninstall;
mod update;
mod verify_project;
mod version;
mod yank;

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

        fn arg_single_package(self, package: &'static str) -> Self {
            self._arg(opt("package", package).short("p").value_name("SPEC"))
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

        fn arg_targets_bins_examples(
            self,
            bin: &'static str,
            bins: &'static str,
            example: &'static str,
            examples: &'static str,
        ) -> Self {
            self._arg(opt("bin", bin).value_name("NAME").multiple(true))
                ._arg(opt("bins", bins))
                ._arg(opt("example", example).value_name("NAME").multiple(true))
                ._arg(opt("examples", examples))
        }

        fn arg_targets_bin_example(
            self,
            bin: &'static str,
            example: &'static str,
        ) -> Self {
            self._arg(opt("bin", bin).value_name("NAME").multiple(true))
                ._arg(opt("example", example).value_name("NAME").multiple(true))
        }

        fn arg_features(self) -> Self {
            self
                ._arg(
                    opt("features", "Space-separated list of features to activate")
                        .value_name("FEATURES")
                )
                ._arg(opt("all-features", "Activate all available features"))
                ._arg(opt("no-default-features", "Do not activate the `default` feature"))
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
                    .value_name("FMT")
                    .case_insensitive(true)
                    .possible_values(&["human", "json"]).default_value("human")
            )
        }

        fn arg_new_opts(self) -> Self {
            self._arg(
                opt("vcs", "\
Initialize a new repository for the given version \
control system (git, hg, pijul, or fossil) or do not \
initialize any version control at all (none), overriding \
a global configuration.")
                    .value_name("VCS")
                    .possible_values(&["git", "hg", "pijul", "fossil", "none"])
            )
                ._arg(opt("bin", "Use a binary (application) template [default]"))
                ._arg(opt("lib", "Use a library template"))
                ._arg(
                    opt("name", "Set the resulting package name, defaults to the directory name")
                        .value_name("NAME")
                )
        }

        fn arg_index(self) -> Self {
            self
                ._arg(
                    opt("index", "Registry index to upload the package to")
                        .value_name("INDEX")
                )
                ._arg(
                    opt("host", "DEPRECATED, renamed to '--index'")
                        .value_name("HOST")
                        .hidden(true)
                )
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
                AppSettings::DontCollapseArgsInUsage,
            ])
    }
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

fn root_manifest_from_args(config: &Config, args: &ArgMatches) -> CargoResult<PathBuf> {
    let manifest_path = args.value_of("manifest-path").map(|s| s.to_string());
    find_root_manifest_for_wd(manifest_path, config.cwd())
}

fn workspace_from_args<'a>(config: &'a Config, args: &ArgMatches) -> CargoResult<Workspace<'a>> {
    let root = root_manifest_from_args(config, args)?;
    Workspace::new(&root, config)
}

fn jobs_from_args(args: &ArgMatches) -> Option<u32> { //FIXME: validation
    args.value_of("jobs").and_then(|v| v.parse().ok())
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

    let message_format = match args.value_of("message-format") {
        None => MessageFormat::Human,
        Some(f) => {
            if f.eq_ignore_ascii_case("json") {
                MessageFormat::Json
            } else if f.eq_ignore_ascii_case("human") {
                MessageFormat::Human
            } else {
                panic!("Impossible message format: {:?}", f)
            }
        }
    };

    let opts = CompileOptions {
        config,
        jobs: jobs_from_args(args),
        target: args.value_of("target"),
        features: &values(args, "features"),
        all_features: args.is_present("all-features"),
        no_default_features: args.is_present("no-default-features"),
        spec,
        mode,
        release: args.is_present("release"),
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

fn compile_options_from_args_for_single_package<'a>(
    config: &'a Config,
    args: &'a ArgMatches<'a>,
    mode: CompileMode,
) -> CargoResult<CompileOptions<'a>> {
    let mut compile_opts = compile_options_from_args(config, args, mode)?;
    let packages = values(args, "package");
    compile_opts.spec = Packages::Packages(&packages);
    Ok(compile_opts)
}

fn new_opts_from_args<'a>(args: &'a ArgMatches<'a>, path: &'a str) -> CargoResult<NewOptions<'a>> {
    let vcs = args.value_of("vcs").map(|vcs| match vcs {
        "git" => VersionControl::Git,
        "hg" => VersionControl::Hg,
        "pijul" => VersionControl::Pijul,
        "fossil" => VersionControl::Fossil,
        "none" => VersionControl::NoVcs,
        vcs => panic!("Impossible vcs: {:?}", vcs),
    });
    NewOptions::new(vcs,
                    args.is_present("bin"),
                    args.is_present("lib"),
                    path,
                    args.value_of("name"))
}

fn registry_from_args(config: &Config, args: &ArgMatches) -> CargoResult<Option<String>> {
    match args.value_of("registry") {
        Some(registry) => {
            if !config.cli_unstable().unstable_options {
                return Err(format_err!("registry option is an unstable feature and \
                                requires -Zunstable-options to use.").into());
            }
            Ok(Some(registry.to_string()))
        }
        None => Ok(None),
    }
}

fn index_from_args(config: &Config, args: &ArgMatches) -> CargoResult<Option<String>> {
    // TODO: Deprecated
    // remove once it has been decided --host can be removed
    // We may instead want to repurpose the host flag, as
    // mentioned in this issue
    // https://github.com/rust-lang/cargo/issues/4208
    let msg = "The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index. Please use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.";

    let index = match args.value_of("host") {
        Some(host) => {
            config.shell().warn(&msg)?;
            Some(host.to_string())
        }
        None => args.value_of("index").map(|s| s.to_string())
    };
    Ok(index)
}
