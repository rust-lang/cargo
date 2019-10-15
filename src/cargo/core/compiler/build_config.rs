use std::cell::RefCell;

use serde::ser;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::util::ProcessBuilder;
use crate::util::{CargoResult, Config, RustfixDiagnosticServer};

#[derive(Debug, Clone)]
pub enum ProfileKind {
    Dev,
    Release,
    Custom(String),
}

impl ProfileKind {
    pub fn name(&self) -> &str {
        match self {
            ProfileKind::Dev => "dev",
            ProfileKind::Release => "release",
            ProfileKind::Custom(name) => name,
        }
    }
}

/// Configuration information for a rustc build.
#[derive(Debug)]
pub struct BuildConfig {
    /// The requested kind of compilation for this session
    pub requested_kind: CompileKind,
    /// Number of rustc jobs to run in parallel.
    pub jobs: u32,
    /// Build profile
    pub profile_kind: ProfileKind,
    /// The mode we are compiling in.
    pub mode: CompileMode,
    /// `true` to print stdout in JSON format (for machine reading).
    pub message_format: MessageFormat,
    /// Force Cargo to do a full rebuild and treat each target as changed.
    pub force_rebuild: bool,
    /// Output a build plan to stdout instead of actually compiling.
    pub build_plan: bool,
    /// An optional override of the rustc path for primary units only
    pub primary_unit_rustc: Option<ProcessBuilder>,
    pub rustfix_diagnostic_server: RefCell<Option<RustfixDiagnosticServer>>,
}

impl BuildConfig {
    /// Parses all config files to learn about build configuration. Currently
    /// configured options are:
    ///
    /// * `build.jobs`
    /// * `build.target`
    /// * `target.$target.ar`
    /// * `target.$target.linker`
    /// * `target.$target.libfoo.metadata`
    pub fn new(
        config: &Config,
        jobs: Option<u32>,
        requested_target: &Option<String>,
        mode: CompileMode,
    ) -> CargoResult<BuildConfig> {
        let cfg = config.build_config()?;
        let requested_kind = match requested_target {
            Some(s) => CompileKind::Target(CompileTarget::new(s)?),
            None => match &cfg.target {
                Some(val) => {
                    let value = if val.raw_value().ends_with(".json") {
                        let path = val.clone().resolve_path(config);
                        path.to_str().expect("must be utf-8 in toml").to_string()
                    } else {
                        val.raw_value().to_string()
                    };
                    CompileKind::Target(CompileTarget::new(&value)?)
                }
                None => CompileKind::Host,
            },
        };

        if jobs == Some(0) {
            failure::bail!("jobs must be at least 1")
        }
        if jobs.is_some() && config.jobserver_from_env().is_some() {
            config.shell().warn(
                "a `-j` argument was passed to Cargo but Cargo is \
                 also configured with an external jobserver in \
                 its environment, ignoring the `-j` parameter",
            )?;
        }
        let jobs = jobs.or(cfg.jobs).unwrap_or(::num_cpus::get() as u32);

        Ok(BuildConfig {
            requested_kind,
            jobs,
            profile_kind: ProfileKind::Dev,
            mode,
            message_format: MessageFormat::Human,
            force_rebuild: false,
            build_plan: false,
            primary_unit_rustc: None,
            rustfix_diagnostic_server: RefCell::new(None),
        })
    }

    /// Whether or not the *user* wants JSON output. Whether or not rustc
    /// actually uses JSON is decided in `add_error_format`.
    pub fn emit_json(&self) -> bool {
        match self.message_format {
            MessageFormat::Json { .. } => true,
            _ => false,
        }
    }

    pub fn profile_name(&self) -> &str {
        self.profile_kind.name()
    }

    pub fn test(&self) -> bool {
        self.mode == CompileMode::Test || self.mode == CompileMode::Bench
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFormat {
    Human,
    Json {
        /// Whether rustc diagnostics are rendered by cargo or included into the
        /// output stream.
        render_diagnostics: bool,
        /// Whether the `rendered` field of rustc diagnostics are using the
        /// "short" rendering.
        short: bool,
        /// Whether the `rendered` field of rustc diagnostics embed ansi color
        /// codes.
        ansi: bool,
    },
    Short,
}

/// The general "mode" for what to do.
/// This is used for two purposes. The commands themselves pass this in to
/// `compile_ws` to tell it the general execution strategy. This influences
/// the default targets selected. The other use is in the `Unit` struct
/// to indicate what is being done with a specific target.
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash, PartialOrd, Ord)]
pub enum CompileMode {
    /// A target being built for a test.
    Test,
    /// Building a target with `rustc` (lib or bin).
    Build,
    /// Building a target with `rustc` to emit `rmeta` metadata only. If
    /// `test` is true, then it is also compiled with `--test` to check it like
    /// a test.
    Check { test: bool },
    /// Used to indicate benchmarks should be built. This is not used in
    /// `Unit`, because it is essentially the same as `Test` (indicating
    /// `--test` should be passed to rustc) and by using `Test` instead it
    /// allows some de-duping of Units to occur.
    Bench,
    /// A target that will be documented with `rustdoc`.
    /// If `deps` is true, then it will also document all dependencies.
    Doc { deps: bool },
    /// A target that will be tested with `rustdoc`.
    Doctest,
    /// A marker for Units that represent the execution of a `build.rs` script.
    RunCustomBuild,
}

impl ser::Serialize for CompileMode {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::CompileMode::*;
        match *self {
            Test => "test".serialize(s),
            Build => "build".serialize(s),
            Check { .. } => "check".serialize(s),
            Bench => "bench".serialize(s),
            Doc { .. } => "doc".serialize(s),
            Doctest => "doctest".serialize(s),
            RunCustomBuild => "run-custom-build".serialize(s),
        }
    }
}

impl CompileMode {
    /// Returns `true` if the unit is being checked.
    pub fn is_check(self) -> bool {
        match self {
            CompileMode::Check { .. } => true,
            _ => false,
        }
    }

    /// Returns `true` if this is generating documentation.
    pub fn is_doc(self) -> bool {
        match self {
            CompileMode::Doc { .. } => true,
            _ => false,
        }
    }

    /// Returns `true` if this a doc test.
    pub fn is_doc_test(self) -> bool {
        self == CompileMode::Doctest
    }

    /// Returns `true` if this is any type of test (test, benchmark, doc test, or
    /// check test).
    pub fn is_any_test(self) -> bool {
        match self {
            CompileMode::Test
            | CompileMode::Bench
            | CompileMode::Check { test: true }
            | CompileMode::Doctest => true,
            _ => false,
        }
    }

    /// Returns `true` if this is something that passes `--test` to rustc.
    pub fn is_rustc_test(self) -> bool {
        match self {
            CompileMode::Test | CompileMode::Bench | CompileMode::Check { test: true } => true,
            _ => false,
        }
    }

    /// Returns `true` if this is the *execution* of a `build.rs` script.
    pub fn is_run_custom_build(self) -> bool {
        self == CompileMode::RunCustomBuild
    }

    /// List of all modes (currently used by `cargo clean -p` for computing
    /// all possible outputs).
    pub fn all_modes() -> &'static [CompileMode] {
        static ALL: [CompileMode; 9] = [
            CompileMode::Test,
            CompileMode::Build,
            CompileMode::Check { test: true },
            CompileMode::Check { test: false },
            CompileMode::Bench,
            CompileMode::Doc { deps: true },
            CompileMode::Doc { deps: false },
            CompileMode::Doctest,
            CompileMode::RunCustomBuild,
        ];
        &ALL
    }
}
