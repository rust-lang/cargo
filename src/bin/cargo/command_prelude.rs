use std::path::PathBuf;
use std::fs;

use clap::{self, SubCommand};
use cargo::CargoResult;
use cargo::core::Workspace;
use cargo::core::compiler::{BuildConfig, MessageFormat};
use cargo::ops::{CompileFilter, CompileOptions, NewOptions, Packages, VersionControl};
use cargo::util::paths;
use cargo::util::important_paths::find_root_manifest_for_wd;

pub use clap::{AppSettings, Arg, ArgMatches};
pub use cargo::{CliError, CliResult, Config};
pub use cargo::core::compiler::CompileMode;

pub type App = clap::App<'static, 'static>;

pub trait AppExt: Sized {
    fn _arg(self, arg: Arg<'static, 'static>) -> Self;

    fn arg_package_spec(
        self,
        package: &'static str,
        all: &'static str,
        exclude: &'static str,
    ) -> Self {
        self.arg_package_spec_simple(package)
            ._arg(opt("all", all))
            ._arg(multi_opt("exclude", "SPEC", exclude))
    }

    fn arg_package_spec_simple(self, package: &'static str) -> Self {
        self._arg(multi_opt("package", "SPEC", package).short("p"))
    }

    fn arg_package(self, package: &'static str) -> Self {
        self._arg(opt("package", package).short("p").value_name("SPEC"))
    }

    fn arg_jobs(self) -> Self {
        self._arg(
            opt("jobs", "Number of parallel jobs, defaults to # of CPUs")
                .short("j")
                .value_name("N"),
        )
    }

    fn arg_targets_all(
        self,
        lib: &'static str,
        bin: &'static str,
        bins: &'static str,
        example: &'static str,
        examples: &'static str,
        test: &'static str,
        tests: &'static str,
        bench: &'static str,
        benches: &'static str,
        all: &'static str,
    ) -> Self {
        self.arg_targets_lib_bin(lib, bin, bins)
            ._arg(multi_opt("example", "NAME", example))
            ._arg(opt("examples", examples))
            ._arg(multi_opt("test", "NAME", test))
            ._arg(opt("tests", tests))
            ._arg(multi_opt("bench", "NAME", bench))
            ._arg(opt("benches", benches))
            ._arg(opt("all-targets", all))
    }

    fn arg_targets_lib_bin(self, lib: &'static str, bin: &'static str, bins: &'static str) -> Self {
        self._arg(opt("lib", lib))
            ._arg(multi_opt("bin", "NAME", bin))
            ._arg(opt("bins", bins))
    }

    fn arg_targets_bins_examples(
        self,
        bin: &'static str,
        bins: &'static str,
        example: &'static str,
        examples: &'static str,
    ) -> Self {
        self._arg(multi_opt("bin", "NAME", bin))
            ._arg(opt("bins", bins))
            ._arg(multi_opt("example", "NAME", example))
            ._arg(opt("examples", examples))
    }

    fn arg_targets_bin_example(self, bin: &'static str, example: &'static str) -> Self {
        self._arg(multi_opt("bin", "NAME", bin))
            ._arg(multi_opt("example", "NAME", example))
    }

    fn arg_features(self) -> Self {
        self._arg(
            opt("features", "Space-separated list of features to activate").value_name("FEATURES"),
        )._arg(opt("all-features", "Activate all available features"))
            ._arg(opt(
                "no-default-features",
                "Do not activate the `default` feature",
            ))
    }

    fn arg_release(self, release: &'static str) -> Self {
        self._arg(opt("release", release))
    }

    fn arg_doc(self, doc: &'static str) -> Self {
        self._arg(opt("doc", doc))
    }

    fn arg_target_triple(self, target: &'static str) -> Self {
        self._arg(opt("target", target).value_name("TRIPLE"))
    }

    fn arg_target_dir(self) -> Self {
        self._arg(opt("target-dir", "Directory for all generated artifacts").value_name("DIRECTORY"))
    }

    fn arg_manifest_path(self) -> Self {
        self._arg(opt("manifest-path", "Path to Cargo.toml").value_name("PATH"))
    }

    fn arg_message_format(self) -> Self {
        self._arg(
            opt("message-format", "Error format")
                .value_name("FMT")
                .case_insensitive(true)
                .possible_values(&["human", "json", "short"])
                .default_value("human"),
        )
    }

