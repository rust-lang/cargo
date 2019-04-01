use std::cell::RefCell;
use std::path::Path;

use serde::ser;

use crate::util::ProcessBuilder;
use crate::util::{CargoResult, CargoResultExt, Config, RustfixDiagnosticServer};

/// Configuration information for a rustc build.
#[derive(Debug)]
pub struct BuildConfig {
    /// The target arch triple.
    /// Default: host arch.
    pub requested_target: Option<String>,
    /// Number of rustc jobs to run in parallel.
    pub jobs: u32,
    /// `true` if we are building for release.
    pub release: bool,
    /// The mode we are compiling in.
    pub mode: CompileMode,
    /// `true` to print stdout in JSON format (for machine reading).
    pub message_format: MessageFormat,
    /// Force Cargo to do a full rebuild and treat each target as changed.
    pub force_rebuild: bool,
    /// Output a build plan to stdout instead of actually compiling.
    pub build_plan: bool,
    /// An optional wrapper, if any, used to wrap rustc invocations
    pub rustc_wrapper: Option<ProcessBuilder>,
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
        let requested_target = match requested_target {
            &Some(ref target) if target.ends_with(".json") => {
                let path = Path::new(target).canonicalize().chain_err(|| {
                    failure::format_err!("Target path {:?} is not a valid file", target)
                })?;
                Some(
                    path.into_os_string()
                        .into_string()
                        .map_err(|_| failure::format_err!("Target path is not valid unicode"))?,
                )
            }
            other => other.clone(),
        };
        if let Some(ref s) = requested_target {
            if s.trim().is_empty() {
                failure::bail!("target was empty")
            }
        }
        let cfg_target = match config.get_string("build.target")? {
            Some(ref target) if target.val.ends_with(".json") => {
                let path = target.definition.root(config).join(&target.val);
                let path_string = path
                    .into_os_string()
                    .into_string()
                    .map_err(|_| failure::format_err!("Target path is not valid unicode"));
                Some(path_string?)
            }
            other => other.map(|t| t.val),
        };
        let target = requested_target.or(cfg_target);

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
        let cfg_jobs: Option<u32> = config.get("build.jobs")?;
        let jobs = jobs.or(cfg_jobs).unwrap_or(::num_cpus::get() as u32);
        Ok(BuildConfig {
            requested_target: target,
            jobs,
            release: false,
            mode,
            message_format: MessageFormat::Human,
            force_rebuild: false,
            build_plan: false,
            rustc_wrapper: None,
            rustfix_diagnostic_server: RefCell::new(None),
        })
    }

    pub fn json_messages(&self) -> bool {
        self.message_format == MessageFormat::Json
    }

    pub fn test(&self) -> bool {
        self.mode == CompileMode::Test || self.mode == CompileMode::Bench
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFormat {
    Human,
    Json,
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
    /// `Target`, because it is essentially the same as `Test` (indicating
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

    /// Returns `true` if this is a doc or doc test. Be careful using this.
    /// Although both run rustdoc, the dependencies for those two modes are
    /// very different.
    pub fn is_doc(self) -> bool {
        match self {
            CompileMode::Doc { .. } | CompileMode::Doctest => true,
            _ => false,
        }
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
