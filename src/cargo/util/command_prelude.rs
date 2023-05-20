use crate::core::compiler::{BuildConfig, MessageFormat, TimingOutput};
use crate::core::resolver::CliFeatures;
use crate::core::{Edition, Workspace};
use crate::ops::{CompileFilter, CompileOptions, NewOptions, Packages, VersionControl};
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::interning::InternedString;
use crate::util::restricted_names::is_glob_pattern;
use crate::util::toml::{StringOrVec, TomlProfile};
use crate::util::validate_package_name;
use crate::util::{
    print_available_benches, print_available_binaries, print_available_examples,
    print_available_packages, print_available_tests,
};
use crate::CargoResult;
use anyhow::bail;
use cargo_util::paths;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub use crate::core::compiler::CompileMode;
pub use crate::{CliError, CliResult, Config};
pub use clap::{value_parser, Arg, ArgAction, ArgMatches};

pub use clap::Command;

pub trait CommandExt: Sized {
    fn _arg(self, arg: Arg) -> Self;

    /// Do not use this method, it is only for backwards compatibility.
    /// Use `arg_package_spec_no_all` instead.
    fn arg_package_spec(
        self,
        package: &'static str,
        all: &'static str,
        exclude: &'static str,
    ) -> Self {
        self.arg_package_spec_no_all(package, all, exclude)
            ._arg(flag("all", "Alias for --workspace (deprecated)"))
    }

    /// Variant of arg_package_spec that does not include the `--all` flag
    /// (but does include `--workspace`). Used to avoid confusion with
    /// historical uses of `--all`.
    fn arg_package_spec_no_all(
        self,
        package: &'static str,
        all: &'static str,
        exclude: &'static str,
    ) -> Self {
        self.arg_package_spec_simple(package)
            ._arg(flag("workspace", all))
            ._arg(multi_opt("exclude", "SPEC", exclude))
    }

    fn arg_package_spec_simple(self, package: &'static str) -> Self {
        self._arg(optional_multi_opt("package", "SPEC", package).short('p'))
    }

    fn arg_package(self, package: &'static str) -> Self {
        self._arg(
            optional_opt("package", package)
                .short('p')
                .value_name("SPEC"),
        )
    }

    fn arg_jobs(self) -> Self {
        self._arg(
            opt("jobs", "Number of parallel jobs, defaults to # of CPUs")
                .short('j')
                .value_name("N")
                .allow_hyphen_values(true),
        )
        ._arg(flag(
            "keep-going",
            "Do not abort the build as soon as there is an error (unstable)",
        ))
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
        self.arg_targets_lib_bin_example(lib, bin, bins, example, examples)
            ._arg(flag("tests", tests))
            ._arg(optional_multi_opt("test", "NAME", test))
            ._arg(flag("benches", benches))
            ._arg(optional_multi_opt("bench", "NAME", bench))
            ._arg(flag("all-targets", all))
    }

    fn arg_targets_lib_bin_example(
        self,
        lib: &'static str,
        bin: &'static str,
        bins: &'static str,
        example: &'static str,
        examples: &'static str,
    ) -> Self {
        self._arg(flag("lib", lib))
            ._arg(flag("bins", bins))
            ._arg(optional_multi_opt("bin", "NAME", bin))
            ._arg(flag("examples", examples))
            ._arg(optional_multi_opt("example", "NAME", example))
    }

    fn arg_targets_bins_examples(
        self,
        bin: &'static str,
        bins: &'static str,
        example: &'static str,
        examples: &'static str,
    ) -> Self {
        self._arg(optional_multi_opt("bin", "NAME", bin))
            ._arg(flag("bins", bins))
            ._arg(optional_multi_opt("example", "NAME", example))
            ._arg(flag("examples", examples))
    }

    fn arg_targets_bin_example(self, bin: &'static str, example: &'static str) -> Self {
        self._arg(optional_multi_opt("bin", "NAME", bin))
            ._arg(optional_multi_opt("example", "NAME", example))
    }

    fn arg_features(self) -> Self {
        self._arg(
            multi_opt(
                "features",
                "FEATURES",
                "Space or comma separated list of features to activate",
            )
            .short('F'),
        )
        ._arg(flag("all-features", "Activate all available features"))
        ._arg(flag(
            "no-default-features",
            "Do not activate the `default` feature",
        ))
    }

    fn arg_release(self, release: &'static str) -> Self {
        self._arg(flag("release", release).short('r'))
    }

    fn arg_profile(self, profile: &'static str) -> Self {
        self._arg(opt("profile", profile).value_name("PROFILE-NAME"))
    }