    fn arg_build_plan(self) -> Self {
        self._arg(opt("build-plan", "Output the build plan in JSON"))
    }

    fn arg_new_opts(self) -> Self {
        self._arg(
            opt(
                "vcs",
                "\
                 Initialize a new repository for the given version \
                 control system (git, hg, pijul, or fossil) or do not \
                 initialize any version control at all (none), overriding \
                 a global configuration.",
            ).value_name("VCS")
                .possible_values(&["git", "hg", "pijul", "fossil", "none"]),
        )._arg(opt("bin", "Use a binary (application) template [default]"))
            ._arg(opt("lib", "Use a library template"))
            ._arg(
                opt(
                    "name",
                    "Set the resulting package name, defaults to the directory name",
                ).value_name("NAME"),
            )
    }

    fn arg_index(self) -> Self {
        self._arg(opt("index", "Registry index to upload the package to").value_name("INDEX"))
            ._arg(
                opt("host", "DEPRECATED, renamed to '--index'")
                    .value_name("HOST")
                    .hidden(true),
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

pub fn multi_opt(
    name: &'static str,
    value_name: &'static str,
    help: &'static str,
) -> Arg<'static, 'static> {
    // Note that all `.multiple(true)` arguments in Cargo should specify
    // `.number_of_values(1)` as well, so that `--foo val1 val2` is
    // **not** parsed as `foo` with values ["val1", "val2"].
    // `number_of_values` should become the default in clap 3.
    opt(name, help)
        .value_name(value_name)
        .multiple(true)
        .number_of_values(1)
}

pub fn subcommand(name: &'static str) -> App {
    SubCommand::with_name(name).settings(&[
        AppSettings::UnifiedHelpMessage,
        AppSettings::DeriveDisplayOrder,
        AppSettings::DontCollapseArgsInUsage,
    ])
}

pub trait ArgMatchesExt {
    fn value_of_u32(&self, name: &str) -> CargoResult<Option<u32>> {
        let arg = match self._value_of(name) {
            None => None,
            Some(arg) => Some(arg.parse::<u32>().map_err(|_| {
                clap::Error::value_validation_auto(format!("could not parse `{}` as a number", arg))
            })?),
        };
        Ok(arg)
    }

    /// Returns value of the `name` command-line argument as an absolute path
    fn value_of_path(&self, name: &str, config: &Config) -> Option<PathBuf> {
        self._value_of(name).map(|path| config.cwd().join(path))
    }

    fn root_manifest(&self, config: &Config) -> CargoResult<PathBuf> {
        if let Some(path) = self.value_of_path("manifest-path", config) {
            // In general, we try to avoid normalizing paths in Cargo,
            // but in this particular case we need it to fix #3586.
            let path = paths::normalize_path(&path);
            if !path.ends_with("Cargo.toml") {
                bail!("the manifest-path must be a path to a Cargo.toml file")
            }
            if fs::metadata(&path).is_err() {
                bail!(
                    "manifest path `{}` does not exist",
                    self._value_of("manifest-path").unwrap()
                )
            }
            return Ok(path);
        }
        find_root_manifest_for_wd(config.cwd())
    }

    fn workspace<'a>(&self, config: &'a Config) -> CargoResult<Workspace<'a>> {
        let root = self.root_manifest(config)?;
        let mut ws = Workspace::new(&root, config)?;
        if config.cli_unstable().avoid_dev_deps {
            ws.set_require_optional_deps(false);
        }
        Ok(ws)
    }

    fn jobs(&self) -> CargoResult<Option<u32>> {
        self.value_of_u32("jobs")
    }

    fn target(&self) -> Option<String> {
        self._value_of("target").map(|s| s.to_string())
    }

    fn compile_options<'a>(
        &self,
        config: &'a Config,
        mode: CompileMode,
    ) -> CargoResult<CompileOptions<'a>> {
        let spec = Packages::from_flags(
            self._is_present("all"),
            self._values_of("exclude"),
            self._values_of("package"),
        )?;

