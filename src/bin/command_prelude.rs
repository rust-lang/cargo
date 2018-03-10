use std::path::PathBuf;

use clap::{self, SubCommand, AppSettings, ArgMatches};
use cargo::{Config, CargoResult};
use cargo::core::Workspace;
use cargo::ops::{CompileMode, CompileOptions, CompileFilter, Packages, MessageFormat,
                 VersionControl, NewOptions};
use cargo::util::important_paths::find_root_manifest_for_wd;

pub use clap::Arg;

pub type App = clap::App<'static, 'static>;

pub trait AppExt: Sized {
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

impl AppExt for App {
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

pub fn values(args: &ArgMatches, name: &str) -> Vec<String> {
    args.values_of(name).unwrap_or_default()
        .map(|s| s.to_string())
        .collect()
}


pub fn root_manifest_from_args(config: &Config, args: &ArgMatches) -> CargoResult<PathBuf> {
    let manifest_path = args.value_of("manifest-path").map(|s| s.to_string());
    find_root_manifest_for_wd(manifest_path, config.cwd())
}

pub fn workspace_from_args<'a>(config: &'a Config, args: &ArgMatches) -> CargoResult<Workspace<'a>> {
    let root = root_manifest_from_args(config, args)?;
    Workspace::new(&root, config)
}

pub fn jobs_from_args(args: &ArgMatches) -> CargoResult<Option<u32>> { //FIXME: validation
    let jobs = match args.value_of("jobs") {
        None => None,
        Some(jobs) => Some(jobs.parse::<u32>().map_err(|_| {
            clap::Error::value_validation_auto(
                format!("could not parse `{}` as a number", jobs)
            )
        })?)
    };
    Ok(jobs)
}

pub fn compile_options_from_args<'a>(
    config: &'a Config,
    args: &'a ArgMatches<'a>,
    mode: CompileMode,
) -> CargoResult<CompileOptions<'a>> {
    let spec = Packages::from_flags(
        args.is_present("all"),
        values(args, "exclude"),
        values(args, "package"),
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
        jobs: jobs_from_args(args)?,
        target: args.value_of("target"),
        features: values(args, "features"),
        all_features: args.is_present("all-features"),
        no_default_features: args.is_present("no-default-features"),
        spec,
        mode,
        release: args.is_present("release"),
        filter: CompileFilter::new(args.is_present("lib"),
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

pub fn compile_options_from_args_for_single_package<'a>(
    config: &'a Config,
    args: &'a ArgMatches<'a>,
    mode: CompileMode,
) -> CargoResult<CompileOptions<'a>> {
    let mut compile_opts = compile_options_from_args(config, args, mode)?;
    compile_opts.spec = Packages::Packages(values(args, "package"));
    Ok(compile_opts)
}

pub fn new_opts_from_args<'a>(args: &'a ArgMatches<'a>, path: &'a str) -> CargoResult<NewOptions<'a>> {
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

pub fn registry_from_args(config: &Config, args: &ArgMatches) -> CargoResult<Option<String>> {
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

pub fn index_from_args(config: &Config, args: &ArgMatches) -> CargoResult<Option<String>> {
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