    fn arg_doc(self, doc: &'static str) -> Self {
        self._arg(flag("doc", doc))
    }

    fn arg_target_triple(self, target: &'static str) -> Self {
        self._arg(multi_opt("target", "TRIPLE", target))
    }

    fn arg_target_dir(self) -> Self {
        self._arg(
            opt("target-dir", "Directory for all generated artifacts").value_name("DIRECTORY"),
        )
    }

    fn arg_manifest_path(self) -> Self {
        self._arg(opt("manifest-path", "Path to Cargo.toml").value_name("PATH"))
    }

    fn arg_message_format(self) -> Self {
        self._arg(multi_opt("message-format", "FMT", "Error format"))
    }

    fn arg_build_plan(self) -> Self {
        self._arg(flag(
            "build-plan",
            "Output the build plan in JSON (unstable)",
        ))
    }

    fn arg_unit_graph(self) -> Self {
        self._arg(flag("unit-graph", "Output build graph in JSON (unstable)"))
    }

    fn arg_new_opts(self) -> Self {
        self._arg(
            opt(
                "vcs",
                "Initialize a new repository for the given version \
                 control system (git, hg, pijul, or fossil) or do not \
                 initialize any version control at all (none), overriding \
                 a global configuration.",
            )
            .value_name("VCS")
            .value_parser(["git", "hg", "pijul", "fossil", "none"]),
        )
        ._arg(flag("bin", "Use a binary (application) template [default]"))
        ._arg(flag("lib", "Use a library template"))
        ._arg(
            opt("edition", "Edition to set for the crate generated")
                .value_parser(Edition::CLI_VALUES)
                .value_name("YEAR"),
        )
        ._arg(
            opt(
                "name",
                "Set the resulting package name, defaults to the directory name",
            )
            .value_name("NAME"),
        )
    }

    fn arg_index(self) -> Self {
        self._arg(opt("index", "Registry index URL to upload the package to").value_name("INDEX"))
    }

    fn arg_dry_run(self, dry_run: &'static str) -> Self {
        self._arg(flag("dry-run", dry_run))
    }

    fn arg_ignore_rust_version(self) -> Self {
        self._arg(flag(
            "ignore-rust-version",
            "Ignore `rust-version` specification in packages",
        ))
    }

    fn arg_future_incompat_report(self) -> Self {
        self._arg(flag(
            "future-incompat-report",
            "Outputs a future incompatibility report at the end of the build",
        ))
    }

    fn arg_quiet(self) -> Self {
        self._arg(flag("quiet", "Do not print cargo log messages").short('q'))
    }

    fn arg_timings(self) -> Self {
        self._arg(
            optional_opt(
                "timings",
                "Timing output formats (unstable) (comma separated): html, json",
            )
            .value_name("FMTS")
            .require_equals(true),
        )
    }
}

impl CommandExt for Command {
    fn _arg(self, arg: Arg) -> Self {
        self.arg(arg)
    }
}

pub fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .help(help)
        .action(ArgAction::SetTrue)
}

pub fn opt(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).help(help).action(ArgAction::Set)
}

pub fn optional_opt(name: &'static str, help: &'static str) -> Arg {
    opt(name, help).num_args(0..=1)
}

pub fn optional_multi_opt(name: &'static str, value_name: &'static str, help: &'static str) -> Arg {
    opt(name, help)
        .value_name(value_name)
        .num_args(0..=1)
        .action(ArgAction::Append)
}

pub fn multi_opt(name: &'static str, value_name: &'static str, help: &'static str) -> Arg {
    opt(name, help)
        .value_name(value_name)
        .action(ArgAction::Append)
}

pub fn subcommand(name: &'static str) -> Command {
    Command::new(name)
}

/// Determines whether or not to gate `--profile` as unstable when resolving it.
pub enum ProfileChecking {
    /// `cargo rustc` historically has allowed "test", "bench", and "check". This
    /// variant explicitly allows those.
    LegacyRustc,
    /// `cargo check` and `cargo fix` historically has allowed "test". This variant
    /// explicitly allows that on stable.
    LegacyTestOnly,
    /// All other commands, which allow any valid custom named profile.
    Custom,
}

