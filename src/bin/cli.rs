extern crate clap;
#[cfg(never)]
extern crate cargo;

use std::io::{self, Read, BufRead};
use std::cmp::min;
use std::fs::File;
use std::collections::HashMap;
use std::process;

use clap::{AppSettings, Arg, ArgMatches};
use toml;

use cargo::{self, Config, CargoError, CliResult, CliError};
use cargo::core::{Source, SourceId, GitReference, Package};
use cargo::util::{ToUrl, CargoResultExt};
use cargo::ops::{self, CompileMode, OutputMetadataOptions};
use cargo::sources::{GitSource, RegistrySource};

use std::collections::BTreeSet;
use std::env;
use std::fs;

use search_directories;
use is_executable;
use command_prelude::*;
use commands;

pub fn do_main(config: &mut Config) -> CliResult {
    let args = cli().get_matches_safe()?;
    let is_verbose = args.occurrences_of("verbose") > 0;
    if args.is_present("version") {
        let version = cargo::version();
        println!("{}", version);
        if is_verbose {
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

    if let Some(ref code) = args.value_of("explain") {
        let mut procss = config.rustc()?.process();
        procss.arg("--explain").arg(code).exec()?;
        return Ok(());
    }

    if args.is_present("list") {
        println!("Installed Commands:");
        for command in list_commands(config) {
            let (command, path) = command;
            if is_verbose {
                match path {
                    Some(p) => println!("    {:<20} {}", command, p),
                    None => println!("    {:<20}", command),
                }
            } else {
                println!("    {}", command);
            }
        }
        return Ok(());
    }

    execute_subcommand(config, args)
}

fn execute_subcommand(config: &mut Config, args: ArgMatches) -> CliResult {
    config.configure(
        args.occurrences_of("verbose") as u32,
        if args.is_present("quite") { Some(true) } else { None },
        &args.value_of("color").map(|s| s.to_string()),
        args.is_present("frozen"),
        args.is_present("locked"),
        &args.values_of_lossy("unstable-features").unwrap_or_default(),
    )?;

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
            let mut ws = workspace_from_args(config, args)?;
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
                spec: values(args, "package"),
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
            let root = args.root_manifest(config)?;

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
                features: values(args, "features"),
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
                jobs: jobs_from_args(args)?,
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
                jobs: jobs_from_args(args)?,
                dry_run: args.is_present("dry-run"),
                registry,
            })?;
            Ok(())
        }
        ("read-manifest", Some(args)) => {
            let root = args.root_manifest(config)?;
            let pkg = Package::for_path(&root, config)?;
            cargo::print_json(&pkg);
            Ok(())
        }
        ("run", Some(args)) => {
            let ws = workspace_from_args(config, args)?;

            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, CompileMode::Build,
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
                Some("check") => CompileMode::Check { test: false },
                Some(mode) => {
                    let err = format_err!("unknown profile: `{}`, use dev,
                                   test, or bench", mode);
                    return Err(CliError::new(err, 101));
                }
            };
            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, mode,
            )?;
            compile_opts.target_rustc_args = Some(values(args, "args"));
            ops::compile(&ws, &compile_opts)?;
            Ok(())
        }
        ("rustdoc", Some(args)) => {
            let ws = workspace_from_args(config, args)?;
            let mut compile_opts = compile_options_from_args_for_single_package(
                config, args, CompileMode::Doc { deps: false },
            )?;
            compile_opts.target_rustdoc_args = Some(values(args, "args"));
            let doc_opts = ops::DocOptions {
                open_result: args.is_present("open"),
                compile_opts,
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
                                                              Vec::new(), false,
                                                              Vec::new(), false,
                                                              Vec::new(), false,
                                                              Vec::new(), false,
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
            ops::uninstall(root, specs, &values(args, "bin"), config)?;
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
            let filename = match args.root_manifest(config) {
                Ok(filename) => filename,
                Err(e) => fail("invalid", &e.to_string()),
            };

            let file = File::open(&filename);
            match file.and_then(|mut f| f.read_to_string(&mut contents)) {
                Ok(_) => {}
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
        ("version", _) => {
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
        (cmd, Some(args)) => {
            if let Some(mut alias) = super::aliased_command(config, cmd)? {
                alias.extend(args.values_of("").unwrap_or_default().map(|s| s.to_string()));
                let args = cli()
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(alias)?;
                return execute_subcommand(config, args);
            }
            let mut ext_args: Vec<&str> = vec![cmd];
            ext_args.extend(args.values_of("").unwrap_or_default());
            super::execute_external_subcommand(config, cmd, &ext_args)
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
        .subcommands(commands::builtin())
    ;
    app
}


/// List all runnable commands
pub fn list_commands(config: &Config) -> BTreeSet<(String, Option<String>)> {
    let prefix = "cargo-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = BTreeSet::new();
    for dir in search_directories(config) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(filename) => filename,
                _ => continue,
            };
            if !filename.starts_with(prefix) || !filename.ends_with(suffix) {
                continue;
            }
            if is_executable(entry.path()) {
                let end = filename.len() - suffix.len();
                commands.insert(
                    (filename[prefix.len()..end].to_string(),
                     Some(path.display().to_string()))
                );
            }
        }
    }

    for cmd in commands::builtin() {
        commands.insert((cmd.get_name().to_string(), None));
    }

    commands
}