        let message_format = match self._value_of("message-format") {
            None => MessageFormat::Human,
            Some(f) => {
                if f.eq_ignore_ascii_case("json") {
                    MessageFormat::Json
                } else if f.eq_ignore_ascii_case("human") {
                    MessageFormat::Human
                } else if f.eq_ignore_ascii_case("short") {
                    MessageFormat::Short
                } else {
                    panic!("Impossible message format: {:?}", f)
                }
            }
        };

        let mut build_config = BuildConfig::new(config, self.jobs()?, &self.target(), mode)?;
        build_config.message_format = message_format;
        build_config.release = self._is_present("release");
        build_config.build_plan = self._is_present("build-plan");
        if build_config.build_plan && !config.cli_unstable().unstable_options {
            Err(format_err!(
                "`--build-plan` flag is unstable, pass `-Z unstable-options` to enable it"
            ))?;
        };

        let opts = CompileOptions {
            config,
            build_config,
            features: self._values_of("features"),
            all_features: self._is_present("all-features"),
            no_default_features: self._is_present("no-default-features"),
            spec,
            filter: CompileFilter::new(
                self._is_present("lib"),
                self._values_of("bin"),
                self._is_present("bins"),
                self._values_of("test"),
                self._is_present("tests"),
                self._values_of("example"),
                self._is_present("examples"),
                self._values_of("bench"),
                self._is_present("benches"),
                self._is_present("all-targets"),
            ),
            target_rustdoc_args: None,
            target_rustc_args: None,
            export_dir: None,
        };
        Ok(opts)
    }

    fn compile_options_for_single_package<'a>(
        &self,
        config: &'a Config,
        mode: CompileMode,
    ) -> CargoResult<CompileOptions<'a>> {
        let mut compile_opts = self.compile_options(config, mode)?;
        compile_opts.spec = Packages::Packages(self._values_of("package"));
        Ok(compile_opts)
    }

    fn new_options(&self, config: &Config) -> CargoResult<NewOptions> {
        let vcs = self._value_of("vcs").map(|vcs| match vcs {
            "git" => VersionControl::Git,
            "hg" => VersionControl::Hg,
            "pijul" => VersionControl::Pijul,
            "fossil" => VersionControl::Fossil,
            "none" => VersionControl::NoVcs,
            vcs => panic!("Impossible vcs: {:?}", vcs),
        });
        NewOptions::new(
            vcs,
            self._is_present("bin"),
            self._is_present("lib"),
            self.value_of_path("path", config).unwrap(),
            self._value_of("name").map(|s| s.to_string()),
        )
    }

    fn registry(&self, config: &Config) -> CargoResult<Option<String>> {
        match self._value_of("registry") {
            Some(registry) => {
                if !config.cli_unstable().unstable_options {
                    return Err(format_err!(
                        "registry option is an unstable feature and \
                         requires -Zunstable-options to use."
                    ));
                }
                Ok(Some(registry.to_string()))
            }
            None => Ok(None),
        }
    }

    fn index(&self, config: &Config) -> CargoResult<Option<String>> {
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

        let index = match self._value_of("host") {
            Some(host) => {
                config.shell().warn(&msg)?;
                Some(host.to_string())
            }
            None => self._value_of("index").map(|s| s.to_string()),
        };
        Ok(index)
    }

    fn _value_of(&self, name: &str) -> Option<&str>;

    fn _values_of(&self, name: &str) -> Vec<String>;

    fn _is_present(&self, name: &str) -> bool;
}

impl<'a> ArgMatchesExt for ArgMatches<'a> {
    fn _value_of(&self, name: &str) -> Option<&str> {
        self.value_of(name)
    }

    fn _values_of(&self, name: &str) -> Vec<String> {
        self.values_of(name)
            .unwrap_or_default()
            .map(|s| s.to_string())
            .collect()
    }

    fn _is_present(&self, name: &str) -> bool {
        self.is_present(name)
    }
}

pub fn values(args: &ArgMatches, name: &str) -> Vec<String> {
    args.values_of(name)
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect()
}

#[derive(PartialEq, PartialOrd, Eq, Ord)]
pub enum CommandInfo {
    BuiltIn { name: String, about: Option<String>, },
    External { name: String, path: PathBuf },
}

impl CommandInfo {
    pub fn name(&self) -> String {
        match self {
            CommandInfo::BuiltIn { name, .. } => name.to_string(),
            CommandInfo::External { name, .. } => name.to_string(),
        }
    }
}
