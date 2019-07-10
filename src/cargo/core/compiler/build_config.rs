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
    pub build_plan: BuildPlanConfig,
    /// An optional wrapper, if any, used to wrap rustc invocations
    pub rustc_wrapper: Option<ProcessBuilder>,
    pub rustfix_diagnostic_server: RefCell<Option<RustfixDiagnosticServer>>,
    /// Whether or not Cargo should cache compiler output on disk.
    cache_messages: bool,
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
            build_plan: BuildPlanConfig(None),
            rustc_wrapper: None,
            rustfix_diagnostic_server: RefCell::new(None),
            cache_messages: config.cli_unstable().cache_messages,
        })
    }

    /// Whether or not Cargo should cache compiler messages on disk.
    pub fn cache_messages(&self) -> bool {
        self.cache_messages
    }

    /// Whether or not the *user* wants JSON output. Whether or not rustc
    /// actually uses JSON is decided in `add_error_format`.
    pub fn emit_json(&self) -> bool {
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

/// Configuration structure for the `--build-plan` option, mainly a wrapper
/// for the `BuildPlanStage` selected (`None` if we're not generating a build
/// plan, then compile all units as usual).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildPlanConfig(pub Option<BuildPlanStage>);

/// Stage in the build process at which to generate the build plan. At later
/// stages more units will be compiled and less will be outputted to the build
/// plan. In one extreme, `Init` is the earliest possible stage where all units
/// are routed to the build plan; the `None` of this `Option` in
/// `BuildPlanConfig` can be thought of as the other extreme at the end, where
/// all units are already compiled so the build plan is empty (not even
/// generated).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildPlanStage {
    /// Initial stage before anything is compiled (default option), all units
    /// will be outputted as `Invocation`s in the build plan instead of calling
    /// `rustc` for them (actually compiling). Ideally (but this is not
    /// guaranteed), generating a build plan at the `Init` stage shouldn't
    /// change anything in the working environment (e.g., it shouldn't compile,
    /// update fingerprints, remove an old `rlib` file).
    Init,
    /// In contrast to `Init`, this stage signals to output the build plan after
    /// *all* custom build scripts (and their dependencies) have been compiled
    /// and executed.
    ///
    /// At this stage in the build process, the remaining units (which will be
    /// outputted to the build plan instead of being compiled) are the ones
    /// actually linked inside the final binary/library target, whereas the
    /// compiled build scripts (and their dependencies) were used only for
    /// their side effects to modify the `rustc` invocations (which will be
    /// reflected in the build plan, something that doesn't happen at the
    /// `Init` stage when the build script haven't been executed yet).
    PostBuildScripts,
}

impl BuildPlanConfig {
    /// Signal if the `--build-plan` option was requested (independent of the
    /// particular stage selected).
    pub fn requested(&self) -> bool {
        self.0.is_some()
    }

    /// Signal if the `PostBuildScripts` stage was selected for the build plan
    /// (implies `requested`).
    pub fn post_build_scripts(&self) -> bool {
        self.0 == Some(BuildPlanStage::PostBuildScripts)
    }

    /// Determine (based on the build plan stage selected, if any) whether the
    /// current unit should be compiled, or included in the build plan instead
    /// (returning `false`, we use a `bool` because there's no other
    /// alternative of what can happen to a unit). The result is usually stored
    /// in `Unit::to_be_compiled`. Based on these rules:
    /// 1. If we didn't request a build plan, compile *any* unit.
    /// 2. If we requested it at the `Init` stage, compile *nothing*.
    /// 3. If we requested it at the `PostBuildScripts` stage, compile *only* a
    ///    `unit_used_for_build_script` (usually determined by `UnitFor::build`).
    pub fn should_compile_unit(&self, unit_used_for_build_script: bool) -> bool {
        match self.0 {
            None => true,
            Some(BuildPlanStage::Init) => false,
            Some(BuildPlanStage::PostBuildScripts) => unit_used_for_build_script,
        }
    }
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