pub trait ArgMatchesExt {
    fn value_of_u32(&self, name: &str) -> CargoResult<Option<u32>> {
        let arg = match self._value_of(name) {
            None => None,
            Some(arg) => Some(arg.parse::<u32>().map_err(|_| {
                clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    format!("Invalid value: could not parse `{}` as a number", arg),
                )
            })?),
        };
        Ok(arg)
    }

    fn value_of_i32(&self, name: &str) -> CargoResult<Option<i32>> {
        let arg = match self._value_of(name) {
            None => None,
            Some(arg) => Some(arg.parse::<i32>().map_err(|_| {
                clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    format!("Invalid value: could not parse `{}` as a number", arg),
                )
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
                anyhow::bail!("the manifest-path must be a path to a Cargo.toml file")
            }
            if !path.exists() {
                anyhow::bail!(
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

    fn jobs(&self) -> CargoResult<Option<i32>> {
        self.value_of_i32("jobs")
    }

    fn verbose(&self) -> u32 {
        self._count("verbose")
    }

    fn dry_run(&self) -> bool {
        self.flag("dry-run")
    }

    fn keep_going(&self) -> bool {
        self.flag("keep-going")
    }

    fn targets(&self) -> Vec<String> {
        self._values_of("target")
    }

    fn get_profile_name(
        &self,
        config: &Config,
        default: &str,
        profile_checking: ProfileChecking,
    ) -> CargoResult<InternedString> {
        let specified_profile = self._value_of("profile");

        // Check for allowed legacy names.
        // This is an early exit, since it allows combination with `--release`.
        match (specified_profile, profile_checking) {
            // `cargo rustc` has legacy handling of these names
            (Some(name @ ("dev" | "test" | "bench" | "check")), ProfileChecking::LegacyRustc)
            // `cargo fix` and `cargo check` has legacy handling of this profile name
            | (Some(name @ "test"), ProfileChecking::LegacyTestOnly) => {
                if self.flag("release") {
                    config.shell().warn(
                        "the `--release` flag should not be specified with the `--profile` flag\n\
                         The `--release` flag will be ignored.\n\
                         This was historically accepted, but will become an error \
                         in a future release."
                    )?;
                }
                return Ok(InternedString::new(name));
            }
            _ => {}
        }

        let conflict = |flag: &str, equiv: &str, specified: &str| -> anyhow::Error {
            anyhow::format_err!(
                "conflicting usage of --profile={} and --{flag}\n\
                 The `--{flag}` flag is the same as `--profile={equiv}`.\n\
                 Remove one flag or the other to continue.",
                specified,
                flag = flag,
                equiv = equiv
            )
        };

        let name = match (self.flag("release"), self.flag("debug"), specified_profile) {
            (false, false, None) => default,
            (true, _, None | Some("release")) => "release",
            (true, _, Some(name)) => return Err(conflict("release", "release", name)),
            (_, true, None | Some("dev")) => "dev",
            (_, true, Some(name)) => return Err(conflict("debug", "dev", name)),
            // `doc` is separate from all the other reservations because
            // [profile.doc] was historically allowed, but is deprecated and
            // has no effect. To avoid potentially breaking projects, it is a
            // warning in Cargo.toml, but since `--profile` is new, we can
            // reject it completely here.
            (_, _, Some("doc")) => {
                bail!("profile `doc` is reserved and not allowed to be explicitly specified")
            }
            (_, _, Some(name)) => {
                TomlProfile::validate_name(name)?;
                name
            }
        };

        Ok(InternedString::new(name))
    }

    fn packages_from_flags(&self) -> CargoResult<Packages> {
        Packages::from_flags(
            // TODO Integrate into 'workspace'
            self.flag("workspace") || self.flag("all"),
            self._values_of("exclude"),
            self._values_of("package"),
        )
    }

    fn compile_options(
        &self,
        config: &Config,
        mode: CompileMode,
        workspace: Option<&Workspace<'_>>,
        profile_checking: ProfileChecking,
    ) -> CargoResult<CompileOptions> {
        let spec = self.packages_from_flags()?;
        let mut message_format = None;
        let default_json = MessageFormat::Json {
            short: false,
            ansi: false,
            render_diagnostics: false,
        };
        let two_kinds_of_msg_format_err = "cannot specify two kinds of `message-format` arguments";
        for fmt in self._values_of("message-format") {
            for fmt in fmt.split(',') {
                let fmt = fmt.to_ascii_lowercase();
                match fmt.as_str() {
                    "json" => {
                        if message_format.is_some() {
                            bail!(two_kinds_of_msg_format_err);
                        }
                        message_format = Some(default_json);
                    }
                    "human" => {
                        if message_format.is_some() {
                            bail!(two_kinds_of_msg_format_err);
                        }
                        message_format = Some(MessageFormat::Human);
                    }
                    "short" => {
                        if message_format.is_some() {
                            bail!(two_kinds_of_msg_format_err);
                        }
                        message_format = Some(MessageFormat::Short);
                    }
                    "json-render-diagnostics" => {
                        if message_format.is_none() {
                            message_format = Some(default_json);
                        }
                        match &mut message_format {
                            Some(MessageFormat::Json {
                                render_diagnostics, ..
                            }) => *render_diagnostics = true,
                            _ => bail!(two_kinds_of_msg_format_err),
                        }
                    }
                    "json-diagnostic-short" => {
                        if message_format.is_none() {
                            message_format = Some(default_json);
                        }
                        match &mut message_format {
                            Some(MessageFormat::Json { short, .. }) => *short = true,
                            _ => bail!(two_kinds_of_msg_format_err),
                        }
                    }
                    "json-diagnostic-rendered-ansi" => {
                        if message_format.is_none() {
                            message_format = Some(default_json);
                        }
                        match &mut message_format {
                            Some(MessageFormat::Json { ansi, .. }) => *ansi = true,
                            _ => bail!(two_kinds_of_msg_format_err),
                        }
                    }
                    s => bail!("invalid message format specifier: `{}`", s),
                }
            }
        }

        let mut build_config = BuildConfig::new(
            config,
            self.jobs()?,
            self.keep_going(),
            &self.targets(),
            mode,
        )?;
        build_config.message_format = message_format.unwrap_or(MessageFormat::Human);
        build_config.requested_profile = self.get_profile_name(config, "dev", profile_checking)?;
        build_config.build_plan = self.flag("build-plan");
        build_config.unit_graph = self.flag("unit-graph");
        build_config.future_incompat_report = self.flag("future-incompat-report");

        if self._contains("timings") {
            for timing_output in self._values_of("timings") {
                for timing_output in timing_output.split(',') {
                    let timing_output = timing_output.to_ascii_lowercase();
                    let timing_output = match timing_output.as_str() {
                        "html" => {
                            config
                                .cli_unstable()
                                .fail_if_stable_opt("--timings=html", 7405)?;
                            TimingOutput::Html
                        }
                        "json" => {
                            config
                                .cli_unstable()
                                .fail_if_stable_opt("--timings=json", 7405)?;
                            TimingOutput::Json
                        }
                        s => bail!("invalid timings output specifier: `{}`", s),
                    };
                    build_config.timing_outputs.push(timing_output);
                }
            }
            if build_config.timing_outputs.is_empty() {
                build_config.timing_outputs.push(TimingOutput::Html);
            }
        }

        if build_config.keep_going {
            config
                .cli_unstable()
                .fail_if_stable_opt("--keep-going", 10496)?;
        }
        if build_config.build_plan {
            config
                .cli_unstable()
                .fail_if_stable_opt("--build-plan", 5579)?;
        };
        if build_config.unit_graph {
            config
                .cli_unstable()
                .fail_if_stable_opt("--unit-graph", 8002)?;
        }

        let opts = CompileOptions {
            build_config,
            cli_features: self.cli_features()?,
            spec,
            filter: CompileFilter::from_raw_arguments(
                self.flag("lib"),
                self._values_of("bin"),
                self.flag("bins"),
                self._values_of("test"),
                self.flag("tests"),
                self._values_of("example"),
                self.flag("examples"),
                self._values_of("bench"),
                self.flag("benches"),
                self.flag("all-targets"),
            ),
            target_rustdoc_args: None,
            target_rustc_args: None,
            target_rustc_crate_types: None,
            rustdoc_document_private_items: false,
            honor_rust_version: !self.flag("ignore-rust-version"),
        };

        if let Some(ws) = workspace {
            self.check_optional_opts(ws, &opts)?;
        } else if self.is_present_with_zero_values("package") {
            // As for cargo 0.50.0, this won't occur but if someone sneaks in
            // we can still provide this informative message for them.
            anyhow::bail!(
                "\"--package <SPEC>\" requires a SPEC format value, \
                which can be any package ID specifier in the dependency graph.\n\
                Run `cargo help pkgid` for more information about SPEC format."
            )
        }

        Ok(opts)
    }

    fn cli_features(&self) -> CargoResult<CliFeatures> {
        CliFeatures::from_command_line(
            &self._values_of("features"),
            self.flag("all-features"),
            !self.flag("no-default-features"),
        )
    }

    fn compile_options_for_single_package(
        &self,
        config: &Config,
        mode: CompileMode,
        workspace: Option<&Workspace<'_>>,
        profile_checking: ProfileChecking,
    ) -> CargoResult<CompileOptions> {
        let mut compile_opts = self.compile_options(config, mode, workspace, profile_checking)?;
        let spec = self._values_of("package");
        if spec.iter().any(is_glob_pattern) {
            anyhow::bail!("Glob patterns on package selection are not supported.")
        }
        compile_opts.spec = Packages::Packages(spec);
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
            self.flag("bin"),
            self.flag("lib"),
            self.value_of_path("path", config).unwrap(),
            self._value_of("name").map(|s| s.to_string()),
            self._value_of("edition").map(|s| s.to_string()),
            self.registry(config)?,
        )
    }

    fn registry(&self, config: &Config) -> CargoResult<Option<String>> {
        let registry = self._value_of("registry");
        let index = self._value_of("index");
        let result = match (registry, index) {
            (None, None) => config.default_registry()?,
            (None, Some(_)) => {
                // If --index is set, then do not look at registry.default.
                None
            }
            (Some(r), None) => {
                validate_package_name(r, "registry name", "")?;
                Some(r.to_string())
            }
            (Some(_), Some(_)) => {
                bail!("both `--index` and `--registry` should not be set at the same time")
            }
        };
        Ok(result)
    }

    fn index(&self) -> CargoResult<Option<String>> {
        let index = self._value_of("index").map(|s| s.to_string());
        Ok(index)
    }

    fn check_optional_opts(
        &self,
        workspace: &Workspace<'_>,
        compile_opts: &CompileOptions,
    ) -> CargoResult<()> {
        if self.is_present_with_zero_values("package") {
            print_available_packages(workspace)?
        }

        if self.is_present_with_zero_values("example") {
            print_available_examples(workspace, compile_opts)?;
        }

        if self.is_present_with_zero_values("bin") {
            print_available_binaries(workspace, compile_opts)?;
        }

        if self.is_present_with_zero_values("bench") {
            print_available_benches(workspace, compile_opts)?;
        }

        if self.is_present_with_zero_values("test") {
            print_available_tests(workspace, compile_opts)?;
        }

        Ok(())
    }

    fn is_present_with_zero_values(&self, name: &str) -> bool {
        self._contains(name) && self._value_of(name).is_none()
    }

    fn flag(&self, name: &str) -> bool;

    fn _value_of(&self, name: &str) -> Option<&str>;

    fn _values_of(&self, name: &str) -> Vec<String>;

    fn _value_of_os(&self, name: &str) -> Option<&OsStr>;

    fn _values_of_os(&self, name: &str) -> Vec<OsString>;

    fn _count(&self, name: &str) -> u32;

    fn _contains(&self, name: &str) -> bool;
}

impl<'a> ArgMatchesExt for ArgMatches {
    fn flag(&self, name: &str) -> bool {
        ignore_unknown(self.try_get_one::<bool>(name))
            .copied()
            .unwrap_or(false)
    }

    fn _value_of(&self, name: &str) -> Option<&str> {
        ignore_unknown(self.try_get_one::<String>(name)).map(String::as_str)
    }

    fn _value_of_os(&self, name: &str) -> Option<&OsStr> {
        ignore_unknown(self.try_get_one::<OsString>(name)).map(OsString::as_os_str)
    }

    fn _values_of(&self, name: &str) -> Vec<String> {
        ignore_unknown(self.try_get_many::<String>(name))
            .unwrap_or_default()
            .cloned()
            .collect()
    }

    fn _values_of_os(&self, name: &str) -> Vec<OsString> {
        ignore_unknown(self.try_get_many::<OsString>(name))
            .unwrap_or_default()
            .cloned()
            .collect()
    }

    fn _count(&self, name: &str) -> u32 {
        *ignore_unknown(self.try_get_one::<u8>(name)).expect("defaulted by clap") as u32
    }

    fn _contains(&self, name: &str) -> bool {
        ignore_unknown(self.try_contains_id(name))
    }
}

pub fn values(args: &ArgMatches, name: &str) -> Vec<String> {
    args._values_of(name)
}

pub fn values_os(args: &ArgMatches, name: &str) -> Vec<OsString> {
    args._values_of_os(name)
}

#[track_caller]
pub fn ignore_unknown<T: Default>(r: Result<T, clap::parser::MatchesError>) -> T {
    match r {
        Ok(t) => t,
        Err(clap::parser::MatchesError::UnknownArgument { .. }) => Default::default(),
        Err(e) => {
            panic!("Mismatch between definition and access: {}", e);
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandInfo {
    BuiltIn { about: Option<String> },
    External { path: PathBuf },
    Alias { target: StringOrVec },
}
